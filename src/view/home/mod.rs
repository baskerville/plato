mod matches_label;
mod summary;
mod category;
mod shelf;
mod book;
mod bottom_bar;

use std::f32;
use std::thread;
use std::sync::mpsc;
use std::path::{Path, PathBuf};
use std::collections::{BTreeSet, VecDeque};
use std::process::{Command, Child, Stdio};
use std::io::{BufRead, BufReader};
use glob::glob;
use regex::Regex;
use serde_json::Value as JsonValue;
use fnv::{FnvHashSet, FnvHashMap};
use failure::{Error, format_err};
use crate::framebuffer::{Framebuffer, UpdateMode};
use crate::metadata::{Info, Metadata, SortMethod, SimpleStatus, sort, make_query, auto_import, clean_up};
use crate::view::{View, Event, Hub, Bus, ViewId, EntryId, EntryKind, THICKNESS_MEDIUM};
use crate::settings::{Hook, SecondColumn};
use crate::view::filler::Filler;
use crate::view::common::{locate, locate_by_id};
use crate::view::common::{toggle_main_menu, toggle_battery_menu, toggle_clock_menu};
use crate::view::keyboard::Keyboard;
use crate::view::named_input::NamedInput;
use crate::view::menu::{Menu, MenuKind};
use crate::view::menu_entry::MenuEntry;
use crate::view::search_bar::SearchBar;
use crate::view::notification::Notification;
use crate::view::intermission::IntermKind;
use crate::gesture::GestureEvent;
use crate::input::{DeviceEvent, ButtonCode, ButtonStatus};
use crate::device::{CURRENT_DEVICE, BAR_SIZES};
use crate::symbolic_path::SymbolicPath;
use crate::helpers::{load_json, save_json};
use crate::unit::scale_by_dpi;
use crate::trash::{self, trash, untrash};
use crate::app::Context;
use crate::color::BLACK;
use crate::geom::{Rectangle, CycleDir, halves};
use crate::font::Fonts;
use super::top_bar::TopBar;
use self::bottom_bar::BottomBar;
use self::summary::Summary;
use self::shelf::Shelf;

const HISTORY_SIZE: usize = 8;

#[derive(Debug)]
pub struct Home {
    rect: Rectangle,
    children: Vec<Box<dyn View>>,
    current_page: usize,
    pages_count: usize,
    summary_size: u8,
    focus: Option<ViewId>,
    query: Option<Regex>,
    target_path: Option<PathBuf>,
    target_category: Option<String>,
    sort_method: SortMethod,
    status_filter: Option<SimpleStatus>,
    reverse_order: bool,
    visible_books: Metadata,
    visible_categories: BTreeSet<String>,
    selected_categories: BTreeSet<String>,
    negated_categories: BTreeSet<String>,
    background_fetchers: FnvHashMap<String, Fetcher>,
    history: VecDeque<HistoryEntry>,
}

#[derive(Debug)]
struct HistoryEntry {
    metadata: Metadata,
    restore_books: bool,
}

#[derive(Debug)]
struct Fetcher {
    process: Option<Child>,
    sort_method: Option<SortMethod>,
    second_column: Option<SecondColumn>,
}

impl Home {
    pub fn new(rect: Rectangle, hub: &Hub, context: &mut Context) -> Result<Home, Error> {
        let dpi = CURRENT_DEVICE.dpi;
        let mut children = Vec::new();

        let thickness = scale_by_dpi(THICKNESS_MEDIUM, dpi) as i32;
        let (small_thickness, big_thickness) = halves(thickness);
        let (_, height) = context.display.dims;
        let &(small_height, big_height) = BAR_SIZES.get(&(height, dpi)).unwrap();

        let sort_method = SortMethod::Opened;
        let reverse_order = sort_method.reverse_order();

        sort(&mut context.metadata, sort_method, reverse_order);

        let visible_books = context.metadata.clone();
        let visible_categories = context.metadata.iter()
                                        .flat_map(|info| info.categories.iter())
                                        .map(|categ| categ.first_component().to_string())
                                        .collect::<BTreeSet<String>>();

        let selected_categories = BTreeSet::default();
        let negated_categories = BTreeSet::default();

        let max_lines = ((height - 3 * small_height) / big_height) as usize;
        let summary_size = context.settings.home.summary_size.max(1).min(max_lines as u8);
        let max_lines = max_lines - summary_size as usize + 1;
        let count = visible_books.len();
        let pages_count = (visible_books.len() as f32 / max_lines as f32).ceil() as usize;
        let current_page = 0;

        let top_bar = TopBar::new(rect![rect.min.x, rect.min.y,
                                        rect.max.x, rect.min.y + small_height as i32 - small_thickness],
                                  Event::Toggle(ViewId::SearchBar),
                                  sort_method.title(),
                                  context);
        children.push(Box::new(top_bar) as Box<dyn View>);

        let separator = Filler::new(rect![rect.min.x, rect.min.y + small_height as i32 - small_thickness,
                                          rect.max.x, rect.min.y + small_height as i32 + big_thickness],
                                    BLACK);
        children.push(Box::new(separator) as Box<dyn View>);

        let summary_height = small_height as i32 - thickness +
                             (summary_size - 1) as i32 * big_height as i32;
        let s_min_y = rect.min.y + small_height as i32 + big_thickness;
        let s_max_y = s_min_y + summary_height;

        let mut summary = Summary::new(rect![rect.min.x, s_min_y,
                                             rect.max.x, s_max_y]);

        let (tx, _rx) = mpsc::channel();

        summary.update(&visible_categories, &selected_categories,
                       &negated_categories, false, &tx, &mut context.fonts);

        children.push(Box::new(summary) as Box<dyn View>);

        let separator = Filler::new(rect![rect.min.x, s_max_y,
                                          rect.max.x, s_max_y + thickness],
                                    BLACK);
        children.push(Box::new(separator) as Box<dyn View>);

        let mut shelf = Shelf::new(rect![rect.min.x, s_max_y + thickness,
                                         rect.max.x, rect.max.y - small_height as i32 - small_thickness],
                                   context.settings.home.second_column);

        let index_lower = current_page * max_lines;
        let index_upper = (index_lower + max_lines).min(visible_books.len());

        shelf.update(&visible_books[index_lower..index_upper], &tx, context);

        children.push(Box::new(shelf) as Box<dyn View>);

        let separator = Filler::new(rect![rect.min.x, rect.max.y - small_height as i32 - small_thickness,
                                          rect.max.x, rect.max.y - small_height as i32 + big_thickness],
                                    BLACK);
        children.push(Box::new(separator) as Box<dyn View>);

        let bottom_bar = BottomBar::new(rect![rect.min.x, rect.max.y - small_height as i32 + big_thickness,
                                              rect.max.x, rect.max.y],
                                        current_page,
                                        pages_count,
                                        count,
                                        false);
        children.push(Box::new(bottom_bar) as Box<dyn View>);

        hub.send(Event::Render(rect, UpdateMode::Full)).ok();

        Ok(Home {
            rect,
            children,
            current_page,
            pages_count,
            summary_size,
            focus: None,
            query: None,
            target_path: None,
            target_category: None,
            sort_method,
            status_filter: None,
            reverse_order,
            visible_books,
            visible_categories,
            selected_categories,
            negated_categories,
            background_fetchers: FnvHashMap::default(),
            history: VecDeque::new(),
        })
    }

