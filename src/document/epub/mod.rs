use std::io::Read;
use std::fs::{self, File};
use std::path::{Path, PathBuf};
use std::borrow::Cow;
use std::collections::{HashMap, BTreeSet};
use zip::ZipArchive;
use failure::{Error, format_err};
use crate::framebuffer::Pixmap;
use crate::helpers::{Normalize, decode_entities};
use crate::document::{Document, Location, TextLocation, TocEntry, BoundedText, chapter_from_uri};
use crate::unit::pt_to_px;
use crate::geom::{Rectangle, Edge, CycleDir};
use super::html::dom::Node;
use super::html::engine::{Page, Engine, ResourceFetcher};
use super::html::layout::{StyleData, LoopContext};
use super::html::layout::{RootData, DrawState, DrawCommand, TextCommand, ImageCommand};
use super::html::layout::TextAlign;
use super::html::css::{CssParser, RuleKind};
use super::html::xml::XmlParser;

const VIEWER_STYLESHEET: &str = "css/epub.css";
const USER_STYLESHEET: &str = "user.css";

type UriCache = HashMap<String, usize>;

impl ResourceFetcher for ZipArchive<File> {
    fn fetch(&mut self, name: &str) -> Result<Vec<u8>, Error> {
        let mut file = self.by_name(name)?;
        let mut buf = Vec::new();
        file.read_to_end(&mut buf)?;
        Ok(buf)
    }
}

pub struct EpubDocument {
    archive: ZipArchive<File>,
    info: Node,
    parent: PathBuf,
    engine: Engine,
    spine: Vec<Chunk>,
    cache: HashMap<usize, Vec<Page>>,
    ignore_document_css: bool,
}

#[derive(Debug)]
struct Chunk {
    path: String,
    size: usize,
}

unsafe impl Send for EpubDocument {}
unsafe impl Sync for EpubDocument {}

impl EpubDocument {
    pub fn new<P: AsRef<Path>>(path: P) -> Result<EpubDocument, Error> {
        let file = File::open(path)?;
        let mut archive = ZipArchive::new(file)?;

        let opf_path = {
            let mut zf = archive.by_name("META-INF/container.xml")?;
            let mut text = String::new();
            zf.read_to_string(&mut text)?;
            let root = XmlParser::new(&text).parse();
            root.find("rootfile")
                .and_then(|e| e.attr("full-path"))
                .map(String::from)
        }.ok_or_else(|| format_err!("Can't get the OPF path."))?;

        let parent = Path::new(&opf_path).parent()
                          .unwrap_or_else(|| Path::new(""));

        let text = {
            let mut zf = archive.by_name(&opf_path)?;
            let mut text = String::new();
            zf.read_to_string(&mut text)?;
            text
        };

        let info = XmlParser::new(&text).parse();
        let mut spine = Vec::new();

        {
            let manifest = info.find("manifest")
                                  .ok_or_else(|| format_err!("The manifest is missing."))?;

            let children = info.find("spine")
                                  .and_then(Node::children)
                                  .ok_or_else(|| format_err!("The spine is missing."))?;

            for child in children {
                let vertebra_opt = child.attr("idref").and_then(|idref| {
                    manifest.find_by_id(idref)
                }).and_then(|entry| {
                    entry.attr("href")
                }).and_then(|href| {
                    let href_path = parent.join(&href.replace("%20", " ").replace("&amp;", "&"));
                    href_path.to_str().and_then(|path| {
                        archive.by_name(path).map_err(|e| {
                            eprintln!("Can't retrieve '{}' from the archive: {}.", path, e)
                        // We're assuming that the size of the spine is less than 4 GiB.
                        }).map(|zf| (zf.size() as usize, path.to_string())).ok()
                    })
                });

                if let Some((size, path)) = vertebra_opt {
                    spine.push(Chunk { path, size });
                }
            }
        }

        if spine.is_empty() {
            return Err(format_err!("The spine is empty."));
        }

        Ok(EpubDocument {
            archive,
            info,
            parent: parent.to_path_buf(),
            engine: Engine::new(),
            spine,
            cache: HashMap::new(),
            ignore_document_css: false,
        })
    }

