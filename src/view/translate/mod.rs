mod bottom_bar;

use crate::device::CURRENT_DEVICE;
use crate::framebuffer::{Framebuffer, UpdateMode, Pixmap};
use crate::geom::{Rectangle, Point, Dir, CycleDir, halves};
use crate::unit::scale_by_dpi;
use crate::font::Fonts;
use crate::view::{View, Event, Hub, Bus, RenderQueue, RenderData};
use crate::view::{ViewId, Id, ID_FEEDER, EntryId, EntryKind};
use crate::view::{SMALL_BAR_HEIGHT, BIG_BAR_HEIGHT, THICKNESS_MEDIUM};
use crate::document::{Document, Location};
use crate::document::html::HtmlDocument;
use crate::view::common::{locate, locate_by_id};
use crate::view::common::{toggle_main_menu, toggle_battery_menu, toggle_clock_menu};
use crate::gesture::GestureEvent;
use crate::input::{DeviceEvent, ButtonCode, ButtonStatus};
use crate::color::BLACK;
use crate::app::Context;
use crate::view::filler::Filler;
use crate::view::image::Image;
use crate::view::keyboard::Keyboard;
use crate::view::menu::{Menu, MenuKind};
use crate::view::search_bar::SearchBar;
use crate::view::top_bar::TopBar;
use self::bottom_bar::BottomBar;
use crate::translate;
use crate::helpers::{first_n_words, trim_non_alphanumeric};

const VIEWER_STYLESHEET: &str = "css/translate.css";
const USER_STYLESHEET: &str = "css/translate-user.css";

pub struct Translate {
    id: Id,
    rect: Rectangle,
    children: Vec<Box<dyn View>>,
    doc: HtmlDocument,
    location: usize,
    source: String,
    query: String,
    target: String,
    active: bool,
    wifi: bool,
    is_stand_alone: bool,
    focus: Option<ViewId>,
}

impl Translate {
    pub fn new(rect: Rectangle, query: &str, source: &str, target: &str, hub: &Hub, rq: &mut RenderQueue, context: &mut Context) -> Translate {
        let id = ID_FEEDER.next();
        let mut children = Vec::new();
        let dpi = CURRENT_DEVICE.dpi;
        let small_height = scale_by_dpi(SMALL_BAR_HEIGHT, dpi) as i32;
        let thickness = scale_by_dpi(THICKNESS_MEDIUM, dpi) as i32;
        let (small_thickness, big_thickness) = halves(thickness);

        let top_bar = TopBar::new(rect![rect.min.x, rect.min.y,
                                        rect.max.x, rect.min.y + small_height - small_thickness],
                                  Event::Back,
                                  "Google Translate".to_string(),
                                  context);
        children.push(Box::new(top_bar) as Box<dyn View>);

        let separator = Filler::new(rect![rect.min.x, rect.min.y + small_height - small_thickness,
                                          rect.max.x, rect.min.y + small_height + big_thickness],
                                    BLACK);
        children.push(Box::new(separator) as Box<dyn View>);

        let image_rect = rect![rect.min.x, rect.min.y + small_height + big_thickness,
                               rect.max.x, rect.max.y - small_height - small_thickness];

        let image = Image::new(image_rect, Pixmap::new(1, 1));
        children.push(Box::new(image) as Box<dyn View>);

        let mut doc = HtmlDocument::new_from_memory("");
        doc.layout(image_rect.width(), image_rect.height(), context.settings.dictionary.font_size, dpi);
        doc.set_margin_width(context.settings.dictionary.margin_width);
        doc.set_viewer_stylesheet(VIEWER_STYLESHEET);
        doc.set_user_stylesheet(USER_STYLESHEET);

        let separator = Filler::new(rect![rect.min.x, rect.max.y - small_height - small_thickness,
                                          rect.max.x, rect.max.y - small_height + big_thickness],
                                    BLACK);
        children.push(Box::new(separator) as Box<dyn View>);

        let bottom_bar = BottomBar::new(rect![rect.min.x, rect.max.y - small_height + big_thickness,
                                              rect.max.x, rect.max.y],
                                              &format!("Translate from:  {}", source),
                                              &format!("to:  {}", target),
                                              false, false);
        children.push(Box::new(bottom_bar) as Box<dyn View>);

        let wifi = context.settings.wifi;
        let is_stand_alone = query.is_empty();

        rq.add(RenderData::new(id, rect, UpdateMode::Gui));

        if is_stand_alone {
            hub.send(Event::Show(ViewId::SearchBar)).ok();
        } else {
            hub.send(Event::Proceed).ok();
        }

        Translate {
            id,
            rect,
            children,
            doc,
            location: 0,
            query: query.to_string(),
            source: source.to_string(),
            target: target.to_string(),
            active: false,
            wifi,
            is_stand_alone,
            focus: None,
        }

    }

