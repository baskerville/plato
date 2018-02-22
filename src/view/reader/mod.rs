mod top_bar;
mod tool_bar;
mod bottom_bar;
mod margin_cropper;

use std::rc::Rc;
use std::path::PathBuf;
use chrono::Local;
use regex::Regex;
use input::FingerStatus;
use framebuffer::{Framebuffer, UpdateMode, Pixmap};
use view::{View, Event, Hub, ViewId, EntryKind, EntryId, SliderId, Bus, THICKNESS_MEDIUM};
use unit::{scale_by_dpi, pt_to_px};
use device::{CURRENT_DEVICE, BAR_SIZES};
use font::{Fonts, DEFAULT_FONT_SIZE};
use self::margin_cropper::{MarginCropper, BUTTON_DIAMETER};
use self::top_bar::TopBar;
use self::tool_bar::ToolBar;
use self::bottom_bar::BottomBar;
use view::common::{locate, locate_by_id, toggle_main_menu};
use view::filler::Filler;
use view::named_input::NamedInput;
use view::keyboard::{Keyboard, DEFAULT_LAYOUT};
use view::menu::{Menu, MenuKind};
use gesture::GestureEvent;
use document::{Document, TocEntry, open, toc_as_html, chapter_at, chapter_relative};
use document::pdf::PdfOpener;
use metadata::{Info, FileInfo, ReaderInfo, Margin};
use geom::{Rectangle, Dir, CycleDir, halves};
use color::{BLACK, WHITE};
use app::Context;

pub struct Reader {
    rect: Rectangle,
    children: Vec<Box<View>>,
    info: Info,
    doc: Box<Document>,
    pixmap: Rc<Pixmap>,
    current_page: usize,
    pages_count: usize,
    page_turns: usize,
    finished: bool,
    ephemeral: bool,
    refresh_every: Option<u8>,
    frame: Rectangle,
    scale: f32,
    focus: Option<ViewId>,
}

impl Reader {
    pub fn new(rect: Rectangle, mut info: Info, hub: &Hub, context: &mut Context) -> Option<Reader> {
        let settings = &context.settings;
        let path = settings.library_path.join(&info.file.path);

        open(&path).map(|mut doc| {
            let (width, height) = CURRENT_DEVICE.dims;
            let font_size = info.reader.as_ref().and_then(|r| r.font_size);
            doc.layout(width as f32, height as f32,
                       pt_to_px(font_size.unwrap_or(DEFAULT_FONT_SIZE),
                                CURRENT_DEVICE.dpi));

            let pages_count;
            let current_page;

            // TODO: use get_or_insert_with?
            if let Some(ref mut r) = info.reader {
                r.opened = Local::now();
                if r.finished {
                    r.finished = false;
                    r.current_page = 0;
                }
                current_page = r.current_page;
                pages_count = r.pages_count;
            } else {
                current_page = 0;
                pages_count = doc.pages_count();
                info.reader = Some(ReaderInfo {
                    current_page,
                    pages_count,
                    .. Default::default()
                });
            }

            println!("{}", info.file.path.display());

            let margin = info.reader.as_ref()
                             .and_then(|r| r.margin_at(current_page))
                             .cloned().unwrap_or_default();
            let (pixmap, scale) = build_pixmap(&rect, doc.as_ref(), current_page, &margin);
            let frame = rect![(margin.left * pixmap.width as f32).ceil() as i32,
                              (margin.top * pixmap.height as f32).ceil() as i32,
                              ((1.0 - margin.right) * pixmap.width as f32).floor() as i32,
                              ((1.0 - margin.bottom) * pixmap.height as f32).floor() as i32];
            let pixmap = Rc::new(pixmap);

            hub.send(Event::Render(rect, UpdateMode::Partial)).unwrap();

            Reader {
                rect,
                children: vec![],
                info,
                doc,
                pixmap,
                current_page,
                pages_count,
                page_turns: 0,
                finished: false,
                ephemeral: false,
                refresh_every: settings.refresh_every,
                frame,
                scale,
                focus: None,
            }
        })
    }

