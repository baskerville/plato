mod bottom_bar;

use regex::Regex;
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
use crate::view::common::{locate_by_id, locate};
use crate::view::common::{toggle_main_menu, toggle_battery_menu, toggle_clock_menu};
use crate::gesture::GestureEvent;
use crate::color::BLACK;
use crate::app::Context;
use crate::view::filler::Filler;
use crate::view::named_input::NamedInput;
use crate::view::image::Image;
use crate::view::keyboard::Keyboard;
use crate::view::menu::{Menu, MenuKind};
use crate::view::search_bar::SearchBar;
use crate::view::top_bar::TopBar;
use self::bottom_bar::BottomBar;

const VIEWER_STYLESHEET: &str = "css/dictionary.css";
const USER_STYLESHEET: &str = "css/dictionary-user.css";

pub struct Dictionary {
    id: Id,
    rect: Rectangle,
    children: Vec<Box<dyn View>>,
    doc: HtmlDocument,
    location: usize,
    fuzzy: bool,
    query: String,
    language: String,
    target: Option<String>,
    focus: Option<ViewId>,
}

fn query_to_content(query: &str, language: &String, fuzzy: bool, target: Option<&String>, context: &mut Context) -> String {
    let mut content = String::new();

    for (name, dict) in context.dictionaries.iter_mut() {
        if target.is_some() && target != Some(name) {
            continue;
        }

        if target.is_none() && !language.is_empty() &&
           context.settings.dictionary.languages.contains_key(name) &&
           !context.settings.dictionary.languages[name].contains(language) {
            continue;
        }

        if let Some(results) = dict.lookup(query, fuzzy)
                                   .map_err(|e| eprintln!("Can't search dictionary: {:#}.", e))
                                   .ok().filter(|r| !r.is_empty()) {

            if target.is_none() {
                content.push_str(&format!("<h1 class=\"dictname\">{}</h1>\n", name.replace('<', "&lt;").replace('>', "&gt;")));
            }
            for [head, body] in results {
                if !body.trim_start().starts_with("<h2") {
                    content.push_str(&format!("<h2 class=\"headword\">{}</h2>\n", head.replace('<', "&lt;").replace('>', "&gt;")));
                }
                if body.trim_start().starts_with('<') {
                    content.push_str(&body);
                } else {
                    content.push_str(&format!("<pre>{}</pre>", body.replace('<', "&lt;").replace('>', "&gt;")));
                }
            }
        }
    }

    if content.is_empty() {
        if context.dictionaries.is_empty() {
            content.push_str("<p class=\"info\">No dictionaries present.</p>");
        } else {
            content.push_str("<p class=\"info\">No definitions found.</p>");
        }
    }

    content
}

