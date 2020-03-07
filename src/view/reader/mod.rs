mod tool_bar;
mod bottom_bar;
mod results_bar;
mod margin_cropper;
mod results_label;

use std::thread;
use std::cmp::Ordering;
use std::sync::{Arc, Mutex, mpsc};
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering as AtomicOrdering;
use std::path::PathBuf;
use std::collections::{VecDeque, BTreeMap, HashMap, HashSet};
use chrono::Local;
use regex::Regex;
use septem::prelude::*;
use septem::{Roman, Digit};
use crate::input::{DeviceEvent, FingerStatus, ButtonCode, ButtonStatus};
use crate::framebuffer::{Framebuffer, UpdateMode, Pixmap};
use crate::view::{View, Event, AppCmd, Hub, Bus, ViewId, EntryKind, EntryId, SliderId};
use crate::view::{SMALL_BAR_HEIGHT, BIG_BAR_HEIGHT, THICKNESS_MEDIUM};
use crate::unit::{scale_by_dpi, mm_to_px};
use crate::device::CURRENT_DEVICE;
use crate::helpers::AsciiExtension;
use crate::font::Fonts;
use crate::font::family_names;
use self::margin_cropper::{MarginCropper, BUTTON_DIAMETER};
use super::top_bar::TopBar;
use self::tool_bar::ToolBar;
use self::bottom_bar::BottomBar;
use self::results_bar::ResultsBar;
use crate::view::common::{locate, rlocate, locate_by_id};
use crate::view::common::{toggle_main_menu, toggle_battery_menu, toggle_clock_menu};
use crate::view::filler::Filler;
use crate::view::named_input::NamedInput;
use crate::view::search_bar::SearchBar;
use crate::view::keyboard::Keyboard;
use crate::view::menu::{Menu, MenuKind};
use crate::view::notification::Notification;
use crate::settings::{guess_frontlight, FinishedAction};
use crate::settings::{DEFAULT_FONT_FAMILY, DEFAULT_TEXT_ALIGN, DEFAULT_LINE_HEIGHT, DEFAULT_MARGIN_WIDTH};
use crate::frontlight::LightLevels;
use crate::gesture::GestureEvent;
use crate::document::{Document, open, Location, TextLocation, BoundedText, Neighbors, BYTES_PER_PAGE};
use crate::document::{TocEntry, SimpleTocEntry, TocLocation, toc_as_html, chapter_from_index};
use crate::document::pdf::PdfOpener;
use crate::metadata::{Info, FileInfo, ReaderInfo, Annotation, TextAlign, ZoomMode, PageScheme};
use crate::metadata::{Margin, CroppingMargins, make_query};
use crate::metadata::{DEFAULT_CONTRAST_EXPONENT, DEFAULT_CONTRAST_GRAY};
use crate::geom::{Point, Rectangle, Boundary, CornerSpec, BorderSpec, Dir, DiagDir, CycleDir, LinearDir, Axis, halves};
use crate::color::{BLACK, WHITE};
use crate::app::Context;

const HISTORY_SIZE: usize = 32;
const RECT_DIST_JITTER: f32 = 24.0;
const ANNOTATION_DRIFT: u8 =  32;

pub struct Reader {
    rect: Rectangle,
    children: Vec<Box<dyn View>>,
    doc: Arc<Mutex<Box<dyn Document>>>,
    cache: BTreeMap<usize, Resource>,
    text: HashMap<usize, Vec<BoundedText>>,
    annotations: HashMap<usize, Vec<Annotation>>,
    chunks: Vec<RenderChunk>,
    focus: Option<ViewId>,
    search: Option<Search>,
    search_direction: LinearDir,
    held_buttons: HashSet<ButtonCode>,
    selection: Option<Selection>,
    target_annotation: Option<[TextLocation; 2]>,
    history: VecDeque<usize>,
    state: State,
    info: Info,
    current_page: usize,
    pages_count: usize,
    view_port: ViewPort,
    contrast: Contrast,
    synthetic: bool,
    page_turns: usize,
    reflowable: bool,
    ephemeral: bool,
    finished: bool,
}

#[derive(Debug)]
struct ViewPort {
    zoom_mode: ZoomMode,
    top_offset: i32,
    margin_width: i32,
}

impl Default for ViewPort {
    fn default() -> Self {
        ViewPort {
            zoom_mode: ZoomMode::FitToPage,
            top_offset: 0,
            margin_width: 0,
        }
    }
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
enum State {
    Idle,
    Selection(i32),
    AdjustSelection,
}

#[derive(Debug)]
struct Selection {
    start: TextLocation,
    end: TextLocation,
    anchor: TextLocation,
}

#[derive(Debug)]
struct Resource {
    pixmap: Pixmap,
    frame: Rectangle,
    scale: f32,
}

#[derive(Debug, Clone)]
struct RenderChunk {
    location: usize,
    frame: Rectangle,
    position: Point,
    scale: f32,
}

#[derive(Debug)]
struct Search {
    query: String,
    highlights: BTreeMap<usize, Vec<Vec<Boundary>>>,
    running: Arc<AtomicBool>,
    current_page: usize,
    results_count: usize,
}

impl Default for Search {
    fn default() -> Self {
        Search {
            query: String::new(),
            highlights: BTreeMap::new(),
            running: Arc::new(AtomicBool::new(true)),
            current_page: 0,
            results_count: 0,
        }
    }
}

#[derive(Debug)]
struct Contrast {
    exponent: f32,
    gray: f32,
}

impl Default for Contrast {
    fn default() -> Contrast {
        Contrast {
            exponent: DEFAULT_CONTRAST_EXPONENT,
            gray: DEFAULT_CONTRAST_GRAY,
        }
    }
}

fn scaling_factor(rect: &Rectangle, cropping_margin: &Margin, screen_margin_width: i32, dims: (f32, f32), zoom_mode: ZoomMode) -> f32 {
    let (page_width, page_height) = dims;
    let surface_width = (rect.width() as i32 - 2 * screen_margin_width) as f32;
    let frame_width = (1.0 - (cropping_margin.left + cropping_margin.right)) * page_width;
    let width_ratio = surface_width / frame_width;
    match zoom_mode {
        ZoomMode::FitToPage => {
            let surface_height = (rect.height() as i32 - 2 * screen_margin_width) as f32;
            let frame_height = (1.0 - (cropping_margin.top + cropping_margin.bottom)) * page_height;
            let height_ratio = surface_height / frame_height;
            width_ratio.min(height_ratio)
        },
        ZoomMode::FitToWidth => width_ratio,
    }
}

fn build_pixmap(rect: &Rectangle, doc: &mut dyn Document, location: usize) -> (Pixmap, usize) {
    let scale = scaling_factor(rect, &Margin::default(), 0, doc.dims(location).unwrap(), ZoomMode::FitToPage);
    doc.pixmap(Location::Exact(location), scale).unwrap()
}

fn find_cut(frame: &Rectangle, y_pos: i32, scale: f32, dir: LinearDir, lines: &[BoundedText]) -> Option<i32> {
    let y_pos_u = y_pos as f32 / scale;
    let frame_u = frame.to_boundary() / scale;
    let mut rect_a: Option<Boundary> = None;
    let max_line_height = frame_u.height() / 10.0;

    for line in lines {
        if frame_u.overlaps(&line.rect) && line.rect.height() <= max_line_height && y_pos_u >= line.rect.min.y && y_pos_u < line.rect.max.y {
            rect_a = Some(line.rect);
            break;
        }
    }

    let ra = rect_a?;

    let mut rect_b: Option<Boundary> = None;
    let target_ordering = if dir == LinearDir::Backward {
        Some(Ordering::Less)
    } else {
        Some(Ordering::Greater)
    };

    for line in lines {
        if line.rect.min.x < ra.max.x && ra.min.x < line.rect.max.x &&
           line.rect.min.y.partial_cmp(&ra.min.y) == target_ordering &&
           (rect_b.is_none() || rect_b.unwrap().min.y.partial_cmp(&line.rect.min.y) == target_ordering) {
            rect_b = Some(line.rect);
        }
    }

    if let Some(rb) = rect_b {
        let sum = if dir == LinearDir::Backward {
            rb.max.y + ra.min.y
        } else {
            ra.max.y + rb.min.y
        };

        Some((scale * sum / 2.0) as i32)
    } else {
        if dir == LinearDir::Backward {
            Some((scale * ra.min.y).floor() as i32 - 1)
        } else {
            Some((scale * ra.max.y).ceil() as i32 + 1)
        }
    }
}

impl Reader {
    pub fn new(rect: Rectangle, mut info: Info, hub: &Hub, context: &mut Context) -> Option<Reader> {
        let settings = &context.settings;
        let path = context.library.home.join(&info.file.path);

        open(&path).and_then(|mut doc| {
            let (width, height) = context.display.dims;
            let font_size = info.reader.as_ref().and_then(|r| r.font_size)
                                .unwrap_or(settings.reader.font_size);
            let first_location = doc.resolve_location(Location::Exact(0))?;

            doc.layout(width, height, font_size, CURRENT_DEVICE.dpi);

            let margin_width = info.reader.as_ref().and_then(|r| r.margin_width)
                                   .unwrap_or(settings.reader.margin_width);

            if margin_width != DEFAULT_MARGIN_WIDTH {
                doc.set_margin_width(margin_width);
            }

            let font_family = info.reader.as_ref().and_then(|r| r.font_family.as_ref())
                                  .unwrap_or(&settings.reader.font_family);

            if font_family != DEFAULT_FONT_FAMILY {
                doc.set_font_family(font_family, &settings.reader.font_path);
            }

            let line_height = info.reader.as_ref().and_then(|r| r.line_height)
                                  .unwrap_or(settings.reader.line_height);

            if (line_height - DEFAULT_LINE_HEIGHT).abs() > f32::EPSILON {
                doc.set_line_height(line_height);
            }

            let text_align = info.reader.as_ref().and_then(|r| r.text_align)
                                 .unwrap_or(settings.reader.text_align);

            if text_align != DEFAULT_TEXT_ALIGN {
                doc.set_text_align(text_align);
            }

            let mut view_port = ViewPort::default();
            let mut contrast = Contrast::default();
            let pages_count = doc.pages_count();
            let current_page;

            // TODO: use get_or_insert_with?
            if let Some(ref mut r) = info.reader {
                r.opened = Local::now();

                if r.finished {
                    r.finished = false;
                    r.current_page = first_location;
                }

                current_page = doc.resolve_location(Location::Exact(r.current_page))
                                  .unwrap_or(first_location);

                if let Some(zoom_mode) = r.zoom_mode {
                    view_port.zoom_mode = zoom_mode;
                }

                if let Some(top_offset) = r.top_offset {
                    view_port.top_offset = top_offset;
                }

                if !doc.is_reflowable() {
                    view_port.margin_width = mm_to_px(r.screen_margin_width.unwrap_or(0) as f32,
                                                      CURRENT_DEVICE.dpi) as i32;
                }

                if let Some(exponent) = r.contrast_exponent {
                    contrast.exponent = exponent;
                }

                if let Some(gray) = r.contrast_gray {
                    contrast.gray = gray;
                }
            } else {
                current_page = first_location;

                info.reader = Some(ReaderInfo {
                    current_page,
                    pages_count,
                    .. Default::default()
                });
            }

            let synthetic = doc.has_synthetic_page_numbers();
            let reflowable = doc.is_reflowable();

            println!("{}", info.file.path.display());

            hub.send(Event::Update(UpdateMode::Partial)).ok();

            Some(Reader {
                rect,
                children: Vec::new(),
                doc: Arc::new(Mutex::new(doc)),
                cache: BTreeMap::new(),
                text: HashMap::new(),
                annotations: HashMap::new(),
                chunks: Vec::new(),
                focus: None,
                search: None,
                search_direction: LinearDir::Forward,
                held_buttons: HashSet::new(),
                selection: None,
                target_annotation: None,
                history: VecDeque::new(),
                state: State::Idle,
                info,
                current_page,
                pages_count,
                view_port,
                synthetic,
                page_turns: 0,
                contrast,
                ephemeral: false,
                reflowable,
                finished: false,
            })
        })
    }

    pub fn from_toc(rect: Rectangle, toc: &[TocEntry], chap_index: usize, hub: &Hub, context: &mut Context) -> Reader {
        let html = toc_as_html(toc, chap_index);

        let info = Info {
            title: "Table of Contents".to_string(),
            file: FileInfo {
                path: PathBuf::from("toc:"),
                kind: "html".to_string(),
                size: html.len() as u64,
            },
            .. Default::default()
        };

        let mut opener = PdfOpener::new().unwrap();
        opener.set_user_css("css/toc.css").unwrap();
        let mut doc = opener.open_memory("html", html.as_bytes()).unwrap();
        let (width, height) = context.display.dims;
        let font_size = context.settings.reader.font_size;
        doc.layout(width, height, font_size, CURRENT_DEVICE.dpi);
        let pages_count = doc.pages_count();

        let mut current_page = 0;

        if let Some(chap) = chapter_from_index(chap_index, toc) {
            let link_uri = match chap.location {
                Location::Uri(ref uri) => format!("@{}", uri),
                Location::Exact(offset) => format!("@{}", offset),
                _ => "#".to_string(),
            };
            let mut loc = Location::Exact(0);
            while let Some((links, offset)) = doc.links(loc) {
                if links.iter().any(|link| link.text == link_uri) {
                    current_page = offset;
                    break;
                }
                loc = Location::Next(offset);
            }
        }

        hub.send(Event::Update(UpdateMode::Partial)).ok();

        Reader {
            rect,
            children: vec![],
            doc: Arc::new(Mutex::new(Box::new(doc))),
            cache: BTreeMap::new(),
            text: HashMap::new(),
            annotations: HashMap::new(),
            chunks: Vec::new(),
            focus: None,
            search: None,
            search_direction: LinearDir::Forward,
            held_buttons: HashSet::new(),
            selection: None,
            target_annotation: None,
            history: VecDeque::new(),
            state: State::Idle,
            info,
            current_page,
            pages_count,
            view_port: ViewPort::default(),
            synthetic: false,
            page_turns: 0,
            contrast: Contrast::default(),
            ephemeral: true,
            reflowable: true,
            finished: false,
        }
    }

    fn load_pixmap(&mut self, location: usize) {
        if self.cache.contains_key(&location) {
            return;
        }

        let mut doc = self.doc.lock().unwrap();
        let cropping_margin = self.info.reader.as_ref()
                                  .and_then(|r| r.cropping_margins.as_ref()
                                                 .map(|c| c.margin(location)))
                                  .cloned().unwrap_or_default();
        let dims = doc.dims(location).unwrap();
        let screen_margin_width = self.view_port.margin_width;
        let scale = scaling_factor(&self.rect, &cropping_margin, screen_margin_width, dims, self.view_port.zoom_mode);
        if let Some((pixmap, _)) = doc.pixmap(Location::Exact(location), scale) {
            let frame = rect![(cropping_margin.left * pixmap.width as f32).ceil() as i32,
                              (cropping_margin.top * pixmap.height as f32).ceil() as i32,
                              ((1.0 - cropping_margin.right) * pixmap.width as f32).floor() as i32,
                              ((1.0 - cropping_margin.bottom) * pixmap.height as f32).floor() as i32];
            self.cache.insert(location, Resource { pixmap, frame, scale });
        }
    }

    fn load_text(&mut self, location: usize) {
        if self.text.contains_key(&location) {
            return;
        }

        let mut doc = self.doc.lock().unwrap();
        let loc = Location::Exact(location);
        let words = doc.words(loc)
                       .map(|(words, _)| words)
                       .unwrap_or_default();
        self.text.insert(location, words);
    }

    fn go_to_page(&mut self, location: usize, record: bool, hub: &Hub, context: &Context) {
        let loc = {
            let mut doc = self.doc.lock().unwrap();
            doc.resolve_location(Location::Exact(location))
        };

        if let Some(location) = loc {
            if record {
                self.history.push_back(self.current_page);
                if self.history.len() > HISTORY_SIZE {
                    self.history.pop_front();
                }
            }

            if let Some(ref mut s) = self.search {
                s.current_page = s.highlights.range(..=location).count().saturating_sub(1);
            }

            self.view_port.top_offset = 0;
            self.current_page = location;
            self.update(None, hub, context);
            self.update_bottom_bar(hub);

            if self.search.is_some() {
                self.update_results_bar(hub);
            }
        }
    }

    fn go_to_chapter(&mut self, dir: CycleDir, hub: &Hub, context: &Context) {
        let current_page = self.current_page;
        let loc = {
            let mut doc = self.doc.lock().unwrap();
            if let Some(toc) = self.toc()
                                   .or_else(|| doc.toc()) {
                let chap_offset = if dir == CycleDir::Previous {
                   doc.chapter(current_page, &toc)
                      .and_then(|chap| doc.resolve_location(chap.location.clone()))
                      .and_then(|chap_offset| if chap_offset < current_page { Some(chap_offset) } else { None })
                } else {
                    None
                };
                chap_offset.or_else(||
                    doc.chapter_relative(current_page, dir, &toc)
                       .and_then(|rel_chap| doc.resolve_location(rel_chap.location.clone())))
            } else {
                None
            }
        };
        if let Some(location) = loc {
            self.go_to_page(location, true, hub, context);
        }
    }

