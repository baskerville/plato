mod bottom_bar;
mod feed_browser;
mod feed_entry;
mod navigation_bar;

use std::thread;

use anyhow::Error;
use navigation_bar::NavigationBar;
use regex::Regex;

use self::bottom_bar::BottomBar;
use self::feed_browser::FeedBrowser;

use lazy_static::lazy_static;

use crate::color::BLACK;
use crate::context::Context;
use crate::device::CURRENT_DEVICE;
use crate::font::Fonts;
use crate::framebuffer::{Framebuffer, UpdateMode};
use crate::geom::{halves, CycleDir, Rectangle};
use crate::input::{ButtonCode, ButtonStatus, DeviceEvent};
use crate::opds::Feed;
use crate::opds::{MimeType, OpdsFetcher};
use crate::unit::scale_by_dpi;
use crate::view::common::{toggle_battery_menu, toggle_clock_menu, toggle_main_menu};
use crate::view::filler::Filler;
use crate::view::top_bar::TopBar;
use crate::view::{Bus, Event, Hub, RenderData, RenderQueue, View};
use crate::view::{Id, ViewId, ID_FEEDER};
use crate::view::{BIG_BAR_HEIGHT, SMALL_BAR_HEIGHT, THICKNESS_MEDIUM};

use super::common::locate_by_id;
use super::common::rlocate;
use super::menu::Menu;
use super::menu::MenuKind;
use super::EntryId;
use super::EntryKind;

lazy_static! {
    static ref filename_regex: Regex =
        Regex::new("[<>:\"/\\\\|?*\u{0000}-\u{001F}\u{007F}\u{0080}-\u{009F}]+").unwrap();
}

pub struct Opds {
    id: Id,
    rect: Rectangle,
    children: Vec<Box<dyn View>>,
    current_page: usize,
    pages_count: usize,
    feed: Feed,
    fetcher: Option<OpdsFetcher>,
    selected_server: usize,
}

