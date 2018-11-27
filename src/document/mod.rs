pub mod djvu;
pub mod pdf;
pub mod epub;

mod djvulibre_sys;
mod mupdf_sys;

use std::ptr;
use std::path::Path;
use std::str::FromStr;
use fnv::FnvHashSet;
use isbn::Isbn;
use unicode_normalization::UnicodeNormalization;
use unicode_normalization::char::{is_combining_mark};
use geom::{Boundary, CycleDir};
use document::djvu::DjvuOpener;
use document::pdf::PdfOpener;
use document::epub::EpubDocument;
use settings::EpubEngine;
use framebuffer::Pixmap;

pub const BYTES_PER_PAGE: f64 = 2048.0;

#[derive(Debug, Copy, Clone)]
pub enum Location<'a> {
    Exact(usize),
    Previous(usize),
    Next(usize),
    Uri(usize, &'a str),
}

#[derive(Debug, Clone)]
pub struct BoundedText {
    pub text: String,
    pub rect: Boundary,
}

#[derive(Debug, Clone)]
pub struct TocEntry {
    pub title: String,
    pub location: usize,
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
    fn words(&mut self, loc: Location) -> Option<(Vec<BoundedText>, usize)>;
    fn lines(&mut self, loc: Location) -> Option<(Vec<BoundedText>, usize)>;
    fn links(&mut self, loc: Location) -> Option<(Vec<BoundedText>, usize)>;

    fn pixmap(&mut self, loc: Location, scale: f32) -> Option<(Pixmap, usize)>;
    fn layout(&mut self, width: u32, height: u32, font_size: f32, dpi: u16);
    fn set_font_family(&mut self, family_name: &str, search_path: &str);
    fn set_margin_width(&mut self, width: i32);
    fn set_line_height(&mut self, line_height: f32);

    fn title(&self) -> Option<String>;
    fn author(&self) -> Option<String>;
    fn metadata(&self, key: &str) -> Option<String>;

    fn is_reflowable(&self) -> bool;

    fn has_synthetic_page_numbers(&self) -> bool {
        false
    }

    fn has_toc(&mut self) -> bool {
        self.toc().map_or(false, |entries| !entries.is_empty())
    }

