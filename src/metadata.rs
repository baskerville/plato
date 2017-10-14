extern crate serde_json;

use std::path::{Path, PathBuf};
use std::fs::File;
use std::ops::{Deref, DerefMut};
use std::collections::BTreeSet;
use std::cmp::Ordering;
use fnv::FnvHashMap;
use regex::Regex;
use chrono::{Local, DateTime};
use errors::*;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Info {
    #[serde(skip_serializing_if = "String::is_empty")]
    pub title: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub subtitle: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub author: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub year: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub language: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub publisher: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub series: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub edition: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub volume: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub number: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub isbn: String, // International Standard Book Number
    // #[serde(skip_serializing_if = "String::is_empty")]
    // pub issn: String, // International Standard Serial Number
    // #[serde(skip_serializing_if = "String::is_empty")]
    // pub ismn: String, // International Standard Music Number
    #[serde(skip_serializing_if = "BTreeSet::is_empty")]
    pub categories: BTreeSet<String>,
    pub file: FileInfo,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reader: Option<ReaderInfo>,
    #[serde(with = "simple_date_format")]
    pub added: DateTime<Local>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct FileInfo {
    pub path: PathBuf,
    pub kind: String,
    pub size: u64,
}

impl Default for FileInfo {
    fn default() -> Self {
        FileInfo {
            path: PathBuf::default(),
            kind: String::default(),
            size: u64::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ReaderInfo {
    pub opened: DateTime<Local>,
    pub last_page: usize,
    pub pages_count: usize,
    pub columns: u8,
}

mod simple_date_format {
    use chrono::{DateTime, Local, TimeZone};
    use serde::{self, Deserialize, Serializer, Deserializer};

    const FORMAT: &str = "%Y-%m-%d %H:%M:%S";

    pub fn serialize<S>(date: &DateTime<Local>, serializer: S) -> Result<S::Ok, S::Error> where S: Serializer {
        let s = format!("{}", date.format(FORMAT));
        serializer.serialize_str(&s)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<DateTime<Local>, D::Error> where D: Deserializer<'de> {
        let s = String::deserialize(deserializer)?;
        Local.datetime_from_str(&s, FORMAT).map_err(serde::de::Error::custom)
    }
}

impl Default for ReaderInfo {
    fn default() -> Self {
        ReaderInfo {
            opened: Local::now(),
            last_page: 0,
            pages_count: 0,
            columns: 1,
        }
    }
}

impl Default for Info {
    fn default() -> Self {
        Info {
            title: String::default(),
            subtitle: String::default(),
            author: String::default(),
            year: String::default(),
            language: String::default(),
            publisher: String::default(),
            series: String::default(),
            edition: String::default(),
            volume: String::default(),
            number: String::default(),
            isbn: String::default(),
            categories: BTreeSet::new(),
            file: FileInfo::default(),
            added: Local::now(),
            reader: None,
        }
    }
}

impl Info {
    pub fn title(&self) -> String {
        let mut title = self.title.clone();
        if !self.number.is_empty() {
            title = format!("{} #{}", title, self.number);
        }
        if !self.volume.is_empty() {
            title = format!("{} — vol. {}", title, self.volume);
        }
        if !self.subtitle.is_empty() {
            title = if self.subtitle.chars().next().unwrap().is_alphabetic() {
                format!("{}: {}", title, self.subtitle)
            } else {
                format!("{} {}", title, self.subtitle)
            };
        }
        title
    }

    pub fn matches(&self, query: &str) -> bool {
        true
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Metadata(pub Vec<Info>);

impl Deref for Metadata {
    type Target = Vec<Info>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for Metadata {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl Metadata {
    pub fn load<P: AsRef<Path>>(path: P) -> Result<Metadata> {
        let file = File::open(path).chain_err(|| "Can't open metadata file")?;
        serde_json::from_reader(file).chain_err(|| "Can't parse metadata file")
    }

    pub fn categories(&self) -> BTreeSet<String> {
        self.0.iter().flat_map(|info| info.categories.clone()).collect()
    }
}

#[derive(Debug, Copy, Clone)]
pub enum SortMethod {
    Opened,
    Added,
    Size,
    Author,
    Year,
    Kind,
    Pages,
    Title,
}

pub fn sort_opened(i1: &Info, i2: &Info) -> Ordering {
    match (&i1.reader, &i2.reader) {
        (&None, &None) => Ordering::Equal,
        (&None, &Some(_)) => Ordering::Less,
        (&Some(_), &None) => Ordering::Greater,
        (&Some(ref r1), &Some(ref r2)) => r1.opened.cmp(&r2.opened),
    }
}

pub fn sort_size(i1: &Info, i2: &Info) -> Ordering {
    i1.file.size.cmp(&i2.file.size)
}

pub fn sort_kind(i1: &Info, i2: &Info) -> Ordering {
    i1.file.kind.cmp(&i2.file.kind)
}

pub fn sort_added(i1: &Info, i2: &Info) -> Ordering {
    i1.added.cmp(&i2.added)
}

pub fn combine_sort_methods<'a, T, F1, F2>(mut f1: F1, mut f2: F2) -> Box<FnMut(&T, &T) -> Ordering + 'a>
where F1: FnMut(&T, &T) -> Ordering + 'a,
      F2: FnMut(&T, &T) -> Ordering + 'a {
    Box::new(move |x, y| {
        match f1(x, y) {
            ord @ Ordering::Less | ord @ Ordering::Greater => ord,
            Ordering::Equal => f2(x, y),
        }
    })
}

lazy_static! {
    pub static ref TITLE_PREFIXES: FnvHashMap<&'static str, Regex> = {
        let mut p = FnvHashMap::default();
        p.insert("english", Regex::new(r"^(The|An?)\s").unwrap());
        p.insert("french", Regex::new(r"^(Les?\s|La\s|L['’]|Une?\s|Des?\s|Du\s)").unwrap());
        p
    };
}