    fn offset(&self, index: usize) -> usize {
        self.spine.iter().take(index).map(|c| c.size).sum()
    }

    fn size(&self) -> usize {
        self.offset(self.spine.len())
    }

    fn vertebra_coordinates_with<F>(&self, test: F) -> Option<(usize, usize)>
                           where F: Fn(usize, usize) -> bool {
        let mut start_offset = 0;
        let mut end_offset = start_offset;
        let mut index = 0;

        while index < self.spine.len() {
            end_offset += self.spine[index].size;
            if test(index, end_offset) {
                return Some((index, start_offset))
            }
            start_offset = end_offset;
            index += 1;
        }

        None
    }

    fn vertebra_coordinates(&self, offset: usize) -> Option<(usize, usize)> {
        self.vertebra_coordinates_with(|_, end_offset| {
            offset < end_offset
        })
    }

    fn vertebra_coordinates_from_name(&self, name: &str) -> Option<(usize, usize)> {
        self.vertebra_coordinates_with(|index, _| {
            self.spine[index].path == name
        })
    }

    fn set_margin(&mut self, margin: &Edge) {
        self.engine.set_margin(margin);
        self.cache.clear();
    }

    fn set_font_size(&mut self, font_size: f32) {
        self.engine.set_font_size(font_size);
        self.cache.clear();
    }

    pub fn set_ignore_document_css(&mut self, value: bool) {
        self.ignore_document_css = value;
        self.cache.clear();
    }

    #[inline]
    fn rect(&self) -> Rectangle {
        let (width, height) = self.engine.dims;
        rect![0, 0, width as i32, height as i32]
    }

    fn walk_toc(&mut self, node: &Node, toc_dir: &Path, index: &mut usize, cache: &mut UriCache) -> Vec<TocEntry> {
        let mut entries = Vec::new();
        // TODO: Take `playOrder` into account?

        if let Some(children) = node.children() {
            for child in children {
                if child.tag_name() == Some("navPoint") {
                    let title = child.find("navLabel").and_then(|label| {
                        label.find("text")
                    }).and_then(|text| {
                        text.text().map(decode_entities).map(Cow::into_owned)
                    }).unwrap_or_default();

                    // Example URI: pr03.html#codecomma_and_what_to_do_with_it
                    let rel_uri = child.find("content").and_then(|content| {
                        content.attr("src").map(String::from)
                    }).unwrap_or_default();

                    let loc = toc_dir.join(&rel_uri).normalize().to_str()
                                     .map(|uri| Location::Uri(uri.to_string()));

                    let current_index = *index;
                    *index += 1;

                    let sub_entries = if child.children().map(|c| c.len() > 2) == Some(true) {
                        self.walk_toc(child, toc_dir, index, cache)
                    } else {
                        Vec::new()
                    };

                    if let Some(location) = loc {
                        entries.push(TocEntry {
                            title,
                            location,
                            index: current_index,
                            children: sub_entries,
                        });
                    }
                }
            }
        }

        entries
    }

    #[inline]
    fn page_index(&mut self, offset: usize, index: usize, start_offset: usize) -> Option<usize> {
        if !self.cache.contains_key(&index) {
            let display_list = self.build_display_list(index, start_offset);
            self.cache.insert(index, display_list);
        }
        self.cache.get(&index).map(|display_list| {
            if display_list.len() < 2 || display_list[1].first().map(|dc| offset < dc.offset()) == Some(true) {
                return 0;
            } else if display_list[display_list.len() - 1].first().map(|dc| offset >= dc.offset()) == Some(true) {
                return display_list.len() - 1;
            } else {
                for i in 1..display_list.len()-1 {
                    if display_list[i].first().map(|dc| offset >= dc.offset()) == Some(true) &&
                       display_list[i+1].first().map(|dc| offset < dc.offset()) == Some(true) {
                        return i;
                    }
                }
            }
            0
        })
    }

