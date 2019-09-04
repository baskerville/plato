pub mod djvu;
pub mod pdf;
pub mod epub;

mod djvulibre_sys;
mod mupdf_sys;

use std::path::Path;
use std::ffi::OsStr;
use fnv::FnvHashSet;
use lazy_static::lazy_static;
use unicode_normalization::UnicodeNormalization;
use unicode_normalization::char::{is_combining_mark};
use serde::{Serialize, Deserialize};
use self::djvu::DjvuOpener;
use self::pdf::PdfOpener;
use self::epub::EpubDocument;
use crate::geom::{Boundary, CycleDir};
use crate::metadata::{TextAlign};
use crate::framebuffer::Pixmap;

pub const BYTES_PER_PAGE: f64 = 2048.0;

#[derive(Debug, Clone)]
pub enum Location {
    Exact(usize),
    Previous(usize),
    Next(usize),
    LocalUri(usize, String),
    Uri(String),
}

#[derive(Debug, Clone)]
pub struct BoundedText {
    pub text: String,
    pub rect: Boundary,
    pub location: TextLocation,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
#[serde(untagged)]
pub enum TextLocation {
    Static(usize, usize),
    Dynamic(usize),
}

impl TextLocation {
    pub fn location(self) -> usize {
        match self {
            TextLocation::Static(page, _) => page,
            TextLocation::Dynamic(offset) => offset,
        }
    }

    #[inline]
    pub fn min_max(self, other: Self) -> (Self, Self) {
        if self < other {
            (self, other)
        } else {
            (other, self)
        }
    }
}

#[derive(Debug, Clone)]
pub struct TocEntry {
    pub title: String,
    pub location: Location,
    pub index: usize,
    pub children: Vec<TocEntry>,
}

#[derive(Debug, Clone)]
pub struct Neighbors {
    pub previous_page: Option<usize>,
    pub next_page: Option<usize>,
}

pub trait Document: Send+Sync {
    fn dims(&self, index: usize) -> Option<(f32, f32)>;
    fn pages_count(&self) -> usize;

    fn toc(&mut self) -> Option<Vec<TocEntry>>;
    fn chapter<'a>(&mut self, offset: usize, toc: &'a [TocEntry]) -> Option<&'a TocEntry>;
    fn chapter_relative<'a>(&mut self, offset: usize, dir: CycleDir, toc: &'a [TocEntry]) -> Option<&'a TocEntry>;
    fn words(&mut self, loc: Location) -> Option<(Vec<BoundedText>, usize)>;
    fn lines(&mut self, loc: Location) -> Option<(Vec<BoundedText>, usize)>;
    fn links(&mut self, loc: Location) -> Option<(Vec<BoundedText>, usize)>;

    fn pixmap(&mut self, loc: Location, scale: f32) -> Option<(Pixmap, usize)>;
    fn layout(&mut self, width: u32, height: u32, font_size: f32, dpi: u16);
    fn set_font_family(&mut self, family_name: &str, search_path: &str);
    fn set_margin_width(&mut self, width: i32);
    fn set_text_align(&mut self, text_align: TextAlign);
    fn set_line_height(&mut self, line_height: f32);

    fn title(&self) -> Option<String>;
    fn author(&self) -> Option<String>;
    fn metadata(&self, key: &str) -> Option<String>;

    fn is_reflowable(&self) -> bool;

    fn has_synthetic_page_numbers(&self) -> bool {
        false
    }

    fn resolve_location(&mut self, loc: Location) -> Option<usize> {
        if self.pages_count() == 0 {
            return None;
        }

        match loc {
            Location::Exact(index) => {
                if index >= self.pages_count() {
                    None
                } else {
                    Some(index)
                }
            },
            Location::Previous(index) => {
                if index > 0 {
                    Some(index - 1)
                } else {
                    None
                }
            },
            Location::Next(index) => {
                if index < self.pages_count() - 1 {
                    Some(index + 1)
                } else {
                    None
                }
            }
            _ => None,
        }
    }
}