    fn text_location_range(&self) -> Option<[TextLocation; 2]> {
        let mut min_loc = None;
        let mut max_loc = None;
        for chunk in &self.chunks {
            for word in &self.text[&chunk.location] {
                let rect = (word.rect * chunk.scale).to_rect();
                if rect.overlaps(&chunk.frame) {
                    if let Some(ref mut min) = min_loc {
                        if word.location < *min {
                            *min = word.location;
                        }
                    } else {
                        min_loc = Some(word.location);
                    }
                    if let Some(ref mut max) = max_loc {
                        if word.location > *max {
                            *max = word.location;
                        }
                    } else {
                        max_loc = Some(word.location);
                    }
                }
            }
        }

        min_loc.and_then(|min| max_loc.map(|max| [min, max]))
    }

    fn go_to_artefact(&mut self, dir: CycleDir, hub: &Hub, context: &Context) {
        let mut loc_bkm = None;
        let mut loc_annot = None;

        if let Some(ref r) = self.info.reader {
            match dir {
                CycleDir::Next => {
                    loc_bkm = r.bookmarks.range(self.current_page+1 ..)
                                         .next().cloned();
                    if let Some([_, max]) = self.text_location_range() {
                        loc_annot = r.annotations.iter()
                                     .filter(|annot| annot.selection[0] > max)
                                     .map(|annot| annot.selection[0]).min()
                                     .map(|tl| tl.location());
                    }
                },
                CycleDir::Previous => {
                    loc_bkm = r.bookmarks.range(.. self.current_page)
                                         .next_back().cloned();
                    if let Some([min, _]) = self.text_location_range() {
                        loc_annot = r.annotations.iter()
                                     .filter(|annot| annot.selection[1] < min)
                                     .map(|annot| annot.selection[1]).max()
                                     .map(|tl| tl.location());
                    }
                },
            }
        }

        let loc = match (loc_bkm, loc_annot) {
            (Some(a), Some(b)) => {
                if dir == CycleDir::Next {
                    Some(a.min(b))
                } else {
                    Some(a.max(b))
                }
            },
            (Some(a), None) => Some(a),
            (None, Some(b)) => Some(b),
            (None, None) => None,
        };

        if let Some(location) = loc {
            self.go_to_page(location, true, hub, context);
        }
    }

    fn go_to_last_page(&mut self, hub: &Hub, context: &Context) {
        if let Some(location) = self.history.pop_back() {
            self.go_to_page(location, false, hub, context);
        }
    }

    fn page_scroll(&mut self, delta_y: i32, hub: &Hub, context: &mut Context) {
        if delta_y == 0 {
            return;
        }

        let mut next_top_offset = self.view_port.top_offset - delta_y;
        let mut location = self.current_page;
        let max_top_offset = self.cache[&location].frame.height().saturating_sub(1) as i32;
        if next_top_offset < 0 {
            let mut doc = self.doc.lock().unwrap();
            if let Some(previous_location) = doc.resolve_location(Location::Previous(location)) {
                location = previous_location;
                let frame = self.cache[&location].frame;
                next_top_offset = (frame.height() as i32 + next_top_offset).max(0);
            } else {
                next_top_offset = 0;
            }
        } else if next_top_offset > max_top_offset {
            let mut doc = self.doc.lock().unwrap();
            if let Some(next_location) = doc.resolve_location(Location::Next(location)) {
                location = next_location;
                let frame = self.cache[&location].frame;
                let mto = frame.height().saturating_sub(1) as i32;
                next_top_offset = (next_top_offset - max_top_offset - 1).min(mto);
            } else {
                next_top_offset = max_top_offset;
            }
        }

        {
            let Resource { frame, scale, .. } = *self.cache.get(&location).unwrap();
            let mut doc = self.doc.lock().unwrap();
            if let Some((lines, _)) = doc.lines(Location::Exact(location)) {
                if let Some(mut y_pos) = find_cut(&frame, frame.min.y + next_top_offset,
                                                  scale, LinearDir::Forward, &lines) {
                    y_pos = y_pos.max(frame.min.y).min(frame.max.y - 1);
                    next_top_offset = y_pos - frame.min.y;
                }
            }
        }

        let location_changed = location != self.current_page;
        if !location_changed && next_top_offset == self.view_port.top_offset {
            return;
        }

        self.view_port.top_offset = next_top_offset;
        self.current_page = location;
        self.update(None, hub, context);

        if location_changed {
            if let Some(ref mut s) = self.search {
                s.current_page = s.highlights.range(..=location).count().saturating_sub(1);
            }
            self.update_bottom_bar(hub);
            if self.search.is_some() {
                self.update_results_bar(hub);
            }
        }
    }

    fn go_to_neighbor(&mut self, dir: CycleDir, hub: &Hub, context: &mut Context) {
        let current_page = self.current_page;
        let top_offset = self.view_port.top_offset;

        let loc = {
            let neighloc = if dir == CycleDir::Previous {
                match self.view_port.zoom_mode {
                    ZoomMode::FitToPage => Location::Previous(current_page),
                    ZoomMode::FitToWidth => {
                        let first_chunk = self.chunks.first().cloned().unwrap();
                        let mut location = first_chunk.location;
                        let available_height = self.rect.height() as i32 - 2 * self.view_port.margin_width;
                        let mut height = 0;

                        loop {
                            self.load_pixmap(location);
                            self.load_text(location);
                            let Resource { mut frame, .. } = self.cache[&location];
                            if location == first_chunk.location {
                                frame.max.y = first_chunk.frame.min.y;
                            }
                            height += frame.height() as i32;
                            if height >= available_height {
                                break;
                            }
                            let mut doc = self.doc.lock().unwrap();
                            if let Some(previous_location) = doc.resolve_location(Location::Previous(location)) {
                                location = previous_location;
                            } else {
                                break;
                            }
                        }

                        let mut next_top_offset = (height - available_height).max(0);
                        if height > available_height {
                            let Resource { frame, scale, .. } = self.cache[&location];
                            let mut doc = self.doc.lock().unwrap();
                            if let Some((lines, _)) = doc.lines(Location::Exact(location)) {
                                if let Some(mut y_pos) = find_cut(&frame, frame.min.y + next_top_offset,
                                                                  scale, LinearDir::Forward, &lines) {
                                    y_pos = y_pos.max(frame.min.y).min(frame.max.y - 1);
                                    next_top_offset = y_pos - frame.min.y;
                                }
                            }
                        }

                        self.view_port.top_offset = next_top_offset;
                        Location::Exact(location)
                    },
                }
            } else {
                match self.view_port.zoom_mode {
                    ZoomMode::FitToPage => Location::Next(current_page),
                    ZoomMode::FitToWidth => {
                        let last_chunk = self.chunks.last().unwrap();
                        let pixmap_frame = self.cache[&last_chunk.location].frame;
                        let next_top_offset = last_chunk.frame.max.y - pixmap_frame.min.y;
                        if next_top_offset == pixmap_frame.height() as i32 {
                            self.view_port.top_offset = 0;
                            Location::Next(last_chunk.location)
                        } else {
                            self.view_port.top_offset = next_top_offset;
                            Location::Exact(last_chunk.location)
                        }
                    },
                }
            };
            let mut doc = self.doc.lock().unwrap();
            doc.resolve_location(neighloc)
        };
        match loc {
            Some(location) if location != current_page || self.view_port.top_offset != top_offset => {
                if let Some(ref mut s) = self.search {
                    s.current_page = s.highlights.range(..=location).count().saturating_sub(1);
                }

                self.current_page = location;
                self.update(None, hub, context);
                self.update_bottom_bar(hub);

                if self.search.is_some() {
                    self.update_results_bar(hub);
                }
            },
            _ => {
                match dir {
                    CycleDir::Next => {
                        self.finished = true;
                        let action = if self.ephemeral {
                            FinishedAction::Notify
                        } else {
                            context.settings.reader.finished
                        };
                        match action {
                            FinishedAction::Notify => {
                                let notif = Notification::new(ViewId::BoundaryNotif,
                                                              "No next page.".to_string(),
                                                              hub,
                                                              context);
                                self.children.push(Box::new(notif) as Box<dyn View>);
                            },
                            FinishedAction::Close => {
                                self.quit(context);
                                hub.send(Event::Back).ok();
                            },
                        }
                    },
                    CycleDir::Previous => {
                        let notif = Notification::new(ViewId::BoundaryNotif,
                                                      "No previous page.".to_string(),
                                                      hub,
                                                      context);
                        self.children.push(Box::new(notif) as Box<dyn View>);
                    },
                }
            },
        }
    }

    fn go_to_results_page(&mut self, index: usize, hub: &Hub, context: &Context) {
        let mut loc = None;
        if let Some(ref mut s) = self.search {
            if index < s.highlights.len() {
                s.current_page = index;
                loc = Some(*s.highlights.keys().nth(index).unwrap());
            }
        }
        if let Some(location) = loc {
            self.view_port.top_offset = 0;
            self.current_page = location;
            self.update_results_bar(hub);
            self.update_bottom_bar(hub);
            self.update(None, hub, context);
        }
    }

    fn go_to_results_neighbor(&mut self, dir: CycleDir, hub: &Hub, context: &Context) {
        let loc = self.search.as_ref().and_then(|s| {
            match dir {
                CycleDir::Next => s.highlights.range(self.current_page+1..)
                                              .next().map(|e| *e.0),
                CycleDir::Previous => s.highlights.range(..self.current_page)
                                                  .next_back().map(|e| *e.0),
            }
        });
        if let Some(location) = loc {
            if let Some(ref mut s) = self.search {
                s.current_page = s.highlights.range(..=location).count().saturating_sub(1);
            }
            self.view_port.top_offset = 0;
            self.current_page = location;
            self.update_results_bar(hub);
            self.update_bottom_bar(hub);
            self.update(None, hub, context);
        }
    }

    fn update_bottom_bar(&mut self, hub: &Hub) {
        if let Some(index) = locate::<BottomBar>(self) {
            let current_page = self.current_page;
            let mut doc = self.doc.lock().unwrap();
            let chapter = self.toc().or_else(|| doc.toc())
                              .as_ref().and_then(|toc| doc.chapter(current_page, toc))
                              .map(|c| c.title.clone())
                              .unwrap_or_default();
            let bottom_bar = self.children[index].as_mut().downcast_mut::<BottomBar>().unwrap();
            let neighbors = Neighbors {
                previous_page: doc.resolve_location(Location::Previous(current_page)),
                next_page: doc.resolve_location(Location::Next(current_page)),
            };
            bottom_bar.update_page_label(self.current_page, self.pages_count, hub);
            bottom_bar.update_icons(&neighbors, hub);
            bottom_bar.update_chapter(&chapter, hub);
        }
    }

    fn update_tool_bar(&mut self, hub: &Hub, context: &mut Context) {
        if let Some(index) = locate::<ToolBar>(self) {
            let tool_bar = self.children[index].as_mut().downcast_mut::<ToolBar>().unwrap();
            let settings = &context.settings;
            if self.reflowable {
                let font_family = self.info.reader.as_ref()
                                      .and_then(|r| r.font_family.clone())
                                      .unwrap_or_else(|| settings.reader.font_family.clone());
                tool_bar.update_font_family(font_family, hub);
                let font_size = self.info.reader.as_ref()
                                    .and_then(|r| r.font_size)
                                    .unwrap_or(settings.reader.font_size);
                tool_bar.update_font_size_slider(font_size, hub);
                let text_align = self.info.reader.as_ref()
                                    .and_then(|r| r.text_align)
                                    .unwrap_or(settings.reader.text_align);
                tool_bar.update_text_align_icon(text_align, hub);
                let line_height = self.info.reader.as_ref()
                                      .and_then(|r| r.line_height)
                                      .unwrap_or(settings.reader.line_height);
                tool_bar.update_line_height(line_height, hub);
            } else {
                tool_bar.update_contrast_exponent_slider(self.contrast.exponent, hub);
                tool_bar.update_contrast_gray_slider(self.contrast.gray, hub);
            }
            let reflowable = self.reflowable;
            let margin_width = self.info.reader.as_ref()
                                   .and_then(|r| if reflowable { r.margin_width } else { r.screen_margin_width })
                                   .unwrap_or_else(|| if reflowable { settings.reader.margin_width } else { 0 });
            tool_bar.update_margin_width(margin_width, hub);
        }
    }

    fn update_results_bar(&mut self, hub: &Hub) {
        if self.search.is_none() {
            return;
        }
        let (count, current_page, pages_count) = {
            let s = self.search.as_ref().unwrap();
            (s.results_count, s.current_page, s.highlights.len())
        };
        if let Some(index) = locate::<ResultsBar>(self) {
            let results_bar = self.child_mut(index).downcast_mut::<ResultsBar>().unwrap();
            results_bar.update_results_label(count, hub);
            results_bar.update_page_label(current_page, pages_count, hub);
            results_bar.update_icons(current_page, pages_count, hub);
        }
    }

    #[inline]
    fn update_annotations(&mut self) {
        self.annotations.clear();
        if let Some(annotations) = self.info.reader.as_ref().map(|r| &r.annotations).filter(|a| !a.is_empty()) {
            for chunk in &self.chunks {
                let words = &self.text[&chunk.location];
                if words.is_empty() {
                    continue;
                }
                for annot in annotations {
                    let [start, end] = annot.selection;
                    if (start >= words[0].location && start <= words[words.len()-1].location) ||
                       (end >= words[0].location && end <= words[words.len()-1].location) {
                        self.annotations.entry(chunk.location)
                            .or_insert_with(|| Vec::new())
                            .push(annot.clone());
                    }
                }
            }
        }
    }

    fn update(&mut self, update_mode: Option<UpdateMode>, hub: &Hub, context: &Context) {
        self.page_turns += 1;
        let update_mode = update_mode.unwrap_or_else(|| {
            let refresh_rate = if context.fb.inverted() {
                context.settings.reader.refresh_rate.inverted
            } else {
                context.settings.reader.refresh_rate.regular
            };
            if refresh_rate == 0 || self.page_turns % (refresh_rate as usize) != 0 {
                UpdateMode::Partial
            } else {
                UpdateMode::Full
            }
        });

        self.chunks.clear();
        let mut location = self.current_page;
        let smw = self.view_port.margin_width;

        match self.view_port.zoom_mode {
            ZoomMode::FitToPage => {
                self.load_pixmap(location);
                self.load_text(location);
                let Resource { frame, scale, .. } = self.cache[&location];
                let dx = smw + ((self.rect.width() - frame.width()) as i32 - 2 * smw) / 2;
                let dy = smw + ((self.rect.height() - frame.height()) as i32 - 2 * smw) / 2;
                self.chunks.push(RenderChunk { frame, location, position: pt!(dx, dy), scale });
            },
            ZoomMode::FitToWidth => {
                let available_height = self.rect.height() as i32 - 2 * smw;
                let mut height = 0;
                while height < available_height {
                    self.load_pixmap(location);
                    self.load_text(location);
                    let Resource { mut frame, scale, .. } = self.cache[&location];
                    if location == self.current_page {
                        frame.min.y += self.view_port.top_offset;
                    }
                    let position = pt!(smw, smw + height);
                    self.chunks.push(RenderChunk { frame, location, position, scale });
                    height += frame.height() as i32;
                    if let Ok(mut doc) = self.doc.lock() {
                        if let Some(next_location) = doc.resolve_location(Location::Next(location)) {
                            location = next_location;
                        } else {
                            break;
                        }
                    }
                }
                if height > available_height {
                    if let Some(last_chunk) = self.chunks.last_mut() {
                        last_chunk.frame.max.y -= height - available_height;
                        let mut doc = self.doc.lock().unwrap();
                        if let Some((lines, _)) = doc.lines(Location::Exact(last_chunk.location)) {
                            let pixmap_frame = self.cache[&last_chunk.location].frame;
                            if let Some(mut y_pos) = find_cut(&pixmap_frame, last_chunk.frame.max.y, last_chunk.scale, LinearDir::Backward, &lines) {
                                y_pos = y_pos.max(pixmap_frame.min.y).min(pixmap_frame.max.y - 1);
                                last_chunk.frame.max.y = y_pos;
                            }
                        }
                    }
                    let actual_height: i32 = self.chunks.iter().map(|c| c.frame.height() as i32).sum();
                    let dy = (available_height - actual_height) / 2;
                    for chunk in &mut self.chunks {
                        chunk.position.y += dy;
                    }
                }
            },
        }

        hub.send(Event::Render(self.rect, update_mode)).ok();
        let first_location = self.chunks.first().map(|c| c.location).unwrap();
        let last_location = self.chunks.last().map(|c| c.location).unwrap();

        while self.cache.len() > 3 {
            let left_count = self.cache.range(..first_location).count();
            let right_count = self.cache.range(last_location+1..).count();
            let extremum = if left_count >= right_count {
                self.cache.keys().next().cloned().unwrap()
            } else {
                self.cache.keys().next_back().cloned().unwrap()
            };
            self.cache.remove(&extremum);
        }

        self.update_annotations();

        let doc2 = self.doc.clone();
        let hub2 = hub.clone();
        thread::spawn(move || {
            let mut doc = doc2.lock().unwrap();
            if let Some(next_location) = doc.resolve_location(Location::Next(last_location)) {
                hub2.send(Event::LoadPixmap(next_location)).ok();
            }
        });
        let doc3 = self.doc.clone();
        let hub3 = hub.clone();
        thread::spawn(move || {
            let mut doc = doc3.lock().unwrap();
            if let Some(previous_location) = doc.resolve_location(Location::Previous(first_location)) {
                hub3.send(Event::LoadPixmap(previous_location)).ok();
            }
        });
    }