    fn resolve_link(&mut self, uri: &str, cache: &mut UriCache) -> Option<usize> {
        let frag_index_opt = uri.find('#');
        let name = &uri[..frag_index_opt.unwrap_or_else(|| uri.len())];

        let (index, start_offset) = self.vertebra_coordinates_from_name(name)?;

        if frag_index_opt.is_some() {
            let mut text = String::new();
            {
                let mut zf = self.archive.by_name(name).ok()?;
                zf.read_to_string(&mut text).ok()?;
            }
            let root = XmlParser::new(&text).parse();
            self.cache_uris(&root, name, start_offset, cache);
            cache.get(uri).cloned()
        } else {
            let page_index = self.page_index(start_offset, index, start_offset)?;
            let offset = self.cache.get(&index)
                             .and_then(|display_list| display_list[page_index].first())
                             .map(DrawCommand::offset)?;
            cache.insert(uri.to_string(), offset);
            Some(offset)
        }
    }

    fn cache_uris(&mut self, node: &Node, name: &str, start_offset: usize, cache: &mut UriCache) {
        if let Some(id) = node.attr("id") {
            let location = start_offset + node.offset();
            cache.insert(format!("{}#{}", name, id), location);
        }
        if let Some(children) = node.children() {
            for child in children {
                self.cache_uris(child, name, start_offset, cache);
            }
        }
    }

    fn images(&mut self, loc: Location) -> Option<(Vec<Rectangle>, usize)> {
        if self.spine.is_empty() {
            return None;
        }

        let offset = self.resolve_location(loc)?;
        let (index, start_offset) = self.vertebra_coordinates(offset)?;
        let page_index = self.page_index(offset, index, start_offset)?;

        self.cache.get(&index).map(|display_list| {
            (display_list[page_index].iter().filter_map(|dc| {
                match dc {
                    DrawCommand::Image(ImageCommand { rect, .. }) => Some(*rect),
                    _ => None,
                }
            }).collect(), offset)
        })
    }