    fn refresh_visibles(&mut self, update: bool, reset_page: bool, hub: &Hub, context: &mut Context) {
        self.visible_books = context.metadata.iter().filter(|info| {
            info.is_match(&self.query) &&
            (self.status_filter.is_none() || info.simple_status() == self.status_filter.unwrap()) &&
            (self.selected_categories.is_subset(&info.categories) ||
             self.selected_categories.iter()
                                     .all(|s| info.categories
                                                  .iter().any(|c| c == s || c.is_descendant_of(s)))) &&
            (self.negated_categories.is_empty() ||
             (self.negated_categories.is_disjoint(&info.categories) &&
              info.categories.iter().all(|c| c.ancestors().all(|a| !self.negated_categories.contains(a)))))
        }).cloned().collect();

        self.visible_categories = self.visible_books.iter()
                                      .flat_map(|info| info.categories.clone()).collect();

        self.visible_categories = self.visible_categories.iter().map(|c| {
            let mut c: &str = c;
            while let Some(p) = c.parent() {
                if self.selected_categories.contains(p) {
                    break;
                }
                c = p;
            }
            c.to_string()
        }).collect();

        for s in &self.selected_categories {
            self.visible_categories.insert(s.clone());
            for a in s.ancestors() {
                self.visible_categories.insert(a.to_string());
            }
        }

        for n in &self.negated_categories {
            self.visible_categories.insert(n.clone());
            for a in n.ancestors() {
                self.visible_categories.insert(a.to_string());
            }
        }

        let max_lines = {
            let shelf = self.child(4).downcast_ref::<Shelf>().unwrap();
            shelf.max_lines
        };
        self.pages_count = (self.visible_books.len() as f32 / max_lines as f32).ceil() as usize;

        if reset_page  {
            self.current_page = 0;
        } else if self.current_page >= self.pages_count {
            self.current_page = self.pages_count.saturating_sub(1);
        }

        if update {
            self.update_summary(false, hub, &mut context.fonts);
            self.update_shelf(false, hub, context);
            self.update_bottom_bar(hub);
        }
    }

    fn terminate_fetchers(&mut self, categ: &str, hub: &Hub) {
        self.background_fetchers.retain(|name, fetcher| {
            if name == categ {
                if let Some(process) = fetcher.process.as_mut() {
                    unsafe { libc::kill(process.id() as libc::pid_t, libc::SIGTERM) };
                    process.wait().ok();
                }
                if let Some(sort_method) = fetcher.sort_method {
                    hub.send(Event::Select(EntryId::Sort(sort_method))).ok();
                }
                if let Some(second_column) = fetcher.second_column {
                    hub.send(Event::Select(EntryId::SecondColumn(second_column))).ok();
                }
                false
            } else {
                true
            }
        });
    }

    fn toggle_select_category(&mut self, categ: &str, hub: &Hub, context: &mut Context) {
        if self.selected_categories.contains(categ) {
            self.selected_categories.remove(categ);
            self.terminate_fetchers(categ, hub);
        } else {
            self.selected_categories = self.selected_categories.iter().filter_map(|s| {
                if s.is_descendant_of(categ) || categ.is_descendant_of(s) {
                    None
                } else {
                    Some(s.clone())
                }
            }).collect();

            self.negated_categories = self.negated_categories.iter().filter_map(|n| {
                if n == categ || categ.is_descendant_of(n) {
                    None
                } else {
                    Some(n.clone())
                }
            }).collect();

            self.selected_categories.insert(categ.to_string());

            for hook in &context.settings.home.hooks {
                if hook.name == categ {
                    self.insert_fetcher(hook, hub, context);
                }
            }
        }
    }

    fn insert_fetcher(&mut self, hook: &Hook, hub: &Hub, context: &Context) {
        let mut sort_method = hook.sort_method;
        let mut second_column = hook.second_column;
        if let Some(sort_method) = sort_method.replace(self.sort_method) {
            hub.send(Event::Select(EntryId::Sort(sort_method))).ok();
        }
        if let Some(second_column) = second_column.replace(context.settings.home.second_column) {
            hub.send(Event::Select(EntryId::SecondColumn(second_column))).ok();
        }
        let process = hook.program.as_ref().and_then(|p| {
            self.spawn_child(&hook.name, p, context.settings.wifi, context.online, hub)
                .map_err(|e| eprintln!("Can't spawn child: {}.", e)).ok()
        });
        self.background_fetchers.insert(hook.name.clone(),
                                        Fetcher { process, sort_method, second_column });
    }

    fn spawn_child(&mut self, name: &str, program: &PathBuf, wifi: bool, online: bool, hub: &Hub) -> Result<Child, Error> {
        let parent = program.parent()
                            .unwrap_or_else(|| Path::new(""));
        let path = program.canonicalize()?;
        let mut process = Command::new(path)
                                 .current_dir(parent)
                                 .arg(name)
                                 .arg(wifi.to_string())
                                 .arg(online.to_string())
                                 .stdout(Stdio::piped())
                                 .spawn()?;
        let stdout = process.stdout.take()
                            .ok_or_else(|| format_err!("Can't take stdout."))?;
        let hub2 = hub.clone();
        thread::spawn(move || {
            let reader = BufReader::new(stdout);
            for line_res in reader.lines() {
                if let Ok(line) = line_res {
                    if let Ok(event) = serde_json::from_str::<JsonValue>(&line) {
                        match event.get("type").and_then(JsonValue::as_str) {
                            Some("notify") => {
                                if let Some(msg) = event.get("message").and_then(JsonValue::as_str) {
                                    hub2.send(Event::Notify(msg.to_string())).ok();
                                }
                            },
                            Some("addDocument") => {
                                if let Some(info) = event.get("info").map(ToString::to_string)
                                                         .and_then(|v| serde_json::from_str(&v).ok()) {
                                    hub2.send(Event::AddDocument(Box::new(info))).ok();
                                }
                            },
                            Some("removeDocument") => {
                                if let Some(path) = event.get("path").and_then(JsonValue::as_str) {
                                    hub2.send(Event::RemoveDocument(PathBuf::from(path))).ok();
                                }
                            },
                            Some("setWifi") => {
                                if let Some(enable) = event.get("enable").and_then(JsonValue::as_bool) {
                                    hub2.send(Event::SetWifi(enable)).ok();
                                }
                            },
                            _ => (),
                        }
                    }
                } else {
                    break;
                }
            }
        });
        Ok(process)
    }

    fn toggle_negate_category(&mut self, categ: &str, hub: &Hub) {
        if self.negated_categories.contains(categ) {
            self.negated_categories.remove(categ);
        } else {
            self.negated_categories = self.negated_categories.iter().filter_map(|s| {
                if s.is_descendant_of(categ) || categ.is_descendant_of(s) {
                    None
                } else {
                    Some(s.clone())
                }
            }).collect();
            let mut deselected_categories = Vec::new();
            self.selected_categories = self.selected_categories.iter().filter_map(|s| {
                if s == categ || s.is_descendant_of(categ) {
                    deselected_categories.push(s.clone());
                    None
                } else {
                    Some(s.clone())
                }
            }).collect();
            for s in deselected_categories {
                self.terminate_fetchers(&s, hub);
            }
            self.negated_categories.insert(categ.to_string());
        }
    }

    fn toggle_negate_category_children(&mut self, parent: &str, hub: &Hub) {
        let mut children = Vec::new();

        for c in &self.visible_categories {
            if c.is_child_of(parent) {
                children.push(c.to_string());
            }
        }

        while let Some(c) = children.pop() {
            self.toggle_negate_category(&c, hub);
        }
    }

    fn go_to_page(&mut self, index: usize, hub: &Hub, context: &Context) {
        if index >= self.pages_count {
            return;
        }
        self.current_page = index;
        self.update_shelf(false, hub, context);
        self.update_bottom_bar(hub);
    }

    fn go_to_neighbor(&mut self, dir: CycleDir, hub: &Hub, context: &Context) {
        match dir {
            CycleDir::Next if self.current_page < self.pages_count.saturating_sub(1) => {
                self.current_page += 1;
            },
            CycleDir::Previous if self.current_page > 0 => {
                self.current_page -= 1;
            },
            _ => return,
        }

        self.update_shelf(false, hub, context);
        self.update_bottom_bar(hub);
    }

    fn update_summary(&mut self, was_resized: bool, hub: &Hub, fonts: &mut Fonts) {
        let summary = self.children[2].as_mut().downcast_mut::<Summary>().unwrap();
        summary.update(&self.visible_categories, &self.selected_categories, &self.negated_categories,
                       was_resized, hub, fonts);
    }

