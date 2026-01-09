use fxhash::FxHashSet;
use http::{HeaderValue, StatusCode};
use regex::{Captures, Regex};
use serde::Deserialize;
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, File},
    io::{Error, Write},
    ops::Deref,
    sync::{
        atomic::{
            AtomicBool,
            Ordering::{Acquire, Release},
        },
        Arc, Mutex,
    },
    thread,
    time::{SystemTime, UNIX_EPOCH},
};
use ureq::Agent;
use url::Url;

use crate::{
    articles::{
        queue_link, read_queued, save_index, Article, ArticleIndex, Changes, Service, ARTICLES_DIR,
    },
    settings::ArticleAuth,
    view::{ArticleUpdateProgress, Event, Hub},
};

struct ClientCredentials {
    client_id: String,
    client_secret: String,
}

pub struct Wallabag {
    auth: ArticleAuth,
    index: Arc<Mutex<ArticleIndex>>,
    updating: Arc<AtomicBool>,
}

impl Wallabag {
    pub fn load(auth: ArticleAuth, index: ArticleIndex) -> Wallabag {
        Wallabag {
            auth: auth,
            index: Arc::new(Mutex::new(index)),
            updating: Arc::new(AtomicBool::new(false)),
        }
    }
}

impl Service for Wallabag {
    fn index(&self) -> std::sync::Arc<std::sync::Mutex<ArticleIndex>> {
        self.index.clone()
    }

    fn save_index(&self) {
        let index = self.index.lock().unwrap();
        if let Err(err) = save_index(index.deref()) {
            eprintln!("failed to save index: {}", err);
        };
    }

    fn update(&mut self, hub: &Hub) -> bool {
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
                hub.send(Event::Notify(err.to_string())).ok();
            };
            hub.send(Event::ArticleUpdateProgress(ArticleUpdateProgress::Finish))
                .ok();
            updating.store(false, Release);
        });
        return true;
    }
}

