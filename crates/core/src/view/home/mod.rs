mod library_label;
mod address_bar;
mod navigation_bar;
mod directories_bar;
mod directory;
mod shelf;
mod book;
mod bottom_bar;

use std::fs;
use std::mem;
use std::thread;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Child, Stdio};
use std::io::{BufRead, BufReader};
use fxhash::FxHashMap;
use rand_core::RngCore;
use serde_json::{json, Value as JsonValue};
use anyhow::{Error, format_err};
use crate::library::Library;
use crate::framebuffer::{Framebuffer, UpdateMode};
use crate::metadata::{Info, Metadata, SortMethod, BookQuery, SimpleStatus, sort};
use crate::view::{View, Event, Hub, Bus, RenderQueue, RenderData};
use crate::view::{Id, ID_FEEDER, ViewId, EntryId, EntryKind};
use crate::view::{SMALL_BAR_HEIGHT, BIG_BAR_HEIGHT, THICKNESS_MEDIUM};
use crate::settings::{Hook, LibraryMode, FirstColumn, SecondColumn};
use crate::view::common::{toggle_main_menu, toggle_battery_menu, toggle_clock_menu};
use crate::view::common::{locate, rlocate, locate_by_id};
use crate::view::filler::Filler;
use crate::view::keyboard::Keyboard;
use crate::view::named_input::NamedInput;
use crate::view::menu::{Menu, MenuKind};
use crate::view::menu_entry::MenuEntry;
use crate::view::search_bar::SearchBar;
use crate::view::notification::Notification;
use super::top_bar::TopBar;
use self::address_bar::AddressBar;
use self::navigation_bar::NavigationBar;
use self::shelf::Shelf;
use self::bottom_bar::BottomBar;
use crate::gesture::GestureEvent;
use crate::geom::{Rectangle, Dir, DiagDir, CycleDir, halves};
use crate::input::{DeviceEvent, ButtonCode, ButtonStatus};
use crate::device::CURRENT_DEVICE;
use crate::unit::scale_by_dpi;
use crate::color::BLACK;
use crate::font::Fonts;
use crate::context::Context;

pub const TRASH_DIRNAME: &str = ".trash";

#[derive(Debug)]
pub struct Home {
    id: Id,
    rect: Rectangle,
    children: Vec<Box<dyn View>>,
    current_page: usize,
    pages_count: usize,
    shelf_index: usize,
    focus: Option<ViewId>,
    query: Option<BookQuery>,
    sort_method: SortMethod,
    reverse_order: bool,
    visible_books: Metadata,
    current_directory: PathBuf,
    target_document: Option<PathBuf>,
    background_fetchers: FxHashMap<u32, Fetcher>,
}

#[derive(Debug)]
struct Fetcher {
    path: PathBuf,
    full_path: PathBuf,
    process: Child,
    sort_method: Option<SortMethod>,
    first_column: Option<FirstColumn>,
    second_column: Option<SecondColumn>,
}

impl Home {
    pub fn new(rect: Rectangle, hub: &Hub, rq: &mut RenderQueue, context: &mut Context) -> Result<Home, Error> {
        let id = ID_FEEDER.next();
        let dpi = CURRENT_DEVICE.dpi;
        let mut children = Vec::new();

        let thickness = scale_by_dpi(THICKNESS_MEDIUM, dpi) as i32;
        let (small_thickness, big_thickness) = halves(thickness);
        let (small_height, big_height) = (scale_by_dpi(SMALL_BAR_HEIGHT, dpi) as i32,
                                          scale_by_dpi(BIG_BAR_HEIGHT, dpi) as i32);

        let selected_library = context.settings.selected_library;
        let library_settings = &context.settings.libraries[selected_library];

        let current_directory = context.library.home.clone();
        let sort_method = library_settings.sort_method;
        let reverse_order = sort_method.reverse_order();

        context.library.sort(sort_method, reverse_order);

        let (visible_books, dirs) = context.library.list(&current_directory, None, false);
        let count = visible_books.len();
        let current_page = 0;
        let mut shelf_index = 2;

        let top_bar = TopBar::new(rect![rect.min.x, rect.min.y,
                                        rect.max.x, rect.min.y + small_height - small_thickness],
                                  Event::Toggle(ViewId::SearchBar),
                                  sort_method.title(),
                                  context);
        children.push(Box::new(top_bar) as Box<dyn View>);

        let separator = Filler::new(rect![rect.min.x, rect.min.y + small_height - small_thickness,
                                          rect.max.x, rect.min.y + small_height + big_thickness],
                                    BLACK);
        children.push(Box::new(separator) as Box<dyn View>);

        let mut y_start = rect.min.y + small_height + big_thickness;

        if context.settings.home.address_bar {
            let addr_bar = AddressBar::new(rect![rect.min.x, y_start,
                                                 rect.max.x, y_start + small_height - thickness],
                                           current_directory.to_string_lossy(),
                                           context);
            children.push(Box::new(addr_bar) as Box<dyn View>);
            y_start += small_height - thickness;

            let separator = Filler::new(rect![rect.min.x, y_start,
                                              rect.max.x, y_start + thickness],
                                        BLACK);
            children.push(Box::new(separator) as Box<dyn View>);
            y_start += thickness;
            shelf_index += 2;
        }

        if context.settings.home.navigation_bar {
            let mut nav_bar = NavigationBar::new(rect![rect.min.x, y_start,
                                                       rect.max.x, y_start + small_height - thickness],
                                                 rect.max.y - small_height - big_height - small_thickness,
                                                 context.settings.home.max_levels);

            nav_bar.set_path(&current_directory, &dirs, &mut RenderQueue::new(), context);
            y_start = nav_bar.rect().max.y;

            children.push(Box::new(nav_bar) as Box<dyn View>);

            let separator = Filler::new(rect![rect.min.x, y_start,
                                              rect.max.x, y_start + thickness],
                                        BLACK);
            children.push(Box::new(separator) as Box<dyn View>);
            y_start += thickness;
            shelf_index += 2;
        }


        let selected_library = context.settings.selected_library;
        let library_settings = &context.settings.libraries[selected_library];

        let mut shelf = Shelf::new(rect![rect.min.x, y_start,
                                         rect.max.x, rect.max.y - small_height - small_thickness],
                                   library_settings.first_column,
                                   library_settings.second_column,
                                   library_settings.thumbnail_previews);


        let max_lines = shelf.max_lines;
        let pages_count = (visible_books.len() as f32 / max_lines as f32).ceil() as usize;
        let index_lower = current_page * max_lines;
        let index_upper = (index_lower + max_lines).min(visible_books.len());

        shelf.update(&visible_books[index_lower..index_upper], hub, &mut RenderQueue::new(), context);

        children.push(Box::new(shelf) as Box<dyn View>);

        let separator = Filler::new(rect![rect.min.x, rect.max.y - small_height - small_thickness,
                                          rect.max.x, rect.max.y - small_height + big_thickness],
                                    BLACK);
        children.push(Box::new(separator) as Box<dyn View>);

        let bottom_bar = BottomBar::new(rect![rect.min.x, rect.max.y - small_height + big_thickness,
                                              rect.max.x, rect.max.y],
                                        current_page,
                                        pages_count,
                                        &library_settings.name,
                                        count,
                                        false);
        children.push(Box::new(bottom_bar) as Box<dyn View>);

        rq.add(RenderData::new(id, rect, UpdateMode::Full));

        Ok(Home {
            id,
            rect,
            children,
            current_page,
            pages_count,
            shelf_index,
            focus: None,
            query: None,
            sort_method,
            reverse_order,
            visible_books,
            current_directory,
            target_document: None,
            background_fetchers: FxHashMap::default(),
        })
    }

    fn select_directory(&mut self, path: &Path, hub: &Hub, rq: &mut RenderQueue, context: &mut Context) {
        if self.current_directory == path {
            return;
        }

        let old_path = mem::replace(&mut self.current_directory, path.to_path_buf());
        self.terminate_fetchers(&old_path, true, hub, context);

        let selected_library = context.settings.selected_library;
        for hook in &context.settings.libraries[selected_library].hooks {
            if context.library.home.join(&hook.path) == path {
                self.insert_fetcher(hook, hub, context);
            }
        }

        let (files, dirs) = context.library.list(&self.current_directory,
                                                 self.query.as_ref(),
                                                 false);
        self.visible_books = files;
        self.current_page = 0;

        let mut index = 2;

        if context.settings.home.address_bar {
            let addr_bar = self.children[index].as_mut().downcast_mut::<AddressBar>().unwrap();
            addr_bar.set_text(self.current_directory.to_string_lossy(), rq, context);
            index += 2;
        }

        if context.settings.home.navigation_bar {
            let nav_bar = self.children[index].as_mut().downcast_mut::<NavigationBar>().unwrap();
            nav_bar.set_path(&self.current_directory, &dirs, rq, context);
            self.adjust_shelf_top_edge();
            rq.add(RenderData::new(self.child(index+1).id(),
                                   *self.child(index+1).rect(),
                                   UpdateMode::Partial));
            rq.add(RenderData::new(self.child(index).id(),
                                   *self.child(index).rect(),
                                   UpdateMode::Partial));
        }

        self.update_shelf(true, hub, rq, context);
        self.update_bottom_bar(rq, context);
    }

