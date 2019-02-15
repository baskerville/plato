mod dom;
pub mod xml;
mod css;
mod parse;
mod style;
mod layout;

use std::io::Read;
use std::fs::{self, File};
use std::path::{Path, PathBuf};
use std::borrow::Cow;
use std::collections::BTreeSet;
use fnv::FnvHashMap;
use zip::ZipArchive;
use hyphenation::{Standard, Hyphenator, Iter};
use failure::{Error, format_err};
use crate::framebuffer::{Framebuffer, Pixmap};
use crate::helpers::Normalize;
use crate::font::{FontOpener, FontFamily};
use crate::document::{Document, Location, TocEntry, BoundedText};
use crate::document::pdf::PdfOpener;
use paragraph_breaker::{Item as ParagraphItem, Breakpoint, INFINITE_PENALTY};
use paragraph_breaker::{total_fit, standard_fit};
use xi_unicode::LineBreakIterator;
use crate::settings::{DEFAULT_FONT_SIZE, DEFAULT_MARGIN_WIDTH, DEFAULT_LINE_HEIGHT};
use crate::unit::{mm_to_px, pt_to_px};
use crate::geom::{Point, Rectangle, Edge};
use self::parse::{parse_display, parse_edge, parse_text_align, parse_text_indent, parse_width, parse_height, parse_inline_material};
use self::parse::{parse_font_kind, parse_font_style, parse_font_weight, parse_font_size, parse_font_features, parse_font_variant, parse_letter_spacing};
use self::parse::{parse_line_height, parse_vertical_align, parse_color};
use self::dom::{Node, ElementData, TextData};
use self::layout::{StyleData, InlineMaterial, TextMaterial, ImageMaterial};
use self::layout::{GlueMaterial, PenaltyMaterial, ChildArtifact, SiblingStyle, LoopContext};
use self::layout::{RootData, DrawCommand, TextCommand, ImageCommand, FontKind, Fonts};
use self::layout::{TextAlign, ParagraphElement, TextElement, ImageElement, Display, LineStats};
use self::layout::{hyph_lang, collapse_margins, DEFAULT_HYPH_LANG, HYPHENATION_PATTERNS};
use self::layout::{EM_SPACE_RATIOS, WORD_SPACE_RATIOS, FONT_SPACES};
use self::style::{Stylesheet, specified_values};
use self::css::{CssParser, RuleKind};
use self::xml::{XmlParser, decode_entities};

const DEFAULT_DPI: u16 = 300;
const DEFAULT_WIDTH: u32 = 1404;
const DEFAULT_HEIGHT: u32 = 1872;
const HYPHEN_PENALTY: i32 = 50;
const STRETCH_TOLERANCE: f32 = 1.26;
const VIEWER_STYLESHEET: &str = "css/epub.css";
const USER_STYLESHEET: &str = "user.css";

type Page = Vec<DrawCommand>;
type UriCache = FnvHashMap<String, usize>;

// TODO: Add min_font_size.
pub struct EpubDocument {
    archive: ZipArchive<File>,
    content: Node,
    parent: PathBuf,
    spine: Vec<Chunk>,
    cache: FnvHashMap<usize, Vec<Page>>,
    fonts: Option<Fonts>,
    ignore_document_css: bool,
    margin: Edge,
    // Font size in points.
    font_size: f32,
    // Line height in ems.
    line_height: f32,
    // Page dimensions in pixels.
    dims: (u32, u32),
    // Device DPI.
    dpi: u16,
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

        let content = XmlParser::new(&text).parse();
        let mut spine = Vec::new();

        {
            let manifest = content.find("manifest")
                                  .ok_or_else(|| format_err!("The manifest is missing."))?;

            let children = content.find("spine")
                                  .and_then(|spine| spine.children())
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

        let margin = Edge::uniform(mm_to_px(DEFAULT_MARGIN_WIDTH as f32, DEFAULT_DPI).round() as i32);
        let line_height = DEFAULT_LINE_HEIGHT;

        Ok(EpubDocument {
            archive,
            content,
            parent: parent.to_path_buf(),
            spine,
            cache: FnvHashMap::default(),
            fonts: None,
            ignore_document_css: false,
            margin,
            font_size: DEFAULT_FONT_SIZE,
            line_height,
            dims: (DEFAULT_WIDTH, DEFAULT_HEIGHT),
            dpi: DEFAULT_DPI,
        })
    }

    fn offset(&self, index: usize) -> usize {
        self.spine.iter().take(index).map(|c| c.size).sum()
    }

    fn size(&self) -> usize {
        self.offset(self.spine.len())
    }

    fn vertebra_coordinates_with<F>(&self, test: F) -> (usize, usize) where F: Fn(usize, usize) -> bool {
        let mut start_offset = 0;
        let mut end_offset = start_offset;
        let mut index = 0;

        while index < self.spine.len() {
            end_offset += self.spine[index].size;
            if test(index, end_offset) {
                break;
            }
            start_offset = end_offset;
            index += 1;
        }

        if index == self.spine.len() {
            index -= 1;
            start_offset -= self.spine[index].size;
        }

        (index, start_offset)
    }

    fn vertebra_coordinates(&self, offset: usize) -> (usize, usize) {
        self.vertebra_coordinates_with(|_, end_offset| {
            offset < end_offset
        })
    }

    fn vertebra_coordinates_from_name(&self, name: &str) -> (usize, usize) {
        self.vertebra_coordinates_with(|index, _| {
            self.spine[index].path == name
        })
    }

    fn set_margin(&mut self, margin: &Edge) {
        self.margin = *margin;
        self.cache.clear();
    }

    fn set_font_size(&mut self, font_size: f32) {
        self.font_size = font_size;
        self.cache.clear();
    }

    fn set_ignore_document_css(&mut self, value: bool) {
        if self.ignore_document_css != value {
            self.ignore_document_css = value;
            self.cache.clear();
        }
    }

    #[inline]
    fn rect(&self) -> Rectangle {
        let (width, height) = self.dims;
        rect![0, 0, width as i32, height as i32]
    }

