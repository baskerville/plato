mod top_bar;
mod tool_bar;
mod bottom_bar;
mod results_bar;
mod margin_cropper;
mod results_label;

use std::thread;
use std::sync::{Arc, Mutex, mpsc};
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering as AtomicOrdering;
use std::rc::Rc;
use std::cmp::Ordering;
use std::path::PathBuf;
use std::collections::VecDeque;
use chrono::Local;
use regex::Regex;
use input::FingerStatus;
use framebuffer::{Framebuffer, UpdateMode, Pixmap};
use view::{View, Event, Hub, ViewId, EntryKind, EntryId, SliderId, Bus, THICKNESS_MEDIUM};
use unit::{scale_by_dpi, mm_to_px};
use device::{CURRENT_DEVICE, BAR_SIZES};
use font::Fonts;
use font::family_names;
use self::margin_cropper::{MarginCropper, BUTTON_DIAMETER};
use self::top_bar::TopBar;
use self::tool_bar::ToolBar;
use self::bottom_bar::BottomBar;
use self::results_bar::ResultsBar;
use view::common::{locate, locate_by_id, toggle_main_menu, shift};
use view::filler::Filler;
use view::named_input::NamedInput;
use view::search_bar::SearchBar;
use view::keyboard::{Keyboard, DEFAULT_LAYOUT};
use view::menu::{Menu, MenuKind};
use view::notification::Notification;
use settings::{guess_frontlight, FinishedAction, DEFAULT_FONT_FAMILY};
use settings::{DEFAULT_FONT_SIZE, DEFAULT_MARGIN_WIDTH, DEFAULT_LINE_HEIGHT};
use frontlight::LightLevels;
use gesture::GestureEvent;
use document::{Document, DocumentOpener, Location, Neighbors};
use document::{TocEntry, toc_as_html, chapter_at, chapter_relative};
use document::pdf::PdfOpener;
use document::epub::LOCATION_EPSILON;
use metadata::{Info, FileInfo, ReaderInfo, PageScheme, Margin, CroppingMargins, make_query};
use geom::{Rectangle, CornerSpec, BorderSpec, Dir, CycleDir, LinearDir, halves};
use color::{BLACK, WHITE};
use app::Context;

const HISTORY_SIZE: usize = 32;

pub struct Reader {
    rect: Rectangle,
    children: Vec<Box<View>>,
    info: Info,
    doc: Arc<Mutex<Box<Document>>>,
    pixmap: Rc<Pixmap>,
    current_page: f32,
    pages_count: f32,
    synthetic: bool,
    page_turns: usize,
    finished: bool,
    ephemeral: bool,
    refresh_every: u8,
    search_direction: LinearDir,
    frame: Rectangle,
    scale: f32,
    focus: Option<ViewId>,
    search: Option<Search>,
    history: VecDeque<f32>,
}

#[derive(Debug)]
struct Search {
    query: String,
    highlights: Vec<Highlight>,
    running: Arc<AtomicBool>,
    current_page: usize,
    results_count: usize,
}

#[derive(Debug)]
struct Highlight {
    location: f32,
    rects: Vec<Rectangle>,
}

impl Default for Search {
    fn default() -> Self {
        Search {
            query: String::new(),
            highlights: Vec::new(),
            running: Arc::new(AtomicBool::new(true)),
            current_page: 0,
            results_count: 0,
        }
    }
}

impl Reader {
    pub fn new(rect: Rectangle, mut info: Info, hub: &Hub, context: &mut Context) -> Option<Reader> {
        let settings = &context.settings;
        let path = settings.library_path.join(&info.file.path);
        let opener = DocumentOpener::new(settings.reader.epub_engine);

        opener.open(&path).and_then(|mut doc| {
            let (width, height) = CURRENT_DEVICE.dims;
            let font_size = info.reader.as_ref().and_then(|r| r.font_size)
                                .unwrap_or(settings.reader.font_size);
            let first_location = doc.resolve_location(Location::Exact(0.0))?;

            doc.layout(width, height, font_size, CURRENT_DEVICE.dpi);

            let pages_count;
            let mut current_page;

            // TODO: use get_or_insert_with?
            if let Some(ref mut r) = info.reader {
                r.opened = Local::now();
                if r.finished {
                    r.finished = false;
                    r.current_page = first_location;
                }
                current_page = r.current_page;
                pages_count = r.pages_count;
                if let Some(ref font_family) = r.font_family {
                    doc.set_font_family(font_family, &settings.reader.font_path);
                }
                doc.set_margin_width(r.margin_width.unwrap_or(settings.reader.margin_width));
                if let Some(line_height) = r.line_height {
                    doc.set_line_height(line_height);
                }
            } else {
                current_page = first_location;
                pages_count = doc.pages_count();
                info.reader = Some(ReaderInfo {
                    current_page,
                    pages_count,
                    .. Default::default()
                });
            }

            let synthetic = doc.has_synthetic_page_numbers();

            println!("{}", info.file.path.display());

            let margin = info.reader.as_ref()
                             .and_then(|r| r.cropping_margins.as_ref()
                                            .map(|c| c.margin(current_page as usize)))
                             .cloned().unwrap_or_default();
            let ((pixmap, location), scale) = build_pixmap(&rect, doc.as_mut(), current_page, &margin);
            let frame = rect![(margin.left * pixmap.width as f32).ceil() as i32,
                              (margin.top * pixmap.height as f32).ceil() as i32,
                              ((1.0 - margin.right) * pixmap.width as f32).floor() as i32,
                              ((1.0 - margin.bottom) * pixmap.height as f32).floor() as i32];
            let pixmap = Rc::new(pixmap);
            current_page = location;

            hub.send(Event::Render(rect, UpdateMode::Partial)).unwrap();

            Some(Reader {
                rect,
                children: vec![],
                info,
                doc: Arc::new(Mutex::new(doc)),
                pixmap,
                current_page,
                pages_count,
                synthetic,
                page_turns: 0,
                finished: false,
                ephemeral: false,
                refresh_every: settings.reader.refresh_every,
                search_direction: LinearDir::Forward,
                frame,
                scale,
                focus: None,
                search: None,
                history: VecDeque::new(),
            })
        })
    }

    pub fn from_toc(rect: Rectangle, toc: &[TocEntry], mut current_page: f32, hub: &Hub, context: &mut Context) -> Reader {
        let html = toc_as_html(toc, current_page);

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
        let pages_count = doc.pages_count();

        current_page = chapter_at(toc, current_page).and_then(|chap| {
            let link_uri = format!("@{}", chap.location);
            let mut loc = Location::Exact(0.0);
            while let Some((links, l)) = doc.links(loc) {
                if links.iter().any(|link| link.text == link_uri) {
                    return Some(l)
                }
                loc = Location::Next(l);
            }
            None
        }).unwrap_or(0.0);

        let ((pixmap, location), scale) = build_pixmap(&rect, &mut doc, current_page, &Margin::default());
        current_page = location;
        let pixmap = Rc::new(pixmap);
        let frame = pixmap.rect();

        hub.send(Event::Render(rect, UpdateMode::Partial)).unwrap();

        Reader {
            rect,
            children: vec![],
            info,
            doc: Arc::new(Mutex::new(Box::new(doc))),
            pixmap,
            current_page,
            pages_count,
            synthetic: false,
            page_turns: 0,
            finished: false,
            ephemeral: true,
            refresh_every: context.settings.reader.refresh_every,
            search_direction: LinearDir::Forward,
            frame,
            scale,
            focus: None,
            search: None,
            history: VecDeque::new(),
        }
    }