    fn search(&mut self, text: &str, query: Regex, hub: &Hub) {
        let s = Search {
            query: text.to_string(),
            .. Default::default()
        };

        let hub2 = hub.clone();
        let doc2 = Arc::clone(&self.doc);
        let running = Arc::clone(&s.running);
        let current_page = self.current_page;
        let search_direction = self.search_direction;

        thread::spawn(move || {
            let mut loc = Location::Exact(current_page);
            let mut started = false;

            loop {
                if !running.load(AtomicOrdering::Relaxed) {
                    break;
                }

                let mut doc = doc2.lock().unwrap();
                let mut text = String::new();
                let mut rects = BTreeMap::new();

                if let Some(location) = doc.resolve_location(loc) {
                    if location == current_page && started {
                        break;
                    }
                    if let Some((ref words, _)) = doc.words(Location::Exact(location)) {
                        for word in words {
                            if !running.load(AtomicOrdering::Relaxed) {
                                break;
                            }
                            if text.ends_with('\u{00AD}') {
                                text.pop();
                            } else if !text.ends_with('-') && !text.is_empty() {
                                text.push(' ');
                            }
                            rects.insert(text.len(), word.rect);
                            text += &word.text;
                        }
                        for m in query.find_iter(&text) {
                            if let Some((first, _)) = rects.range(..= m.start()).next_back() {
                                let mut match_rects = Vec::new();
                                for (_, rect) in rects.range(*first .. m.end()) {
                                    match_rects.push(*rect);
                                }
                                hub2.send(Event::SearchResult(location, match_rects)).ok();
                            }
                        }
                    }
                    loc = match search_direction {
                        LinearDir::Forward => Location::Next(location),
                        LinearDir::Backward => Location::Previous(location),
                    };
                } else {
                    loc = match search_direction {
                        LinearDir::Forward => Location::Exact(0),
                        LinearDir::Backward => Location::Exact(doc.pages_count()-1),
                    };
                }

                started = true;
            }

            running.store(false, AtomicOrdering::Relaxed);
            hub2.send(Event::EndOfSearch).ok();
        });

        if self.search.is_some() {
            self.render_results(hub);
        }

        self.search = Some(s);
    }

    fn toggle_keyboard(&mut self, enable: bool, id: Option<ViewId>, hub: &Hub, context: &mut Context) {
        if let Some(index) = locate::<Keyboard>(self) {
            if enable {
                return;
            }

            let mut rect = *self.child(index).rect();
            rect.absorb(self.child(index-1).rect());

            if index == 1 {
                rect.absorb(self.child(index+1).rect());
                self.children.drain(index - 1 ..= index + 1);
                hub.send(Event::Expose(rect, UpdateMode::Gui)).ok();
            } else {
                self.children.drain(index - 1 ..= index);

                let start_index = locate::<TopBar>(self).map(|index| index+2).unwrap_or(0);
                let y_min = self.child(start_index).rect().min.y;
                let delta_y = rect.height() as i32;

                for i in start_index..index-1 {
                    let shifted_rect = *self.child(i).rect() + pt!(0, delta_y);
                    self.child_mut(i).resize(shifted_rect, hub, context);
                    hub.send(Event::Render(shifted_rect, UpdateMode::Gui)).ok();
                }

                let rect = rect![self.rect.min.x, y_min, self.rect.max.x, y_min + delta_y];
                hub.send(Event::Expose(rect, UpdateMode::Gui)).ok();
            }

            hub.send(Event::Focus(None)).ok();
        } else {
            if !enable {
                return;
            }

            let dpi = CURRENT_DEVICE.dpi;
            let (small_height, big_height) = (scale_by_dpi(SMALL_BAR_HEIGHT, dpi) as i32,
                                              scale_by_dpi(BIG_BAR_HEIGHT, dpi) as i32);
            let thickness = scale_by_dpi(THICKNESS_MEDIUM, dpi) as i32;
            let (small_thickness, big_thickness) = halves(thickness);

            let mut kb_rect = rect![self.rect.min.x,
                                    self.rect.max.y - (small_height + 3 * big_height) as i32 + big_thickness,
                                    self.rect.max.x,
                                    self.rect.max.y - small_height - small_thickness];

            let number = match id {
                Some(ViewId::GoToPageInput) |
                Some(ViewId::GoToResultsPageInput) |
                Some(ViewId::NamePageInput) => true,
                _ => false,
            };

            let index = rlocate::<Filler>(self).unwrap_or(0);

            if index == 0 {
                let separator = Filler::new(rect![self.rect.min.x, kb_rect.max.y,
                                                  self.rect.max.x, kb_rect.max.y + thickness],
                                            BLACK);
                self.children.insert(index, Box::new(separator) as Box<dyn View>);
            }

            let keyboard = Keyboard::new(&mut kb_rect, number, context);
            self.children.insert(index, Box::new(keyboard) as Box<dyn View>);

            let separator = Filler::new(rect![self.rect.min.x, kb_rect.min.y - thickness,
                                              self.rect.max.x, kb_rect.min.y],
                                        BLACK);
            self.children.insert(index, Box::new(separator) as Box<dyn View>);

            if index == 0 {
                for i in index..index+3 {
                    hub.send(Event::Render(*self.child(i).rect(), UpdateMode::Gui)).ok();
                }
            } else {
                for i in index..index+2 {
                    hub.send(Event::Render(*self.child(i).rect(), UpdateMode::Gui)).ok();
                }

                let delta_y = kb_rect.height() as i32 + thickness;
                let start_index = locate::<TopBar>(self).map(|index| index+2).unwrap_or(0);

                for i in start_index..index {
                    let shifted_rect = *self.child(i).rect() + pt!(0, -delta_y);
                    self.child_mut(i).resize(shifted_rect, hub, context);
                    hub.send(Event::Render(shifted_rect, UpdateMode::Gui)).ok();
                }
            }
        }
    }

    fn toggle_tool_bar(&mut self, enable: bool, hub: &Hub, context: &mut Context) {
        if let Some(index) = locate::<ToolBar>(self) {
            if enable {
                return;
            }

            let mut rect = *self.child(index).rect();
            rect.absorb(self.child(index - 1).rect());
            self.children.drain(index - 1 ..= index);
            hub.send(Event::Expose(rect, UpdateMode::Gui)).ok();
        } else {
            if !enable {
                return;
            }

            let dpi = CURRENT_DEVICE.dpi;
            let big_height = scale_by_dpi(BIG_BAR_HEIGHT, dpi) as i32;
            let tb_height = 2 * big_height;

            let sp_rect = *self.child(2).rect() - pt!(0, tb_height as i32);

            let tool_bar = ToolBar::new(rect![self.rect.min.x,
                                              sp_rect.max.y,
                                              self.rect.max.x,
                                              sp_rect.max.y + tb_height as i32],
                                        self.reflowable,
                                        self.info.reader.as_ref(),
                                        &context.settings.reader);
            self.children.insert(2, Box::new(tool_bar) as Box<dyn View>);

            let separator = Filler::new(sp_rect, BLACK);
            self.children.insert(2, Box::new(separator) as Box<dyn View>);
        }
    }

    fn toggle_results_bar(&mut self, enable: bool, hub: &Hub, _context: &mut Context) {
        if let Some(index) = locate::<ResultsBar>(self) {
            if enable {
                return;
            }

            let mut rect = *self.child(index).rect();
            rect.absorb(self.child(index - 1).rect());
            self.children.drain(index - 1 ..= index);
            hub.send(Event::Expose(rect, UpdateMode::Gui)).ok();
        } else {
            if !enable {
                return;
            }

            let dpi = CURRENT_DEVICE.dpi;
            let thickness = scale_by_dpi(THICKNESS_MEDIUM, dpi) as i32;
            let small_height = scale_by_dpi(SMALL_BAR_HEIGHT, dpi) as i32;
            let index = locate::<TopBar>(self).map(|index| index+2).unwrap_or(0);

            let sp_rect = *self.child(index).rect() - pt!(0, small_height);
            let y_min = sp_rect.max.y;
            let mut rect = rect![self.rect.min.x, y_min,
                                 self.rect.max.x, y_min + small_height - thickness];

            if let Some(ref s) = self.search {
                let results_bar = ResultsBar::new(rect, s.current_page,
                                                  s.highlights.len(), s.results_count,
                                                  !s.running.load(AtomicOrdering::Relaxed));
                self.children.insert(index, Box::new(results_bar) as Box<dyn View>);
                let separator = Filler::new(sp_rect, BLACK);
                self.children.insert(index, Box::new(separator) as Box<dyn View>);
                rect.absorb(&sp_rect);
                hub.send(Event::Render(rect, UpdateMode::Gui)).ok();
            }
        }
    }

    fn toggle_search_bar(&mut self, enable: bool, hub: &Hub, context: &mut Context) {
        if let Some(index) = locate::<SearchBar>(self) {
            if enable {
                return;
            }

            if let Some(ViewId::ReaderSearchInput) = self.focus {
                self.toggle_keyboard(false, None, hub, context);
            }

            if self.child(0).is::<TopBar>() {
                self.toggle_bars(Some(false), hub, context);
            } else {
                let mut rect = *self.child(index).rect();
                rect.absorb(self.child(index-1).rect());
                rect.absorb(self.child(index+1).rect());
                self.children.drain(index - 1 ..= index + 1);
                hub.send(Event::Expose(rect, UpdateMode::Gui)).ok();
            }
        } else {
            if !enable {
                return;
            }

            self.toggle_tool_bar(false, hub, context);

            let dpi = CURRENT_DEVICE.dpi;
            let thickness = scale_by_dpi(THICKNESS_MEDIUM, dpi) as i32;
            let (small_thickness, big_thickness) = halves(thickness);
            let small_height = scale_by_dpi(SMALL_BAR_HEIGHT, dpi) as i32;
            let index = locate::<TopBar>(self).map(|index| index+2).unwrap_or(0);

            if index == 0 {
                let sp_rect = rect![self.rect.min.x, self.rect.max.y - small_height - small_thickness,
                                    self.rect.max.x, self.rect.max.y - small_height + big_thickness];
                let separator = Filler::new(sp_rect, BLACK);
                self.children.insert(index, Box::new(separator) as Box<dyn View>);
            }

            let sp_rect = rect![self.rect.min.x, self.rect.max.y - 2 * small_height - small_thickness,
                                self.rect.max.x, self.rect.max.y - 2 * small_height + big_thickness];
            let y_min = sp_rect.max.y;
            let rect = rect![self.rect.min.x, y_min,
                             self.rect.max.x, y_min + small_height - thickness];
            let search_bar = SearchBar::new(rect, ViewId::ReaderSearchInput, "", "", context);
            self.children.insert(index, Box::new(search_bar) as Box<dyn View>);

            let separator = Filler::new(sp_rect, BLACK);
            self.children.insert(index, Box::new(separator) as Box<dyn View>);

            hub.send(Event::Render(*self.child(index).rect(), UpdateMode::Gui)).ok();
            hub.send(Event::Render(*self.child(index+1).rect(), UpdateMode::Gui)).ok();

            if index == 0 {
                hub.send(Event::Render(*self.child(index+2).rect(), UpdateMode::Gui)).ok();
            }

            hub.send(Event::Focus(Some(ViewId::ReaderSearchInput))).ok();
        }
    }

    fn toggle_bars(&mut self, enable: Option<bool>, hub: &Hub, context: &mut Context) {
        if let Some(top_index) = locate::<TopBar>(self) {
            if let Some(true) = enable {
                return;
            }

            if let Some(bottom_index) = locate::<BottomBar>(self) {
                let mut top_rect = *self.child(top_index).rect();
                top_rect.absorb(self.child(top_index+1).rect());
                let mut bottom_rect = *self.child(bottom_index).rect();
                for i in top_index+2 .. bottom_index {
                    bottom_rect.absorb(self.child(i).rect());
                }

                self.children.drain(top_index..=bottom_index);

                hub.send(Event::Expose(top_rect, UpdateMode::Gui)).ok();
                hub.send(Event::Expose(bottom_rect, UpdateMode::Gui)).ok();
                hub.send(Event::Focus(None)).ok();
            }
        } else {
            if let Some(false) = enable {
                return;
            }

            let dpi = CURRENT_DEVICE.dpi;
            let thickness = scale_by_dpi(THICKNESS_MEDIUM, dpi) as i32;
            let (small_thickness, big_thickness) = halves(thickness);
            let (small_height, big_height) = (scale_by_dpi(SMALL_BAR_HEIGHT, dpi) as i32,
                                              scale_by_dpi(BIG_BAR_HEIGHT, dpi) as i32);

            let mut doc = self.doc.lock().unwrap();
            let mut index = 0;

            let top_bar = TopBar::new(rect![self.rect.min.x,
                                            self.rect.min.y,
                                            self.rect.max.x,
                                            self.rect.min.y + small_height - small_thickness],
                                      Event::Back,
                                      self.info.title(),
                                      context);

            self.children.insert(index, Box::new(top_bar) as Box<dyn View>);
            index += 1;

            let separator = Filler::new(rect![self.rect.min.x,
                                              self.rect.min.y + small_height - small_thickness,
                                              self.rect.max.x,
                                              self.rect.min.y + small_height + big_thickness],
                                        BLACK);
            self.children.insert(index, Box::new(separator) as Box<dyn View>);
            index += 1;

            if let Some(ref s) = self.search {
                if let Some(sindex) = rlocate::<SearchBar>(self) {
                    index = sindex + 2;
                } else {
                    let separator = Filler::new(rect![self.rect.min.x,
                                                      self.rect.max.y - 3 * small_height - small_thickness,
                                                      self.rect.max.x,
                                                      self.rect.max.y - 3 * small_height + big_thickness],
                                                BLACK);
                    self.children.insert(index, Box::new(separator) as Box<dyn View>);
                    index += 1;

                    let results_bar = ResultsBar::new(rect![self.rect.min.x,
                                                            self.rect.max.y - 3 * small_height + big_thickness,
                                                            self.rect.max.x,
                                                            self.rect.max.y - 2 * small_height - small_thickness],
                                                      s.current_page, s.highlights.len(),
                                                      s.results_count, !s.running.load(AtomicOrdering::Relaxed));
                    self.children.insert(index, Box::new(results_bar) as Box<dyn View>);
                    index += 1;

                    let separator = Filler::new(rect![self.rect.min.x,
                                                      self.rect.max.y - 2 * small_height - small_thickness,
                                                      self.rect.max.x,
                                                      self.rect.max.y - 2 * small_height + big_thickness],
                                                BLACK);
                    self.children.insert(index, Box::new(separator) as Box<dyn View>);
                    index += 1;

                    let search_bar = SearchBar::new(rect![self.rect.min.x,
                                                          self.rect.max.y - 2 * small_height + big_thickness,
                                                          self.rect.max.x,
                                                          self.rect.max.y - small_height - small_thickness],
                                                    ViewId::ReaderSearchInput,
                                                    "", &s.query, context);
                    self.children.insert(index, Box::new(search_bar) as Box<dyn View>);
                    index += 1;
                }
            } else {
                let tb_height = 2 * big_height;
                let separator = Filler::new(rect![self.rect.min.x,
                                                  self.rect.max.y - (small_height + tb_height) as i32 - small_thickness,
                                                  self.rect.max.x,
                                                  self.rect.max.y - (small_height + tb_height) as i32 + big_thickness],
                                            BLACK);
                self.children.insert(index, Box::new(separator) as Box<dyn View>);
                index += 1;

                let tool_bar = ToolBar::new(rect![self.rect.min.x,
                                                  self.rect.max.y - (small_height + tb_height) as i32 + big_thickness,
                                                  self.rect.max.x,
                                                  self.rect.max.y - small_height - small_thickness],
                                            self.reflowable,
                                            self.info.reader.as_ref(),
                                            &context.settings.reader);
                self.children.insert(index, Box::new(tool_bar) as Box<dyn View>);
                index += 1;
            }

            let separator = Filler::new(rect![self.rect.min.x,
                                              self.rect.max.y - small_height - small_thickness,
                                              self.rect.max.x,
                                              self.rect.max.y - small_height + big_thickness],
                                        BLACK);
            self.children.insert(index, Box::new(separator) as Box<dyn View>);
            index += 1;

            let neighbors = Neighbors {
                previous_page: doc.resolve_location(Location::Previous(self.current_page)),
                next_page: doc.resolve_location(Location::Next(self.current_page)),
            };

            let bottom_bar = BottomBar::new(rect![self.rect.min.x,
                                                  self.rect.max.y - small_height + big_thickness,
                                                  self.rect.max.x,
                                                  self.rect.max.y],
                                            doc.as_mut(),
                                            self.toc(),
                                            self.current_page,
                                            self.pages_count,
                                            &neighbors,
                                            self.synthetic);
            self.children.insert(index, Box::new(bottom_bar) as Box<dyn View>);

            for i in 0..=index {
                hub.send(Event::Render(*self.child(i).rect(), UpdateMode::Gui)).ok();
            }
        }
    }