    fn toggle_source_lang_menu(&mut self, rect: Rectangle, enable: Option<bool>, rq: &mut RenderQueue, context: &mut Context) {
        if let Some(index) = locate_by_id(self, ViewId::SourceLangMenu) {
            if let Some(true) = enable {
                return;
            }

            rq.add(RenderData::expose(*self.child(index).rect(), UpdateMode::Gui));
            self.children.remove(index);
        } else {
            if let Some(false) = enable {
                return;
            }
            let langs = &context.settings.languages;
            let mut entries = langs.iter().rev()
                                   .map(|x| EntryKind::RadioButton(x.to_string(),
                                                                   EntryId::SetSourceLang(x.to_string()),
                                                                   self.source == x.to_string()))
                                   .collect::<Vec<EntryKind>>();
            entries.push(EntryKind::Separator);
            entries.push(EntryKind::RadioButton("auto".to_string(),
                                                EntryId::SetSourceLang("auto".to_string()),
                                                self.source == "auto".to_string()));
            let source_lang_menu = Menu::new(rect, ViewId::SourceLangMenu, MenuKind::DropDown, entries, context);
            rq.add(RenderData::new(source_lang_menu.id(), *source_lang_menu.rect(), UpdateMode::Gui));
            self.children.push(Box::new(source_lang_menu) as Box<dyn View>);
        }
    }

    fn toggle_target_lang_menu(&mut self, rect: Rectangle, enable: Option<bool>, rq: &mut RenderQueue, context: &mut Context) {
        if let Some(index) = locate_by_id(self, ViewId::TargetLangMenu) {
            if let Some(true) = enable {
                return;
            }

            rq.add(RenderData::expose(*self.child(index).rect(), UpdateMode::Gui));
            self.children.remove(index);
        } else {
            if let Some(false) = enable {
                return;
            }
            let langs = &context.settings.languages;
            let entries = langs.iter().rev()
                               .map(|x| EntryKind::RadioButton(x.to_string(),
                                                               EntryId::SetTargetLang(x.to_string()),
                                                               self.target == x.to_string()))
                               .collect::<Vec<EntryKind>>();
            let target_lang_menu = Menu::new(rect, ViewId::TargetLangMenu, MenuKind::DropDown, entries, context);
            rq.add(RenderData::new(target_lang_menu.id(), *target_lang_menu.rect(), UpdateMode::Gui));
            self.children.push(Box::new(target_lang_menu) as Box<dyn View>);
        }
    }