    fn go_to_page(&mut self, location: f32, record: bool, hub: &Hub) {
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
                let search_page = s.highlights
                                   .binary_search_by(|a| a.location.partial_cmp(&location)
                                                          .unwrap_or(Ordering::Equal));
                s.current_page = if search_page.is_ok() {
                    search_page.unwrap()
                } else {
                    search_page.unwrap_err()
                               .min(s.highlights.len().saturating_sub(1))
                };
            }

            self.current_page = location;
            self.update(hub);
            self.update_bottom_bar(hub);

            if self.search.is_some() {
                self.update_results_bar(hub);
            }
        }
    }

    fn go_to_chapter(&mut self, dir: CycleDir, hub: &Hub) {
        let current_page = self.current_page;
        let chap = {
            let mut doc = self.doc.lock().unwrap();
            doc.toc().and_then(|toc| chapter_relative(&toc, current_page, dir))
        };
        if let Some(location) = chap {
            self.go_to_page(location, true, hub);
        }
    }

    fn go_to_bookmark(&mut self, dir: CycleDir, hub: &Hub) {
        let mut loc = None;
        if let Some(ref r) = self.info.reader {
            match dir {
                CycleDir::Next => {
                    loc = r.bookmarks.iter().find(|&loc| *loc > self.current_page).cloned();
                },
                CycleDir::Previous => {
                    loc = r.bookmarks.iter().filter(|&loc| *loc < self.current_page)
                                     .next_back().cloned();
                },
            }
        }
        if let Some(location) = loc {
            self.go_to_page(location, true, hub);
        }
    }

    fn go_to_last_page(&mut self, hub: &Hub) {
        if let Some(location) = self.history.pop_back() {
            self.go_to_page(location, false, hub);
        }
    }

    fn go_to_neighbor(&mut self, dir: CycleDir, hub: &Hub, context: &mut Context) {
        let current_page = self.current_page;
        let loc = {
            let neighloc = if dir == CycleDir::Previous {
                Location::Previous(current_page)
            } else {
                Location::Next(current_page)
            };
            let mut doc = self.doc.lock().unwrap();
            doc.resolve_location(neighloc)
        };
        if let Some(location) = loc {
            self.go_to_page(location, false, hub);
        } else {
            match dir {
                CycleDir::Next => {
                    self.finished = true;
                    match context.settings.reader.finished {
                        FinishedAction::Notify => {
                            let notif = Notification::new(ViewId::BoundaryNotif,
                                                          "No next page.".to_string(),
                                                          &mut context.notification_index,
                                                          &mut context.fonts,
                                                          hub);
                            self.children.push(Box::new(notif) as Box<View>);
                        },
                        FinishedAction::Close => {
                            hub.send(Event::Back).unwrap();
                        },
                    }
                },
                CycleDir::Previous => {
                    let notif = Notification::new(ViewId::BoundaryNotif,
                                                  "No previous page.".to_string(),
                                                  &mut context.notification_index,
                                                  &mut context.fonts,
                                                  hub);
                    self.children.push(Box::new(notif) as Box<View>);
                },
            }
        }
    }

    fn go_to_results_page(&mut self, index: usize, hub: &Hub) {
        let mut loc = None;
        if let Some(ref mut s) = self.search {
            if index < s.highlights.len() {
                s.current_page = index;
                loc = Some(s.highlights[index].location);
            }
        }
        if let Some(location) = loc {
            self.current_page = location;
            self.update_results_bar(hub);
            self.update_bottom_bar(hub);
            self.update(hub);
        }
    }

    fn go_to_results_neighbor(&mut self, dir: CycleDir, hub: &Hub) {
        let loc = self.search.as_ref().and_then(|s| {
            match dir {
                CycleDir::Next => s.highlights.iter().find(|h| h.location > self.current_page)
                                              .map(|e| e.location),
                CycleDir::Previous => s.highlights.iter().filter(|h| h.location < self.current_page)
                                                  .next_back().map(|e| e.location),
            }
        });
        if let Some(location) = loc {
            if let Some(ref mut s) = self.search {
                let search_page = s.highlights
                                   .binary_search_by(|a| a.location.partial_cmp(&location)
                                                          .unwrap_or(Ordering::Equal));
                s.current_page = if search_page.is_ok() {
                    search_page.unwrap()
                } else {
                    search_page.unwrap_err()
                };
            }
            self.current_page = location;
            self.update_results_bar(hub);
            self.update_bottom_bar(hub);
            self.update(hub);
        }
    }

    fn update_bottom_bar(&mut self, hub: &Hub) {
        if let Some(index) = locate::<BottomBar>(self) {
            let current_page = self.current_page;
            let bottom_bar = self.children[index].as_mut().downcast_mut::<BottomBar>().unwrap();
            let mut doc = self.doc.lock().unwrap();
            let neighbors = Neighbors {
                previous_page: doc.resolve_location(Location::Previous(current_page)),
                next_page: doc.resolve_location(Location::Next(current_page)),
            };
            bottom_bar.update_page_label(self.current_page, self.pages_count, hub);
            bottom_bar.update_icons(&neighbors, hub);
            let chapter = doc.toc().as_ref().and_then(|t| chapter_at(t, current_page))
                                   .map(|c| c.title.clone())
                                   .unwrap_or_default();
            bottom_bar.update_chapter(chapter, hub);
        }
    }

    fn update_tool_bar(&mut self, hub: &Hub, context: &mut Context) {
        if let Some(index) = locate::<ToolBar>(self) {
            let settings = &context.settings;
            let tool_bar = self.children[index].as_mut().downcast_mut::<ToolBar>().unwrap();
            let font_family = self.info.reader.as_ref()
                                  .and_then(|r| r.font_family.clone())
                                  .unwrap_or_else(|| settings.reader.font_family.clone());
            tool_bar.update_font_family(font_family, hub);
            let font_size = self.info.reader.as_ref()
                                .and_then(|r| r.font_size)
                                .unwrap_or(settings.reader.font_size);
            tool_bar.update_slider(font_size, hub);
            let margin_width = self.info.reader.as_ref()
                                   .and_then(|r| r.margin_width)
                                   .unwrap_or(settings.reader.margin_width);
            tool_bar.update_margin_width(margin_width, hub);
            let line_height = self.info.reader.as_ref()
                                  .and_then(|r| r.line_height)
                                  .unwrap_or(settings.reader.line_height);
            tool_bar.update_line_height(line_height, hub);
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

    fn update(&mut self, hub: &Hub) {
        self.page_turns += 1;
        let update_mode = if self.refresh_every > 0 {
            if self.page_turns % (self.refresh_every as usize) == 0 {
                UpdateMode::Full
            } else {
                UpdateMode::Partial
            }
        } else {
            UpdateMode::Partial
        };
        let margin = self.info.reader.as_ref()
                         .and_then(|r| r.cropping_margins.as_ref()
                                        .map(|c| c.margin(self.current_page as usize)))
                         .cloned().unwrap_or_default();
        let mut doc = self.doc.lock().unwrap();
        let ((pixmap, location), scale) = build_pixmap(&self.rect, doc.as_mut(), self.current_page, &margin);
        self.current_page = location;
        self.pixmap = Rc::new(pixmap);
        let frame = rect![(margin.left * self.pixmap.width as f32).ceil() as i32,
                          (margin.top * self.pixmap.height as f32).ceil() as i32,
                          ((1.0 - margin.right) * self.pixmap.width as f32).floor() as i32,
                          ((1.0 - margin.bottom) * self.pixmap.height as f32).floor() as i32];
        self.frame = frame;
        self.scale = scale;
        hub.send(Event::Render(self.rect, update_mode)).unwrap();
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

                if let Some((ref words, location)) = doc.words(loc) {
                    if (location - current_page).abs() < LOCATION_EPSILON && started {
                        break;
                    }
                    for word in words {
                        if query.is_match(&word.text) {
                            if !running.load(AtomicOrdering::Relaxed) {
                                break;
                            }
                            hub2.send(Event::SearchResult(location, word.rect)).unwrap();
                        }
                    }
                    loc = match search_direction {
                        LinearDir::Forward => Location::Next(location),
                        LinearDir::Backward => Location::Previous(location),
                    };
                } else {
                    loc = match search_direction {
                        LinearDir::Forward => Location::Exact(0.0),
                        LinearDir::Backward => Location::Exact(doc.pages_count()),
                    };
                }

                started = true;
            }

            running.store(false, AtomicOrdering::Relaxed);
            hub2.send(Event::EndOfSearch).unwrap();
        });

        self.search = Some(s);
    }

    fn toggle_keyboard(&mut self, enable: bool, id: Option<ViewId>, hub: &Hub) {
        if let Some(index) = locate::<Keyboard>(self) {
            if enable {
                return;
            }

            let mut rect = *self.child(index).rect();
            rect.absorb(self.child(index-1).rect());
            if index == 1 {
                rect.absorb(self.child(index+1).rect());
            }

            hub.send(Event::Expose(rect)).unwrap();

            if index == 1 {
                self.children.drain(index - 1 .. index + 2);
            } else {
                self.children.drain(index - 1 .. index + 1);
            }

            self.focus = None;

            if index > 3 {
                let delta_y = rect.height() as i32;

                for i in 2..index-1 {
                    shift(self.child_mut(i), pt!(0, delta_y));
                }
            }
        } else {
            if !enable {
                return;
            }

            let dpi = CURRENT_DEVICE.dpi;
            let (_, height) = CURRENT_DEVICE.dims;
            let &(small_height, big_height) = BAR_SIZES.get(&(height, dpi)).unwrap();
            let thickness = scale_by_dpi(THICKNESS_MEDIUM, dpi) as i32;
            let (small_thickness, big_thickness) = halves(thickness);

            let mut kb_rect = rect![self.rect.min.x,
                                    self.rect.max.y - (small_height + 3 * big_height) as i32 + big_thickness,
                                    self.rect.max.x,
                                    self.rect.max.y - small_height as i32 - small_thickness];

            let number = match id {
                Some(ViewId::GoToPageInput) | Some(ViewId::GoToResultsPageInput) => true,
                _ => false,
            };

            let index = locate::<BottomBar>(self).unwrap_or(0).saturating_sub(1);

            if index == 0 {
                let separator = Filler::new(rect![self.rect.min.x, kb_rect.max.y,
                                                  self.rect.max.x, kb_rect.max.y + thickness],
                                            BLACK);
                self.children.insert(index, Box::new(separator) as Box<View>);
            }

            let keyboard = Keyboard::new(&mut kb_rect, DEFAULT_LAYOUT.clone(), number);
            self.children.insert(index, Box::new(keyboard) as Box<View>);

            let separator = Filler::new(rect![self.rect.min.x, kb_rect.min.y - thickness,
                                              self.rect.max.x, kb_rect.min.y],
                                        BLACK);
            self.children.insert(index, Box::new(separator) as Box<View>);

            if index == 0 {
                for i in index..index+3 {
                    hub.send(Event::Render(*self.child(i).rect(), UpdateMode::Gui)).unwrap();
                }
            } else {
                for i in index..index+2 {
                    hub.send(Event::Render(*self.child(i).rect(), UpdateMode::Gui)).unwrap();
                }

                let delta_y = kb_rect.height() as i32 + thickness;

                for i in 2..index {
                    shift(self.child_mut(i), pt!(0, -delta_y));
                    hub.send(Event::Render(*self.child(i).rect(), UpdateMode::Gui)).unwrap();
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
            self.children.drain(index - 1 .. index + 1);
            hub.send(Event::Expose(rect)).unwrap();
        } else {
            if !enable {
                return;
            }

            let dpi = CURRENT_DEVICE.dpi;
            let (_, height) = CURRENT_DEVICE.dims;
            let &(_, big_height) = BAR_SIZES.get(&(height, dpi)).unwrap();
            let thickness = scale_by_dpi(THICKNESS_MEDIUM, dpi) as i32;
            let doc = self.doc.lock().unwrap();
            let tb_height = if doc.is_reflowable() { 2 * big_height } else { big_height };

            let s_rect = *self.child(2).rect() - pt!(0, tb_height as i32 + thickness);

            let tool_bar = ToolBar::new(rect![self.rect.min.x,
                                              s_rect.max.y,
                                              self.rect.max.x,
                                              s_rect.max.y + tb_height as i32],
                                        doc.is_reflowable(),
                                        self.info.reader.as_ref(),
                                        &context.settings.reader);
            self.children.insert(2, Box::new(tool_bar) as Box<View>);

            let separator = Filler::new(s_rect, BLACK);
            self.children.insert(2, Box::new(separator) as Box<View>);
        }
    }

    fn toggle_results_bar(&mut self, enable: bool, hub: &Hub) {
        if let Some(index) = locate::<ResultsBar>(self) {
            if enable {
                return;
            }

            let mut rect = *self.child(index).rect();
            rect.absorb(self.child(index - 1).rect());
            self.children.drain(index - 1 .. index + 1);
            hub.send(Event::Expose(rect)).unwrap();
        } else {
            if !enable {
                return;
            }

            let dpi = CURRENT_DEVICE.dpi;
            let (_, height) = CURRENT_DEVICE.dims;
            let thickness = scale_by_dpi(THICKNESS_MEDIUM, dpi) as i32;
            let &(small_height, _) = BAR_SIZES.get(&(height, dpi)).unwrap();

            let s_rect = *self.child(2).rect() - pt!(0, thickness + small_height as i32);
            let y_min = s_rect.max.y;
            let mut rect = rect![self.rect.min.x, y_min,
                                 self.rect.max.x, y_min + small_height as i32];

            if let Some(ref s) = self.search {
                let results_bar = ResultsBar::new(rect, s.current_page,
                                                  s.highlights.len(), s.results_count,
                                                  !s.running.load(AtomicOrdering::Relaxed));
                self.children.insert(2, Box::new(results_bar) as Box<View>);
                let separator = Filler::new(s_rect, BLACK);
                self.children.insert(2, Box::new(separator) as Box<View>);
                rect.absorb(&s_rect);
                hub.send(Event::Render(rect, UpdateMode::Gui)).unwrap();
            }
        }
    }

    fn toggle_bars(&mut self, enable: Option<bool>, hub: &Hub, context: &mut Context) {
        if let Some(top_index) = locate::<TopBar>(self) {
            if let Some(true) = enable {
                return;
            }

            if let Some(bottom_index) = locate::<BottomBar>(self) {
                self.children.drain(top_index..bottom_index+1);
                self.focus = None;
                hub.send(Event::Focus(None)).unwrap();
                hub.send(Event::Expose(self.rect)).unwrap();
            }
        } else {
            if let Some(false) = enable {
                return;
            }

            let dpi = CURRENT_DEVICE.dpi;
            let (_, height) = CURRENT_DEVICE.dims;
            let thickness = scale_by_dpi(THICKNESS_MEDIUM, dpi) as i32;
            let (small_thickness, big_thickness) = halves(thickness);
            let &(small_height, big_height) = BAR_SIZES.get(&(height, dpi)).unwrap();

            let mut doc = self.doc.lock().unwrap();
            let mut index = 0;

            let top_bar = TopBar::new(rect![self.rect.min.x, self.rect.min.y,
                                            self.rect.max.x, small_height as i32 - small_thickness],
                                      &self.info,
                                      context);

            self.children.insert(index, Box::new(top_bar) as Box<View>);
            index += 1;

            let separator = Filler::new(rect![self.rect.min.x,
                                              small_height as i32 - small_thickness,
                                              self.rect.max.x,
                                              small_height as i32 + big_thickness],
                                        BLACK);
            self.children.insert(index, Box::new(separator) as Box<View>);
            index += 1;

            if let Some(ref s) = self.search {
                let separator = Filler::new(rect![self.rect.min.x,
                                                  self.rect.max.y - 3 * small_height as i32 - small_thickness,
                                                  self.rect.max.x,
                                                  self.rect.max.y - 3 * small_height as i32 + big_thickness],
                                            BLACK);
                self.children.insert(index, Box::new(separator) as Box<View>);
                index += 1;

                let results_bar = ResultsBar::new(rect![self.rect.min.x,
                                                        self.rect.max.y - 3 * small_height as i32 + big_thickness,
                                                        self.rect.max.x,
                                                        self.rect.max.y - 2 * small_height as i32 - small_thickness],
                                                  s.current_page, s.highlights.len(),
                                                  s.results_count, !s.running.load(AtomicOrdering::Relaxed));
                self.children.insert(index, Box::new(results_bar) as Box<View>);
                index += 1;

                let separator = Filler::new(rect![self.rect.min.x,
                                                  self.rect.max.y - 2 * small_height as i32 - small_thickness,
                                                  self.rect.max.x,
                                                  self.rect.max.y - 2 * small_height as i32 + big_thickness],
                                            BLACK);
                self.children.insert(index, Box::new(separator) as Box<View>);
                index += 1;

                let search_bar = SearchBar::new(rect![self.rect.min.x,
                                                      self.rect.max.y - 2 * small_height as i32 + big_thickness,
                                                      self.rect.max.x,
                                                      self.rect.max.y - small_height as i32 - small_thickness],
                                                "", &s.query);
                self.children.insert(index, Box::new(search_bar) as Box<View>);
                index += 1;
            } else {
                let tb_height = if doc.is_reflowable() { 2 * big_height } else { big_height };
                let separator = Filler::new(rect![self.rect.min.x,
                                                  self.rect.max.y - (small_height + tb_height) as i32 - small_thickness,
                                                  self.rect.max.x,
                                                  self.rect.max.y - (small_height + tb_height) as i32 + big_thickness],
                                            BLACK);
                self.children.insert(index, Box::new(separator) as Box<View>);
                index += 1;

                let tool_bar = ToolBar::new(rect![self.rect.min.x,
                                                  self.rect.max.y - (small_height + tb_height) as i32 + big_thickness,
                                                  self.rect.max.x,
                                                  self.rect.max.y - small_height as i32 - small_thickness],
                                            doc.is_reflowable(),
                                            self.info.reader.as_ref(),
                                            &context.settings.reader);
                self.children.insert(index, Box::new(tool_bar) as Box<View>);
                index += 1;
            }

            let separator = Filler::new(rect![self.rect.min.x,
                                              self.rect.max.y - small_height as i32 - small_thickness,
                                              self.rect.max.x,
                                              self.rect.max.y - small_height as i32 + big_thickness],
                                        BLACK);
            self.children.insert(index, Box::new(separator) as Box<View>);
            index += 1;

            let neighbors = Neighbors {
                previous_page: doc.resolve_location(Location::Previous(self.current_page)),
                next_page: doc.resolve_location(Location::Next(self.current_page)),
            };

            let bottom_bar = BottomBar::new(rect![self.rect.min.x,
                                                  self.rect.max.y - small_height as i32 + big_thickness,
                                                  self.rect.max.x,
                                                  self.rect.max.y],
                                            doc.as_mut(),
                                            self.current_page,
                                            self.pages_count,
                                            &neighbors,
                                            self.synthetic);
            self.children.insert(index, Box::new(bottom_bar) as Box<View>);

            hub.send(Event::Render(self.rect, UpdateMode::Gui)).unwrap();
        }
    }

    fn toggle_go_to_page(&mut self, enable: Option<bool>, id: ViewId, hub: &Hub, fonts: &mut Fonts) {
        let (text, input_id) = if id == ViewId::GoToPage {
            ("Go to page", ViewId::GoToPageInput)
        } else {
            ("Go to results page", ViewId::GoToResultsPageInput)
        };

        if let Some(index) = locate_by_id(self, id) {
            if let Some(true) = enable {
                return;
            }

            hub.send(Event::Expose(*self.child(index).rect())).unwrap();
            self.children.remove(index);

            if self.focus.map(|focus_id| focus_id == input_id).unwrap_or(false) {
                self.toggle_keyboard(false, None, hub);
                hub.send(Event::Focus(None)).unwrap();
            }
        } else {
            if let Some(false) = enable {
                return;
            }

            let go_to_page = NamedInput::new(text.to_string(), id, input_id, 4, fonts);
            hub.send(Event::Render(*go_to_page.rect(), UpdateMode::Gui)).unwrap();
            hub.send(Event::Focus(Some(input_id))).unwrap();

            self.focus = Some(input_id);
            self.children.push(Box::new(go_to_page) as Box<View>);
        }
    }

    fn toggle_font_family_menu(&mut self, rect: Rectangle, enable: Option<bool>, hub: &Hub, context: &mut Context) {
        if let Some(index) = locate_by_id(self, ViewId::FontFamilyMenu) {
            if let Some(true) = enable {
                return;
            }

            hub.send(Event::Expose(*self.child(index).rect())).unwrap();
            self.children.remove(index);
        } else {
            if let Some(false) = enable {
                return;
            }

            let fonts = &mut context.fonts;
            let mut families = family_names(&context.settings.reader.font_path).unwrap_or_default();
            let current_family = self.info.reader.as_ref()
                                     .and_then(|r| r.font_family.clone())
                                     .unwrap_or_else(|| DEFAULT_FONT_FAMILY.to_string());
            families.insert(DEFAULT_FONT_FAMILY.to_string());
            let entries = families.iter().map(|f| EntryKind::RadioButton(f.clone(),
                                                                         EntryId::SetFontFamily(f.clone()),
                                                                         *f == current_family)).collect();
            let font_family_menu = Menu::new(rect, ViewId::FontFamilyMenu, MenuKind::DropDown, entries, fonts);
            hub.send(Event::Render(*font_family_menu.rect(), UpdateMode::Gui)).unwrap();
            self.children.push(Box::new(font_family_menu) as Box<View>);
        }
    }

    fn toggle_font_size_menu(&mut self, rect: Rectangle, enable: Option<bool>, hub: &Hub, fonts: &mut Fonts) {
        if let Some(index) = locate_by_id(self, ViewId::FontSizeMenu) {
            if let Some(true) = enable {
                return;
            }

            hub.send(Event::Expose(*self.child(index).rect())).unwrap();
            self.children.remove(index);
        } else {
            if let Some(false) = enable {
                return;
            }

            let font_size = self.info.reader.as_ref().and_then(|r| r.font_size)
                                .unwrap_or(DEFAULT_FONT_SIZE);
            let entries = (0..=20).map(|v| {
                let fs = 10.0 + v as f32 / 10.0;
                EntryKind::RadioButton(format!("{:.1}", fs),
                                       EntryId::SetFontSize(v),
                                       (fs - font_size).abs() < 0.05)
            }).collect();
            let font_size_menu = Menu::new(rect, ViewId::FontSizeMenu, MenuKind::Contextual, entries, fonts);
            hub.send(Event::Render(*font_size_menu.rect(), UpdateMode::Gui)).unwrap();
            self.children.push(Box::new(font_size_menu) as Box<View>);
        }
    }

    fn toggle_line_height_menu(&mut self, rect: Rectangle, enable: Option<bool>, hub: &Hub, fonts: &mut Fonts) {
        if let Some(index) = locate_by_id(self, ViewId::LineHeightMenu) {
            if let Some(true) = enable {
                return;
            }

            hub.send(Event::Expose(*self.child(index).rect())).unwrap();
            self.children.remove(index);
        } else {
            if let Some(false) = enable {
                return;
            }

            let line_height = self.info.reader.as_ref()
                                  .and_then(|r| r.line_height).unwrap_or(DEFAULT_LINE_HEIGHT);
            let entries = (0..=10).map(|x| {
                let lh = 1.0 + x as f32 / 10.0;
                EntryKind::RadioButton(format!("{:.1}", lh),
                                       EntryId::SetLineHeight(x),
                                       (lh - line_height).abs() < 0.05)
            }).collect();
            let line_height_menu = Menu::new(rect, ViewId::LineHeightMenu, MenuKind::DropDown, entries, fonts);
            hub.send(Event::Render(*line_height_menu.rect(), UpdateMode::Gui)).unwrap();
            self.children.push(Box::new(line_height_menu) as Box<View>);
        }
    }

    fn toggle_margin_width_menu(&mut self, rect: Rectangle, enable: Option<bool>, hub: &Hub, fonts: &mut Fonts) {
        if let Some(index) = locate_by_id(self, ViewId::MarginWidthMenu) {
            if let Some(true) = enable {
                return;
            }

            hub.send(Event::Expose(*self.child(index).rect())).unwrap();
            self.children.remove(index);
        } else {
            if let Some(false) = enable {
                return;
            }

            let margin_width = self.info.reader.as_ref().and_then(|r| r.margin_width).unwrap_or(DEFAULT_MARGIN_WIDTH);
            let entries = (0..=10).map(|mw| EntryKind::RadioButton(format!("{}", mw),
                                                                  EntryId::SetMarginWidth(mw),
                                                                  mw == margin_width)).collect();
            let margin_width_menu = Menu::new(rect, ViewId::MarginWidthMenu, MenuKind::DropDown, entries, fonts);
            hub.send(Event::Render(*margin_width_menu.rect(), UpdateMode::Gui)).unwrap();
            self.children.push(Box::new(margin_width_menu) as Box<View>);
        }
    }

    fn toggle_page_menu(&mut self, rect: Rectangle, enable: Option<bool>, hub: &Hub, fonts: &mut Fonts) {
        if let Some(index) = locate_by_id(self, ViewId::PageMenu) {
            if let Some(true) = enable {
                return;
            }

            hub.send(Event::Expose(*self.child(index).rect())).unwrap();
            self.children.remove(index);
        } else {
            if let Some(false) = enable {
                return;
            }

            let first_page = self.info.reader.as_ref()
                                 .and_then(|r| r.first_page).unwrap_or(0);
            let current_page = self.current_page as usize;
            let entries = vec![EntryKind::CheckBox("First Page".to_string(),
                                                   EntryId::ToggleFirstPage,
                                                   current_page == first_page)];
            let page_menu = Menu::new(rect, ViewId::PageMenu, MenuKind::DropDown, entries, fonts);
            hub.send(Event::Render(*page_menu.rect(), UpdateMode::Gui)).unwrap();
            self.children.push(Box::new(page_menu) as Box<View>);
        }
    }

    fn toggle_margin_cropper_menu(&mut self, rect: Rectangle, enable: Option<bool>, hub: &Hub, fonts: &mut Fonts) {
        if let Some(index) = locate_by_id(self, ViewId::MarginCropperMenu) {
            if let Some(true) = enable {
                return;
            }

            hub.send(Event::Expose(*self.child(index).rect())).unwrap();
            self.children.remove(index);
        } else {
            if let Some(false) = enable {
                return;
            }

            let current_page = self.current_page as usize;
            let is_split = self.info.reader.as_ref()
                               .and_then(|r| r.cropping_margins
                                              .as_ref().map(|c| c.is_split()));

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

            let margin_cropper_menu = Menu::new(rect, ViewId::MarginCropperMenu, MenuKind::DropDown, entries, fonts);
            hub.send(Event::Render(*margin_cropper_menu.rect(), UpdateMode::Gui)).unwrap();
            self.children.push(Box::new(margin_cropper_menu) as Box<View>);
        }
    }

    fn toggle_search_menu(&mut self, rect: Rectangle, enable: Option<bool>, hub: &Hub, fonts: &mut Fonts) {
        if let Some(index) = locate_by_id(self, ViewId::SearchMenu) {
            if let Some(true) = enable {
                return;
            }

            hub.send(Event::Expose(*self.child(index).rect())).unwrap();
            self.children.remove(index);
        } else {
            if let Some(false) = enable {
                return;
            }

            let mut entries = vec![EntryKind::RadioButton("Forward".to_string(),
                                                          EntryId::SearchDirection(LinearDir::Forward),
                                                          self.search_direction == LinearDir::Forward),
                                   EntryKind::RadioButton("Backward".to_string(),
                                                          EntryId::SearchDirection(LinearDir::Backward),
                                                          self.search_direction == LinearDir::Backward)];

            let kind = if locate::<SearchBar>(self).is_some() {
                MenuKind::Contextual
            } else {
                MenuKind::DropDown
            };
            let search_menu = Menu::new(rect, ViewId::SearchMenu, kind, entries, fonts);
            hub.send(Event::Render(*search_menu.rect(), UpdateMode::Gui)).unwrap();
            self.children.push(Box::new(search_menu) as Box<View>);
        }
    }

    fn toggle_search_bar(&mut self, enable: bool, hub: &Hub, context: &mut Context) {
        if locate::<SearchBar>(self).is_some() {
            if enable {
                return;
            }

            self.toggle_bars(Some(false), hub, context);

            if let Some(ref mut s) = self.search {
                s.running.store(false, AtomicOrdering::Relaxed);
            }

            self.search = None;
        } else {
            if !enable {
                return;
            }

            self.toggle_tool_bar(false, hub, context);

            let dpi = CURRENT_DEVICE.dpi;
            let (_, height) = CURRENT_DEVICE.dims;
            let &(small_height, _) = BAR_SIZES.get(&(height, dpi)).unwrap();

            let index = locate::<TopBar>(self).unwrap() + 2;
            let s_rect = *self.child(index).rect();

            let rect = rect![self.rect.min.x,
                             s_rect.min.y - small_height as i32,
                             self.rect.max.x,
                             s_rect.min.y];
            let search_bar = SearchBar::new(rect, "", "");
            self.children.insert(index, Box::new(search_bar) as Box<View>);

            let separator = Filler::new(s_rect - pt!(0, (s_rect.height() + small_height) as i32),
                                        BLACK);
            self.children.insert(index, Box::new(separator) as Box<View>);

            hub.send(Event::Render(*self.child(index).rect(), UpdateMode::Gui)).unwrap();
            hub.send(Event::Render(*self.child(index+1).rect(), UpdateMode::Gui)).unwrap();

            hub.send(Event::Focus(Some(ViewId::SearchInput))).unwrap();
        }
    }

    fn toggle_margin_cropper(&mut self, enable: bool, hub: &Hub, context: &mut Context) {
        if let Some(index) = locate::<MarginCropper>(self) {
            if enable {
                return;
            }

            hub.send(Event::Expose(*self.child(index).rect())).unwrap();
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
                                            .map(|c| c.margin(self.current_page as usize)))
                             .cloned().unwrap_or_default();

            let mut doc = self.doc.lock().unwrap();
            let ((pixmap, location), _) = build_pixmap(&pixmap_rect,
                                                       doc.as_mut(),
                                                       self.current_page,
                                                       &Margin::default());

            self.current_page = location;
            let margin_cropper = MarginCropper::new(self.rect, pixmap, &margin);
            hub.send(Event::Render(*margin_cropper.rect(), UpdateMode::Gui)).unwrap();
            self.children.push(Box::new(margin_cropper) as Box<View>);
        }
    }

    fn set_font_size(&mut self, font_size: f32, hub: &Hub, context: &mut Context) {
        if Arc::strong_count(&self.doc) > 1 {
            return;
        }

        if let Some(ref mut r) = self.info.reader {
            r.font_size = Some(font_size);
        }

        let (width, height) = CURRENT_DEVICE.dims;
        {
            let mut doc = self.doc.lock().unwrap();

            doc.layout(width, height, font_size, CURRENT_DEVICE.dpi);

            if !self.synthetic {
                let ratio = doc.pages_count() / self.pages_count;
                self.pages_count = doc.pages_count();
                self.current_page = (ratio * self.current_page).min(self.pages_count - 1.0);
            }
        }

        self.update(hub);
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

            if !self.synthetic {
                self.pages_count = doc.pages_count();
                self.current_page = self.current_page.min(self.pages_count - 1.0);
            }
        }

        self.update(hub);
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

            if !self.synthetic {
                self.pages_count = doc.pages_count();
                self.current_page = self.current_page.min(self.pages_count - 1.0);
            }
        }

        self.update(hub);
        self.update_tool_bar(hub, context);
        self.update_bottom_bar(hub);
    }

    fn set_margin_width(&mut self, width: i32, hub: &Hub, context: &mut Context) {
        if Arc::strong_count(&self.doc) > 1 {
            return;
        }

        if let Some(ref mut r) = self.info.reader {
            r.margin_width = Some(width);
        }

        {
            let mut doc = self.doc.lock().unwrap();
            doc.set_margin_width(width);

            if !self.synthetic {
                self.pages_count = doc.pages_count();
                self.current_page = self.current_page.min(self.pages_count - 1.0);
            }
        }

        self.update(hub);
        self.update_tool_bar(hub, context);
        self.update_bottom_bar(hub);
    }

    fn add_remove_bookmark(&mut self, hub: &Hub) {
        let current_page = self.current_page;
        if let Some(ref mut r) = self.info.reader {
            if let Ok(index) = r.bookmarks.binary_search_by(|a| a.partial_cmp(&current_page)
                                                                 .unwrap_or(Ordering::Equal)) {
                r.bookmarks.remove(index);
            } else {
                r.bookmarks.push(self.current_page);
                r.bookmarks.sort_unstable_by(|a, b| a.partial_cmp(b)
                                                     .unwrap_or(Ordering::Equal));
            }
        }
        self.update(hub);
    }

    fn crop_margins(&mut self, index: usize, margin: &Margin, hub: &Hub) {
        if let Some(r) = self.info.reader.as_mut() {
            if r.cropping_margins.is_none() {
                r.cropping_margins = Some(CroppingMargins::Any(Margin::default()));
            }
            for c in r.cropping_margins.iter_mut() {
                *c.margin_mut(index) = margin.clone();
            }
        }
        self.update(hub);
    }

    fn reseed(&mut self, hub: &Hub, context: &mut Context) {
        let (tx, _rx) = mpsc::channel();
        if let Some(index) = locate::<TopBar>(self) {
            self.child_mut(index).downcast_mut::<TopBar>().unwrap()
                .update_frontlight_icon(&tx, context);
            hub.send(Event::ClockTick).unwrap();
            hub.send(Event::BatteryTick).unwrap();
        }
        hub.send(Event::Render(self.rect, UpdateMode::Gui)).unwrap();
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
        }

        for i in &mut context.metadata {
            if i.file.path == self.info.file.path {
                *i = self.info.clone();
                break;
            }
        }
    }
}