    pub fn from_toc(rect: Rectangle, toc: &[TocEntry], mut current_page: usize, hub: &Hub, context: &mut Context) -> Reader {
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
        let doc = opener.open_memory("html", html.as_bytes()).unwrap();
        let pages_count = doc.pages_count();

        current_page = chapter_at(toc, current_page).and_then(|chap| {
            let link_uri = format!("@{}", chap.page);
            (0..pages_count).find(|index| doc.links(*index).as_ref()
                                             .and_then(|links| links.iter()
                                                                    .find(|link| link.uri == link_uri))
                                             .is_some())}).unwrap_or(0);

        let (pixmap, scale) = build_pixmap(&rect, &doc, current_page, &Margin::default());
        let pixmap = Rc::new(pixmap);
        let frame = rect![0, 0, pixmap.width, pixmap.height];

        hub.send(Event::Render(rect, UpdateMode::Partial)).unwrap();

        Reader {
            rect,
            children: vec![],
            info,
            doc: Box::new(doc) as Box<Document>,
            pixmap,
            current_page,
            pages_count,
            page_turns: 0,
            finished: false,
            ephemeral: true,
            refresh_every: context.settings.refresh_every,
            frame,
            scale,
            focus: None,
        }
    }

    fn go_to_page(&mut self, index: usize, hub: &Hub) {
        if index >= self.pages_count {
            return;
        }
        self.current_page = index;
        self.update(hub);
        self.update_bottom_bar(hub);
    }

    fn go_to_chapter(&mut self, dir: CycleDir, hub: &Hub) {
        let current_page = self.current_page;
        if let Some(index) = self.doc.toc().and_then(|t| chapter_relative(&t, current_page, dir)) {
            self.go_to_page(index, hub);
        }
    }

    fn set_current_page(&mut self, dir: CycleDir, hub: &Hub) {
        match dir {
            CycleDir::Next if self.current_page < self.pages_count - 1 => {
                self.current_page += 1;
                self.update(hub);
                self.update_bottom_bar(hub);
            },
            CycleDir::Previous if self.current_page > 0 => {
                self.current_page -= 1;
                self.update(hub);
                self.update_bottom_bar(hub);
            },
            CycleDir::Next if self.current_page == self.pages_count - 1 => {
                // TODO: create popup, or close?
                self.finished = true;
            },
            _ => (),
        }
    }

    fn update_bottom_bar(&mut self, hub: &Hub) {
        if let Some(index) = locate::<BottomBar>(self) {
            let current_page = self.current_page;
            let bottom_bar = self.children[index].as_mut().downcast_mut::<BottomBar>().unwrap();
            bottom_bar.update_page_label(self.current_page, self.pages_count, hub);
            bottom_bar.update_icons(self.current_page, self.pages_count, hub);
            let chapter = self.doc.toc().as_ref().and_then(|t| chapter_at(&t, current_page))
                                        .map(|c| c.title.clone())
                                        .unwrap_or_default();
            bottom_bar.update_chapter(chapter, hub);
        }
    }

    fn update(&mut self, hub: &Hub) {
        self.page_turns += 1;
        let update_mode = if let Some(n) = self.refresh_every {
            if self.page_turns % (n as usize) == 0 {
                UpdateMode::Full
            } else {
                UpdateMode::Partial
            }
        } else {
            UpdateMode::Partial
        };
        let margin = self.info.reader.as_ref()
                         .and_then(|r| r.margin_at(self.current_page))
                         .cloned().unwrap_or_default();
        let (pixmap, scale) = build_pixmap(&self.rect, self.doc.as_ref(), self.current_page, &margin);
        self.pixmap = Rc::new(pixmap);
        let frame = rect![(margin.left * self.pixmap.width as f32).ceil() as i32,
                          (margin.top * self.pixmap.height as f32).ceil() as i32,
                          ((1.0 - margin.right) * self.pixmap.width as f32).floor() as i32,
                          ((1.0 - margin.bottom) * self.pixmap.height as f32).floor() as i32];
        self.frame = frame;
        self.scale = scale;
        hub.send(Event::Render(self.rect, update_mode)).unwrap();
    }