// Create new API client by doing HTTP requests and reading the response HTML,
// similar to how the Android app does it. If an API client with the given name
// already exists, that API client is returned instead of creating a new one.
//
// This is a terrible idea, but there doesn't seem to be an alternative that
// doesn't involve asking the user to manually create an API client and copying
// over the client ID and secret. Since the Android app does something similar,
// I hope the Wallabag authors won't break this.
fn create_api_client(
    server: String,
    client_name: String,
    username: String,
    password: String,
) -> Result<ClientCredentials, Error> {
    let url = "https://".to_owned() + &server + "/";

    // Create a HTTP client that works similar to a browser.
    // Disable redirection though since that saves a request during login.
    let agent: Agent = Agent::config_builder().max_redirects(0).build().into();

    // Fetch the login page (which importantly includes the CSRF token).
    let login_url = url.clone() + "login";
    let mut response = match agent.get(&login_url).call() {
        Ok(response) => response,
        Err(ureq::Error::Io(_)) => {
            // Any I/O error, but most likely there's a networking issuing.
            return Err(Error::other("failed to connect to the server"));
        }
        Err(ureq::Error::StatusCode(404)) => {
            // Special case: provide better error message for invalid
            // (mistyped?) addresses.
            return Err(Error::other(format!(
                "login page does not exist: {login_url}",
            )));
        }
        Err(err) => {
            return Err(Error::other(format!("could not fetch login page: {err}")));
        }
    };
    let response_body = match response.body_mut().read_to_string() {
        Ok(text) => text,
        Err(err) => {
            return Err(Error::other(format!(
                "could not fetch response body of login page: {err}",
            )))
        }
    };

    // Extract the CSRF token of the login page.
    let csrf_token = match Regex::new("name=\"_csrf_token\" value=\"(.*+)\"")
        .unwrap()
        .captures(&response_body)
    {
        Some(caps) => caps[1].to_owned(),
        None => return Err(Error::other("could not find CSRF token in login page")),
    };

    // Log in to Wallabag.
    let login_result_url = url.clone() + "login_check";
    let response = match agent.post(&login_result_url).send_form([
        ("_username", username),
        ("_password", password),
        ("_csrf_token", csrf_token),
    ]) {
        Ok(response) => response,
        Err(err) => return Err(Error::other(format!("failed to log in: {err}"))),
    };

    // Check that we got a 302 redirect to the homepage (which indicates a
    // successful login).
    if response.status() != StatusCode::FOUND {
        return Err(Error::other(format!(
            "could not log in, expected 302 redirect but got {}",
            response.status()
        )));
    }
    let empty_header_value = &HeaderValue::from_str("").unwrap();
    let redirect_url = response
        .headers()
        .get("Location")
        .unwrap_or(empty_header_value)
        .to_str()
        .unwrap();
    if redirect_url != url {
        // Try to determine why the login failed.
        // Not really handling any errors here, since we'll fall back to the
        // "could not log in" error message below.
        if redirect_url == login_url {
            if let Ok(mut response) = agent.get(redirect_url).call() {
                if let Ok(body) = response.body_mut().read_to_string() {
                    if let Some(caps) = Regex::new("<script>Materialize.toast\\('(.*?)'")
                        .unwrap()
                        .captures(&body)
                    {
                        return Err(Error::other(format!(
                            "could not log in: {}",
                            caps[1].to_owned()
                        )));
                    };
                };
            };
        }

        return Err(Error::other(format!(
            "could not log in (invalid credentials?), got redirected to {redirect_url}"
        )));
    }

    // We are logged in!

    // Request the API client credentials page to check whether we already have
    // an API client of the right name. (The caller should make sure the API
    // client name is unique).
    let mut response = match agent.get(url.clone() + "developer").call() {
        Ok(response) => response,
        Err(err) => {
            return Err(Error::other(format!(
                "failed to fetch API client list: {err}"
            )))
        }
    };
    let response_body = match response.body_mut().read_to_string() {
        Ok(body) => body,
        Err(err) => {
            return Err(Error::other(format!(
                "failed to fetch API client list body: {err}",
            )))
        }
    };

    // Look for a client with a matching name in the response, and if we find
    // one, use that.
    let re = Regex::new("\"collapsible-header\">(.+) - #[0-9]+</div>(?s).*?Client ID(?s).*?<code>(.+?)</code>(?s).*?Client secret(?s).*?<code>(.+?)</code>")
        .unwrap();
    for (_, [name, client_id, client_secret]) in
        re.captures_iter(&response_body).map(|c| c.extract())
    {
        if name == client_name {
            return Ok(ClientCredentials {
                client_id: String::from(client_id),
                client_secret: String::from(client_secret),
            });
        }
    }

    // No existing client was found. Create a new one.
    // First we need to obtain a CSRF token from the "new client" page.
    let mut response = match agent.get(url.clone() + "developer/client/create").call() {
        Ok(response) => response,
        Err(err) => {
            return Err(Error::other(format!(
                "failed to fetch page to create new API client: {err}",
            )))
        }
    };
    let response_body = match response.body_mut().read_to_string() {
        Ok(body) => body,
        Err(err) => {
            return Err(Error::other(format!(
                "failed to fetch API client create body: {err}",
            )))
        }
    };

    // Extract the CSRF token from the /developer/client/create form.
    let token = match Regex::new("name=\"client\\[_token\\]\" value=\"(.*+)\"")
        .unwrap()
        .captures(&response_body)
    {
        Some(caps) => caps[1].to_owned(),
        None => return Err(Error::other("no token found in API client create form")),
    };

    // Create a new API client.
    let mut response = match agent
        .post(url.clone() + "developer/client/create")
        .send_form([
            ("client[name]", client_name),
            ("client[redirect_uris]", "".to_string()),
            ("client[_token]", token),
        ]) {
        Ok(response) => response,
        Err(err) => {
            return Err(Error::other(format!(
                "failed to create new API client: {err}",
            )))
        }
    };
    let response_body = match response.body_mut().read_to_string() {
        Ok(body) => body,
        Err(err) => {
            return Err(Error::other(format!(
                "failed to create new API client: {err}",
            )))
        }
    };

    // Parse and return the client credentials.
    // Since there is only one client listed in this response, we don't need to
    // match on the client name.
    match Regex::new(
        "Client ID(?s).*?<code>(.+?)</code>(?s).*?Client secret(?s).*?<code>(.+?)</code>",
    )
    .unwrap()
    .captures(&response_body)
    {
        Some(caps) => Ok(ClientCredentials {
            client_id: caps[1].to_owned(),
            client_secret: caps[2].to_owned(),
        }),
        None => Err(Error::other("no credentials found")),
    }
}

