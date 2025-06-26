mod accountview;
mod shelf;

use std::fs::remove_file;

use self::accountview::AccountWindow;
use crate::articles;
use crate::color::BLACK;
use crate::context::Context;
use crate::device::CURRENT_DEVICE;
use crate::font::Fonts;
use crate::framebuffer::{Framebuffer, UpdateMode};
use crate::geom::{halves, CycleDir, Rectangle};
use crate::settings::{ArticleAuth, ArticleList};
use crate::unit::scale_by_dpi;
use crate::view::articles::shelf::Shelf;
use crate::view::common::{locate, locate_by_id};
use crate::view::common::{toggle_battery_menu, toggle_clock_menu, toggle_main_menu};
use crate::view::filler::Filler;
use crate::view::menu::{Menu, MenuKind};
use crate::view::pager_bar::PagerBar;
use crate::view::top_bar::TopBar;
use crate::view::{
    library_label, ArticleUpdateProgress, Bus, Event, Hub, RenderData, RenderQueue, View,
};
use crate::view::{EntryId, EntryKind, Id, ViewId, ID_FEEDER};
use crate::view::{BIG_BAR_HEIGHT, SMALL_BAR_HEIGHT, THICKNESS_MEDIUM};

const ARTICLES_TITLE: &str = "Articles";

pub struct Articles {
    id: Id,
    rect: Rectangle,
    children: Vec<Box<dyn View>>,
    current_page: usize,
    pages_count: usize,
    service: Box<dyn articles::Service>,
    articles: Vec<articles::Article>,
}