    fn adjust_shelf_top_edge(&mut self) {
        let index = self.shelf_index - 2;
        let y_shift = self.children[index].rect().max.y - self.children[index+1].rect().min.y;
        *self.children[index+1].rect_mut() += pt!(0, y_shift);
        self.children[index+2].rect_mut().min.y = self.children[index+1].rect().max.y;
    }

    fn toggle_select_directory(&mut self, path: &Path, hub: &Hub, rq: &mut RenderQueue, context: &mut Context) {
        if self.current_directory.starts_with(path) {
            if let Some(parent) = path.parent() {
                self.select_directory(parent, hub, rq, context);
            }
        } else {
            self.select_directory(path, hub, rq, context);
        }
    }

    fn go_to_page(&mut self, index: usize, hub: &Hub, rq: &mut RenderQueue, context: &mut Context) {
        if index >= self.pages_count {
            return;
        }
        self.current_page = index;
        self.update_shelf(false, hub, rq, context);
        self.update_bottom_bar(rq, context);
    }

    fn go_to_neighbor(&mut self, dir: CycleDir, hub: &Hub, rq: &mut RenderQueue, context: &mut Context) {
        match dir {
            CycleDir::Next if self.current_page < self.pages_count.saturating_sub(1) => {
                self.current_page += 1;
            },
            CycleDir::Previous if self.current_page > 0 => {
                self.current_page -= 1;
            },
            _ => return,
        }

        self.update_shelf(false, hub, rq, context);
        self.update_bottom_bar(rq, context);
    }

    fn go_to_status_change(&mut self, dir: CycleDir, hub: &Hub, rq: &mut RenderQueue, context: &mut Context) {
        if self.pages_count < 2 {
            return;
        }

        let max_lines = self.children[self.shelf_index].as_ref().downcast_ref::<Shelf>().unwrap().max_lines;
        let index_lower = self.current_page * max_lines;
        let index_upper = (index_lower + max_lines).min(self.visible_books.len());
        let book_index = match dir {
            CycleDir::Next => index_upper.saturating_sub(1),
            CycleDir::Previous => index_lower,
        };
        let status = self.visible_books[book_index].simple_status();

        let page = match dir {
            CycleDir::Next => self.visible_books[book_index+1..].iter()
                                  .position(|info| info.simple_status() != status)
                                  .map(|delta| self.current_page + 1 + delta / max_lines),
            CycleDir::Previous => self.visible_books[..book_index].iter().rev()
                                      .position(|info| info.simple_status() != status)
                                      .map(|delta| self.current_page - 1 - delta / max_lines),
        };

        if let Some(page) = page {
            self.current_page = page;
            self.update_shelf(false, hub, rq, context);
            self.update_bottom_bar(rq, context);
        }
    }

    // NOTE: This function assumes that the shelf wasn't resized.
    fn refresh_visibles(&mut self, update: bool, reset_page: bool, hub: &Hub, rq: &mut RenderQueue, context: &mut Context) {
        let (files, _) = context.library.list(&self.current_directory,
                                              self.query.as_ref(),
                                              false);
        self.visible_books = files;

        let max_lines = {
            let shelf = self.child(self.shelf_index).downcast_ref::<Shelf>().unwrap();
            shelf.max_lines
        };

        self.pages_count = (self.visible_books.len() as f32 / max_lines as f32).ceil() as usize;

        if reset_page  {
            self.current_page = 0;
        } else if self.current_page >= self.pages_count {
            self.current_page = self.pages_count.saturating_sub(1);
        }

        if update {
            self.update_shelf(false, hub, rq, context);
            self.update_bottom_bar(rq, context);
        }
    }

    fn update_first_column(&mut self, hub: &Hub, rq: &mut RenderQueue, context: &mut Context) {
        let selected_library = context.settings.selected_library;
        self.children[self.shelf_index].as_mut().downcast_mut::<Shelf>().unwrap()
           .set_first_column(context.settings.libraries[selected_library].first_column);
        self.update_shelf(false, hub, rq, context);
    }

    fn update_second_column(&mut self, hub: &Hub, rq: &mut RenderQueue, context: &mut Context) {
        let selected_library = context.settings.selected_library;
        self.children[self.shelf_index].as_mut().downcast_mut::<Shelf>().unwrap()
           .set_second_column(context.settings.libraries[selected_library].second_column);
        self.update_shelf(false, hub, rq, context);
    }

    fn update_thumbnail_previews(&mut self, hub: &Hub, rq: &mut RenderQueue, context: &mut Context) {
        let selected_library = context.settings.selected_library;
        self.children[self.shelf_index].as_mut().downcast_mut::<Shelf>().unwrap()
           .set_thumbnail_previews(context.settings.libraries[selected_library].thumbnail_previews);
        self.update_shelf(false, hub, rq, context);
    }

    fn update_shelf(&mut self, was_resized: bool, hub: &Hub, rq: &mut RenderQueue, context: &mut Context) {
        let dpi = CURRENT_DEVICE.dpi;
        let big_height = scale_by_dpi(BIG_BAR_HEIGHT, dpi) as i32;
        let thickness = scale_by_dpi(THICKNESS_MEDIUM, dpi) as i32;
        let shelf = self.children[self.shelf_index].as_mut().downcast_mut::<Shelf>().unwrap();
        let max_lines = ((shelf.rect.height() as i32 + thickness) / big_height) as usize;

        if was_resized {
            let page_position = if self.visible_books.is_empty() {
                0.0
            } else {
                self.current_page as f32 * (shelf.max_lines as f32 /
                                            self.visible_books.len() as f32)
            };

            let mut page_guess = page_position * self.visible_books.len() as f32 / max_lines as f32;
            let page_ceil = page_guess.ceil();

            if (page_ceil - page_guess).abs() < f32::EPSILON {
                page_guess = page_ceil;
            }

            self.pages_count = (self.visible_books.len() as f32 / max_lines as f32).ceil() as usize;
            self.current_page = (page_guess as usize).min(self.pages_count.saturating_sub(1));
        }

        let index_lower = self.current_page * max_lines;
        let index_upper = (index_lower + max_lines).min(self.visible_books.len());

        shelf.update(&self.visible_books[index_lower..index_upper], hub, rq, context);
    }

    fn update_top_bar(&mut self, search_visible: bool, rq: &mut RenderQueue) {
        if let Some(index) = locate::<TopBar>(self) {
            let top_bar = self.children[index].as_mut().downcast_mut::<TopBar>().unwrap();
            let name = if search_visible { "back" } else { "search" };
            top_bar.update_root_icon(name, rq);
            top_bar.update_title_label(&self.sort_method.title(), rq);
        }
    }

    fn update_bottom_bar(&mut self, rq: &mut RenderQueue, context: &Context) {
        if let Some(index) = rlocate::<BottomBar>(self) {
            let bottom_bar = self.children[index].as_mut().downcast_mut::<BottomBar>().unwrap();
            let filter = self.query.is_some() ||
                         self.current_directory != context.library.home;
            let selected_library = context.settings.selected_library;
            let library_settings = &context.settings.libraries[selected_library];
            bottom_bar.update_library_label(&library_settings.name, self.visible_books.len(), filter, rq);
            bottom_bar.update_page_label(self.current_page, self.pages_count, rq);
            bottom_bar.update_icons(self.current_page, self.pages_count, rq);
        }
    }

    fn toggle_keyboard(&mut self, enable: bool, update: bool, id: Option<ViewId>, hub: &Hub, rq: &mut RenderQueue, context: &mut Context) {
        let dpi = CURRENT_DEVICE.dpi;
        let (small_height, big_height) = (scale_by_dpi(SMALL_BAR_HEIGHT, dpi) as i32,
                                          scale_by_dpi(BIG_BAR_HEIGHT, dpi) as i32);
        let thickness = scale_by_dpi(THICKNESS_MEDIUM, dpi) as i32;
        let (small_thickness, big_thickness) = halves(thickness);
        let has_search_bar = self.children[self.shelf_index+2].is::<SearchBar>();

        if let Some(index) = rlocate::<Keyboard>(self) {
            if enable {
                return;
            }

            let y_min = self.child(self.shelf_index+1).rect().min.y;
            let mut rect = *self.child(index).rect();
            rect.absorb(self.child(index-1).rect());

            self.children.drain(index - 1 ..= index);

            let delta_y = rect.height() as i32;

            if has_search_bar {
                for i in self.shelf_index+1..=self.shelf_index+2 {
                    let shifted_rect = *self.child(i).rect() + pt!(0, delta_y);
                    self.child_mut(i).resize(shifted_rect, hub, rq, context);
                }
            }

            hub.send(Event::Focus(None)).ok();
            if update {
                let rect = rect![self.rect.min.x, y_min,
                                 self.rect.max.x, y_min + delta_y];
                rq.add(RenderData::expose(rect, UpdateMode::Gui));
            }
        } else {
            if !enable {
                return;
            }

            let index = rlocate::<BottomBar>(self).unwrap() - 1;
            let mut kb_rect = rect![self.rect.min.x,
                                    self.rect.max.y - (small_height + 3 * big_height) as i32 + big_thickness,
                                    self.rect.max.x,
                                    self.rect.max.y - small_height - small_thickness];

            let number = matches!(id, Some(ViewId::GoToPageInput));
            let keyboard = Keyboard::new(&mut kb_rect, number, context);
            self.children.insert(index, Box::new(keyboard) as Box<dyn View>);

            let separator = Filler::new(rect![self.rect.min.x, kb_rect.min.y - thickness,
                                              self.rect.max.x, kb_rect.min.y],
                                        BLACK);
            self.children.insert(index, Box::new(separator) as Box<dyn View>);

            let delta_y = kb_rect.height() as i32 + thickness;

            if has_search_bar {
                for i in self.shelf_index+1..=self.shelf_index+2 {
                    let shifted_rect = *self.child(i).rect() + pt!(0, -delta_y);
                    self.child_mut(i).resize(shifted_rect, hub, rq, context);
                }
            }
        }

        if update {
            if enable {
                if has_search_bar {
                    for i in self.shelf_index+1..=self.shelf_index+4 {
                        let update_mode = if (i - self.shelf_index) == 1 { UpdateMode::Partial } else { UpdateMode::Gui };
                        rq.add(RenderData::new(self.child(i).id(), *self.child(i).rect(), update_mode));
                    }
                } else {
                    for i in self.shelf_index+1..=self.shelf_index+2 {
                        rq.add(RenderData::new(self.child(i).id(), *self.child(i).rect(), UpdateMode::Gui));
                    }
                }
            } else if has_search_bar {
                for i in self.shelf_index+1..=self.shelf_index+2 {
                    rq.add(RenderData::new(self.child(i).id(), *self.child(i).rect(), UpdateMode::Gui));
                }
            }
        }
    }