impl Dictionary {
    pub fn new(rect: Rectangle, query: &str, language: &str, hub: &Hub, rq: &mut RenderQueue, context: &mut Context) -> Dictionary {
        let id = ID_FEEDER.next();
        let mut children = Vec::new();
        let dpi = CURRENT_DEVICE.dpi;
        let small_height = scale_by_dpi(SMALL_BAR_HEIGHT, dpi) as i32;
        let thickness = scale_by_dpi(THICKNESS_MEDIUM, dpi) as i32;
        let (small_thickness, big_thickness) = halves(thickness);

        let top_bar = TopBar::new(rect![rect.min.x, rect.min.y,
                                        rect.max.x, rect.min.y + small_height - small_thickness],
                                  Event::Back,
                                  "Dictionary".to_string(),
                                  context);
        children.push(Box::new(top_bar) as Box<dyn View>);

        let separator = Filler::new(rect![rect.min.x, rect.min.y + small_height - small_thickness,
                                          rect.max.x, rect.min.y + small_height + big_thickness],
                                    BLACK);
        children.push(Box::new(separator) as Box<dyn View>);

        let search_bar = SearchBar::new(rect![rect.min.x, rect.min.y + small_height + big_thickness,
                                              rect.max.x, rect.min.y + 2 * small_height - small_thickness],
                                        ViewId::DictionarySearchInput,
                                        "", query, context);
        children.push(Box::new(search_bar) as Box<dyn View>);

        let separator = Filler::new(rect![rect.min.x, rect.min.y + 2 * small_height - small_thickness,
                                          rect.max.x, rect.min.y + 2 * small_height + big_thickness],
                                    BLACK);
        children.push(Box::new(separator) as Box<dyn View>);

        let langs = &context.settings.dictionary.languages;
        let matches = context.dictionaries.keys()
                             .filter(|&k| langs.contains_key(k) && langs[k].contains(&language.to_string()))
                             .collect::<Vec<&String>>();
        let target = if matches.len() == 1 {
            Some(matches[0].clone())
        } else {
            if context.dictionaries.len() == 1 {
                Some(context.dictionaries.keys().next().cloned().unwrap())
            } else {
                None
            }
        };

        let image_rect = rect![rect.min.x, rect.min.y + 2 * small_height + big_thickness,
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
                                        target.as_deref().unwrap_or("All"), false, false);
        children.push(Box::new(bottom_bar) as Box<dyn View>);

        rq.add(RenderData::new(id, rect, UpdateMode::Gui));

        if query.is_empty() {
            hub.send(Event::Focus(Some(ViewId::DictionarySearchInput))).ok();
        } else {
            hub.send(Event::Define(query.to_string())).ok();
        }

        Dictionary {
            id,
            rect,
            children,
            doc,
            location: 0,
            fuzzy: false,
            query: query.to_string(),
            language: language.to_string(),
            target,
            focus: None,
        }
    }

    pub fn toggle_title_menu(&mut self, rect: Rectangle, enable: Option<bool>, rq: &mut RenderQueue, context: &mut Context) {
        if let Some(index) = locate_by_id(self, ViewId::TitleMenu) {
            if let Some(true) = enable {
                return;
            }

            rq.add(RenderData::expose(*self.child(index).rect(), UpdateMode::Gui));
            self.children.remove(index);
        } else {
            if let Some(false) = enable {
                return;
            }
            let entries = vec![EntryKind::Command("Reload Dictionaries".to_string(), EntryId::ReloadDictionaries)];
            let title_menu = Menu::new(rect, ViewId::TitleMenu, MenuKind::DropDown, entries, context);
            rq.add(RenderData::new(title_menu.id(), *title_menu.rect(), UpdateMode::Gui));
            self.children.push(Box::new(title_menu) as Box<dyn View>);
        }
    }

    fn toggle_search_menu(&mut self, rect: Rectangle, enable: Option<bool>, rq: &mut RenderQueue, context: &mut Context) {
        if let Some(index) = locate_by_id(self, ViewId::SearchMenu) {
            if let Some(true) = enable {
                return;
            }

            rq.add(RenderData::expose(*self.child(index).rect(), UpdateMode::Gui));
            self.children.remove(index);
        } else {
            if let Some(false) = enable {
                return;
            }
            let entries = vec![EntryKind::CheckBox("Fuzzy".to_string(),
                                                   EntryId::ToggleFuzzy,
                                                   self.fuzzy)];
            let search_menu = Menu::new(rect, ViewId::SearchMenu, MenuKind::Contextual, entries, context);
            rq.add(RenderData::new(search_menu.id(), *search_menu.rect(), UpdateMode::Gui));
            self.children.push(Box::new(search_menu) as Box<dyn View>);
        }
    }