    fn update_second_column(&mut self, hub: &Hub, context: &mut Context) {
        self.children[4].as_mut().downcast_mut::<Shelf>().unwrap()
           .set_second_column(context.settings.home.second_column);
        self.update_shelf(false, hub, context);
    }

    fn update_shelf(&mut self, was_resized: bool, hub: &Hub, context: &Context) {
        let dpi = CURRENT_DEVICE.dpi;
        let (_, height) = context.display.dims;
        let &(_, big_height) = BAR_SIZES.get(&(height, dpi)).unwrap();
        let thickness = scale_by_dpi(THICKNESS_MEDIUM, dpi) as i32;

        let shelf = self.children[4].as_mut().downcast_mut::<Shelf>().unwrap();
        let max_lines = ((shelf.rect.height() + thickness as u32) / big_height) as usize;

        // TODO: extract this into a function and call this when the shelf is resized to avoid the
        // temporal dependency between update_shelf and update_bottom_bar
        if was_resized {
            let page_position = if self.visible_books.is_empty() {
                0.0
            } else {
                self.current_page as f32 * (shelf.max_lines as f32 /
                                            self.visible_books.len() as f32)
            };

            let mut page_guess = page_position * self.visible_books.len() as f32 / max_lines as f32;
            let page_ceil = page_guess.ceil();

            if (page_ceil - page_guess) < f32::EPSILON {
                page_guess = page_ceil;
            }

            self.pages_count = (self.visible_books.len() as f32 / max_lines as f32).ceil() as usize;
            self.current_page = (page_guess as usize).min(self.pages_count.saturating_sub(1));
        }

        let index_lower = self.current_page * max_lines;
        let index_upper = (index_lower + max_lines).min(self.visible_books.len());

        shelf.update(&self.visible_books[index_lower..index_upper], hub, context);
    }

    fn update_top_bar(&mut self, search_visible: bool, hub: &Hub) {
        if let Some(index) = locate::<TopBar>(self) {
            let top_bar = self.children[index].as_mut().downcast_mut::<TopBar>().unwrap();
            let name = if search_visible { "home" } else { "search" };
            top_bar.update_root_icon(name, hub);
            top_bar.update_title_label(&self.sort_method.title(), hub);
        }
    }

    fn update_bottom_bar(&mut self, hub: &Hub) {
        if let Some(index) = locate::<BottomBar>(self) {
            let bottom_bar = self.children[index].as_mut().downcast_mut::<BottomBar>().unwrap();
            let filter = self.query.is_some() ||
                         self.status_filter.is_some() ||
                         !self.selected_categories.is_empty() ||
                         !self.negated_categories.is_empty();
            bottom_bar.update_matches_label(self.visible_books.len(), filter, hub);
            bottom_bar.update_page_label(self.current_page, self.pages_count, hub);
            bottom_bar.update_icons(self.current_page, self.pages_count, hub);
        }
    }

    fn toggle_keyboard(&mut self, enable: bool, update: bool, id: Option<ViewId>, hub: &Hub, context: &mut Context) {
        let dpi = CURRENT_DEVICE.dpi;
        let (_, height) = context.display.dims;
        let &(small_height, big_height) = BAR_SIZES.get(&(height, dpi)).unwrap();
        let thickness = scale_by_dpi(THICKNESS_MEDIUM, dpi) as i32;
        let (small_thickness, big_thickness) = halves(thickness);
        let mut has_search_bar = false;

        if let Some(index) = locate::<Keyboard>(self) {
            if enable {
                return;
            }

            let y_min = self.child(5).rect().min.y;
            let mut rect = *self.child(index).rect();
            rect.absorb(self.child(index-1).rect());

            self.children.drain(index - 1 ..= index);

            let delta_y = rect.height() as i32;

            if index > 6 {
                has_search_bar = true;
                for i in 5..=6 {
                    let shifted_rect = *self.child(i).rect() + pt!(0, delta_y);
                    self.child_mut(i).resize(shifted_rect, hub, context);
                }
            }

            hub.send(Event::Focus(None)).ok();
            let rect = rect![self.rect.min.x, y_min, self.rect.max.x, y_min + delta_y];
            hub.send(Event::Expose(rect, UpdateMode::Gui)).ok();
        } else {
            if !enable {
                return;
            }

            let index = locate::<BottomBar>(self).unwrap() - 1;
            let mut kb_rect = rect![self.rect.min.x,
                                    self.rect.max.y - (small_height + 3 * big_height) as i32 + big_thickness,
                                    self.rect.max.x,
                                    self.rect.max.y - small_height as i32 - small_thickness];

            let number = match id {
                Some(ViewId::GoToPageInput) => true,
                _ => false,
            };

            let keyboard = Keyboard::new(&mut kb_rect, number, context);
            self.children.insert(index, Box::new(keyboard) as Box<dyn View>);

            let separator = Filler::new(rect![self.rect.min.x, kb_rect.min.y - thickness,
                                              self.rect.max.x, kb_rect.min.y],
                                        BLACK);
            self.children.insert(index, Box::new(separator) as Box<dyn View>);

            let delta_y = kb_rect.height() as i32 + thickness;

            if index > 5 {
                has_search_bar = true;
                for i in 5..=6 {
                    let shifted_rect = *self.child(i).rect() + pt!(0, -delta_y);
                    self.child_mut(i).resize(shifted_rect, hub, context);
                }
            }
        }

        if update {
            if enable {
                if has_search_bar {
                    for i in 5..9 {
                        hub.send(Event::Render(*self.child(i).rect(), UpdateMode::Gui)).ok();
                    }
                } else {
                    for i in 5..7 {
                        hub.send(Event::Render(*self.child(i).rect(), UpdateMode::Gui)).ok();
                    }
                }
            } else {
                if has_search_bar {
                    for i in 5..7 {
                        hub.send(Event::Render(*self.child(i).rect(), UpdateMode::Gui)).ok();
                    }
                }
            }
        }
    }

    fn toggle_search_bar(&mut self, enable: Option<bool>, update: bool, hub: &Hub, context: &mut Context) {
        let dpi = CURRENT_DEVICE.dpi;
        let (_, height) = context.display.dims;
        let &(small_height, _) = BAR_SIZES.get(&(height, dpi)).unwrap();
        let thickness = scale_by_dpi(THICKNESS_MEDIUM, dpi) as i32;
        let delta_y = small_height as i32;
        let search_visible: bool;

        if let Some(index) = locate::<SearchBar>(self) {
            if let Some(true) = enable {
                return;
            }

            if let Some(ViewId::HomeSearchInput) = self.focus {
                self.toggle_keyboard(false, false, Some(ViewId::HomeSearchInput), hub, context);
            }

            self.children.drain(index - 1 ..= index);

            {
                let shelf = self.child_mut(4).downcast_mut::<Shelf>().unwrap();
                shelf.rect.max.y += delta_y;
            }

            self.resize_summary(0, false, hub, context);

            self.query = None;

            search_visible = false;
        } else {
            if let Some(false) = enable {
                return;
            }

            let sp_rect = *self.child(5).rect() - pt!(0, delta_y);

            let search_bar = SearchBar::new(rect![self.rect.min.x, sp_rect.max.y,
                                                  self.rect.max.x,
                                                  sp_rect.max.y + delta_y - thickness],
                                            ViewId::HomeSearchInput,
                                            "Title, author, category",
                                            "");

            self.children.insert(5, Box::new(search_bar) as Box<dyn View>);

            let separator = Filler::new(sp_rect, BLACK);
            self.children.insert(5, Box::new(separator) as Box<dyn View>);

            // move the shelf's bottom edge
            {
                let shelf = self.child_mut(4).downcast_mut::<Shelf>().unwrap();
                shelf.rect.max.y -= delta_y;
            }

            if locate::<Keyboard>(self).is_none() {
                self.toggle_keyboard(true, false, Some(ViewId::HomeSearchInput), hub, context);
            }

            hub.send(Event::Focus(Some(ViewId::HomeSearchInput))).ok();

            self.resize_summary(0, false, hub, context);
            search_visible = true;
        }

        if update {
            if search_visible {
                // TODO: don't update if the keyboard is already present
                for i in [3usize, 5, 6, 7, 8].iter().cloned() {
                    hub.send(Event::Render(*self.child(i).rect(), UpdateMode::Gui)).ok();
                }
            } else {
                for i in [3usize, 5].iter().cloned() {
                    hub.send(Event::Render(*self.child(i).rect(), UpdateMode::Gui)).ok();
                }
            }

            self.update_top_bar(search_visible, hub);
            self.update_summary(true, hub, &mut context.fonts);
            self.update_shelf(true, hub, context);
            self.update_bottom_bar(hub);

            if !search_visible {
                self.refresh_visibles(true, true, hub, context);
            }
        }
    }