    fn toggle_address_bar(&mut self, enable: Option<bool>, update: bool, hub: &Hub, rq: &mut RenderQueue, context: &mut Context) {
        let dpi = CURRENT_DEVICE.dpi;
        let (small_height, big_height) = (scale_by_dpi(SMALL_BAR_HEIGHT, dpi) as i32,
                                          scale_by_dpi(BIG_BAR_HEIGHT, dpi) as i32);
        let thickness = scale_by_dpi(THICKNESS_MEDIUM, dpi) as i32;

        if let Some(index) = locate::<AddressBar>(self) {
            if let Some(true) = enable {
                return;
            }

            if let Some(ViewId::AddressBarInput) = self.focus {
                self.toggle_keyboard(false, false, Some(ViewId::AddressBarInput), hub, rq, context);
            }

            // Remove the address bar and its separator.
            self.children.drain(index ..= index + 1);
            self.shelf_index -= 2;
            context.settings.home.address_bar = false;

            // Move the navigation bar up.
            if context.settings.home.navigation_bar {
                let nav_bar = self.children[self.shelf_index-2]
                                  .downcast_mut::<NavigationBar>().unwrap();
                nav_bar.shift(pt!(0, -small_height));
            }

            // Move the separator above the shelf up.
            *self.children[self.shelf_index-1].rect_mut() += pt!(0, -small_height);

            // Move the shelf's top edge up.
            self.children[self.shelf_index].rect_mut().min.y -= small_height;
        } else {
            if let Some(false) = enable {
                return;
            }

            let sp_rect = *self.child(1).rect() + pt!(0, small_height);

            let separator = Filler::new(sp_rect, BLACK);
            self.children.insert(2, Box::new(separator) as Box<dyn View>);

            let addr_bar = AddressBar::new(rect![self.rect.min.x,
                                                 sp_rect.min.y - small_height + thickness,
                                                 self.rect.max.x,
                                                 sp_rect.min.y],
                                           self.current_directory.to_string_lossy(),
                                           context);
            self.children.insert(2, Box::new(addr_bar) as Box<dyn View>);

            self.shelf_index += 2;
            context.settings.home.address_bar = true;

            // Move the separator above the shelf down.
            *self.children[self.shelf_index-1].rect_mut() += pt!(0, small_height);

            // Move the shelf's top edge down.
            self.children[self.shelf_index].rect_mut().min.y += small_height;

            if context.settings.home.navigation_bar {
                let rect = *self.children[self.shelf_index].rect();
                let y_shift = rect.height() as i32 - (big_height - thickness);
                let nav_bar = self.children[self.shelf_index-2]
                                  .downcast_mut::<NavigationBar>().unwrap();
                // Move the navigation bar down.
                nav_bar.shift(pt!(0, small_height));

                // Shrink the nav bar.
                if y_shift < 0 {
                    let y_shift = nav_bar.shrink(y_shift, &mut context.fonts);
                    self.children[self.shelf_index].rect_mut().min.y += y_shift;
                    *self.children[self.shelf_index-1].rect_mut() += pt!(0, y_shift);
                }
            }
        }

        if update {
            for i in 2..self.shelf_index {
                rq.add(RenderData::new(self.child(i).id(), *self.child(i).rect(), UpdateMode::Gui));
            }

            self.update_shelf(true, hub, rq, context);
            self.update_bottom_bar(rq, context);
        }
    }

    fn toggle_navigation_bar(&mut self, enable: Option<bool>, update: bool, hub: &Hub, rq: &mut RenderQueue, context: &mut Context) {
        let dpi = CURRENT_DEVICE.dpi;
        let (small_height, big_height) = (scale_by_dpi(SMALL_BAR_HEIGHT, dpi) as i32,
                                          scale_by_dpi(BIG_BAR_HEIGHT, dpi) as i32);
        let thickness = scale_by_dpi(THICKNESS_MEDIUM, dpi) as i32;
        let (small_thickness, _) = halves(thickness);

        if let Some(index) = locate::<NavigationBar>(self) {
            if let Some(true) = enable {
                return;
            }

            let mut rect = *self.child(index).rect();
            rect.absorb(self.child(index+1).rect());
            let delta_y = rect.height() as i32;

            // Remove the navigation bar and its separator.
            self.children.drain(index ..= index + 1);
            self.shelf_index -= 2;
            context.settings.home.navigation_bar = false;

            // Move the shelf's top edge up.
            self.children[self.shelf_index].rect_mut().min.y -= delta_y;
        } else {
            if let Some(false) = enable {
                return;
            }

            let sep_index = if context.settings.home.address_bar { 3 } else { 1 };
            let sp_rect = *self.child(sep_index).rect() + pt!(0, small_height);

            let separator = Filler::new(sp_rect, BLACK);
            self.children.insert(sep_index+1, Box::new(separator) as Box<dyn View>);

            let mut nav_bar = NavigationBar::new(rect![self.rect.min.x,
                                                       sp_rect.min.y - small_height + thickness,
                                                       self.rect.max.x,
                                                       sp_rect.min.y],
                                                 self.rect.max.y - small_height - big_height - small_thickness,
                                                 context.settings.home.max_levels);
            let (_, dirs) = context.library.list(&self.current_directory, None, true);
            nav_bar.set_path(&self.current_directory, &dirs, rq, context);
            self.children.insert(sep_index+1, Box::new(nav_bar) as Box<dyn View>);

            self.shelf_index += 2;
            context.settings.home.navigation_bar = true;

            self.adjust_shelf_top_edge();
        }

        if update {
            for i in 2..self.shelf_index {
                rq.add(RenderData::new(self.child(i).id(), *self.child(i).rect(), UpdateMode::Gui));
            }

            self.update_shelf(true, hub, rq, context);
            self.update_bottom_bar(rq, context);
        }
    }

    fn toggle_search_bar(&mut self, enable: Option<bool>, update: bool, hub: &Hub, rq: &mut RenderQueue, context: &mut Context) {
        let dpi = CURRENT_DEVICE.dpi;
        let (small_height, big_height) = (scale_by_dpi(SMALL_BAR_HEIGHT, dpi) as i32,
                                          scale_by_dpi(BIG_BAR_HEIGHT, dpi) as i32);
        let thickness = scale_by_dpi(THICKNESS_MEDIUM, dpi) as i32;
        let delta_y = small_height;
        let search_visible: bool;
        let mut has_keyboard = false;

        if let Some(index) = rlocate::<SearchBar>(self) {
            if let Some(true) = enable {
                return;
            }

            if let Some(ViewId::HomeSearchInput) = self.focus {
                self.toggle_keyboard(false, false, Some(ViewId::HomeSearchInput), hub, rq, context);
            }

            // Remove the search bar and its separator.
            self.children.drain(index - 1 ..= index);

            // Move the shelf's bottom edge.
            self.children[self.shelf_index].rect_mut().max.y += delta_y;

            if context.settings.home.navigation_bar {
                let nav_bar = self.children[self.shelf_index-2]
                                  .downcast_mut::<NavigationBar>().unwrap();
                nav_bar.vertical_limit += delta_y;
            }

            self.query = None;
            search_visible = false;
        } else {
            if let Some(false) = enable {
                return;
            }

            let sp_rect = *self.child(self.shelf_index+1).rect() - pt!(0, delta_y);
            let search_bar = SearchBar::new(rect![self.rect.min.x, sp_rect.max.y,
                                                  self.rect.max.x,
                                                  sp_rect.max.y + delta_y - thickness],
                                            ViewId::HomeSearchInput,
                                            "Title, author, series",
                                            "", context);
            self.children.insert(self.shelf_index+1, Box::new(search_bar) as Box<dyn View>);

            let separator = Filler::new(sp_rect, BLACK);
            self.children.insert(self.shelf_index+1, Box::new(separator) as Box<dyn View>);

            // Move the shelf's bottom edge.
            self.children[self.shelf_index].rect_mut().max.y -= delta_y;

            if context.settings.home.navigation_bar {
                let rect = *self.children[self.shelf_index].rect();
                let y_shift = rect.height() as i32 - (big_height - thickness);
                let nav_bar = self.children[self.shelf_index-2]
                                  .downcast_mut::<NavigationBar>().unwrap();
                nav_bar.vertical_limit -= delta_y;

                // Shrink the nav bar.
                if y_shift < 0 {
                    let y_shift = nav_bar.shrink(y_shift, &mut context.fonts);
                    self.children[self.shelf_index].rect_mut().min.y += y_shift;
                    *self.children[self.shelf_index-1].rect_mut() += pt!(0, y_shift);
                }
            }

            if self.query.is_none() {
                if rlocate::<Keyboard>(self).is_none() {
                    self.toggle_keyboard(true, false, Some(ViewId::HomeSearchInput), hub, rq, context);
                    has_keyboard = true;
                }

                hub.send(Event::Focus(Some(ViewId::HomeSearchInput))).ok();
            }

            search_visible = true;
        }

        if update {
            if !search_visible {
                self.refresh_visibles(false, true, hub, rq, context);
            }

            self.update_top_bar(search_visible, rq);

            if search_visible {
                rq.add(RenderData::new(self.child(self.shelf_index-1).id(), *self.child(self.shelf_index-1).rect(), UpdateMode::Partial));
                let mut rect = *self.child(self.shelf_index).rect();
                rect.max.y = self.child(self.shelf_index+1).rect().min.y;
                // Render the part of the shelf that isn't covered.
                self.update_shelf(true, hub, &mut RenderQueue::new(), context);
                rq.add(RenderData::new(self.child(self.shelf_index).id(), rect, UpdateMode::Partial));
                // Render the views on top of the shelf.
                rect.min.y = rect.max.y;
                let end_index = self.shelf_index + if has_keyboard { 4 } else { 2 };
                rect.max.y = self.child(end_index).rect().max.y;
                rq.add(RenderData::expose(rect, UpdateMode::Partial));
            } else {
                for i in self.shelf_index - 1 ..= self.shelf_index + 1 {
                    if i == self.shelf_index {
                        self.update_shelf(true, hub, rq, context);
                        continue;
                    }
                    rq.add(RenderData::new(self.child(i).id(), *self.child(i).rect(), UpdateMode::Partial));
                }
            }

            self.update_bottom_bar(rq, context);
        }
    }