    fn toggle_search_target_menu(&mut self, rect: Rectangle, enable: Option<bool>, rq: &mut RenderQueue, context: &mut Context) {
        if let Some(index) = locate_by_id(self, ViewId::SearchTargetMenu) {
            if let Some(true) = enable {
                return;
            }

            rq.add(RenderData::expose(*self.child(index).rect(), UpdateMode::Gui));
            self.children.remove(index);
        } else {
            if let Some(false) = enable {
                return;
            }
            let mut entries = context.dictionaries.keys()
                                     .map(|k| EntryKind::RadioButton(k.to_string(),
                                                                     EntryId::SetSearchTarget(Some(k.to_string())),
                                                                     self.target == Some(k.to_string())))
                                     .collect::<Vec<EntryKind>>();
            if !entries.is_empty() {
                entries.push(EntryKind::Separator);
            }
            entries.push(EntryKind::RadioButton("All".to_string(),
                                                EntryId::SetSearchTarget(None),
                                                self.target.is_none()));
            let search_target_menu = Menu::new(rect, ViewId::SearchTargetMenu, MenuKind::DropDown, entries, context);
            rq.add(RenderData::new(search_target_menu.id(), *search_target_menu.rect(), UpdateMode::Gui));
            self.children.push(Box::new(search_target_menu) as Box<dyn View>);
        }
    }

    fn toggle_keyboard(&mut self, enable: bool, id: Option<ViewId>, hub: &Hub, rq: &mut RenderQueue, context: &mut Context) {
        if let Some(index) = locate::<Keyboard>(self) {
            if enable {
                return;
            }

            let mut rect = *self.child(index).rect();
            rect.absorb(self.child(index-1).rect());
            self.children.drain(index - 1 ..= index);

            rq.add(RenderData::expose(rect, UpdateMode::Gui));
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

            let number = id == Some(ViewId::GoToPageInput);
            let index = locate::<BottomBar>(self).unwrap() + 1;

            let keyboard = Keyboard::new(&mut kb_rect, number, context);
            self.children.insert(index, Box::new(keyboard) as Box<dyn View>);

            let separator = Filler::new(rect![self.rect.min.x, kb_rect.min.y - thickness,
                                              self.rect.max.x, kb_rect.min.y],
                                        BLACK);
            self.children.insert(index, Box::new(separator) as Box<dyn View>);

            for i in index..=index+1 {
                rq.add(RenderData::new(self.child(i).id(), *self.child(i).rect(), UpdateMode::Gui));
            }
        }
    }

    fn toggle_edit_languages(&mut self, enable: Option<bool>, hub: &Hub, rq: &mut RenderQueue, context: &mut Context) {
        if let Some(index) = locate_by_id(self, ViewId::EditLanguages) {
            if let Some(true) = enable {
                return;
            }

            rq.add(RenderData::expose(*self.child(index).rect(), UpdateMode::Gui));
            self.children.remove(index);

            if self.focus.map(|focus_id| focus_id == ViewId::EditLanguagesInput).unwrap_or(false) {
                self.toggle_keyboard(false, None, hub, rq, context);
            }
        } else {
            if let Some(false) = enable {
                return;
            }

            let mut edit_languages = NamedInput::new("Languages".to_string(), ViewId::EditLanguages,
                                                     ViewId::EditLanguagesInput, 16, context);
            if let Some(langs) = self.target.as_ref()
                                     .and_then(|name| context.settings.dictionary.languages.get(name))
                                     .filter(|langs| !langs.is_empty()) {
                edit_languages.set_text(&langs.join(", "), &mut RenderQueue::new(), context);
            }

            rq.add(RenderData::new(edit_languages.id(), *edit_languages.rect(), UpdateMode::Gui));
            hub.send(Event::Focus(Some(ViewId::EditLanguagesInput))).ok();

            self.children.push(Box::new(edit_languages) as Box<dyn View>);
        }
    }

    fn reseed(&mut self, rq: &mut RenderQueue, context: &mut Context) {
        if let Some(top_bar) = self.child_mut(0).downcast_mut::<TopBar>() {
            top_bar.reseed(rq, context);
        }

        rq.add(RenderData::new(self.id, self.rect, UpdateMode::Gui));
    }

