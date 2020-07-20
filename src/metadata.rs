use std::fs;
use std::fmt;
use std::ffi::OsStr;
use std::collections::{BTreeSet, BTreeMap};
use std::path::{Path, PathBuf};
use std::cmp::Ordering;
use regex::Regex;
use chrono::{Local, DateTime};
use fxhash::FxHashMap;
use serde::{Serialize, Deserialize};
use lazy_static::lazy_static;
use titlecase::titlecase;
use crate::document::{Document, SimpleTocEntry, TextLocation};
use crate::document::asciify;
use crate::document::epub::EpubDocument;
use crate::helpers::datetime_format;

pub const DEFAULT_CONTRAST_EXPONENT: f32 = 1.0;
pub const DEFAULT_CONTRAST_GRAY: f32 = 224.0;

pub type Metadata = Vec<Info>;

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
    pub isbn: String,
    #[serde(skip_serializing_if = "BTreeSet::is_empty")]
    pub categories: BTreeSet<String>,
    pub file: FileInfo,
    #[serde(skip)]
    pub reader: Option<ReaderInfo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub toc: Option<Vec<SimpleTocEntry>>,
    #[serde(with = "datetime_format")]
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
pub struct Annotation {
    #[serde(skip_serializing_if = "String::is_empty")]
    pub note: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub text: String,
    pub selection: [TextLocation; 2],
    #[serde(with = "datetime_format")]
    pub modified: DateTime<Local>,
}

