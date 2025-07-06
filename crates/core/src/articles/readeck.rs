use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, File},
    io::Write,
    ops::Deref,
    sync::{
        atomic::{
            AtomicBool,
            Ordering::{Acquire, Release},
        },
        Arc, Mutex,
    },
    thread,
};

use fxhash::FxHashSet;
use serde::{Deserialize, Serialize};
use ureq::Agent;

use crate::{
    articles::{
        queue_link, read_queued, save_index, Article, ArticleIndex, Changes, Service, ARTICLES_DIR,
    },
    settings::ArticleAuth,
    view::{ArticleUpdateProgress, Event, Hub},
};

pub struct Readeck {
    auth: ArticleAuth,
    index: Arc<Mutex<ArticleIndex>>,
    updating: Arc<AtomicBool>,
}

impl Readeck {
    pub fn load(auth: ArticleAuth, index: ArticleIndex) -> Readeck {
        Readeck {
            auth: auth,
            index: Arc::new(Mutex::new(index)),
            updating: Arc::new(AtomicBool::new(false)),
        }
    }
}

impl Service for Readeck {
    fn index(&self) -> Arc<Mutex<ArticleIndex>> {
        self.index.clone()
    }

    fn save_index(&self) {
        let index = self.index.lock().unwrap();
        if let Err(err) = save_index(index.deref()) {
            eprintln!("failed to save index: {}", err);
        };
    }

    fn update(&mut self, hub: &crate::view::Hub) -> bool {
        if self.updating.swap(true, Acquire) {
            return false;
        }
        hub.send(Event::ArticleUpdateProgress(
            ArticleUpdateProgress::ListStart,
        ))
        .ok();
        let hub = hub.clone();
        let auth = self.auth.clone();
        let updating = self.updating.clone();
        let index = self.index.clone();
        thread::spawn(move || {
            if let Err(err) = update(&hub, auth, index) {
                eprintln!("while fetching article list: {err}");
                hub.send(Event::Notify(err)).ok();
            };
            hub.send(Event::ArticleUpdateProgress(ArticleUpdateProgress::Finish))
                .ok();
            updating.store(false, Release);
        });
        return true;
    }
}

#[derive(Serialize)]
struct APIAuth {
    application: String,
    username: String,
    password: String,
}

#[derive(Deserialize)]
struct APIAuthResponse {
    token: String,
}

#[derive(Deserialize)]
struct APIBookmarks {
    id: String,
    created: String,
    loaded: bool,
    title: String,
    site_name: String,
    authors: Option<Vec<String>>,
    lang: String,
    has_article: bool,
    is_marked: bool,
    is_archived: bool,
    reading_time: Option<u32>,
}

#[derive(Serialize)]
struct APIBookmarkUpdate {
    is_archived: Option<bool>,
    is_marked: Option<bool>,
}

pub fn authenticate(
    server: String,
    client_name: String,
    username: String,
    password: String,
) -> Result<ArticleAuth, String> {
    let url = url_from_server(&server);
    let agent: Agent = Agent::config_builder().max_redirects(0).build().into();

    let login_url = url.to_owned() + "api/auth";
    let mut response = match agent
        .post(&login_url)
        .content_type("application/json")
        .send_json(APIAuth {
            application: client_name,
            username: username,
            password: password,
        }) {
        Ok(response) => response,
        Err(ureq::Error::Io(_)) => {
            // Any I/O error, but most likely there's a networking issuing.
            return Err("failed to connect to the server".to_string());
        }
        Err(ureq::Error::StatusCode(404)) => {
            // Special case: provide better error message for invalid
            // (mistyped?) addresses.
            return Err(format!("login page does not exist: {login_url}"));
        }
        Err(err) => {
            return Err(format!("could not fetch login page: {err}"));
        }
    };
    if response.status().is_redirection() {
        return Err(format!("login page does not exist: {login_url}"));
    }

    let response_body = response
        .body_mut()
        .read_json::<APIAuthResponse>()
        .map_err(|err| format!("could not fetch authentication response: {err}"))?;
    Ok(ArticleAuth {
        api: "readeck".to_string(),
        server: server,
        access_token: response_body.token,
        ..Default::default()
    })
}

