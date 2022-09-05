pub mod djvu;
pub mod pdf;
pub mod epub;
pub mod html;

mod djvulibre_sys;
mod mupdf_sys;

use std::env;
use std::process::Command;
use std::path::Path;
use std::fs::{self, File};
use std::ffi::OsStr;
use std::collections::BTreeSet;
use std::os::unix::fs::FileExt;
use anyhow::{Error, format_err};
use regex::Regex;
use nix::sys::statvfs;
#[cfg(target_os = "linux")]
use nix::sys::sysinfo;
use fxhash::{FxHashMap, FxHashSet};
use lazy_static::lazy_static;
use unicode_normalization::UnicodeNormalization;
use unicode_normalization::char::{is_combining_mark};
use serde::{Serialize, Deserialize};
use self::djvu::DjvuOpener;
use self::pdf::PdfOpener;
use self::epub::EpubDocument;
use self::html::HtmlDocument;
use crate::geom::{Boundary, CycleDir};
use crate::metadata::{TextAlign, Annotation};
use crate::framebuffer::Pixmap;
use crate::settings::INTERNAL_CARD_ROOT;
use crate::device::CURRENT_DEVICE;

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
    fn chapter<'a>(&mut self, offset: usize, toc: &'a [TocEntry]) -> Option<(&'a TocEntry, f32)>;
    fn chapter_relative<'a>(&mut self, offset: usize, dir: CycleDir, toc: &'a [TocEntry]) -> Option<&'a TocEntry>;
    fn words(&mut self, loc: Location) -> Option<(Vec<BoundedText>, usize)>;
    fn lines(&mut self, loc: Location) -> Option<(Vec<BoundedText>, usize)>;
    fn links(&mut self, loc: Location) -> Option<(Vec<BoundedText>, usize)>;
    fn images(&mut self, loc: Location) -> Option<(Vec<Boundary>, usize)>;

    fn pixmap(&mut self, loc: Location, scale: f32) -> Option<(Pixmap, usize)>;
    fn layout(&mut self, width: u32, height: u32, font_size: f32, dpi: u16);
    fn set_font_family(&mut self, family_name: &str, search_path: &str);
    fn set_margin_width(&mut self, width: i32);
    fn set_text_align(&mut self, text_align: TextAlign);
    fn set_line_height(&mut self, line_height: f32);
    fn set_hyphen_penalty(&mut self, hyphen_penalty: i32);
    fn set_stretch_tolerance(&mut self, stretch_tolerance: f32);
    fn set_ignore_document_css(&mut self, ignore: bool);

    fn title(&self) -> Option<String>;
    fn author(&self) -> Option<String>;
    fn metadata(&self, key: &str) -> Option<String>;

    fn is_reflowable(&self) -> bool;

    fn has_synthetic_page_numbers(&self) -> bool {
        false
    }

    fn save(&self, _path: &str) -> Result<(), Error> {
        Err(format_err!("this document can't be saved"))
    }

    fn preview_pixmap(&mut self, width: f32, height: f32) -> Option<Pixmap> {
        self.dims(0).and_then(|dims| {
            let scale = (width / dims.0).min(height / dims.1);
            self.pixmap(Location::Exact(0), scale)
        }).map(|(pixmap, _)| pixmap)
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
        .or_else(|| guess_kind(path.as_ref())
                              .ok()
                              .map(String::from))
}