    fn toggle_keyboard(&mut self, enable: bool, id: Option<ViewId>, hub: &Hub) {
        if let Some(index) = locate::<Keyboard>(self) {
            if enable {
                return;
            }

            let mut kb_rect = *self.child(index).rect();
            kb_rect.absorb(self.child(index-1).rect());
            hub.send(Event::Expose(kb_rect)).unwrap();
            self.children.drain(index - 1 .. index + 1);
        } else {
            if !enable {
                return;
            }

            let index = locate::<BottomBar>(self).unwrap() - 1;

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
                Some(ViewId::GoToPageInput) => true,
                _ => false,
            };

            let keyboard = Keyboard::new(&mut kb_rect, DEFAULT_LAYOUT.clone(), number);
            self.children.insert(index, Box::new(keyboard) as Box<View>);

            let separator = Filler::new(rect![self.rect.min.x, kb_rect.min.y - thickness,
                                              self.rect.max.x, kb_rect.min.y],
                                        BLACK);
            self.children.insert(index, Box::new(separator) as Box<View>);

            for i in index..index+2 {
                hub.send(Event::Render(*self.child(i).rect(), UpdateMode::Gui)).unwrap();
            }
        }
    }

    fn toggle_bars(&mut self, context: &mut Context) {
        if let Some(index) = locate::<TopBar>(self) {
            self.children.drain(index..index+6);
        } else {
            let dpi = CURRENT_DEVICE.dpi;
            let (_, height) = CURRENT_DEVICE.dims;
            let thickness = scale_by_dpi(THICKNESS_MEDIUM, dpi) as i32;
            let (small_thickness, big_thickness) = halves(thickness);
            let &(small_height, big_height) = BAR_SIZES.get(&(height, dpi)).unwrap();

            let top_bar = TopBar::new(rect![self.rect.min.x, self.rect.min.y,
                                            self.rect.max.x, small_height as i32 - small_thickness],
                                      &self.info,
                                      context);

            self.children.push(Box::new(top_bar) as Box<View>);

            let separator = Filler::new(rect![self.rect.min.x,
                                              small_height as i32 - small_thickness,
                                              self.rect.max.x,
                                              small_height as i32 + big_thickness],
                                        BLACK);
            self.children.push(Box::new(separator) as Box<View>);

            let separator = Filler::new(rect![self.rect.min.x,
                                              self.rect.max.y - (small_height + big_height) as i32 - small_thickness,
                                              self.rect.max.x,
                                              self.rect.max.y - (small_height + big_height) as i32 + big_thickness],
                                        BLACK);
            self.children.push(Box::new(separator) as Box<View>);

            let font_size = self.info.reader.as_ref().and_then(|r| r.font_size);
            let tool_bar = ToolBar::new(rect![self.rect.min.x,
                                              self.rect.max.y - (small_height + big_height) as i32 + big_thickness,
                                              self.rect.max.x,
                                              self.rect.max.y - small_height as i32 - small_thickness],
                                        self.doc.is_reflowable(),
                                        font_size.unwrap_or(DEFAULT_FONT_SIZE));
            self.children.push(Box::new(tool_bar) as Box<View>);

            let separator = Filler::new(rect![self.rect.min.x,
                                              self.rect.max.y - small_height as i32 - small_thickness,
                                              self.rect.max.x,
                                              self.rect.max.y - small_height as i32 + big_thickness],
                                        BLACK);
            self.children.push(Box::new(separator) as Box<View>);

            let bottom_bar = BottomBar::new(rect![self.rect.min.x,
                                                  self.rect.max.y - small_height as i32 + big_thickness,
                                                  self.rect.max.x,
                                                  self.rect.max.y],
                                            self.doc.as_ref(),
                                            self.current_page,
                                            self.pages_count);
            self.children.push(Box::new(bottom_bar) as Box<View>);
        }
    }

    fn toggle_go_to_page(&mut self, enable: Option<bool>, hub: &Hub, fonts: &mut Fonts) {
        if let Some(index) = locate_by_id(self, ViewId::GoToPage) {
            if let Some(true) = enable {
                return;
            }

            hub.send(Event::Expose(*self.child(index).rect())).unwrap();
            self.children.remove(index);

            if let Some(ViewId::GoToPageInput) = self.focus {
                self.toggle_keyboard(false, Some(ViewId::GoToPageInput), hub);
                self.focus = None;
            }
        } else {
            if let Some(false) = enable {
                return;
            }

            let go_to_page = NamedInput::new("Go to page".to_string(), ViewId::GoToPage, 4, ViewId::GoToPageInput, fonts);
            hub.send(Event::Render(*go_to_page.rect(), UpdateMode::Gui)).unwrap();
            hub.send(Event::Focus(Some(ViewId::GoToPageInput))).unwrap();

            self.focus = Some(ViewId::GoToPageInput);
            self.children.push(Box::new(go_to_page) as Box<View>);
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
            let entries = vec![EntryKind::CheckBox("First Page".to_string(),
                                                   EntryId::ToggleFirstPage,
                                                   self.current_page == first_page)];
            let page_menu = Menu::new(rect, ViewId::PageMenu, MenuKind::DropDown, entries, fonts);
            hub.send(Event::Render(*page_menu.rect(), UpdateMode::Gui)).unwrap();
            self.children.push(Box::new(page_menu) as Box<View>);
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

            self.toggle_bars(context);

            let dpi = CURRENT_DEVICE.dpi;
            let padding = scale_by_dpi(BUTTON_DIAMETER / 2.0, dpi) as i32;
            let pixmap_rect = rect![self.rect.min + pt!(padding),
                                    self.rect.max - pt!(padding)];

            let margin = self.info.reader.as_ref()
                             .and_then(|r| r.margin_at(self.current_page))
                             .cloned().unwrap_or_default();

            let (pixmap, _) = build_pixmap(&pixmap_rect,
                                           self.doc.as_ref(),
                                           self.current_page,
                                           &Margin::default());

            let margin_cropper = MarginCropper::new(self.rect, pixmap, &margin);
            hub.send(Event::Render(*margin_cropper.rect(), UpdateMode::Gui)).unwrap();
            self.children.push(Box::new(margin_cropper) as Box<View>);
        }
    }

    fn set_font_size(&mut self, font_size: f32, hub: &Hub) {
        if let Some(ref mut r) = self.info.reader {
            r.font_size = Some(font_size);
        }
        let (width, height) = CURRENT_DEVICE.dims;
        self.doc.layout(width as f32, height as f32,
                        pt_to_px(font_size,
                                 CURRENT_DEVICE.dpi));
        let ratio = self.doc.pages_count() as f32 / self.pages_count as f32;
        self.current_page = (self.current_page as f32 * ratio) as usize;
        if let Some(ref mut first_page) = self.info.reader.as_mut().and_then(|r| r.first_page) {
            *first_page = (*first_page as f32 * ratio) as usize;
        }
        self.pages_count = self.doc.pages_count();
        self.update(hub);
        self.update_bottom_bar(hub);
    }

    fn crop_margins(&mut self, margin: &Margin, hub: &Hub) {
        if let Some(ref mut r) = self.info.reader {
            r.cropping_margins.insert(self.current_page, margin.clone());
        }
        self.update(hub);
    }

    fn reseed(&mut self, hub: &Hub, context: &mut Context) {
        if let Some(index) = locate::<TopBar>(self) {
            self.child_mut(index).downcast_mut::<TopBar>().unwrap()
                .update_frontlight_icon(hub, context);
        }

        hub.send(Event::ClockTick).unwrap();
        hub.send(Event::BatteryTick).unwrap();
        hub.send(Event::Render(self.rect, UpdateMode::Gui)).unwrap();
    }

    fn quit(&mut self, context: &mut Context) {
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
            Event::Gesture(GestureEvent::Swipe { dir, ref start, .. }) if self.rect.includes(start) => {
                match dir {
                    Dir::West => self.set_current_page(CycleDir::Next, hub),
                    Dir::East => self.set_current_page(CycleDir::Previous, hub),
                    _ => (),
                };
                true
            },
            Event::Gesture(GestureEvent::HoldFinger(ref center)) if self.rect.includes(center) => {
                let w = self.rect.width() as i32;
                let x1 = self.rect.min.x + w / 3;
                let x2 = self.rect.max.x - w / 3;
                if center.x < x1 {
                    self.go_to_chapter(CycleDir::Previous, hub);
                } else if center.x > x2 {
                    self.go_to_chapter(CycleDir::Next, hub);
                } else {
                    hub.send(Event::Render(self.rect, UpdateMode::Full)).unwrap();
                }
                true
            },
            Event::Gesture(GestureEvent::Tap { ref center, .. }) if self.rect.includes(center) => {
                let dx = (self.rect.width() - self.frame.width()) as i32 / 2;
                let dy = (self.rect.height() - self.frame.height()) as i32 / 2;

                for link in &self.doc.links(self.current_page).unwrap_or_default() {
                    let r = link.rect;
                    let x_min = r.min.x as f32 * self.scale;
                    let y_min = r.min.y as f32 * self.scale;
                    let x_max = r.max.x as f32 * self.scale;
                    let y_max = r.max.y as f32 * self.scale;
                    let rect = rect![x_min as i32 - self.frame.min.x + dx,
                                     y_min as i32 - self.frame.min.y + dy,
                                     x_max as i32 - self.frame.min.x + dx,
                                     y_max as i32 - self.frame.min.y + dy];
                    if rect.includes(center) {
                        let re = Regex::new(r"^([#@])(\d+)$").unwrap();
                        if let Some(caps) = re.captures(&link.uri) {
                            if let Ok(index) = caps[2].parse::<usize>() {
                                if &caps[1] == "@" {
                                    hub.send(Event::Back).unwrap();
                                    hub.send(Event::Toggle(ViewId::TopBottomBars)).unwrap();
                                    hub.send(Event::GoTo(index)).unwrap();
                                } else {
                                    self.go_to_page(index.saturating_sub(1), hub);
                                }
                            }
                        } else {
                            println!("Unrecognized URI: {}.", link.uri);
                        }
                        return true;
                    }
                }

                let w = self.rect.width() as i32;
                let x1 = self.rect.min.x + w / 3;
                let x2 = self.rect.max.x - w / 3;
                if center.x < x1 {
                    self.set_current_page(CycleDir::Previous, hub);
                } else if center.x > x2 {
                    self.set_current_page(CycleDir::Next, hub);
                } else {
                    self.toggle_bars(context);
                    hub.send(Event::Render(self.rect, UpdateMode::Gui)).unwrap();
                }
                true
            },
            Event::Submit(ViewId::GoToPageInput, ref text) => {
                let re = Regex::new(r#"^(")?(\d+)$"#).unwrap();
                if let Some(caps) = re.captures(text) {
                    if let Ok(mut index) = caps[2].parse::<usize>() {
                        index = index.saturating_sub(1);
                        if caps.get(1).is_some() {
                            index = index.saturating_add(self.info.reader.as_ref()
                                                             .and_then(|r| r.first_page).unwrap_or(0))
                                         .min(self.pages_count - 1);
                        }
                        self.go_to_page(index, hub);
                    }
                }
                true
            },
            Event::Page(dir) => {
                self.set_current_page(dir, hub);
                true
            },
            Event::GoTo(index) => {
                self.go_to_page(index, hub);
                true
            },
            Event::Chapter(dir) => {
                self.go_to_chapter(dir, hub);
                true
            },
            Event::CropMargins(ref margin) => {
                self.crop_margins(margin.as_ref(), hub);
                true
            },
            Event::Toggle(ViewId::TopBottomBars) => {
                self.toggle_bars(context);
                hub.send(Event::Render(self.rect, UpdateMode::Gui)).unwrap();
                true
            },
            Event::Toggle(ViewId::GoToPage) => {
                self.toggle_go_to_page(None, hub, &mut context.fonts);
                true
            },
            Event::Slider(SliderId::FontSize, font_size, FingerStatus::Up) => {
                self.set_font_size(font_size, hub);
                true
            },
            Event::ToggleNear(ViewId::MainMenu, rect) => {
                toggle_main_menu(self, rect, None, hub, context);
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
            Event::Close(ViewId::GoToPage) => {
                self.toggle_go_to_page(Some(false), hub, &mut context.fonts);
                true
            },
            Event::Show(ViewId::TableOfContents) => {
                if self.doc.has_toc() {
                    hub.send(Event::OpenToc(self.doc.toc().unwrap(), self.current_page)).unwrap();
                }
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
            Event::Select(EntryId::ToggleFirstPage) => {
                let current_page = self.current_page;
                if let Some(ref mut r) = self.info.reader.as_mut() {
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
                self.focus = v;
                self.toggle_keyboard(true, v, hub);
                false
            },
            _ => false,
        }
    }

    fn render(&self, fb: &mut Framebuffer, _fonts: &mut Fonts) {
        let dx = (self.rect.width() - self.frame.width()) as i32 / 2;
        let dy = (self.rect.height() - self.frame.height()) as i32 / 2;
        fb.draw_rectangle(&self.rect, WHITE);
        fb.draw_framed_pixmap(&self.pixmap, &self.frame, &pt!(dx, dy));
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

fn build_pixmap(rect: &Rectangle, doc: &Document, index: usize, margin: &Margin) -> (Pixmap, f32) {
    let (width, height) = doc.dims(index).unwrap();
    let p_width = (1.0 - (margin.left + margin.right)) * width;
    let p_height = (1.0 - (margin.top + margin.bottom)) * height;
    let w_ratio = rect.width() as f32 / p_width;
    let h_ratio = rect.height() as f32 / p_height;
    let scale = w_ratio.min(h_ratio);
    (doc.pixmap(index, scale).unwrap(), scale)
}