impl Opds {
    pub fn new(rect: Rectangle, hub: &Hub, rq: &mut RenderQueue, context: &mut Context) -> Opds {
        let id = ID_FEEDER.next();
        let mut children = Vec::new();
        let dpi = CURRENT_DEVICE.dpi;
        let small_height = scale_by_dpi(SMALL_BAR_HEIGHT, dpi) as i32;
        let thickness = scale_by_dpi(THICKNESS_MEDIUM, dpi) as i32;
        let (small_thickness, big_thickness) = halves(thickness);

        //TODO turn wifi on?
        if !context.settings.wifi {
            hub.send(Event::Notify("Wifi is disabled".to_owned())).ok();
        }

        let context_settings = context.settings.opds.get(0);
        let mut fetcher = None;
        let mut feed = Feed::default();
        let mut server_name = "Unnamed".to_string();
        //Handle loading even if opds stuff isnt there
        if let Some(settings) = context_settings {
            server_name = settings.name.clone();
            match OpdsFetcher::new(settings.clone()) {
                Ok(new_fetcher) => {
                    match new_fetcher.home() {
                        Ok(new_feed) => feed = new_feed,
                        Err(err) => {
                            hub.send(Event::Notify("Failed to pull root feed".to_owned()))
                                .ok();
                            println!("Failed to pull root feed: {:?}", err)
                        }
                    }
                    fetcher = Some(new_fetcher);
                }
                Err(err) => {
                    hub.send(Event::Notify("Failed to create fetcher".to_owned()))
                        .ok();
                    println!("Failed to create fetcher: {:?}", err)
                }
            }
        }

        let top_bar = TopBar::new(
            rect![
                rect.min.x,
                rect.min.y,
                rect.max.x,
                rect.min.y + small_height - small_thickness
            ],
            Event::Back,
            "OPDS".to_string(),
            context,
        );
        children.push(Box::new(top_bar) as Box<dyn View>);

        let separator = Filler::new(
            rect![
                rect.min.x,
                rect.min.y + small_height - small_thickness,
                rect.max.x,
                rect.min.y + small_height + big_thickness
            ],
            BLACK,
        );
        children.push(Box::new(separator) as Box<dyn View>);

        let navigation = NavigationBar::new(
            rect![
                rect.min.x,
                rect.min.y + small_height + big_thickness,
                rect.max.x,
                rect.min.y + 2 * small_height - small_thickness
            ],
            &feed.title,
        );
        children.push(Box::new(navigation) as Box<dyn View>);

        let separator = Filler::new(
            rect![
                rect.min.x,
                rect.min.y + 2 * small_height - small_thickness,
                rect.max.x,
                rect.min.y + 2 * small_height + big_thickness
            ],
            BLACK,
        );
        children.push(Box::new(separator) as Box<dyn View>);

        let mut browser = FeedBrowser::new(rect![
            rect.min.x,
            rect.min.y + 2 * small_height + big_thickness,
            rect.max.x,
            rect.max.y - small_height - small_thickness
        ]);

        let current_page = 0;
        let entries = &feed.entries;
        let entries_count = entries.len();
        let max_lines = browser.max_lines;
        let pages_count = (entries_count as f32 / max_lines as f32).ceil() as usize;
        let index_lower = current_page * max_lines;
        let index_upper = (index_lower + max_lines).min(entries_count);

        browser.update(
            &entries[index_lower..index_upper],
            hub,
            &mut RenderQueue::new(),
            context,
        );

        children.push(Box::new(browser) as Box<dyn View>);

        let separator = Filler::new(
            rect![
                rect.min.x,
                rect.max.y - small_height - small_thickness,
                rect.max.x,
                rect.max.y - small_height + big_thickness
            ],
            BLACK,
        );
        children.push(Box::new(separator) as Box<dyn View>);

        let bottom_bar = BottomBar::new(
            rect![
                rect.min.x,
                rect.max.y - small_height + big_thickness,
                rect.max.x,
                rect.max.y
            ],
            current_page,
            pages_count,
            &server_name,
        );
        children.push(Box::new(bottom_bar) as Box<dyn View>);

        rq.add(RenderData::new(id, rect, UpdateMode::Gui));

        Opds {
            id,
            rect,
            children,
            current_page: current_page,
            pages_count: pages_count,
            feed: feed,
            fetcher: fetcher,
            selected_server: 0,
        }
    }

    fn reseed(&mut self, rq: &mut RenderQueue, context: &mut Context) {
        if let Some(top_bar) = self.child_mut(0).downcast_mut::<TopBar>() {
            top_bar.reseed(rq, context);
        }

        rq.add(RenderData::new(self.id, self.rect, UpdateMode::Gui));
    }

    fn go_to_neighbor(
        &mut self,
        dir: CycleDir,
        hub: &Hub,
        rq: &mut RenderQueue,
        context: &mut Context,
    ) {
        match dir {
            CycleDir::Next if self.current_page < self.pages_count.saturating_sub(1) => {
                self.current_page += 1;
            }
            CycleDir::Previous if self.current_page > 0 => {
                self.current_page -= 1;
            }
            _ => return,
        }

        self.update_view(hub, rq, context);
    }

    fn update_feed(
        &mut self,
        feed: Result<Feed, Error>,
        hub: &Hub,
        rq: &mut RenderQueue,
        context: &mut Context,
    ) {
        if let Ok(feed) = feed {
            self.feed = feed;
            self.update_view(hub, rq, context);
        } else {
            hub.send(Event::Notify("Failed to pull feed".to_owned()))
                .ok();
            println!("Failed to pull feed: {:?}", feed.err())
        }
    }

    fn update_view(&mut self, hub: &Hub, rq: &mut RenderQueue, context: &mut Context) {
        self.update_navigation_bar(hub, rq, context);
        self.update_browser(hub, rq, context);
        self.update_bottom_bar(hub, rq, context);
    }