fn update(hub: &Hub, auth: ArticleAuth, index: Arc<Mutex<ArticleIndex>>) -> Result<(), String> {
    let url = url_from_server(&auth.server);
    let agent: Agent = Agent::config_builder().max_redirects(0).build().into();

    // Submit new URLs.
    let queued = read_queued();
    if !queued.is_empty() {
        if let Err(err) = agent
            .post(format!("{url}bookmarks/import/text"))
            .header("Authorization", "Bearer ".to_owned() + &auth.access_token)
            .send_form([("data", queued.join("\n"))])
        {
            // Add the links back (this is inefficient, but it should work).
            for link in queued {
                queue_link(link);
            }

            return Err(format!("submitting articles failed: {err}"));
        };
    }

    // Sync local changes.
    let mut changes: BTreeMap<String, APIBookmarkUpdate> = BTreeMap::new();
    let mut deleted: BTreeSet<String> = BTreeSet::new();
    for (id, article) in index.lock().unwrap().articles.iter_mut() {
        if article.changed.contains(&Changes::Deleted) {
            deleted.insert(id.clone());
            continue;
        }
        let update = APIBookmarkUpdate {
            is_marked: if article.changed.contains(&Changes::Starred) {
                Some(article.starred)
            } else {
                None
            },
            is_archived: if article.changed.contains(&Changes::Archived) {
                Some(article.archived)
            } else {
                None
            },
        };
        if update.is_marked.is_some() || update.is_archived.is_some() {
            changes.insert(id.clone(), update);
        }
    }
    for id in deleted {
        match agent
            .delete(format!("{url}api/bookmarks/{id}"))
            .header("Authorization", "Bearer ".to_owned() + &auth.access_token)
            .header("accept", "application/json")
            .call()
        {
            Ok(_) | Err(ureq::Error::StatusCode(404)) => {
                // Either successfully deleted or the article was already
                // deleted on the server, so we can remove the entry locally.
                index.lock().unwrap().articles.remove(&id);
            }
            Err(err) => {
                return Err(format!("deleting article failed: {err}"));
            }
        };
    }
    for (id, update) in changes {
        match agent
            .patch(format!("{url}api/bookmarks/{id}"))
            .header("Authorization", "Bearer ".to_owned() + &auth.access_token)
            .header("accept", "application/json")
            .content_type("application/json")
            .send_json(update)
        {
            Ok(_) => {
                // Change was successfully sent, so we can remove the change
                // flags.
                if let Some(article) = index.lock().unwrap().articles.get_mut(&id) {
                    article.changed.remove(&Changes::Starred);
                    article.changed.remove(&Changes::Archived);
                }
            }
            Err(ureq::Error::StatusCode(404)) => {
                // Article was deleted on the server.
                // We'll just let it as-is, the article will be removed locally
                // when updating the list of articles.
            }
            Err(err) => {
                return Err(format!("sending local changes failed: {err}"));
            }
        };
    }

    // Create articles directory if it doesn't exist yet.
    std::fs::create_dir(ARTICLES_DIR).ok();

    // Fetch the list of articles.
    let mut response = match agent
        .get(format!("{url}api/bookmarks"))
        .header("accept", "application/json")
        .header("authorization", format!("Bearer {}", auth.access_token))
        .call()
    {
        Ok(response) => response,
        Err(err) => {
            return Err(format!("could not get list of bookmarks: {err}"));
        }
    };
    let bookmarks = response
        .body_mut()
        .read_json::<Vec<APIBookmarks>>()
        .map_err(|err| format!("could not get list of bookmarks: {err}"))?;

    // Create articles index.
    let articles: BTreeMap<String, Article> = bookmarks
        .into_iter()
        .filter(|bookmark| bookmark.has_article)
        .map(|bookmark| Article {
            id: bookmark.id,
            changed: FxHashSet::default(),
            loaded: bookmark.loaded,
            title: bookmark.title,
            domain: bookmark.site_name,
            format: "epub".to_string(),
            authors: bookmark.authors.unwrap_or_default(),
            language: bookmark.lang,
            reading_time: bookmark.reading_time.unwrap_or(0),
            added: chrono::DateTime::parse_from_rfc3339(&bookmark.created).unwrap_or_default(),
            starred: bookmark.is_marked,
            archived: bookmark.is_archived,
        })
        .map(|article| (article.id.clone(), article))
        .collect();

    // Make a list of articles to download.
    let to_download: Vec<Article> = articles
        .values()
        .filter(|article| article.loaded)
        .filter(|article| match fs::exists(article.path()) {
            Ok(exists) => !exists,
            Err(_) => false,
        })
        .cloned()
        .collect();

    // Update the in-memory list of articles, and save.
    {
        let mut index = index.lock().unwrap();
        index.articles.clear();
        index.articles.extend(articles);
        save_index(&index).map_err(|err| err.to_string())?;
    }

    // Notify the Articles app that the list of articles has been updated, and
    // the shelf can be updated.
    hub.send(Event::ArticleUpdateProgress(
        ArticleUpdateProgress::ListFinished,
    ))
    .ok();

    // Download all articles.
    for (i, article) in to_download.iter().enumerate() {
        hub.send(Event::ArticleUpdateProgress(
            ArticleUpdateProgress::Download(i + 1, to_download.len()),
        ))
        .ok();

        // Download now.
        let mut response = agent
            .get(format!(
                "{}api/bookmarks/{}/article.{}",
                url, article.id, article.format
            ))
            .header("Authorization", "Bearer ".to_owned() + &auth.access_token)
            .call()
            .map_err(|err| format!("article fetch failed: {err}"))?;
        let response_body = response
            .body_mut()
            .read_to_vec()
            .map_err(|err| format!("article fetch failed: {err}"))?;

        // Write article to filesystem.
        let path = format!("{}/article-{}.{}", ARTICLES_DIR, article.id, article.format);
        let tmppath = path.to_owned() + ".tmp";
        let mut file = File::create(&tmppath).map_err(|err| err.to_string())?;
        file.write_all(&response_body)
            .map_err(|err| err.to_string())?;
        file.flush().map_err(|err| err.to_string())?;
        drop(file);
        fs::rename(tmppath, path).map_err(|err| err.to_string())?;
    }

    Ok(())
}

fn url_from_server(server: &String) -> String {
    let mut url = "https://".to_owned() + &server;
    if !url.ends_with("/") {
        url += "/";
    }
    url
}
