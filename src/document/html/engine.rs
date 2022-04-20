use std::path::PathBuf;
use std::convert::TryFrom;
use anyhow::Error;
use kl_hyphenate::{Standard, Hyphenator, Iter};
use paragraph_breaker::{Item as ParagraphItem, Breakpoint, INFINITE_PENALTY};
use paragraph_breaker::{total_fit, standard_fit};
use xi_unicode::LineBreakIterator;
use percent_encoding::percent_decode_str;
use septem::Roman;
use crate::helpers::{Normalize, decode_entities};
use crate::framebuffer::{Framebuffer, Pixmap};
use crate::font::{FontOpener, FontFamily};
use crate::document::{Document, Location};
use crate::document::pdf::PdfOpener;
use crate::unit::{mm_to_px, pt_to_px};
use crate::geom::{Point, Vec2, Rectangle, Edge};
use crate::settings::{HYPHEN_PENALTY, STRETCH_TOLERANCE};
use crate::settings::{DEFAULT_FONT_SIZE, DEFAULT_MARGIN_WIDTH, DEFAULT_TEXT_ALIGN, DEFAULT_LINE_HEIGHT};
use super::parse::{parse_display, parse_edge, parse_float, parse_text_align, parse_text_indent};
use super::parse::{parse_width, parse_height, parse_inline_material, parse_font_kind, parse_font_style};
use super::parse::{parse_font_weight, parse_font_size, parse_font_features, parse_font_variant};
use super::parse::{parse_letter_spacing, parse_word_spacing};
use super::parse::{parse_line_height, parse_vertical_align, parse_color, parse_list_style_type};
use super::dom::{NodeRef, NodeData, ElementData, TextData, WRAPPER_TAG_NAME};
use super::layout::{StyleData, InlineMaterial, TextMaterial, ImageMaterial};
use super::layout::{GlueMaterial, PenaltyMaterial, ChildArtifact, SiblingStyle, LoopContext};
use super::layout::{RootData, DrawState, DrawCommand, TextCommand, ImageCommand, FontKind, Fonts};
use super::layout::{TextAlign, ParagraphElement, TextElement, ImageElement, Display, Float};
use super::layout::{WordSpacing, ListStyleType, LineStats};
use super::layout::{hyph_lang, collapse_margins, DEFAULT_HYPH_LANG, HYPHENATION_PATTERNS};
use super::layout::{EM_SPACE_RATIOS, WORD_SPACE_RATIOS, FONT_SPACES};
use super::style::{StyleSheet, specified_values};
use super::xml::XmlExt;

const DEFAULT_DPI: u16 = 300;
const DEFAULT_WIDTH: u32 = 1404;
const DEFAULT_HEIGHT: u32 = 1872;

pub type Page = Vec<DrawCommand>;

pub trait ResourceFetcher {
    fn fetch(&mut self, name: &str) -> Result<Vec<u8>, Error>;
}

// TODO: Add min_font_size.
pub struct Engine {
    // The fonts used for each CSS font family.
    fonts: Option<Fonts>,
    // The penalty for lines ending with a hyphen.
    hyphen_penalty: i32,
    // The stretching/shrinking allowed for word spaces.
    stretch_tolerance: f32,
    // Page margins in pixels.
    pub margin: Edge,
    // Font size in points.
    pub font_size: f32,
    // Text alignment.
    pub text_align: TextAlign,
    // Line height in ems.
    pub line_height: f32,
    // Page dimensions in pixels.
    pub dims: (u32, u32),
    // Device DPI.
    pub dpi: u16,
}

impl Engine {
    pub fn new() -> Engine {
        let margin = Edge::uniform(mm_to_px(DEFAULT_MARGIN_WIDTH as f32, DEFAULT_DPI).round() as i32);
        let line_height = DEFAULT_LINE_HEIGHT;

        Engine {
            fonts: None,
            hyphen_penalty: HYPHEN_PENALTY,
            stretch_tolerance: STRETCH_TOLERANCE,
            margin,
            font_size: DEFAULT_FONT_SIZE,
            text_align: DEFAULT_TEXT_ALIGN,
            line_height,
            dims: (DEFAULT_WIDTH, DEFAULT_HEIGHT),
            dpi: DEFAULT_DPI,
        }
    }

    #[inline]
    pub fn load_fonts(&mut self) {
        if self.fonts.is_none() {
            self.fonts = default_fonts().ok();
        }
    }

    pub fn set_hyphen_penalty(&mut self, hyphen_penalty: i32) {
        self.hyphen_penalty = hyphen_penalty;
    }

    pub fn set_stretch_tolerance(&mut self, stretch_tolerance: f32) {
        self.stretch_tolerance = stretch_tolerance;
    }

    pub fn set_margin(&mut self, margin: &Edge) {
        self.margin = *margin;
    }

    pub fn set_font_size(&mut self, font_size: f32) {
        self.font_size = font_size;
    }

    pub fn layout(&mut self, width: u32, height: u32, font_size: f32, dpi: u16) {
        // TODO: Reject absurd values?
        self.dims = (width, height);
        self.dpi = dpi;
        self.font_size = font_size;
    }

    pub fn set_text_align(&mut self, text_align: TextAlign) {
        self.text_align = text_align;
    }

    pub fn set_font_family(&mut self, family_name: &str, search_path: &str) {
        if let Ok(serif_family) = FontFamily::from_name(family_name, search_path) {
            self.load_fonts();
            if let Some(fonts) = self.fonts.as_mut() {
                fonts.serif = serif_family;
            }
        }
    }

    pub fn set_margin_width(&mut self, width: i32) {
        if width >= 0 && width <= 10 {
            self.margin = Edge::uniform(mm_to_px(width as f32, self.dpi).round() as i32);
        }
    }