    fn build_display_list(&mut self, index: usize, start_offset: usize) -> Vec<Page> {
        let mut text = String::new();
        let mut spine_dir = PathBuf::from("");

        {
            let path = &self.spine[index].path;
            if let Some(parent) = Path::new(path).parent() {
                spine_dir = parent.to_path_buf();
            }

            if let Ok(mut zf) = self.archive.by_name(path) {
                zf.read_to_string(&mut text).ok();
            }
        }

        let mut root = XmlParser::new(&text).parse();
        root.wrap_lost_inlines();

        let mut stylesheet = Vec::new();

        if let Ok(text) = fs::read_to_string(VIEWER_STYLESHEET) {
            let (mut css, _) = CssParser::new(&text).parse(RuleKind::Viewer);
            stylesheet.append(&mut css);
        }

        if let Ok(text) = fs::read_to_string(USER_STYLESHEET) {
            let (mut css, _) = CssParser::new(&text).parse(RuleKind::User);
            stylesheet.append(&mut css);
        }

        if !self.ignore_document_css {
            if let Some(head) = root.find("head") {
                if let Some(children) = head.children() {
                    for child in children {
                        if child.tag_name() == Some("link") && child.attr("rel") == Some("stylesheet") {
                            if let Some(href) = child.attr("href") {
                                if let Some(name) = spine_dir.join(href).normalize().to_str() {
                                    let mut text = String::new();
                                    if let Ok(mut zf) = self.archive.by_name(name) {
                                        zf.read_to_string(&mut text).ok();
                                        let (mut css, _) = CssParser::new(&text).parse(RuleKind::Document);
                                        stylesheet.append(&mut css);
                                    }
                                }
                            }
                        } else if child.tag_name() == Some("style") && child.attr("type") == Some("text/css") {
                            if let Some(text) = child.text() {
                                let (mut css, _) = CssParser::new(text).parse(RuleKind::Document);
                                stylesheet.append(&mut css);
                            }
                        }
                    }
                }
            }
        }

        let mut display_list = Vec::new();

        if let Some(body) = root.find("body").as_mut() {
            let mut rect = self.engine.rect();
            rect.shrink(&self.engine.margin);

            let language = self.language().or_else(|| {
                root.find("html")
                    .and_then(|html| html.attr("xml:lang"))
                    .map(String::from)
            });

            let style = StyleData {
                language,
                font_size: self.engine.font_size,
                line_height: pt_to_px(self.engine.line_height * self.engine.font_size, self.engine.dpi).round() as i32,
                text_align: self.engine.text_align,
                start_x: rect.min.x,
                end_x: rect.max.x,
                width: rect.max.x - rect.min.x,
                .. Default::default()
            };

            let loop_context = LoopContext::default();
            let mut draw_state = DrawState {
                position: pt!(rect.min.x, rect.min.y),
                .. Default::default()
            };

            let root_data = RootData {
                start_offset,
                spine_dir,
                rect,
            };

            display_list.push(Vec::new());

            self.engine.build_display_list(body, &style, &loop_context, &stylesheet, &root_data, &mut self.archive, &mut draw_state, &mut display_list);

            display_list.retain(|page| !page.is_empty());

            if display_list.is_empty() {
                display_list.push(vec![DrawCommand::Marker(start_offset + body.offset())]);
            }
        }

        display_list
    }

    pub fn metadata_by_name(&self, name: &str) -> Option<String> {
        self.info.find("metadata")
            .and_then(Node::children)
            .and_then(|children| children.iter()
                                         .find(|child| child.tag_name() == Some("meta") &&
                                                       child.attr("name") == Some(name)))
            .and_then(|child| child.attr("content").map(|s| decode_entities(s).into_owned()))
    }

    pub fn categories(&self) -> BTreeSet<String> {
        let mut result = BTreeSet::new();
        self.info.find("metadata")
            .and_then(Node::children)
            .map(|children| {
                for child in children {
                    if child.tag_name() == Some("dc:subject") {
                        if let Some(subject) = child.text().map(|text| decode_entities(text)) {
                            // Pipe separated list of BISAC categories
                            if subject.contains(" / ") {
                                for categ in subject.split('|') {
                                    let start_index = if let Some(index) = categ.find(" - ") { index+3 } else { 0 };
                                    result.insert(categ[start_index..].trim().replace(" / ", "."));
                                }
                            } else {
                                result.insert(subject.into_owned());
                            }
                        }
                    }
                }
            });
        result
    }