    fn toggle_margin_cropper(&mut self, enable: bool, hub: &Hub, context: &mut Context) {
        if let Some(index) = locate::<MarginCropper>(self) {
            if enable {
                return;
            }

            hub.send(Event::Expose(*self.child(index).rect(), UpdateMode::Gui)).ok();
            self.children.remove(index);
        } else {
            if !enable {
                return;
            }

            self.toggle_bars(Some(false), hub, context);

            let dpi = CURRENT_DEVICE.dpi;
            let padding = scale_by_dpi(BUTTON_DIAMETER / 2.0, dpi) as i32;
            let pixmap_rect = rect![self.rect.min + pt!(padding),
                                    self.rect.max - pt!(padding)];

            let margin = self.info.reader.as_ref()
                             .and_then(|r| r.cropping_margins.as_ref()
                                            .map(|c| c.margin(self.current_page)))
                             .cloned().unwrap_or_default();

            let mut doc = self.doc.lock().unwrap();
            let (pixmap, _) = build_pixmap(&pixmap_rect, doc.as_mut(), self.current_page);

            let margin_cropper = MarginCropper::new(self.rect, pixmap, &margin, context);
            hub.send(Event::Render(*margin_cropper.rect(), UpdateMode::Gui)).ok();
            self.children.push(Box::new(margin_cropper) as Box<dyn View>);
        }
    }

    fn toggle_edit_note(&mut self, text: Option<String>, enable: Option<bool>, hub: &Hub, context: &mut Context) {
        if let Some(index) = locate_by_id(self, ViewId::EditNote) {
            if let Some(true) = enable {
                return;
            }

            hub.send(Event::Expose(*self.child(index).rect(), UpdateMode::Gui)).ok();
            self.children.remove(index);

            if self.focus.map(|focus_id| focus_id == ViewId::EditNoteInput).unwrap_or(false) {
                self.toggle_keyboard(false, None, hub, context);
            }
        } else {
            if let Some(false) = enable {
                return;
            }

            let mut edit_note = NamedInput::new("Note".to_string(), ViewId::EditNote, ViewId::EditNoteInput, 32, context);
            if let Some(text) = text.as_ref() {
                let (tx, _rx) = mpsc::channel();
                edit_note.set_text(text, &tx, context);
            }

            hub.send(Event::Render(*edit_note.rect(), UpdateMode::Gui)).ok();
            hub.send(Event::Focus(Some(ViewId::EditNoteInput))).ok();

            self.children.push(Box::new(edit_note) as Box<dyn View>);
        }
    }

    fn toggle_name_page(&mut self, enable: Option<bool>, hub: &Hub, context: &mut Context) {
        if let Some(index) = locate_by_id(self, ViewId::NamePage) {
            if let Some(true) = enable {
                return;
            }

            hub.send(Event::Expose(*self.child(index).rect(), UpdateMode::Gui)).ok();
            self.children.remove(index);

            if self.focus.map(|focus_id| focus_id == ViewId::NamePageInput).unwrap_or(false) {
                self.toggle_keyboard(false, None, hub, context);
            }
        } else {
            if let Some(false) = enable {
                return;
            }

            let name_page = NamedInput::new("Name page".to_string(), ViewId::NamePage, ViewId::NamePageInput, 4, context);
            hub.send(Event::Render(*name_page.rect(), UpdateMode::Gui)).ok();
            hub.send(Event::Focus(Some(ViewId::NamePageInput))).ok();

            self.children.push(Box::new(name_page) as Box<dyn View>);
        }
    }

    fn toggle_go_to_page(&mut self, enable: Option<bool>, id: ViewId, hub: &Hub, context: &mut Context) {
        let (text, input_id) = if id == ViewId::GoToPage {
            ("Go to page", ViewId::GoToPageInput)
        } else {
            ("Go to results page", ViewId::GoToResultsPageInput)
        };

        if let Some(index) = locate_by_id(self, id) {
            if let Some(true) = enable {
                return;
            }

            hub.send(Event::Expose(*self.child(index).rect(), UpdateMode::Gui)).ok();
            self.children.remove(index);

            if self.focus.map(|focus_id| focus_id == input_id).unwrap_or(false) {
                self.toggle_keyboard(false, None, hub, context);
            }
        } else {
            if let Some(false) = enable {
                return;
            }

            let go_to_page = NamedInput::new(text.to_string(), id, input_id, 4, context);
            hub.send(Event::Render(*go_to_page.rect(), UpdateMode::Gui)).ok();
            hub.send(Event::Focus(Some(input_id))).ok();

            self.children.push(Box::new(go_to_page) as Box<dyn View>);
        }
    }

    pub fn toggle_annotation_menu(&mut self, annot: &Annotation, rect: Rectangle, enable: Option<bool>, hub: &Hub, context: &mut Context) {
        if let Some(index) = locate_by_id(self, ViewId::AnnotationMenu) {
            if let Some(true) = enable {
                return;
            }

            hub.send(Event::Expose(*self.child(index).rect(), UpdateMode::Gui)).ok();
            self.children.remove(index);
        } else {
            if let Some(false) = enable {
                return;
            }

            let sel = annot.selection;
            let mut entries = Vec::new();

            if annot.note.is_empty() {
                entries.push(EntryKind::Command("Remove Highlight".to_string(), EntryId::RemoveAnnotation(sel)));
                entries.push(EntryKind::Separator);
                entries.push(EntryKind::Command("Add Note".to_string(), EntryId::EditAnnotationNote(sel)));
            } else {
                entries.push(EntryKind::Command("Remove Annotation".to_string(), EntryId::RemoveAnnotation(sel)));
                entries.push(EntryKind::Separator);
                entries.push(EntryKind::Command("Edit Note".to_string(), EntryId::EditAnnotationNote(sel)));
                entries.push(EntryKind::Command("Remove Note".to_string(), EntryId::RemoveAnnotationNote(sel)));
            }

            let selection_menu = Menu::new(rect, ViewId::AnnotationMenu, MenuKind::Contextual, entries, context);
            hub.send(Event::Render(*selection_menu.rect(), UpdateMode::Gui)).ok();
            self.children.push(Box::new(selection_menu) as Box<dyn View>);
        }
    }

    pub fn toggle_selection_menu(&mut self, rect: Rectangle, enable: Option<bool>, hub: &Hub, context: &mut Context) {
        if let Some(index) = locate_by_id(self, ViewId::SelectionMenu) {
            if let Some(true) = enable {
                return;
            }

            hub.send(Event::Expose(*self.child(index).rect(), UpdateMode::Gui)).ok();
            self.children.remove(index);
        } else {
            if let Some(false) = enable {
                return;
            }
            let mut entries = vec![
                EntryKind::Command("Highlight".to_string(), EntryId::HighlightSelection),
                EntryKind::Command("Add Note".to_string(), EntryId::AnnotateSelection)
            ];

            entries.push(EntryKind::Separator);
            entries.push(EntryKind::Command("Define".to_string(), EntryId::DefineSelection));
            entries.push(EntryKind::Command("Search".to_string(), EntryId::SearchForSelection));

            if self.info.reader.as_ref().map_or(false, |r| !r.page_names.is_empty()) {
                entries.push(EntryKind::Command("Go To".to_string(), EntryId::GoToSelectedPageName));
            }

            entries.push(EntryKind::Separator);
            entries.push(EntryKind::Command("Adjust Selection".to_string(), EntryId::AdjustSelection));

            let selection_menu = Menu::new(rect, ViewId::SelectionMenu, MenuKind::Contextual, entries, context);
            hub.send(Event::Render(*selection_menu.rect(), UpdateMode::Gui)).ok();
            self.children.push(Box::new(selection_menu) as Box<dyn View>);
        }
    }

    pub fn toggle_title_menu(&mut self, rect: Rectangle, enable: Option<bool>, hub: &Hub, context: &mut Context) {
        if let Some(index) = locate_by_id(self, ViewId::TitleMenu) {
            if let Some(true) = enable {
                return;
            }

            hub.send(Event::Expose(*self.child(index).rect(), UpdateMode::Gui)).ok();
            self.children.remove(index);
        } else {
            if let Some(false) = enable {
                return;
            }
            let mut entries = Vec::new();
            if !self.reflowable {
                let zoom_mode = self.view_port.zoom_mode;
                entries.push(EntryKind::SubMenu("Zoom Mode".to_string(), vec![
                                      EntryKind::RadioButton("Fit to Page".to_string(),
                                                             EntryId::SetZoomMode(ZoomMode::FitToPage),
                                                             zoom_mode == ZoomMode::FitToPage),
                                      EntryKind::RadioButton("Fit to Width".to_string(),
                                                             EntryId::SetZoomMode(ZoomMode::FitToWidth),
                                                             zoom_mode == ZoomMode::FitToWidth)]));
            }
            entries.push(EntryKind::Command("Metadata".to_string(),
                                            EntryId::OpenMetadata));
            let title_menu = Menu::new(rect, ViewId::TitleMenu, MenuKind::DropDown, entries, context);
            hub.send(Event::Render(*title_menu.rect(), UpdateMode::Gui)).ok();
            self.children.push(Box::new(title_menu) as Box<dyn View>);
        }
    }


    fn toggle_font_family_menu(&mut self, rect: Rectangle, enable: Option<bool>, hub: &Hub, context: &mut Context) {
        if let Some(index) = locate_by_id(self, ViewId::FontFamilyMenu) {
            if let Some(true) = enable {
                return;
            }

            hub.send(Event::Expose(*self.child(index).rect(), UpdateMode::Gui)).ok();
            self.children.remove(index);
        } else {
            if let Some(false) = enable {
                return;
            }

            let mut families = family_names(&context.settings.reader.font_path)
                                           .map_err(|e| eprintln!("Can't get family names: {}", e))
                                           .unwrap_or_default();
            let current_family = self.info.reader.as_ref()
                                     .and_then(|r| r.font_family.clone())
                                     .unwrap_or_else(|| context.settings.reader.font_family.clone());
            families.insert(DEFAULT_FONT_FAMILY.to_string());
            let entries = families.iter().map(|f| EntryKind::RadioButton(f.clone(),
                                                                         EntryId::SetFontFamily(f.clone()),
                                                                         *f == current_family)).collect();
            let font_family_menu = Menu::new(rect, ViewId::FontFamilyMenu, MenuKind::DropDown, entries, context);
            hub.send(Event::Render(*font_family_menu.rect(), UpdateMode::Gui)).ok();
            self.children.push(Box::new(font_family_menu) as Box<dyn View>);
        }
    }

    fn toggle_font_size_menu(&mut self, rect: Rectangle, enable: Option<bool>, hub: &Hub, context: &mut Context) {
        if let Some(index) = locate_by_id(self, ViewId::FontSizeMenu) {
            if let Some(true) = enable {
                return;
            }

            hub.send(Event::Expose(*self.child(index).rect(), UpdateMode::Gui)).ok();
            self.children.remove(index);
        } else {
            if let Some(false) = enable {
                return;
            }

            let font_size = self.info.reader.as_ref().and_then(|r| r.font_size)
                                .unwrap_or(context.settings.reader.font_size);
            let min_font_size = context.settings.reader.font_size / 2.0;
            let max_font_size = 3.0 * context.settings.reader.font_size / 2.0;
            let entries = (0..=20).filter_map(|v| {
                let fs = font_size - 1.0 + v as f32 / 10.0;
                if fs >= min_font_size && fs <= max_font_size {
                    Some(EntryKind::RadioButton(format!("{:.1}", fs),
                                                EntryId::SetFontSize(v),
                                                (fs - font_size).abs() < 0.05))
                } else {
                    None
                }
            }).collect();
            let font_size_menu = Menu::new(rect, ViewId::FontSizeMenu, MenuKind::Contextual, entries, context);
            hub.send(Event::Render(*font_size_menu.rect(), UpdateMode::Gui)).ok();
            self.children.push(Box::new(font_size_menu) as Box<dyn View>);
        }
    }

    fn toggle_text_align_menu(&mut self, rect: Rectangle, enable: Option<bool>, hub: &Hub, context: &mut Context) {
        if let Some(index) = locate_by_id(self, ViewId::TextAlignMenu) {
            if let Some(true) = enable {
                return;
            }

            hub.send(Event::Expose(*self.child(index).rect(), UpdateMode::Gui)).ok();
            self.children.remove(index);
        } else {
            if let Some(false) = enable {
                return;
            }

            let text_align = self.info.reader.as_ref().and_then(|r| r.text_align)
                                .unwrap_or(context.settings.reader.text_align);
            let choices = [TextAlign::Justify, TextAlign::Left, TextAlign::Right, TextAlign::Center];
            let entries = choices.iter().map(|v| {
                EntryKind::RadioButton(v.to_string(),
                                       EntryId::SetTextAlign(*v),
                                       text_align == *v)
            }).collect();
            let text_align_menu = Menu::new(rect, ViewId::TextAlignMenu, MenuKind::Contextual, entries, context);
            hub.send(Event::Render(*text_align_menu.rect(), UpdateMode::Gui)).ok();
            self.children.push(Box::new(text_align_menu) as Box<dyn View>);
        }
    }

    fn toggle_line_height_menu(&mut self, rect: Rectangle, enable: Option<bool>, hub: &Hub, context: &mut Context) {
        if let Some(index) = locate_by_id(self, ViewId::LineHeightMenu) {
            if let Some(true) = enable {
                return;
            }

            hub.send(Event::Expose(*self.child(index).rect(), UpdateMode::Gui)).ok();
            self.children.remove(index);
        } else {
            if let Some(false) = enable {
                return;
            }

            let line_height = self.info.reader.as_ref()
                                  .and_then(|r| r.line_height).unwrap_or(context.settings.reader.line_height);
            let entries = (0..=10).map(|x| {
                let lh = 1.0 + x as f32 / 10.0;
                EntryKind::RadioButton(format!("{:.1}", lh),
                                       EntryId::SetLineHeight(x),
                                       (lh - line_height).abs() < 0.05)
            }).collect();
            let line_height_menu = Menu::new(rect, ViewId::LineHeightMenu, MenuKind::DropDown, entries, context);
            hub.send(Event::Render(*line_height_menu.rect(), UpdateMode::Gui)).ok();
            self.children.push(Box::new(line_height_menu) as Box<dyn View>);
        }
    }

    fn toggle_contrast_exponent_menu(&mut self, rect: Rectangle, enable: Option<bool>, hub: &Hub, context: &mut Context) {
        if let Some(index) = locate_by_id(self, ViewId::ContrastExponentMenu) {
            if let Some(true) = enable {
                return;
            }

            hub.send(Event::Expose(*self.child(index).rect(), UpdateMode::Gui)).ok();
            self.children.remove(index);
        } else {
            if let Some(false) = enable {
                return;
            }

            let entries = (0..=8).map(|x| {
                let e = 1.0 + x as f32 / 2.0;
                EntryKind::RadioButton(format!("{:.1}", e),
                                       EntryId::SetContrastExponent(x),
                                       (e - self.contrast.exponent).abs() < f32::EPSILON)
            }).collect();
            let contrast_exponent_menu = Menu::new(rect, ViewId::ContrastExponentMenu, MenuKind::DropDown, entries, context);
            hub.send(Event::Render(*contrast_exponent_menu.rect(), UpdateMode::Gui)).ok();
            self.children.push(Box::new(contrast_exponent_menu) as Box<dyn View>);
        }
    }

    fn toggle_contrast_gray_menu(&mut self, rect: Rectangle, enable: Option<bool>, hub: &Hub, context: &mut Context) {
        if let Some(index) = locate_by_id(self, ViewId::ContrastGrayMenu) {
            if let Some(true) = enable {
                return;
            }

            hub.send(Event::Expose(*self.child(index).rect(), UpdateMode::Gui)).ok();
            self.children.remove(index);
        } else {
            if let Some(false) = enable {
                return;
            }

            let entries = (1..=6).map(|x| {
                let g = ((1 << 8) - (1 << (8 - x))) as f32;
                EntryKind::RadioButton(format!("{:.1}", g),
                                       EntryId::SetContrastGray(x),
                                       (g - self.contrast.gray).abs() < f32::EPSILON)
            }).collect();
            let contrast_gray_menu = Menu::new(rect, ViewId::ContrastGrayMenu, MenuKind::DropDown, entries, context);
            hub.send(Event::Render(*contrast_gray_menu.rect(), UpdateMode::Gui)).ok();
            self.children.push(Box::new(contrast_gray_menu) as Box<dyn View>);
        }
    }