    fn toggle_rename_document(&mut self, enable: Option<bool>, hub: &Hub, rq: &mut RenderQueue, context: &mut Context) {
        if let Some(index) = locate_by_id(self, ViewId::RenameDocument) {
            if let Some(true) = enable {
                return;
            }
            self.target_document = None;
            rq.add(RenderData::expose(*self.child(index).rect(), UpdateMode::Gui));
            self.children.remove(index);
            if let Some(ViewId::RenameDocumentInput) = self.focus {
                self.toggle_keyboard(false, true, Some(ViewId::RenameDocumentInput), hub, rq, context);
            }
        } else {
            if let Some(false) = enable {
                return;
            }
            let mut ren_doc = NamedInput::new("Rename document".to_string(),
                                              ViewId::RenameDocument,
                                              ViewId::RenameDocumentInput,
                                              21, context);
            if let Some(text) = self.target_document.as_ref()
                                    .and_then(|path| path.file_name())
                                    .and_then(|file_name| file_name.to_str()) {
                ren_doc.set_text(text, rq, context);
            }
            rq.add(RenderData::new(ren_doc.id(), *ren_doc.rect(), UpdateMode::Gui));
            hub.send(Event::Focus(Some(ViewId::RenameDocumentInput))).ok();
            self.children.push(Box::new(ren_doc) as Box<dyn View>);
        }
    }

    fn toggle_go_to_page(&mut self, enable: Option<bool>, hub: &Hub, rq: &mut RenderQueue, context: &mut Context) {
        if let Some(index) = locate_by_id(self, ViewId::GoToPage) {
            if let Some(true) = enable {
                return;
            }
            rq.add(RenderData::expose(*self.child(index).rect(), UpdateMode::Gui));
            self.children.remove(index);
            if let Some(ViewId::GoToPageInput) = self.focus {
                self.toggle_keyboard(false, true, Some(ViewId::GoToPageInput), hub, rq, context);
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
            rq.add(RenderData::new(go_to_page.id(), *go_to_page.rect(), UpdateMode::Gui));
            hub.send(Event::Focus(Some(ViewId::GoToPageInput))).ok();
            self.children.push(Box::new(go_to_page) as Box<dyn View>);
        }
    }

    fn toggle_sort_menu(&mut self, rect: Rectangle, enable: Option<bool>, rq: &mut RenderQueue, context: &mut Context) {
        if let Some(index) = locate_by_id(self, ViewId::SortMenu) {
            if let Some(true) = enable {
                return;
            }
            rq.add(RenderData::expose(*self.child(index).rect(), UpdateMode::Gui));
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
                               EntryKind::RadioButton("Status".to_string(),
                                                      EntryId::Sort(SortMethod::Status),
                                                      self.sort_method == SortMethod::Status),
                               EntryKind::RadioButton("Progress".to_string(),
                                                      EntryId::Sort(SortMethod::Progress),
                                                      self.sort_method == SortMethod::Progress),
                               EntryKind::RadioButton("Author".to_string(),
                                                      EntryId::Sort(SortMethod::Author),
                                                      self.sort_method == SortMethod::Author),
                               EntryKind::RadioButton("Title".to_string(),
                                                      EntryId::Sort(SortMethod::Title),
                                                      self.sort_method == SortMethod::Title),
                               EntryKind::RadioButton("Year".to_string(),
                                                      EntryId::Sort(SortMethod::Year),
                                                      self.sort_method == SortMethod::Year),
                               EntryKind::RadioButton("Series".to_string(),
                                                      EntryId::Sort(SortMethod::Series),
                                                      self.sort_method == SortMethod::Series),
                               EntryKind::RadioButton("File Size".to_string(),
                                                      EntryId::Sort(SortMethod::Size),
                                                      self.sort_method == SortMethod::Size),
                               EntryKind::RadioButton("File Type".to_string(),
                                                      EntryId::Sort(SortMethod::Kind),
                                                      self.sort_method == SortMethod::Kind),
                               EntryKind::RadioButton("File Name".to_string(),
                                                      EntryId::Sort(SortMethod::FileName),
                                                      self.sort_method == SortMethod::FileName),
                               EntryKind::RadioButton("File Path".to_string(),
                                                      EntryId::Sort(SortMethod::FilePath),
                                                      self.sort_method == SortMethod::FilePath),
                               EntryKind::Separator,
                               EntryKind::CheckBox("Reverse Order".to_string(),
                                                   EntryId::ReverseOrder, self.reverse_order)];
            let sort_menu = Menu::new(rect, ViewId::SortMenu, MenuKind::DropDown, entries, context);
            rq.add(RenderData::new(sort_menu.id(), *sort_menu.rect(), UpdateMode::Gui));
            self.children.push(Box::new(sort_menu) as Box<dyn View>);
        }
    }

    fn book_index(&self, index: usize) -> usize {
        let max_lines = self.child(self.shelf_index).downcast_ref::<Shelf>().unwrap().max_lines;
        let index_lower = self.current_page * max_lines;
        (index_lower + index).min(self.visible_books.len())
    }

    fn toggle_book_menu(&mut self, index: usize, rect: Rectangle, enable: Option<bool>, rq: &mut RenderQueue, context: &mut Context) {
        if let Some(index) = locate_by_id(self, ViewId::BookMenu) {
            if let Some(true) = enable {
                return;
            }
            rq.add(RenderData::expose(*self.child(index).rect(), UpdateMode::Gui));
            self.children.remove(index);
        } else {
            if let Some(false) = enable {
                return;
            }

            let book_index = self.book_index(index);
            let info = &self.visible_books[book_index];
            let path = &info.file.path;

            let mut entries = Vec::new();

            if let Some(parent) = path.parent() {
                entries.push(EntryKind::Command("Select Parent".to_string(),
                                                EntryId::SelectDirectory(context.library.home.join(parent))));
            }

            if !info.author.is_empty() {
                entries.push(EntryKind::Command("Search Author".to_string(),
                                                EntryId::SearchAuthor(info.author.clone())));
            }

            if !entries.is_empty() {
                entries.push(EntryKind::Separator);
            }

            let submenu: &[SimpleStatus] = match info.simple_status() {
                SimpleStatus::New => &[SimpleStatus::Reading, SimpleStatus::Finished],
                SimpleStatus::Reading => &[SimpleStatus::New, SimpleStatus::Finished],
                SimpleStatus::Finished => &[SimpleStatus::New, SimpleStatus::Reading],
            };

            let submenu = submenu.iter().map(|s| EntryKind::Command(s.to_string(),
                                                                    EntryId::SetStatus(path.clone(), *s)))
                                 .collect();
            entries.push(EntryKind::SubMenu("Mark As".to_string(), submenu));
            entries.push(EntryKind::Separator);

            let selected_library = context.settings.selected_library;
            let libraries = context.settings.libraries.iter().enumerate()
                                   .filter(|(index, _)| *index != selected_library)
                                   .map(|(index, lib)| (index, lib.name.clone()))
                                   .collect::<Vec<(usize, String)>>();
            if !libraries.is_empty() {
                let copy_to = libraries.iter().map(|(index, name)| {
                    EntryKind::Command(name.clone(),
                                       EntryId::CopyTo(path.clone(), *index))
                }).collect::<Vec<EntryKind>>();
                let move_to = libraries.iter().map(|(index, name)| {
                    EntryKind::Command(name.clone(),
                                       EntryId::MoveTo(path.clone(), *index))
                }).collect::<Vec<EntryKind>>();
                entries.push(EntryKind::SubMenu("Copy To".to_string(), copy_to));
                entries.push(EntryKind::SubMenu("Move To".to_string(), move_to));
            }

            entries.push(EntryKind::Command("Rename".to_string(),
                                            EntryId::Rename(path.clone())));
            entries.push(EntryKind::Command("Remove".to_string(),
                                            EntryId::Remove(path.clone())));


            let book_menu = Menu::new(rect, ViewId::BookMenu, MenuKind::Contextual, entries, context);
            rq.add(RenderData::new(book_menu.id(), *book_menu.rect(), UpdateMode::Gui));
            self.children.push(Box::new(book_menu) as Box<dyn View>);
        }
    }