pub fn guess_kind<P: AsRef<Path>>(path: P) -> Result<&'static str, Error> {
    let file = File::open(path.as_ref())?;
    let mut magic = [0; 4];
    file.read_exact_at(&mut magic, 0)?;

    if &magic == b"PK\x03\x04" {
        let mut mime_type = [0; 28];
        file.read_exact_at(&mut mime_type, 30)?;
        if &mime_type == b"mimetypeapplication/epub+zip" {
            return Ok("epub");
        }
    } else if &magic == b"%PDF" {
        return Ok("pdf");
    } else if &magic == b"AT&T" {
        return Ok("djvu");
    }

    Err(format_err!("Unknown file type"))
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
                EpubDocument::new(&path)
                             .map_err(|e| eprintln!("{}: {:#}.", path.as_ref().display(), e))
                             .map(|d| Box::new(d) as Box<dyn Document>).ok()
            },
            "html" | "htm" => {
                HtmlDocument::new(&path)
                             .map_err(|e| eprintln!("{}: {:#}.", path.as_ref().display(), e))
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
    let mut buf = "<html>\n\t<head>\n\t\t<title>Table of Contents</title>\n\t\t\
                   <link rel=\"stylesheet\" type=\"text/css\" href=\"css/toc.css\"/>\n\t\
                   </head>\n\t<body>\n".to_string();
    toc_as_html_aux(toc, chap_index, 0, &mut buf);
    buf.push_str("\t</body>\n</html>");
    buf
}

pub fn toc_as_html_aux(toc: &[TocEntry], chap_index: usize, depth: usize, buf: &mut String) {
    buf.push_str(&"\t".repeat(depth + 2));
    buf.push_str("<ul>\n");
    for entry in toc {
        buf.push_str(&"\t".repeat(depth + 3));
        match entry.location {
            Location::Exact(n) => buf.push_str(&format!("<li><a href=\"@{}\">", n)),
            Location::Uri(ref uri) => buf.push_str(&format!("<li><a href=\"@{}\">", uri)),
            _ => buf.push_str("<li><a href=\"#\">"),
        }
        let title = entry.title.replace('<', "&lt;").replace('>', "&gt;");
        if entry.index == chap_index {
            buf.push_str(&format!("<strong>{}</strong>", title));
        } else {
            buf.push_str(&title);
        }
        buf.push_str("</a></li>\n");
        if !entry.children.is_empty() {
            toc_as_html_aux(&entry.children, chap_index, depth + 1, buf);
        }
    }
    buf.push_str(&"\t".repeat(depth + 2));
    buf.push_str("</ul>\n");
}

pub fn annotations_as_html(annotations: &[Annotation], active_range: Option<(TextLocation, TextLocation)>) -> String {
    let mut buf = "<html>\n\t<head>\n\t\t<title>Annotations</title>\n\t\t\
                   <link rel=\"stylesheet\" type=\"text/css\" href=\"css/annotations.css\"/>\n\t\
                   </head>\n\t<body>\n".to_string();
    buf.push_str("\t\t<ul>\n");
    for annot in annotations {
        let mut note = annot.note.replace('<', "&lt;").replace('>', "&gt;");
        let mut text = annot.text.replace('<', "&lt;").replace('>', "&gt;");
        let start = annot.selection[0];
        if active_range.map_or(false, |(first, last)| start >= first && start <= last) {
            if !note.is_empty() {
                note = format!("<b>{}</b>", note);
            }
            text = format!("<b>{}</b>", text);
        }
        if note.is_empty() {
            buf.push_str(&format!("\t\t<li><a href=\"@{}\">{}</a></li>\n", start.location(), text));
        } else {
            buf.push_str(&format!("\t\t<li><a href=\"@{}\"><i>{}</i> — {}</a></li>\n", start.location(), note, text));
        }
    }
    buf.push_str("\t\t</ul>\n");
    buf.push_str("\t</body>\n</html>");
    buf
}

pub fn bookmarks_as_html(bookmarks: &BTreeSet<usize>, index: usize, synthetic: bool) -> String {
    let mut buf = "<html>\n\t<head>\n\t\t<title>Bookmarks</title>\n\t\t\
                   <link rel=\"stylesheet\" type=\"text/css\" href=\"css/bookmarks.css\"/>\n\t\
                   </head>\n\t<body>\n".to_string();
    buf.push_str("\t\t<ul>\n");
    for bkm in bookmarks {
        let mut text = if synthetic {
            format!("{:.1}", *bkm as f64 / BYTES_PER_PAGE)
        } else {
            format!("{}", bkm + 1)
        };
        if *bkm == index {
            text = format!("<b>{}</b>", text);
        }
        buf.push_str(&format!("\t\t<li><a href=\"@{}\">{}</a></li>\n", bkm, text));
    }
    buf.push_str("\t\t</ul>\n");
    buf.push_str("\t</body>\n</html>");
    buf
}