    fn toggle_margin_width_menu(&mut self, rect: Rectangle, enable: Option<bool>, hub: &Hub, context: &mut Context) {
        if let Some(index) = locate_by_id(self, ViewId::MarginWidthMenu) {
            if let Some(true) = enable {
                return;
            }

            hub.send(Event::Expose(*self.child(index).rect(), UpdateMode::Gui)).ok();
            self.children.remove(index);
        } else {
            if let Some(false) = enable {
                return;
            }

            let reflowable = self.reflowable;
            let margin_width = self.info.reader.as_ref()
                                   .and_then(|r| if reflowable { r.margin_width } else { r.screen_margin_width })
                                   .unwrap_or_else(|| if reflowable { context.settings.reader.margin_width } else { 0 });
            let entries = (0..=10).map(|mw| EntryKind::RadioButton(format!("{}", mw),
                                                                  EntryId::SetMarginWidth(mw),
                                                                  mw == margin_width)).collect();
            let margin_width_menu = Menu::new(rect, ViewId::MarginWidthMenu, MenuKind::DropDown, entries, context);
            hub.send(Event::Render(*margin_width_menu.rect(), UpdateMode::Gui)).ok();
            self.children.push(Box::new(margin_width_menu) as Box<dyn View>);
        }
    }

    fn toggle_page_menu(&mut self, rect: Rectangle, enable: Option<bool>, hub: &Hub, context: &mut Context) {
        if let Some(index) = locate_by_id(self, ViewId::PageMenu) {
            if let Some(true) = enable {
                return;
            }

            hub.send(Event::Expose(*self.child(index).rect(), UpdateMode::Gui)).ok();
            self.children.remove(index);
        } else {
            if let Some(false) = enable {
                return;
            }

            let has_name = self.info.reader.as_ref()
                               .map_or(false, |r| r.page_names.contains_key(&self.current_page));

            let mut entries = vec![EntryKind::Command("Name".to_string(), EntryId::SetPageName)];
            if has_name {
                entries.push(EntryKind::Command("Remove Name".to_string(), EntryId::RemovePageName));
            }
            let names = self.info.reader.as_ref()
                            .map(|r| r.page_names.iter()
                                      .map(|(i, s)| EntryKind::Command(s.to_string(), EntryId::GoTo(*i)))
                                      .collect::<Vec<EntryKind>>())
                            .unwrap_or_default();
            if !names.is_empty() {
                entries.push(EntryKind::Separator);
                entries.push(EntryKind::SubMenu("Go To".to_string(), names));
            }

            let page_menu = Menu::new(rect, ViewId::PageMenu, MenuKind::DropDown, entries, context);
            hub.send(Event::Render(*page_menu.rect(), UpdateMode::Gui)).ok();
            self.children.push(Box::new(page_menu) as Box<dyn View>);
        }
    }

    fn toggle_margin_cropper_menu(&mut self, rect: Rectangle, enable: Option<bool>, hub: &Hub, context: &mut Context) {
        if let Some(index) = locate_by_id(self, ViewId::MarginCropperMenu) {
            if let Some(true) = enable {
                return;
            }

            hub.send(Event::Expose(*self.child(index).rect(), UpdateMode::Gui)).ok();
            self.children.remove(index);
        } else {
            if let Some(false) = enable {
                return;
            }

            let current_page = self.current_page;
            let is_split = self.info.reader.as_ref()
                               .and_then(|r| r.cropping_margins
                                              .as_ref().map(CroppingMargins::is_split));

            let mut entries = vec![EntryKind::RadioButton("Any".to_string(),
                                                          EntryId::ApplyCroppings(current_page, PageScheme::Any),
                                                          is_split.is_some() && !is_split.unwrap()),
                                   EntryKind::RadioButton("Even/Odd".to_string(),
                                                          EntryId::ApplyCroppings(current_page, PageScheme::EvenOdd),
                                                          is_split.is_some() && is_split.unwrap())];

            let is_applied = self.info.reader.as_ref()
                                 .map(|r| r.cropping_margins.is_some())
                                 .unwrap_or(false);
            if is_applied {
                entries.extend_from_slice(&[EntryKind::Separator,
                                            EntryKind::Command("Remove".to_string(), EntryId::RemoveCroppings)]);
            }

            let margin_cropper_menu = Menu::new(rect, ViewId::MarginCropperMenu, MenuKind::DropDown, entries, context);
            hub.send(Event::Render(*margin_cropper_menu.rect(), UpdateMode::Gui)).ok();
            self.children.push(Box::new(margin_cropper_menu) as Box<dyn View>);
        }
    }

    fn toggle_search_menu(&mut self, rect: Rectangle, enable: Option<bool>, hub: &Hub, context: &mut Context) {
        if let Some(index) = locate_by_id(self, ViewId::SearchMenu) {
            if let Some(true) = enable {
                return;
            }

            hub.send(Event::Expose(*self.child(index).rect(), UpdateMode::Gui)).ok();
            self.children.remove(index);
        } else {
            if let Some(false) = enable {
                return;
            }

            let entries = vec![EntryKind::RadioButton("Forward".to_string(),
                                                      EntryId::SearchDirection(LinearDir::Forward),
                                                      self.search_direction == LinearDir::Forward),
                               EntryKind::RadioButton("Backward".to_string(),
                                                      EntryId::SearchDirection(LinearDir::Backward),
                                                      self.search_direction == LinearDir::Backward)];

            let search_menu = Menu::new(rect, ViewId::SearchMenu, MenuKind::Contextual, entries, context);
            hub.send(Event::Render(*search_menu.rect(), UpdateMode::Gui)).ok();
            self.children.push(Box::new(search_menu) as Box<dyn View>);
        }
    }

    fn set_font_size(&mut self, font_size: f32, hub: &Hub, context: &mut Context) {
        if Arc::strong_count(&self.doc) > 1 {
            return;
        }

        if let Some(ref mut r) = self.info.reader {
            r.font_size = Some(font_size);
        }

        let (width, height) = context.display.dims;
        {
            let mut doc = self.doc.lock().unwrap();

            doc.layout(width, height, font_size, CURRENT_DEVICE.dpi);

            if self.synthetic {
                let current_page = self.current_page.min(doc.pages_count() - 1);
                if let Some(location) =  doc.resolve_location(Location::Exact(current_page)) {
                    self.current_page = location;
                }
            } else {
                let ratio = doc.pages_count() / self.pages_count;
                self.pages_count = doc.pages_count();
                self.current_page = (ratio * self.current_page).min(self.pages_count - 1);
            }
        }

        self.cache.clear();
        self.text.clear();
        self.update(None, hub, context);
        self.update_tool_bar(hub, context);
        self.update_bottom_bar(hub);
    }

    fn set_text_align(&mut self, text_align: TextAlign, hub: &Hub, context: &mut Context) {
        if Arc::strong_count(&self.doc) > 1 {
            return;
        }

        if let Some(ref mut r) = self.info.reader {
            r.text_align = Some(text_align);
        }

        {
            let mut doc = self.doc.lock().unwrap();
            doc.set_text_align(text_align);

            if self.synthetic {
                let current_page = self.current_page.min(doc.pages_count() - 1);
                if let Some(location) =  doc.resolve_location(Location::Exact(current_page)) {
                    self.current_page = location;
                }
            } else {
                self.pages_count = doc.pages_count();
                self.current_page = self.current_page.min(self.pages_count - 1);
            }
        }

        self.cache.clear();
        self.text.clear();
        self.update(None, hub, context);
        self.update_tool_bar(hub, context);
        self.update_bottom_bar(hub);
    }

    fn set_font_family(&mut self, font_family: &str, hub: &Hub, context: &mut Context) {
        if Arc::strong_count(&self.doc) > 1 {
            return;
        }

        if let Some(ref mut r) = self.info.reader {
            r.font_family = Some(font_family.to_string());
        }

        {
            let mut doc = self.doc.lock().unwrap();
            let font_path = if font_family == DEFAULT_FONT_FAMILY {
                "fonts"
            } else {
                &context.settings.reader.font_path
            };

            doc.set_font_family(font_family, font_path);

            if self.synthetic {
                let current_page = self.current_page.min(doc.pages_count() - 1);
                if let Some(location) =  doc.resolve_location(Location::Exact(current_page)) {
                    self.current_page = location;
                }
            } else {
                self.pages_count = doc.pages_count();
                self.current_page = self.current_page.min(self.pages_count - 1);
            }
        }

        self.cache.clear();
        self.text.clear();
        self.update(None, hub, context);
        self.update_tool_bar(hub, context);
        self.update_bottom_bar(hub);
    }

    fn set_line_height(&mut self, line_height: f32, hub: &Hub, context: &mut Context) {
        if Arc::strong_count(&self.doc) > 1 {
            return;
        }

        if let Some(ref mut r) = self.info.reader {
            r.line_height = Some(line_height);
        }

        {
            let mut doc = self.doc.lock().unwrap();
            doc.set_line_height(line_height);

            if self.synthetic {
                let current_page = self.current_page.min(doc.pages_count() - 1);
                if let Some(location) =  doc.resolve_location(Location::Exact(current_page)) {
                    self.current_page = location;
                }
            } else {
                self.pages_count = doc.pages_count();
                self.current_page = self.current_page.min(self.pages_count - 1);
            }
        }

        self.cache.clear();
        self.text.clear();
        self.update(None, hub, context);
        self.update_tool_bar(hub, context);
        self.update_bottom_bar(hub);
    }

    fn set_margin_width(&mut self, width: i32, hub: &Hub, context: &mut Context) {
        if Arc::strong_count(&self.doc) > 1 {
            return;
        }

        if let Some(ref mut r) = self.info.reader {
            if self.reflowable {
                r.margin_width = Some(width);
            } else {
                if width == 0 {
                    r.screen_margin_width = None;
                } else {
                    r.screen_margin_width = Some(width);
                }
            }
        }

        if self.reflowable {
            let mut doc = self.doc.lock().unwrap();
            doc.set_margin_width(width);

            if self.synthetic {
                let current_page = self.current_page.min(doc.pages_count() - 1);
                if let Some(location) =  doc.resolve_location(Location::Exact(current_page)) {
                    self.current_page = location;
                }
            } else {
                self.pages_count = doc.pages_count();
                self.current_page = self.current_page.min(self.pages_count - 1);
            }
        } else {
            let next_margin_width = mm_to_px(width as f32, CURRENT_DEVICE.dpi) as i32;
            let ratio = (self.rect.width() as i32 - 2 * next_margin_width) as f32 /
                        (self.rect.width() as i32 - 2 * self.view_port.margin_width) as f32;
            self.view_port.top_offset = (self.view_port.top_offset as f32 * ratio) as i32;
            self.view_port.margin_width = next_margin_width;
        }

        self.text.clear();
        self.cache.clear();
        self.update(None, hub, context);
        self.update_tool_bar(hub, context);
        self.update_bottom_bar(hub);
    }

    fn toggle_bookmark(&mut self, hub: &Hub) {
        if let Some(ref mut r) = self.info.reader {
            if !r.bookmarks.insert(self.current_page) {
                r.bookmarks.remove(&self.current_page);
            }
        }
        let dpi = CURRENT_DEVICE.dpi;
        let thickness = scale_by_dpi(3.0, dpi) as u16;
        let radius = mm_to_px(0.4, dpi) as i32 + thickness as i32;
        let center = pt!(self.rect.max.x - 5 * radius,
                         self.rect.min.y + 5 * radius);
        let rect = Rectangle::from_disk(center, radius);
        hub.send(Event::RenderRegion(rect, UpdateMode::Gui)).ok();
    }

    fn set_contrast_exponent(&mut self, exponent: f32, hub: &Hub, context: &mut Context) {
        if let Some(ref mut r) = self.info.reader {
            r.contrast_exponent = Some(exponent);
        }
        self.contrast.exponent = exponent;
        self.update(None, hub, context);
        self.update_tool_bar(hub, context);
    }

    fn set_contrast_gray(&mut self, gray: f32, hub: &Hub, context: &mut Context) {
        if let Some(ref mut r) = self.info.reader {
            r.contrast_gray = Some(gray);
        }
        self.contrast.gray = gray;
        self.update(None, hub, context);
        self.update_tool_bar(hub, context);
    }

    fn set_zoom_mode(&mut self, zoom_mode: ZoomMode, hub: &Hub, context: &Context) {
        if self.view_port.zoom_mode == zoom_mode {
            return;
        }
        self.view_port.zoom_mode = zoom_mode;
        self.view_port.top_offset = 0;
        self.cache.clear();
        self.update(None, hub, context);
    }

    fn crop_margins(&mut self, index: usize, margin: &Margin, hub: &Hub, context: &Context) {
        if self.view_port.zoom_mode == ZoomMode::FitToWidth {
            let Resource { pixmap, frame, .. } = self.cache.get(&index).unwrap();
            let ratio = (frame.min.y + self.view_port.top_offset) as f32 / pixmap.height as f32;
            if ratio >= margin.top && ratio <= (1.0 - margin.bottom) {
                let dims = {
                    let doc = self.doc.lock().unwrap();
                    doc.dims(index).unwrap()
                };
                let scale = scaling_factor(&self.rect, margin, self.view_port.margin_width, dims, ZoomMode::FitToWidth);
                self.view_port.top_offset = (scale * (ratio - margin.top) * dims.1) as i32;
            } else {
                self.view_port.top_offset = 0;
            }
        }
        if let Some(r) = self.info.reader.as_mut() {
            if r.cropping_margins.is_none() {
                r.cropping_margins = Some(CroppingMargins::Any(Margin::default()));
            }
            for c in r.cropping_margins.iter_mut() {
                *c.margin_mut(index) = margin.clone();
            }
        }
        self.cache.clear();
        self.update(None, hub, context);
    }

    fn toc(&self) -> Option<Vec<TocEntry>> {
        let mut index = 0;
        self.info.toc.as_ref()
            .map(|simple_toc| self.toc_aux(simple_toc, &mut index))
    }

    fn toc_aux(&self, simple_toc: &[SimpleTocEntry], index: &mut usize) -> Vec<TocEntry> {
        let mut toc = Vec::new();
        for entry in simple_toc {
            *index += 1;
            match entry {
                SimpleTocEntry::Leaf(title, location) | SimpleTocEntry::Container(title, location, _) => {
                    let current_title = title.clone();
                    let current_location = match location {
                        TocLocation::Uri(uri) if uri.starts_with('\'') => {
                            self.find_page_by_name(&uri[1..])
                                .map(Location::Exact)
                                .unwrap_or_else(|| location.clone().into())
                        },
                        _ => location.clone().into(),
                    };
                    let current_index = *index;
                    let current_children = if let SimpleTocEntry::Container(_, _, children) = entry {
                        self.toc_aux(children, index)
                    } else {
                        Vec::new()
                    };
                    toc.push(TocEntry {
                        title: current_title,
                        location: current_location,
                        index: current_index,
                        children: current_children,
                    });
                },
            }
        }
        toc
    }

    fn find_page_by_name(&self, name: &str) -> Option<usize> {
        self.info.reader.as_ref().and_then(|r| {
            if let Ok(a) = u32::from_str_radix(name, 10) {
                r.page_names
                 .iter().filter_map(|(i, s)| u32::from_str_radix(s, 10).ok().map(|b| (b, i)))
                 .filter(|(b, _)| *b <= a)
                 .max_by(|x, y| x.0.cmp(&y.0))
                 .map(|(b, i)| *i + (a - b) as usize)
            } else if let Some(a) = name.chars().next().and_then(|c| c.to_alphabetic_digit()) {
                r.page_names
                 .iter().filter_map(|(i, s)| s.chars().next()
                                              .and_then(|c| c.to_alphabetic_digit())
                                              .map(|c| (c, i)))
                 .filter(|(b, _)| *b <= a)
                 .max_by(|x, y| x.0.cmp(&y.0))
                 .map(|(b, i)| *i + (a - b) as usize)
            } else if let Ok(a) = Roman::from_str(name) {
                r.page_names
                 .iter().filter_map(|(i, s)| Roman::from_str(s).ok().map(|b| (*b, i)))
                 .filter(|(b, _)| *b <= *a)
                 .max_by(|x, y| x.0.cmp(&y.0))
                 .map(|(b, i)| *i + (*a - b) as usize)
            } else {
                None
            }
        })
    }

    fn text_excerpt(&self, sel: [TextLocation; 2]) -> Option<String> {
        let [start, end] = sel;
        let parts = self.text.values().flatten()
                        .filter(|bnd| bnd.location >= start && bnd.location <= end)
                        .map(|bnd| bnd.text.as_str()).collect::<Vec<&str>>();

        if parts.is_empty() {
            return None;
        }

        let mut text = parts[0].to_string();

        for p in &parts[1..] {
            if text.ends_with('\u{00AD}') {
                text.pop();
            } else if !text.ends_with('-') {
                text.push(' ');
            }
            text += p;
        }

        Some(text)
    }

    fn selected_text(&self) -> Option<String> {
        self.selection.as_ref().and_then(|sel| self.text_excerpt([sel.start, sel.end]))
    }

    fn text_rect(&self, sel: [TextLocation; 2]) -> Option<Rectangle> {
        let [start, end] = sel;
        let mut result: Option<Rectangle> = None;

        for chunk in &self.chunks {
            if let Some(words) = self.text.get(&chunk.location) {
                for word in words {
                    if word.location >= start && word.location <= end {
                        let rect = (word.rect * chunk.scale).to_rect() - chunk.frame.min + chunk.position;
                        if let Some(ref mut r) = result {
                            r.absorb(&rect);
                        } else {
                            result = Some(rect);
                        }
                    }
                }
            }
        }

        result
    }