    fn toggle_go_to_page(&mut self, enable: Option<bool>, hub: &Hub, context: &mut Context) {
        if let Some(index) = locate_by_id(self, ViewId::GoToPage) {
            if let Some(true) = enable {
                return;
            }
            hub.send(Event::Expose(*self.child(index).rect(), UpdateMode::Gui)).ok();
            self.children.remove(index);
            if let Some(ViewId::GoToPageInput) = self.focus {
                self.toggle_keyboard(false, true, Some(ViewId::GoToPageInput), hub, context);
            }
        } else {
            if let Some(false) = enable {
                return;
            }
            if self.pages_count < 2 {
                return;
            }
            let go_to_page = NamedInput::new("Go to page".to_string(),
                                             ViewId::GoToPage,
                                             ViewId::GoToPageInput,
                                             4, context);
            hub.send(Event::Render(*go_to_page.rect(), UpdateMode::Gui)).ok();
            hub.send(Event::Focus(Some(ViewId::GoToPageInput))).ok();
            self.children.push(Box::new(go_to_page) as Box<dyn View>);
        }
    }

    fn toggle_sort_menu(&mut self, rect: Rectangle, enable: Option<bool>, hub: &Hub, context: &mut Context) {
        if let Some(index) = locate_by_id(self, ViewId::SortMenu) {
            if let Some(true) = enable {
                return;
            }
            hub.send(Event::Expose(*self.child(index).rect(), UpdateMode::Gui)).ok();
            self.children.remove(index);
        } else {
            if let Some(false) = enable {
                return;
            }
            let entries = vec![EntryKind::RadioButton("Date Opened".to_string(),
                                                      EntryId::Sort(SortMethod::Opened),
                                                      self.sort_method == SortMethod::Opened),
                               EntryKind::RadioButton("Date Added".to_string(),
                                                      EntryId::Sort(SortMethod::Added),
                                                      self.sort_method == SortMethod::Added),
                               EntryKind::RadioButton("Progress".to_string(),
                                                      EntryId::Sort(SortMethod::Progress),
                                                      self.sort_method == SortMethod::Progress),
                               EntryKind::RadioButton("Author".to_string(),
                                                      EntryId::Sort(SortMethod::Author),
                                                      self.sort_method == SortMethod::Author),
                               EntryKind::RadioButton("File Size".to_string(),
                                                      EntryId::Sort(SortMethod::Size),
                                                      self.sort_method == SortMethod::Size),
                               EntryKind::RadioButton("File Type".to_string(),
                                                      EntryId::Sort(SortMethod::Kind),
                                                      self.sort_method == SortMethod::Kind),
                               EntryKind::Separator,
                               EntryKind::CheckBox("Reverse Order".to_string(),
                                                   EntryId::ReverseOrder, self.reverse_order)];
            let sort_menu = Menu::new(rect, ViewId::SortMenu, MenuKind::DropDown, entries, context);
            hub.send(Event::Render(*sort_menu.rect(), UpdateMode::Gui)).ok();
            self.children.push(Box::new(sort_menu) as Box<dyn View>);
        }
    }

    fn book_index(&self, index: usize) -> usize {
        let max_lines = self.child(4).downcast_ref::<Shelf>().unwrap().max_lines;
        let index_lower = self.current_page * max_lines;
        (index_lower + index).min(self.visible_books.len())
    }

    fn toggle_book_menu(&mut self, index: usize, rect: Rectangle, enable: Option<bool>, hub: &Hub, context: &mut Context) {
        if let Some(index) = locate_by_id(self, ViewId::BookMenu) {
            if let Some(true) = enable {
                return;
            }
            hub.send(Event::Expose(*self.child(index).rect(), UpdateMode::Gui)).ok();
            self.children.remove(index);
        } else {
            if let Some(false) = enable {
                return;
            }

            let book_index = self.book_index(index);
            let info = &self.visible_books[book_index];
            let path = &info.file.path;

            let categories = info.categories.iter()
                                  .map(|c| EntryKind::Command(c.to_string(),
                                                              EntryId::RemoveBookCategory(path.clone(),
                                                                                          c.to_string())))
                                  .collect::<Vec<EntryKind>>();

            let mut entries = vec![EntryKind::Command("Add Categories".to_string(),
                                                      EntryId::AddBookCategories(path.clone()))];

            if !categories.is_empty() {
                entries.push(EntryKind::SubMenu("Remove Category".to_string(), categories));
            }

            entries.push(EntryKind::Separator);

            let submenu: &[SimpleStatus] = match info.simple_status() {
                SimpleStatus::Reading => &[SimpleStatus::New, SimpleStatus::Finished],
                SimpleStatus::New => &[SimpleStatus::Finished],
                SimpleStatus::Finished => &[SimpleStatus::New],
            };

            let submenu = submenu.iter().map(|s| EntryKind::Command(s.to_string(),
                                                                    EntryId::SetStatus(path.clone(), *s)))
                                 .collect();
            entries.push(EntryKind::SubMenu("Mark As".to_string(), submenu));

            {
                let images = &context.settings.intermission_images;
                let submenu = [IntermKind::Suspend,
                               IntermKind::PowerOff,
                               IntermKind::Share].iter().map(|k| {
                                   EntryKind::CheckBox(k.label().to_string(),
                                                       EntryId::ToggleIntermissionImage(*k, path.clone()),
                                                       images.get(k.key()) == Some(path))
                               }).collect::<Vec<EntryKind>>();


                entries.push(EntryKind::SubMenu("Set As".to_string(), submenu))
            }

            entries.push(EntryKind::Separator);
            entries.push(EntryKind::Command("Remove".to_string(), EntryId::Remove(path.clone())));

            let book_menu = Menu::new(rect, ViewId::BookMenu, MenuKind::Contextual, entries, context);
            hub.send(Event::Render(*book_menu.rect(), UpdateMode::Gui)).ok();
            self.children.push(Box::new(book_menu) as Box<dyn View>);
        }
    }

    fn toggle_category_menu(&mut self, categ: &str, rect: Rectangle, enable: Option<bool>, hub: &Hub, context: &mut Context) {
        if let Some(index) = locate_by_id(self, ViewId::CategoryMenu) {
            if let Some(true) = enable {
                return;
            }
            hub.send(Event::Expose(*self.child(index).rect(), UpdateMode::Gui)).ok();
            self.children.remove(index);
        } else {
            if let Some(false) = enable {
                return;
            }

            let entries = vec![EntryKind::Command("Add".to_string(),
                                                  EntryId::AddMatchesCategories),
                               EntryKind::Command("Rename".to_string(),
                                                  EntryId::RenameCategory(categ.to_string())),
                               EntryKind::Command("Remove".to_string(),
                                                  EntryId::RemoveCategory(categ.to_string()))];

            let category_menu = Menu::new(rect, ViewId::CategoryMenu, MenuKind::Contextual, entries, context);
            hub.send(Event::Render(*category_menu.rect(), UpdateMode::Gui)).ok();
            self.children.push(Box::new(category_menu) as Box<dyn View>);
        }
    }