    fn chapter_aux<'a>(&mut self, toc: &'a [TocEntry], offset: usize, next_offset: usize, path: &str, chap_before: &mut Option<&'a TocEntry>, offset_before: &mut usize, chap_after: &mut Option<&'a TocEntry>, offset_after: &mut usize) {
        for entry in toc {
            if let Location::Uri(ref uri) = entry.location {
                if uri.starts_with(path) {
                    if let Some(entry_offset) = self.resolve_location(entry.location.clone()) {
                        if entry_offset < offset && (chap_before.is_none() || entry_offset > *offset_before) {
                            *chap_before = Some(entry);
                            *offset_before = entry_offset;
                        }
                        if entry_offset >= offset && entry_offset < next_offset && (chap_after.is_none() || entry_offset < *offset_after) {
                            *chap_after = Some(entry);
                            *offset_after = entry_offset;
                        }
                    }
                }
            }
            self.chapter_aux(&entry.children, offset, next_offset, path,
                             chap_before, offset_before, chap_after, offset_after);
        }
    }

    fn previous_chapter<'a>(&mut self, chap: Option<&TocEntry>, start_offset: usize, end_offset: usize, toc: &'a [TocEntry]) -> Option<&'a TocEntry> {
        for entry in toc.iter().rev() {
            let result = self.previous_chapter(chap, start_offset, end_offset, &entry.children);
            if result.is_some() {
                return result;
            }

            if let Some(chap) = chap {
                if entry.index < chap.index {
                    let entry_offset = self.resolve_location(entry.location.clone())?;
                    if entry_offset < start_offset || entry_offset >= end_offset {
                        return Some(entry)
                    }
                }
            } else {
                let entry_offset = self.resolve_location(entry.location.clone())?;
                if entry_offset < start_offset {
                    return Some(entry);
                }
            }
        }
        None
    }

    fn next_chapter<'a>(&mut self, chap: Option<&TocEntry>, start_offset: usize, end_offset: usize, toc: &'a [TocEntry]) -> Option<&'a TocEntry> {
        for entry in toc {
            if let Some(chap) = chap {
                if entry.index > chap.index {
                    let entry_offset = self.resolve_location(entry.location.clone())?;
                    if entry_offset < start_offset || entry_offset >= end_offset {
                        return Some(entry)
                    }
                }
            } else {
                let entry_offset = self.resolve_location(entry.location.clone())?;
                if entry_offset >= end_offset {
                    return Some(entry);
                }
            }

            let result = self.next_chapter(chap, start_offset, end_offset, &entry.children);
            if result.is_some() {
                return result;
            }
        }
        None
    }

    pub fn series(&self) -> Option<String> {
        self.metadata_by_name("calibre:series")
    }

    pub fn series_index(&self) -> Option<String> {
        self.metadata_by_name("calibre:series_index")
    }

    pub fn description(&self) -> Option<String> {
        self.metadata("dc:description")
    }

    pub fn publisher(&self) -> Option<String> {
        self.metadata("dc:publisher")
    }

    pub fn language(&self) -> Option<String> {
        self.metadata("dc:language")
    }

    pub fn year(&self) -> Option<String> {
        self.metadata("dc:date").map(|s| s.chars().take(4).collect())
    }
}

impl Document for EpubDocument {
    #[inline]
    fn dims(&self, _index: usize) -> Option<(f32, f32)> {
        Some((self.engine.dims.0 as f32, self.engine.dims.1 as f32))
    }

    fn pages_count(&self) -> usize {
        self.spine.iter().map(|c| c.size).sum()
    }

    fn toc(&mut self) -> Option<Vec<TocEntry>> {
        let name = self.info.find("spine").and_then(|spine| {
            spine.attr("toc")
        }).and_then(|toc_id| {
            self.info.find("manifest")
                .and_then(|manifest| manifest.find_by_id(toc_id))
                .and_then(|entry| entry.attr("href"))
        }).map(|href| {
            self.parent.join(href).normalize()
                .to_string_lossy().into_owned()
        })?;

        let toc_dir = Path::new(&name).parent()
                           .unwrap_or_else(|| Path::new(""));

        let mut text = String::new();
        if let Ok(mut zf) = self.archive.by_name(&name) {
            zf.read_to_string(&mut text).ok()?;
        } else {
            return None;
        }

        let root = XmlParser::new(&text).parse();
        root.find("navMap").map(|map| {
            let mut cache = HashMap::new();
            let mut index = 0;
            self.walk_toc(&map, &toc_dir, &mut index, &mut cache)
        })
    }