    fn walk_toc(&mut self, node: &Node, toc_dir: &Path, cache: &mut UriCache) -> Vec<TocEntry> {
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

                    let loc = toc_dir.join(&rel_uri).normalize().to_str().and_then(|uri| {
                        cache.get(uri).cloned().or_else(|| self.resolve_link(uri, cache))
                    });

                    let sub_entries = if child.children().map(|c| c.len() > 2) == Some(true) {
                        self.walk_toc(child, toc_dir, cache)
                    } else {
                        Vec::new()
                    };

                    if let Some(location) = loc {
                        entries.push(TocEntry {
                            title,
                            location,
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

        let (_, start_offset) = self.vertebra_coordinates_from_name(name);

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
            cache.insert(uri.to_string(), start_offset);
            Some(start_offset)
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
        let (index, start_offset) = self.vertebra_coordinates(offset);
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
            let mut style = StyleData::default();
            let mut rect = self.rect();

            rect.shrink(&self.margin);

            let language = self.metadata("dc:language").or_else(|| {
                root.find("html")
                    .and_then(|html| html.attr("xml:lang"))
                    .map(String::from)
            });

            style.language = language;
            style.font_size = self.font_size;
            style.line_height = pt_to_px(self.line_height * self.font_size, self.dpi).round() as i32;
            style.start_x = rect.min.x;
            style.end_x = rect.max.x;
            style.width = style.end_x - style.start_x;

            let loop_context = LoopContext::default();
            let mut position = pt!(rect.min.x, rect.min.y);

            let root_data = RootData {
                start_offset,
                spine_dir,
                rect,
            };

            display_list.push(Vec::new());

            self.build_display_list_rec(body, &style, &loop_context, &stylesheet, &root_data, &mut position, &mut display_list);

            display_list.retain(|page| !page.is_empty());

            if display_list.is_empty() {
                display_list.push(vec![DrawCommand::Marker(start_offset + body.offset())]);
            }

        }

        display_list
    }

    fn build_display_list_rec(&mut self, node: &Node, parent_style: &StyleData, loop_context: &LoopContext, stylesheet: &Stylesheet, root_data: &RootData, position: &mut Point, display_list: &mut Vec<Page>) -> ChildArtifact {
        // TODO: border, background, text-transform, tab-size.
        let mut style = StyleData::default();
        let mut rects = Vec::new();

        style.font_style = parent_style.font_style;
        style.line_height = parent_style.line_height;
        style.retain_whitespace = parent_style.retain_whitespace;

        if node.tag_name() == Some("pre") {
            style.retain_whitespace = true;
        }

        let props = specified_values(node, loop_context.parent, loop_context.sibling, stylesheet);

        style.display = props.get("display").and_then(|value| parse_display(value))
                             .unwrap_or(Display::Block);

        if style.display == Display::None {
            return ChildArtifact {
                sibling_style: SiblingStyle {
                    padding_bottom: 0,
                    margin_bottom: 0,
                },
                rects: Vec::new(),
            }
        }

        style.language = props.get("lang").cloned()
                              .or_else(|| parent_style.language.clone());

        style.font_size = props.get("font-size")
                               .and_then(|value| parse_font_size(value, parent_style.font_size, self.font_size))
                               .unwrap_or(parent_style.font_size);

        style.line_height = props.get("line-height")
                                 .and_then(|value| parse_line_height(value, style.font_size, self.font_size, self.dpi))
                                 .unwrap_or_else(|| ((style.font_size / parent_style.font_size) * parent_style.line_height as f32).round() as i32);

        style.letter_spacing = props.get("letter-spacing")
                                    .and_then(|value| parse_letter_spacing(value, style.font_size, self.font_size, self.dpi))
                                    .unwrap_or(parent_style.letter_spacing);

        style.vertical_align = props.get("vertical-align")
                                    .and_then(|value| parse_vertical_align(value, style.font_size, self.font_size, self.dpi))
                                    .unwrap_or(parent_style.vertical_align);

        style.font_kind = props.get("font-family")
                               .and_then(|value| parse_font_kind(value))
                               .unwrap_or(parent_style.font_kind);

        style.font_style = props.get("font-style")
                                .and_then(|value| parse_font_style(value))
                                .unwrap_or(parent_style.font_style);

        style.font_weight = props.get("font-weight")
                                .and_then(|value| parse_font_weight(value))
                                .unwrap_or(parent_style.font_weight);

        style.color = props.get("color")
                           .and_then(|value| parse_color(value))
                           .unwrap_or(parent_style.color);

        style.text_indent = props.get("text-indent")
                                 .and_then(|value| parse_text_indent(value, style.font_size, self.font_size,
                                                                 parent_style.width, self.dpi))
                                 .unwrap_or(parent_style.text_indent);

        style.text_align = props.get("text-align")
                                .and_then(|value| parse_text_align(value))
                                .unwrap_or(parent_style.text_align);

        style.font_features = props.get("font-feature-settings")
                                   .map(|value| parse_font_features(value))
                                   .or_else(|| parent_style.font_features.clone());

        if let Some(value) = props.get("font-variant") {
            let mut features = parse_font_variant(value);
            if let Some(v) = style.font_features.as_mut() {
                v.append(&mut features);
            }
        }

        if node.tag_name() != Some("body") {
            style.margin = parse_edge(props.get("margin-top").map(String::as_str),
                                      props.get("margin-right").map(String::as_str),
                                      props.get("margin-bottom").map(String::as_str),
                                      props.get("margin-left").map(String::as_str),
                                      style.font_size, self.font_size, parent_style.width, self.dpi);

            // Collapse the bottom margin of the previous sibling with the current top margin
            style.margin.top = collapse_margins(loop_context.sibling_style.margin_bottom, style.margin.top);

            // Collapse the top margin of the first child and its parent.
            if loop_context.is_first {
                style.margin.top = collapse_margins(parent_style.margin.top, style.margin.top);
            }

            style.padding = parse_edge(props.get("padding-top").map(String::as_str),
                                       props.get("padding-right").map(String::as_str),
                                       props.get("padding-bottom").map(String::as_str),
                                       props.get("padding-left").map(String::as_str),
                                       style.font_size, self.font_size, parent_style.width, self.dpi);
        }

        style.height = props.get("height")
                            .and_then(|value| parse_height(value, style.font_size, self.font_size, parent_style.width, self.dpi))
                            .unwrap_or(0);

        style.start_x = parent_style.start_x + style.margin.left + style.padding.left;
        style.end_x = parent_style.end_x - style.margin.right - style.padding.right;

        let mut width = style.end_x - style.start_x;

        if width < 0 {
            if style.width > 0 {
                let total_space = style.margin.left + style.padding.left + style.margin.right + style.padding.right;
                let remaining_space = parent_style.width - style.width;
                let ratio = remaining_space as f32 / total_space as f32;
                style.margin.left = (style.margin.left as f32 * ratio).round() as i32;
                style.padding.left = (style.padding.left as f32 * ratio).round() as i32;
                style.margin.right = (style.margin.right as f32 * ratio).round() as i32;
                // TODO: Make sure that this is always > 0.
                style.padding.right = remaining_space - (style.margin.left + style.padding.left + style.margin.right);
                style.start_x = parent_style.start_x + style.margin.left + style.padding.left;
                style.end_x = parent_style.end_x - style.margin.right - style.padding.right;
                width = style.width;
            } else {
                style.margin.left = 0;
                style.padding.left = 0;
                style.margin.right = 0;
                style.padding.right = 0;
                style.start_x = parent_style.start_x;
                style.end_x = parent_style.end_x;
                width = parent_style.width;
            }
        }

        style.width = width;

        if props.get("page-break-before").map(String::as_str) == Some("always") {
            display_list.push(Vec::new());
            position.y = root_data.rect.min.y;
        }

        position.y += style.padding.top;

        let last_y = position.y;

        let has_blocks = node.children().and_then(|children| {
            children.iter().skip_while(|child| child.is_whitespace())
                    .next().map(|child| child.is_block())
        });

        if has_blocks == Some(true) || has_blocks == None {
            if node.id().is_some() {
                display_list.last_mut().unwrap()
                            .push(DrawCommand::Marker(root_data.start_offset + node.offset()));
            }
            if let Some(children) = node.children() {
                let mut loop_context = LoopContext::default();
                loop_context.is_first = true;
                loop_context.parent = Some(node);
                let mut iter = children.iter().filter(|child| child.is_element()).peekable();

                while let Some(child) = iter.next() {
                    if iter.peek().is_none() {
                        loop_context.is_last = true;
                    }
                    let artifact = self.build_display_list_rec(child, &style, &loop_context, stylesheet, root_data, position, display_list);
                    loop_context.sibling = Some(&child);
                    loop_context.sibling_style = artifact.sibling_style;
                    loop_context.is_first = false;
                    // Collapse the bottom margin of the last child and its parent.
                    if loop_context.is_last {
                        style.margin.bottom = collapse_margins(loop_context.sibling_style.margin_bottom, style.margin.bottom);
                    }
                    // TODO: Merge artifact.rects in rects: [(i, r1), (i, r2)] becomes
                    // [(i, r1.merge(r2).grow(style.padding)]
                }
            }
        } else {
            let mut inlines = Vec::new();
            let mut markers = Vec::new();
            if node.id().is_some() {
                markers.push(node.offset());
            }
            let mut sibling = None;
            if let Some(children) = node.children() {
                for child in children {
                    self.gather_inline_material(child, Some(node), sibling, stylesheet, &style, &root_data.spine_dir, &mut markers, &mut inlines);
                    sibling = Some(child);
                }
            }
            if !inlines.is_empty() {
                self.place_paragraphs(&inlines, &style, root_data, &markers, position, &mut rects, display_list);
            }
        }

        // FIXME: Properly handle the height property:
        // let height: i32 = rects.iter().map(|(_, r)| r.height()).sum::<u32>() as i32;
        // if style.height > height {
        //     position.y += style.height - height;
        // }
        position.y += style.height;

        // Collapse top and bottom margins of empty blocks.
        // FIXME: The correct test is rects.is_empty(), but we
        // need to fill the rects vector in the general case
        // in order to be able to use the proper condition.
        if position.y == last_y {
            style.margin.bottom = collapse_margins(style.margin.bottom, style.margin.top);
            style.margin.top = 0;
        }

        position.y += style.padding.bottom;

        if props.get("page-break-after").map(String::as_str) == Some("always") {
            display_list.push(Vec::new());
            position.y = root_data.rect.min.y;
        }

        ChildArtifact {
            sibling_style: SiblingStyle {
                padding_bottom: style.padding.bottom,
                margin_bottom: style.margin.bottom,
            },
            rects,
        }
    }

    fn gather_inline_material(&self, node: &Node, parent: Option<&Node>, sibling: Option<&Node>, stylesheet: &Stylesheet, parent_style: &StyleData, spine_dir: &PathBuf, markers: &mut Vec<usize>, inlines: &mut Vec<InlineMaterial>) {
        match node {
            Node::Element(ElementData { offset, name, attributes, children }) => {
                let mut style = StyleData::default();
                let props = specified_values(node, parent, sibling, stylesheet);

                style.font_style = parent_style.font_style;
                style.line_height = parent_style.line_height;
                style.text_indent = parent_style.text_indent;
                style.retain_whitespace = parent_style.retain_whitespace;
                style.language = parent_style.language.clone();
                style.uri = parent_style.uri.clone();

                style.display = props.get("display").and_then(|value| parse_display(value))
                                     .unwrap_or(Display::Inline);

                if style.display == Display::None {
                    return;
                }

                style.font_size = props.get("font-size")
                                       .and_then(|value| parse_font_size(value, parent_style.font_size, self.font_size))
                                       .unwrap_or(parent_style.font_size);

                style.width = props.get("width")
                                   .and_then(|value| parse_width(value, style.font_size, self.font_size, parent_style.width, self.dpi))
                                   .unwrap_or(0);

                style.height = props.get("height")
                                    .and_then(|value| parse_height(value, style.font_size, self.font_size, parent_style.width, self.dpi))
                                    .unwrap_or(0);

                style.font_kind = props.get("font-family")
                                       .and_then(|value| parse_font_kind(value))
                                       .unwrap_or(parent_style.font_kind);

                style.color = props.get("color")
                                   .and_then(|value| parse_color(value))
                                   .unwrap_or(parent_style.color);

                style.letter_spacing = props.get("letter-spacing")
                                            .and_then(|value| parse_letter_spacing(value, style.font_size, self.font_size, self.dpi))
                                            .unwrap_or(parent_style.letter_spacing);

                style.vertical_align = props.get("vertical-align")
                                            .and_then(|value| parse_vertical_align(value, style.font_size, self.font_size, self.dpi))
                                            .unwrap_or(parent_style.vertical_align);

                style.font_style = props.get("font-style")
                                        .and_then(|value| parse_font_style(value))
                                        .unwrap_or(parent_style.font_style);

                style.font_weight = props.get("font-weight")
                                        .and_then(|value| parse_font_weight(value))
                                        .unwrap_or(parent_style.font_weight);

                style.font_features = props.get("font-feature-settings")
                                           .map(|value| parse_font_features(value))
                                           .or_else(|| parent_style.font_features.clone());


                if let Some(value) = props.get("font-variant") {
                    let mut features = parse_font_variant(value);
                    if let Some(v) = style.font_features.as_mut() {
                        v.append(&mut features);
                    }
                }

                if node.id().is_some() {
                    markers.push(node.offset());
                }

                match name.as_ref() {
                    "img" | "image" | "svg:image" => {
                        let attr = if name == "img" { "src" } else { "xlink:href" };

                        let path = attributes.get(attr).and_then(|src| {
                            spine_dir.join(src).normalize().to_str().map(String::from)
                        }).unwrap_or_default();

                        let is_block = style.display == Display::Block;
                        if is_block {
                            style.margin = parse_edge(props.get("margin-top").map(String::as_str),
                                                      props.get("margin-right").map(String::as_str),
                                                      props.get("margin-bottom").map(String::as_str),
                                                      props.get("margin-left").map(String::as_str),
                                                      style.font_size, self.font_size, parent_style.width, self.dpi);
                            style.padding = parse_edge(props.get("padding-top").map(String::as_str),
                                                       props.get("padding-right").map(String::as_str),
                                                       props.get("padding-bottom").map(String::as_str),
                                                       props.get("padding-left").map(String::as_str),
                                                       style.font_size, self.font_size, parent_style.width, self.dpi);
                            inlines.push(InlineMaterial::LineBreak);
                        }
                        inlines.push(InlineMaterial::Image(ImageMaterial {
                            offset: *offset,
                            path,
                            style,
                        }));
                        if is_block {
                            inlines.push(InlineMaterial::LineBreak);
                        }
                        return;
                    },
                    "a" => {
                        style.uri = attributes.get("href").cloned();
                    },
                    "br" => {
                        inlines.push(InlineMaterial::LineBreak);
                        return;
                    },
                    _ => {},
                }

                if let Some(mut v) = props.get("-plato-insert-before")
                                          .map(|value| parse_inline_material(value, style.font_size, self.font_size, self.dpi)) {
                    inlines.append(&mut v);
                }

                let mut sibling = None;
                for child in children {
                    self.gather_inline_material(child, Some(node), sibling, stylesheet, &style, spine_dir, markers, inlines);
                    sibling = Some(child);
                }

                if let Some(mut v) = props.get("-plato-insert-after")
                                          .map(|value| parse_inline_material(value, style.font_size, self.font_size, self.dpi)) {
                    inlines.append(&mut v);
                }
            },
            Node::Text(TextData { offset, text }) => {
                let mut index = 0;
                while let Some(start_delta) = text[index..].find('&') {
                    if start_delta > 0 {
                        inlines.push(InlineMaterial::Text(TextMaterial {
                            offset: *offset + index,
                            text: text[index..index+start_delta].to_string(),
                            style: parent_style.clone(),
                        }));
                    }
                    index += start_delta;
                    if let Some(end_delta) = text[index..].find(';') {
                        inlines.push(InlineMaterial::Text(TextMaterial {
                            offset: *offset + index,
                            text: decode_entities(&text[index..=index+end_delta]).into_owned(),
                            style: parent_style.clone(),
                        }));
                        index += end_delta + 1;
                    } else {
                        break;
                    }
                }
                if index < text.len() {
                    inlines.push(InlineMaterial::Text(TextMaterial {
                        offset: *offset + index,
                        text: text[index..].to_string(),
                        style: parent_style.clone(),
                    }));
                }
                return;
            },
            Node::Whitespace(TextData { offset, text }) => {
                inlines.push(InlineMaterial::Text(TextMaterial {
                    offset: *offset,
                    text: text.to_string(),
                    style: parent_style.clone(),
                }));
            },
        }
    }

    fn make_paragraph_items(&mut self, inlines: &[InlineMaterial], parent_style: &StyleData, line_width: i32) -> Vec<ParagraphItem<ParagraphElement>> {
        let mut items = Vec::new();
        let font_size = (parent_style.font_size * 64.0) as u32;
        let space_plan = {
            let font = self.fonts.as_mut().unwrap()
                           .get_mut(parent_style.font_kind,
                                    parent_style.font_style,
                                    parent_style.font_weight);
            font.set_size(font_size, self.dpi);
            font.plan(" 0.", None, None)
        };

        let big_stretch = 3 * space_plan.glyph_advance(0);

        if parent_style.text_align == TextAlign::Center {
            items.push(ParagraphItem::Box { width: 0, data: ParagraphElement::Nothing });
            items.push(ParagraphItem::Glue { width: 0, stretch: big_stretch, shrink: 0 });
        }

        let mut last_c = None;

        for m in inlines.iter() {
            match m {
                InlineMaterial::Image(ImageMaterial { offset, path, style }) => {
                    last_c = None;
                    let (mut width, mut height) = (style.width, style.height);
                    let mut scale = 1.0;
                    let dpi = self.dpi;

                    if let Ok(mut zf) = self.archive.by_name(path) {
                        let mut buf = Vec::new();

                        if zf.read_to_end(&mut buf).is_ok() {
                            if let Some(doc) = PdfOpener::new().and_then(|opener| opener.open_memory(path, &buf)) {
                                if let Some((w, h)) = doc.dims(0) {
                                    if width == 0 && height == 0 {
                                        width = pt_to_px(w, dpi).round() as i32;
                                        height = pt_to_px(h, dpi).round() as i32;
                                    } else if width != 0 {
                                        height = (width as f32 * h / w).round() as i32;
                                    } else if height != 0 {
                                        width = (height as f32 * w / h).round() as i32;
                                    }
                                    scale = width as f32 / w;
                                }
                            }
                        }

                        if width * height > 0 {
                            let edge = Edge {
                                top: style.padding.top,
                                right: style.margin.right,
                                bottom: style.padding.bottom,
                                left: style.margin.left,
                            };
                            items.push(ParagraphItem::Box {
                                width,
                                data: ParagraphElement::Image(ImageElement {
                                    offset: *offset,
                                    width,
                                    height,
                                    scale,
                                    vertical_align: style.vertical_align,
                                    display: style.display,
                                    edge,
                                    path: path.clone(),
                                    uri: style.uri.clone(),
                                }),
                            });
                        }
                    }
                },
                InlineMaterial::Text(TextMaterial { offset, text, style }) => {
                    let mut buf = String::new();
                    let font_size = (style.font_size * 64.0) as u32;

                    for (i, c) in text.char_indices() {
                        if c.is_whitespace() {
                            if !buf.is_empty() {
                                let local_offset = offset + i - buf.len() + 1;
                                let mut plan = {
                                    let font = self.fonts.as_mut().unwrap()
                                                   .get_mut(style.font_kind,
                                                            style.font_style,
                                                            style.font_weight);
                                    font.set_size(font_size, self.dpi);
                                    font.plan(&buf, None, style.font_features.as_ref().map(Vec::as_slice))
                                };
                                plan.space_out(style.letter_spacing.max(0) as u32);

                                items.push(ParagraphItem::Box {
                                    width: plan.width as i32,
                                    data: ParagraphElement::Text(TextElement {
                                        offset: local_offset,
                                        language: style.language.clone(),
                                        text: buf,
                                        plan,
                                        font_features: style.font_features.clone(),
                                        font_kind: style.font_kind,
                                        font_style: style.font_style,
                                        font_weight: style.font_weight,
                                        vertical_align: style.vertical_align,
                                        letter_spacing: style.letter_spacing,
                                        font_size,
                                        color: style.color,
                                        uri: style.uri.clone(),
                                    }),
                                });

                                buf = String::new();
                            }

                            if c == '\n' && parent_style.retain_whitespace {
                                let stretch = if parent_style.text_align == TextAlign::Center { big_stretch } else { line_width };

                                items.push(ParagraphItem::Penalty { penalty: INFINITE_PENALTY, width: 0, flagged: false });
                                items.push(ParagraphItem::Glue { width: 0, stretch, shrink: 0 });

                                items.push(ParagraphItem::Penalty { width: 0, penalty: -INFINITE_PENALTY, flagged: false });

                                if parent_style.text_align == TextAlign::Center {
                                    items.push(ParagraphItem::Box { width: 0, data: ParagraphElement::Nothing });
                                    items.push(ParagraphItem::Penalty { width: 0, penalty: INFINITE_PENALTY, flagged: false });
                                    items.push(ParagraphItem::Glue { width: 0, stretch: big_stretch, shrink: 0 });
                                }
                                last_c = Some(c);
                                continue;
                            }

                            if !parent_style.retain_whitespace && (c == ' ' || c.is_control()) &&
                               (last_c.map(|c| c == ' ' || c.is_control()) == Some(true)) {
                                   last_c = Some(c);
                                   continue;
                            }

                            let mut width = if let Some(index) = FONT_SPACES.chars().position(|x| x == c) {
                                space_plan.glyph_advance(index)
                            } else if let Some(ratio) = WORD_SPACE_RATIOS.get(&c) {
                                (space_plan.glyph_advance(0) as f32 * ratio) as i32
                            } else if let Some(ratio) = EM_SPACE_RATIOS.get(&c) {
                                pt_to_px(style.font_size * ratio, self.dpi).round() as i32
                            } else {
                                space_plan.glyph_advance(0)
                            };

                            width += 2 * style.letter_spacing;

                            let (stretch, shrink) = if style.font_kind != FontKind::Monospace {
                                (width / 2, width / 3)
                            } else {
                                (0, 0)
                            };

                            if parent_style.retain_whitespace && last_c == Some('\n') {
                                items.push(ParagraphItem::Box { width: 0, data: ParagraphElement::Nothing });
                            }

                            let is_unbreakable = c == '\u{00A0}' || c == '\u{202F}';

                            if is_unbreakable {
                                items.push(ParagraphItem::Penalty { width: 0, penalty: INFINITE_PENALTY, flagged: false });
                            }

                            match parent_style.text_align {
                                TextAlign::Justify => {
                                    items.push(ParagraphItem::Glue { width, stretch, shrink });
                                },
                                TextAlign::Center => {
                                    if is_unbreakable {
                                        items.push(ParagraphItem::Glue { width, stretch: 0, shrink: 0 });
                                    } else {
                                        let stretch = 3 * width;
                                        items.push(ParagraphItem::Glue { width: 0, stretch, shrink: 0 });
                                        items.push(ParagraphItem::Penalty { width: 0, penalty: 0, flagged: false });
                                        items.push(ParagraphItem::Glue { width, stretch: -2 * stretch, shrink: 0 });
                                        items.push(ParagraphItem::Box { width: 0, data: ParagraphElement::Nothing });
                                        items.push(ParagraphItem::Penalty { width: 0, penalty: INFINITE_PENALTY, flagged: false });
                                        items.push(ParagraphItem::Glue { width: 0, stretch, shrink: 0 });
                                    }
                                },
                                TextAlign::Left | TextAlign::Right => {
                                    if is_unbreakable {
                                        items.push(ParagraphItem::Glue { width, stretch: 0, shrink: 0 });
                                    } else {
                                        let stretch = 3 * width;
                                        items.push(ParagraphItem::Glue { width: 0, stretch, shrink: 0 });
                                        items.push(ParagraphItem::Penalty { width: 0, penalty: 0, flagged: false });
                                        items.push(ParagraphItem::Glue { width, stretch: -stretch, shrink: 0 });
                                    }
                                },
                            }

                        } else {
                            buf.push(c);
                        }

                        last_c = Some(c);
                    }

                    // TODO: Find a way to integrate this into the main loop?
                    if !buf.is_empty() {
                        let local_offset = offset + text.char_indices().last().map(|(i, _)| i).unwrap_or(text.len() - 1) - buf.len() + 1;
                        let font_size = (style.font_size * 64.0) as u32;
                        let mut plan = {
                            let font = self.fonts.as_mut().unwrap()
                                           .get_mut(style.font_kind,
                                                    style.font_style,
                                                    style.font_weight);
                            font.set_size(font_size, self.dpi);
                            font.plan(&buf, None, style.font_features.as_ref().map(Vec::as_slice))
                        };
                        plan.space_out(style.letter_spacing.max(0) as u32);
                        items.push(ParagraphItem::Box {
                            width: plan.width as i32,
                            data: ParagraphElement::Text(TextElement {
                                offset: local_offset,
                                language: style.language.clone(),
                                text: buf,
                                plan,
                                font_features: style.font_features.clone(),
                                font_kind: style.font_kind,
                                font_style: style.font_style,
                                font_weight: style.font_weight,
                                vertical_align: style.vertical_align,
                                letter_spacing: style.letter_spacing,
                                font_size,
                                color: style.color,
                                uri: style.uri.clone(),
                            }),
                        });
                        buf = String::new();
                    }
                },
                InlineMaterial::LineBreak => {
                    last_c = None;

                    let stretch = if parent_style.text_align == TextAlign::Center { big_stretch } else { line_width };

                    items.push(ParagraphItem::Penalty { penalty: INFINITE_PENALTY, width: 0, flagged: false });
                    items.push(ParagraphItem::Glue { width: 0, stretch, shrink: 0 });

                    items.push(ParagraphItem::Penalty { width: 0, penalty: -INFINITE_PENALTY, flagged: false });

                    if parent_style.text_align == TextAlign::Center {
                        items.push(ParagraphItem::Box { width: 0, data: ParagraphElement::Nothing });
                        items.push(ParagraphItem::Penalty { width: 0, penalty: INFINITE_PENALTY, flagged: false });
                        items.push(ParagraphItem::Glue { width: 0, stretch: big_stretch, shrink: 0 });
                    }
                },
                InlineMaterial::Glue(GlueMaterial { width, stretch, shrink }) => {
                    items.push(ParagraphItem::Glue { width: *width, stretch: *stretch, shrink: *shrink });
                },
                InlineMaterial::Penalty(PenaltyMaterial { width, penalty, flagged }) => {
                    items.push(ParagraphItem::Penalty { width: *width, penalty: *penalty, flagged: *flagged });
                },
                InlineMaterial::Box(width) => {
                    items.push(ParagraphItem::Box { width: *width, data: ParagraphElement::Nothing });
                },
            }
        }

        if items.last().map(|x| x.penalty()) != Some(-INFINITE_PENALTY) {
            items.push(ParagraphItem::Penalty { penalty: INFINITE_PENALTY,  width: 0, flagged: false });

            let stretch = if parent_style.text_align == TextAlign::Center { big_stretch } else { line_width };
            items.push(ParagraphItem::Glue { width: 0, stretch, shrink: 0 });

            items.push(ParagraphItem::Penalty { penalty: -INFINITE_PENALTY, width: 0, flagged: true });
        }

        items
    }

    fn place_paragraphs(&mut self, inlines: &[InlineMaterial], style: &StyleData, root_data: &RootData, markers: &Vec<usize>, position: &mut Point, rects: &mut Vec<(usize, Rectangle)>, display_list: &mut Vec<Page>) {
        let text_indent = if style.text_align == TextAlign::Center {
            0
        } else {
            style.text_indent
        };

        let stretch_tolerance = if style.text_align == TextAlign::Justify {
            STRETCH_TOLERANCE
        } else {
            10.0
        };
        let (ascender, descender) = {
            let fonts = self.fonts.as_mut().unwrap();
            let font = fonts.get_mut(style.font_kind, style.font_style, style.font_weight);
            font.set_size((style.font_size * 64.0) as u32, self.dpi);
            (font.ascender(), font.descender())
        };

        let ratio = ascender as f32 / (ascender - descender) as f32;
        let space_top = (style.line_height as f32 * ratio) as i32;
        let space_bottom = style.line_height - space_top;

        let mut start_y = position.y + style.margin.top - style.padding.top;
        position.y += style.margin.top + space_top;

        let line_width = style.end_x - style.start_x;
        let line_lengths = [line_width - text_indent, line_width];

        let mut page = display_list.pop().unwrap();
        let mut items = self.make_paragraph_items(inlines, style, line_width);

        let mut bps = total_fit(&items, &line_lengths, stretch_tolerance, 0);

        let mut hyph_indices = Vec::new();
        let mut glue_drifts = Vec::new();

        if bps.is_empty() {
            let dictionary = if style.text_align == TextAlign::Justify {
                hyph_lang(style.language.as_ref().map_or(DEFAULT_HYPH_LANG, String::as_str))
                         .and_then(|lang| HYPHENATION_PATTERNS.get(&lang))
            } else {
                None
            };

            // Insert optional breaks.
            items = self.insert_breaks(dictionary, items, &mut hyph_indices);
            bps = total_fit(&items, &line_lengths, stretch_tolerance, 0);
        }

        if bps.is_empty() {
            bps = standard_fit(&items, &line_lengths, stretch_tolerance);
        }

        if bps.is_empty() {
            let max_width = line_lengths[0].min(line_lengths[1]);

            for itm in &mut items {
                if let ParagraphItem::Box { width, data } = itm {
                    if *width > max_width {
                        match data {
                            ParagraphElement::Text(TextElement { plan, font_kind, font_style, font_weight, font_size, .. }) => {
                                let font = self.fonts.as_mut().unwrap()
                                               .get_mut(*font_kind, *font_style, *font_weight);
                                font.set_size(*font_size, self.dpi);
                                font.crop_right(plan, max_width as u32);
                                *width = plan.width as i32;
                            },
                            ParagraphElement::Image(ImageElement { width: image_width, height, scale, .. }) => {
                                let ratio = max_width as f32 / *image_width as f32;
                                *scale *= ratio;
                                *image_width = max_width;
                                *height = (*height as f32 * ratio) as i32;
                                *width = max_width;
                            },
                            _ => (),
                        }
                    }
                }
            }

            bps = standard_fit(&items, &line_lengths, STRETCH_TOLERANCE);
        }

        // Remove unselected optional hyphens (prevents broken ligatures).
        if !bps.is_empty() && !hyph_indices.is_empty() {
            items = self.cleanup_paragraph(items, &hyph_indices, &mut glue_drifts, &mut bps);
        }

        let mut last_index = 0;
        let mut markers_index = 0;
        let mut last_text_offset = 0;
        let mut last_x_position = 0;
        let mut is_first_line = true;
        let mut j = 0;

        for bp in bps {
            if position.y > root_data.rect.max.y - space_bottom {
                if !is_first_line {
                    let end_y = position.y - style.line_height + space_bottom;
                    rects.push((display_list.len(),
                                rect![style.start_x - style.padding.left, start_y,
                                      style.end_x + style.padding.right, end_y]));
                }
                display_list.push(page);
                start_y = root_data.rect.min.y;
                position.y = root_data.rect.min.y + space_top;
                page = Vec::new();
            }

            let drift = if glue_drifts.is_empty() {
                0.0
            } else {
                glue_drifts[j]
            };

            let Breakpoint { index, width, mut ratio } = bp;
            let mut epsilon: f32 = 0.0;
            let current_text_indent = if is_first_line { text_indent } else { 0 };

            match style.text_align {
                TextAlign::Right => position.x = style.end_x - width - current_text_indent,
                _ => position.x = style.start_x + current_text_indent,
            }

            if style.text_align == TextAlign::Left || style.text_align == TextAlign::Right {
                ratio = ratio.min(0.0);
            }

            while last_index < index && !items[last_index].is_box()  {
                last_index += 1;
            }

            for i in last_index..index {
                match items[i] {
                    ParagraphItem::Box { ref data, width } => {
                        match data {
                            ParagraphElement::Text(element) => {
                                let pt = pt!(position.x, position.y - element.vertical_align);
                                let rect = rect![pt + pt!(0, -ascender), pt + pt!(element.plan.width as i32, -descender)];
                                last_text_offset = element.offset;
                                while let Some(offset) = markers.get(markers_index) {
                                    if *offset < element.offset {
                                        page.push(DrawCommand::Marker(root_data.start_offset + *offset));
                                        markers_index += 1;
                                    } else {
                                        break;
                                    }
                                }
                                page.push(DrawCommand::Text(TextCommand {
                                    offset: element.offset + root_data.start_offset,
                                    position: pt,
                                    rect,
                                    text: element.text.clone(),
                                    plan: element.plan.clone(),
                                    uri: element.uri.clone(),
                                    font_kind: element.font_kind,
                                    font_style: element.font_style,
                                    font_weight: element.font_weight,
                                    font_size: element.font_size,
                                    color: element.color,
                                }));
                            },
                            ParagraphElement::Image(element) => {
                                while let Some(offset) = markers.get(markers_index) {
                                    if *offset < element.offset {
                                        page.push(DrawCommand::Marker(root_data.start_offset + *offset));
                                        markers_index += 1;
                                    } else {
                                        break;
                                    }
                                }
                                let mut k = last_index;
                                while k < index {
                                    match items[k] {
                                        ParagraphItem::Box { width, .. } if width > 0 && k != i => break,
                                        _ => k += 1,
                                    }
                                }
                                // The image is the only consistent box on this line.
                                let (w, h, pt, scale) = if k == index {
                                    position.y += element.edge.top;
                                    if element.display == Display::Block {
                                        position.y -= space_top;
                                    }
                                    let (mut width, mut height) = (element.width, element.height);
                                    let r = width as f32 / height as f32;
                                    if position.y + height > root_data.rect.max.y - space_bottom {
                                        let mut ratio = (root_data.rect.max.y - position.y - space_bottom) as f32 / height as f32;
                                        if ratio < 0.33 {
                                            display_list.push(page);
                                            position.y = root_data.rect.min.y;
                                            page = Vec::new();
                                            ratio = ((root_data.rect.max.y - position.y - space_bottom) as f32 / height as f32).min(1.0);
                                        }
                                        height = (height as f32 * ratio).round() as i32;
                                        width = (height as f32 * r).round() as i32;
                                    }
                                    let scale = element.scale * width as f32 / element.width as f32;
                                    if element.display == Display::Block {
                                        let mut left_edge = element.edge.left;
                                        let total_width = left_edge + width + element.edge.right;
                                        if total_width > line_width {
                                            let remaining_space = line_width - width;
                                            let ratio = left_edge as f32 / (left_edge + element.edge.right) as f32;
                                            left_edge = (ratio * remaining_space as f32).round() as i32;
                                        }
                                        position.x = style.start_x + left_edge;
                                        if last_x_position < position.x && position.y > root_data.rect.min.y {
                                            position.y -= style.line_height;
                                        }
                                    } else if width < element.width {
                                        if style.text_align == TextAlign::Center {
                                            position.x += (element.width - width) / 2;
                                        } else if style.text_align == TextAlign::Right {
                                            position.x += element.width - width;
                                        }
                                    }
                                    let pt = pt!(position.x, position.y);
                                    position.y += height + element.edge.bottom;
                                    if element.display == Display::Block {
                                        position.y -= space_bottom;
                                    }
                                    (width, height, pt, scale)
                                } else {
                                    let pt = pt!(position.x, position.y - element.height - element.vertical_align);
                                    (element.width, element.height, pt, element.scale)
                                };


                                let rect = rect![pt, pt + pt!(w, h)];
                                rects.push((display_list.len(),
                                            rect![style.start_x - style.padding.left, rect.min.y,
                                                  style.end_x + style.padding.right, rect.max.y]));
                                page.push(DrawCommand::Image(ImageCommand {
                                    offset: element.offset + root_data.start_offset,
                                    position: pt,
                                    rect,
                                    scale,
                                    path: element.path.clone(),
                                    uri: element.uri.clone(),
                                }));
                            },
                            _ => (),
                        }

                        position.x += width;
                        last_x_position = position.x;
                    },
                    ParagraphItem::Glue { width, stretch, shrink } => {
                        let amplitude = if ratio.is_sign_positive() { stretch } else { shrink };
                        let exact_width = width as f32 + ratio * amplitude as f32 + drift;
                        let approx_width = if epsilon.is_sign_positive() {
                            exact_width.floor() as i32
                        } else {
                            exact_width.ceil() as i32
                        };
                        epsilon += approx_width as f32 - exact_width;
                        position.x += approx_width;
                    },
                    _ => (),
                }
            }

            if let ParagraphItem::Penalty { width, .. } = items[index] {
                if width > 0 {
                    let font_size = (style.font_size * 64.0) as u32;
                    let plan = {
                        let font = self.fonts.as_mut().unwrap()
                                       .get_mut(style.font_kind, style.font_style, style.font_weight);
                        font.set_size(font_size, self.dpi);
                        font.plan("-", None, style.font_features.as_ref().map(Vec::as_slice))
                    };
                    let rect = rect![*position + pt!(0, -ascender), *position + pt!(plan.width as i32, -descender)];
                    page.push(DrawCommand::Text(TextCommand {
                        offset: last_text_offset + root_data.start_offset,
                        position: *position,
                        rect,
                        text: '\u{00AD}'.to_string(),
                        plan,
                        uri: None,
                        font_kind: style.font_kind,
                        font_style: style.font_style,
                        font_weight: style.font_weight,
                        font_size,
                        color: style.color,
                    }));
                }
            }

            last_index = index;
            is_first_line = false;

            if index < items.len() - 1 {
                position.y += style.line_height;
            }

            j += 1;
        }

        while let Some(offset) = markers.get(markers_index) {
            page.push(DrawCommand::Marker(root_data.start_offset + *offset));
            markers_index += 1;
        }

        if !is_first_line {
            let end_y = position.y + space_bottom + style.padding.bottom;
            rects.push((display_list.len(),
                        rect![style.start_x - style.padding.left, start_y,
                              style.end_x + style.padding.right, end_y]));
        }

        position.y += space_bottom;

        display_list.push(page);
    }

    #[inline]
    fn box_from_chunk(&mut self, chunk: &str, index: usize, element: &TextElement) -> ParagraphItem<ParagraphElement> {
        let offset = element.offset + index;
        let mut plan = {
            let font = self.fonts.as_mut().unwrap()
                           .get_mut(element.font_kind,
                                    element.font_style,
                                    element.font_weight);
            font.set_size(element.font_size, self.dpi);
            font.plan(chunk, None, element.font_features.as_ref().map(Vec::as_slice))
        };
        plan.space_out(element.letter_spacing.max(0) as u32);
        ParagraphItem::Box {
            width: plan.width as i32,
            data: ParagraphElement::Text(TextElement {
                offset,
                text: chunk.to_string(),
                plan,
                language: element.language.clone(),
                font_features: element.font_features.clone(),
                font_kind: element.font_kind,
                font_style: element.font_style,
                font_weight: element.font_weight,
                font_size: element.font_size,
                vertical_align: element.vertical_align,
                letter_spacing: element.letter_spacing,
                color: element.color,
                uri: element.uri.clone(),
            }),
        }
    }

    fn insert_breaks(&mut self, dictionary: Option<&Standard>, items: Vec<ParagraphItem<ParagraphElement>>, hyph_indices: &mut Vec<[usize; 2]>) -> Vec<ParagraphItem<ParagraphElement>> {
        let mut hyph_items = Vec::with_capacity(items.len());

        for itm in items {
            match itm {
                ParagraphItem::Box { data: ParagraphElement::Text(ref element), .. } => {
                    let text = &element.text;
                    let mut start_index = 0;
                    let hyphen_width = if dictionary.is_some() {
                        let font = self.fonts.as_mut().unwrap()
                                       .get_mut(element.font_kind, element.font_style, element.font_weight);
                        font.set_size(element.font_size, self.dpi);
                        font.plan("-", None, element.font_features.as_ref().map(Vec::as_slice)).width as i32
                    } else {
                        0
                    };
                    for (end_index, is_hardbreak) in LineBreakIterator::new(text) {
                        let chunk = &text[start_index..end_index];
                        // Hyphenate.
                        if let Some(dict) = dictionary {
                            let mut index_before = chunk.find(|c: char| c.is_alphabetic()).unwrap_or_else(|| chunk.len());
                            if index_before > 0 {
                                    let subelem = self.box_from_chunk(&chunk[0..index_before],
                                                                      start_index,
                                                                      &element);
                                    hyph_items.push(subelem);

                            }

                            let mut index_after = chunk[index_before..].find(|c: char| !c.is_alphabetic())
                                                                       .map(|i| index_before + i)
                                                                       .unwrap_or_else(|| chunk.len());
                            while index_before < index_after {
                                let mut index = 0;
                                let subchunk = &chunk[index_before..index_after];
                                let len_before = hyph_items.len();
                                for segment in dict.hyphenate(subchunk).iter().segments() {

                                    let subelem = self.box_from_chunk(segment,
                                                                      start_index + index_before + index,
                                                                      &element);
                                    hyph_items.push(subelem);
                                    index += segment.len();
                                    if index < subchunk.len() {
                                        hyph_items.push(ParagraphItem::Penalty { width: hyphen_width, penalty: HYPHEN_PENALTY, flagged: true });
                                    }
                                }
                                let len_after = hyph_items.len();
                                if len_after > 1 + len_before {
                                    hyph_indices.push([len_before, len_after]);
                                }
                                index_before = chunk[index_after..].find(|c: char| c.is_alphabetic())
                                                                   .map(|i| index_after + i)
                                                                   .unwrap_or(chunk.len());
                                if index_before > index_after {
                                    let subelem = self.box_from_chunk(&chunk[index_after..index_before],
                                                                      start_index + index_after,
                                                                      &element);
                                    hyph_items.push(subelem);
                                }

                                index_after = chunk[index_before..].find(|c: char| !c.is_alphabetic())
                                                                   .map(|i| index_before + i)
                                                                   .unwrap_or(chunk.len());
                            }
                        } else {
                            let subelem = self.box_from_chunk(chunk, start_index, &element);
                            hyph_items.push(subelem);
                        }
                        if !is_hardbreak {
                            let penalty = if chunk.ends_with('-') { HYPHEN_PENALTY } else { 0 };
                            let flagged = penalty > 0;
                            hyph_items.push(ParagraphItem::Penalty { width: 0, penalty, flagged });
                        }
                        start_index = end_index;
                    }

                },
                _ => { hyph_items.push(itm) },
            }
        }

        hyph_items
    }

    fn cleanup_paragraph(&mut self, items: Vec<ParagraphItem<ParagraphElement>>, hyph_indices: &[[usize; 2]], glue_drifts: &mut Vec<f32>, bps: &mut Vec<Breakpoint>) -> Vec<ParagraphItem<ParagraphElement>> {
        let mut merged_items = Vec::with_capacity(items.len());
        let mut j = 0;
        let mut k = 0;
        let mut index_drift = 0;
        let [mut start_index, mut end_index] = hyph_indices[j];
        let mut bp = bps[k];
        let mut line_stats = LineStats::default();
        let mut merged_element = ParagraphElement::Nothing;

        for (i, itm) in items.into_iter().enumerate() {
            if i == bp.index {
                let mut merged_width = 0;

                if let ParagraphElement::Text(TextElement { ref text, ref mut plan, font_size, font_kind,
                                                            font_style, font_weight, letter_spacing, ref font_features, .. }) = merged_element {
                    *plan = {
                        let font = self.fonts.as_mut().unwrap()
                                       .get_mut(font_kind, font_style, font_weight);
                        font.set_size(font_size, self.dpi);
                        font.plan(text, None, font_features.as_ref().map(Vec::as_slice))
                    };
                    plan.space_out(letter_spacing.max(0) as u32);
                    merged_width = plan.width as i32;
                }

                if merged_width > 0 {
                    merged_items.push(ParagraphItem::Box { width: merged_width, data: merged_element });
                    merged_element = ParagraphElement::Nothing;
                }

                line_stats.merged_width += merged_width;
                let delta_width = line_stats.merged_width - line_stats.width;
                glue_drifts.push(-delta_width as f32 / line_stats.glues_count as f32);

                bps[k].index = bps[k].index.saturating_sub(index_drift);
                bps[k].width += delta_width;
                k += 1;

                if k < bps.len() {
                    bp = bps[k];
                }

                line_stats = LineStats::default();
                merged_items.push(itm);
            } else if i >= start_index && i < end_index {
                if let ParagraphItem::Box { width, data } = itm {
                    match merged_element {
                        ParagraphElement::Text(TextElement { ref mut text, .. }) => {
                            if let ParagraphElement::Text(TextElement { text: other_text, .. }) = data {
                                text.push_str(&other_text);
                            }
                        },
                        ParagraphElement::Nothing => merged_element = data,
                        _ => (),
                    }
                    line_stats.width += width;
                    if !line_stats.started {
                        line_stats.started = true;
                    }
                } else {
                    index_drift += 2;
                }
                if i == end_index - 1 {
                    j += 1;
                    if let Some(&[s, e]) = hyph_indices.get(j) {
                        start_index = s;
                        end_index = e;
                    } else {
                        start_index = usize::max_value();
                        end_index = 0;
                    }
                    let mut merged_width = 0;
                    if let ParagraphElement::Text(TextElement { ref text, ref mut plan, font_size, font_kind,
                                                                font_style, font_weight, letter_spacing, ref font_features, .. }) = merged_element {
                        *plan = {
                            let font = self.fonts.as_mut().unwrap()
                                           .get_mut(font_kind, font_style, font_weight);
                            font.set_size(font_size, self.dpi);
                            font.plan(text, None, font_features.as_ref().map(Vec::as_slice))
                        };
                        plan.space_out(letter_spacing.max(0) as u32);
                        merged_width = plan.width as i32;
                    }
                    merged_items.push(ParagraphItem::Box { width: merged_width, data: merged_element });
                    merged_element = ParagraphElement::Nothing;
                    line_stats.merged_width += merged_width;
                }
            } else {
                match itm {
                    ParagraphItem::Glue { .. } if line_stats.started => line_stats.glues_count += 1,
                    ParagraphItem::Box { .. } if !line_stats.started => line_stats.started = true,
                    _ => (),
                }
                merged_items.push(itm);
            }
        }

        merged_items
    }

    fn render_page(&mut self, page: &[DrawCommand]) -> Pixmap {
        let (width, height) = self.dims;
        let mut fb = Pixmap::new(width, height);

        for dc in page {
            match dc {
                DrawCommand::Text(TextCommand { position, plan, font_kind, font_style, font_weight, font_size, color, .. }) => {
                    let font = self.fonts.as_mut().unwrap()
                                   .get_mut(*font_kind, *font_style, *font_weight);
                    font.set_size(*font_size, self.dpi);
                    font.render(&mut fb, *color, plan, *position);
                },
                DrawCommand::Image(ImageCommand { position, path, scale, .. }) => {
                    if let Ok(mut zf) = self.archive.by_name(path) {
                        let mut buf = Vec::new();
                        if zf.read_to_end(&mut buf).is_ok() {
                            PdfOpener::new().and_then(|opener| {
                                opener.open_memory(path, &buf)
                            }).and_then(|mut doc| {
                                doc.pixmap(Location::Exact(0), *scale)
                            }).map(|(pixmap, _)| {
                                fb.draw_pixmap(&pixmap, *position);
                            });
                        }
                    }
                },
                _ => (),
            }
        }

        fb
    }

    pub fn metadata_by_name(&self, name: &str) -> Option<String> {
        self.content.find("metadata")
            .and_then(|metadata| metadata.children())
            .and_then(|children| children.iter()
                                         .find(|child| child.tag_name() == Some("meta") &&
                                                       child.attr("name") == Some(name)))
            .and_then(|child| child.attr("content").map(|s| decode_entities(s).into_owned()))
    }

    pub fn categories(&self) -> BTreeSet<String> {
        let mut result = BTreeSet::new();
        self.content.find("metadata")
            .and_then(|metadata| metadata.children())
            .map(|children| {
                for child in children {
                    if child.tag_name() == Some("dc:subject") {
                        for subject in child.text().map(|text| decode_entities(text)) {
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
        Some((self.dims.0 as f32, self.dims.1 as f32))
    }

    fn pages_count(&self) -> usize {
        self.spine.iter().map(|c| c.size).sum()
    }

    fn toc(&mut self) -> Option<Vec<TocEntry>> {
        let name = self.content.find("spine").and_then(|spine| {
            spine.attr("toc")
        }).and_then(|toc_id| {
            self.content.find("manifest")
                .and_then(|manifest| manifest.find_by_id(toc_id))
                .and_then(|entry| entry.attr("href"))
        }).map(|href| {
            self.parent.join(href).normalize()
                .to_string_lossy().into_owned()
        })?;

        let toc_dir = Path::new(&name).parent()?;

        let mut text = String::new();
        if let Ok(mut zf) = self.archive.by_name(&name) {
            zf.read_to_string(&mut text).ok()?;
        } else {
            return None;
        }

        let root = XmlParser::new(&text).parse();
        root.find("navMap").map(|map| {
            let mut cache = FnvHashMap::default();
            self.walk_toc(&map, &toc_dir, &mut cache)
        })
    }

    fn resolve_location(&mut self, loc: Location) -> Option<usize> {
        if self.fonts.is_none() {
            self.fonts = default_fonts().ok();
        }

        match loc {
            Location::Exact(offset) => {
                let (index, start_offset) = self.vertebra_coordinates(offset);
                let page_index = self.page_index(offset, index, start_offset)?;
                self.cache.get(&index)
                    .and_then(|display_list| display_list[page_index].first())
                    .map(|dc| dc.offset())
            },
            Location::Previous(offset) => {
                let (index, start_offset) = self.vertebra_coordinates(offset);
                let page_index = self.page_index(offset, index, start_offset)?;
                if page_index > 0 {
                    self.cache.get(&index)
                        .and_then(|display_list| display_list[page_index-1].first().map(|dc| dc.offset()))
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
                        .and_then(|display_list| display_list.last().and_then(|page| page.first()).map(|dc| dc.offset()))
                }
            },
            Location::Next(offset) => {
                let (index, start_offset) = self.vertebra_coordinates(offset);
                let page_index = self.page_index(offset, index, start_offset)?;
                if page_index < self.cache.get(&index).map(|display_list| display_list.len())? - 1 {
                    self.cache.get(&index).and_then(|display_list| display_list[page_index+1].first().map(|dc| dc.offset()))
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
            Location::Uri(offset, uri) => {
                let mut cache = FnvHashMap::default();
                let normalized_uri: String = {
                    let (index, _) = self.vertebra_coordinates(offset);
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
        }
    }

    fn words(&mut self, loc: Location) -> Option<(Vec<BoundedText>, usize)> {
        if self.spine.is_empty() {
            return None;
        }

        let offset = self.resolve_location(loc)?;
        let (index, start_offset) = self.vertebra_coordinates(offset);
        let page_index = self.page_index(offset, index, start_offset)?;

        self.cache.get(&index).map(|display_list| {
            (display_list[page_index].iter().filter_map(|dc| {
                match dc {
                    DrawCommand::Text(TextCommand { text, rect, .. }) => {
                        Some(BoundedText {
                            text: text.clone(),
                            rect: (*rect).into(),
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
        let (index, start_offset) = self.vertebra_coordinates(offset);
        let page_index = self.page_index(offset, index, start_offset)?;

        self.cache.get(&index).map(|display_list| {
            (display_list[page_index].iter().filter_map(|dc| {
                match dc {
                    DrawCommand::Text(TextCommand { uri, rect, .. }) |
                    DrawCommand::Image(ImageCommand { uri, rect, .. }) if uri.is_some() => {
                        Some(BoundedText {
                            text: uri.clone().unwrap(),
                            rect: (*rect).into(),
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
        let (index, start_offset) = self.vertebra_coordinates(offset);

        let page_index = self.page_index(offset, index, start_offset)?;
        let page = self.cache.get(&index)?.get(page_index)?.clone();

        let pixmap = self.render_page(&page);

        Some((pixmap, offset))
    }

    fn layout(&mut self, width: u32, height: u32, font_size: f32, dpi: u16) {
        // TODO: Reject absurd values?
        self.dims = (width, height);
        self.dpi = dpi;
        self.font_size = font_size;
        self.cache.clear();
    }

    fn set_font_family(&mut self, family_name: &str, search_path: &str) {
        if let Ok(serif_family) = FontFamily::from_name(family_name, search_path) {
            if self.fonts.is_none() {
                self.fonts = default_fonts().ok();
            }
            if let Some(fonts) = self.fonts.as_mut() {
                fonts.serif = serif_family;
                self.cache.clear();
            }
        }
    }

    fn set_margin_width(&mut self, width: i32) {
        if width >= 0 && width <= 10 {
            self.margin = Edge::uniform(mm_to_px(width as f32, self.dpi).round() as i32);
            self.cache.clear();
        }
    }

    fn set_line_height(&mut self, line_height: f32) {
        self.line_height = line_height;
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
        self.content.find("metadata")
            .and_then(|metadata| metadata.children())
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

fn default_fonts() -> Result<Fonts, Error> {
    let opener = FontOpener::new()?;
    let mut fonts = Fonts {
        serif: FontFamily {
            regular: opener.open("fonts/LibertinusSerif-Regular.otf")?,
            italic: opener.open("fonts/LibertinusSerif-Italic.otf")?,
            bold: opener.open("fonts/LibertinusSerif-Bold.otf")?,
            bold_italic: opener.open("fonts/LibertinusSerif-BoldItalic.otf")?,
        },
        sans_serif: FontFamily {
            regular: opener.open("fonts/NotoSans-Regular.ttf")?,
            italic: opener.open("fonts/NotoSans-Italic.ttf")?,
            bold: opener.open("fonts/NotoSans-Bold.ttf")?,
            bold_italic: opener.open("fonts/NotoSans-BoldItalic.ttf")?,
        },
        monospace: FontFamily {
            regular: opener.open("fonts/SourceCodeVariable-Roman.otf")?,
            italic: opener.open("fonts/SourceCodeVariable-Italic.otf")?,
            bold: opener.open("fonts/SourceCodeVariable-Roman.otf")?,
            bold_italic: opener.open("fonts/SourceCodeVariable-Italic.otf")?,
        },
        cursive: opener.open("fonts/Parisienne-Regular.ttf")?,
        fantasy: opener.open("fonts/Delius-Regular.ttf")?,
    };
    fonts.monospace.bold.set_variations(&["wght=600"]);
    fonts.monospace.bold_italic.set_variations(&["wght=600"]);
    Ok(fonts)
}