    fn toggle_matches_menu(&mut self, rect: Rectangle, enable: Option<bool>, hub: &Hub, context: &mut Context) {
        if let Some(index) = locate_by_id(self, ViewId::MatchesMenu) {
            if let Some(true) = enable {
                return;
            }

            hub.send(Event::Expose(*self.child(index).rect(), UpdateMode::Gui)).ok();
            self.children.remove(index);
        } else {
            if let Some(false) = enable {
                return;
            }

            let loadables: Vec<PathBuf> = context.settings.library_path.join(".metadata*.json").to_str().and_then(|s| {
                glob(s).ok().map(|paths| {
                    paths.filter_map(|x| x.ok().and_then(|p| p.file_name().map(PathBuf::from))
                                                              .filter(|p| *p != context.filename)).collect()
                })
            }).unwrap_or_default();


            let mut entries = vec![EntryKind::Command("Save".to_string(), EntryId::Save),
                                   EntryKind::Command("Save As".to_string(), EntryId::SaveAs),
                                   EntryKind::Command("Import".to_string(), EntryId::Import)];

            if !loadables.is_empty() {
                entries.push(EntryKind::SubMenu("Load".to_string(),
                                                loadables.into_iter().map(|e| EntryKind::Command(e.to_string_lossy().into_owned(),
                                                                                                 EntryId::Load(e))).collect()));
            }

            entries.push(EntryKind::Command("Reload".to_string(), EntryId::Reload));
            entries.push(EntryKind::Command("Clean Up".to_string(), EntryId::CleanUp));

            let hooks: Vec<EntryKind> =
                context.settings.home.hooks.iter()
                       .map(|v| EntryKind::Command(v.name.clone(),
                                                   EntryId::ToggleSelectCategory(v.name.clone()))).collect();

            if !self.visible_books.is_empty() || !hooks.is_empty() {
                entries.push(EntryKind::Separator);
            }

            if !self.visible_books.is_empty() {
                entries.push(EntryKind::Command("Add Categories".to_string(), EntryId::AddMatchesCategories));
                let categories: BTreeSet<String> = self.visible_books.iter().flat_map(|info| info.categories.clone()).collect();
                let categories: Vec<EntryKind> = categories.iter().map(|c| EntryKind::Command(c.clone(), EntryId::RemoveCategory(c.clone()))).collect();

                if !categories.is_empty() {
                    entries.push(EntryKind::SubMenu("Remove Category".to_string(), categories));
                }
            }

            if !hooks.is_empty() {
                entries.push(EntryKind::SubMenu("Toggle Hook".to_string(), hooks));
            }

            entries.push(EntryKind::Separator);

            let status_filter = self.status_filter;
            entries.push(EntryKind::SubMenu("Show".to_string(),
                vec![EntryKind::RadioButton("All".to_string(), EntryId::StatusFilter(None), status_filter == None),
                     EntryKind::Separator,
                     EntryKind::RadioButton("Reading".to_string(), EntryId::StatusFilter(Some(SimpleStatus::Reading)), status_filter == Some(SimpleStatus::Reading)),
                     EntryKind::RadioButton("New".to_string(), EntryId::StatusFilter(Some(SimpleStatus::New)), status_filter == Some(SimpleStatus::New)),
                     EntryKind::RadioButton("Finished".to_string(), EntryId::StatusFilter(Some(SimpleStatus::Finished)), status_filter == Some(SimpleStatus::Finished))]));
            let second_column = context.settings.home.second_column;
            entries.push(EntryKind::SubMenu("Second Column".to_string(),
                vec![EntryKind::RadioButton("Progress".to_string(), EntryId::SecondColumn(SecondColumn::Progress), second_column == SecondColumn::Progress),
                     EntryKind::RadioButton("Year".to_string(), EntryId::SecondColumn(SecondColumn::Year), second_column == SecondColumn::Year)]));

            if !self.visible_books.is_empty() || !self.history.is_empty() || !trash::is_empty(context) {
                entries.push(EntryKind::Separator);
            }

            if !trash::is_empty(context) {
                entries.push(EntryKind::Command("Empty Trash".to_string(), EntryId::EmptyTrash));
            }

            if !self.visible_books.is_empty() {
                entries.push(EntryKind::Command("Remove".to_string(), EntryId::RemoveMatches));
            }

            if !self.history.is_empty() {
                entries.push(EntryKind::Command("Undo".to_string(), EntryId::Undo));
            }

            let matches_menu = Menu::new(rect, ViewId::MatchesMenu, MenuKind::DropDown, entries, context);
            hub.send(Event::Render(*matches_menu.rect(), UpdateMode::Gui)).ok();
            self.children.push(Box::new(matches_menu) as Box<dyn View>);
        }
    }

    // Relatively moves the bottom edge of the summary
    // And consequently moves the top edge of the shelf
    // and the separator between them.
    fn resize_summary(&mut self, delta_y: i32, update: bool, hub: &Hub, context: &mut Context) {
        let dpi = CURRENT_DEVICE.dpi;
        let (_, height) = context.display.dims;
        let &(small_height, big_height) = BAR_SIZES.get(&(height, dpi)).unwrap();
        let thickness = scale_by_dpi(THICKNESS_MEDIUM, dpi) as i32;

        let min_height = if locate::<SearchBar>(self).is_some() {
            big_height as i32 - thickness
        } else {
            small_height as i32 - thickness
        };

        let (current_height, next_height) = {
            let summary = self.child(2).downcast_ref::<Summary>().unwrap();
            let shelf = self.child(4).downcast_ref::<Shelf>().unwrap();
            let max_height = min_height.max(shelf.rect.max.y - summary.rect.min.y - big_height as i32);
            let current_height = summary.rect.height() as i32;
            let size_factor = ((current_height + delta_y - min_height) as f32 / big_height as f32).round() as i32;
            let next_height = max_height.min(min_height.max(min_height + size_factor * big_height as i32));
            (current_height, next_height)
        };

        if current_height == next_height {
            return;
        }

        self.summary_size = (1 + (next_height - min_height) / big_height as i32) as u8;

        // Move the summary's bottom edge.
        let delta_y = {
            let summary = self.child_mut(2).downcast_mut::<Summary>().unwrap();
            let last_max_y = summary.rect.max.y;
            summary.rect.max.y = summary.rect.min.y + next_height;
            summary.rect.max.y - last_max_y
        };

        // Move the separator.
        {
            let separator = self.child_mut(3).downcast_mut::<Filler>().unwrap();
            separator.rect += pt!(0, delta_y);
        }

        // Move the shelf's top edge.
        {
            let shelf = self.child_mut(4).downcast_mut::<Shelf>().unwrap();

            shelf.rect.min.y += delta_y;
        }

        if update {
            hub.send(Event::Render(*self.child(3).rect(), UpdateMode::Gui)).ok();
            self.update_summary(true, hub, &mut context.fonts);
            self.update_shelf(true, hub, context);
            self.update_bottom_bar(hub);
        }
    }

    fn history_push(&mut self, restore_books: bool, context: &mut Context) {
        self.history.push_back(HistoryEntry { metadata: context.metadata.clone(),
                                              restore_books });
        if self.history.len() > HISTORY_SIZE {
            self.history.pop_front();
        }
    }

    fn undo(&mut self, hub: &Hub, context: &mut Context) {
        if let Some(entry) = self.history.pop_back() {
            context.metadata = entry.metadata;
            if entry.restore_books {
                untrash(context).map_err(|e| eprintln!("Can't restore books from trash: {}", e)).ok();
            }
            sort(&mut context.metadata, self.sort_method, self.reverse_order);
            self.refresh_visibles(true, false, hub, context);
        }
    }

    fn remove_matches(&mut self, hub: &Hub, context: &mut Context) {
        let paths: FnvHashSet<PathBuf> = self.visible_books.drain(..)
                                             .map(|info| info.file.path).collect();
        if trash(&paths, context).map_err(|e| eprintln!("Can't trash matches: {}", e)).is_ok() {
            self.history_push(true, context);
            context.metadata.retain(|info| !paths.contains(&info.file.path));
            context.settings.intermission_images.retain(|_, path| !paths.contains(path));
            self.refresh_visibles(true, false, hub, context);
        }
    }