impl Articles {
    pub fn new(rect: Rectangle, rq: &mut RenderQueue, context: &mut Context) -> Articles {
        let id = ID_FEEDER.next();
        let mut children = Vec::new();

        let dpi = CURRENT_DEVICE.dpi;
        let thickness = scale_by_dpi(THICKNESS_MEDIUM, dpi) as i32;
        let (small_thickness, big_thickness) = halves(thickness);
        let (small_height, _big_height) = (
            scale_by_dpi(SMALL_BAR_HEIGHT, dpi) as i32,
            scale_by_dpi(BIG_BAR_HEIGHT, dpi) as i32,
        );
        let side = small_height;

        let top_bar = TopBar::new(
            rect![
                rect.min.x,
                rect.min.y,
                rect.max.x,
                rect.min.y + side - small_thickness
            ],
            Event::Back,
            ARTICLES_TITLE.to_string(),
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

        let shelf = Shelf::new(rect![
            rect.min.x,
            rect.min.y + small_height + big_thickness,
            rect.max.x,
            rect.max.y - small_height - small_thickness
        ]);

        children.push(Box::new(shelf) as Box<dyn View>);

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

        let bottom_bar = PagerBar::new(
            rect![
                rect.min.x,
                rect.max.y - small_height + big_thickness,
                rect.max.x,
                rect.max.y
            ],
            0,
            0,
            "",
            0,
            library_label::Kind::Articles,
        );
        children.push(Box::new(bottom_bar) as Box<dyn View>);

        rq.add(RenderData::new(id, rect, UpdateMode::Full));

        let mut articles = Articles {
            id: id,
            rect: rect,
            children: children,
            current_page: 0,
            pages_count: 0,
            service: articles::load(context.settings.article_auth.clone()),
            articles: Vec::new(),
        };
        articles.update_shelf(&mut RenderQueue::new(), context);
        return articles;
    }

    fn update_shelf(&mut self, rq: &mut RenderQueue, context: &mut Context) {
        // Update list of articles.
        self.articles = articles::filter(&self.service, context.settings.article_list);

        let shelf = self.children[2].as_mut().downcast_mut::<Shelf>().unwrap();

        let max_lines = shelf.max_lines();
        self.pages_count = (self.articles.len() + max_lines - 1) / max_lines as usize;
        let index_lower = self.current_page * max_lines;
        let index_upper = (index_lower + max_lines).min(self.articles.len());
        shelf.update(&self.articles[index_lower..index_upper], rq);

        let bottom_bar = self.children[4]
            .as_mut()
            .downcast_mut::<PagerBar>()
            .unwrap();
        let name = match context.settings.article_list {
            ArticleList::Unread => "Unread",
            ArticleList::Starred => "Starred",
            ArticleList::Archive => "Archive",
        };
        bottom_bar.update_library_label(
            name,
            self.articles.len(),
            library_label::Kind::Articles,
            rq,
        );
        bottom_bar.update_page_label(self.current_page, self.pages_count, rq);
        bottom_bar.update_icons(self.current_page, self.pages_count, rq);
    }

    fn update_top_bar(&mut self, title: &str, rq: &mut RenderQueue) {
        if let Some(index) = locate::<TopBar>(self) {
            let top_bar = self.children[index]
                .as_mut()
                .downcast_mut::<TopBar>()
                .unwrap();
            top_bar.update_title_label(title, rq);
        }
    }

    fn toggle_articles_menu(
        &mut self,
        rect: Rectangle,
        enable: Option<bool>,
        rq: &mut RenderQueue,
        context: &mut Context,
    ) {
        if let Some(index) = locate_by_id(self, ViewId::ArticlesMenu) {
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

            let mut entries = vec![];

            entries.push(EntryKind::RadioButton(
                "Archive".to_string(),
                EntryId::ArticleList(ArticleList::Archive),
                context.settings.article_list == ArticleList::Archive,
            ));
            entries.push(EntryKind::RadioButton(
                "Starred".to_string(),
                EntryId::ArticleList(ArticleList::Starred),
                context.settings.article_list == ArticleList::Starred,
            ));
            entries.push(EntryKind::RadioButton(
                "Unread".to_string(),
                EntryId::ArticleList(ArticleList::Unread),
                context.settings.article_list == ArticleList::Unread,
            ));

            entries.push(EntryKind::Separator);

            if context.settings.article_auth.api.is_empty() {
                entries.push(EntryKind::Command(
                    "Log in to Wallabag...".to_string(),
                    EntryId::LoginWallabag,
                ));
            } else {
                entries.push(EntryKind::Command("Logout".to_string(), EntryId::Logout));
                entries.push(EntryKind::Command("Update".to_string(), EntryId::Update));
            }

            let menu = Menu::new(
                rect,
                ViewId::ArticlesMenu,
                MenuKind::DropDown,
                entries,
                context,
            );
            rq.add(RenderData::new(menu.id(), *menu.rect(), UpdateMode::Gui));
            self.children.push(Box::new(menu) as Box<dyn View>);
        }
    }

    fn article_index(&mut self, index: usize) -> usize {
        let max_lines = self.children[2]
            .as_mut()
            .downcast_ref::<Shelf>()
            .unwrap()
            .max_lines();

        self.current_page * max_lines + index
    }

    fn toggle_article_menu(
        &mut self,
        index: usize,
        rect: Rectangle,
        enable: Option<bool>,
        rq: &mut RenderQueue,
        context: &mut Context,
    ) {
        if let Some(index) = locate_by_id(self, ViewId::BookMenu) {
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

            let index = self.article_index(index);
            let article = &self.articles[index];

            let mut entries = Vec::new();

            if article.starred {
                entries.push(EntryKind::Command(
                    "Unstar".to_string(),
                    EntryId::Unstar(article.id.clone()),
                ));
            } else {
                entries.push(EntryKind::Command(
                    "Star".to_string(),
                    EntryId::Star(article.id.clone()),
                ));
            }

            if article.archived {
                entries.push(EntryKind::Command(
                    "Unarchive".to_string(),
                    EntryId::Unarchive(article.id.clone()),
                ));
            } else {
                entries.push(EntryKind::Command(
                    "Archive".to_string(),
                    EntryId::Archive(article.id.clone()),
                ));
            }

            entries.push(EntryKind::Command(
                "Delete".to_string(),
                EntryId::Delete(article.id.clone()),
            ));

            entries.push(EntryKind::Separator);

            if article.file().path.exists() {
                entries.push(EntryKind::Command(
                    "Remove download".to_string(),
                    EntryId::RemoveDownload(article.id.clone()),
                ));
            } else {
                entries.push(EntryKind::Command(
                    "Download".to_string(),
                    EntryId::Download(article.id.clone()),
                ));
            }

            entries.push(EntryKind::Separator);

            let article_menu = Menu::new(
                rect,
                ViewId::BookMenu,
                MenuKind::Contextual,
                entries,
                context,
            );
            rq.add(RenderData::new(
                article_menu.id(),
                *article_menu.rect(),
                UpdateMode::Gui,
            ));
            self.children.push(Box::new(article_menu) as Box<dyn View>);
        }
    }

    fn close_authentication_window(&mut self, rq: &mut RenderQueue) {
        if let Some(index) = locate::<AccountWindow>(self) {
            self.children.remove(index);
            rq.add(RenderData::expose(self.rect, UpdateMode::Gui));
        }
    }

    fn reseed(&mut self, _hub: &Hub, rq: &mut RenderQueue, context: &mut Context) {
        self.update_shelf(&mut RenderQueue::new(), context);

        rq.add(RenderData::new(self.id, self.rect, UpdateMode::Gui));
    }
}

impl View for Articles {
    fn handle_event(
        &mut self,
        evt: &Event,
        hub: &Hub,
        _bus: &mut Bus,
        rq: &mut RenderQueue,
        context: &mut Context,
    ) -> bool {
        match *evt {
            Event::Select(EntryId::LoginWallabag) => {
                let view = AccountWindow::new(
                    context,
                    "wallabag".to_string(),
                    "app.wallabag.it".to_string(),
                    "Wallabag".to_string(),
                );
                rq.add(RenderData::new(view.id(), *view.rect(), UpdateMode::Gui));
                self.children.push(Box::new(view) as Box<dyn View>);
                true
            }
            Event::Select(EntryId::Logout) => {
                // TODO: remove all existing articles.
                context.settings.article_auth = ArticleAuth::default();
                self.service = articles::load(ArticleAuth::default());
                self.update_shelf(rq, context);
                true
            }
            Event::Select(EntryId::Update) => {
                if !self.service.update(hub) {
                    hub.send(Event::Notify("Update already in progress".to_string()))
                        .ok();
                }
                true
            }
            Event::Select(EntryId::ArticleList(list)) => {
                if list != context.settings.article_list {
                    context.settings.article_list = list;
                    self.current_page = 0;
                    self.update_shelf(&mut RenderQueue::new(), context);
                }
                true
            }
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
            Event::ToggleNear(ViewId::LibraryMenu, rect) => {
                self.toggle_articles_menu(rect, None, rq, context);
                true
            }
            Event::Close(ViewId::ArticlesSettings) => {
                self.close_authentication_window(rq);
                true
            }
            Event::ArticlesAuth(ref result) => {
                match result {
                    Ok(auth) => {
                        self.close_authentication_window(rq);
                        context.settings.article_auth = auth.clone();
                        self.service = articles::load(context.settings.article_auth.clone());
                        self.service.update(hub);
                    }
                    Err(msg) => {
                        // Shouldn't happen? All cases should have been handled
                        // in accountview.
                        println!("auth fail: {msg}");
                    }
                }

                true
            }
            Event::ArticleUpdateProgress(progress) => {
                match progress {
                    ArticleUpdateProgress::ListStart => {
                        self.update_top_bar(
                            (ARTICLES_TITLE.to_owned() + " (updating)").as_str(),
                            rq,
                        );
                    }
                    ArticleUpdateProgress::ListFinished => {
                        self.update_shelf(rq, context);
                    }
                    ArticleUpdateProgress::Download(current, total) => {
                        let title = format!("{ARTICLES_TITLE} (downloading {current}/{total})");
                        self.update_top_bar(title.as_str(), rq);
                    }
                    ArticleUpdateProgress::Finish => {
                        self.update_top_bar(ARTICLES_TITLE, rq);
                    }
                };

                true
            }
            Event::Select(EntryId::Archive(ref id)) => {
                let index = self.service.index();
                if let Some(article) = index.lock().unwrap().articles.get_mut(id) {
                    article.archived = true;
                    article.changed.insert(articles::Changes::Archived);
                }
                self.update_shelf(&mut RenderQueue::new(), context);
                self.service.save_index();
                true
            }
            Event::Select(EntryId::Unarchive(ref id)) => {
                let index = self.service.index();
                if let Some(article) = index.lock().unwrap().articles.get_mut(id) {
                    article.archived = false;
                    article.changed.insert(articles::Changes::Archived);
                }
                self.update_shelf(&mut RenderQueue::new(), context);
                self.service.save_index();
                true
            }
            Event::Select(EntryId::Star(ref id)) => {
                let index = self.service.index();
                if let Some(article) = index.lock().unwrap().articles.get_mut(id) {
                    article.starred = true;
                    article.changed.insert(articles::Changes::Starred);
                }
                self.update_shelf(&mut RenderQueue::new(), context);
                self.service.save_index();
                true
            }
            Event::Select(EntryId::Unstar(ref id)) => {
                let index = self.service.index();
                if let Some(article) = index.lock().unwrap().articles.get_mut(id) {
                    article.starred = false;
                    article.changed.insert(articles::Changes::Starred);
                }
                self.update_shelf(&mut RenderQueue::new(), context);
                self.service.save_index();
                true
            }
            Event::Select(EntryId::Delete(ref id)) => {
                let index = self.service.index();
                if let Some(article) = index.lock().unwrap().articles.get_mut(id) {
                    article.changed.insert(articles::Changes::Deleted);
                }
                self.update_shelf(&mut RenderQueue::new(), context);
                self.service.save_index();
                true
            }
            Event::Select(EntryId::Download(ref id)) => {
                println!("todo: download article {id}");
                true
            }
            Event::Select(EntryId::RemoveDownload(ref id)) => {
                let index = self.service.index();
                if let Some(article) = &index.lock().unwrap().articles.get(id) {
                    if let Err(err) = remove_file(article.file().path) {
                        hub.send(Event::Notify(
                            format!("failed to remove: {}", err).to_string(),
                        ))
                        .ok();
                    };
                }
                true
            }
            Event::Page(dir) => {
                match dir {
                    CycleDir::Next => {
                        if self.current_page + 1 < self.pages_count {
                            self.current_page += 1;
                            self.update_shelf(rq, context);
                        }
                    }
                    CycleDir::Previous => {
                        if self.current_page > 0 {
                            self.current_page -= 1;
                            self.update_shelf(rq, context);
                        }
                    }
                }

                true
            }
            Event::ToggleBookMenu(rect, index) => {
                self.toggle_article_menu(index, rect, None, rq, context);
                true
            }
            Event::Reseed => {
                self.reseed(hub, rq, context);
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
        let (small_height, _big_height) = (
            scale_by_dpi(SMALL_BAR_HEIGHT, dpi) as i32,
            scale_by_dpi(BIG_BAR_HEIGHT, dpi) as i32,
        );
        let side = small_height;

        let top_bar_rect = rect![
            rect.min.x,
            rect.min.y,
            rect.max.x,
            rect.min.y + side - small_thickness
        ];
        self.children[0].resize(top_bar_rect, hub, rq, context);

        let separator_rect = rect![
            rect.min.x,
            rect.min.y + small_height - small_thickness,
            rect.max.x,
            rect.min.y + small_height + big_thickness
        ];
        self.children[1].resize(separator_rect, hub, rq, context);

        let shelf_rect = rect![
            rect.min.x,
            rect.min.y + small_height + big_thickness,
            rect.max.x,
            rect.max.y - small_height - small_thickness
        ];
        self.children[2].resize(shelf_rect, hub, rq, context);

        let separator_rect = rect![
            rect.min.x,
            rect.max.y - small_height - small_thickness,
            rect.max.x,
            rect.max.y - small_height + big_thickness
        ];
        self.children[3].resize(separator_rect, hub, rq, context);

        let bottom_bar_rect = rect![
            rect.min.x,
            rect.max.y - small_height + big_thickness,
            rect.max.x,
            rect.max.y
        ];
        self.children[4].resize(bottom_bar_rect, hub, rq, context);

        self.update_shelf(&mut RenderQueue::new(), context);

        self.rect = rect;
        rq.add(RenderData::new(self.id, self.rect, UpdateMode::Full));
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

    fn id(&self) -> Id {
        self.id
    }
}