    fn go_to_neighbor(&mut self, dir: CycleDir, rq: &mut RenderQueue) {
        let location = match dir {
            CycleDir::Previous => Location::Previous(self.location),
            CycleDir::Next => Location::Next(self.location),
        };
        if let Some(image) = self.children[4].downcast_mut::<Image>() {
            if let Some((pixmap, loc)) = self.doc.pixmap(location, 1.0) {
                image.update(pixmap, rq);
                self.location = loc;
            }
        }
        if let Some(bottom_bar) = self.children[6].downcast_mut::<BottomBar>() {
            bottom_bar.update_icons(self.doc.resolve_location(Location::Previous(self.location)).is_some(),
                                    self.doc.resolve_location(Location::Next(self.location)).is_some(), rq);
        }
    }

    fn define(&mut self, text: Option<&str>, rq: &mut RenderQueue, context: &mut Context) {
        if let Some(query) = text {
            self.query = query.to_string();
            if let Some(search_bar) = self.children[2].downcast_mut::<SearchBar>() {
                search_bar.set_text(query, rq, context);
            }
        }
        let content = query_to_content(&self.query, &self.language, self.fuzzy, self.target.as_ref(), context);
        self.doc.update(&content);
        if let Some(image) = self.children[4].downcast_mut::<Image>() {
            if let Some((pixmap, loc)) = self.doc.pixmap(Location::Exact(0), 1.0) {
                image.update(pixmap, rq);
                self.location = loc;
            }
        }
        if let Some(bottom_bar) = self.children[6].downcast_mut::<BottomBar>() {
            bottom_bar.update_icons(false, self.doc.resolve_location(Location::Next(self.location)).is_some(), rq);
        }
    }