    pub fn set_line_height(&mut self, line_height: f32) {
        self.line_height = line_height;
    }

    #[inline]
    pub fn rect(&self) -> Rectangle {
        let (width, height) = self.dims;
        rect![0, 0, width as i32, height as i32]
    }

    pub fn build_display_list(&mut self, node: NodeRef, parent_style: &StyleData, loop_context: &LoopContext, stylesheet: &StyleSheet, root_data: &RootData, resource_fetcher: &mut dyn ResourceFetcher, draw_state: &mut DrawState, display_list: &mut Vec<Page>) -> ChildArtifact {
        // TODO: border, background, text-transform, tab-size, text-decoration.
        let mut style = StyleData::default();
        let mut rects: Vec<Option<Rectangle>> = vec![None];

        let props = specified_values(node, stylesheet);

        style.display = props.get("display").and_then(|value| parse_display(value))
                             .unwrap_or(Display::Block);

        if style.display == Display::None {
            return ChildArtifact {
                sibling_style: SiblingStyle {
                    padding: Edge::default(),
                    margin: Edge::default(),
                },
                rects: Vec::new(),
            }
        }

        style.font_style = parent_style.font_style;
        style.line_height = parent_style.line_height;
        style.retain_whitespace = parent_style.retain_whitespace;

        match node.tag_name() {
            Some("pre") => style.retain_whitespace = true,
            Some("li") | Some(WRAPPER_TAG_NAME) => style.list_style_type = parent_style.list_style_type,
            Some("table") => {
                let position = draw_state.position;
                draw_state.column_widths.clear();
                draw_state.min_column_widths.clear();
                draw_state.max_column_widths.clear();
                draw_state.center_table = style.display == Display::InlineTable &&
                                          parent_style.text_align == TextAlign::Center;
                self.compute_column_widths(node, parent_style, loop_context, stylesheet, root_data, resource_fetcher, draw_state);
                draw_state.position = position;
            },
            _ => (),
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

        style.word_spacing = props.get("word-spacing")
                                    .and_then(|value| parse_word_spacing(value, style.font_size, self.font_size, self.dpi))
                                    .unwrap_or(parent_style.word_spacing);

        style.vertical_align = props.get("vertical-align")
                                    .and_then(|value| parse_vertical_align(value, style.font_size, self.font_size, style.line_height, self.dpi))
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
                                .map(String::as_str)
                                .or_else(|| node.attribute("align"))
                                .and_then(|value| parse_text_align(value))
                                .unwrap_or(parent_style.text_align);

        style.font_features = props.get("font-feature-settings")
                                   .map(|value| parse_font_features(value))
                                   .or_else(|| parent_style.font_features.clone());

        if let Some(value) = props.get("list-style-type")
                                  .map(|value| parse_list_style_type(value)) {
            style.list_style_type = value;
        }

        if let Some(value) = props.get("font-variant") {
            let mut features = parse_font_variant(value);
            if let Some(v) = style.font_features.as_mut() {
                v.append(&mut features);
            }
        }

        if node.parent().is_some() {
            style.margin = parse_edge(props.get("margin-top").map(String::as_str),
                                      props.get("margin-right").map(String::as_str),
                                      props.get("margin-bottom").map(String::as_str),
                                      props.get("margin-left").map(String::as_str),
                                      style.font_size, self.font_size, parent_style.width, self.dpi);

            // Collapse the bottom margin of the previous sibling with the current top margin
            style.margin.top = collapse_margins(loop_context.sibling_style.margin.bottom, style.margin.top);

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

        style.width = props.get("width")
                           .and_then(|value| parse_width(value, style.font_size, self.font_size,
                                                         parent_style.width, self.dpi))
                           .unwrap_or(0);

        style.height = props.get("height")
                            .and_then(|value| parse_height(value, style.font_size, self.font_size,
                                                           parent_style.width, self.dpi))
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
                style.padding.right = (style.padding.right as f32 * ratio).round() as i32;
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
            draw_state.position.y = root_data.rect.min.y;
        }

        draw_state.position.y += style.padding.top;

        let has_blocks = node.children().any(|n| n.is_block());

        if has_blocks {
            if node.id().is_some() {
                display_list.last_mut().unwrap()
                            .push(DrawCommand::Marker(root_data.start_offset + node.offset()));
            }
            if node.has_children() {
                let mut inner_loop_context = LoopContext::default();

                if node.tag_name() == Some("tr") {
                    inner_loop_context.is_first = loop_context.is_first;
                    inner_loop_context.is_last = loop_context.is_last;

                    if draw_state.column_widths.is_empty() {
                        let min_row_width: i32 = draw_state.min_column_widths.iter().sum();
                        let max_row_width: i32 = draw_state.max_column_widths.iter().sum();
                        // https://www.w3.org/MarkUp/html3/tables.html
                        if min_row_width >= width {
                            draw_state.column_widths =
                                draw_state.min_column_widths.iter()
                                          .map(|w| ((*w as f32 / min_row_width as f32) *
                                                   width as f32).round() as i32)
                                          .collect();
                        } else if max_row_width <= width {
                            draw_state.column_widths = draw_state.max_column_widths.clone();
                        } else {
                            let dw = (width - min_row_width) as f32;
                            let dr = (max_row_width - min_row_width) as f32;
                            let gf = dw / dr;
                            draw_state.column_widths =
                                draw_state.min_column_widths.iter()
                                          .zip(draw_state.max_column_widths.iter())
                                          .map(|(a, b)| a + ((b - a) as f32 * gf).round() as i32)
                                          .collect();
                        }
                    }

                    if draw_state.center_table {
                        let actual_width = draw_state.column_widths.iter().sum();
                        let delta_width = width - actual_width;
                        let left_shift = delta_width / 2;
                        let right_shift = delta_width - left_shift;
                        style.start_x += left_shift;
                        style.end_x -= right_shift;
                        style.width = actual_width;
                    }

                    let start_x = style.start_x;
                    let end_x = style.end_x;
                    let mut cur_x = start_x;
                    let position = draw_state.position;
                    let mut final_page = (0, position);
                    let page_index = display_list.len() - 1;
                    let mut index = 0;

                    // TODO: rowspan, vertical-align
                    for child in node.children().filter(|child| child.is_element()) {
                        if index >= draw_state.column_widths.len() {
                            break;
                        }

                        let colspan = child.attribute("colspan")
                                           .and_then(|v| v.parse().ok())
                                           .unwrap_or(1)
                                           .min(draw_state.column_widths.len()-index);
                        let column_width = draw_state.column_widths[index..index+colspan]
                                                     .iter().sum::<i32>();
                        let mut child_display_list = vec![Vec::new()];
                        style.start_x = cur_x;
                        style.end_x = cur_x + column_width;
                        draw_state.position = position;
                        let artifact = self.build_display_list(child, &style, &inner_loop_context, stylesheet, root_data, resource_fetcher, draw_state, &mut child_display_list);
                        let pages_count = child_display_list.len();
                        if pages_count > final_page.0 ||
                           (pages_count == final_page.0 && draw_state.position.y > final_page.1.y) {
                            final_page = (pages_count, draw_state.position);
                        }

                        for (i, mut pg) in child_display_list.into_iter().enumerate() {
                            if let Some(page) = display_list.get_mut(page_index+i) {
                                page.append(&mut pg);
                            } else {
                                display_list.push(pg);
                            }
                        }

                        for (i, rect) in artifact.rects.into_iter().enumerate() {
                            if let Some(page_rect) = rects.get_mut(i) {
                                if let Some(pr) = page_rect.as_mut() {
                                    if let Some(r) = rect.as_ref() {
                                        pr.absorb(r);
                                    }
                                } else {
                                    *page_rect = rect;
                                }
                            } else {
                                rects.push(rect);
                            }
                        }

                        inner_loop_context.sibling_style = artifact.sibling_style;

                        if inner_loop_context.is_last {
                            style.margin.bottom = collapse_margins(inner_loop_context.sibling_style.margin.bottom, style.margin.bottom);
                        }

                        index += colspan;
                        cur_x += column_width;
                    }

                    style.start_x = start_x;
                    style.end_x = end_x;
                    draw_state.position = final_page.1;
                } else {
                    let mut iter = node.children().filter(|child| child.is_element()).peekable();
                    inner_loop_context.is_first = true;
                    let mut index = 0;

                    while let Some(child) = iter.next() {
                        if iter.peek().is_none() {
                            inner_loop_context.is_last = true;
                        }

                        inner_loop_context.index = index;

                        if child.is_wrapper() {
                            inner_loop_context.index = loop_context.index;
                        }

                        let artifact = self.build_display_list(child, &style, &inner_loop_context, stylesheet, root_data, resource_fetcher, draw_state, display_list);
                        inner_loop_context.sibling_style = artifact.sibling_style;
                        inner_loop_context.is_first = false;

                        // Collapse the bottom margin of the last child and its parent.
                        if inner_loop_context.is_last {
                            style.margin.bottom = collapse_margins(inner_loop_context.sibling_style.margin.bottom, style.margin.bottom);
                        }

                        let last_index = rects.len() - 1;
                        for (i, rect) in artifact.rects.into_iter().enumerate() {
                            if let Some(page_rect) = rects.get_mut(last_index + i) {
                                if let Some(pr) = page_rect.as_mut() {
                                    if let Some(r) = rect.as_ref() {
                                        pr.absorb(r);
                                    }
                                } else {
                                    *page_rect = rect;
                                }
                            } else {
                                rects.push(rect);
                            }
                        }

                        index += 1
                    }
                }
            }
        } else {
            if node.has_children() {
                let mut inlines = Vec::new();
                let mut markers = Vec::new();
                if node.id().is_some() {
                    markers.push(node.offset());
                }
                for child in node.children() {
                    self.gather_inline_material(child, stylesheet, &style, &root_data.spine_dir, &mut markers, &mut inlines);
                }
                if !inlines.is_empty() {
                    draw_state.prefix = match style.list_style_type {
                        None => {
                            let parent = if node.is_wrapper() {
                                node.ancestor_elements().nth(1)
                            } else {
                                node.parent_element()
                            };
                            match parent.and_then(|parent| parent.tag_name()) {
                                Some("ul") => format_list_prefix(ListStyleType::Disc, loop_context.index),
                                Some("ol") => format_list_prefix(ListStyleType::Decimal, loop_context.index),
                                _ => None,
                            }
                        },
                        Some(kind) => format_list_prefix(kind, loop_context.index),
                    };
                    self.place_paragraphs(&inlines, &style, root_data, &markers, resource_fetcher, draw_state, &mut rects, display_list);
                }
            } else {
                if node.id().is_some() {
                    display_list.last_mut().unwrap()
                                .push(DrawCommand::Marker(root_data.start_offset + node.offset()));
                }
            }
        }

        if style.height > 0 {
            let height = rects.iter()
                              .filter_map(|v| v.map(|r| r.height() as i32))
                              .sum::<i32>();
            draw_state.position.y += (style.height - height).max(0);
        }

        // Collapse top and bottom margins of empty blocks.
        if rects.is_empty() {
            style.margin.bottom = collapse_margins(style.margin.bottom, style.margin.top);
            style.margin.top = 0;
        }

        draw_state.position.y += style.padding.bottom;

        if props.get("page-break-after").map(String::as_str) == Some("always") {
            display_list.push(Vec::new());
            draw_state.position.y = root_data.rect.min.y;
        }

        ChildArtifact {
            sibling_style: SiblingStyle {
                padding: style.padding,
                margin: style.margin,
            },
            rects,
        }
    }

    fn compute_column_widths(&mut self, node: NodeRef, parent_style: &StyleData, loop_context: &LoopContext, stylesheet: &StyleSheet, root_data: &RootData, resource_fetcher: &mut dyn ResourceFetcher, draw_state: &mut DrawState) {
        if node.tag_name() == Some("tr") {
            let mut index = 0;
            for child in node.children().filter(|c| c.is_element()) {
                let colspan = child.attribute("colspan")
                                   .and_then(|v| v.parse().ok())
                                   .unwrap_or(1);
                let mut display_list = vec![Vec::new()];
                let artifact = self.build_display_list(child, parent_style, loop_context, stylesheet, root_data, resource_fetcher, draw_state, &mut display_list);
                let horiz_padding = artifact.sibling_style.padding.left +
                                    artifact.sibling_style.padding.right;
                let min_width = display_list.into_iter()
                                            .flatten()
                                            .filter_map(|dc| {
                                                match dc {
                                                    DrawCommand::Text(TextCommand { rect, .. }) => Some(rect.width() as i32 + horiz_padding),
                                                    DrawCommand::Image(ImageCommand { rect, .. }) => Some((rect.width() as i32).min(pt_to_px(parent_style.font_size, self.dpi).round().max(1.0) as i32) + horiz_padding),
                                                    _ => None,
                                                }
                                            })
                                            .max().unwrap_or(0);
                let max_width = artifact.rects.into_iter()
                                        .filter_map(|v| v.map(|r| r.width() as i32 + horiz_padding))
                                        .max().unwrap_or(0);
                if colspan == 1 {
                    if let Some(cw) = draw_state.min_column_widths.get_mut(index) {
                        *cw = (*cw).max(min_width);
                    } else {
                        draw_state.min_column_widths.push(min_width);
                    }
                    if let Some(cw) = draw_state.max_column_widths.get_mut(index) {
                        *cw = (*cw).max(max_width);
                    } else {
                        draw_state.max_column_widths.push(max_width);
                    }
                }

                index += colspan;
            }
        } else {
            for child in node.children().filter(|c| c.is_element()) {
                self.compute_column_widths(child, parent_style, loop_context, stylesheet, root_data, resource_fetcher, draw_state);
            }
        }
    }

    fn gather_inline_material(&self, node: NodeRef, stylesheet: &StyleSheet, parent_style: &StyleData, spine_dir: &PathBuf, markers: &mut Vec<usize>, inlines: &mut Vec<InlineMaterial>) {
        match node.data() {
            NodeData::Element(ElementData { offset, name, attributes, .. }) => {
                let mut style = StyleData::default();
                let props = specified_values(node, stylesheet);

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

                style.word_spacing = props.get("word-spacing")
                                            .and_then(|value| parse_word_spacing(value, style.font_size, self.font_size, self.dpi))
                                            .unwrap_or(parent_style.word_spacing);

                style.vertical_align = props.get("vertical-align")
                                            .and_then(|value| parse_vertical_align(value, style.font_size, self.font_size, style.line_height, self.dpi))
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
                    "img" | "image" => {
                        let attr = if name == "img" { "src" } else { "xlink:href" };

                        let path = attributes.get(attr).and_then(|src| {
                            spine_dir.join(src).normalize().to_str()
                                     .map(|uri| percent_decode_str(&decode_entities(uri))
                                                                  .decode_utf8_lossy()
                                                                  .into_owned())
                        }).unwrap_or_default();

                        style.float = props.get("float").and_then(|value| parse_float(value));

                        let is_block = style.display == Display::Block;
                        if is_block || style.float.is_some() {
                            style.margin = parse_edge(props.get("margin-top").map(String::as_str),
                                                      props.get("margin-right").map(String::as_str),
                                                      props.get("margin-bottom").map(String::as_str),
                                                      props.get("margin-left").map(String::as_str),
                                                      style.font_size, self.font_size, parent_style.width, self.dpi);
                        }
                        if is_block {
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
                        style.uri = attributes.get("href")
                                              .map(|uri| percent_decode_str(&decode_entities(uri))
                                                                           .decode_utf8_lossy().into_owned());
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

                for child in node.children() {
                    self.gather_inline_material(child, stylesheet, &style, spine_dir, markers, inlines);
                }

                if let Some(mut v) = props.get("-plato-insert-after")
                                          .map(|value| parse_inline_material(value, style.font_size, self.font_size, self.dpi)) {
                    inlines.append(&mut v);
                }
            },
            NodeData::Text(TextData { offset, text }) => {
                inlines.push(InlineMaterial::Text(TextMaterial {
                    offset: *offset,
                    text: decode_entities(text).into_owned(),
                    style: parent_style.clone(),
                }));
            },
            NodeData::Whitespace(TextData { offset, text }) if parent_style.retain_whitespace => {
                inlines.push(InlineMaterial::Text(TextMaterial {
                    offset: *offset,
                    text: text.to_string(),
                    style: parent_style.clone(),
                }));
            },
            _ => (),
        }
    }

    fn make_paragraph_items(&mut self, inlines: &[InlineMaterial], parent_style: &StyleData, line_width: i32, resource_fetcher: &mut dyn ResourceFetcher) -> (Vec<ParagraphItem<ParagraphElement>>, Vec<ImageElement>) {
        let mut items = Vec::new();
        let mut floats = Vec::new();
        let big_stretch = 3 * {
            let font_size = (parent_style.font_size * 64.0) as u32;
            let font = self.fonts.as_mut().unwrap()
                           .get_mut(parent_style.font_kind,
                                    parent_style.font_style,
                                    parent_style.font_weight);
            font.set_size(font_size, self.dpi);
            font.plan(" ", None, None).width
        };

        if parent_style.text_align == TextAlign::Center {
            items.push(ParagraphItem::Box { width: 0, data: ParagraphElement::Nothing });
            items.push(ParagraphItem::Glue { width: 0, stretch: big_stretch, shrink: 0 });
        }

        for (index, mater) in inlines.iter().enumerate() {
            match mater {
                InlineMaterial::Image(ImageMaterial { offset, path, style }) => {
                    let (mut width, mut height) = (style.width, style.height);
                    let mut scale = 1.0;
                    let dpi = self.dpi;

                    if let Ok(buf) = resource_fetcher.fetch(path) {
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

                        if width * height > 0 {
                            let element = ImageElement {
                                    offset: *offset,
                                    width,
                                    height,
                                    scale,
                                    vertical_align: style.vertical_align,
                                    display: style.display,
                                    margin: style.margin,
                                    float: style.float,
                                    path: path.clone(),
                                    uri: style.uri.clone(),
                            };
                            if style.float.is_none() {
                                items.push(ParagraphItem::Box {
                                    width,
                                    data: ParagraphElement::Image(element),
                                });
                            } else {
                                floats.push(element);
                            }
                        }
                    }
                },
                InlineMaterial::Text(TextMaterial { offset, text, style }) => {
                    let font_size = (style.font_size * 64.0) as u32;
                    let space_plan = {
                        let font = self.fonts.as_mut().unwrap()
                                       .get_mut(parent_style.font_kind,
                                                parent_style.font_style,
                                                parent_style.font_weight);
                        font.set_size(font_size, self.dpi);
                        font.plan(" 0.", None, None)
                    };
                    let mut start_index = 0;
                    for (end_index, _is_hardbreak) in LineBreakIterator::new(text) {
                        for chunk in text[start_index..end_index].split_inclusive(char::is_whitespace) {
                            if let Some((i, c)) = chunk.char_indices().next_back() {
                                let j = i + if c.is_whitespace() { 0 } else { c.len_utf8() };
                                if j > 0 {
                                    let buf = &text[start_index..start_index+j];
                                    let local_offset = offset + start_index;
                                    let mut plan = {
                                        let font = self.fonts.as_mut().unwrap()
                                                       .get_mut(style.font_kind,
                                                                style.font_style,
                                                                style.font_weight);
                                        font.set_size(font_size, self.dpi);
                                        font.plan(buf, None, style.font_features.as_deref())
                                    };
                                    plan.space_out(style.letter_spacing);

                                    items.push(ParagraphItem::Box {
                                        width: plan.width,
                                        data: ParagraphElement::Text(TextElement {
                                            offset: local_offset,
                                            language: style.language.clone(),
                                            text: buf.to_string(),
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
                                }
                                if c.is_whitespace() {
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
                                        start_index += chunk.len();
                                        continue;
                                    }

                                    let last_c = text[..start_index+i].chars().next_back().or_else(|| {
                                        if index > 0 {
                                            inlines[index-1].text().and_then(|text| text.chars().next_back())
                                        } else {
                                            None
                                        }
                                    });

                                    if !parent_style.retain_whitespace && c.is_xml_whitespace() &&
                                        (last_c.map(|c| c.is_xml_whitespace()) == Some(true)) {
                                            start_index += chunk.len();
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

                                    width += match style.word_spacing {
                                        WordSpacing::Normal => 0,
                                        WordSpacing::Length(l) => l,
                                        WordSpacing::Ratio(r) => (r * width as f32) as i32,
                                    } + style.letter_spacing;

                                    let is_unbreakable = c == '\u{00A0}' || c == '\u{202F}' || c == '\u{2007}';

                                    if (is_unbreakable || (parent_style.retain_whitespace && c.is_xml_whitespace())) &&
                                       (last_c == Some('\n') || last_c.is_none()) {
                                        items.push(ParagraphItem::Box { width: 0, data: ParagraphElement::Nothing });
                                    }

                                    if is_unbreakable {
                                        items.push(ParagraphItem::Penalty { width: 0, penalty: INFINITE_PENALTY, flagged: false });
                                    }

                                    match parent_style.text_align {
                                        TextAlign::Justify => {
                                            items.push(ParagraphItem::Glue { width, stretch: width/2, shrink: width/3 });
                                        },
                                        TextAlign::Center => {
                                            if style.font_kind == FontKind::Monospace || is_unbreakable {
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
                                            if style.font_kind == FontKind::Monospace || is_unbreakable {
                                                items.push(ParagraphItem::Glue { width, stretch: 0, shrink: 0 });
                                            } else {
                                                let stretch = 3 * width;
                                                items.push(ParagraphItem::Glue { width: 0, stretch, shrink: 0 });
                                                items.push(ParagraphItem::Penalty { width: 0, penalty: 0, flagged: false });
                                                items.push(ParagraphItem::Glue { width, stretch: -stretch, shrink: 0 });
                                            }
                                        },
                                    }
                                } else if end_index < text.len() {
                                    let penalty = if c == '-' { self.hyphen_penalty } else { 0 };
                                    let flagged = penalty > 0;
                                    items.push(ParagraphItem::Penalty { width: 0, penalty, flagged });
                                }
                            }
                            start_index += chunk.len();
                        }
                    }
                },
                InlineMaterial::LineBreak => {
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

        if items.last().map(ParagraphItem::penalty) != Some(-INFINITE_PENALTY) {
            items.push(ParagraphItem::Penalty { penalty: INFINITE_PENALTY,  width: 0, flagged: false });

            let stretch = if parent_style.text_align == TextAlign::Center { big_stretch } else { line_width };
            items.push(ParagraphItem::Glue { width: 0, stretch, shrink: 0 });

            items.push(ParagraphItem::Penalty { penalty: -INFINITE_PENALTY, width: 0, flagged: true });
        }

        (items, floats)
    }

    fn place_paragraphs(&mut self, inlines: &[InlineMaterial], style: &StyleData, root_data: &RootData, markers: &[usize], resource_fetcher: &mut dyn ResourceFetcher, draw_state: &mut DrawState, rects: &mut Vec<Option<Rectangle>>, display_list: &mut Vec<Page>) {
        let position = &mut draw_state.position;

        let text_indent = if style.text_align == TextAlign::Center {
            0
        } else {
            style.text_indent
        };

        let stretch_tolerance = if style.text_align == TextAlign::Justify {
            self.stretch_tolerance
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

        position.y += style.margin.top + space_top;

        let line_width = style.end_x - style.start_x;

        let mut page = display_list.pop().unwrap();
        let mut page_rect = rects.pop().unwrap();
        if position.y > root_data.rect.max.y - space_bottom {
            rects.push(page_rect.take());
            display_list.push(page);
            position.y = root_data.rect.min.y + space_top;
            page = Vec::new();
        }

        let (mut items, floats) = self.make_paragraph_items(inlines, style, line_width, resource_fetcher);
        let page_index = display_list.len();

        for mut element in floats.into_iter() {
            let horiz_margin = element.margin.left + element.margin.right;
            let vert_margin = element.margin.top + element.margin.bottom;
            let mut width = element.width;
            let mut height = element.height;

            let max_width = line_width / 3;
            if width + horiz_margin > max_width {
                let ratio = (max_width - horiz_margin) as f32 / width as f32;
                element.scale *= ratio;
                width = max_width - horiz_margin;
                height = (ratio * height as f32).round() as i32;
            }

            let mut y_min = position.y - space_top;
            let side = if element.float == Some(Float::Left) { 0 } else { 1 };

            if let Some(ref mut floating_rects) = draw_state.floats.get_mut(&page_index) {
                if let Some(orect) = floating_rects.iter().rev()
                                                   .find(|orect| orect.max.y > y_min &&
                                                                 (orect.min.x - style.start_x).signum() == side) {
                    y_min = orect.max.y;
                }
            }

            let max_height = 2 * (root_data.rect.max.y - space_bottom - y_min) / 3;
            if height + vert_margin > max_height {
                let ratio = (max_height - vert_margin) as f32 / height as f32;
                element.scale *= ratio;
                height = max_height - vert_margin;
                width = (ratio * width as f32).round() as i32;
            }

            if width > 0 && height > 0 {
                let mut rect = if element.float == Some(Float::Left) {
                    rect![style.start_x, y_min,
                          style.start_x + width + horiz_margin,
                          y_min + height + vert_margin]
                } else {
                    rect![style.end_x - width - horiz_margin, y_min,
                          style.end_x, y_min + height + vert_margin]
                };

                let floating_rects = draw_state.floats.entry(page_index).or_default();
                floating_rects.push(rect);

                rect.shrink(&element.margin);
                page.push(DrawCommand::Image(ImageCommand {
                    offset: element.offset + root_data.start_offset,
                    position: rect.min,
                    rect,
                    scale: element.scale,
                    path: element.path,
                    uri: element.uri,
                }));
            }
        }

        let para_shape = if let Some(floating_rects) = draw_state.floats.get(&page_index) {
            let max_lines = (root_data.rect.max.y - position.y + space_top) / style.line_height;
            let mut para_shape = Vec::new();
            for index in 0..max_lines {
                let y_min = position.y - space_top + index * style.line_height;
                let mut rect = rect![pt!(style.start_x, y_min),
                                     pt!(style.end_x, y_min + style.line_height)];
                for frect in floating_rects {
                    if rect.overlaps(frect) {
                        if frect.min.x > rect.min.x {
                            rect.max.x = frect.min.x;
                        } else {
                            rect.min.x = frect.max.x;
                        }
                    }
                }
                para_shape.push((rect.min.x, rect.max.x));
            }
            para_shape.push((style.start_x, style.end_x));
            para_shape
        } else {
            vec![(style.start_x, style.end_x); 2]
        };

        let mut line_lengths: Vec<i32> = para_shape.iter().map(|(a, b)| b - a).collect();
        line_lengths[0] -= text_indent;

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

            items = self.hyphenate_paragraph(dictionary, items, &mut hyph_indices);
            bps = total_fit(&items, &line_lengths, stretch_tolerance, 0);
        }

        if bps.is_empty() {
            bps = standard_fit(&items, &line_lengths, stretch_tolerance);
        }

        if bps.is_empty() {
            let max_width = *line_lengths.iter().min().unwrap();

            for itm in &mut items {
                if let ParagraphItem::Box { width, data } = itm {
                    if *width > max_width {
                        match data {
                            ParagraphElement::Text(TextElement { plan, font_kind, font_style, font_weight, font_size, .. }) => {
                                let font = self.fonts.as_mut().unwrap()
                                               .get_mut(*font_kind, *font_style, *font_weight);
                                font.set_size(*font_size, self.dpi);
                                font.crop_right(plan, max_width);
                                *width = plan.width;
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

            bps = standard_fit(&items, &line_lengths, self.stretch_tolerance);
        }

        // Remove unselected optional hyphens (prevents broken ligatures).
        if !bps.is_empty() && !hyph_indices.is_empty() {
            items = self.cleanup_paragraph(items, &hyph_indices, &mut glue_drifts, &mut bps);
        }

        let mut last_index = 0;
        let mut markers_index = 0;
        let mut last_x_position = 0;
        let mut is_first_line = true;

        if let Some(prefix) = draw_state.prefix.as_ref() {
            let font_size = (style.font_size * 64.0) as u32;
            let prefix_plan = {
                let font = self.fonts.as_mut().unwrap()
                               .get_mut(style.font_kind, style.font_style, style.font_weight);
                font.set_size(font_size, self.dpi);
                font.plan(prefix, None, style.font_features.as_deref())
            };
            let (start_x, _) = para_shape[0];
            let pt = pt!(start_x - prefix_plan.width, position.y);
            let rect = rect![pt + pt!(0, -ascender), pt + pt!(prefix_plan.width, -descender)];
            if let Some(first_offset) = inlines.iter().filter_map(|elt| elt.offset()).next() {
                page.push(DrawCommand::ExtraText(TextCommand {
                    offset: root_data.start_offset + first_offset,
                    position: pt,
                    rect,
                    text: prefix.to_string(),
                    plan: prefix_plan,
                    uri: None,
                    font_kind: style.font_kind,
                    font_style: style.font_style,
                    font_weight: style.font_weight,
                    font_size,
                    color: style.color,
                }));
            }
        }

        for (j, bp) in bps.into_iter().enumerate() {
            let drift = if glue_drifts.is_empty() {
                0.0
            } else {
                glue_drifts[j]
            };

            let (start_x, end_x) = para_shape[j.min(para_shape.len() - 1)];

            let Breakpoint { index, width, mut ratio } = bp;
            let mut epsilon: f32 = 0.0;
            let current_text_indent = if is_first_line { text_indent } else { 0 };

            match style.text_align {
                TextAlign::Right => position.x = end_x - width - current_text_indent,
                _ => position.x = start_x + current_text_indent,
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
                                let rect = rect![pt + pt!(0, -ascender), pt + pt!(element.plan.width, -descender)];
                                if let Some(pr) = page_rect.as_mut() {
                                    pr.absorb(&rect);
                                } else {
                                    page_rect = Some(rect);
                                }
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
                                    position.y += element.margin.top;
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
                                        let mut left_margin = element.margin.left;
                                        let total_width = left_margin + width + element.margin.right;
                                        if total_width > line_width {
                                            let remaining_space = line_width - width;
                                            let ratio = left_margin as f32 / (left_margin + element.margin.right) as f32;
                                            left_margin = (ratio * remaining_space as f32).round() as i32;
                                        }
                                        position.x = start_x + left_margin;
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
                                    position.y += height + element.margin.bottom;
                                    if element.display == Display::Block {
                                        position.y -= space_bottom;
                                    }
                                    (width, height, pt, scale)
                                } else {
                                    let mut pt = pt!(position.x, position.y - element.height - element.vertical_align);

                                    if pt.y < root_data.rect.min.y {
                                        pt.y = root_data.rect.min.y;
                                    }

                                    (element.width, element.height, pt, element.scale)
                                };

                                let rect = rect![pt, pt + pt!(w, h)];

                                if let Some(pr) = page_rect.as_mut() {
                                    pr.absorb(&rect);
                                } else {
                                    page_rect = Some(rect);
                                }

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
                    ParagraphItem::Glue { width, stretch, shrink } if ratio.is_finite() => {
                        let amplitude = if ratio.is_sign_positive() { stretch } else { shrink };
                        let exact_width = width as f32 + ratio * amplitude as f32 + drift;
                        let approx_width = if epsilon.is_sign_positive() {
                            exact_width.floor() as i32
                        } else {
                            exact_width.ceil() as i32
                        };
                        // <td>&nbsp;=&nbsp;</td>
                        if stretch == 0 && shrink == 0 {
                            let rect = rect![*position + pt!(0, -ascender),
                                             *position + pt!(approx_width, -descender)];
                            if let Some(pr) = page_rect.as_mut() {
                                pr.absorb(&rect);
                            } else {
                                page_rect = Some(rect);
                            }
                        }
                        epsilon += approx_width as f32 - exact_width;
                        position.x += approx_width;
                    },
                    _ => (),
                }
            }

            if let ParagraphItem::Penalty { width, .. } = items[index] {
                if width > 0 {
                    let font_size = (style.font_size * 64.0) as u32;
                    let mut hyphen_plan = {
                        let font = self.fonts.as_mut().unwrap()
                                       .get_mut(style.font_kind, style.font_style, style.font_weight);
                        font.set_size(font_size, self.dpi);
                        font.plan("-", None, style.font_features.as_deref())
                    };
                    if let Some(DrawCommand::Text(TextCommand { ref mut rect, ref mut plan, ref mut text, .. })) = page.last_mut() {
                        rect.max.x += hyphen_plan.width;
                        plan.append(&mut hyphen_plan);
                        text.push('\u{00AD}');
                    }
                }
            }

            last_index = index;
            is_first_line = false;

            if index < items.len() - 1 {
                position.y += style.line_height;
            }

            if position.y > root_data.rect.max.y - space_bottom {
                rects.push(page_rect.take());
                display_list.push(page);
                position.y = root_data.rect.min.y + space_top;
                page = Vec::new();
            }
        }

        let last_page = if !page.is_empty() {
            Some(&mut page)
        } else {
            display_list.iter_mut().rev().find(|page| !page.is_empty())
        };

        if let Some(last_page) = last_page {
            while let Some(offset) = markers.get(markers_index) {
                last_page.push(DrawCommand::Marker(root_data.start_offset + *offset));
                markers_index += 1;
            }
        }

        rects.push(page_rect.take());

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
            font.plan(chunk, None, element.font_features.as_deref())
        };
        plan.space_out(element.letter_spacing);
        ParagraphItem::Box {
            width: plan.width,
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

    fn hyphenate_paragraph(&mut self, dictionary: Option<&Standard>, items: Vec<ParagraphItem<ParagraphElement>>, hyph_indices: &mut Vec<[usize; 2]>) -> Vec<ParagraphItem<ParagraphElement>> {
        let mut hyph_items = Vec::with_capacity(items.len());

        for itm in items {
            match itm {
                ParagraphItem::Box { data: ParagraphElement::Text(ref element), .. } => {
                    let text = &element.text;
                    let hyphen_width = if dictionary.is_some() {
                        let font = self.fonts.as_mut().unwrap()
                                       .get_mut(element.font_kind, element.font_style, element.font_weight);
                        font.set_size(element.font_size, self.dpi);
                        font.plan("-", None, element.font_features.as_deref()).width
                    } else {
                        0
                    };

                    if let Some(dict) = dictionary {
                        let mut index_before = text.find(char::is_alphabetic).unwrap_or_else(|| text.len());
                        if index_before > 0 {
                            let subelem = self.box_from_chunk(&text[0..index_before],
                                                              0,
                                                              element);
                            hyph_items.push(subelem);
                        }

                        let mut index_after = text[index_before..].find(|c: char| !c.is_alphabetic())
                                                                  .map(|i| index_before + i)
                                                                  .unwrap_or_else(|| text.len());
                        while index_before < index_after {
                            let mut index = 0;
                            let chunk = &text[index_before..index_after];
                            let len_before = hyph_items.len();
                            for segment in dict.hyphenate(chunk).iter().segments() {
                                let subelem = self.box_from_chunk(segment,
                                                                  index_before + index,
                                                                  element);
                                hyph_items.push(subelem);
                                index += segment.len();
                                if index < chunk.len() {
                                    hyph_items.push(ParagraphItem::Penalty { width: hyphen_width, penalty: self.hyphen_penalty, flagged: true });
                                }
                            }
                            let len_after = hyph_items.len();
                            if len_after > 1 + len_before {
                                hyph_indices.push([len_before, len_after]);
                            }
                            index_before = text[index_after..].find(char::is_alphabetic)
                                                               .map(|i| index_after + i)
                                                               .unwrap_or_else(|| text.len());
                            if index_before > index_after {
                                let subelem = self.box_from_chunk(&text[index_after..index_before],
                                                                  index_after,
                                                                  &element);
                                hyph_items.push(subelem);
                            }

                            index_after = text[index_before..].find(|c: char| !c.is_alphabetic())
                                                               .map(|i| index_before + i)
                                                               .unwrap_or_else(|| text.len());
                        }
                    } else {
                        let subelem = self.box_from_chunk(text, 0, element);
                        hyph_items.push(subelem);
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
                    plan.space_out(letter_spacing);
                    merged_width = plan.width;
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
                        start_index = usize::MAX;
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
                        plan.space_out(letter_spacing);
                        merged_width = plan.width;
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

    pub fn render_page(&mut self, page: &[DrawCommand], scale_factor: f32, resource_fetcher: &mut dyn ResourceFetcher) -> Option<Pixmap> {
        let width = (self.dims.0 as f32 * scale_factor) as u32;
        let height = (self.dims.1 as f32 * scale_factor) as u32;
        let mut fb = Pixmap::try_new(width, height)?;

        for dc in page {
            match dc {
                DrawCommand::Text(TextCommand { position, plan, font_kind,
                                                font_style, font_weight, font_size, color, .. }) |
                DrawCommand::ExtraText(TextCommand { position, plan, font_kind, font_style,
                                                     font_weight, font_size, color, .. }) => {
                    let font = self.fonts.as_mut().unwrap()
                                   .get_mut(*font_kind, *font_style, *font_weight);
                    let font_size = (scale_factor * *font_size as f32) as u32;
                    let position = Point::from(scale_factor * Vec2::from(*position));
                    let plan = plan.scale(scale_factor);
                    font.set_size(font_size, self.dpi);
                    font.render(&mut fb, *color, &plan, position);
                },
                DrawCommand::Image(ImageCommand { position, path, scale, .. }) => {
                    if let Ok(buf) = resource_fetcher.fetch(path) {
                        if let Some((pixmap, _)) = PdfOpener::new().and_then(|opener| {
                            opener.open_memory(path, &buf)
                        }).and_then(|mut doc| {
                            doc.pixmap(Location::Exact(0), scale_factor * *scale)
                        }) {
                            let position = Point::from(scale_factor * Vec2::from(*position));
                            fb.draw_pixmap(&pixmap, position);
                        }
                    }
                },
                _ => (),
            }
        }

        Some(fb)
    }
}

fn format_list_prefix(kind: ListStyleType, index: usize) -> Option<String> {
    match kind {
        ListStyleType::None => None,
        ListStyleType::Disc => Some("• ".to_string()),
        ListStyleType::Circle => Some("◦ ".to_string()),
        ListStyleType::Square => Some("▪ ".to_string()),
        ListStyleType::Decimal => Some(format!("{}. ", index + 1)),
        ListStyleType::LowerRoman => Some(format!("{}. ", Roman::from_unchecked(index as u32 + 1).to_lowercase())),
        ListStyleType::UpperRoman => Some(format!("{}. ", Roman::from_unchecked(index as u32 + 1).to_uppercase())),
        ListStyleType::LowerAlpha | ListStyleType::UpperAlpha => {
            let i = index as u32 % 26;
            let start = if kind == ListStyleType::LowerAlpha { 0x61 } else { 0x41 };
            Some(format!("{}. ", char::try_from(start + i).unwrap()))
        },
        ListStyleType::LowerGreek | ListStyleType::UpperGreek => {
            let mut i = index as u32 % 24;
            // Skip ς.
            if i >= 17 {
                i += 1;
            }
            let start = if kind == ListStyleType::LowerGreek { 0x03B1 } else { 0x0391 };
            Some(format!("{}. ", char::try_from(start + i).unwrap()))
        },
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