pub fn authenticate(
    server: String,
    client_name: String,
    username: String,
    password: String,
) -> Result<ArticleAuth, String> {
    match create_api_client(
        server.clone(),
        client_name,
        username.clone(),
        password.clone(),
    ) {
        Ok(creds) => {
            let auth = ArticleAuth {
                api: "wallabag".to_string(),
                server: server,
                client_id: creds.client_id,
                client_secret: creds.client_secret,
                username: username,
                password: password,
                access_token: "".to_string(),
                refresh_token: "".to_string(),
                access_token_expires: 0,
            };
            Ok(auth)
        }
        Err(err) => Err(err.to_string()),
    }
}

#[derive(Deserialize)]
struct OAuth2 {
    token_type: String,
    refresh_token: String,
    access_token: String,
    expires_in: u64,
}

#[derive(Deserialize)]
struct Entries {
    _embedded: Embedded,
}

#[derive(Deserialize)]
struct Embedded {
    items: Vec<Item>,
}

#[derive(Deserialize)]
struct Item {
    id: u64,
    title: String,
    url: String,
    created_at: String, // when this entry was added to Wallabag
    //published_at: Option<String>,      // when this article was published
    published_by: Option<Vec<String>>, // author of the article
    language: Option<String>,
    reading_time: u32,
    is_starred: u8,
    is_archived: u8,
}

// Request a new refresh token.
// This needs to happen once in a while, since the refresh token doesn't live
// very long.
fn auth_userpass(
    agent: &Agent,
    auth: &mut ArticleAuth,
    url: String,
    now_secs: u64,
) -> Result<(), Error> {
    let mut response = match agent.post(url + "oauth/v2/token").send_form([
        ("grant_type", "password"),
        ("client_id", &auth.client_id),
        ("client_secret", &auth.client_secret),
        ("username", &auth.username),
        ("password", &auth.password),
    ]) {
        Ok(response) => response,
        Err(ureq::Error::Io(_)) => {
            // Any I/O error, but most likely there's a networking issuing.
            return Err(Error::other("failed to connect to the server"));
        }
        Err(err) => return Err(Error::other(format!("OAuth token fetch failed: {err}"))),
    };
    let response_body = match response.body_mut().read_to_string() {
        Ok(text) => text,
        Err(err) => return Err(Error::other(format!("OAuth token fetch failed: {err}"))),
    };

    // Parse the response as a JSON object.
    let response_values: OAuth2 = match serde_json::from_str(&response_body) {
        Ok(values) => values,
        Err(err) => return Err(Error::other(format!("OAuth token fetch failed: {err}"))),
    };
    if response_values.token_type != "bearer" {
        return Err(Error::other(format!(
            "OAuth token fetch failed: unexpected token type {}",
            response_values.token_type
        )));
    }

    // Update the authentication in the stored settings.
    auth.access_token = response_values.access_token;
    auth.refresh_token = response_values.refresh_token;
    auth.access_token_expires = now_secs + response_values.expires_in;

    Ok(())
}

