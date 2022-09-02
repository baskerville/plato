use std::io::Read;
use std::fs::{self, File};
use std::path::{Path, PathBuf};
use std::collections::BTreeSet;
use fxhash::FxHashMap;
use zip::ZipArchive;
use percent_encoding::percent_decode_str;
use anyhow::{Error, format_err};
use crate::framebuffer::Pixmap;
use crate::helpers::{Normalize, decode_entities};
use crate::document::{Document, Location, TextLocation, TocEntry, BoundedText, chapter_from_uri};
use crate::unit::pt_to_px;
use crate::geom::{Boundary, CycleDir};
use super::pdf::PdfOpener;
use super::html::dom::{XmlTree, NodeRef};
use super::html::engine::{Page, Engine, ResourceFetcher};
use super::html::layout::{StyleData, LoopContext};
use super::html::layout::{RootData, DrawState, DrawCommand, TextCommand, ImageCommand};
use super::html::layout::TextAlign;
use super::html::style::StyleSheet;
use super::html::css::CssParser;
use super::html::xml::XmlParser;

const VIEWER_STYLESHEET: &str = "css/epub.css";
const USER_STYLESHEET: &str = "css/epub-user.css";

type UriCache = FxHashMap<String, usize>;

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
    info: XmlTree,
    parent: PathBuf,
    engine: Engine,
    spine: Vec<Chunk>,
    cache: FxHashMap<usize, Vec<Page>>,
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
            root.root().find("rootfile")
                .and_then(|e| e.attribute("full-path"))
                .map(String::from)
        }.ok_or_else(|| format_err!("can't get the OPF path"))?;

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
            let manifest = info.root().find("manifest")
                               .ok_or_else(|| format_err!("the manifest is missing"))?;

            let spn = info.root().find("spine")
                         .ok_or_else(|| format_err!("the spine is missing"))?;

            for child in spn.children() {
                let vertebra_opt = child.attribute("idref").and_then(|idref| {
                    manifest.find_by_id(idref)
                }).and_then(|entry| {
                    entry.attribute("href")
                }).and_then(|href| {
                    let href = decode_entities(href);
                    let href = percent_decode_str(&href).decode_utf8_lossy();
                    let href_path = parent.join(href.as_ref());
                    href_path.to_str().and_then(|path| {
                        archive.by_name(path).map_err(|e| {
                            eprintln!("Can't retrieve '{}' from the archive: {:#}.", path, e)
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
            return Err(format_err!("the spine is empty"));
        }

        Ok(EpubDocument {
            archive,
            info,
            parent: parent.to_path_buf(),
            engine: Engine::new(),
            spine,
            cache: FxHashMap::default(),
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

    fn walk_toc_ncx(&mut self, node: NodeRef, toc_dir: &Path, index: &mut usize, cache: &mut UriCache) -> Vec<TocEntry> {
        let mut entries = Vec::new();
        // TODO: Take `playOrder` into account?

        for child in node.children() {
            if child.tag_name() == Some("navPoint") {
                let title = child.find("navLabel").and_then(|label| {
                    label.find("text")
                }).map(|text| {
                    decode_entities(&text.text()).into_owned()
                }).unwrap_or_default();

                // Example URI: pr03.html#codecomma_and_what_to_do_with_it
                let rel_uri = child.find("content").and_then(|content| {
                    content.attribute("src")
                           .map(|src| percent_decode_str(&decode_entities(src)).decode_utf8_lossy()
                                                                               .into_owned())
                }).unwrap_or_default();

                let loc = toc_dir.join(&rel_uri).normalize().to_str()
                                 .map(|uri| Location::Uri(uri.to_string()));

                let current_index = *index;
                *index += 1;

                let sub_entries = if child.children().count() > 2 {
                    self.walk_toc_ncx(child, toc_dir, index, cache)
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

        entries
    }

    fn walk_toc_nav(&mut self, node: NodeRef, toc_dir: &Path, index: &mut usize, cache: &mut UriCache) -> Vec<TocEntry> {
        let mut entries = Vec::new();

        for child in node.children() {
            if child.tag_name() == Some("li") {
                let link = child.children()
                                .find(|child| child.tag_name() == Some("a"));
                let title = link.map(|link| {
                    decode_entities(&link.text()).into_owned()
                }).unwrap_or_default();
                let rel_uri = link.and_then(|link| {
                    link.attribute("href")
                        .map(|href| percent_decode_str(&decode_entities(href))
                                                      .decode_utf8_lossy()
                                                      .into_owned())
                }).unwrap_or_default();

                let loc = toc_dir.join(&rel_uri).normalize().to_str()
                                 .map(|uri| Location::Uri(uri.to_string()));

                let current_index = *index;
                *index += 1;

                let sub_entries = if let Some(sub_list) = child.find("ol") {
                    self.walk_toc_nav(sub_list, toc_dir, index, cache)
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
            self.cache_uris(root.root(), name, start_offset, cache);
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

    fn cache_uris(&mut self, node: NodeRef, name: &str, start_offset: usize, cache: &mut UriCache) {
        if let Some(id) = node.attribute("id") {
            let location = start_offset + node.offset();
            cache.insert(format!("{}#{}", name, id), location);
        }
        for child in node.children() {
            self.cache_uris(child, name, start_offset, cache);
        }
    }

    fn build_display_list(&mut self, index: usize, start_offset: usize) -> Vec<Page> {
        let mut text = String::new();
        let mut spine_dir = PathBuf::default();

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

        let mut stylesheet = StyleSheet::new();

        if let Ok(text) = fs::read_to_string(VIEWER_STYLESHEET) {
            let mut css = CssParser::new(&text).parse();
            stylesheet.append(&mut css, true);
        }

        if let Ok(text) = fs::read_to_string(USER_STYLESHEET) {
            let mut css = CssParser::new(&text).parse();
            stylesheet.append(&mut css, true);
        }

        if !self.ignore_document_css {
            let mut inner_css = StyleSheet::new();
            if let Some(head) = root.root().find("head") {
                for child in head.children() {
                    if child.tag_name() == Some("link") && child.attribute("rel") == Some("stylesheet") {
                        if let Some(href) = child.attribute("href") {
                            if let Some(name) = spine_dir.join(href).normalize().to_str() {
                                let mut text = String::new();
                                if let Ok(mut zf) = self.archive.by_name(name) {
                                    zf.read_to_string(&mut text).ok();
                                    let mut css = CssParser::new(&text).parse();
                                    inner_css.append(&mut css, false);
                                }
                            }
                        }
                    } else if child.tag_name() == Some("style") && child.attribute("type") == Some("text/css") {
                        let mut css = CssParser::new(&child.text()).parse();
                        inner_css.append(&mut css, false);
                    }
                }
            }

            stylesheet.append(&mut inner_css, true);
        }

        let mut display_list = Vec::new();

        if let Some(body) = root.root().find("body") {
            let mut rect = self.engine.rect();
            rect.shrink(&self.engine.margin);

            let language = self.language().or_else(|| {
                root.root().find("html")
                    .and_then(|html| html.attribute("xml:lang"))
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
                position: rect.min,
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

    pub fn categories(&self) -> BTreeSet<String> {
        let mut result = BTreeSet::new();

        if let Some(md) = self.info.root().find("metadata") {
            for child in md.children() {
                if child.tag_qualified_name() == Some("dc:subject") {
                    let text = child.text();
                    let subject = decode_entities(&text);
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

        result
    }

    fn chapter_aux<'a>(&mut self, toc: &'a [TocEntry], offset: usize, next_offset: usize, path: &str, end_offset: &mut usize,
                       chap_before: &mut Option<&'a TocEntry>, offset_before: &mut usize, chap_after: &mut Option<&'a TocEntry>, offset_after: &mut usize) {
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
                        if entry_offset >= next_offset && entry_offset < *end_offset {
                            *end_offset = entry_offset;
                        }
                    }
                }
            }
            self.chapter_aux(&entry.children, offset, next_offset, path, end_offset,
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

    pub fn series(&self) -> Option<(String, String)> {
        self.info.root().find("metadata")
            .and_then(|md| {
                let mut title = None;
                let mut index = None;

                for child in md.children() {
                    if child.tag_name() == Some("meta") {
                        if child.attribute("name") == Some("calibre:series") {
                            title = child.attribute("content").map(|s| decode_entities(s).into_owned());
                        } else if child.attribute("name") == Some("calibre:series_index") {
                            index = child.attribute("content").map(|s| decode_entities(s).into_owned());
                        } else if child.attribute("property") == Some("belongs-to-collection") {
                            title = Some(decode_entities(&child.text()).into_owned());
                        } else if child.attribute("property") == Some("group-position") {
                            index = Some(decode_entities(&child.text()).into_owned());
                        }
                    }

                    if title.is_some() && index.is_some() {
                        break;
                    }
                }

                title.into_iter().zip(index).next()
            })
    }

    pub fn cover_image(&self) -> Option<&str> {
        self.info.root().find("metadata")
            .and_then(|md| md.children().find(|child| {
                child.tag_name() == Some("meta") &&
                child.attribute("name") == Some("cover")
            }))
            .and_then(|entry| entry.attribute("content"))
            .and_then(|cover_id| {
                self.info.root().find("manifest")
                    .and_then(|entry| entry.find_by_id(cover_id))
                    .and_then(|entry| entry.attribute("href"))
            })
            .or_else(|| {
                self.info.root().find("manifest")
                    .and_then(|mf| mf.children().find(|child| {
                        (child.attribute("href").map_or(false, |hr| hr.contains("cover") || hr.contains("Cover")) ||
                         child.id().map_or(false, |id| id.contains("cover"))) &&
                        child.attribute("media-type").map_or(false, |mt| mt.starts_with("image/"))
                    }))
                    .and_then(|entry| entry.attribute("href"))
            })
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
    fn preview_pixmap(&mut self, width: f32, height: f32) -> Option<Pixmap> {
        let opener = PdfOpener::new()?;
        self.cover_image()
            .map(|path| self.parent.join(path)
                            .to_string_lossy().into_owned())
            .and_then(|path| {
                self.archive.fetch(&path).ok()
                    .and_then(|buf| opener.open_memory(&path, &buf))
                    .and_then(|mut doc| {
                        doc.dims(0).and_then(|dims| {
                            let scale = (width / dims.0).min(height / dims.1);
                            doc.pixmap(Location::Exact(0), scale)
                        })
                    })
            })
            .or_else(|| {
                self.dims(0).and_then(|dims| {
                    let scale = (width / dims.0).min(height / dims.1);
                    self.pixmap(Location::Exact(0), scale)
                })
            })
            .map(|(pixmap, _)| pixmap)
    }

    #[inline]
    fn dims(&self, _index: usize) -> Option<(f32, f32)> {
        Some((self.engine.dims.0 as f32, self.engine.dims.1 as f32))
    }

    fn pages_count(&self) -> usize {
        self.spine.iter().map(|c| c.size).sum()
    }

    fn toc(&mut self) -> Option<Vec<TocEntry>> {
        let name = self.info.root().find("spine").and_then(|spine| {
            spine.attribute("toc")
        }).and_then(|toc_id| {
            self.info.root().find("manifest")
                .and_then(|manifest| manifest.find_by_id(toc_id))
                .and_then(|entry| entry.attribute("href"))
        }).or_else(|| {
            self.info.root().find("manifest")
                .and_then(|manifest| manifest.children().find(|child| {
                    child.attribute("properties").iter()
                         .any(|props| props.split_whitespace().any(|prop| prop == "nav"))
                }))
                .and_then(|entry| entry.attribute("href"))
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

        if name.ends_with(".ncx") {
            root.root().find("navMap").map(|map| {
                self.walk_toc_ncx(map, toc_dir, &mut 0, &mut FxHashMap::default())
            })
        } else {
            root.root().descendants()
                .find(|desc| desc.tag_name() == Some("nav") &&
                             desc.attribute("epub:type") == Some("toc"))
                .and_then(|map| map.find("ol")).map(|map| {
                self.walk_toc_nav(map, toc_dir, &mut 0, &mut FxHashMap::default())
            })
        }
    }

    fn chapter<'a>(&mut self, offset: usize, toc: &'a [TocEntry]) -> Option<(&'a TocEntry, f32)> {
        let next_offset = self.resolve_location(Location::Next(offset))
                              .unwrap_or(usize::MAX);
        let (index, start_offset) = self.vertebra_coordinates(offset)?;
        let path = self.spine[index].path.clone();
        let mut end_offset = start_offset + self.spine[index].size;
        let mut chap_before = None;
        let mut chap_after = None;
        let mut offset_before = 0;
        let mut offset_after = usize::MAX;

        self.chapter_aux(toc, offset, next_offset, &path, &mut end_offset,
                         &mut chap_before, &mut offset_before,
                         &mut chap_after, &mut offset_after);

        if chap_after.is_none() && chap_before.is_none() {
            for i in (0..index).rev() {
                let chap = chapter_from_uri(&self.spine[i].path, toc);
                if chap.is_some() {
                    end_offset = if let Some(j) = (index+1..self.spine.len()).find(|&j| chapter_from_uri(&self.spine[j].path, toc).is_some()) {
                        self.offset(j)
                    } else {
                        self.size()
                    };
                    let chap_offset = self.offset(i);
                    let progress = (offset - chap_offset) as f32 / (end_offset - chap_offset) as f32;
                    return chap.zip(Some(progress));
                }
            }
            None
        } else {
            match (chap_after, chap_before) {
                (Some(..), _) => chap_after.zip(Some(0.0)),
                (None, Some(..)) => chap_before.zip(Some((offset - offset_before) as f32 / (end_offset - offset_before) as f32)),
                _ => None,
            }
        }
    }

    fn chapter_relative<'a>(&mut self, offset: usize, dir: CycleDir, toc: &'a [TocEntry]) -> Option<&'a TocEntry> {
        let next_offset = self.resolve_location(Location::Next(offset))
                              .unwrap_or(usize::MAX);
        let chap = self.chapter(offset, toc).map(|(c, _)| c);

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
                let mut cache = FxHashMap::default();
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
                let mut cache = FxHashMap::default();
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

    fn images(&mut self, loc: Location) -> Option<(Vec<Boundary>, usize)> {
        if self.spine.is_empty() {
            return None;
        }

        let offset = self.resolve_location(loc)?;
        let (index, start_offset) = self.vertebra_coordinates(offset)?;
        let page_index = self.page_index(offset, index, start_offset)?;

        self.cache.get(&index).map(|display_list| {
            (display_list[page_index].iter().filter_map(|dc| {
                match dc {
                    DrawCommand::Image(ImageCommand { rect, .. }) => Some((*rect).into()),
                    _ => None,
                }
            }).collect(), offset)
        })
    }

    fn pixmap(&mut self, loc: Location, scale: f32) -> Option<(Pixmap, usize)> {
        if self.spine.is_empty() {
            return None;
        }

        let offset = self.resolve_location(loc)?;
        let (index, start_offset) = self.vertebra_coordinates(offset)?;

        let page_index = self.page_index(offset, index, start_offset)?;
        let page = self.cache.get(&index)?.get(page_index)?.clone();

        let pixmap = self.engine.render_page(&page, scale, &mut self.archive)?;

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

    fn set_hyphen_penalty(&mut self, hyphen_penalty: i32) {
        self.engine.set_hyphen_penalty(hyphen_penalty);
        self.cache.clear();
    }

    fn set_stretch_tolerance(&mut self, stretch_tolerance: f32) {
        self.engine.set_stretch_tolerance(stretch_tolerance);
        self.cache.clear();
    }

    fn set_ignore_document_css(&mut self, ignore: bool) {
        self.ignore_document_css = ignore;
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
        self.info.root().find("metadata")
            .and_then(|md| md.children().find(|child| child.tag_qualified_name() == Some(key)))
            .map(|child| decode_entities(&child.text()).into_owned())
    }

    fn is_reflowable(&self) -> bool {
        true
    }

    fn has_synthetic_page_numbers(&self) -> bool {
        true
    }
}