    fn toggle_search_bar(&mut self, enable: Option<bool>, hub: &Hub, rq: &mut RenderQueue, context: &mut Context) {
        if let Some(index) = locate::<SearchBar>(self) {
            if let Some(true) = enable {
                return;
            }

            let mut rect = *self.child(index).rect();
            rect.absorb(self.child(index-1).rect()); // top sep
            rect.absorb(self.child(index+1).rect()); // kbd's sep
            rect.absorb(self.child(index+2).rect()); // kbd
            self.children.drain(index - 1 ..= index + 2);
            rq.add(RenderData::expose(rect, UpdateMode::Gui));
            hub.send(Event::Focus(None)).ok();
        } else {
            if let Some(false) = enable {
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

            let index = locate::<BottomBar>(self).unwrap();

            let keyboard = Keyboard::new(&mut kb_rect, false, context);
            self.children.insert(index, Box::new(keyboard) as Box<dyn View>);

            let separator = Filler::new(rect![self.rect.min.x, kb_rect.min.y - thickness,
                                              self.rect.max.x, kb_rect.min.y],
                                        BLACK);
            self.children.insert(index, Box::new(separator) as Box<dyn View>);


            let sp_rect = rect![self.rect.min.x, kb_rect.min.y - small_height - small_thickness,
                                self.rect.max.x, kb_rect.min.y - small_height + big_thickness];
            let y_min = sp_rect.max.y;
            let rect = rect![self.rect.min.x, y_min,
                             self.rect.max.x, y_min + small_height - thickness];
            let search_bar = SearchBar::new(rect,
                                            ViewId::TranslateSearchInput,
                                            "",
                                            &first_n_words(&self.query, 10),
                                            false,
                                            context);
            self.children.insert(index, Box::new(search_bar) as Box<dyn View>);

            let separator = Filler::new(sp_rect, BLACK);
            self.children.insert(index, Box::new(separator) as Box<dyn View>);

            for i in index..index+4 {  // 4 items added
                rq.add(RenderData::new(self.child(i).id(), *self.child(i).rect(), UpdateMode::Gui));
            }
            hub.send(Event::Focus(Some(ViewId::TranslateSearchInput))).ok();
        }
    }

    fn translate(&mut self, rq: &mut RenderQueue, context: &mut Context) {
        let res = translate::translate(&self.query, &self.source, &self.target, context);
        match res {
            Ok((content, lang)) => {
                if let Some(index) = locate::<TopBar>(self) {
                    let top_bar = self.children[index].as_mut().downcast_mut::<TopBar>().unwrap();
                    let label = if self.source != lang
                                    { format!("Detected language:  {}", lang) }
                                else
                                    { "Google Translate".to_string() };
                    top_bar.update_title_label(&label, rq);
                }
                self.doc.update(&content);
            }
            Err(e) => self.doc.update(&format!("<h2>Error</h2><p>{:?}</p>", e)),
        }
        self.go_to_location(Location::Exact(0), rq);
        self.active = false;
    }

    fn go_to_neighbor(&mut self, dir: CycleDir, hub: &Hub, rq: &mut RenderQueue) {
        let location = match dir {
            CycleDir::Previous => Location::Previous(self.location),
            CycleDir::Next => Location::Next(self.location),
        };
        if let Some(loc) = self.doc.resolve_location(location) {
            self.go_to_location(Location::Exact(loc), rq);
        } else {
            if self.is_stand_alone {
                match dir {
                    CycleDir::Previous => self.go_to_location(Location::Exact(std::usize::MAX), rq),
                    CycleDir::Next => self.go_to_location(Location::Exact(0), rq),
                }
            } else {
                hub.send(Event::Back).ok();
            }
        }
    }

    fn go_to_location(&mut self, location: Location, rq: &mut RenderQueue) {
        if let Some(image) = self.children[2].downcast_mut::<Image>() {
            if let Some((pixmap, loc)) = self.doc.pixmap(location, 1.0) {
                image.update(pixmap, rq);
                self.location = loc;
            }
        }
        if let Some(index) = locate::<BottomBar>(self) {
            let bottom_bar = self.children[index].downcast_mut::<BottomBar>().unwrap();
            bottom_bar.update_icons(self.doc.resolve_location(Location::Previous(self.location)).is_some(),
                                    self.doc.resolve_location(Location::Next(self.location)).is_some(), rq);
        }
    }

    fn underlying_word(&mut self, pt: Point) -> Option<String> {
        let dpi = CURRENT_DEVICE.dpi;
        let small_height = scale_by_dpi(SMALL_BAR_HEIGHT, dpi) as i32;
        let thickness = scale_by_dpi(THICKNESS_MEDIUM, dpi) as i32;
        let (_, big_thickness) = halves(thickness);
        let offset = pt!(self.rect.min.x, self.rect.min.y + small_height + big_thickness);

        if let Some((words, _)) = self.doc.words(Location::Exact(self.location)) {
            for word in words {
                let rect = word.rect.to_rect() + offset;
                if rect.includes(pt) {
                    return Some(word.text)
                }
            }
        }

        None
    }

}

impl View for Translate {
    fn handle_event(&mut self, evt: &Event, hub: &Hub, _bus: &mut Bus, rq: &mut RenderQueue, context: &mut Context) -> bool {
        match *evt {
            Event::Device(DeviceEvent::NetUp) => {
                if self.active {
                    self.translate(rq, context);
                }
                true
            },
            Event::Proceed => {
                self.active = true;
                if context.online {
                    self.translate(rq, context);
                } else {
                    if !context.settings.wifi {
                        hub.send(Event::SetWifi(true)).ok();
                    }
                    hub.send(Event::Notify("Waiting for network connection.".to_string())).ok();
                }
                true
            },
            Event::Submit(ViewId::TranslateSearchInput, ref text) => {
                if !text.trim().is_empty() {
                    self.toggle_search_bar(Some(false), hub, rq, context);
                    self.query = text.trim().to_string();
                    hub.send(Event::Proceed).ok();
                }
                true
            },
            Event::Gesture(GestureEvent::HoldFingerLong(pt, _)) => {
                if let Some(text) = self.underlying_word(pt) {
                    self.query = trim_non_alphanumeric(&text);
                    hub.send(Event::Proceed).ok();
                }
                true
            },
            Event::Select(EntryId::SetSourceLang(ref source)) => {
                if *source != self.source {
                    self.source = source.clone();
                    if let Some(index) = locate::<BottomBar>(self) {
                        let bottom_bar = self.children[index].downcast_mut::<BottomBar>().unwrap();
                        bottom_bar.update_source(&format!("Translate from:  {}", self.source), rq);
                    }
                    hub.send(Event::Proceed).ok();
                }
                true
            },
            Event::Select(EntryId::SetTargetLang(ref target)) => {
                if *target != self.target {
                    self.target = target.clone();
                    if let Some(index) = locate::<BottomBar>(self) {
                        let bottom_bar = self.children[index].downcast_mut::<BottomBar>().unwrap();
                        bottom_bar.update_target(&format!("to:  {}", self.target), rq);
                    }
                    hub.send(Event::Proceed).ok();
                }
                true
            },
            Event::Page(dir) => {
                self.go_to_neighbor(dir, hub,  rq);
                true
            },
            Event::Gesture(GestureEvent::Swipe { dir, start, .. }) if self.rect.includes(start) => {
                match dir {
                    Dir::West => self.go_to_neighbor(CycleDir::Next, hub,  rq),
                    Dir::East => self.go_to_neighbor(CycleDir::Previous, hub,  rq),
                    _ => (),
                }
                true
            },
            Event::Device(DeviceEvent::Button { code, status: ButtonStatus::Released, .. }) => {
                match code {
                    ButtonCode::Backward => self.go_to_neighbor(CycleDir::Previous, hub, rq),
                    ButtonCode::Forward => self.go_to_neighbor(CycleDir::Next, hub, rq),
                    _ => (),
                }
                true
            },
            Event::Gesture(GestureEvent::Tap(center)) if self.rect.includes(center) => {
                if self.focus.is_some() {
                    self.toggle_search_bar(Some(false), hub, rq, context);
                } else {
                    let fifth_width = self.rect.width() as i32 / 5;
                    if center.x < 2 * fifth_width {
                        self.go_to_neighbor(CycleDir::Previous, hub, rq);
                    } else if center.x > 3 * fifth_width {
                        self.go_to_neighbor(CycleDir::Next, hub, rq);
                    }
                }
                true
            },
            Event::ToggleNear(ViewId::SourceLangMenu, rect) => {
                self.toggle_source_lang_menu(rect, None, rq, context);
                true
            },
            Event::ToggleNear(ViewId::TargetLangMenu, rect) => {
                self.toggle_target_lang_menu(rect, None, rq, context);
                true
            },
            Event::Show(ViewId::SearchBar) => {
                self.toggle_search_bar(None, hub, rq, context);
                true
            }
            Event::Close(ViewId::SearchBar) => {
                self.toggle_search_bar(Some(false), hub, rq, context);
                true
            }
            Event::Focus(v) => {
                self.focus = v;
                true
            },
            Event::ToggleNear(ViewId::MainMenu, rect) => {
                toggle_main_menu(self, rect, None, rq, context);
                true
            },
            Event::ToggleNear(ViewId::BatteryMenu, rect) => {
                toggle_battery_menu(self, rect, None, rq, context);
                true
            },
            Event::ToggleNear(ViewId::ClockMenu, rect) => {
                toggle_clock_menu(self, rect, None, rq, context);
                true
            },
            Event::Gesture(GestureEvent::Cross(_)) => {
                hub.send(Event::Back).ok();
                true
            },
            Event::Back => {
                if !self.wifi {
                    hub.send(Event::SetWifi(false)).ok();
                }
                false
            },
            _ => false,
        }
    }