fn update(hub: &Hub, auth: ArticleAuth, index: Arc<Mutex<ArticleIndex>>) -> Result<(), Error> {
    let url = "https://".to_owned() + &auth.server + "/";

    let mut auth = auth;

    let agent: Agent = Agent::config_builder().max_redirects(0).build().into();

    let now_secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();

    if auth.refresh_token.is_empty() {
        // First update after creating/fetching the API client.
        auth_userpass(&agent, &mut auth, url.clone(), now_secs)?;
        hub.send(Event::ArticlesAuth(Ok(auth.clone()))).ok();
    } else if now_secs >= auth.access_token_expires {
        // Update refresh token (every hour or so).
        let mut result = agent.post(url.clone() + "oauth/v2/token").send_form([
            ("grant_type", "refresh_token"),
            ("refresh_token", &auth.refresh_token),
            ("client_id", &auth.client_id),
            ("client_secret", &auth.client_secret),
        ]);

        // Check for an expired grant, and try requesting a new one.
        if let Err(ureq::Error::StatusCode(400)) = result {
            // HTTP status 400 could mean a lot of things, but we'll assume it
            // means the grant expired. Normally it will also return an error
            // like the following:
            //
            //     {"error":"invalid_grant","error_description":"Invalid refresh token"}
            //
            // ...but we're not checking that here.
            println!("OAuth2 grant seems to be expired, requesting a new one");

            // Authenticate anew using the username/password.
            auth_userpass(&agent, &mut auth, url.clone(), now_secs)?;
            hub.send(Event::ArticlesAuth(Ok(auth.clone()))).ok();

            // Try requesting the token again. This will hopefully succeed.
            result = agent.post(url.clone() + "oauth/v2/token").send_form([
                ("grant_type", "refresh_token"),
                ("refresh_token", &auth.refresh_token),
                ("client_id", &auth.client_id),
                ("client_secret", &auth.client_secret),
            ]);
        }

        // Now actually process the response (either from the first attempt, or
        // the second after requesting a new grant).
        let mut response = match result {
            Ok(response) => response,
            Err(ureq::Error::Io(_)) => {
                // Any I/O error, but most likely there's a networking issuing.
                return Err(Error::other("failed to connect to the server"));
            }
            Err(err) => {
                return Err(Error::other(format!(
                    "OAuth token refresh fetch failed: {err}"
                )))
            }
        };
        let response_body = match response.body_mut().read_to_string() {
            Ok(text) => text,
            Err(err) => {
                return Err(Error::other(format!(
                    "OAuth token refresh fetch failed: {err}"
                )))
            }
        };

        // Parse the response as a JSON object.
        let response_values: OAuth2 = match serde_json::from_str(&response_body) {
            Ok(values) => values,
            Err(err) => {
                return Err(Error::other(format!(
                    "OAuth token refresh fetch failed: {err}"
                )))
            }
        };
        if response_values.token_type != "bearer" {
            return Err(Error::other(format!(
                "OAuth token refresh fetch failed: unexpected token type {}",
                response_values.token_type
            )));
        }

        // Update the authentication in the stored settings.
        auth.access_token = response_values.access_token;
        auth.refresh_token = response_values.refresh_token;
        auth.access_token_expires = now_secs + response_values.expires_in;
        hub.send(Event::ArticlesAuth(Ok(auth.clone()))).ok();
    }

    // Submit new URLs.
    let queued = read_queued();
    if !queued.is_empty() {
        // Send the list of URLs via a GET parameter, because for some reason
        // the Wallabag server only accepts those (and not a form in the POST
        // request).
        // See: https://github.com/wallabag/wallabag/issues/8353
        if let Err(err) = agent
            .post(format!("{url}api/entries/lists"))
            .query("urls", serde_json::to_string(&queued).unwrap())
            .header("Authorization", "Bearer ".to_owned() + &auth.access_token)
            .send_empty()
        {
            // Add the links back (this is inefficient, but it should work).
            for link in queued {
                queue_link(link);
            }

            return Err(Error::other(format!("submitting article failed: {err}")));
        };
    }

    // Sync local changes.
    let mut changes: BTreeMap<String, Vec<(&str, &str)>> = BTreeMap::new();
    let mut deleted: BTreeSet<String> = BTreeSet::new();
    for (id, article) in index.lock().unwrap().articles.iter_mut() {
        if article.changed.contains(&Changes::Deleted) {
            deleted.insert(id.clone());
            continue;
        }
        let mut form: Vec<(&'static str, &str)> = Vec::new();
        if article.changed.contains(&Changes::Starred) {
            form.push(("starred", if article.starred { "1" } else { "0" }));
        }
        if article.changed.contains(&Changes::Archived) {
            form.push(("archive", if article.archived { "1" } else { "0" }));
        }
        if !form.is_empty() {
            changes.insert(id.clone(), form);
        }
    }
    for id in deleted {
        match agent
            .delete(format!("{url}api/entries/{id}"))
            .header("Authorization", "Bearer ".to_owned() + &auth.access_token)
            .call()
        {
            Ok(_) | Err(ureq::Error::StatusCode(404)) => {
                // Either successfully deleted or the article was already
                // deleted on the server, so we can remove the entry locally.
                index.lock().unwrap().articles.remove(&id);
            }
            Err(err) => {
                return Err(Error::other(format!("deleting article failed: {err}")));
            }
        };
    }
    for (id, form) in changes {
        match agent
            .patch(format!("{url}api/entries/{id}"))
            .header("Authorization", "Bearer ".to_owned() + &auth.access_token)
            .send_form(form)
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
                return Err(Error::other(format!("sending local changes failed: {err}")));
            }
        };
    }

    // Fetch the list of articles.
    let mut response = match agent
        .get(url.to_owned() + "api/entries.json?detail=metadata&perPage=999999")
        .header("Authorization", "Bearer ".to_owned() + &auth.access_token)
        .call()
    {
        Ok(response) => response,
        Err(ureq::Error::Io(_)) => {
            // Any I/O error, but most likely there's a networking issuing.
            return Err(Error::other("failed to connect to the server"));
        }
        Err(err) => return Err(Error::other(format!("article list fetch failed: {err}"))),
    };
    let response_body = match response.body_mut().read_to_string() {
        Ok(text) => text,
        Err(err) => return Err(Error::other(format!("article list fetch failed: {err}"))),
    };
    let response_values: Entries = match serde_json::from_str(&response_body) {
        Ok(values) => values,
        Err(err) => return Err(Error::other(format!("article list fetch failed: {err}"))),
    };

    // Create articles directory if it doesn't exist yet.
    std::fs::create_dir(ARTICLES_DIR).ok();

    // Create articles index.
    // Wallabag has datetimes that end in something like +0200 which is not
    // RFC3339 compatible. So we fix that by inserting a colon.
    let date_fix = Regex::new("([+-][0-9]{2})([0-9]{2})$").unwrap();
    let articles: BTreeMap<String, Article> = response_values
        ._embedded
        .items
        .into_iter()
        .map(|item| Article {
            id: format!("{}", item.id).to_string(),
            changed: FxHashSet::default(),
            loaded: true, // not sure how to detect this from the Wallabag API
            title: item.title,
            domain: Url::parse(item.url.as_str())
                .unwrap()
                .host_str()
                .unwrap_or_default()
                .to_string(),
            format: "epub".to_string(),
            authors: item.published_by.unwrap_or_default(),
            language: item.language.unwrap_or_default(),
            reading_time: item.reading_time,
            added: chrono::DateTime::parse_from_rfc3339(
                &date_fix.replace(&item.created_at, |caps: &Captures| {
                    format!("{}:{}", &caps[1], &caps[2])
                }),
            )
            .unwrap_or_default(),
            starred: item.is_starred != 0,
            archived: item.is_archived != 0,
        })
        .map(|article| (article.id.clone(), article))
        .collect();

    // Make a list of articles to download.
    let to_download: Vec<Article> = articles
        .values()
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
        save_index(&index)?;
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
        let mut response = match agent
            .get(format!(
                "{}api/entries/{}/export.{}",
                url, article.id, article.format
            ))
            .header("Authorization", "Bearer ".to_owned() + &auth.access_token)
            .call()
        {
            Ok(response) => response,
            Err(err) => return Err(Error::other(format!("article fetch failed: {err}"))),
        };
        let response_body = match response.body_mut().read_to_vec() {
            Ok(text) => text,
            Err(err) => return Err(Error::other(format!("article fetch failed: {err}"))),
        };

        // Write article to filesystem.
        let path = format!("{}/article-{}.{}", ARTICLES_DIR, article.id, article.format);
        let tmppath = path.to_owned() + ".tmp";
        let mut file = File::create(&tmppath)?;
        file.write_all(&response_body)?;
        file.flush()?;
        drop(file);
        fs::rename(tmppath, path)?;
    }

    Ok(())
}