impl Default for Annotation {
    fn default() -> Self {
        Annotation {
            note: String::new(),
            text: String::new(),
            selection: [TextLocation::Dynamic(0), TextLocation::Dynamic(1)],
            modified: Local::now(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Margin {
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
    pub left: f32,
}

impl Margin {
    pub fn new(top: f32, right: f32, bottom: f32, left: f32) -> Margin {
        Margin { top, right, bottom, left }
    }
}

impl Default for Margin {
    fn default() -> Margin {
        Margin::new(0.0, 0.0, 0.0, 0.0)
    }
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum PageScheme {
    Any,
    EvenOdd,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum CroppingMargins {
    Any(Margin),
    EvenOdd([Margin; 2]),
}

impl CroppingMargins {
    pub fn margin(&self, index: usize) -> &Margin {
        match *self {
            CroppingMargins::Any(ref margin) => margin,
            CroppingMargins::EvenOdd(ref pair) => &pair[index % 2],
        }
    }

    pub fn margin_mut(&mut self, index: usize) -> &mut Margin {
        match *self {
            CroppingMargins::Any(ref mut margin) => margin,
            CroppingMargins::EvenOdd(ref mut pair) => &mut pair[index % 2],
        }
    }

    pub fn apply(&mut self, index: usize, scheme: PageScheme) {
        let margin = self.margin(index).clone();

        match scheme {
            PageScheme::Any => *self = CroppingMargins::Any(margin),
            PageScheme::EvenOdd => *self = CroppingMargins::EvenOdd([margin.clone(), margin]),
        }
    }

    pub fn is_split(&self) -> bool {
        match *self {
            CroppingMargins::Any(..) => false,
            _ => true,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Copy, Clone, Eq, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum TextAlign {
    Justify,
    Left,
    Right,
    Center,
}

impl TextAlign {
    pub fn icon_name(&self) -> &str {
        match self {
            TextAlign::Justify => "align-justify",
            TextAlign::Left => "align-left",
            TextAlign::Right => "align-right",
            TextAlign::Center => "align-center",
        }
    }
}

impl fmt::Display for TextAlign {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Debug::fmt(self, f)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ReaderInfo {
    #[serde(with = "datetime_format")]
    pub opened: DateTime<Local>,
    pub current_page: usize,
    pub pages_count: usize,
    pub finished: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub zoom_mode: Option<ZoomMode>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_offset: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rotation: Option<i8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cropping_margins: Option<CroppingMargins>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub margin_width: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub screen_margin_width: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub font_family: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub font_size: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text_align: Option<TextAlign>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line_height: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub contrast_exponent: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub contrast_gray: Option<f32>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub page_names: BTreeMap<usize, String>,
    #[serde(skip_serializing_if = "BTreeSet::is_empty")]
    pub bookmarks: BTreeSet<usize>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub annotations: Vec<Annotation>,
}

#[derive(Serialize, Deserialize, Debug, Copy, Clone, PartialEq, Eq)]
pub enum ZoomMode {
    FitToPage,
    FitToWidth,
}

impl ReaderInfo {
    pub fn progress(&self) -> f32 {
        (self.current_page / self.pages_count) as f32
    }
}

impl Default for ReaderInfo {
    fn default() -> Self {
        ReaderInfo {
            opened: Local::now(),
            current_page: 0,
            pages_count: 1,
            finished: false,
            zoom_mode: None,
            top_offset: None,
            rotation: None,
            cropping_margins: None,
            margin_width: None,
            screen_margin_width: None,
            font_family: None,
            font_size: None,
            text_align: None,
            line_height: None,
            contrast_exponent: None,
            contrast_gray: None,
            page_names: BTreeMap::new(),
            bookmarks: BTreeSet::new(),
            annotations: Vec::new(),
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
            toc: None,
        }
    }
}

#[derive(Debug, Copy, Clone)]
pub enum Status {
    New,
    Reading(f32),
    Finished,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum SimpleStatus {
    New,
    Reading,
    Finished,
}

impl fmt::Display for SimpleStatus {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Debug::fmt(self, f)
    }
}

impl Info {
    pub fn status(&self) -> Status {
        if let Some(ref r) = self.reader {
            if r.finished {
                Status::Finished
            } else {
                Status::Reading(r.current_page as f32 / r.pages_count as f32)
            }
        } else {
            Status::New
        }
    }

    pub fn simple_status(&self) -> SimpleStatus {
        if let Some(ref r) = self.reader {
            if r.finished {
                SimpleStatus::Finished
            } else {
                SimpleStatus::Reading
            }
        } else {
            SimpleStatus::New
        }
    }

    pub fn file_stem(&self) -> String {
        self.file.path.file_stem().unwrap().to_string_lossy().into_owned()
    }

    pub fn title(&self) -> String {
        if self.title.is_empty() {
            return self.file_stem();
        }

        let mut title = self.title.clone();

        if !self.number.is_empty() && self.series.is_empty() {
            title = format!("{} #{}", title, self.number);
        }

        if !self.volume.is_empty() {
            title = format!("{} — vol. {}", title, self.volume);
        }

        if !self.subtitle.is_empty() {
            title = if self.subtitle.chars().next().unwrap().is_alphanumeric() &&
                       title.chars().last().unwrap().is_alphanumeric() {
                format!("{}: {}", title, self.subtitle)
            } else {
                format!("{} {}", title, self.subtitle)
            };
        }

        if !self.series.is_empty() && !self.number.is_empty() {
            title = format!("{} ({} #{})", title, self.series, self.number);
        }

        title
    }

    #[inline]
    pub fn is_match(&self, query: &Option<Regex>) -> bool {
        if let Some(ref query) = *query {
            query.is_match(&self.title) ||
            query.is_match(&self.subtitle) ||
            query.is_match(&self.author) ||
            query.is_match(&self.series) ||
            self.categories.iter().any(|c| query.is_match(c)) ||
            self.file.path.to_str().map(|s| query.is_match(s)).unwrap_or(false)
        } else {
            true
        }
    }

    // TODO: handle the following case: *Walter M. Miller Jr.*?
    // NOTE: e.g.: John Le Carré: the space between *Le* and *Carré*
    // is a non-breaking space
    pub fn alphabetic_author(&self) -> &str {
        self.author.split(',').next()
                     .and_then(|a| a.split(' ').last())
                     .unwrap_or_default()
    }

    pub fn alphabetic_title(&self) -> &str {
        let mut start = 0;

        let lang = if self.language.is_empty() || self.language.starts_with("en") {
            "en"
        } else if self.language.starts_with("fr") {
            "fr"
        } else {
            &self.language
        };

        if let Some(m) = TITLE_PREFIXES.get(lang)
                                       .and_then(|re| re.find(&self.title)) {
            start = m.end()
        }

        &self.title[start..]
    }

    pub fn label(&self) -> String {
        if !self.author.is_empty() {
            format!("{} · {}", self.title(), &self.author)
        } else {
            self.title()
        }
    }
}

pub fn make_query(text: &str) -> Option<Regex> {
    let any = Regex::new(r"^(\.*|\s)$").unwrap();

    if any.is_match(text) {
        return None;
    }

    let text = text.replace('a', "[aáàâä]")
                   .replace('e', "[eéèêë]")
                   .replace('i', "[iíìîï]")
                   .replace('o', "[oóòôö]")
                   .replace('u', "[uúùûü]")
                   .replace('c', "[cç]")
                   .replace("ae", "(ae|æ)")
                   .replace("oe", "(oe|œ)");
    Regex::new(&format!("(?i){}", text))
          .map_err(|e| eprintln!("{}", e))
          .ok()
}

#[derive(Serialize, Deserialize, Debug, Copy, Clone, Eq, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum SortMethod {
    Opened,
    Added,
    Progress,
    Title,
    Year,
    Author,
    Pages,
    Size,
    Kind,
    FileName,
    FilePath,
}

impl SortMethod {
    pub fn reverse_order(self) -> bool {
        match self {
            SortMethod::Author |
            SortMethod::Title |
            SortMethod::Kind |
            SortMethod::FileName |
            SortMethod::FilePath => false,
            _ => true,
        }
    }

    pub fn label(&self) -> &str {
        match *self {
            SortMethod::Opened => "Date Opened",
            SortMethod::Added => "Date Added",
            SortMethod::Progress => "Progress",
            SortMethod::Author => "Author",
            SortMethod::Title => "Title",
            SortMethod::Year => "Year",
            SortMethod::Size => "File Size",
            SortMethod::Kind => "File Type",
            SortMethod::Pages => "Pages Count",
            SortMethod::FileName => "File Name",
            SortMethod::FilePath => "File Path",
        }
    }

    pub fn title(self) -> String {
        format!("Sort by: {}", self.label())
    }
}

pub fn sort(md: &mut Metadata, sort_method: SortMethod, reverse_order: bool) {
    let sort_fn = sorter(sort_method);

    if reverse_order {
        md.sort_by(|a, b| sort_fn(a, b).reverse());
    } else {
        md.sort_by(sort_fn);
    }
}

#[inline]
pub fn sorter(sort_method: SortMethod) -> fn(&Info, &Info) -> Ordering {
    match sort_method {
        SortMethod::Opened => sort_opened,
        SortMethod::Added => sort_added,
        SortMethod::Progress => sort_progress,
        SortMethod::Author => sort_author,
        SortMethod::Title => sort_title,
        SortMethod::Year => sort_year,
        SortMethod::Size => sort_size,
        SortMethod::Kind => sort_kind,
        SortMethod::Pages => sort_pages,
        SortMethod::FileName => sort_filename,
        SortMethod::FilePath => sort_filepath,
    }
}

pub fn sort_opened(i1: &Info, i2: &Info) -> Ordering {
    i1.reader.as_ref().map(|r1| r1.opened)
      .cmp(&i2.reader.as_ref().map(|r2| r2.opened))
}

pub fn sort_added(i1: &Info, i2: &Info) -> Ordering {
    i1.added.cmp(&i2.added)
}

pub fn sort_pages(i1: &Info, i2: &Info) -> Ordering {
    i1.reader.as_ref().map(|r1| r1.pages_count)
      .cmp(&i2.reader.as_ref().map(|r2| r2.pages_count))
}

// FIXME: 'Z'.cmp('É') equals Ordering::Less
pub fn sort_author(i1: &Info, i2: &Info) -> Ordering {
    i1.alphabetic_author().cmp(i2.alphabetic_author())
}

pub fn sort_title(i1: &Info, i2: &Info) -> Ordering {
    i1.alphabetic_title().cmp(i2.alphabetic_title())
}

// Ordering: Finished < New < Reading.
pub fn sort_progress(i1: &Info, i2: &Info) -> Ordering {
    match (i1.status(), i2.status()) {
        (Status::Finished, Status::Finished) => Ordering::Equal,
        (Status::New, Status::New) => Ordering::Equal,
        (Status::New, Status::Finished) => Ordering::Greater,
        (Status::Finished, Status::New) => Ordering::Less,
        (Status::New, Status::Reading(..)) => Ordering::Less,
        (Status::Reading(..), Status::New) => Ordering::Greater,
        (Status::Finished, Status::Reading(..)) => Ordering::Less,
        (Status::Reading(..), Status::Finished) => Ordering::Greater,
        (Status::Reading(p1), Status::Reading(p2)) => p1.partial_cmp(&p2)
                                                        .unwrap_or(Ordering::Equal),
    }
}

pub fn sort_size(i1: &Info, i2: &Info) -> Ordering {
    i1.file.size.cmp(&i2.file.size)
}

pub fn sort_kind(i1: &Info, i2: &Info) -> Ordering {
    i1.file.kind.cmp(&i2.file.kind)
}

pub fn sort_year(i1: &Info, i2: &Info) -> Ordering {
    i1.year.cmp(&i2.year)
}

pub fn sort_filename(i1: &Info, i2: &Info) -> Ordering {
    i1.file.path.file_name().cmp(&i2.file.path.file_name())
}

pub fn sort_filepath(i1: &Info, i2: &Info) -> Ordering {
    i1.file.path.cmp(&i2.file.path)
}

lazy_static! {
    pub static ref TITLE_PREFIXES: FxHashMap<&'static str, Regex> = {
        let mut p = FxHashMap::default();
        p.insert("en", Regex::new(r"^(The|An?)\s").unwrap());
        p.insert("fr", Regex::new(r"^(Les?\s|La\s|L’|Une?\s|Des?\s|Du\s)").unwrap());
        p
    };
}

#[inline]
pub fn extract_metadata_from_epub(prefix: &Path, info: &mut Info) {
    if !info.title.is_empty() || info.file.kind != "epub" {
        return;
    }

    let path = prefix.join(&info.file.path);

    match EpubDocument::new(&path) {
        Ok(doc) => {
            info.title = doc.title().unwrap_or_default();
            info.author = doc.author().unwrap_or_default();
            info.year = doc.year().unwrap_or_default();
            info.publisher = doc.publisher().unwrap_or_default();
            if let Some((title, index)) = doc.series() {
                info.series = title;
                info.number = index;
            }
            info.language = doc.language().unwrap_or_default();
            info.categories.append(&mut doc.categories());
        },
        Err(e) => eprintln!("Can't open {}: {}", info.file.path.display(), e),
    }
}

pub fn extract_metadata_from_filename(_prefix: &Path, info: &mut Info) {
    if !info.title.is_empty() {
        return;
    }

    if let Some(filename) = info.file.path.file_name().and_then(OsStr::to_str) {
        let mut start_index = 0;

        if filename.starts_with('(') {
            start_index += 1;
            if let Some(index) = filename[start_index..].find(')') {
                info.series = filename[start_index..start_index+index].trim_end().to_string();
                start_index += index + 1;
            }
        }

        if let Some(index) = filename[start_index..].find("- ") {
            info.author = filename[start_index..start_index+index].trim().to_string();
            start_index += index + 1;
        }

        let title_start = start_index;

        if let Some(index) = filename[start_index..].find('_') {
            info.title = filename[start_index..start_index+index].trim_start().to_string();
            start_index += index + 1;
        }

        if let Some(index) = filename[start_index..].find('-') {
            if title_start == start_index {
                info.title = filename[start_index..start_index+index].trim_start().to_string();
            } else {
                info.subtitle = filename[start_index..start_index+index].trim_start().to_string();
            }
            start_index += index + 1;
        }

        if let Some(index) = filename[start_index..].find('(') {
            info.publisher = filename[start_index..start_index+index].trim_end().to_string();
            start_index += index + 1;
        }

        if let Some(index) = filename[start_index..].find(')') {
            info.year = filename[start_index..start_index+index].to_string();
        }
    }
}

pub fn consolidate(_prefix: &Path, info: &mut Info) {
    if info.subtitle.is_empty() {
        if let Some(index) = info.title.find(':') {
            let cur_title = info.title.clone();
            let (title, subtitle) = cur_title.split_at(index);
            info.title = title.trim_end().to_string();
            info.subtitle = subtitle[1..].trim_start().to_string();
        }
    }

    if info.language.is_empty() || info.language.starts_with("en") {
        info.title = titlecase(&info.title);
        info.subtitle = titlecase(&info.subtitle);
    }

    info.title = info.title.replace('\'', "’");
    info.subtitle = info.subtitle.replace('\'', "’");
    info.author = info.author.replace('\'', "’");
    if info.year.len() > 4 {
        info.year = info.year[..4].to_string();
    }
    info.series = info.series.replace('\'', "’");
    info.publisher = info.publisher.replace('\'', "’");
}

pub fn rename_from_info(prefix: &Path, info: &mut Info) {
    let new_file_name = file_name_from_info(info);
    if !new_file_name.is_empty() {
        let old_path = prefix.join(&info.file.path);
        let new_path = old_path.with_file_name(&new_file_name);
        if old_path != new_path {
            match fs::rename(&old_path, &new_path) {
                Err(e) => eprintln!("Can't rename {} to {}: {}.",
                                    old_path.display(),
                                    new_path.display(), e),
                Ok(..) => {
                    let relat = new_path.strip_prefix(prefix)
                                        .unwrap_or(&new_path);
                    info.file.path = relat.to_path_buf();
                },
            }
        }
    }
}

pub fn file_name_from_info(info: &Info) -> String {
    if info.title.is_empty() {
        return "".to_string();
    }
    let mut base = asciify(&info.title);
    if !info.subtitle.is_empty() {
        base = format!("{} - {}", base, asciify(&info.subtitle));
    }
    if !info.volume.is_empty() {
        base = format!("{} - {}", base, info.volume);
    }
    if !info.number.is_empty() && info.series.is_empty() {
        base = format!("{} - {}", base, info.number);
    }
    if !info.author.is_empty() {
        base = format!("{} - {}", base, asciify(&info.author));
    }
    base = format!("{}.{}", base, info.file.kind);
    base.replace("..", ".")
        .replace('/', " ")
        .replace('?', "")
        .replace('!', "")
        .replace(':', "")
}