    fn resolve_location(&mut self, loc: Location) -> Option<usize> {
        if self.pages_count() == 0 {
            return None;
        }

        match loc {
            Location::Exact(index) => {
                Some(index.max(0).min(self.pages_count() - 1))
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

    fn isbn(&mut self) -> Option<String> {
        let mut found = false;
        let mut result = None;
        let mut loc = Location::Exact(0);
        let mut pages_count = 0;
        while let Some((ref words, l)) = self.words(loc) {
            for word in words.iter().map(|w| &*w.text) {
                if word.contains("ISBN") {
                    found = true;
                    continue;
                }
                if found && word.len() >= 10 {
                    let digits: String = word.chars()
                                             .filter(|&c| c.is_digit(10) ||
                                                          c == 'X')
                                             .collect();
                    if let Ok(isbn) = Isbn::from_str(&digits) {
                        result = Some(isbn.to_string());
                        break;
                    }
                }
            }
            pages_count += 1;
            if pages_count > 10 || result.is_some() {
                break;
            }
            loc = Location::Next(l);
        }
        result
    }
}

pub fn file_kind<P: AsRef<Path>>(path: P) -> Option<String> {
    path.as_ref().extension()
        .and_then(|os_ext| os_ext.to_str())
        .map(|ext| ext.to_lowercase())
}

pub trait HumanSize {
    fn human_size(&self) -> String;
}

impl HumanSize for u64 {
    fn human_size(&self) -> String {
        let value = *self as f32;
        let level = (value.log(1024f32).floor() as usize).min(3);
        let factor = value / (1024f32).powi(level as i32);
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

pub struct DocumentOpener {
    epub_engine: EpubEngine,
}

impl DocumentOpener {
    pub fn new(epub_engine: EpubEngine) -> DocumentOpener {
        DocumentOpener { epub_engine }
    }

    pub fn open<P: AsRef<Path>>(&self, path: P) -> Option<Box<Document>> {
        file_kind(path.as_ref()).and_then(|k| {
            match k.as_ref() {
                "epub" => {
                    match self.epub_engine {
                        EpubEngine::BuiltIn => {
                            EpubDocument::new(path)
                                         .map(|d| Box::new(d) as Box<Document>).ok()
                        },
                        EpubEngine::Mupdf => {
                            PdfOpener::new()
                                .and_then(|mut o| {
                                    let css_path = Path::new("user.css");
                                    if css_path.exists() {
                                        o.set_user_css(css_path).ok();
                                    }
                                    o.open(path)
                                     .map(|d| Box::new(d) as Box<Document>)
                                })
                        },
                    }
                },
                "djvu" | "djv" => {
                    DjvuOpener::new().and_then(|o| {
                        o.open(path)
                         .map(|d| Box::new(d) as Box<Document>)
                    })
                },
                _ => {
                    PdfOpener::new().and_then(|o| {
                        o.open(path)
                         .map(|d| Box::new(d) as Box<Document>)
                    })
                },
            }
        })
    }
}

pub fn toc_as_html(toc: &[TocEntry], location: usize, next_location: Option<usize>) -> String {
    let chap = chapter_at(toc, location, next_location);
    let mut buf = r#"<html>
                         <head>
                             <title>Table of Contents</title>
                             <link rel="stylesheet" type="text/css" href="css/toc.css"/>
                         </head>
                     <body>"#.to_string();
    toc_as_html_aux(toc, &mut buf, chap);
    buf.push_str("</body></html>");
    buf
}

pub fn toc_as_html_aux(toc: &[TocEntry], buf: &mut String, chap: Option<&TocEntry>) {
    buf.push_str("<ul>");
    for entry in toc {
        buf.push_str(&format!(r#"<li><a href="@{}">"#, entry.location));
        let title = entry.title.replace('<', "&lt;").replace('>', "&gt;");
        if chap.is_some() && ptr::eq(entry, chap.unwrap()) {
            buf.push_str(&format!("<strong>{}</strong>", title));
        } else {
            buf.push_str(&title);
        }
        buf.push_str("</a></li>");
        if !entry.children.is_empty() {
            toc_as_html_aux(&entry.children, buf, chap);
        }
    }
    buf.push_str("</ul>");
}

pub fn chapter_at(toc: &[TocEntry], location: usize, next_location: Option<usize>) -> Option<&TocEntry> {
    let mut chap_before = None;
    let mut chap_after = None;
    chapter_at_aux(toc, location, next_location, &mut chap_before, &mut chap_after);
    chap_after.or(chap_before)
}

fn chapter_at_aux<'a>(toc: &'a [TocEntry], location: usize, next_location: Option<usize>, chap_before: &mut Option<&'a TocEntry>, chap_after: &mut Option<&'a TocEntry>) {
    for entry in toc {
        if entry.location < location && (chap_before.is_none() || entry.location > chap_before.unwrap().location) {
            *chap_before = Some(entry);
        }
        if entry.location >= location && (next_location.is_none() || entry.location < next_location.unwrap()) && (chap_after.is_none() || entry.location < chap_after.unwrap().location) {
            *chap_after = Some(entry);
        }
        chapter_at_aux(&entry.children, location, next_location, chap_before, chap_after);
    }
}

pub fn chapter_relative(toc: &[TocEntry], location: usize, next_location: Option<usize>, dir: CycleDir) -> Option<usize> {
    let mut page = None;
    let chap = chapter_at(toc, location, next_location);
    if dir == CycleDir::Next {
        chapter_relative_next(toc, location, next_location, &mut page, chap);
    } else {
        if chap.map(|c| c.location < location) == Some(true) {
            page = Some(chap.unwrap().location);
        } else {
            chapter_relative_prev(toc, location, &mut page, chap);
        }
    }
    page
}

fn chapter_relative_next<'a>(toc: &'a [TocEntry], location: usize, next_location: Option<usize>, page: &mut Option<usize>, chap: Option<&TocEntry>) {
    for entry in toc {
        if entry.location > location && (next_location.is_none() || entry.location >= next_location.unwrap()) && (page.is_none() || entry.location < page.unwrap()) && (chap.is_none() || !ptr::eq(chap.unwrap(), entry)) {
            *page = Some(entry.location);
        }

        chapter_relative_next(&entry.children, location, next_location, page, chap);
    }
}

fn chapter_relative_prev<'a>(toc: &'a [TocEntry], location: usize, page: &mut Option<usize>, chap: Option<&TocEntry>) {
    for entry in toc.iter().rev() {
        chapter_relative_prev(&entry.children, location, page, chap);

        if entry.location < location && (page.is_none() || entry.location > page.unwrap()) && (chap.is_none() || !ptr::eq(chap.unwrap(), entry)) {
            *page = Some(entry.location);
        }
    }
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
