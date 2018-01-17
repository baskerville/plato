pub mod djvu;
pub mod pdf;

use std::ptr;
use std::path::Path;
use std::str::FromStr;
use fnv::FnvHashSet;
use isbn::Isbn;
use unicode_normalization::UnicodeNormalization;
use unicode_normalization::char::{is_combining_mark};
use geom::{Rectangle, CycleDir};
use document::djvu::{DjvuOpener};
use document::pdf::{PdfOpener};
use framebuffer::Pixmap;

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum LayerGrain {
    Page,
    Column,
    Region,
    Paragraph,
    Line,
    Word,
    Character,
}

#[derive(Debug, Clone)]
pub struct TextLayer {
    pub grain: LayerGrain,
    pub rect: Rectangle,
    pub text: Option<String>,
    pub children: Vec<TextLayer>,
}


#[derive(Debug, Clone)]
pub struct Link {
    pub uri: String,
    pub rect: Rectangle,
}

impl TextLayer {
    fn is_empty(&self) -> bool {
        self.text.is_none() &&
            self.children.iter().all(|t| t.is_empty())
    }
}

#[derive(Debug, Clone)]
pub struct TocEntry {
    pub title: String,
    pub page: usize,
    pub children: Vec<TocEntry>,
}

pub fn toc_as_html(toc: &[TocEntry], index: usize) -> String {
    let chap = chapter_at(toc, index);
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
        buf.push_str(&format!(r##"<li><a href="#{}">"##, entry.page));
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

pub fn chapter_at<'a>(toc: &'a [TocEntry], index: usize) -> Option<&'a TocEntry> {
    let mut chap = None;
    chapter_at_aux(toc, index, &mut chap);
    chap
}

fn chapter_at_aux<'a>(toc: &'a [TocEntry], index: usize, chap: &mut Option<&'a TocEntry>) {
    for entry in toc {
        if entry.page <= index && (chap.is_none() || entry.page > chap.map(|c| c.page).unwrap()) {
            *chap = Some(entry);
        }
        chapter_at_aux(&entry.children, index, chap);
    }
}

pub fn chapter_relative(toc: &[TocEntry], index: usize, dir: CycleDir) -> Option<usize> {
    let mut page = None;
    if dir == CycleDir::Next {
        chapter_relative_next(toc, index, &mut page);
    } else {
        chapter_relative_prev(toc, index, &mut page);
    }
    page
}

fn chapter_relative_next<'a>(toc: &'a [TocEntry], index: usize, page: &mut Option<usize>) {
    for entry in toc {
        if entry.page > index && (page.is_none() || entry.page < page.unwrap()) {
            *page = Some(entry.page);
        }

        chapter_relative_next(&entry.children, index, page);
    }
}

fn chapter_relative_prev<'a>(toc: &'a [TocEntry], index: usize, page: &mut Option<usize>) {
    for entry in toc.iter().rev() {
        chapter_relative_prev(&entry.children, index, page);

        if entry.page < index && (page.is_none() || entry.page > page.unwrap()) {
            *page = Some(entry.page);
        }
    }
}

pub trait Document {
    fn pages_count(&self) -> usize;
    fn pixmap(&self, index: usize, scale: f32) -> Option<Pixmap>;
    fn dims(&self, index: usize) -> Option<(f32, f32)>;

    fn toc(&self) -> Option<Vec<TocEntry>>;
    fn text(&self, index: usize) -> Option<TextLayer>;
    fn links(&self, index: usize) -> Option<Vec<Link>>;

    fn title(&self) -> Option<String>;
    fn author(&self) -> Option<String>;

    fn is_reflowable(&self) -> bool;
    fn layout(&mut self, width: f32, height: f32, em: f32);

    fn has_text(&self) -> bool {
        (0..self.pages_count()).any(|i| self.text(i).map_or(false, |t| !t.is_empty()))
    }

    fn has_toc(&self) -> bool {
        self.toc().map_or(false, |v| !v.is_empty())
    }

    fn isbn(&self) -> Option<String> {
        let mut found = false;
        let mut result = None;
        'pursuit: for index in 0..10 {
            if let Some(ref text) = self.text(index) {
                for word in text.words() {
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
                            break 'pursuit;
                        }
                    }
                }
            }
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

impl TextLayer {
    pub fn words(&self) -> Vec<String> {
        match self.grain {
            LayerGrain::Word => {
                vec![self.text.as_ref().unwrap().to_string()]
            },
            LayerGrain::Character => vec![],
            _ => {
                let mut result = Vec::new();
                for child in &self.children {
                    result.extend_from_slice(&child.words());
                }
                result
            }
        }
    }
}

pub fn asciify(name: &str) -> String {
    name.nfkd().filter(|&c| !is_combining_mark(c)).collect::<String>()
        .replace('œ', "oe")
        .replace('Œ', "OE")
        .replace('æ', "ae")
        .replace('Æ', "AE")
        .replace('—', "-")
        .replace('–', "-")
        .replace('’', "'")
}

pub fn open<P: AsRef<Path>>(path: P) -> Option<Box<Document>> {
    file_kind(path.as_ref()).and_then(|k| {
        match k.as_ref() {
            "djvu" | "djv" => {
                DjvuOpener::new()
                    .and_then(|o| o.open(path)
                                   .map(|d| Box::new(d) as Box<Document>))
            },
            _ => {
                PdfOpener::new()
                    .and_then(|mut o| {
                        let css_path = Path::new("user.css");
                        if css_path.exists() && o.set_user_css(css_path).is_err() {
                            return None;
                        }
                        o.open(path)
                         .map(|d| Box::new(d) as Box<Document>)
                    })
            },
        }
    })
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
    // svg
    "svg",
    // xps
    "oxps",
    "xps",
    ].iter().cloned().collect();
}