    fn render_results(&self, hub: &Hub) {
        for chunk in &self.chunks {
            if let Some(groups) = self.search.as_ref().and_then(|s| s.highlights.get(&chunk.location)) {
                for rects in groups {
                    let mut rect_opt: Option<Rectangle> = None;
                    for rect in rects {
                        let rect = (*rect * chunk.scale).to_rect() - chunk.frame.min + chunk.position;
                        if let Some(ref mut r) = rect_opt {
                            r.absorb(&rect);
                        } else {
                            rect_opt = Some(rect);
                        }
                    }
                    if let Some(rect) = rect_opt {
                        hub.send(Event::RenderRegion(rect, UpdateMode::Gui)).ok();
                    }
                }
            }
        }
    }

    fn selection_rect(&self) -> Option<Rectangle> {
        self.selection.as_ref().and_then(|sel| self.text_rect([sel.start, sel.end]))
    }

    fn find_annotation_ref(&mut self, sel: [TextLocation; 2]) -> Option<&Annotation> {
        self.info.reader.as_ref()
            .and_then(|r| r.annotations.iter()
                           .find(|a| a.selection[0] == sel[0] && a.selection[1] == sel[1]))
    }

    fn find_annotation_mut(&mut self, sel: [TextLocation; 2]) -> Option<&mut Annotation> {
        self.info.reader.as_mut()
            .and_then(|r| r.annotations.iter_mut()
                           .find(|a| a.selection[0] == sel[0] && a.selection[1] == sel[1]))
    }

    fn reseed(&mut self, hub: &Hub, context: &mut Context) {
        let (tx, _rx) = mpsc::channel();
        if let Some(index) = locate::<TopBar>(self) {
            self.child_mut(index).downcast_mut::<TopBar>().unwrap()
                .update_frontlight_icon(&tx, context);
            hub.send(Event::ClockTick).ok();
            hub.send(Event::BatteryTick).ok();
        }
        hub.send(Event::Render(self.rect, UpdateMode::Gui)).ok();
    }

