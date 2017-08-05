pub mod djvu;
pub mod pdf;

use std::cmp;
use std::path::Path;
use fnv::FnvHashSet;
use unicode_normalization::UnicodeNormalization;
use unicode_normalization::char::{is_combining_mark};
use geom::Rectangle;
use document::djvu::{DjvuOpener};
use document::pdf::{PdfOpener};
use framebuffer::Bitmap;

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
    grain: LayerGrain,
    rect: Rectangle,
    text: Option<String>,
    children: Vec<TextLayer>,
}

#[derive(Debug, Clone)]
pub struct TocEntry {
    title: String,
    page: usize,
    children: Vec<TocEntry>,
}

pub trait Document {
    fn pages_count(&self) -> usize;
    fn toc(&self) -> Option<Vec<TocEntry>>;
    fn text(&self, index: usize) -> Option<TextLayer>;
    fn title(&self) -> Option<String>;
    fn author(&self) -> Option<String>;
    // fn dims(&self, index: usize) -> Option<(f32, f32)>;
    // fn render(&self, index: usize, scale: f32) -> Option<Bitmap>;
    fn is_reflowable(&self) -> bool;
    // fn page(&self, index: usize) -> Option<Box<Page>>;
}

pub trait Page {
    fn dims(&self) -> (u32, u32);
    fn render(&self, scale: f32) -> Option<Bitmap>;
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
        let level = cmp::min(3, value.log(1024f32).floor() as usize);
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

pub fn detox(name: &str) -> String {
    name.nfkd().filter(|&c| !is_combining_mark(c)).collect()
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
                    .and_then(|o| o.open(path)
                                   .map(|d| Box::new(d) as Box<Document>))
            },
        }
    })
}

// cd mupdf/source && awk '/_extensions\[/,/}/' */*.c
lazy_static! {
pub static ref ALLOWED_KINDS: FnvHashSet<&'static str> =
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