    fn render(&self, _fb: &mut dyn Framebuffer, _rect: Rectangle, _fonts: &mut Fonts) {
    }

    fn resize(&mut self, rect: Rectangle, hub: &Hub, rq: &mut RenderQueue, context: &mut Context) {

        self.toggle_search_bar(Some(false), hub, rq, context);

        let dpi = CURRENT_DEVICE.dpi;
        let small_height = scale_by_dpi(SMALL_BAR_HEIGHT, dpi) as i32;
        let thickness = scale_by_dpi(THICKNESS_MEDIUM, dpi) as i32;
        let (small_thickness, big_thickness) = halves(thickness);

        self.children[0].resize(rect![rect.min.x, rect.min.y,
                                      rect.max.x, rect.min.y + small_height - small_thickness],
                                hub, rq, context);

        self.children[1].resize(rect![rect.min.x, rect.min.y + small_height - small_thickness,
                                      rect.max.x, rect.min.y + small_height + big_thickness],
                                hub, rq, context);

        let image_rect = rect![rect.min.x, rect.min.y + small_height + big_thickness,
                               rect.max.x, rect.max.y - small_height - small_thickness];

        self.doc.layout(image_rect.width(), image_rect.height(), context.settings.dictionary.font_size, dpi);

        if let Some(image) = self.children[2].downcast_mut::<Image>() {
            if let Some((pixmap, loc)) = self.doc.pixmap(Location::Exact(self.location), 1.0) {
                image.update(pixmap, &mut RenderQueue::new());
                self.location = loc;
            }
        }
        self.children[2].resize(image_rect, hub, rq, context);

        self.children[3].resize(rect![rect.min.x, rect.max.y - small_height - small_thickness,
                                      rect.max.x, rect.max.y - small_height + big_thickness],
                                hub, rq, context);

        self.children[4].resize(rect![rect.min.x, rect.max.y - small_height + big_thickness,
                                      rect.max.x, rect.max.y],
                                hub, rq, context);
        if let Some(bottom_bar) = self.children[4].downcast_mut::<BottomBar>() {
            bottom_bar.update_icons(self.doc.resolve_location(Location::Previous(self.location)).is_some(),
                                    self.doc.resolve_location(Location::Next(self.location)).is_some(), &mut RenderQueue::new());
        }
        self.rect = rect;
        rq.add(RenderData::new(self.id, self.rect, UpdateMode::Full));

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

    fn id(&self) -> Id {
        self.id
    }
}