    fn underlying_word(&mut self, pt: Point) -> Option<String> {
        let dpi = CURRENT_DEVICE.dpi;
        let small_height = scale_by_dpi(SMALL_BAR_HEIGHT, dpi) as i32;
        let thickness = scale_by_dpi(THICKNESS_MEDIUM, dpi) as i32;
        let (_, big_thickness) = halves(thickness);
        let offset = pt!(self.rect.min.x, self.rect.min.y + 2 * small_height + big_thickness);

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

    fn follow_link(&mut self, pt: Point, rq: &mut RenderQueue, context: &mut Context) {
        let dpi = CURRENT_DEVICE.dpi;
        let small_height = scale_by_dpi(SMALL_BAR_HEIGHT, dpi) as i32;
        let thickness = scale_by_dpi(THICKNESS_MEDIUM, dpi) as i32;
        let (_, big_thickness) = halves(thickness);
        let offset = pt!(self.rect.min.x, self.rect.min.y + 2 * small_height + big_thickness);

        if let Some((links, _)) = self.doc.links(Location::Exact(self.location)) {
            for link in links {
                let rect = link.rect.to_rect() + offset;
                if rect.includes(pt) && link.text.starts_with('?') {
                    self.define(Some(&link.text[1..]), rq, context);
                    return;
                }
            }
        }

        let half_width = self.rect.width() as i32 / 2;
        if pt.x - offset.x < half_width {
            self.go_to_neighbor(CycleDir::Previous, rq);
        } else {
            self.go_to_neighbor(CycleDir::Next, rq);
        }
    }
}

impl View for Dictionary {
    fn handle_event(&mut self, evt: &Event, hub: &Hub, _bus: &mut Bus, rq: &mut RenderQueue, context: &mut Context) -> bool {
        match *evt {
            Event::Define(ref query) => {
                self.define(Some(query), rq, context);
                true
            },
            Event::Submit(ViewId::DictionarySearchInput, ref text) => {
                if !text.is_empty() {
                    self.toggle_keyboard(false, None, hub, rq, context);
                    self.define(Some(text), rq, context);
                }
                true
            },
            Event::Page(dir) => {
                self.go_to_neighbor(dir, rq);
                true
            },
            Event::Device(DeviceEvent::Button { code, status: ButtonStatus::Released, .. }) => {
                match code {
                    ButtonCode::Backward =>
                        if self.doc.resolve_location(Location::Previous(self.location)).is_some() {
                            self.go_to_neighbor(CycleDir::Previous, rq);
                        } else {
                            hub.send(Event::Back).ok();
                        },
                    ButtonCode::Forward =>
                        if self.doc.resolve_location(Location::Next(self.location)).is_some() {
                            self.go_to_neighbor(CycleDir::Next, rq);
                        } else {
                            // auto close view if at end
                            hub.send(Event::Back).ok();
                        },
                    _ => (),
                }
                true
            },
            Event::Gesture(GestureEvent::Swipe { dir, start, .. }) if self.rect.includes(start) => {
                match dir {
                    Dir::West => self.go_to_neighbor(CycleDir::Next, rq),
                    Dir::East => self.go_to_neighbor(CycleDir::Previous, rq),
                    _ => (),
                }
                true
            },
            Event::Gesture(GestureEvent::Tap(center)) if self.rect.includes(center) => {
                self.follow_link(center, rq, context);
                true
            },
            Event::Gesture(GestureEvent::HoldFingerLong(pt, _)) => {
                if let Some(text) = self.underlying_word(pt) {
                    let query = text.trim_matches(|c: char| !c.is_alphanumeric()).to_string();
                    self.define(Some(&query), rq, context);
                }
                true
            },
            Event::Select(EntryId::SetSearchTarget(ref target)) => {
                if *target != self.target {
                    self.target = target.clone();
                    let name = self.target.as_deref().unwrap_or("All");
                    if let Some(bottom_bar) = self.children[6].downcast_mut::<BottomBar>() {
                        bottom_bar.update_name(name, rq);
                    }
                    if !self.query.is_empty() {
                        self.define(None, rq, context);
                    }
                }
                true
            },
            Event::Select(EntryId::ToggleFuzzy) => {
                self.fuzzy = !self.fuzzy;
                if !self.query.is_empty() {
                    self.define(None, rq, context);
                }
                true
            },
            Event::Select(EntryId::ReloadDictionaries) => {
                context.dictionaries.clear();
                context.load_dictionaries();
                if let Some(name) = self.target.as_ref() {
                    if !context.dictionaries.contains_key(name) {
                        self.target = None;
                        if let Some(bottom_bar) = self.child_mut(6).downcast_mut::<BottomBar>() {
                            bottom_bar.update_name("All", rq);
                        }
                    }
                }
                true
            },
            Event::EditLanguages => {
                if self.target.is_some() {
                    self.toggle_edit_languages(None, hub, rq, context);
                }
                true
            },
            Event::Submit(ViewId::EditLanguagesInput, ref text) => {
                if let Some(name) = self.target.as_ref() {
                    let re = Regex::new(r"\s*,\s*").unwrap();
                    context.settings.dictionary.languages
                           .insert(name.clone(), re.split(text).map(String::from).collect());
                    if self.target.is_none() && !self.query.is_empty() {
                        self.define(None, rq, context);
                    }
                }
                true
            },
            Event::Close(ViewId::EditLanguages) => {
                self.toggle_keyboard(false, None, hub, rq, context);
                false
            },
            Event::Close(ViewId::SearchBar) => {
                hub.send(Event::Back).ok();
                true
            },
            Event::Focus(v) => {
                self.focus = v;
                if v.is_some() {
                    self.toggle_keyboard(true, v, hub, rq, context);
                }
                true
            },
            Event::ToggleNear(ViewId::TitleMenu, rect) => {
                self.toggle_title_menu(rect, None, rq, context);
                true
            },
            Event::ToggleNear(ViewId::SearchMenu, rect) => {
                self.toggle_search_menu(rect, None, rq, context);
                true
            },
            Event::ToggleNear(ViewId::SearchTargetMenu, rect) => {
                self.toggle_search_target_menu(rect, None, rq, context);
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
            Event::Reseed => {
                self.reseed(rq, context);
                true
            },
            Event::Gesture(GestureEvent::Cross(_)) => {
                hub.send(Event::Back).ok();
                true
            },
            _ => false,
        }
    }

    fn render(&self, _fb: &mut dyn Framebuffer, _rect: Rectangle, _fonts: &mut Fonts) {
    }

    fn resize(&mut self, rect: Rectangle, hub: &Hub, rq: &mut RenderQueue, context: &mut Context) {
        let dpi = CURRENT_DEVICE.dpi;
        let (small_height, big_height) = (scale_by_dpi(SMALL_BAR_HEIGHT, dpi) as i32,
                                          scale_by_dpi(BIG_BAR_HEIGHT, dpi) as i32);
        let thickness = scale_by_dpi(THICKNESS_MEDIUM, dpi) as i32;
        let (small_thickness, big_thickness) = halves(thickness);

        self.children[0].resize(rect![rect.min.x, rect.min.y,
                                      rect.max.x, rect.min.y + small_height - small_thickness],
                                hub, rq, context);

        self.children[1].resize(rect![rect.min.x, rect.min.y + small_height - small_thickness,
                                      rect.max.x, rect.min.y + small_height + big_thickness],
                                hub, rq, context);

        self.children[2].resize(rect![rect.min.x, rect.min.y + small_height + big_thickness,
                                      rect.max.x, rect.min.y + 2 * small_height - small_thickness],
                                hub, rq, context);

        self.children[3].resize(rect![rect.min.x, rect.min.y + 2 * small_height - small_thickness,
                                      rect.max.x, rect.min.y + 2 * small_height + big_thickness],
                                hub, rq, context);

        let image_rect = rect![rect.min.x, rect.min.y + 2 * small_height + big_thickness,
                               rect.max.x, rect.max.y - small_height - small_thickness];
        self.doc.layout(image_rect.width(), image_rect.height(), context.settings.dictionary.font_size, dpi);
        if let Some(image) = self.children[4].downcast_mut::<Image>() {
            if let Some((pixmap, loc)) = self.doc.pixmap(Location::Exact(self.location), 1.0) {
                image.update(pixmap, &mut RenderQueue::new());
                self.location = loc;
            }
        }
        self.children[4].resize(image_rect, hub, rq, context);

        self.children[5].resize(rect![rect.min.x, rect.max.y - small_height - small_thickness,
                                      rect.max.x, rect.max.y - small_height + big_thickness],
                                hub, rq, context);

        self.children[6].resize(rect![rect.min.x, rect.max.y - small_height + big_thickness,
                                      rect.max.x, rect.max.y],
                                hub, rq, context);
        if let Some(bottom_bar) = self.children[6].downcast_mut::<BottomBar>() {
            bottom_bar.update_icons(self.doc.resolve_location(Location::Previous(self.location)).is_some(),
                                    self.doc.resolve_location(Location::Next(self.location)).is_some(), &mut RenderQueue::new());
        }
        let mut index = 7;
        if self.len() >= 9 {
            if self.children[8].is::<Keyboard>() {
                let kb_rect = rect![rect.min.x,
                                    rect.max.y - (small_height + 3 * big_height) as i32 + big_thickness,
                                    rect.max.x,
                                    rect.max.y - small_height - small_thickness];
                self.children[8].resize(kb_rect, hub, rq, context);
                let kb_rect = *self.children[8].rect();
                self.children[7].resize(rect![rect.min.x, kb_rect.min.y - thickness,
                                              rect.max.x, kb_rect.min.y],
                                        hub, rq, context);
                index = 9;
            }
        }

        for i in index..self.children.len() {
            self.children[i].resize(rect, hub, rq, context);
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