    fn update_navigation_bar(&mut self, _hub: &Hub, rq: &mut RenderQueue, _context: &Context) {
        let navigation_bar = self.children[2]
            .as_mut()
            .downcast_mut::<NavigationBar>()
            .unwrap();

        navigation_bar.set_feed_name(&self.feed.title, rq);
    }

    fn update_browser(&mut self, hub: &Hub, rq: &mut RenderQueue, context: &mut Context) {
        let dpi = CURRENT_DEVICE.dpi;
        let big_height = scale_by_dpi(BIG_BAR_HEIGHT, dpi) as i32;
        let thickness = scale_by_dpi(THICKNESS_MEDIUM, dpi) as i32;
        let browser = self.children[4]
            .as_mut()
            .downcast_mut::<FeedBrowser>()
            .unwrap();
        let max_lines = ((browser.rect.height() as i32 + thickness) / big_height) as usize;

        let page_position = if self.feed.entries.is_empty() {
            0.0
        } else {
            self.current_page as f32 * (browser.max_lines as f32 / self.feed.entries.len() as f32)
        };

        let mut page_guess = page_position * self.feed.entries.len() as f32 / max_lines as f32;
        let page_ceil = page_guess.ceil();

        if (page_ceil - page_guess).abs() < f32::EPSILON {
            page_guess = page_ceil;
        }

        self.pages_count = (self.feed.entries.len() as f32 / max_lines as f32).ceil() as usize;
        self.current_page = (page_guess as usize).min(self.pages_count.saturating_sub(1));

        let index_lower = self.current_page * max_lines;
        let index_upper = (index_lower + max_lines).min(self.feed.entries.len());

        browser.update(
            &&self.feed.entries[index_lower..index_upper],
            hub,
            rq,
            context,
        );
    }

    fn update_bottom_bar(&mut self, _hub: &Hub, rq: &mut RenderQueue, _context: &Context) {
        if let Some(fetcher) = &self.fetcher {
            if let Some(index) = rlocate::<BottomBar>(self) {
                let bottom_bar = self.children[index]
                    .as_mut()
                    .downcast_mut::<BottomBar>()
                    .unwrap();
                bottom_bar.update_server_label(&fetcher.settings.name, rq);
                bottom_bar.update_page_label(self.current_page, self.pages_count, rq);
                bottom_bar.update_icons(self.current_page, self.pages_count, rq);
            }
        }
    }

    fn toggle_server_menu(
        &mut self,
        rect: Rectangle,
        enable: Option<bool>,
        rq: &mut RenderQueue,
        context: &mut Context,
    ) {
        if let Some(index) = locate_by_id(self, ViewId::OpdsServerMenu) {
            if let Some(true) = enable {
                return;
            }

            rq.add(RenderData::expose(
                *self.child(index).rect(),
                UpdateMode::Gui,
            ));
            self.children.remove(index);
        } else {
            if let Some(false) = enable {
                return;
            }

            let servers: Vec<EntryKind> = context
                .settings
                .opds
                .iter()
                .enumerate()
                .map(|(index, server)| {
                    EntryKind::RadioButton(
                        server.name.clone(),
                        EntryId::OpdsServer(index),
                        index == self.selected_server,
                    )
                })
                .collect();

            let server_menu = Menu::new(
                rect,
                ViewId::OpdsServerMenu,
                MenuKind::DropDown,
                servers,
                context,
            );
            rq.add(RenderData::new(
                server_menu.id(),
                *server_menu.rect(),
                UpdateMode::Gui,
            ));
            self.children.push(Box::new(server_menu) as Box<dyn View>);
        }
    }

    fn home(&mut self, hub: &Hub, rq: &mut RenderQueue, context: &mut Context) {
        if let Some(fetcher) = &self.fetcher {
            let feed = fetcher.home();
            self.update_feed(feed, hub, rq, context);
        }
    }