    fn add_matches_categories(&mut self, categs: &Vec<String>, hub: &Hub, context: &mut Context) {
        if categs.is_empty() {
            return;
        }

        self.history_push(false, context);
        let mut paths: FnvHashSet<PathBuf> = self.visible_books.drain(..)
                                                 .map(|info| info.file.path).collect();

        for info in &mut context.metadata {
            if paths.remove(&info.file.path) {
                info.categories.extend(categs.clone());
                if paths.is_empty() {
                    break;
                }
            }
        }

        self.refresh_visibles(true, false, hub, context);
    }

    fn remove_category(&mut self, categ: &str, hub: &Hub, context: &mut Context) {
        self.history_push(false, context);

        self.selected_categories = self.selected_categories.iter().filter_map(|c| {
            if c == categ || c.is_descendant_of(categ) {
                None
            } else {
                Some(c.clone())
            }
        }).collect();

        self.negated_categories = self.negated_categories.iter().filter_map(|c| {
            if c == categ || c.is_descendant_of(categ) {
                None
            } else {
                Some(c.clone())
            }
        }).collect();

        for info in &mut context.metadata {
            info.categories = info.categories.iter().filter_map(|c| {
                if c == categ || c.is_descendant_of(categ) {
                    None
                } else {
                    Some(c.clone())
                }
            }).collect();
        }

        self.refresh_visibles(true, false, hub, context);
    }

    fn rename_category(&mut self, categ_old: &str, categ_new: &str, hub: &Hub, context: &mut Context) {
        if categ_old == categ_new {
            return;
        }

        self.history_push(false, context);

        self.selected_categories = self.selected_categories.iter().map(|c| {
            if c == categ_old {
                categ_new.to_string()
            } else if c.is_descendant_of(categ_old) {
                categ_new.join(&c[categ_old.len()+1..])
            } else {
                c.clone()
            }
        }).collect();

        self.negated_categories = self.negated_categories.iter().map(|c| {
            if c == categ_old {
                categ_new.to_string()
            } else if c.is_descendant_of(categ_old) {
                categ_new.join(&c[categ_old.len()+1..])
            } else {
                c.clone()
            }
        }).collect();

        for info in &mut context.metadata {
            info.categories = info.categories.iter().map(|c| {
                if c == categ_old {
                    categ_new.to_string()
                } else if c.is_descendant_of(categ_old) {
                    categ_new.join(&c[categ_old.len()+1..])
                } else {
                    c.clone()
                }
            }).collect();
        }

        self.refresh_visibles(true, false, hub, context);
    }

    fn add_document(&mut self, mut info: Info, hub: &Hub, context: &mut Context) {
        if let Ok(path) = info.file.path.strip_prefix(&context.settings.library_path) {
            info.file.path = path.to_path_buf();
            context.metadata.push(info);
            // TODO: Only update bars and shelves once.
            self.refresh_visibles(true, false, hub, context);
            self.sort(false, hub, context);
        }
    }

    fn remove_document(&mut self, path: &PathBuf, hub: &Hub, context: &mut Context) {
        let paths: FnvHashSet<PathBuf> = [path.clone()].iter().cloned().collect();
        if trash(&paths, context).map_err(|e| eprintln!("Can't trash {}: {}", path.display(), e)).is_ok() {
            self.history_push(true, context);
            context.metadata.retain(|info| info.file.path != *path);
            context.settings.intermission_images.retain(|_, path| !paths.contains(path));
            self.refresh_visibles(true, false, hub, context);
        }
    }

    fn add_book_categories(&mut self, path: &PathBuf, categs: &Vec<String>, hub: &Hub, context: &mut Context) {
        if categs.is_empty() {
            return;
        }

        self.history_push(false, context);

        for info in &mut context.metadata {
            if info.file.path == *path {
                info.categories.extend(categs.clone());
                break;
            }
        }

        self.refresh_visibles(true, false, hub, context);
    }


    fn remove_book_category(&mut self, path: &PathBuf, categ: &str, hub: &Hub, context: &mut Context) {
        self.history_push(false, context);

        for info in &mut context.metadata {
            if info.file.path == *path {
                info.categories.remove(categ);
                break;
            }
        }

        self.refresh_visibles(true, false, hub, context);
    }

    fn set_status(&mut self, path: &PathBuf, status: SimpleStatus, hub: &Hub, context: &mut Context) {
        self.history_push(false, context);

        for info in &mut context.metadata {
            if info.file.path == *path {
                if status == SimpleStatus::New {
                    info.reader = None;
                } else if let Some(r) = info.reader.as_mut() {
                    r.finished = true;
                }
                break;
            }
        }

        if self.sort_method == SortMethod::Progress ||
           self.sort_method == SortMethod::Opened {
            self.sort(false, hub, context);
        }

        self.refresh_visibles(true, false, hub, context);
    }

    fn set_reverse_order(&mut self, value: bool, hub: &Hub, context: &mut Context) {
        self.reverse_order = value;
        self.sort(true, hub, context);
    }

    fn set_sort_method(&mut self, sort_method: SortMethod, hub: &Hub, context: &mut Context) {
        self.sort_method = sort_method;
        self.reverse_order = sort_method.reverse_order();

        if let Some(index) = locate_by_id(self, ViewId::SortMenu) {
            self.child_mut(index)
                .children_mut().last_mut().unwrap()
                .downcast_mut::<MenuEntry>().unwrap()
                .update(sort_method.reverse_order(), hub);
        }

        self.sort(true, hub, context);
    }

    fn sort(&mut self, reset_page: bool, hub: &Hub, context: &mut Context) {
        if reset_page {
            self.current_page = 0;
        }

        sort(&mut context.metadata, self.sort_method, self.reverse_order);
        sort(&mut self.visible_books, self.sort_method, self.reverse_order);
        self.update_shelf(false, hub, context);
        let search_visible = locate::<SearchBar>(self).is_some();
        self.update_top_bar(search_visible, hub);
        self.update_bottom_bar(hub);
    }

    fn reseed(&mut self, reset_page: bool, hub: &Hub, context: &mut Context) {
        let (tx, _rx) = mpsc::channel();
        self.refresh_visibles(true, reset_page, &tx, context);
        self.sort(false, &tx, context);
        if let Some(top_bar) = self.child_mut(0).downcast_mut::<TopBar>() {
            top_bar.update_frontlight_icon(&tx, context);
        }
        hub.send(Event::ClockTick).ok();
        hub.send(Event::BatteryTick).ok();
        hub.send(Event::Render(self.rect, UpdateMode::Gui)).ok();
    }

    fn save_as(&mut self, filename: Option<&str>, context: &mut Context) {
        let path = if let Some(filename) = filename.as_ref() {
            context.settings.library_path.join(format!(".metadata-{}.json", filename))
        } else {
            context.settings.library_path.join(&context.filename)
        };
        save_json(&self.visible_books, path).map_err(|e| {
            eprintln!("Can't save: {}.", e);
        }).ok();
    }

    fn load(&mut self, filename: &PathBuf, hub: &Hub, context: &mut Context) {
        let md = load_json::<Metadata, _>(context.settings.library_path.join(filename))
                           .map_err(|e| eprintln!("Can't load: {}", e));
        if let Ok(metadata) = md {
            let saved = save_json(&context.metadata,
                                  context.settings.library_path.join(&context.filename))
                                 .map_err(|e| eprintln!("Can't save: {}", e)).is_ok();
            if saved {
                context.filename = filename.clone();
                context.metadata = metadata;
                self.history.clear();
                self.selected_categories.clear();
                self.negated_categories.clear();
                self.reseed(true, hub, context);
            }
        }
    }

    fn reload(&mut self, hub: &Hub, context: &mut Context) {
        let md = load_json::<Metadata, _>(context.settings.library_path.join(&context.filename))
                           .map_err(|e| eprintln!("Can't load: {}", e));
        if let Ok(metadata) = md {
            context.metadata = metadata;
            self.history.clear();
            self.selected_categories.clear();
            self.negated_categories.clear();
            self.reseed(true, hub, context);
        }
    }