    fn quit(&mut self, context: &mut Context) {
        if let Some(ref mut s) = self.search {
            s.running.store(false, AtomicOrdering::Relaxed);
        }

        if self.ephemeral {
            return;
        }

        if let Some(ref mut r) = self.info.reader {
            r.current_page = self.current_page;
            r.pages_count = self.pages_count;
            r.finished = self.finished;

            if self.view_port.zoom_mode == ZoomMode::FitToPage {
                r.zoom_mode = None;
                r.top_offset = None;
            } else {
                r.zoom_mode = Some(self.view_port.zoom_mode);
                r.top_offset = Some(self.view_port.top_offset);
            }

            r.rotation = Some(context.display.rotation);

            if (self.contrast.exponent - DEFAULT_CONTRAST_EXPONENT).abs() > f32::EPSILON {
                r.contrast_exponent = Some(self.contrast.exponent);
                if (self.contrast.gray - DEFAULT_CONTRAST_GRAY).abs() > f32::EPSILON {
                    r.contrast_gray = Some(self.contrast.gray);
                } else {
                    r.contrast_gray = None;
                }
            } else {
                r.contrast_exponent = None;
                r.contrast_gray = None;
            }

            context.library.sync_reader_info(&self.info.file.path, r);
        }

        // for i in &mut context.metadata {
        //     if i.file.path == self.info.file.path {
        //         *i = self.info.clone();
        //         break;
        //     }
        // }
    }
}

impl View for Reader {
    fn handle_event(&mut self, evt: &Event, hub: &Hub, _bus: &mut Bus, context: &mut Context) -> bool {
        match *evt {
            Event::Gesture(GestureEvent::Rotate { quarter_turns, .. }) if quarter_turns != 0 => {
                let (_, dir) = CURRENT_DEVICE.mirroring_scheme();
                let n = (4 + (context.display.rotation - dir * quarter_turns)) % 4;
                hub.send(Event::Select(EntryId::Rotate(n))).ok();
                true
            },
            Event::Gesture(GestureEvent::Swipe { dir, start, end, .. }) if self.rect.includes(start) => {
                match dir {
                    Dir::West => self.go_to_neighbor(CycleDir::Next, hub, context),
                    Dir::East => self.go_to_neighbor(CycleDir::Previous, hub, context),
                    Dir::South | Dir::North => self.page_scroll(end.y - start.y, hub, context),
                };
                true
            },
            Event::Gesture(GestureEvent::Spread { axis: Axis::Horizontal, starts, .. }) if self.rect.includes(starts[0]) => {
                if !self.reflowable {
                    self.set_zoom_mode(ZoomMode::FitToWidth, hub, context);
                }
                true

            },
            Event::Gesture(GestureEvent::Pinch { axis: Axis::Horizontal, starts, .. }) if self.rect.includes(starts[0]) => {
                if !self.reflowable {
                    self.set_zoom_mode(ZoomMode::FitToPage, hub, context);
                }
                true
            },
            Event::Gesture(GestureEvent::Arrow { dir, .. }) => {
                match dir {
                    Dir::West => {
                        if self.search.is_none() {
                            self.go_to_chapter(CycleDir::Previous, hub, context);
                        } else {
                            self.go_to_results_page(0, hub, context);
                        }
                    },
                    Dir::East => {
                        if self.search.is_none() {
                            self.go_to_chapter(CycleDir::Next, hub, context);
                        } else {
                            let last_page = self.search.as_ref().unwrap().highlights.len() - 1;
                            self.go_to_results_page(last_page, hub, context);
                        }
                    },
                    Dir::North => {
                        self.search_direction = LinearDir::Backward;
                        self.toggle_search_bar(true, hub, context);
                    },
                    Dir::South => {
                        self.search_direction = LinearDir::Forward;
                        self.toggle_search_bar(true, hub, context);
                    },
                };
                true
            },
            Event::Gesture(GestureEvent::Corner { dir, .. }) => {
                match dir {
                    DiagDir::NorthWest => self.go_to_artefact(CycleDir::Previous, hub, context),
                    DiagDir::NorthEast => self.go_to_artefact(CycleDir::Next, hub, context),
                    DiagDir::SouthEast => {
                        hub.send(Event::Select(EntryId::ToggleMonochrome)).ok();
                    },
                    DiagDir::SouthWest => {
                        if context.settings.frontlight_presets.len() > 1 {
                            if context.settings.frontlight {
                                let lightsensor_level = if CURRENT_DEVICE.has_lightsensor() {
                                    context.lightsensor.level().ok()
                                } else {
                                    None
                                };
                                if let Some(ref frontlight_levels) = guess_frontlight(lightsensor_level, &context.settings.frontlight_presets) {
                                    let LightLevels { intensity, warmth } = *frontlight_levels;
                                    context.frontlight.set_intensity(intensity);
                                    context.frontlight.set_warmth(warmth);
                                }
                            }
                        } else {
                            hub.send(Event::ToggleFrontlight).ok();
                        }
                    },
                };
                true
            },
            Event::Gesture(GestureEvent::Cross(_)) => {
                self.quit(context);
                hub.send(Event::Back).ok();
                true
            },
            Event::Gesture(GestureEvent::HoldButtonShort(code, ..)) => {
                match code {
                    ButtonCode::Backward => self.go_to_chapter(CycleDir::Previous, hub, context),
                    ButtonCode::Forward => self.go_to_chapter(CycleDir::Next, hub, context),
                    _ => (),
                }
                self.held_buttons.insert(code);
                true
            },
            Event::Device(DeviceEvent::Button { code, status: ButtonStatus::Released, .. }) => {
                if !self.held_buttons.remove(&code) {
                    match code {
                        ButtonCode::Backward => {
                            if self.search.is_none() {
                                self.go_to_neighbor(CycleDir::Previous, hub, context);
                            } else {
                                self.go_to_results_neighbor(CycleDir::Previous, hub, context);
                            }
                        },
                        ButtonCode::Forward => {
                            if self.search.is_none() {
                                self.go_to_neighbor(CycleDir::Next, hub, context);
                            } else {
                                self.go_to_results_neighbor(CycleDir::Next, hub, context);
                            }
                        },
                        _ => (),
                    }
                }
                true
            },
            Event::Device(DeviceEvent::Finger { position, status: FingerStatus::Motion, id, .. }) if self.state == State::Selection(id) => {
                let mut nearest_word = None;
                let mut dmin = u32::MAX;
                let dmax = (scale_by_dpi(RECT_DIST_JITTER, CURRENT_DEVICE.dpi) as i32).pow(2) as u32;
                let mut rects = Vec::new();

                for chunk in &self.chunks {
                    for word in &self.text[&chunk.location] {
                        let rect = (word.rect * chunk.scale).to_rect() - chunk.frame.min + chunk.position;
                        rects.push((rect, word.location));
                        let d = position.rdist2(&rect);
                        if d < dmax && d < dmin {
                            dmin = d;
                            nearest_word = Some(word.clone());
                        }
                    }
                }

                let selection = self.selection.as_mut().unwrap();

                if let Some(word) = nearest_word {
                    let old_start = selection.start;
                    let old_end = selection.end;
                    let (start, end) = word.location.min_max(selection.anchor);

                    if start == old_start && end == old_end {
                        return true;
                    }

                    let (start_low, start_high) = old_start.min_max(start);
                    let (end_low, end_high) = old_end.min_max(end);

                    if start_low != start_high {
                        if let Some(mut i) = rects.iter().position(|(_, loc)| *loc == start_low) {
                            let mut rect = rects[i].0;
                            while rects[i].1 < start_high {
                                let next_rect = rects[i+1].0;
                                if rect.min.y < next_rect.max.y && next_rect.min.y < rect.max.y {
                                    if rects[i+1].1 == start_high {
                                        if rect.min.x < next_rect.min.x {
                                            rect.max.x = next_rect.min.x;
                                        } else {
                                            rect.min.x = next_rect.max.x;
                                        }
                                        rect.min.y = rect.min.y.min(next_rect.min.y);
                                        rect.max.y = rect.max.y.max(next_rect.max.y);
                                    } else {
                                        rect.absorb(&next_rect);
                                    }
                                } else {
                                    hub.send(Event::RenderRegion(rect, UpdateMode::Fast)).ok();
                                    rect = next_rect;
                                }
                                i += 1;
                            }
                            hub.send(Event::RenderRegion(rect, UpdateMode::Fast)).ok();
                        }
                    }

                    if end_low != end_high {
                        if let Some(mut i) = rects.iter().rposition(|(_, loc)| *loc == end_high) {
                            let mut rect = rects[i].0;
                            while rects[i].1 > end_low {
                                let prev_rect = rects[i-1].0;
                                if rect.min.y < prev_rect.max.y && prev_rect.min.y < rect.max.y {
                                    if rects[i-1].1 == end_low {
                                        if rect.min.x > prev_rect.min.x {
                                            rect.min.x = prev_rect.max.x;
                                        } else {
                                            rect.max.x = prev_rect.min.x;
                                        }
                                        rect.min.y = rect.min.y.min(prev_rect.min.y);
                                        rect.max.y = rect.max.y.max(prev_rect.max.y);
                                    } else {
                                        rect.absorb(&prev_rect);
                                    }
                                } else {
                                    hub.send(Event::RenderRegion(rect, UpdateMode::Fast)).ok();
                                    rect = prev_rect;
                                }
                                i -= 1;
                            }
                            hub.send(Event::RenderRegion(rect, UpdateMode::Fast)).ok();
                        }
                    }

                    selection.start = start;
                    selection.end = end;
                }
                true
            },
            Event::Device(DeviceEvent::Finger { status: FingerStatus::Up, position, id, .. }) if self.state == State::Selection(id) => {
                self.state = State::Idle;
                let radius = scale_by_dpi(24.0, CURRENT_DEVICE.dpi) as i32;
                self.toggle_selection_menu(Rectangle::from_disk(position, radius), Some(true), hub, context);
                true
            },
            Event::Gesture(GestureEvent::Tap(center)) if self.state == State::AdjustSelection && self.rect.includes(center) => {
                let mut found = None;
                let mut dmin = u32::MAX;
                let dmax = (scale_by_dpi(RECT_DIST_JITTER, CURRENT_DEVICE.dpi) as i32).pow(2) as u32;
                let mut rects = Vec::new();

                for chunk in &self.chunks {
                    for word in &self.text[&chunk.location] {
                        let rect = (word.rect * chunk.scale).to_rect() - chunk.frame.min + chunk.position;
                        rects.push((rect, word.location));
                        let d = center.rdist2(&rect);
                        if d < dmax && d < dmin {
                            dmin = d;
                            found = Some((word.clone(), rects.len() - 1));
                        }
                    }
                }

                let selection = self.selection.as_mut().unwrap();

                if let Some((word, index)) = found {
                    let old_start = selection.start;
                    let old_end = selection.end;

                    let (start, end) = if word.location <= old_start {
                        (word.location, old_end)
                    } else if word.location >= old_end {
                        (old_start, word.location)
                    } else {
                        let (start_index, end_index) = (rects.iter().position(|(_, loc)| *loc == old_start),
                                                        rects.iter().position(|(_, loc)| *loc == old_end));
                        match (start_index, end_index) {
                            (Some(s), Some(e)) => {
                                if index - s > e - index {
                                    (old_start, word.location)
                                } else {
                                    (word.location, old_end)
                                }
                            },
                            (Some(..), None) => (word.location, old_end),
                            (None, Some(..)) => (old_start, word.location),
                            (None, None) => (old_start, old_end)
                        }
                    };

                    if start == old_start && end == old_end {
                        return true;
                    }

                    let (start_low, start_high) = old_start.min_max(start);
                    let (end_low, end_high) = old_end.min_max(end);

                    if start_low != start_high {
                        if let Some(mut i) = rects.iter().position(|(_, loc)| *loc == start_low) {
                            let mut rect = rects[i].0;
                            while i < rects.len() - 1 && rects[i].1 < start_high {
                                let next_rect = rects[i+1].0;
                                if rect.min.y < next_rect.max.y && next_rect.min.y < rect.max.y {
                                    if rects[i+1].1 == start_high {
                                        if rect.min.x < next_rect.min.x {
                                            rect.max.x = next_rect.min.x;
                                        } else {
                                            rect.min.x = next_rect.max.x;
                                        }
                                        rect.min.y = rect.min.y.min(next_rect.min.y);
                                        rect.max.y = rect.max.y.max(next_rect.max.y);
                                    } else {
                                        rect.absorb(&next_rect);
                                    }
                                } else {
                                    hub.send(Event::RenderRegion(rect, UpdateMode::Fast)).ok();
                                    rect = next_rect;
                                }
                                i += 1;
                            }
                            hub.send(Event::RenderRegion(rect, UpdateMode::Fast)).ok();
                        }
                    }

                    if end_low != end_high {
                        if let Some(mut i) = rects.iter().rposition(|(_, loc)| *loc == end_high) {
                            let mut rect = rects[i].0;
                            while i > 0 && rects[i].1 > end_low {
                                let prev_rect = rects[i-1].0;
                                if rect.min.y < prev_rect.max.y && prev_rect.min.y < rect.max.y {
                                    if rects[i-1].1 == end_low {
                                        if rect.min.x > prev_rect.min.x {
                                            rect.min.x = prev_rect.max.x;
                                        } else {
                                            rect.max.x = prev_rect.min.x;
                                        }
                                        rect.min.y = rect.min.y.min(prev_rect.min.y);
                                        rect.max.y = rect.max.y.max(prev_rect.max.y);
                                    } else {
                                        rect.absorb(&prev_rect);
                                    }
                                } else {
                                    hub.send(Event::RenderRegion(rect, UpdateMode::Fast)).ok();
                                    rect = prev_rect;
                                }
                                i -= 1;
                            }
                            hub.send(Event::RenderRegion(rect, UpdateMode::Fast)).ok();
                        }
                    }

                    selection.start = start;
                    selection.end = end;
                }
                true
            },
            Event::Gesture(GestureEvent::Tap(center)) if self.rect.includes(center) => {
                if self.focus.is_some() {
                    return true;
                }

                let mut nearest_link = None;
                let mut dmin = u32::MAX;
                let dmax = (scale_by_dpi(RECT_DIST_JITTER, CURRENT_DEVICE.dpi) as i32).pow(2) as u32;

                for chunk in &self.chunks {
                    let (links, _) = self.doc.lock().ok()
                                         .and_then(|mut doc| doc.links(Location::Exact(chunk.location)))
                                         .unwrap_or((Vec::new(), 0));
                    for link in links {
                        let rect = (link.rect * chunk.scale).to_rect() - chunk.frame.min + chunk.position;
                        let d = center.rdist2(&rect);
                        if d < dmax && d < dmin {
                            dmin = d;
                            nearest_link = Some(link.clone());
                        }
                    }
                }

                if let Some(link) = nearest_link.take() {
                    let pdf_page = Regex::new(r"^#(\d+)(?:,-?\d+,-?\d+)?$").unwrap();
                    let toc_page = Regex::new(r"^@(.+)$").unwrap();
                    if let Some(caps) = toc_page.captures(&link.text) {
                        let loc_opt = if caps[1].chars().all(|c| c.is_digit(10)) {
                            caps[1].parse::<usize>()
                                   .map(Location::Exact)
                                   .ok()
                        } else {
                            Some(Location::Uri(caps[1].to_string()))
                        };
                        if let Some(location) = loc_opt {
                            self.quit(context);
                            hub.send(Event::Back).ok();
                            hub.send(Event::GoToLocation(location)).ok();
                        }
                    } else if let Some(caps) = pdf_page.captures(&link.text) {
                        if let Ok(index) = caps[1].parse::<usize>() {
                            self.go_to_page(index.saturating_sub(1), true, hub, context);
                        }
                    } else {
                        let mut doc = self.doc.lock().unwrap();
                        let loc = Location::LocalUri(self.current_page, link.text.clone());
                        if let Some(location) = doc.resolve_location(loc) {
                            hub.send(Event::GoTo(location)).ok();
                        } else {
                            eprintln!("Can't resolve URI: {}.", link.text);
                        }
                    }
                    return true;
                }

                let w = self.rect.width() as i32;
                let h = self.rect.height() as i32;
                let m = w.min(h);
                let db = m / 3;
                let ds = db / 2;
                let x1 = self.rect.min.x + db;
                let x2 = self.rect.max.x - db;
                let sx1 = self.rect.min.x + ds;
                let sx2 = self.rect.max.x - ds;

                if center.x < x1 {
                    let dc = sx1 - center.x;
                    // Top left corner.
                    if dc > 0 && center.y < self.rect.min.y + dc {
                        self.go_to_last_page(hub, context);
                    // Bottom left corner.
                    } else if dc > 0 && center.y > self.rect.max.y - dc {
                        if self.search.is_none() {
                            if self.ephemeral {
                                self.quit(context);
                                hub.send(Event::Back).ok();
                            } else {
                                hub.send(Event::Show(ViewId::TableOfContents)).ok();
                            }
                        } else {
                            self.go_to_neighbor(CycleDir::Previous, hub, context);
                        }
                    // Left ear.
                    } else {
                        if self.search.is_none() {
                            self.go_to_neighbor(CycleDir::Previous, hub, context);
                        } else {
                            self.go_to_results_neighbor(CycleDir::Previous, hub, context);
                        }
                    }
                } else if center.x > x2 {
                    let dc = center.x - sx2;
                    // Top right corner.
                    if dc > 0 && center.y < self.rect.min.y + dc {
                        self.toggle_bookmark(hub);
                    // Bottom right corner.
                    } else if dc > 0 && center.y > self.rect.max.y - dc {
                        if self.search.is_none() {
                            hub.send(Event::Toggle(ViewId::GoToPage)).ok();
                        } else {
                            self.go_to_neighbor(CycleDir::Next, hub, context);
                        }
                    // Right ear.
                    } else {
                        if self.search.is_none() {
                            self.go_to_neighbor(CycleDir::Next, hub, context);
                        } else {
                            self.go_to_results_neighbor(CycleDir::Next, hub, context);
                        }
                    }
                // Middle band.
                } else {
                    self.toggle_bars(None, hub, context);
                }

                true
            },
            Event::Gesture(GestureEvent::HoldFingerShort(center, id)) if self.rect.includes(center) => {
                if self.focus.is_some() {
                    return true;
                }

                let mut found = None;
                let mut dmin = u32::MAX;
                let dmax = (scale_by_dpi(RECT_DIST_JITTER, CURRENT_DEVICE.dpi) as i32).pow(2) as u32;

                if let Some(rect) = self.selection_rect() {
                    let d = center.rdist2(&rect);
                    if d < dmax {
                        self.state = State::Idle;
                        let radius = scale_by_dpi(24.0, CURRENT_DEVICE.dpi) as i32;
                        self.toggle_selection_menu(Rectangle::from_disk(center, radius), Some(true), hub, context);
                    }
                    return true;
                }

                for chunk in &self.chunks {
                    for word in &self.text[&chunk.location] {
                        let rect = (word.rect * chunk.scale).to_rect() - chunk.frame.min + chunk.position;
                        let d = center.rdist2(&rect);
                        if d < dmax && d < dmin {
                            dmin = d;
                            found = Some((word.clone(), rect));
                        }
                    }
                }

                if let Some((nearest_word, rect)) = found {
                    let anchor = nearest_word.location;
                    if let Some(annot) = self.annotations.values().flatten()
                                             .find(|annot| anchor >= annot.selection[0] && anchor <= annot.selection[1]).cloned() {
                        let radius = scale_by_dpi(24.0, CURRENT_DEVICE.dpi) as i32;
                        self.toggle_annotation_menu(&annot, Rectangle::from_disk(center, radius), Some(true), hub, context);
                    } else {
                        self.selection = Some(Selection {
                            start: anchor,
                            end: anchor,
                            anchor,
                        });
                        self.state = State::Selection(id);
                        hub.send(Event::RenderRegion(rect, UpdateMode::Fast)).ok();
                    }
                }

                true
            },
            Event::Gesture(GestureEvent::HoldFingerLong(center, _)) if self.rect.includes(center) => {
                if let Some(text) = self.selected_text() {
                    let query = text.trim_matches(|c: char| !c.is_alphanumeric()).to_string();
                    let language = self.info.language.clone();
                    hub.send(Event::Select(EntryId::Launch(AppCmd::Dictionary { query, language }))).ok();
                }
                self.selection = None;
                self.state = State::Idle;
                true
            },
            Event::Update(mode) => {
                self.update(Some(mode), hub, context);
                true
            },
            Event::LoadPixmap(location) => {
                self.load_pixmap(location);
                true
            },
            Event::Submit(ViewId::GoToPageInput, ref text) => {
                let re = Regex::new(r#"^([-+"'])?(.+)$"#).unwrap();
                if let Some(caps) = re.captures(text) {
                    let prefix = caps.get(1).map(|m| m.as_str());
                    if prefix == Some("\"") || prefix == Some("'") {
                        if let Some(location) = self.find_page_by_name(&caps[2]) {
                            self.go_to_page(location, true, hub, context);
                        }
                    } else {
                        if let Ok(number) = caps[2].parse::<f64>() {
                            let location = if !self.synthetic {
                                let mut index = number.max(0.0) as usize;
                                match prefix {
                                    Some("-") => index = self.current_page.saturating_sub(index),
                                    Some("+") => index += self.current_page,
                                    _ => index = index.saturating_sub(1),
                                }
                                index
                            } else {
                                (number * BYTES_PER_PAGE).max(0.0).round() as usize
                            };
                            self.go_to_page(location, true, hub, context);
                        }
                    }
                }
                true
            },
            Event::Submit(ViewId::GoToResultsPageInput, ref text) => {
                if let Ok(index) = text.parse::<usize>() {
                    self.go_to_results_page(index.saturating_sub(1), hub, context);
                }
                true
            },
            Event::Submit(ViewId::NamePageInput, ref text) => {
                if !text.is_empty() {
                    if let Some(ref mut r) = self.info.reader {
                        r.page_names.insert(self.current_page, text.to_string());
                    }
                }
                self.toggle_keyboard(false, None, hub, context);
                true
            },
            Event::Submit(ViewId::EditNoteInput, ref note) => {
                let selection = self.selection.take().map(|sel| [sel.start, sel.end]);

                if let Some(sel) = selection {
                    let text = self.text_excerpt(sel).unwrap();
                    self.info.reader.as_mut().map(|r| {
                        r.annotations.push(Annotation {
                            selection: sel,
                            note: note.to_string(),
                            text,
                            modified: Local::now(),
                        });
                    });
                    if let Some(rect) = self.text_rect(sel) {
                        hub.send(Event::RenderRegion(rect, UpdateMode::Gui)).ok();
                    }
                } else {
                    if let Some(sel) = self.target_annotation.take() {
                        if let Some(annot) = self.find_annotation_mut(sel) {
                            annot.note = note.to_string();
                            annot.modified = Local::now();
                        }
                    }
                }

                self.update_annotations();
                self.toggle_keyboard(false, None, hub, context);
                true
            },
            Event::Submit(ViewId::ReaderSearchInput, ref text) => {
                match make_query(text) {
                    Some(query) => {
                        self.search(text, query, hub);
                        self.toggle_keyboard(false, None, hub, context);
                        self.toggle_results_bar(true, hub, context);
                    },
                    None => {
                        let notif = Notification::new(ViewId::InvalidSearchQueryNotif,
                                                      "Invalid search query.".to_string(),
                                                      hub,
                                                      context);
                        self.children.push(Box::new(notif) as Box<dyn View>);
                    },
                }
                true
            },
            Event::Page(dir) => {
                self.go_to_neighbor(dir, hub, context);
                true
            },
            Event::GoTo(location) | Event::Select(EntryId::GoTo(location)) => {
                self.go_to_page(location, true, hub, context);
                true
            },
            Event::GoToLocation(ref location) => {
                let offset_opt = {
                    let mut doc = self.doc.lock().unwrap();
                    doc.resolve_location(location.clone())
                };
                if let Some(offset) = offset_opt {
                    self.go_to_page(offset, true, hub, context);
                }
                true
            },
            Event::Chapter(dir) => {
                self.go_to_chapter(dir, hub, context);
                true
            },
            Event::ResultsPage(dir) => {
                self.go_to_results_neighbor(dir, hub, context);
                true
            },
            Event::CropMargins(ref margin) => {
                let current_page = self.current_page;
                self.crop_margins(current_page, margin.as_ref(), hub, context);
                true
            },
            Event::Toggle(ViewId::TopBottomBars) => {
                self.toggle_bars(None, hub, context);
                true
            },
            Event::Toggle(ViewId::GoToPage) => {
                self.toggle_go_to_page(None, ViewId::GoToPage, hub, context);
                true
            },
            Event::Toggle(ViewId::GoToResultsPage) => {
                self.toggle_go_to_page(None, ViewId::GoToResultsPage, hub, context);
                true
            },
            Event::Slider(SliderId::FontSize, font_size, FingerStatus::Up) => {
                self.set_font_size(font_size, hub, context);
                true
            },
            Event::Slider(SliderId::ContrastExponent, exponent, FingerStatus::Up) => {
                self.set_contrast_exponent(exponent, hub, context);
                true
            },
            Event::Slider(SliderId::ContrastGray, gray, FingerStatus::Up) => {
                self.set_contrast_gray(gray, hub, context);
                true
            },
            Event::ToggleNear(ViewId::TitleMenu, rect) => {
                self.toggle_title_menu(rect, None, hub, context);
                true
            },
            Event::ToggleNear(ViewId::MainMenu, rect) => {
                toggle_main_menu(self, rect, None, hub, context);
                true
            },
            Event::ToggleNear(ViewId::BatteryMenu, rect) => {
                toggle_battery_menu(self, rect, None, hub, context);
                true
            },
            Event::ToggleNear(ViewId::ClockMenu, rect) => {
                toggle_clock_menu(self, rect, None, hub, context);
                true
            },
            Event::ToggleNear(ViewId::MarginCropperMenu, rect) => {
                self.toggle_margin_cropper_menu(rect, None, hub, context);
                true
            },
            Event::ToggleNear(ViewId::SearchMenu, rect) => {
                self.toggle_search_menu(rect, None, hub, context);
                true
            },
            Event::ToggleNear(ViewId::FontFamilyMenu, rect) => {
                self.toggle_font_family_menu(rect, None, hub, context);
                true
            },
            Event::ToggleNear(ViewId::FontSizeMenu, rect) => {
                self.toggle_font_size_menu(rect, None, hub, context);
                true
            },
            Event::ToggleNear(ViewId::TextAlignMenu, rect) => {
                self.toggle_text_align_menu(rect, None, hub, context);
                true
            },
            Event::ToggleNear(ViewId::MarginWidthMenu, rect) => {
                self.toggle_margin_width_menu(rect, None, hub, context);
                true
            },
            Event::ToggleNear(ViewId::LineHeightMenu, rect) => {
                self.toggle_line_height_menu(rect, None, hub, context);
                true
            },
            Event::ToggleNear(ViewId::ContrastExponentMenu, rect) => {
                self.toggle_contrast_exponent_menu(rect, None, hub, context);
                true
            },
            Event::ToggleNear(ViewId::ContrastGrayMenu, rect) => {
                self.toggle_contrast_gray_menu(rect, None, hub, context);
                true
            },
            Event::ToggleNear(ViewId::PageMenu, rect) => {
                self.toggle_page_menu(rect, None, hub, context);
                true
            },
            Event::Close(ViewId::MainMenu) => {
                toggle_main_menu(self, Rectangle::default(), Some(false), hub, context);
                true
            },
            Event::Close(ViewId::SearchBar) => {
                self.toggle_results_bar(false, hub, context);
                self.toggle_search_bar(false, hub, context);
                if let Some(ref mut s) = self.search {
                    s.running.store(false, AtomicOrdering::Relaxed);
                    self.render_results(hub);
                    self.search = None;
                }
                true
            },
            Event::Close(ViewId::GoToPage) => {
                self.toggle_go_to_page(Some(false), ViewId::GoToPage, hub, context);
                true
            },
            Event::Close(ViewId::GoToResultsPage) => {
                self.toggle_go_to_page(Some(false), ViewId::GoToResultsPage, hub, context);
                true
            },
            Event::Close(ViewId::SelectionMenu) => {
                if self.state == State::Idle && self.target_annotation.is_none() {
                    if let Some(rect) = self.selection_rect() {
                        self.selection = None;
                        hub.send(Event::RenderRegion(rect, UpdateMode::Gui)).ok();
                    }
                }
                false
            },
            Event::Close(ViewId::EditNote) => {
                self.toggle_edit_note(None, Some(false), hub, context);
                if let Some(rect) = self.selection_rect() {
                    self.selection = None;
                    hub.send(Event::RenderRegion(rect, UpdateMode::Gui)).ok();
                }
                self.target_annotation = None;
                false
            },
            Event::Close(ViewId::NamePage) => {
                self.toggle_keyboard(false, None, hub, context);
                false
            },
            Event::Show(ViewId::TableOfContents) => {
                {
                    self.toggle_bars(Some(false), hub, context);
                }
                let mut doc = self.doc.lock().unwrap();
                if let Some(toc) = self.toc()
                                       .or_else(|| doc.toc())
                                       .filter(|toc| !toc.is_empty()) {
                    let chap_index = doc.chapter(self.current_page, &toc)
                                        .map(|chap| chap.index)
                                        .unwrap_or(usize::MAX);
                    hub.send(Event::OpenToc(toc, chap_index)).ok();
                }
                true
            },
            Event::Show(ViewId::SearchBar) => {
                self.toggle_search_bar(true, hub, context);
                true
            },
            Event::Show(ViewId::MarginCropper) => {
                self.toggle_margin_cropper(true, hub, context);
                true
            },
            Event::Close(ViewId::MarginCropper) => {
                self.toggle_margin_cropper(false, hub, context);
                true
            },
            Event::SearchResult(location, ref rects) => {
                if self.search.is_none() {
                    return true;
                }

                let mut results_count = 0;

                if let Some(ref mut s) = self.search {
                    let pages_count = s.highlights.len();
                    s.highlights.entry(location).or_insert_with(Vec::new).push(rects.clone());
                    s.results_count += 1;
                    results_count = s.results_count;
                    if results_count > 1 && location <= self.current_page && s.highlights.len() > pages_count {
                        s.current_page += 1;
                    }
                }

                self.update_results_bar(hub);

                if results_count == 1 {
                    self.toggle_results_bar(false, hub, context);
                    self.toggle_search_bar(false, hub, context);
                    self.go_to_page(location, true, hub, context);
                } else if location == self.current_page {
                    self.update(None, hub, context);
                }

                true
            },
            Event::EndOfSearch => {
                let results_count = self.search.as_ref().map(|s| s.results_count)
                                        .unwrap_or(usize::MAX);
                if results_count == 0 {
                    let notif = Notification::new(ViewId::NoSearchResultsNotif,
                                                  "No search results.".to_string(),
                                                  hub,
                                                  context);
                    self.children.push(Box::new(notif) as Box<dyn View>);
                    self.toggle_search_bar(true, hub, context);
                    hub.send(Event::Focus(Some(ViewId::ReaderSearchInput))).ok();
                }
                true
            },
            Event::Select(EntryId::AnnotateSelection) => {
                self.toggle_edit_note(None, Some(true), hub, context);
                true
            },
            Event::Select(EntryId::HighlightSelection) => {
                if let Some(sel) = self.selection.take() {
                    let text = self.text_excerpt([sel.start, sel.end]).unwrap();
                    self.info.reader.as_mut().map(|r| {
                        r.annotations.push(Annotation {
                            selection: [sel.start, sel.end],
                            note: String::new(),
                            text,
                            modified: Local::now(),
                        });
                    });
                    if let Some(rect) = self.text_rect([sel.start, sel.end]) {
                        hub.send(Event::RenderRegion(rect, UpdateMode::Gui)).ok();
                    }
                    self.update_annotations();
                }

                true
            },
            Event::Select(EntryId::DefineSelection) => {
                if let Some(text) = self.selected_text() {
                    let query = text.trim_matches(|c: char| !c.is_alphanumeric()).to_string();
                    let language = self.info.language.clone();
                    hub.send(Event::Select(EntryId::Launch(AppCmd::Dictionary { query, language }))).ok();
                }
                self.selection = None;
                true
            },
            Event::Select(EntryId::SearchForSelection) => {
                if let Some(text) = self.selected_text() {
                    let text = text.trim_matches(|c: char| !c.is_alphanumeric());
                    match make_query(text) {
                        Some(query) => {
                            self.search(text, query, hub);
                        },
                        None => {
                            let notif = Notification::new(ViewId::InvalidSearchQueryNotif,
                                                          "Invalid search query.".to_string(),
                                                          hub,
                                                          context);
                            self.children.push(Box::new(notif) as Box<dyn View>);
                        },
                    }
                }
                if let Some(rect) = self.selection_rect() {
                    hub.send(Event::RenderRegion(rect, UpdateMode::Gui)).ok();
                }
                self.selection = None;
                true
            },
            Event::Select(EntryId::GoToSelectedPageName) => {
                self.selected_text().and_then(|text| {
                    let end = text.find(|c: char| !c.is_ascii_digit() &&
                                                  !Digit::from_char(c).is_ok() &&
                                                  !c.is_ascii_uppercase())
                                  .unwrap_or(text.len());
                    self.find_page_by_name(&text[..end])
                }).map(|loc| {
                    self.go_to_page(loc, true, hub, context);
                });
                if let Some(rect) = self.selection_rect() {
                    hub.send(Event::RenderRegion(rect, UpdateMode::Gui)).ok();
                }
                self.selection = None;
                true
            },
            Event::Select(EntryId::AdjustSelection) => {
                self.state = State::AdjustSelection;
                true
            },
            Event::Select(EntryId::EditAnnotationNote(sel)) => {
                let text = self.find_annotation_ref(sel).map(|annot| annot.note.clone());
                self.toggle_edit_note(text, Some(true), hub, context);
                self.target_annotation = Some(sel);
                true
            },
            Event::Select(EntryId::RemoveAnnotationNote(sel)) => {
                if let Some(annot) = self.find_annotation_mut(sel) {
                    annot.note.clear();
                    annot.modified = Local::now();
                }
                self.update_annotations();
                true
            },
            Event::Select(EntryId::RemoveAnnotation(sel)) => {
                if let Some(annotations) = self.info.reader.as_mut().map(|r| &mut r.annotations) {
                    annotations.retain(|annot| annot.selection[0] != sel[0] || annot.selection[1] != sel[1]); 
                    self.update_annotations();
                }
                if let Some(rect) = self.text_rect(sel) {
                    hub.send(Event::RenderRegion(rect, UpdateMode::Gui)).ok();
                }
                true
            },
            Event::Select(EntryId::SetZoomMode(zoom_mode)) => {
                self.set_zoom_mode(zoom_mode, hub, context);
                true
            },
            Event::Select(EntryId::ApplyCroppings(index, scheme)) => {
                self.info.reader.as_mut().map(|r| {
                    if r.cropping_margins.is_none() {
                        r.cropping_margins = Some(CroppingMargins::Any(Margin::default()));
                    }
                    r.cropping_margins.as_mut().map(|c| c.apply(index, scheme))
                });
                true
            },
            Event::Select(EntryId::RemoveCroppings) => {
                if let Some(r) = self.info.reader.as_mut() {
                    r.cropping_margins = None;
                }
                self.cache.clear();
                self.update(None, hub, context);
                true
            },
            Event::Select(EntryId::SearchDirection(dir)) => {
                self.search_direction = dir;
                true
            },
            Event::Select(EntryId::SetFontFamily(ref font_family)) => {
                self.set_font_family(font_family, hub, context);
                true
            },
            Event::Select(EntryId::SetTextAlign(text_align)) => {
                self.set_text_align(text_align, hub, context);
                true
            },
            Event::Select(EntryId::SetFontSize(v)) => {
                let font_size = self.info.reader.as_ref()
                                    .and_then(|r| r.font_size)
                                    .unwrap_or(context.settings.reader.font_size);
                let font_size = font_size - 1.0 + v as f32 / 10.0;
                self.set_font_size(font_size, hub, context);
                true
            },
            Event::Select(EntryId::SetMarginWidth(width)) => {
                self.set_margin_width(width, hub, context);
                true
            },
            Event::Select(EntryId::SetLineHeight(v)) => {
                let line_height = 1.0 + v as f32 / 10.0;
                self.set_line_height(line_height, hub, context);
                true
            },
            Event::Select(EntryId::SetContrastExponent(v)) => {
                let exponent = 1.0 + v as f32 / 2.0;
                self.set_contrast_exponent(exponent, hub, context);
                true
            },
            Event::Select(EntryId::SetContrastGray(v)) => {
                let gray = ((1 << 8) - (1 << (8 - v))) as f32;
                self.set_contrast_gray(gray, hub, context);
                true
            },
            Event::Select(EntryId::SetPageName) => {
                self.toggle_name_page(None, hub, context);
                true
            },
            Event::Select(EntryId::RemovePageName) => {
                if let Some(ref mut r) = self.info.reader {
                    r.page_names.remove(&self.current_page);
                }
                true
            },
            Event::Reseed => {
                self.reseed(hub, context);
                true
            },
            Event::ToggleFrontlight => {
                if let Some(index) = locate::<TopBar>(self) {
                    self.child_mut(index).downcast_mut::<TopBar>().unwrap()
                        .update_frontlight_icon(hub, context);
                }
                true
            },
            Event::Device(DeviceEvent::Button { code: ButtonCode::Home, status: ButtonStatus::Pressed, .. }) => {
                self.quit(context);
                hub.send(Event::Back).ok();
                true
            },
            Event::Select(EntryId::Quit) |
            Event::Select(EntryId::Reboot) |
            Event::Select(EntryId::StartNickel) |
            Event::Back => {
                self.quit(context);
                false
            },
            Event::Focus(v) => {
                if let Some(ViewId::ReaderSearchInput) = v {
                    self.toggle_results_bar(false, hub, context);
                    if let Some(ref mut s) = self.search {
                        s.running.store(false, AtomicOrdering::Relaxed);
                    }
                    self.render_results(hub);
                    self.search = None;
                }
                self.focus = v;
                if v.is_some() {
                    self.toggle_keyboard(true, v, hub, context);
                }
                true
            },
            _ => false,
        }
    }

    fn render(&self, fb: &mut dyn Framebuffer, rect: Rectangle, _fonts: &mut Fonts) {
        fb.draw_rectangle(&rect, WHITE);

        for chunk in &self.chunks {
            let Resource { ref pixmap, scale, .. } = self.cache[&chunk.location];
            let chunk_rect = chunk.frame - chunk.frame.min + chunk.position;

            if let Some(region_rect) = rect.intersection(&chunk_rect) {
                let chunk_frame = region_rect - chunk.position + chunk.frame.min;
                let chunk_position = region_rect.min;
                fb.draw_framed_pixmap_contrast(pixmap, &chunk_frame, chunk_position, self.contrast.exponent, self.contrast.gray);

                if let Some(groups) = self.search.as_ref().and_then(|s| s.highlights.get(&chunk.location)) {
                    for rects in groups {
                        let mut last_rect: Option<Rectangle> = None;
                        for r in rects {
                            let rect = (*r * scale).to_rect() - chunk.frame.min + chunk.position;
                            if let Some(ref search_rect) = rect.intersection(&region_rect) {
                                fb.invert_region(search_rect);
                            }
                            if let Some(last) = last_rect {
                                if rect.min.y < last.max.y && last.min.y < rect.max.y && (last.max.x < rect.min.x || rect.max.x < last.min.x) {
                                    let space = if last.max.x < rect.min.x {
                                        rect![last.max.x, (last.min.y + rect.min.y) / 2,
                                              rect.min.x, (last.max.y + rect.max.y) / 2]
                                    } else {
                                        rect![rect.max.x, (last.min.y + rect.min.y) / 2,
                                              last.min.x, (last.max.y + rect.max.y) / 2]
                                    };
                                    if let Some(ref res_rect) = space.intersection(&region_rect) {
                                        fb.invert_region(res_rect);
                                    }
                                }
                            }
                            last_rect = Some(rect);
                        }
                    }
                }

                if let Some(annotations) = self.annotations.get(&chunk.location) {
                    for annot in annotations {
                        let [start, end] = annot.selection;
                        if let Some(text) = self.text.get(&chunk.location) {
                            let mut last_rect: Option<Rectangle> = None;
                            for word in text.iter().filter(|w| w.location >= start && w.location <= end) {
                                let rect = (word.rect * scale).to_rect() - chunk.frame.min + chunk.position;
                                if let Some(ref sel_rect) = rect.intersection(&region_rect) {
                                    fb.shift_region(sel_rect, ANNOTATION_DRIFT);
                                }
                                if let Some(last) = last_rect {
                                    if rect.min.y < last.max.y && last.min.y < rect.max.y && (last.max.x < rect.min.x || rect.max.x < last.min.x) {
                                        let space = if last.max.x < rect.min.x {
                                            rect![last.max.x, (last.min.y + rect.min.y) / 2,
                                                  rect.min.x, (last.max.y + rect.max.y) / 2]
                                        } else {
                                            rect![rect.max.x, (last.min.y + rect.min.y) / 2,
                                                  last.min.x, (last.max.y + rect.max.y) / 2]
                                        };
                                        if let Some(ref sel_rect) = space.intersection(&region_rect) {
                                            fb.shift_region(sel_rect, ANNOTATION_DRIFT);
                                        }
                                    }
                                }
                                last_rect = Some(rect);
                            }
                        }
                    }
                }

                if let Some(sel) = self.selection.as_ref() {
                    if let Some(text) = self.text.get(&chunk.location) {
                        let mut last_rect: Option<Rectangle> = None;
                        for word in text.iter().filter(|w| w.location >= sel.start && w.location <= sel.end) {
                            let rect = (word.rect * scale).to_rect() - chunk.frame.min + chunk.position;
                            if let Some(ref sel_rect) = rect.intersection(&region_rect) {
                                fb.invert_region(sel_rect);
                            }
                            if let Some(last) = last_rect {
                                if rect.min.y < last.max.y && last.min.y < rect.max.y && (last.max.x < rect.min.x || rect.max.x < last.min.x) {
                                    let space = if last.max.x < rect.min.x {
                                        rect![last.max.x, (last.min.y + rect.min.y) / 2,
                                              rect.min.x, (last.max.y + rect.max.y) / 2]
                                    } else {
                                        rect![rect.max.x, (last.min.y + rect.min.y) / 2,
                                              last.min.x, (last.max.y + rect.max.y) / 2]
                                    };
                                    if let Some(ref sel_rect) = space.intersection(&region_rect) {
                                        fb.invert_region(sel_rect);
                                    }
                                }
                            }
                            last_rect = Some(rect);
                        }
                    }
                }
            }
        }

        if self.info.reader.as_ref().map_or(false, |r| r.bookmarks.contains(&self.current_page)) {
            let dpi = CURRENT_DEVICE.dpi;
            let thickness = scale_by_dpi(3.0, dpi) as u16;
            let radius = mm_to_px(0.4, dpi) as i32 + thickness as i32;
            let center = pt!(self.rect.max.x - 5 * radius,
                             self.rect.min.y + 5 * radius);
            fb.draw_rounded_rectangle_with_border(&Rectangle::from_disk(center, radius),
                                                  &CornerSpec::Uniform(radius),
                                                  &BorderSpec { thickness, color: WHITE },
                                                  &BLACK);
        }
    }

    fn render_rect(&self, rect: &Rectangle) -> Rectangle {
        rect.intersection(&self.rect)
            .unwrap_or(self.rect)
    }

    fn resize(&mut self, rect: Rectangle, hub: &Hub, context: &mut Context) {
        if !self.children.is_empty() {
            let dpi = CURRENT_DEVICE.dpi;
            let thickness = scale_by_dpi(THICKNESS_MEDIUM, dpi) as i32;
            let (small_thickness, big_thickness) = halves(thickness);
            let (small_height, big_height) = (scale_by_dpi(SMALL_BAR_HEIGHT, dpi) as i32,
                                              scale_by_dpi(BIG_BAR_HEIGHT, dpi) as i32);
            let mut floating_layer_start = 0;

            self.children.retain(|child| !child.is::<Menu>());

            if self.children[0].is::<TopBar>() {
                let top_bar_rect = rect![rect.min.x, rect.min.y,
                                         rect.max.x, small_height - small_thickness];
                self.children[0].resize(top_bar_rect, hub, context);
                let separator_rect = rect![rect.min.x,
                                           small_height - small_thickness,
                                           rect.max.x,
                                           small_height + big_thickness];
                self.children[1].resize(separator_rect, hub, context);
            } else if self.children[0].is::<Filler>() {
                let mut index = 1;
                if self.children[index].is::<SearchBar>() {
                    let sb_rect = rect![rect.min.x,
                                        rect.max.y - (3 * big_height + 2 * small_height) as i32 + big_thickness,
                                        rect.max.x,
                                        rect.max.y - (3 * big_height + small_height) as i32 - small_thickness];
                    self.children[index].resize(sb_rect, hub, context);
                    self.children[index-1].resize(rect![rect.min.x, sb_rect.min.y - thickness,
                                                        rect.max.x, sb_rect.min.y],
                                                  hub, context);
                    index += 2;
                }
                if self.children[index].is::<Keyboard>() {
                    let kb_rect = rect![rect.min.x,
                                        rect.max.y - (small_height + 3 * big_height) as i32 + big_thickness,
                                        rect.max.x,
                                        rect.max.y - small_height - small_thickness];
                    self.children[index].resize(kb_rect, hub, context);
                    self.children[index+1].resize(rect![rect.min.x, kb_rect.max.y,
                                                        rect.max.x, kb_rect.max.y + thickness],
                                                  hub, context);
                    let kb_rect = *self.children[index].rect();
                    self.children[index-1].resize(rect![rect.min.x, kb_rect.min.y - thickness,
                                                        rect.max.x, kb_rect.min.y],
                                                  hub, context);
                    index += 2;
                }
                floating_layer_start = index;
            }

            if let Some(mut index) = locate::<BottomBar>(self) {
                floating_layer_start = index + 1;
                let separator_rect = rect![rect.min.x,
                                           rect.max.y - small_height - small_thickness,
                                           rect.max.x,
                                           rect.max.y - small_height + big_thickness];
                self.children[index-1].resize(separator_rect, hub, context);
                let bottom_bar_rect = rect![rect.min.x,
                                            rect.max.y - small_height + big_thickness,
                                            rect.max.x,
                                            rect.max.y];
                self.children[index].resize(bottom_bar_rect, hub, context);

                index -= 2;

                while index > 2 {
                    let bar_height = if self.children[index].is::<ToolBar>() {
                        2 * big_height
                    } else if self.children[index].is::<Keyboard>() {
                        3 * big_height
                    } else {
                        small_height
                    } as i32;

                    let y_max = self.children[index+1].rect().min.y;
                    let bar_rect = rect![rect.min.x,
                                         y_max - bar_height + thickness,
                                         rect.max.x,
                                         y_max];
                    self.children[index].resize(bar_rect, hub, context);
                    let y_max = self.children[index].rect().min.y;
                    let sp_rect = rect![rect.min.x,
                                        y_max - thickness,
                                        rect.max.x,
                                        y_max];
                    self.children[index-1].resize(sp_rect, hub, context);

                    index -= 2;
                }
            }

            for i in floating_layer_start..self.children.len() {
                self.children[i].resize(rect, hub, context);
            }
        }

        if self.view_port.zoom_mode == ZoomMode::FitToWidth {
            let ratio = (rect.width() as i32 - 2 * self.view_port.margin_width) as f32 /
                        (self.rect.width() as i32 - 2 * self.view_port.margin_width) as f32;
            self.view_port.top_offset = (self.view_port.top_offset as f32 * ratio) as i32;
        }

        self.rect = rect;

        if self.reflowable {
            let font_size = self.info.reader.as_ref()
                                .and_then(|r| r.font_size)
                                .unwrap_or(context.settings.reader.font_size);
            let mut doc = self.doc.lock().unwrap();
            doc.layout(rect.width(), rect.height(), font_size, CURRENT_DEVICE.dpi);
            let current_page = self.current_page.min(doc.pages_count() - 1);
            if let Some(location) = doc.resolve_location(Location::Exact(current_page)) {
                self.current_page = location;
            }
            self.text.clear();
        }

        self.cache.clear();
        self.update(Some(UpdateMode::Full), hub, context);
    }

    fn might_rotate(&self) -> bool {
        self.search.is_none()
    }

    fn is_background(&self) -> bool {
        true
    }

    fn rect(&self) -> &Rectangle {
        &self.rect
    }

    fn rect_mut(&mut self) -> &mut Rectangle {
        &mut self.rect
    }

    fn children(&self) -> &Vec<Box<dyn View>> {
        &self.children
    }

    fn children_mut(&mut self) -> &mut Vec<Box<dyn View>> {
        &mut self.children
    }
}
