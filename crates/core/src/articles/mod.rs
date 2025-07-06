mod dummy;
mod readeck;
mod wallabag;

use chrono::FixedOffset;
use fxhash::FxHashSet;
use serde::{Deserialize, Serialize};
use std::fs::OpenOptions;
use std::io::prelude::*;
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, File},
    io::{self, Error, Write},
    os::unix::fs::MetadataExt,
    path::PathBuf,
    sync::{Arc, Mutex},
};

use crate::settings::ArticleAuth;
use crate::{
    articles::readeck::Readeck,
    articles::wallabag::Wallabag,
    metadata::{FileInfo, Info},
    settings::{self, ArticleList},
    view::Hub,
};

pub const ARTICLES_DIR: &str = ".articles";

#[derive(Serialize, Deserialize)]
pub struct ArticleIndex {
    pub articles: BTreeMap<String, Article>,
}

impl Default for ArticleIndex {
    fn default() -> Self {
        ArticleIndex {
            articles: BTreeMap::new(),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Eq, PartialEq, Hash)]
#[serde(rename_all = "kebab-case")]
pub enum Changes {
    Deleted,
    Starred,
    Archived,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Article {
    pub id: String,
    #[serde(skip_serializing_if = "FxHashSet::is_empty")]
    #[serde(default)]
    pub changed: FxHashSet<Changes>,
    pub loaded: bool,
    pub title: String,
    pub domain: String,
    pub authors: Vec<String>,
    pub format: String,
    pub language: String,
    pub reading_time: u32,
    pub added: chrono::DateTime<FixedOffset>,
    pub starred: bool,
    pub archived: bool,
}

impl Article {
    fn path(&self) -> PathBuf {
        std::path::absolute(PathBuf::from(format!(
            "{}/article-{}.{}",
            ARTICLES_DIR, self.id, self.format
        )))
        .unwrap()
    }

    pub fn file(&self) -> FileInfo {
        let path = self.path();
        let size = match fs::metadata(&path) {
            Ok(metadata) => metadata.size(),
            Err(_err) => 0,
        };
        FileInfo {
            path: path,
            kind: self.format.to_owned(),
            size: size,
        }
    }

    pub fn info(&self) -> Info {
        Info {
            title: self.title.to_owned(),
            subtitle: self.domain.to_owned(),
            author: self.authors.join(", "),
            year: "".to_string(),
            language: self.language.to_owned(),
            publisher: "".to_string(),
            series: "".to_string(),
            edition: "".to_string(),
            volume: "".to_string(),
            number: "".to_string(),
            identifier: "".to_string(),
            categories: BTreeSet::new(),
            file: self.file(),
            reader: None,
            reader_info: None,
            toc: None,
            added: self.added.naive_local(),
        }
    }
}

pub trait Service {
    fn index(&self) -> Arc<Mutex<ArticleIndex>>;

    fn save_index(&self);

    // Update the list of articles.
    // Returns true when the update was started, false when an update is already
    // in progress.
    fn update(&mut self, hub: &Hub) -> bool;
}

fn read_index() -> Result<ArticleIndex, Error> {
    let file = File::open(ARTICLES_DIR.to_owned() + "/index.json")?;
    let index: ArticleIndex = serde_json::from_reader(file)?;

    Ok(index)
}

pub fn load(auth: settings::ArticleAuth) -> Box<dyn Service> {
    let index = read_index().unwrap_or_default();
    match auth.api.as_str() {
        "readeck" => Box::new(Readeck::load(auth, index)),
        "wallabag" => Box::new(Wallabag::load(auth, index)),
        _ => Box::new(dummy::Dummy::new()),
    }
}

pub fn authenticate(
    api: String,
    server: String,
    username: String,
    password: String,
) -> Result<ArticleAuth, String> {
    match api.as_str() {
        "readeck" => readeck::authenticate(server, "Plato".to_string(), username, password),
        "wallabag" => wallabag::authenticate(server, "Plato".to_string(), username, password),
        _ => Err(format!("unknown API: {api}")),
    }
}

pub fn filter(service: &Box<dyn Service>, list: crate::settings::ArticleList) -> Vec<Article> {
    // TODO: perhaps only return a list of articles on the current page, to
    // reduce the amount of cloning?
    let mut articles: Vec<Article> = service.index()
            .lock()
            .unwrap()
            .articles
            .values()
            .filter(|article| match list {
                ArticleList::Unread => !article.archived,
                ArticleList::Starred => article.starred,
                ArticleList::Archive => article.archived,
            } && !article.changed.contains(&Changes::Deleted))
            .cloned()
            .collect();

    // Sort newest first.
    articles.sort_by(|a, b| b.added.cmp(&a.added));

    articles
}

fn save_index(index: &ArticleIndex) -> io::Result<()> {
    let buf = serde_json::to_string(index).unwrap();
    let mut file = File::create(ARTICLES_DIR.to_owned() + "/index.json.tmp")?;
    file.write_all(buf.as_bytes())?;
    fs::rename(
        ARTICLES_DIR.to_owned() + "/index.json.tmp",
        ARTICLES_DIR.to_owned() + "/index.json",
    )
}

static QUEUE_MUTEX: Mutex<u32> = Mutex::new(0);

pub fn queue_link(link: String) {
    let lock = QUEUE_MUTEX.lock().unwrap();
    let path = format!("{ARTICLES_DIR}/queued.txt");
    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(&path) {
        if let Err(e) = writeln!(file, "{}", link) {
            eprintln!("Couldn't write to {}: {:#}.", path, e);
        }
    }
    std::mem::drop(lock);
}

pub fn read_queued() -> Vec<String> {
    // Lock the queue to avoid race conditions between adding a link and reading
    // the links.
    let lock = QUEUE_MUTEX.lock().unwrap();

    // Read all the data in the file.
    let path = format!("{ARTICLES_DIR}/queued.txt");
    let mut file = match File::open(&path) {
        Ok(file) => file,
        Err(_) => return Vec::new(),
    };
    let mut data = String::new();
    if let Err(_) = file.read_to_string(&mut data) {
        return Vec::new();
    }

    // Remove the file.
    fs::remove_file(path).ok();

    // Make sure the lock stays locked until here.
    std::mem::drop(lock);

    // Split each line in the file.
    data.split("\n")
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .collect()
}