#[inline]
fn chapter(index: usize, pages_count: usize, toc: &[TocEntry]) -> Option<(&TocEntry, f32)> {
    let mut chap = None;
    let mut chap_index = 0;
    let mut end_index = pages_count;
    chapter_aux(toc, index, &mut chap, &mut chap_index, &mut end_index);
    chap.zip(Some((index - chap_index) as f32 / (end_index - chap_index) as f32))
}

fn chapter_aux<'a>(toc: &'a [TocEntry], index: usize, chap: &mut Option<&'a TocEntry>,
                   chap_index: &mut usize, end_index: &mut usize) {
    for entry in toc {
        if let Location::Exact(entry_index) = entry.location {
            if entry_index <= index && (chap.is_none() || entry_index > *chap_index) {
                *chap = Some(entry);
                *chap_index = entry_index;
            }
            if entry_index > index && entry_index < *end_index {
                *end_index = entry_index;
            }
        }
        chapter_aux(&entry.children, index, chap, chap_index, end_index);
    }
}

#[inline]
fn chapter_relative(index: usize, dir: CycleDir, toc: &[TocEntry]) -> Option<&TocEntry> {
    let chap = chapter(index, usize::MAX, toc).map(|(c, _)| c);

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

pub fn chapter_from_uri<'a>(target_uri: &str, toc: &'a [TocEntry]) -> Option<&'a TocEntry> {
    for entry in toc {
        if let Location::Uri(ref uri) = entry.location {
            if uri.starts_with(target_uri) {
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

const CPUINFO_KEYS: [&str; 3] = ["Processor", "Features", "Hardware"];
const HWINFO_KEYS: [&str; 19] = ["CPU", "PCB", "DisplayPanel", "DisplayCtrl", "DisplayBusWidth",
                                 "DisplayResolution", "FrontLight", "FrontLight_LEDrv", "FL_PWM",
                                 "TouchCtrl", "TouchType", "Battery", "IFlash", "RamSize", "RamType",
                                 "LightSensor", "HallSensor", "RSensor", "Wifi"];

pub fn sys_info_as_html() -> String {
    let mut buf = "<html>\n\t<head>\n\t\t<title>System Info</title>\n\t\t\
                   <link rel=\"stylesheet\" type=\"text/css\" \
                   href=\"css/sysinfo.css\"/>\n\t</head>\n\t<body>\n".to_string();

    buf.push_str("\t\t<table>\n");

    buf.push_str("\t\t\t<tr>\n");
    buf.push_str("\t\t\t\t<td class=\"key\">Model name</td>\n");
    buf.push_str(&format!("\t\t\t\t<td class=\"value\">{}</td>\n", CURRENT_DEVICE.model));
    buf.push_str("\t\t\t</tr>\n");

    buf.push_str("\t\t\t<tr>\n");
    buf.push_str("\t\t\t\t<td class=\"key\">Hardware</td>\n");
    buf.push_str(&format!("\t\t\t\t<td class=\"value\">Mark {}</td>\n", CURRENT_DEVICE.mark()));
    buf.push_str("\t\t\t</tr>\n");
    buf.push_str("\t\t\t<tr class=\"sep\"></tr>\n");

    for (name, var) in [("Code name", "PRODUCT"),
                         ("Model number", "MODEL_NUMBER"),
                         ("Firmare version", "FIRMWARE_VERSION")].iter() {
        if let Ok(value) = env::var(var) {
            buf.push_str("\t\t\t<tr>\n");
            buf.push_str(&format!("\t\t\t\t<td class=\"key\">{}</td>\n", name));
            buf.push_str(&format!("\t\t\t\t<td class=\"value\">{}</td>\n", value));
            buf.push_str("\t\t\t</tr>\n");
        }
    }

    buf.push_str("\t\t\t<tr class=\"sep\"></tr>\n");

    let output = Command::new("scripts/ip.sh")
                         .output()
                         .map_err(|e| eprintln!("Can't execute command: {:#}.", e))
                         .ok();

    if let Some(stdout) = output.filter(|output| output.status.success())
                                .and_then(|output| String::from_utf8(output.stdout).ok())
                                .filter(|stdout| !stdout.is_empty()) {
        buf.push_str("\t\t\t<tr>\n");
        buf.push_str("\t\t\t\t<td>IP Address</td>\n");
        buf.push_str(&format!("\t\t\t\t<td>{}</td>\n", stdout));
        buf.push_str("\t\t\t</tr>\n");
    }

    if let Ok(info) = statvfs::statvfs(INTERNAL_CARD_ROOT) {
        let fbs = info.fragment_size() as u64;
        let free = info.blocks_free() as u64 * fbs;
        let total = info.blocks() as u64 * fbs;
        buf.push_str("\t\t\t<tr>\n");
        buf.push_str("\t\t\t\t<td>Storage (Free / Total)</td>\n");
        buf.push_str(&format!("\t\t\t\t<td>{} / {}</td>\n", free.human_size(), total.human_size()));
        buf.push_str("\t\t\t</tr>\n");
    }

    #[cfg(target_os = "linux")]
    if let Ok(info) = sysinfo::sysinfo() {
        buf.push_str("\t\t\t<tr>\n");
        buf.push_str("\t\t\t\t<td>Memory (Free / Total)</td>\n");
        buf.push_str(&format!("\t\t\t\t<td>{} / {}</td>\n",
                              info.ram_unused().human_size(),
                              info.ram_total().human_size()));
        buf.push_str("\t\t\t</tr>\n");
        let load = info.load_average();
        buf.push_str("\t\t\t<tr>\n");
        buf.push_str("\t\t\t\t<td>Load Average</td>\n");
        buf.push_str(&format!("\t\t\t\t<td>{:.1}% {:.1}% {:.1}%</td>\n",
                              load.0 * 100.0,
                              load.1 * 100.0,
                              load.2 * 100.0));
        buf.push_str("\t\t\t</tr>\n");
    }

    buf.push_str("\t\t\t<tr class=\"sep\"></tr>\n");

    if let Ok(info) = fs::read_to_string("/proc/cpuinfo") {
        for line in info.lines() {
            if let Some(index) = line.find(':') {
                let key = line[0..index].trim();
                let value = line[index+1..].trim();
                if CPUINFO_KEYS.contains(&key) {
                    buf.push_str("\t\t\t<tr>\n");
                    buf.push_str(&format!("\t\t\t\t<td class=\"key\">{}</td>\n", key));
                    buf.push_str(&format!("\t\t\t\t<td class=\"value\">{}</td>\n", value));
                    buf.push_str("\t\t\t</tr>\n");
                }
            }
        }
    }

    buf.push_str("\t\t\t<tr class=\"sep\"></tr>\n");

    let output = Command::new("/bin/ntx_hwconfig")
                         .args(&["-s", "/dev/mmcblk0"])
                         .output()
                         .map_err(|e| eprintln!("Can't execute command: {:#}.", e))
                         .ok();

    let mut map = FxHashMap::default();

    if let Some(stdout) = output.and_then(|output| String::from_utf8(output.stdout).ok()) {
        let re = Regex::new(r#"\[\d+\]\s+(?P<key>[^=]+)='(?P<value>[^']+)'"#).unwrap();
        for caps in re.captures_iter(&stdout) {
            map.insert(caps["key"].to_string(), caps["value"].to_string());
        }
    }

    if !map.is_empty() {
        for key in HWINFO_KEYS.iter() {
            if let Some(value) = map.get(*key) {
                buf.push_str("\t\t\t<tr>\n");
                buf.push_str(&format!("\t\t\t\t<td>{}</td>\n", key));
                buf.push_str(&format!("\t\t\t\t<td>{}</td>\n", value));
                buf.push_str("\t\t\t</tr>\n");
            }
        }
    }

    buf.push_str("\t\t</table>\n\t</body>\n</html>");
    buf
}

// cd mupdf/source && awk '/_extensions\[/,/}/' */*.c
lazy_static! {
pub static ref RECOGNIZED_KINDS: FxHashSet<&'static str> =
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
