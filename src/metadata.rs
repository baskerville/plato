use std::ops::{Deref, DerefMut};
use std::collections::HashSet;
use fnv::FnvHashMap;
use chrono::{Local, DateTime};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Info {
    pub title: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub subtitle: String,
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
    #[serde(skip_serializing_if = "HashSet::is_empty")]
    pub keywords: HashSet<String>,
    pub file: FileInfo,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reader: Option<ReaderInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct FileInfo {
    pub path: String,
    pub kind: String,
    pub size: u64,
}

impl Default for FileInfo {
    fn default() -> Self {
        FileInfo {
            path: String::default(),
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
    pub columns: u8,
}

impl Default for ReaderInfo {
    fn default() -> Self {
        ReaderInfo {
            opened: Local::now(),
            last_page: 0,
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
            keywords: HashSet::new(),
            file: FileInfo::default(),
            reader: None,
        }
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