    fn toggle_library_menu(&mut self, rect: Rectangle, enable: Option<bool>, rq: &mut RenderQueue, context: &mut Context) {
        if let Some(index) = locate_by_id(self, ViewId::LibraryMenu) {
            if let Some(true) = enable {
                return;
            }

            rq.add(RenderData::expose(*self.child(index).rect(), UpdateMode::Gui));
            self.children.remove(index);
        } else {
            if let Some(false) = enable {
                return;
            }

            let selected_library = context.settings.selected_library;
            let library_settings = &context.settings.libraries[selected_library];

            let libraries: Vec<EntryKind> = context.settings.libraries.iter().enumerate().map(|(index, lib)| {
                EntryKind::RadioButton(lib.name.clone(), EntryId::LoadLibrary(index), index == selected_library)
            }).collect();

            let database = if library_settings.mode == LibraryMode::Database {
                vec![EntryKind::Command("Import".to_string(), EntryId::Import),
                     EntryKind::Command("Flush".to_string(), EntryId::Flush)]
            } else {
                Vec::new()
            };

            let filesystem = if library_settings.mode == LibraryMode::Filesystem {
               vec![EntryKind::CheckBox("Show Hidden".to_string(), EntryId::ToggleShowHidden, context.library.show_hidden),
                    EntryKind::Separator,
                    EntryKind::Command("Clean Up".to_string(), EntryId::CleanUp),
                    EntryKind::Command("Flush".to_string(), EntryId::Flush)]
            } else {
                Vec::new()
            };

            let mut entries = vec![EntryKind::SubMenu("Library".to_string(), libraries)];

            if !database.is_empty() {
                entries.push(EntryKind::SubMenu("Database".to_string(), database));
            }

            if !filesystem.is_empty() {
                entries.push(EntryKind::SubMenu("Filesystem".to_string(), filesystem));
            }

            let hooks: Vec<EntryKind> =
                context.settings.libraries[selected_library].hooks.iter()
                       .map(|v| EntryKind::Command(v.path.to_string_lossy().into_owned(),
                                                   EntryId::ToggleSelectDirectory(context.library.home.join(&v.path)))).collect();

            if !hooks.is_empty() {
                entries.push(EntryKind::SubMenu("Toggle Select".to_string(), hooks));
            }

            entries.push(EntryKind::Separator);

            let first_column = library_settings.first_column;
            entries.push(EntryKind::SubMenu("First Column".to_string(),
                vec![EntryKind::RadioButton("Title and Author".to_string(), EntryId::FirstColumn(FirstColumn::TitleAndAuthor), first_column == FirstColumn::TitleAndAuthor),
                     EntryKind::RadioButton("File Name".to_string(), EntryId::FirstColumn(FirstColumn::FileName), first_column == FirstColumn::FileName)]));

            let second_column = library_settings.second_column;
            entries.push(EntryKind::SubMenu("Second Column".to_string(),
                vec![EntryKind::RadioButton("Progress".to_string(), EntryId::SecondColumn(SecondColumn::Progress), second_column == SecondColumn::Progress),
                     EntryKind::RadioButton("Year".to_string(), EntryId::SecondColumn(SecondColumn::Year), second_column == SecondColumn::Year)]));

            entries.push(EntryKind::CheckBox("Thumbnail Previews".to_string(),
                                             EntryId::ThumbnailPreviews,
                                             library_settings.thumbnail_previews));

            let trash_path = context.library.home.join(TRASH_DIRNAME);
            if let Ok(trash) = Library::new(trash_path, LibraryMode::Database)
                                       .map_err(|e| eprintln!("Can't inspect trash: {:#?}.", e)) {
                if trash.is_empty() == Some(false) {
                    entries.push(EntryKind::Separator);
                    entries.push(EntryKind::Command("Empty Trash".to_string(),
                                                    EntryId::EmptyTrash));
                }
            }

            let library_menu = Menu::new(rect, ViewId::LibraryMenu, MenuKind::DropDown, entries, context);
            rq.add(RenderData::new(library_menu.id(), *library_menu.rect(), UpdateMode::Gui));
            self.children.push(Box::new(library_menu) as Box<dyn View>);
        }
    }

    fn add_document(&mut self, info: Info, hub: &Hub, rq: &mut RenderQueue, context: &mut Context) {
        context.library.add_document(info);
        self.sort(false, hub, rq, context);
        self.refresh_visibles(true, false, hub, rq, context);
    }

    fn update_document(&mut self, path: &Path, info: Info, hub: &Hub, rq: &mut RenderQueue, context: &mut Context) -> Result<(), Error> {
        let full_path = context.library.home.join(path);
        context.library.update(full_path, info)?;
        self.refresh_visibles(true, false, hub, rq, context);
        Ok(())
    }

    fn set_status(&mut self, path: &Path, status: SimpleStatus, hub: &Hub, rq: &mut RenderQueue, context: &mut Context) {
        context.library.set_status(path, status);

        // Is the current sort method affected by this change?
        if self.sort_method.is_status_related() {
            self.sort(false, hub, rq, context);
        }

        self.refresh_visibles(true, false, hub, rq, context);
    }

    fn empty_trash(&mut self, hub: &Hub, rq: &mut RenderQueue, context: &mut Context) {
        let trash_path = context.library.home.join(TRASH_DIRNAME);

        let trash = Library::new(trash_path, LibraryMode::Database)
                            .map_err(|e| eprintln!("Can't load trash: {:#}.", e));
        if trash.is_err() {
            return;
        }

        let mut trash = trash.unwrap();

        let (files, _) = trash.list(&trash.home, None, false);
        if files.is_empty() {
            return;
        }

        let mut count = 0;
        for info in files {
            match trash.remove(&info.file.path) {
                Err(e) => eprintln!("Can't erase {}: {:#}.", info.file.path.display(), e),
                Ok(()) => count += 1,
            }
        }
        trash.flush();
        let message = format!("Removed {} book{}.", count, if count != 1 { "s" } else { "" });
        let notif = Notification::new(message, hub, rq, context);
        self.children.push(Box::new(notif) as Box<dyn View>);
    }

    fn rename(&mut self, path: &Path, file_name: &str, hub: &Hub, rq: &mut RenderQueue, context: &mut Context) -> Result<(), Error> {
        context.library.rename(path, file_name)?;
        self.refresh_visibles(true, false, hub, rq, context);
        Ok(())
    }

    fn remove(&mut self, path: &Path, hub: &Hub, rq: &mut RenderQueue, context: &mut Context) -> Result<(), Error> {
        let full_path = context.library.home.join(path);
        if full_path.exists() {
            let trash_path = context.library.home.join(TRASH_DIRNAME);
            if !trash_path.is_dir() {
                fs::create_dir(&trash_path)?;
            }
            let mut trash = Library::new(trash_path, LibraryMode::Database)?;
            context.library.move_to(path, &mut trash)?;
            let (mut files, _) = trash.list(&trash.home, None, false);
            let mut size = files.iter().map(|info| info.file.size).sum::<u64>();
            if size > context.settings.home.max_trash_size {
                sort(&mut files, SortMethod::Added, true);
                while size > context.settings.home.max_trash_size {
                    let info = files.pop().unwrap();
                    if let Err(e) = trash.remove(&info.file.path) {
                        eprintln!("Can't erase {}: {:#}", info.file.path.display(), e);
                        break;
                    }
                    size -= info.file.size;
                }
            }
            trash.flush();
        } else {
            context.library.remove(path)?;
        }
        self.refresh_visibles(true, false, hub, rq, context);
        Ok(())
    }

    fn copy_to(&mut self, path: &Path, index: usize, context: &mut Context) -> Result<(), Error> {
        let library_settings = &context.settings.libraries[index];
        let mut library = Library::new(&library_settings.path, library_settings.mode)?;
        context.library.copy_to(path, &mut library)?;
        library.flush();
        Ok(())
    }

    fn move_to(&mut self, path: &Path, index: usize, hub: &Hub, rq: &mut RenderQueue, context: &mut Context) -> Result<(), Error> {
        let library_settings = &context.settings.libraries[index];
        let mut library = Library::new(&library_settings.path, library_settings.mode)?;
        context.library.move_to(path, &mut library)?;
        library.flush();
        self.refresh_visibles(true, false, hub, rq, context);
        Ok(())
    }

    fn set_reverse_order(&mut self, value: bool, hub: &Hub, rq: &mut RenderQueue, context: &mut Context) {
        self.reverse_order = value;
        self.current_page = 0;
        self.sort(true, hub, rq, context);
    }