    fn chapter<'a>(&mut self, offset: usize, toc: &'a [TocEntry]) -> Option<&'a TocEntry> {
        let next_offset = self.resolve_location(Location::Next(offset))
                              .unwrap_or(usize::max_value());
        let (index, _) = self.vertebra_coordinates(offset)?;
        let path = self.spine[index].path.clone();
        let mut chap_before = None;
        let mut chap_after = None;
        let mut offset_before = 0;
        let mut offset_after = usize::max_value();
        self.chapter_aux(toc, offset, next_offset, &path,
                         &mut chap_before, &mut offset_before,
                         &mut chap_after, &mut offset_after);
        if chap_after.is_none() && chap_before.is_none() {
            for i in (0..index).rev() {
                let chap = chapter_from_uri(&self.spine[i].path, toc);
                if chap.is_some() {
                    return chap;
                }
            }
            None
        } else {
            chap_after.or(chap_before)
        }
    }

    fn chapter_relative<'a>(&mut self, offset: usize, dir: CycleDir, toc: &'a [TocEntry]) -> Option<&'a TocEntry> {
        let next_offset = self.resolve_location(Location::Next(offset))
                              .unwrap_or(usize::max_value());
        let chap = self.chapter(offset, toc);

        match dir {
            CycleDir::Previous => self.previous_chapter(chap, offset, next_offset, toc),
            CycleDir::Next => self.next_chapter(chap, offset, next_offset, toc),
        }
    }

    fn resolve_location(&mut self, loc: Location) -> Option<usize> {
        self.engine.load_fonts();

        match loc {
            Location::Exact(offset) => {
                let (index, start_offset) = self.vertebra_coordinates(offset)?;
                let page_index = self.page_index(offset, index, start_offset)?;
                self.cache.get(&index)
                    .and_then(|display_list| display_list[page_index].first())
                    .map(DrawCommand::offset)
            },
            Location::Previous(offset) => {
                let (index, start_offset) = self.vertebra_coordinates(offset)?;
                let page_index = self.page_index(offset, index, start_offset)?;
                if page_index > 0 {
                    self.cache.get(&index)
                        .and_then(|display_list| display_list[page_index-1].first().map(DrawCommand::offset))
                } else {
                    if index == 0 {
                        return None;
                    }
                    let (index, start_offset) = (index - 1, start_offset - self.spine[index-1].size);
                    if !self.cache.contains_key(&index) {
                        let display_list = self.build_display_list(index, start_offset);
                        self.cache.insert(index, display_list);
                    }
                    self.cache.get(&index)
                        .and_then(|display_list| display_list.last().and_then(|page| page.first()).map(DrawCommand::offset))
                }
            },
            Location::Next(offset) => {
                let (index, start_offset) = self.vertebra_coordinates(offset)?;
                let page_index = self.page_index(offset, index, start_offset)?;
                if page_index < self.cache.get(&index).map(Vec::len)? - 1 {
                    self.cache.get(&index).and_then(|display_list| display_list[page_index+1].first().map(DrawCommand::offset))
                } else {
                    if index == self.spine.len() - 1 {
                        return None;
                    }
                    let (index, start_offset) = (index + 1, start_offset + self.spine[index].size);
                    if !self.cache.contains_key(&index) {
                        let display_list = self.build_display_list(index, start_offset);
                        self.cache.insert(index, display_list);
                    }
                    self.cache.get(&index)
                        .and_then(|display_list| display_list.first().and_then(|page| page.first()).map(|dc| dc.offset()))
                }
            },
            Location::LocalUri(offset, ref uri) => {
                let mut cache = HashMap::new();
                let normalized_uri: String = {
                    let (index, _) = self.vertebra_coordinates(offset)?;
                    let path = &self.spine[index].path;
                    if uri.starts_with('#') {
                        format!("{}{}", path, uri)
                    } else {
                        let parent = Path::new(path).parent()
                                          .unwrap_or_else(|| Path::new(""));
                        parent.join(uri).normalize()
                              .to_string_lossy().into_owned()
                    }
                };
                self.resolve_link(&normalized_uri, &mut cache)
            },
            Location::Uri(ref uri) => {
                let mut cache = HashMap::new();
                self.resolve_link(uri, &mut cache)
            },
        }
    }

    fn words(&mut self, loc: Location) -> Option<(Vec<BoundedText>, usize)> {
        if self.spine.is_empty() {
            return None;
        }

        let offset = self.resolve_location(loc)?;
        let (index, start_offset) = self.vertebra_coordinates(offset)?;
        let page_index = self.page_index(offset, index, start_offset)?;

        self.cache.get(&index).map(|display_list| {
            (display_list[page_index].iter().filter_map(|dc| {
                match dc {
                    DrawCommand::Text(TextCommand { text, rect, offset, .. }) => {
                        Some(BoundedText {
                            text: text.clone(),
                            rect: (*rect).into(),
                            location: TextLocation::Dynamic(*offset),
                        })
                    },
                    _ => None,
                }
            }).collect(), offset)
        })
    }

    fn lines(&mut self, _loc: Location) -> Option<(Vec<BoundedText>, usize)> {
        None
    }

    fn links(&mut self, loc: Location) -> Option<(Vec<BoundedText>, usize)> {
        if self.spine.is_empty() {
            return None;
        }

        let offset = self.resolve_location(loc)?;
        let (index, start_offset) = self.vertebra_coordinates(offset)?;
        let page_index = self.page_index(offset, index, start_offset)?;

        self.cache.get(&index).map(|display_list| {
            (display_list[page_index].iter().filter_map(|dc| {
                match dc {
                    DrawCommand::Text(TextCommand { uri, rect, offset, .. }) |
                    DrawCommand::Image(ImageCommand { uri, rect, offset, .. }) if uri.is_some() => {
                        Some(BoundedText {
                            text: uri.clone().unwrap(),
                            rect: (*rect).into(),
                            location: TextLocation::Dynamic(*offset),
                        })
                    },
                    _ => None,
                }
            }).collect(), offset)
        })
    }

    fn pixmap(&mut self, loc: Location, _scale: f32) -> Option<(Pixmap, usize)> {
        if self.spine.is_empty() {
            return None;
        }

        let offset = self.resolve_location(loc)?;
        let (index, start_offset) = self.vertebra_coordinates(offset)?;

        let page_index = self.page_index(offset, index, start_offset)?;
        let page = self.cache.get(&index)?.get(page_index)?.clone();

        let pixmap = self.engine.render_page(&page, &mut self.archive);

        Some((pixmap, offset))
    }

    fn layout(&mut self, width: u32, height: u32, font_size: f32, dpi: u16) {
        self.engine.layout(width, height, font_size, dpi);
        self.cache.clear();
    }

    fn set_text_align(&mut self, text_align: TextAlign) {
        self.engine.set_text_align(text_align);
        self.cache.clear();
    }

    fn set_font_family(&mut self, family_name: &str, search_path: &str) {
        self.engine.set_font_family(family_name, search_path);
        self.cache.clear();
    }

    fn set_margin_width(&mut self, width: i32) {
        self.engine.set_margin_width(width);
        self.cache.clear();
    }

    fn set_line_height(&mut self, line_height: f32) {
        self.engine.set_line_height(line_height);
        self.cache.clear();
    }

    fn title(&self) -> Option<String> {
        self.metadata("dc:title")
    }

    fn author(&self) -> Option<String> {
        // TODO: Consider the opf:file-as attribute?
        self.metadata("dc:creator")
    }

    fn metadata(&self, key: &str) -> Option<String> {
        self.info.find("metadata")
            .and_then(Node::children)
            .and_then(|children| children.iter().find(|child| child.tag_name() == Some(key)))
            .and_then(|child| child.children().and_then(|c| c.get(0)))
            .and_then(|child| child.text().map(|s| decode_entities(s).into_owned()))
    }

    fn is_reflowable(&self) -> bool {
        true
    }

    fn has_synthetic_page_numbers(&self) -> bool {
        true
    }
}