impl View for Reader {
    fn handle_event(&mut self, evt: &Event, hub: &Hub, _bus: &mut Bus, context: &mut Context) -> bool {
        match *evt {
            Event::Gesture(GestureEvent::Swipe { dir, ref start, .. }) if self.rect.includes(*start) => {
                match dir {
                    Dir::West => self.go_to_neighbor(CycleDir::Next, hub, context),
                    Dir::East => self.go_to_neighbor(CycleDir::Previous, hub, context),
                    _ => (),
                };
                true
            },
            Event::Gesture(GestureEvent::Tap(ref center)) if self.rect.includes(*center) => {
                if self.focus.is_some() {
                    return true;
                }

                let dx = (self.rect.width() - self.frame.width()) as i32 / 2;
                let dy = (self.rect.height() - self.frame.height()) as i32 / 2;

                let (links, _) = self.doc.lock().ok()
                                     .and_then(|mut doc| doc.links(Location::Exact(self.current_page)))
                                     .unwrap_or((Vec::new(), 0.0));

                for link in links {
                    let r = link.rect;
                    let x_min = r.min.x as f32 * self.scale;
                    let y_min = r.min.y as f32 * self.scale;
                    let x_max = r.max.x as f32 * self.scale;
                    let y_max = r.max.y as f32 * self.scale;
                    let rect = rect![x_min as i32 - self.frame.min.x + dx,
                                     y_min as i32 - self.frame.min.y + dy,
                                     x_max as i32 - self.frame.min.x + dx,
                                     y_max as i32 - self.frame.min.y + dy];

                    if rect.includes(*center) {
                        let pdf_page = Regex::new(r"^#(\d+)(?:,\d+,\d+)?$").unwrap();
                        let toc_page = Regex::new(r"^@(.*)$").unwrap();
                        if let Some(caps) = toc_page.captures(&link.text) {
                            if let Ok(location) = caps[1].parse::<f32>() {
                                hub.send(Event::Back).unwrap();
                                hub.send(Event::GoTo(location)).unwrap();
                            }
                        } else if let Some(caps) = pdf_page.captures(&link.text) {
                            if let Ok(index) = caps[1].parse::<usize>() {
                                self.go_to_page(index.saturating_sub(1) as f32, true, hub);
                            }
                        } else {
                            println!("Unrecognized URI: {}.", link.text);
                        }
                        return true;
                    }
                }

                let w = self.rect.width() as i32;
                let x1 = self.rect.min.x + w / 3;
                let x2 = self.rect.max.x - w / 3;

                if center.x < x1 {
                    let dx = x1 - center.x;
                    // Top left corner.
                    if center.y < self.rect.min.y + dx {
                        self.go_to_last_page(hub);
                    // Bottom left corner.
                    } else if center.y > self.rect.max.y - dx {
                        if self.search.is_none() {
                            if self.ephemeral {
                                hub.send(Event::Back).unwrap();
                            } else {
                                hub.send(Event::Show(ViewId::TableOfContents)).unwrap();
                            }
                        } else {
                            self.go_to_neighbor(CycleDir::Previous, hub, context);
                        }
                    // Left ear.
                    } else {
                        if self.search.is_none() {
                            self.go_to_neighbor(CycleDir::Previous, hub, context);
                        } else {
                            self.go_to_results_neighbor(CycleDir::Previous, hub);
                        }
                    }
                } else if center.x > x2 {
                    let dx = center.x - x2;
                    // Top right corner.
                    if center.y < self.rect.min.y + dx {
                        self.add_remove_bookmark(hub);
                    // Bottom right corner.
                    } else if center.y > self.rect.max.y - dx {
                        if self.search.is_none() {
                            hub.send(Event::Toggle(ViewId::GoToPage)).unwrap();
                        } else {
                            self.go_to_neighbor(CycleDir::Next, hub, context);
                        }
                    // Right ear.
                    } else {
                        if self.search.is_none() {
                            self.go_to_neighbor(CycleDir::Next, hub, context);
                        } else {
                            self.go_to_results_neighbor(CycleDir::Next, hub);
                        }
                    }
                // Middle band.
                } else {
                    self.toggle_bars(None, hub, context);
                }

                true
            },
            Event::Gesture(GestureEvent::HoldFinger(ref center)) if self.rect.includes(*center) => {
                if self.focus.is_some() {
                    return true;
                }

                let w = self.rect.width() as i32;
                let x1 = self.rect.min.x + w / 3;
                let x2 = self.rect.max.x - w / 3;

                if center.x < x1 {
                    let dx = x1 - center.x;
                    // Top left corner.
                    if center.y < self.rect.min.y + dx {
                        self.go_to_bookmark(CycleDir::Previous, hub);
                    // Bottom left corner.
                    } else if center.y > self.rect.max.y - dx {
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
                            hub.send(Event::ToggleFrontlight).unwrap();
                        }
                    // Left ear.
                    } else {
                        if self.search.is_none() {
                            self.go_to_chapter(CycleDir::Previous, hub);
                        } else {
                            self.go_to_results_page(0, hub);
                        }
                    }
                } else if center.x > x2 {
                    let dx = center.x - x2;
                    // Top right corner.
                    if center.y < self.rect.min.y + dx {
                        self.go_to_bookmark(CycleDir::Next, hub);
                    // Bottom right corner.
                    } else if center.y > self.rect.max.y - dx {
                        hub.send(Event::Select(EntryId::ToggleInverted)).unwrap();
                    // Right ear.
                    } else {
                        if self.search.is_none() {
                            self.go_to_chapter(CycleDir::Next, hub);
                        } else {
                            let last_page = self.search.as_ref().unwrap().highlights.len() - 1;
                            self.go_to_results_page(last_page, hub);
                        }
                    }
                } else {
                    hub.send(Event::Render(self.rect, UpdateMode::Full)).unwrap();
                }

                true
            },
            Event::Submit(ViewId::GoToPageInput, ref text) => {
                let re = Regex::new(r#"^(")?(.+)$"#).unwrap();
                if let Some(caps) = re.captures(text) {
                    if let Ok(mut location) = caps[2].parse::<f32>() {
                        if !self.synthetic {
                            location = (location - 1.0).max(0.0);
                            if caps.get(1).is_some() {
                                location += self.info.reader.as_ref()
                                                .and_then(|r| r.first_page).unwrap_or(0) as f32;
                            }
                        }
                        self.go_to_page(location, true, hub);
                    }
                }
                true
            },
            Event::Submit(ViewId::GoToResultsPageInput, ref text) => {
                if let Ok(index) = text.parse::<usize>() {
                    self.go_to_results_page(index.saturating_sub(1), hub);
                }
                true
            },
            Event::Submit(ViewId::SearchInput, ref text) => {
                match make_query(text) {
                    Some(query) => {
                        self.search(text, query, hub);
                        self.toggle_keyboard(false, None, hub);
                        self.toggle_results_bar(true, hub);
                    },
                    None => {
                        let notif = Notification::new(ViewId::InvalidSearchQueryNotif,
                                                      "Invalid search query.".to_string(),
                                                      &mut context.notification_index,
                                                      &mut context.fonts,
                                                      hub);
                        self.children.push(Box::new(notif) as Box<View>);
                    }
                }
                true
            },
            Event::Page(dir) => {
                self.go_to_neighbor(dir, hub, context);
                true
            },
            Event::GoTo(location) => {
                self.go_to_page(location, true, hub);
                true
            },
            Event::Chapter(dir) => {
                self.go_to_chapter(dir, hub);
                true
            },
            Event::ResultsPage(dir) => {
                self.go_to_results_neighbor(dir, hub);
                true
            },
            Event::CropMargins(ref margin) => {
                let current_page = self.current_page as usize;
                self.crop_margins(current_page, margin.as_ref(), hub);
                true
            },
            Event::Toggle(ViewId::TopBottomBars) => {
                self.toggle_bars(None, hub, context);
                true
            },
            Event::Toggle(ViewId::GoToPage) => {
                self.toggle_go_to_page(None, ViewId::GoToPage, hub, &mut context.fonts);
                true
            },
            Event::Toggle(ViewId::GoToResultsPage) => {
                self.toggle_go_to_page(None, ViewId::GoToResultsPage, hub, &mut context.fonts);
                true
            },
            Event::Slider(SliderId::FontSize, font_size, FingerStatus::Up) => {
                self.set_font_size(font_size, hub, context);
                true
            },
            Event::ToggleNear(ViewId::MainMenu, rect) => {
                toggle_main_menu(self, rect, None, hub, context);
                true
            },
            Event::ToggleNear(ViewId::MarginCropperMenu, rect) => {
                self.toggle_margin_cropper_menu(rect, None, hub, &mut context.fonts);
                true
            },
            Event::ToggleNear(ViewId::SearchMenu, rect) => {
                self.toggle_search_menu(rect, None, hub, &mut context.fonts);
                true
            },
            Event::ToggleNear(ViewId::FontFamilyMenu, rect) => {
                self.toggle_font_family_menu(rect, None, hub, context);
                true
            },
            Event::ToggleNear(ViewId::FontSizeMenu, rect) => {
                self.toggle_font_size_menu(rect, None, hub, &mut context.fonts);
                true
            },
            Event::ToggleNear(ViewId::MarginWidthMenu, rect) => {
                self.toggle_margin_width_menu(rect, None, hub, &mut context.fonts);
                true
            },
            Event::ToggleNear(ViewId::LineHeightMenu, rect) => {
                self.toggle_line_height_menu(rect, None, hub, &mut context.fonts);
                true
            },
            Event::ToggleNear(ViewId::PageMenu, rect) => {
                self.toggle_page_menu(rect, None, hub, &mut context.fonts);
                true
            },
            Event::Close(ViewId::MainMenu) => {
                toggle_main_menu(self, Rectangle::default(), Some(false), hub, context);
                true
            },
            Event::Close(ViewId::SearchBar) => {
                self.toggle_search_bar(false, hub, context);
                true
            },
            Event::Close(ViewId::GoToPage) => {
                self.toggle_go_to_page(Some(false), ViewId::GoToPage, hub, &mut context.fonts);
                true
            },
            Event::Close(ViewId::GoToResultsPage) => {
                self.toggle_go_to_page(Some(false), ViewId::GoToResultsPage, hub, &mut context.fonts);
                true
            },
            Event::Show(ViewId::TableOfContents) => {
                {
                    self.toggle_bars(Some(false), hub, context);
                }
                let mut doc = self.doc.lock().unwrap();
                if doc.has_toc() {
                    hub.send(Event::OpenToc(doc.toc().unwrap(), self.current_page)).unwrap();
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
            Event::SearchResult(location, rect) => {
                if self.search.is_none() {
                    return true;
                }

                let mut results_count = 0;

                if let Some(ref mut s) = self.search {
                    let pages_count = s.highlights.len();
                    let search_page = s.highlights
                                       .binary_search_by(|a| a.location.partial_cmp(&location)
                                                              .unwrap_or(Ordering::Equal));
                    if let Ok(index) = search_page {
                        s.highlights[index].rects.push(rect);
                    } else {
                        s.highlights.push(Highlight { location, rects: vec![rect] });
                        s.highlights.sort_unstable_by(|a, b| a.location.partial_cmp(&b.location)
                                                              .unwrap_or(Ordering::Equal));
                    }

                    s.results_count += 1;
                    results_count = s.results_count;
                    if results_count > 1 && location <= self.current_page && s.highlights.len() > pages_count {
                        s.current_page += 1;
                    }
                }

                self.update_results_bar(hub);

                if results_count == 1 {
                    self.go_to_page(location, true, hub);
                    self.toggle_bars(Some(false), hub, context);
                } else if (location - self.current_page).abs() < LOCATION_EPSILON {
                    self.update(hub);
                }

                true
            },
            Event::EndOfSearch => {
                let results_count = self.search.as_ref().map(|s| s.results_count)
                                        .unwrap_or(usize::max_value());
                if results_count == 0 {
                    let notif = Notification::new(ViewId::NoSearchResultsNotif,
                                                  "No search results.".to_string(),
                                                  &mut context.notification_index,
                                                  &mut context.fonts,
                                                  hub);
                    self.children.push(Box::new(notif) as Box<View>);
                    self.toggle_bars(Some(true), hub, context);
                    hub.send(Event::Focus(Some(ViewId::SearchInput))).unwrap();
                }
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
                self.update(hub);
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
            Event::Select(EntryId::SetFontSize(v)) => {
                let font_size = 10.0 + v as f32 / 10.0;
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
            Event::Select(EntryId::ToggleFirstPage) => {
                let current_page = self.current_page as usize;
                if let Some(ref mut r) = self.info.reader {
                    if r.first_page.unwrap_or(0) == current_page {
                        r.first_page = None;
                    } else {
                        r.first_page = Some(current_page);
                    }
                }
                true
            },
            Event::Reseed => {
                self.reseed(hub, context);
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
                if let Some(ViewId::SearchInput) = v {
                    self.toggle_results_bar(false, hub);
                    if let Some(ref mut s) = self.search {
                        s.running.store(false, AtomicOrdering::Relaxed);
                    }
                    self.search = None;
                }
                self.focus = v;
                if v.is_some() {
                    self.toggle_keyboard(true, v, hub);
                }
                true
            },
            _ => false,
        }
    }

    fn render(&self, fb: &mut Framebuffer, _fonts: &mut Fonts) {
        let dx = (self.rect.width() - self.frame.width()) as i32 / 2;
        let dy = (self.rect.height() - self.frame.height()) as i32 / 2;

        fb.draw_rectangle(&self.rect, WHITE);
        fb.draw_framed_pixmap(&self.pixmap, &self.frame, &pt!(dx, dy));

        if let Some(rects) = self.search.as_ref()
                                 .and_then(|s| s.highlights.binary_search_by(|a| a.location.partial_cmp(&self.current_page).unwrap_or(Ordering::Equal)).ok()
                                 .map(|index| &s.highlights[index].rects)) {
            let dx = (self.rect.width() - self.frame.width()) as i32 / 2;
            let dy = (self.rect.height() - self.frame.height()) as i32 / 2;

            for r in rects {
                let x_min = r.min.x as f32 * self.scale;
                let y_min = r.min.y as f32 * self.scale;
                let x_max = r.max.x as f32 * self.scale;
                let y_max = r.max.y as f32 * self.scale;
                let rect = rect![x_min as i32 - self.frame.min.x + dx,
                                 y_min as i32 - self.frame.min.y + dy,
                                 x_max as i32 - self.frame.min.x + dx,
                                 y_max as i32 - self.frame.min.y + dy];

                if let Some(ref it) = rect.intersection(&fb.rect()) {
                    fb.invert_region(it);
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

    fn is_background(&self) -> bool {
        true
    }

    fn rect(&self) -> &Rectangle {
        &self.rect
    }

    fn rect_mut(&mut self) -> &mut Rectangle {
        &mut self.rect
    }

    fn children(&self) -> &Vec<Box<View>> {
        &self.children
    }

    fn children_mut(&mut self) -> &mut Vec<Box<View>> {
        &mut self.children
    }
}

fn build_pixmap(rect: &Rectangle, doc: &mut Document, location: f32, margin: &Margin) -> ((Pixmap, f32), f32) {
    let (width, height) = doc.dims(location as usize).unwrap();
    let p_width = (1.0 - (margin.left + margin.right)) * width;
    let p_height = (1.0 - (margin.top + margin.bottom)) * height;
    let w_ratio = rect.width() as f32 / p_width;
    let h_ratio = rect.height() as f32 / p_height;
    let scale = w_ratio.min(h_ratio);
    (doc.pixmap(Location::Exact(location), scale).unwrap(), scale)
}
