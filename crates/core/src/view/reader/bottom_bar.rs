use display::framebuffer::{Framebuffer, UpdateMode};
use display::{rect, pt};
use crate::view::{View, Event, Hub, Bus, Id, ID_FEEDER, RenderQueue, RenderData};
use crate::view::icon::Icon;
use crate::view::filler::Filler;
use crate::view::page_label::PageLabel;
use super::chapter_label::ChapterLabel;
use crate::gesture::GestureEvent;
use display::input::DeviceEvent;
use display::geom::{Rectangle, CycleDir, halves};
use crate::document::{Document, Neighbors, TocEntry};
use display::color::WHITE;
use crate::font::Fonts;
use crate::context::Context;

#[derive(Debug)]
pub struct BottomBar {
    id: Id,
    rect: Rectangle,
    children: Vec<Box<dyn View>>,
    is_prev_disabled: bool,
    is_next_disabled: bool,
}

impl BottomBar {
    pub fn new(rect: Rectangle, doc: &mut dyn Document, toc: Option<Vec<TocEntry>>, current_page: usize, pages_count: usize, neighbors: &Neighbors, synthetic: bool) -> BottomBar {
        let id = ID_FEEDER.next();
        let mut children = Vec::new();
        let side = rect.height() as i32;
        let is_prev_disabled = neighbors.previous_page.is_none();
        let is_next_disabled = neighbors.next_page.is_none();

        let prev_rect = rect![rect.min, rect.min + side];

        if is_prev_disabled {
            let prev_filler = Filler::new(prev_rect, WHITE);
            children.push(Box::new(prev_filler) as Box<dyn View>);
        } else {
            let prev_icon = Icon::new("arrow-left",
                                      prev_rect,
                                      Event::Page(CycleDir::Previous));
            children.push(Box::new(prev_icon) as Box<dyn View>);
        }

        let (small_half_width, big_half_width) = halves(rect.width() as i32 - 2 * side);


        let chapter_rect = rect![pt!(rect.min.x + side, rect.min.y),
                                 pt!(rect.min.x + side + small_half_width, rect.max.y)];

        let rtoc = toc.or_else(|| doc.toc());
        let chapter = rtoc.as_ref()
                          .and_then(|toc| doc.chapter(current_page, toc));
        let title = chapter.map(|(c, _)| c.title.clone())
                           .unwrap_or_default();
        let progress = chapter.map(|(_, p)| p)
                              .unwrap_or_default();
        let chapter_label = ChapterLabel::new(chapter_rect,
                                              title,
                                              progress);
        children.push(Box::new(chapter_label) as Box<dyn View>);

        let page_label = PageLabel::new(rect![pt!(rect.max.x - side - big_half_width, rect.min.y),
                                              pt!(rect.max.x - side, rect.max.y)],
                                        current_page,
                                        pages_count,
                                        synthetic);
        children.push(Box::new(page_label) as Box<dyn View>);

        let next_rect = rect![rect.max - side, rect.max];

        if is_next_disabled {
            let next_filler = Filler::new(next_rect, WHITE);
            children.push(Box::new(next_filler) as Box<dyn View>);
        } else {
            let next_icon = Icon::new("arrow-right",
                                      rect![rect.max - side, rect.max],
                                      Event::Page(CycleDir::Next));
            children.push(Box::new(next_icon) as Box<dyn View>);
        }

        BottomBar {
            id,
            rect,
            children,
            is_prev_disabled,
            is_next_disabled,
        }
    }

    pub fn update_chapter_label(&mut self, title: String, progress: f32, rq: &mut RenderQueue) {
        let chapter_label = self.child_mut(1).downcast_mut::<ChapterLabel>().unwrap();
        chapter_label.update(title, progress, rq);
    }

    pub fn update_page_label(&mut self, current_page: usize, pages_count: usize, rq: &mut RenderQueue) {
        let page_label = self.child_mut(2).downcast_mut::<PageLabel>().unwrap();
        page_label.update(current_page, pages_count, rq);
    }

    pub fn update_icons(&mut self, neighbors: &Neighbors, rq: &mut RenderQueue) {
        let is_prev_disabled = neighbors.previous_page.is_none();

        if self.is_prev_disabled != is_prev_disabled {
            let index = 0;
            let prev_rect = *self.child(index).rect();
            if is_prev_disabled {
                let prev_filler = Filler::new(prev_rect, WHITE);
                self.children[index] = Box::new(prev_filler) as Box<dyn View>;
            } else {
                let prev_icon = Icon::new("arrow-left",
                                          prev_rect,
                                          Event::Page(CycleDir::Previous));
                self.children[index] = Box::new(prev_icon) as Box<dyn View>;
            }
            self.is_prev_disabled = is_prev_disabled;
            rq.add(RenderData::new(self.id, prev_rect, UpdateMode::Gui));
        }

        let is_next_disabled = neighbors.next_page.is_none();

        if self.is_next_disabled != is_next_disabled {
            let index = self.len() - 1;
            let next_rect = *self.child(index).rect();
            if is_next_disabled {
                let next_filler = Filler::new(next_rect, WHITE);
                self.children[index] = Box::new(next_filler) as Box<dyn View>;
            } else {
                let next_icon = Icon::new("arrow-right",
                                          next_rect,
                                          Event::Page(CycleDir::Next));
                self.children[index] = Box::new(next_icon) as Box<dyn View>;
            }
            self.is_next_disabled = is_next_disabled;
            rq.add(RenderData::new(self.id, next_rect, UpdateMode::Gui));
        }
    }
}

impl View for BottomBar {
    fn handle_event(&mut self, evt: &Event, _hub: &Hub, _bus: &mut Bus, _rq: &mut RenderQueue, _context: &mut Context) -> bool {
        match *evt {
            Event::Gesture(GestureEvent::Tap(center)) |
            Event::Gesture(GestureEvent::HoldFingerShort(center, ..)) if self.rect.includes(center) => true,
            Event::Gesture(GestureEvent::Swipe { start, end, .. }) if self.rect.includes(start) && self.rect.includes(end) => true,
            Event::Device(DeviceEvent::Finger { position, .. }) if self.rect.includes(position) => true,
            _ => false,
        }
    }

    fn render(&self, _fb: &mut dyn Framebuffer, _rect: Rectangle, _fonts: &mut Fonts) {
    }

    fn resize(&mut self, rect: Rectangle, hub: &Hub, rq: &mut RenderQueue, context: &mut Context) {
        let side = rect.height() as i32;
        let (small_half_width, big_half_width) = halves(rect.width() as i32 - 2 * side);
        let prev_rect = rect![rect.min, rect.min + side];
        self.children[0].resize(prev_rect, hub, rq, context);
        let chapter_rect = rect![pt!(rect.min.x + side, rect.min.y),
                                 pt!(rect.min.x + side + small_half_width, rect.max.y)];
        self.children[1].resize(chapter_rect, hub, rq, context);
        let page_label_rect = rect![pt!(rect.max.x - side - big_half_width, rect.min.y),
                                    pt!(rect.max.x - side, rect.max.y)];
        self.children[2].resize(page_label_rect, hub, rq, context);
        let next_rect = rect![rect.max - side, rect.max];
        self.children[3].resize(next_rect, hub, rq, context);
        self.rect = rect;
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