pub fn file_kind<P: AsRef<Path>>(path: P) -> Option<String> {
    path.as_ref().extension()
        .and_then(OsStr::to_str)
        .map(str::to_lowercase)
}

pub trait HumanSize {
    fn human_size(&self) -> String;
}

const SIZE_BASE: f32 = 1024.0;

impl HumanSize for u64 {
    fn human_size(&self) -> String {
        let value = *self as f32;
        let level = (value.max(1.0).log(SIZE_BASE).floor() as usize).min(3);
        let factor = value / (SIZE_BASE).powi(level as i32);
        let precision = level.saturating_sub(1 + factor.log(10.0).floor() as usize);
        format!("{0:.1$} {2}", factor, precision, ['B', 'K', 'M', 'G'][level])
    }
}

pub fn asciify(name: &str) -> String {
    name.nfkd().filter(|&c| !is_combining_mark(c)).collect::<String>()
        .replace('œ', "oe")
        .replace('Œ', "Oe")
        .replace('æ', "ae")
        .replace('Æ', "Ae")
        .replace('—', "-")
        .replace('–', "-")
        .replace('’', "'")
}


pub fn open<P: AsRef<Path>>(path: P) -> Option<Box<dyn Document>> {
    file_kind(path.as_ref()).and_then(|k| {
        match k.as_ref() {
            "epub" => {
                EpubDocument::new(path)
                             .map(|d| Box::new(d) as Box<dyn Document>).ok()
            },
            "djvu" | "djv" => {
                DjvuOpener::new().and_then(|o| {
                    o.open(path)
                     .map(|d| Box::new(d) as Box<dyn Document>)
                })
            },
            _ => {
                PdfOpener::new().and_then(|o| {
                    o.open(path)
                     .map(|d| Box::new(d) as Box<dyn Document>)
                })
            },
        }
    })
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum SimpleTocEntry {
    Leaf(String, TocLocation),
    Container(String, TocLocation, Vec<SimpleTocEntry>),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum TocLocation {
    Exact(usize),
    Uri(String),
}

impl From<TocLocation> for Location {
    fn from(loc: TocLocation) -> Location {
        match loc {
            TocLocation::Exact(n) => Location::Exact(n),
            TocLocation::Uri(uri) => Location::Uri(uri),
        }
    }
}

pub fn toc_as_html(toc: &[TocEntry], chap_index: usize) -> String {
    let mut buf = r#"<html>
                         <head>
                             <title>Table of Contents</title>
                             <link rel="stylesheet" type="text/css" href="css/toc.css"/>
                         </head>
                     <body>"#.to_string();
    toc_as_html_aux(toc, chap_index, &mut buf);
    buf.push_str("</body></html>");
    buf
}

pub fn toc_as_html_aux(toc: &[TocEntry], chap_index: usize, buf: &mut String) {
    buf.push_str("<ul>");
    for entry in toc {
        match entry.location {
            Location::Exact(n) => buf.push_str(&format!(r#"<li><a href="@{}">"#, n)),
            Location::Uri(ref uri) => buf.push_str(&format!(r#"<li><a href="@{}">"#, uri)),
            _ => buf.push_str("<li><a href=\"#\">"),
        }
        let title = entry.title.replace('<', "&lt;").replace('>', "&gt;");
        if entry.index == chap_index {
            buf.push_str(&format!("<strong>{}</strong>", title));
        } else {
            buf.push_str(&title);
        }
        buf.push_str("</a></li>");
        if !entry.children.is_empty() {
            toc_as_html_aux(&entry.children, chap_index, buf);
        }
    }
    buf.push_str("</ul>");
}

#[inline]
fn chapter(index: usize, toc: &[TocEntry]) -> Option<&TocEntry> {
    let mut chap = None;
    let mut chap_index = 0;
    chapter_aux(toc, index, &mut chap, &mut chap_index);
    chap
}

fn chapter_aux<'a>(toc: &'a [TocEntry], index: usize, chap: &mut Option<&'a TocEntry>, chap_index: &mut usize) {
    for entry in toc {
        if let Location::Exact(entry_index) = entry.location {
            if entry_index <= index && (chap.is_none() || entry_index > *chap_index) {
                *chap = Some(entry);
                *chap_index = entry_index;
            }
        }
        chapter_aux(&entry.children, index, chap, chap_index);
    }
}

#[inline]
fn chapter_relative(index: usize, dir: CycleDir, toc: &[TocEntry]) -> Option<&TocEntry> {
    let chap = chapter(index, toc);

    match dir {
        CycleDir::Previous => previous_chapter(chap, index, toc),
        CycleDir::Next => next_chapter(chap, index, toc),
    }
}

fn previous_chapter<'a>(chap: Option<&TocEntry>, index: usize, toc: &'a [TocEntry]) -> Option<&'a TocEntry> {
    for entry in toc.iter().rev() {
        let result = previous_chapter(chap, index, &entry.children);
        if result.is_some() {
            return result;
        }

        if let Some(chap) = chap {
            if entry.index < chap.index {
                if let Location::Exact(entry_index) = entry.location {
                    if entry_index != index {
                        return Some(entry)
                    }
                }
            }
        } else {
            if let Location::Exact(entry_index) = entry.location {
                if entry_index < index {
                    return Some(entry);
                }
            }
        }
    }
    None
}

fn next_chapter<'a>(chap: Option<&TocEntry>, index: usize, toc: &'a [TocEntry]) -> Option<&'a TocEntry> {
    for entry in toc {
        if let Some(chap) = chap {
            if entry.index > chap.index {
                if let Location::Exact(entry_index) = entry.location {
                    if entry_index != index {
                        return Some(entry)
                    }
                }
            }
        } else {
            if let Location::Exact(entry_index) = entry.location {
                if entry_index > index {
                    return Some(entry);
                }
            }
        }

        let result = next_chapter(chap, index, &entry.children);
        if result.is_some() {
            return result;
        }
    }
    None
}

pub fn chapter_from_index(index: usize, toc: &[TocEntry]) -> Option<&TocEntry> {
    for entry in toc {
        if entry.index == index {
            return Some(entry);
        }
        let result = chapter_from_index(index, &entry.children);
        if result.is_some() {
            return result;
        }
    }
    None
}

pub fn chapter_from_uri<'a>(target_uri: &str, toc: &'a [TocEntry]) -> Option<&'a TocEntry> {
    for entry in toc {
        if let Location::Uri(ref uri) = entry.location {
            if target_uri == uri {
                return Some(entry);
            }
        }
        let result = chapter_from_uri(target_uri, &entry.children);
        if result.is_some() {
            return result;
        }
    }
    None
}

// cd mupdf/source && awk '/_extensions\[/,/}/' */*.c
lazy_static! {
pub static ref RECOGNIZED_KINDS: FnvHashSet<&'static str> =
    [
    // djvu
    "djvu",
    "djv",
    // cbz
    "cbt",
    "cbz",
    "tar",
    "zip",
    // img
    "bmp",
    "gif",
    "hdp",
    "j2k",
    "jfif",
    "jfif-tbnl",
    "jp2",
    "jpe",
    "jpeg",
    "jpg",
    "jpx",
    "jxr",
    "pam",
    "pbm",
    "pgm",
    "png",
    "pnm",
    "ppm",
    "wdp",
    // tiff
    "tif",
    "tiff",
    // gprf
    "gproof",
    // epub
    "epub",
    // html
    "fb2",
    "htm",
    "html",
    "xhtml",
    "xml",
    // pdf
    "pdf",
    "ai",
    // svg
    "svg",
    // xps
    "oxps",
    "xps",
    ].iter().cloned().collect();
}