    fn clean_up(&mut self, hub: &Hub, context: &mut Context) {
        self.history_push(false, context);
        let library_path = &context.settings.library_path;
        clean_up(library_path, &mut context.metadata);
        self.refresh_visibles(true, false, hub, context);
    }

    fn import(&mut self, hub: &Hub, context: &mut Context) {
        let imd = auto_import(&context.settings.library_path,
                              &context.metadata,
                              &context.settings.import)
                             .map_err(|e| eprintln!("Can't import: {}", e));
        if let Ok(mut imported_metadata) = imd {
            context.metadata.append(&mut imported_metadata);
            sort(&mut context.metadata, self.sort_method, self.reverse_order);
            self.refresh_visibles(true, false, hub, context);
        }
    }
}

// TODO: make the update_* and resize_* methods take a mutable bit fields as argument and make a
// generic method for updating everything based on the bit field to avoid needlessly updating
// things multiple times?

impl View for Home {
    fn handle_event(&mut self, evt: &Event, hub: &Hub, _bus: &mut Bus, context: &mut Context) -> bool {
        match *evt {
            Event::Gesture(GestureEvent::Rotate { quarter_turns, .. }) if quarter_turns != 0 => {
                let (_, dir) = CURRENT_DEVICE.mirroring_scheme();
                let n = (4 + (context.display.rotation - dir * quarter_turns)) % 4;
                hub.send(Event::Select(EntryId::Rotate(n))).ok();
                true
            },
            Event::Focus(v) => {
                self.focus = v;
                if v.is_some() {
                    self.toggle_keyboard(true, true, v, hub, context);
                }
                true
            },
            Event::Show(ViewId::Keyboard) => {
                self.toggle_keyboard(true, true, None, hub, context);
                true
            },
            Event::Toggle(ViewId::GoToPage) => {
                self.toggle_go_to_page(None, hub, context);
                true
            },
            Event::Toggle(ViewId::SearchBar) => {
                self.toggle_search_bar(None, true, hub, context);
                true
            },
            Event::ToggleNear(ViewId::TitleMenu, rect) => {
                self.toggle_sort_menu(rect, None, hub, context);
                true
            },
            Event::ToggleCategoryMenu(rect, ref categ) => {
                self.toggle_category_menu(categ, rect, None, hub, context);
                true
            },
            Event::ToggleBookMenu(rect, index) => {
                self.toggle_book_menu(index, rect, None, hub, context);
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
            Event::ToggleNear(ViewId::MatchesMenu, rect) => {
                self.toggle_matches_menu(rect, None, hub, context);
                true
            },
            Event::Close(ViewId::SearchBar) => {
                self.toggle_search_bar(Some(false), true, hub, context);
                true
            },
            Event::Close(ViewId::SortMenu) => {
                self.toggle_sort_menu(Rectangle::default(), Some(false), hub, context);
                true
            },
            Event::Close(ViewId::MatchesMenu) => {
                self.toggle_matches_menu(Rectangle::default(), Some(false), hub, context);
                true
            },
            Event::Close(ViewId::MainMenu) => {
                toggle_main_menu(self, Rectangle::default(), Some(false), hub, context);
                true
            },
            Event::Close(ViewId::GoToPage) => {
                self.toggle_go_to_page(Some(false), hub, context);
                true
            },
            Event::Close(ViewId::AddCategories) |
            Event::Close(ViewId::RenameCategory) |
            Event::Close(ViewId::SaveAs) => {
                self.toggle_keyboard(false, true, None, hub, context);
                false
            },
            Event::Select(EntryId::Sort(sort_method)) => {
                self.set_sort_method(sort_method, hub, context);
                true
            },
            Event::Select(EntryId::ReverseOrder) => {
                let next_value = !self.reverse_order;
                self.set_reverse_order(next_value, hub, context);
                true
            },
            Event::Select(EntryId::Save) => {
                self.save_as(None, context);
                true
            },
            Event::Select(EntryId::SaveAs) => {
                let save_as = NamedInput::new("Save as".to_string(),
                                              ViewId::SaveAs,
                                              ViewId::SaveAsInput,
                                              12, context);
                hub.send(Event::Render(*save_as.rect(), UpdateMode::Gui)).ok();
                hub.send(Event::Focus(Some(ViewId::SaveAsInput))).ok();
                self.children.push(Box::new(save_as) as Box<dyn View>);
                true
            },
            Event::Select(EntryId::Import) => {
                self.import(hub, context);
                true
            },
            Event::Select(EntryId::Load(ref filename)) => {
                self.load(filename, hub, context);
                true
            },
            Event::Select(EntryId::Reload) => {
                self.reload(hub, context);
                true
            },
            Event::Select(EntryId::CleanUp) => {
                self.clean_up(hub, context);
                true
            },
            Event::AddDocument(ref info) => {
                let info2 = info.clone();
                self.add_document(*info2, hub, context);
                true
            },
            Event::Select(EntryId::Remove(ref path)) |
            Event::RemoveDocument(ref path) => {
                self.remove_document(path, hub, context);
                true
            },
            Event::Select(EntryId::RemoveBookCategory(ref path, ref categ)) => {
                self.remove_book_category(path, categ, hub, context);
                true
            },
            Event::Select(ref id @ EntryId::AddBookCategories(..)) |
            Event::Select(ref id @ EntryId::AddMatchesCategories) => {
                if let EntryId::AddBookCategories(ref path) = *id {
                    self.target_path = Some(path.clone());
                }
                let add_categs = NamedInput::new("Add categories".to_string(),
                                                 ViewId::AddCategories,
                                                 ViewId::AddCategoriesInput,
                                                 21, context);
                hub.send(Event::Render(*add_categs.rect(), UpdateMode::Gui)).ok();
                hub.send(Event::Focus(Some(ViewId::AddCategoriesInput))).ok();
                self.children.push(Box::new(add_categs) as Box<dyn View>);
                true
            },
            Event::Select(EntryId::RenameCategory(ref categ_old)) => {
                self.target_category = Some(categ_old.to_string());
                let mut ren_categ = NamedInput::new("Rename category".to_string(),
                                                    ViewId::RenameCategory,
                                                    ViewId::RenameCategoryInput,
                                                    21, context);
                let (tx, _rx) = mpsc::channel();
                ren_categ.set_text(categ_old, &tx, context);
                hub.send(Event::Render(*ren_categ.rect(), UpdateMode::Gui)).ok();
                hub.send(Event::Focus(Some(ViewId::RenameCategoryInput))).ok();
                self.children.push(Box::new(ren_categ) as Box<dyn View>);
                true
            },
            Event::Select(EntryId::RemoveMatches) => {
                self.remove_matches(hub, context);
                true
            },
            Event::Select(EntryId::RemoveCategory(ref categ)) => {
                self.remove_category(categ, hub, context);
                true
            },
            Event::Select(EntryId::SetStatus(ref path, status)) => {
                self.set_status(path, status, hub, context);
                true
            },
            Event::Select(EntryId::EmptyTrash) => {
                trash::empty(context).map_err(|e| eprintln!("Can't empty the trash: {}", e)).ok();
                true
            },
            Event::Select(EntryId::Undo) => {
                self.undo(hub, context);
                true
            },
            Event::Select(EntryId::SecondColumn(second_column)) => {
                context.settings.home.second_column = second_column;
                self.update_second_column(hub, context);
                true
            },
            Event::Select(EntryId::StatusFilter(status_filter)) => {
                if self.status_filter != status_filter {
                    self.status_filter = status_filter;
                    self.refresh_visibles(true, true, hub, context);
                }
                true
            },
            Event::Submit(ViewId::SaveAsInput, ref text) => {
                if !text.is_empty() {
                    self.save_as(Some(text), context);
                }
                self.toggle_keyboard(false, true, None, hub, context);
                true
            },
            Event::Submit(ViewId::AddCategoriesInput, ref text) => {
                let categs = text.split(',')
                                 .map(|s| s.trim().to_string())
                                 .filter(|s| !s.is_empty())
                                 .collect();
                if let Some(ref path) = self.target_path.take() {
                    self.add_book_categories(path, &categs, hub, context);
                } else {
                    self.add_matches_categories(&categs, hub, context);
                }
                self.toggle_keyboard(false, true, None, hub, context);
                true
            },
            Event::Submit(ViewId::RenameCategoryInput, ref categ_new) => {
                if !categ_new.is_empty() {
                    if let Some(ref categ_old) = self.target_category.take() {
                        self.rename_category(categ_old, categ_new, hub, context);
                    }
                }
                self.toggle_keyboard(false, true, None, hub, context);
                true
            },
            Event::Submit(ViewId::HomeSearchInput, ref text) => {
                self.query = make_query(text);
                if self.query.is_some() {
                    // TODO: avoid updating things twice
                    self.toggle_keyboard(false, true, None, hub, context);
                    self.refresh_visibles(true, true, hub, context);
                } else {
                    let notif = Notification::new(ViewId::InvalidSearchQueryNotif,
                                                  "Invalid search query.".to_string(),
                                                  hub,
                                                  context);
                    self.children.push(Box::new(notif) as Box<dyn View>);
                }
                true
            },
            Event::Submit(ViewId::GoToPageInput, ref text) => {
                if let Ok(index) = text.parse::<usize>() {
                    self.go_to_page(index.saturating_sub(1), hub, context);
                }
                true
            },
            Event::ResizeSummary(delta_y) => {
                self.resize_summary(delta_y, true, hub, context);
                true
            },
            Event::ToggleSelectCategory(ref categ) |
            Event::Select(EntryId::ToggleSelectCategory(ref categ)) => {
                self.toggle_select_category(categ, hub, context);
                self.refresh_visibles(true, true, hub, context);
                true
            },
            Event::ToggleNegateCategory(ref categ) => {
                self.toggle_negate_category(categ, hub);
                self.refresh_visibles(true, true, hub, context);
                true
            },
            Event::ToggleNegateCategoryChildren(ref categ) => {
                self.toggle_negate_category_children(categ, hub);
                self.refresh_visibles(true, true, hub, context);
                true
            },
            Event::GoTo(location) => {
                self.go_to_page(location as usize, hub, context);
                true
            },
            Event::Chapter(dir) => {
                let pages_count = self.pages_count;
                match dir {
                    CycleDir::Previous => self.go_to_page(0, hub, context),
                    CycleDir::Next => self.go_to_page(pages_count.saturating_sub(1), hub, context),
                }
                true
            },
            Event::Page(dir) => {
                self.go_to_neighbor(dir, hub, context);
                true
            },
            Event::Device(DeviceEvent::Button { code: ButtonCode::Backward, status: ButtonStatus::Pressed, .. }) => {
                self.go_to_neighbor(CycleDir::Previous, hub, context);
                true
            },
            Event::Device(DeviceEvent::Button { code: ButtonCode::Forward, status: ButtonStatus::Pressed, .. }) => {
                self.go_to_neighbor(CycleDir::Next, hub, context);
                true
            },
            Event::Device(DeviceEvent::NetUp) => {
                for fetcher in self.background_fetchers.values() {
                    if let Some(process) = fetcher.process.as_ref() {
                        unsafe { libc::kill(process.id() as libc::pid_t, libc::SIGUSR1) };
                    }
                }
                true
            },
            Event::ToggleFrontlight => {
                if let Some(index) = locate::<TopBar>(self) {
                    self.child_mut(index).downcast_mut::<TopBar>().unwrap()
                        .update_frontlight_icon(hub, context);
                }
                true
            },
            Event::Reseed => {
                self.reseed(false, hub, context);
                true
            },
            _ => false,
        }
    }

    fn render(&self, _fb: &mut dyn Framebuffer, _rect: Rectangle, _fonts: &mut Fonts) {
    }

    fn resize(&mut self, rect: Rectangle, hub: &Hub, context: &mut Context) {
        let dpi = CURRENT_DEVICE.dpi;
        let thickness = scale_by_dpi(THICKNESS_MEDIUM, dpi) as i32;
        let (small_thickness, big_thickness) = halves(thickness);
        let (_, height) = context.display.dims;
        let &(small_height, big_height) = BAR_SIZES.get(&(height, dpi)).unwrap();
        let (tx, _rx) = mpsc::channel();

        self.children.retain(|child| !child.is::<Menu>());

        // Top bar.
        let top_bar_rect = rect![rect.min.x, rect.min.y,
                                 rect.max.x, rect.min.y + small_height as i32 - small_thickness];
        self.children[0].resize(top_bar_rect, hub, context);

        let separator_rect = rect![rect.min.x, rect.min.y + small_height as i32 - small_thickness,
                                   rect.max.x, rect.min.y + small_height as i32 + big_thickness];
        self.children[1].resize(separator_rect, hub, context);

        // Summary.
        let min_height = if locate::<SearchBar>(self).is_some() {
            big_height as i32 - thickness
        } else {
            small_height as i32 - thickness
        };

        let summary_height = min_height + (self.summary_size - 1) as i32 * big_height as i32;
        let sm_min_y = rect.min.y + small_height as i32 + big_thickness;
        let sm_max_y = sm_min_y + summary_height;
        let summary_rect = rect![rect.min.x, sm_min_y, rect.max.x, sm_max_y];
        self.children[2].resize(summary_rect, hub, context);
        self.update_summary(true, &tx, &mut context.fonts);

        let separator_rect = rect![rect.min.x, sm_max_y,
                                   rect.max.x, sm_max_y + thickness];
        self.children[3].resize(separator_rect, hub, context);

        // Bottom bar.
        let bottom_bar_index = locate::<BottomBar>(self).unwrap_or(6);
        let mut index = bottom_bar_index;

        let separator_rect = rect![rect.min.x, rect.max.y - small_height as i32 - small_thickness,
                                   rect.max.x, rect.max.y - small_height as i32 + big_thickness];
        self.children[index-1].resize(separator_rect, hub, context);

        let bottom_bar_rect = rect![rect.min.x, rect.max.y - small_height as i32 + big_thickness,
                                    rect.max.x, rect.max.y];
        self.children[index].resize(bottom_bar_rect, hub, context);

        let mut shelf_max_y = rect.max.y - small_height as i32 - small_thickness;

        if index > 6 {
            index -= 2;
            // Keyboard.
            if self.children[index].is::<Keyboard>() {
                let kb_rect = rect![rect.min.x,
                                    rect.max.y - (small_height + 3 * big_height) as i32 + big_thickness,
                                    rect.max.x,
                                    rect.max.y - small_height as i32 - small_thickness];
                self.children[index].resize(kb_rect, hub, context);
                let s_max_y = self.children[index].rect().min.y;
                self.children[index-1].resize(rect![rect.min.x, s_max_y - thickness,
                                                    rect.max.x, s_max_y],
                                              hub, context);
                index -= 2;
            }
            // Search bar.
            if self.children[index].is::<SearchBar>() {
                let sp_rect = *self.children[index+1].rect() - pt!(0, small_height as i32);
                self.children[index].resize(rect![rect.min.x,
                                                  sp_rect.max.y,
                                                  rect.max.x,
                                                  sp_rect.max.y + small_height as i32 - thickness],
                                            hub, context);
                self.children[index-1].resize(sp_rect, hub, context);
                shelf_max_y -= small_height as i32;
            }
        }

        let shelf_rect = rect![rect.min.x, sm_max_y + thickness,
                               rect.max.x, shelf_max_y];
        self.children[4].resize(shelf_rect, hub, context);

        self.update_shelf(true, &tx, context);
        self.update_bottom_bar(&tx);

        for i in bottom_bar_index+1..self.children.len() {
            self.children[i].resize(rect, hub, context);
        }

        self.rect = rect;
        hub.send(Event::Render(self.rect, UpdateMode::Full)).ok();
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