    fn load_server(
        &mut self,
        index: usize,
        hub: &Hub,
        rq: &mut RenderQueue,
        context: &mut Context,
    ) {
        let opds_settings = &context.settings.opds[index];
        match OpdsFetcher::new(opds_settings.clone()) {
            Ok(fetcher) => {
                self.selected_server = index;
                let feed = fetcher.home();
                self.fetcher = Some(fetcher);
                self.update_feed(feed, hub, rq, context);
            }
            Err(err) => {
                hub.send(Event::Notify("Failed to switch server".to_owned()))
                    .ok();
                println!("Failed to switch server: {:?}", err)
            }
        }
    }

    fn select_entry(
        &mut self,
        entry_id: &str,
        hub: &Hub,
        rq: &mut RenderQueue,
        context: &mut Context,
    ) {
        if let Some(fetcher) = &self.fetcher {
            if let Some(entry) = self.feed.entries.iter().find(|x| x.id == *entry_id) {
                //TODO check for abs urls?
                let download_link = entry.links.iter().find(|&x| {
                    (x.mime_type == Some(MimeType::Epub)
                        || x.mime_type == Some(MimeType::Cbz)
                        || x.mime_type == Some(MimeType::Pdf))
                        && x.href.is_some()
                        && x.mime_type.is_some()
                });
                if let Some(link) = download_link.clone() {
                    let hub2 = hub.clone();
                    let fetcher2 = fetcher.clone();
                    let title = entry.title.clone();
                    let href = link.href.clone().unwrap();
                    let mime_type = link.mime_type.clone().unwrap();

                    let clean_title = filename_regex.replace_all(&entry.title, "").into_owned();
                    let save_path = fetcher2.settings.download_location.clone();
                    let file_name = format!("{}.{}", clean_title, mime_type.to_string());
                    let file_path = save_path.join(file_name.clone());
                    //TODO make dirs
                    //TODO fix multiple downloads concurrently
                    thread::spawn(move || {
                        hub2.send(Event::Notify(format!("Downloading: {}", title)))
                            .ok();

                        match fetcher2.download_relative(&href, &file_path) {
                            Ok(_) => {
                                hub2.send(Event::Notify(format!("Downloaded: {}", title)))
                                    .ok();
                                hub2.send(Event::OpdsDocumentDownloaded).ok();
                            }
                            Err(err) => {
                                hub2.send(Event::Notify("Failed download file".to_owned()))
                                    .ok();
                                println!("Failed download file: {:?}", err)
                            }
                        }
                    });
                    return;
                }
                let navigation_link = entry.links.iter().find(|&x| {
                    (x.mime_type == Some(MimeType::OpdsCatalog)
                        || x.mime_type == Some(MimeType::OpdsEntry))
                        && x.href.is_some()
                });
                if let Some(link) = navigation_link {
                    let href = link.href.as_ref().unwrap();
                    let feed = fetcher.pull_relative(&href);
                    self.update_feed(feed, hub, rq, context);
                    return;
                }
            }
        }
    }
}