    fn set_sort_method(&mut self, sort_method: SortMethod, hub: &Hub, rq: &mut RenderQueue, context: &mut Context) {
        self.sort_method = sort_method;
        self.reverse_order = sort_method.reverse_order();

        if let Some(index) = locate_by_id(self, ViewId::SortMenu) {
            self.child_mut(index)
                .children_mut().last_mut().unwrap()
                .downcast_mut::<MenuEntry>().unwrap()
                .update(sort_method.reverse_order(), rq);
        }

        self.current_page = 0;
        self.sort(true, hub, rq, context);
    }

    fn sort(&mut self, update: bool, hub: &Hub, rq: &mut RenderQueue, context: &mut Context) {
        context.library.sort(self.sort_method, self.reverse_order);
        sort(&mut self.visible_books, self.sort_method, self.reverse_order);

        if update {
            self.update_shelf(false, hub, rq, context);
            let search_visible = rlocate::<SearchBar>(self).is_some();
            self.update_top_bar(search_visible, rq);
            self.update_bottom_bar(rq, context);
        }
    }

    fn load_library(&mut self, index: usize, hub: &Hub, rq: &mut RenderQueue, context: &mut Context) {
        if index == context.settings.selected_library {
            return;
        }

        let library_settings = context.settings.libraries[index].clone();
        let library = Library::new(&library_settings.path,
                                    library_settings.mode)
                              .map_err(|e| eprintln!("Can't load library: {:#}.", e));

        if library.is_err() {
            return;
        }

        let library = library.unwrap();

        let old_path = mem::take(&mut self.current_directory);
        self.terminate_fetchers(&old_path, false, hub, context);

        let mut update_top_bar = false;

        if self.query.is_some() {
            self.toggle_search_bar(Some(false), false, hub, rq, context);
            update_top_bar = true;
        }

        context.library.flush();

        context.library = library;
        context.settings.selected_library = index;

        if self.sort_method != library_settings.sort_method {
            self.sort_method = library_settings.sort_method;
            self.reverse_order = library_settings.sort_method.reverse_order();
            update_top_bar = true;
        }

        context.library.sort(self.sort_method, self.reverse_order);

        if update_top_bar {
            let search_visible = rlocate::<SearchBar>(self).is_some();
            self.update_top_bar(search_visible, rq);
        }

        if let Some(shelf) = self.children[self.shelf_index].as_mut().downcast_mut::<Shelf>() {
            shelf.set_first_column(library_settings.first_column);
            shelf.set_second_column(library_settings.second_column);
            shelf.set_thumbnail_previews(library_settings.thumbnail_previews);
        }

        let home = context.library.home.clone();
        self.select_directory(&home, hub, rq, context);
    }

    fn import(&mut self, hub: &Hub, rq: &mut RenderQueue, context: &mut Context) {
        context.library.import(&context.settings.import);
        context.library.sort(self.sort_method, self.reverse_order);
        self.refresh_visibles(true, false, hub, rq, context);
    }

    fn clean_up(&mut self, hub: &Hub, rq: &mut RenderQueue, context: &mut Context) {
        context.library.clean_up();
        self.refresh_visibles(true, false, hub, rq, context);
    }

    fn flush(&mut self, context: &mut Context) {
        context.library.flush();
    }

    fn terminate_fetchers(&mut self, path: &Path, update: bool, hub: &Hub, context: &mut Context) {
        self.background_fetchers.retain(|id, fetcher| {
            if fetcher.full_path == path {
                unsafe { libc::kill(*id as libc::pid_t, libc::SIGTERM) };
                fetcher.process.wait().ok();
                if update {
                    if let Some(sort_method) = fetcher.sort_method {
                        hub.send(Event::Select(EntryId::Sort(sort_method))).ok();
                    }
                    if let Some(first_column) = fetcher.first_column {
                        hub.send(Event::Select(EntryId::FirstColumn(first_column))).ok();
                    }
                    if let Some(second_column) = fetcher.second_column {
                        hub.send(Event::Select(EntryId::SecondColumn(second_column))).ok();
                    }
                } else {
                    let selected_library = context.settings.selected_library;
                    if let Some(sort_method) = fetcher.sort_method {
                        context.settings.libraries[selected_library].sort_method = sort_method;
                    }
                    if let Some(first_column) = fetcher.first_column {
                        context.settings.libraries[selected_library].first_column = first_column;
                    }
                    if let Some(second_column) = fetcher.second_column {
                        context.settings.libraries[selected_library].second_column = second_column;
                    }
                }
                false
            } else {
                true
            }
        });
    }

    fn insert_fetcher(&mut self, hook: &Hook, hub: &Hub, context: &Context) {
        let library_path = &context.library.home;
        let save_path = context.library.home.join(&hook.path);
        match self.spawn_child(library_path, &save_path, &hook.program, context.settings.wifi, context.online, hub) {
            Ok(process) => {
                let mut sort_method = hook.sort_method;
                let mut first_column = hook.first_column;
                let mut second_column = hook.second_column;
                if let Some(sort_method) = sort_method.replace(self.sort_method) {
                    hub.send(Event::Select(EntryId::Sort(sort_method))).ok();
                }
                let selected_library = context.settings.selected_library;
                if let Some(first_column) = first_column.replace(context.settings.libraries[selected_library].first_column) {
                    hub.send(Event::Select(EntryId::FirstColumn(first_column))).ok();
                }
                if let Some(second_column) = second_column.replace(context.settings.libraries[selected_library].second_column) {
                    hub.send(Event::Select(EntryId::SecondColumn(second_column))).ok();
                }
                self.background_fetchers.insert(process.id(),
                                                Fetcher { path: hook.path.clone(), full_path: save_path, process,
                                                          sort_method, first_column, second_column });
            },
            Err(e) => eprintln!("Can't spawn child: {:#}.", e),
        }
    }

    fn spawn_child(&mut self, library_path: &Path, save_path: &Path, program: &Path, wifi: bool, online: bool, hub: &Hub) -> Result<Child, Error> {
        let path = program.canonicalize()?;
        let parent = path.parent()
                         .unwrap_or_else(|| Path::new(""));
        let mut process = Command::new(&path)
                                 .current_dir(parent)
                                 .arg(library_path)
                                 .arg(save_path)
                                 .arg(wifi.to_string())
                                 .arg(online.to_string())
                                 .stdin(Stdio::piped())
                                 .stdout(Stdio::piped())
                                 .spawn()?;
        let stdout = process.stdout.take()
                            .ok_or_else(|| format_err!("can't take stdout"))?;
        let id = process.id();
        let hub2 = hub.clone();
        thread::spawn(move || {
            let reader = BufReader::new(stdout);
            for line_res in reader.lines() {
                if let Ok(line) = line_res {
                    if let Ok(event) = serde_json::from_str::<JsonValue>(&line) {
                        match event.get("type")
                                   .and_then(JsonValue::as_str) {
                            Some("notify") => {
                                if let Some(msg) = event.get("message")
                                                        .and_then(JsonValue::as_str) {
                                    hub2.send(Event::Notify(msg.to_string())).ok();
                                }
                            },
                            Some("setWifi") => {
                                if let Some(enable) = event.get("enable")
                                                           .and_then(JsonValue::as_bool) {
                                    hub2.send(Event::SetWifi(enable)).ok();
                                }
                            },
                            Some("addDocument") => {
                                if let Some(info) = event.get("info")
                                                         .map(ToString::to_string)
                                                         .and_then(|v| serde_json::from_str(&v).ok()) {
                                    hub2.send(Event::FetcherAddDocument(id, Box::new(info))).ok();
                                }
                            },
                            Some("removeDocument") => {
                                if let Some(path) = event.get("path")
                                                         .and_then(JsonValue::as_str) {
                                    hub2.send(Event::FetcherRemoveDocument(id, PathBuf::from(path))).ok();
                                }
                            },
                            Some("updateDocument") => {
                                if let Some(info) = event.get("info")
                                                         .map(ToString::to_string)
                                                         .and_then(|v| serde_json::from_str(&v).ok()) {
                                    if let Some(path) = event.get("path")
                                                             .and_then(JsonValue::as_str) {
                                        hub2.send(Event::FetcherUpdateDocument(id, PathBuf::from(path), Box::new(info))).ok();
                                    }
                                }
                            },
                            Some("search") => {
                                let path = event.get("path")
                                                .and_then(JsonValue::as_str)
                                                .map(PathBuf::from);
                                let query = event.get("query")
                                                 .and_then(JsonValue::as_str)
                                                 .map(String::from);
                                let sort_by = event.get("sortBy")
                                                   .map(ToString::to_string)
                                                   .and_then(|v| serde_json::from_str(&v).ok());
                                hub2.send(Event::FetcherSearch { id, path, query, sort_by }).ok();
                            },
                            _ => (),
                        }
                    }
                } else {
                    break;
                }
            }
            hub2.send(Event::CheckFetcher(id)).ok();
        });
        Ok(process)
    }

    fn reseed(&mut self, hub: &Hub, rq: &mut RenderQueue, context: &mut Context) {
        context.library.sort(self.sort_method, self.reverse_order);
        self.refresh_visibles(true, false, hub, &mut RenderQueue::new(), context);

        if let Some(top_bar) = self.child_mut(0).downcast_mut::<TopBar>() {
            top_bar.reseed(&mut RenderQueue::new(), context);
        }

        rq.add(RenderData::new(self.id, self.rect, UpdateMode::Gui));
    }
}