impl View for Opds {
    fn handle_event(
        &mut self,
        evt: &Event,
        hub: &Hub,
        _bus: &mut Bus,
        rq: &mut RenderQueue,
        context: &mut Context,
    ) -> bool {
        match *evt {
            Event::ToggleNear(ViewId::MainMenu, rect) => {
                toggle_main_menu(self, rect, None, rq, context);
                true
            }
            Event::ToggleNear(ViewId::BatteryMenu, rect) => {
                toggle_battery_menu(self, rect, None, rq, context);
                true
            }
            Event::ToggleNear(ViewId::ClockMenu, rect) => {
                toggle_clock_menu(self, rect, None, rq, context);
                true
            }
            Event::ToggleNear(ViewId::OpdsServerMenu, rect) => {
                self.toggle_server_menu(rect, None, rq, context);
                true
            }
            Event::Select(EntryId::OpdsServer(index)) => {
                self.load_server(index, hub, rq, context);
                true
            }
            Event::OpdsHome => {
                self.home(hub, rq, context);
                true
            }
            Event::OpdsDocumentDownloaded => {
                self.update_browser(hub, rq, context); //Fixes the active flag
                                                       //TODO: replace with AddDocument
                context.library.import(&context.settings.import);
                true
            }
            Event::Select(EntryId::OpdsEntry(ref entry_id)) => {
                self.select_entry(entry_id, hub, rq, context);
                true
            }
            Event::Reseed => {
                self.reseed(rq, context);
                true
            }
            Event::Page(dir) => {
                self.go_to_neighbor(dir, hub, rq, context);
                true
            }
            Event::Device(DeviceEvent::Button {
                code: ButtonCode::Backward,
                status: ButtonStatus::Pressed,
                ..
            }) => {
                self.go_to_neighbor(CycleDir::Previous, hub, rq, context);
                true
            }
            Event::Device(DeviceEvent::Button {
                code: ButtonCode::Forward,
                status: ButtonStatus::Pressed,
                ..
            }) => {
                self.go_to_neighbor(CycleDir::Next, hub, rq, context);
                true
            }
            _ => false,
        }
    }

    fn render(&self, _fb: &mut dyn Framebuffer, _rect: Rectangle, _fonts: &mut Fonts) {}

    fn resize(&mut self, rect: Rectangle, hub: &Hub, rq: &mut RenderQueue, context: &mut Context) {
        let dpi = CURRENT_DEVICE.dpi;
        let thickness = scale_by_dpi(THICKNESS_MEDIUM, dpi) as i32;
        let (small_thickness, big_thickness) = halves(thickness);
        let small_height = scale_by_dpi(SMALL_BAR_HEIGHT, dpi) as i32;

        self.children.retain(|child| !child.is::<Menu>());

        let top_bar_rect = rect![
            rect.min.x,
            rect.min.y,
            rect.max.x,
            rect.min.y + small_height - small_thickness
        ];

        self.children[0].resize(top_bar_rect, hub, rq, context);

        let separator_rect = rect![
            rect.min.x,
            rect.min.y + small_height - small_thickness,
            rect.max.x,
            rect.min.y + small_height + big_thickness
        ];

        self.children[1].resize(separator_rect, hub, rq, context);

        let navigation_rect = rect![
            rect.min.x,
            rect.min.y + small_height + big_thickness,
            rect.max.x,
            rect.min.y + 2 * small_height - small_thickness
        ];
        self.children[2].resize(navigation_rect, hub, rq, context);

        let separator_rect = rect![
            rect.min.x,
            rect.min.y + 2 * small_height - small_thickness,
            rect.max.x,
            rect.min.y + 2 * small_height + big_thickness
        ];
        self.children[3].resize(separator_rect, hub, rq, context);

        let browser_rect = rect![
            rect.min.x,
            rect.min.y + 2 * small_height + big_thickness,
            rect.max.x,
            rect.max.y - small_height - small_thickness
        ];
        self.children[4].resize(browser_rect, hub, rq, context);

        let separator_rect = rect![
            rect.min.x,
            rect.max.y - small_height - small_thickness,
            rect.max.x,
            rect.max.y - small_height + big_thickness
        ];
        self.children[5].resize(separator_rect, hub, rq, context);

        let bottom_bar_rect = rect![
            rect.min.x,
            rect.max.y - small_height + big_thickness,
            rect.max.x,
            rect.max.y
        ];
        self.children[6].resize(bottom_bar_rect, hub, rq, context);

        self.update_browser(hub, &mut RenderQueue::new(), context);
        self.update_bottom_bar(hub, &mut RenderQueue::new(), context);

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