impl View for Home {
    fn handle_event(&mut self, evt: &Event, hub: &Hub, _bus: &mut Bus, rq: &mut RenderQueue, context: &mut Context) -> bool {
        match *evt {
            Event::Gesture(GestureEvent::Swipe { dir, start, end, .. }) => {
                match dir {
                    Dir::South if self.children[0].rect().includes(start) &&
                                  self.children[self.shelf_index].rect().includes(end) => {
                        if !context.settings.home.navigation_bar {
                            self.toggle_navigation_bar(Some(true), true, hub, rq, context);
                        } else if !context.settings.home.address_bar {
                            self.toggle_address_bar(Some(true), true, hub, rq, context);
                        }
                    },
                    Dir::North if self.children[self.shelf_index].rect().includes(start) &&
                                  self.children[0].rect().includes(end) => {
                        if context.settings.home.address_bar {
                            self.toggle_address_bar(Some(false), true, hub, rq, context);
                        } else if context.settings.home.navigation_bar {
                            self.toggle_navigation_bar(Some(false), true, hub, rq, context);
                        }
                    },
                    _ => (),
                }
                true
            },
            Event::Gesture(GestureEvent::Rotate { quarter_turns, .. }) if quarter_turns != 0 => {
                let (_, dir) = CURRENT_DEVICE.mirroring_scheme();
                let n = (4 + (context.display.rotation - dir * quarter_turns)) % 4;
                hub.send(Event::Select(EntryId::Rotate(n))).ok();
                true
            },
            Event::Gesture(GestureEvent::Arrow { dir, .. }) => {
                match dir {
                    Dir::West => self.go_to_page(0, hub, rq, context),
                    Dir::East => {
                        let pages_count = self.pages_count;
                        self.go_to_page(pages_count.saturating_sub(1), hub, rq, context);
                    },
                    Dir::North => {
                        let path = context.library.home.clone();
                        self.select_directory(&path, hub, rq, context);
                    },
                    Dir::South => self.toggle_search_bar(None, true, hub, rq, context),
                };
                true
            },
            Event::Gesture(GestureEvent::Corner { dir, .. }) => {
                match dir {
                    DiagDir::NorthWest |
                    DiagDir::SouthWest => self.go_to_status_change(CycleDir::Previous, hub, rq, context),
                    DiagDir::NorthEast |
                    DiagDir::SouthEast => self.go_to_status_change(CycleDir::Next, hub, rq, context),
                };
                true
            },
            Event::Focus(v) => {
                if self.focus != v {
                    self.focus = v;
                    if v.is_some() {
                        self.toggle_keyboard(true, true, v, hub, rq, context);
                    }
                }
                true
            },
            Event::Show(ViewId::Keyboard) => {
                self.toggle_keyboard(true, true, None, hub, rq, context);
                true
            },
            Event::Toggle(ViewId::GoToPage) => {
                self.toggle_go_to_page(None, hub, rq, context);
                true
            },
            Event::Toggle(ViewId::SearchBar) => {
                self.toggle_search_bar(None, true, hub, rq, context);
                true
            },
            Event::ToggleNear(ViewId::TitleMenu, rect) => {
                self.toggle_sort_menu(rect, None, rq, context);
                true
            },
            Event::ToggleBookMenu(rect, index) => {
                self.toggle_book_menu(index, rect, None, rq, context);
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
            Event::ToggleNear(ViewId::LibraryMenu, rect) => {
                self.toggle_library_menu(rect, None, rq, context);
                true
            },
            Event::Close(ViewId::AddressBar) => {
                self.toggle_address_bar(Some(false), true, hub, rq, context);
                true
            },
            Event::Close(ViewId::SearchBar) => {
                self.toggle_search_bar(Some(false), true, hub, rq, context);
                true
            },
            Event::Close(ViewId::SortMenu) => {
                self.toggle_sort_menu(Rectangle::default(), Some(false), rq, context);
                true
            },
            Event::Close(ViewId::LibraryMenu) => {
                self.toggle_library_menu(Rectangle::default(), Some(false), rq, context);
                true
            },
            Event::Close(ViewId::MainMenu) => {
                toggle_main_menu(self, Rectangle::default(), Some(false), rq, context);
                true
            },
            Event::Close(ViewId::GoToPage) => {
                self.toggle_go_to_page(Some(false), hub, rq, context);
                true
            },
            Event::Close(ViewId::RenameDocument) => {
                self.toggle_rename_document(Some(false), hub, rq, context);
                true
            },
            Event::Select(EntryId::Sort(sort_method)) => {
                let selected_library = context.settings.selected_library;
                context.settings.libraries[selected_library].sort_method = sort_method;
                self.set_sort_method(sort_method, hub, rq, context);
                true
            },
            Event::Select(EntryId::ReverseOrder) => {
                let next_value = !self.reverse_order;
                self.set_reverse_order(next_value, hub, rq, context);
                true
            },
            Event::Select(EntryId::LoadLibrary(index)) => {
                self.load_library(index, hub, rq, context);
                true
            },
            Event::Select(EntryId::Import) => {
                self.import(hub, rq, context);
                true
            },
            Event::Select(EntryId::CleanUp) => {
                self.clean_up(hub, rq, context);
                true
            },
            Event::Select(EntryId::Flush) => {
                self.flush(context);
                true
            },
            Event::FetcherAddDocument(_, ref info) => {
                self.add_document(*info.clone(), hub, rq, context);
                true
            },
            Event::Select(EntryId::SetStatus(ref path, status)) => {
                self.set_status(path, status, hub, rq, context);
                true
            },
            Event::Select(EntryId::FirstColumn(first_column)) => {
                let selected_library = context.settings.selected_library;
                context.settings.libraries[selected_library].first_column = first_column;
                self.update_first_column(hub, rq, context);
                true
            },
            Event::Select(EntryId::SecondColumn(second_column)) => {
                let selected_library = context.settings.selected_library;
                context.settings.libraries[selected_library].second_column = second_column;
                self.update_second_column(hub, rq, context);
                true
            },
            Event::Select(EntryId::ThumbnailPreviews) => {
                let selected_library = context.settings.selected_library;
                context.settings.libraries[selected_library].thumbnail_previews = !context.settings.libraries[selected_library].thumbnail_previews;
                self.update_thumbnail_previews(hub, rq, context);
                true
            },
            Event::Submit(ViewId::AddressBarInput, ref addr) => {
                self.toggle_keyboard(false, true, None, hub, rq, context);
                self.select_directory(Path::new(addr), hub, rq, context);
                true
            },
            Event::Submit(ViewId::HomeSearchInput, ref text) => {
                self.query = BookQuery::new(text);
                if self.query.is_some() {
                    self.toggle_keyboard(false, false, None, hub, rq, context);
                    // Render the search bar and its separator.
                    for i in self.shelf_index + 1 ..= self.shelf_index + 2 {
                        rq.add(RenderData::new(self.child(i).id(), *self.child(i).rect(), UpdateMode::Gui));
                    }
                    self.refresh_visibles(true, true, hub, rq, context);
                } else {
                    let notif = Notification::new("Invalid search query.".to_string(),
                                                  hub, rq, context);
                    self.children.push(Box::new(notif) as Box<dyn View>);
                }
                true
            },
            Event::Submit(ViewId::GoToPageInput, ref text) => {
                if text == "(" {
                    self.go_to_page(0, hub, rq, context);
                } else if text == ")" {
                    self.go_to_page(self.pages_count.saturating_sub(1), hub, rq, context);
                } else if text == "_" {
                    let index = (context.rng.next_u64() % self.pages_count as u64) as usize;
                    self.go_to_page(index, hub, rq, context);
                } else if let Ok(index) = text.parse::<usize>() {
                    self.go_to_page(index.saturating_sub(1), hub, rq, context);
                }
                true
            },
            Event::Submit(ViewId::RenameDocumentInput, ref file_name) => {
                if let Some(ref path) = self.target_document.take() {
                    self.rename(path, file_name, hub, rq, context)
                        .map_err(|e| eprintln!("Can't rename document: {:#}.", e))
                        .ok();
                }
                true
            },
            Event::NavigationBarResized(_) => {
                self.adjust_shelf_top_edge();
                self.update_shelf(true, hub, rq, context);
                self.update_bottom_bar(rq, context);
                for i in self.shelf_index - 2..=self.shelf_index - 1 {
                    rq.add(RenderData::new(self.child(i).id(), *self.child(i).rect(), UpdateMode::Gui));
                }
                true
            },
            Event::Select(EntryId::EmptyTrash) => {
                self.empty_trash(hub, rq, context);
                true
            },
            Event::Select(EntryId::Rename(ref path)) => {
                self.target_document = Some(path.clone());
                self.toggle_rename_document(Some(true), hub, rq, context);
                true
            },
            Event::Select(EntryId::Remove(ref path)) | Event::FetcherRemoveDocument(_, ref path) => {
                self.remove(path, hub, rq, context)
                    .map_err(|e| eprintln!("Can't remove document: {:#}.", e))
                    .ok();
                true
            },
            Event::FetcherUpdateDocument(_, ref path, ref info) => {
                self.update_document(path, *info.clone(), hub, rq, context)
                    .map_err(|e| eprintln!("Can't remove document: {:#}.", e))
                    .ok();
                true
            },
            Event::Select(EntryId::CopyTo(ref path, index)) => {
                self.copy_to(path, index, context)
                    .map_err(|e| eprintln!("Can't copy document: {:#}.", e))
                    .ok();
                true
            },
            Event::Select(EntryId::MoveTo(ref path, index)) => {
                self.move_to(path, index, hub, rq, context)
                    .map_err(|e| eprintln!("Can't move document: {:#}.", e))
                    .ok();
                true
            },
            Event::Select(EntryId::ToggleShowHidden) => {
                context.library.show_hidden = !context.library.show_hidden;
                self.refresh_visibles(true, false, hub, rq, context);
                true
            },
            Event::SelectDirectory(ref path) |
            Event::Select(EntryId::SelectDirectory(ref path)) => {
                self.select_directory(path, hub, rq, context);
                true
            },
            Event::ToggleSelectDirectory(ref path) |
            Event::Select(EntryId::ToggleSelectDirectory(ref path)) => {
                self.toggle_select_directory(path, hub, rq, context);
                true
            },
            Event::Select(EntryId::SearchAuthor(ref author)) => {
                let text = format!("'a {}", author);
                let query = BookQuery::new(&text);
                if query.is_some() {
                    self.query = query;
                    self.toggle_search_bar(Some(true), false, hub, rq, context);
                    self.toggle_keyboard(false, false, None, hub, rq, context);
                    if let Some(search_bar) = self.children[self.shelf_index+2].downcast_mut::<SearchBar>() {
                        search_bar.set_text(&text, rq, context);
                    }
                    // Render the search bar and its separator.
                    for i in self.shelf_index + 1 ..= self.shelf_index + 2 {
                        rq.add(RenderData::new(self.child(i).id(), *self.child(i).rect(), UpdateMode::Gui));
                    }
                    self.refresh_visibles(true, true, hub, rq, context);
                }
                true
            },
            Event::GoTo(location) => {
                self.go_to_page(location as usize, hub, rq, context);
                true
            },
            Event::Chapter(dir) => {
                let pages_count = self.pages_count;
                match dir {
                    CycleDir::Previous => self.go_to_page(0, hub, rq, context),
                    CycleDir::Next => self.go_to_page(pages_count.saturating_sub(1), hub, rq, context),
                }
                true
            },
            Event::Page(dir) => {
                self.go_to_neighbor(dir, hub, rq, context);
                true
            },
            Event::Device(DeviceEvent::Button { code: ButtonCode::Backward, status: ButtonStatus::Pressed, .. }) => {
                self.go_to_neighbor(CycleDir::Previous, hub, rq, context);
                true
            },
            Event::Device(DeviceEvent::Button { code: ButtonCode::Forward, status: ButtonStatus::Pressed, .. }) => {
                self.go_to_neighbor(CycleDir::Next, hub, rq, context);
                true
            },
            Event::Device(DeviceEvent::NetUp) => {
                for fetcher in self.background_fetchers.values_mut() {
                    if let Some(stdin) = fetcher.process.stdin.as_mut() {
                        writeln!(stdin, "{}", json!({"type": "network", "status": "up"})).ok();
                    }
                }
                true
            },
            Event::FetcherSearch { id, ref path, ref query, ref sort_by } => {
                let path = path.as_ref().unwrap_or(&context.library.home);
                let query = query.as_ref().and_then(|text| BookQuery::new(text));
                let (mut files, _) = context.library.list(path, query.as_ref(), false);
                if let Some((sort_method, reverse_order)) = *sort_by {
                    sort(&mut files, sort_method, reverse_order);
                }
                for entry in &mut files {
                    // Let the *reader* field pass through.
                    mem::swap(&mut entry.reader, &mut entry._reader);
                }
                if let Some(fetcher) = self.background_fetchers.get_mut(&id) {
                    if let Some(stdin) = fetcher.process.stdin.as_mut() {
                        writeln!(stdin, "{}", json!({"type": "search",
                                                     "results": files})).ok();
                    }
                }
                true
            },
            Event::CheckFetcher(id) => {
                if let Some(fetcher) = self.background_fetchers.get_mut(&id) {
                    if let Ok(exit_status) = fetcher.process.wait() {
                        if !exit_status.success() {
                            let msg = format!("{}: abnormal process termination.", fetcher.path.display());
                            let notif = Notification::new(msg, hub, rq, context);
                            self.children.push(Box::new(notif) as Box<dyn View>);
                        }
                    }
                }
                true
            },
            Event::ToggleFrontlight => {
                if let Some(index) = locate::<TopBar>(self) {
                    self.child_mut(index).downcast_mut::<TopBar>().unwrap()
                        .update_frontlight_icon(rq, context);
                }
                true
            },
            Event::Reseed => {
                self.reseed(hub, rq, context);
                true
            },
            _ => false,
        }
    }

    fn render(&self, _fb: &mut dyn Framebuffer, _rect: Rectangle, _fonts: &mut Fonts) {
    }

    fn resize(&mut self, rect: Rectangle, hub: &Hub, rq: &mut RenderQueue, context: &mut Context) {
        let dpi = CURRENT_DEVICE.dpi;
        let thickness = scale_by_dpi(THICKNESS_MEDIUM, dpi) as i32;
        let (small_thickness, big_thickness) = halves(thickness);
        let (small_height, big_height) = (scale_by_dpi(SMALL_BAR_HEIGHT, dpi) as i32,
                                          scale_by_dpi(BIG_BAR_HEIGHT, dpi) as i32);

        self.children.retain(|child| !child.is::<Menu>());

        // Top bar.
        let top_bar_rect = rect![rect.min.x, rect.min.y,
                                 rect.max.x, rect.min.y + small_height - small_thickness];
        self.children[0].resize(top_bar_rect, hub, rq, context);

        let separator_rect = rect![rect.min.x, rect.min.y + small_height - small_thickness,
                                   rect.max.x, rect.min.y + small_height + big_thickness];
        self.children[1].resize(separator_rect, hub, rq, context);

        let mut shelf_min_y = rect.min.y + small_height + big_thickness;
        let mut index = 2;

        // Address bar.
        if context.settings.home.address_bar {
            self.children[index].resize(rect![rect.min.x, shelf_min_y,
                                              rect.max.x, shelf_min_y + small_height - thickness],
                                        hub, rq, context);
            shelf_min_y += small_height - thickness;
            index += 1;

            self.children[index].resize(rect![rect.min.x, shelf_min_y,
                                              rect.max.x, shelf_min_y + thickness],
                                        hub, rq, context);
            shelf_min_y += thickness;
            index += 1;
        }

        // Navigation bar.
        if context.settings.home.navigation_bar {
            let count = if self.children[self.shelf_index+2].is::<SearchBar>() { 2 } else { 1 };
            let nav_bar = self.children[index].as_mut().downcast_mut::<NavigationBar>().unwrap();
            let (_, dirs) = context.library.list(&self.current_directory, None, true);
            nav_bar.clear();
            nav_bar.resize(rect![rect.min.x, shelf_min_y,
                                 rect.max.x, shelf_min_y + small_height - thickness],
                           hub, rq, context);
            nav_bar.vertical_limit = rect.max.y - count * small_height - big_height - small_thickness;
            nav_bar.set_path(&self.current_directory, &dirs, &mut RenderQueue::new(), context);
            shelf_min_y += nav_bar.rect().height() as i32;
            index += 1;

            self.children[index].resize(rect![rect.min.x, shelf_min_y,
                                              rect.max.x, shelf_min_y + thickness],
                                        hub, rq, context);
            shelf_min_y += thickness;
        }

        // Bottom bar.
        let bottom_bar_index = rlocate::<BottomBar>(self).unwrap();
        index = bottom_bar_index;

        let separator_rect = rect![rect.min.x, rect.max.y - small_height - small_thickness,
                                   rect.max.x, rect.max.y - small_height + big_thickness];
        self.children[index-1].resize(separator_rect, hub, rq, context);

        let bottom_bar_rect = rect![rect.min.x, rect.max.y - small_height + big_thickness,
                                    rect.max.x, rect.max.y];
        self.children[index].resize(bottom_bar_rect, hub, rq, context);

        let mut shelf_max_y = rect.max.y - small_height - small_thickness;

        if index - self.shelf_index > 2 {
            index -= 2;
            // Keyboard.
            if self.children[index].is::<Keyboard>() {
                let kb_rect = rect![rect.min.x,
                                    rect.max.y - (small_height + 3 * big_height) as i32 + big_thickness,
                                    rect.max.x,
                                    rect.max.y - small_height - small_thickness];
                self.children[index].resize(kb_rect, hub, rq, context);
                let s_max_y = self.children[index].rect().min.y;
                self.children[index-1].resize(rect![rect.min.x, s_max_y - thickness,
                                                    rect.max.x, s_max_y],
                                              hub, rq, context);
                index -= 2;
            }
            // Search bar.
            if self.children[index].is::<SearchBar>() {
                let sp_rect = *self.children[index+1].rect() - pt!(0, small_height);
                self.children[index].resize(rect![rect.min.x,
                                                  sp_rect.max.y,
                                                  rect.max.x,
                                                  sp_rect.max.y + small_height - thickness],
                                            hub, rq, context);
                self.children[index-1].resize(sp_rect, hub, rq, context);
                shelf_max_y -= small_height;
            }
        }

        // Shelf.
        let shelf_rect = rect![rect.min.x, shelf_min_y,
                               rect.max.x, shelf_max_y];
        self.children[self.shelf_index].resize(shelf_rect, hub, rq, context);

        self.update_shelf(true, hub, &mut RenderQueue::new(), context);
        self.update_bottom_bar(&mut RenderQueue::new(), context);

        // Floating windows.
        for i in bottom_bar_index+1..self.children.len() {
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
