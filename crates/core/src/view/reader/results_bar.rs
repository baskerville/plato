use crate::framebuffer::{Framebuffer, UpdateMode};
use crate::view::{View, Event, Hub, Bus, Id, ID_FEEDER, RenderQueue, RenderData, ViewId};
use crate::view::icon::Icon;
use crate::view::filler::Filler;
use crate::view::page_label::PageLabel;
use super::results_label::ResultsLabel;
use crate::gesture::GestureEvent;
use crate::input::DeviceEvent;
use crate::geom::{Rectangle, CycleDir, halves};
use crate::color::WHITE;
use crate::context::Context;
use crate::font::Fonts;

#[derive(Debug)]
pub struct ResultsBar {
    id: Id,
    rect: Rectangle,
    children: Vec<Box<dyn View>>,
    is_prev_disabled: bool,
    is_next_disabled: bool,
}

impl ResultsBar {
    pub fn new(rect: Rectangle, current_page: usize, pages_count: usize, count: usize, completed: bool) -> ResultsBar {
        let id = ID_FEEDER.next();
        let mut children = Vec::new();
        let side = rect.height() as i32;
        let is_prev_disabled = pages_count < 2 || current_page == 0;
        let is_next_disabled = pages_count < 2 || current_page == pages_count - 1;

        let prev_rect = rect![rect.min, rect.min + side];

        if is_prev_disabled {
            let prev_filler = Filler::new(prev_rect, WHITE);
            children.push(Box::new(prev_filler) as Box<dyn View>);
        } else {
            let prev_icon = Icon::new("angle-left",
                                      prev_rect,
                                      Event::ResultsPage(CycleDir::Previous));
            children.push(Box::new(prev_icon) as Box<dyn View>);
        }

        let (small_half_width, big_half_width) = halves(rect.width() as i32 - 2 * side);
        let results_label = ResultsLabel::new(rect![pt!(rect.min.x + side, rect.min.y),
                                                    pt!(rect.min.x + side + small_half_width, rect.max.y)],
                                              count,
                                              completed);
        children.push(Box::new(results_label) as Box<dyn View>);

        let page_label = PageLabel::new(rect![pt!(rect.max.x - side - big_half_width, rect.min.y),
                                              pt!(rect.max.x - side, rect.max.y)],
                                        current_page,
                                        pages_count,
                                        false);
        children.push(Box::new(page_label) as Box<dyn View>);

        let next_rect = rect![rect.max - side, rect.max];

        if is_next_disabled {
            let next_filler = Filler::new(next_rect, WHITE);
            children.push(Box::new(next_filler) as Box<dyn View>);
        } else {
            let next_icon = Icon::new("angle-right",
                                      rect![rect.max - side, rect.max],
                                      Event::ResultsPage(CycleDir::Next));
            children.push(Box::new(next_icon) as Box<dyn View>);
        }

        ResultsBar {
            id,
            rect,
            children,
            is_prev_disabled,
            is_next_disabled,
        }
    }

    pub fn update_results_label(&mut self, count: usize, rq: &mut RenderQueue) {
        let results_label = self.children[1].as_mut().downcast_mut::<ResultsLabel>().unwrap();
        results_label.update(count, rq);
    }

    pub fn update_page_label(&mut self, current_page: usize, pages_count: usize, rq: &mut RenderQueue) {
        let page_label = self.children[2].as_mut().downcast_mut::<PageLabel>().unwrap();
        page_label.update(current_page, pages_count, rq);
    }

    pub fn update_icons(&mut self, current_page: usize, pages_count: usize, rq: &mut RenderQueue) {
        let is_prev_disabled = pages_count < 2 || current_page == 0;

        if self.is_prev_disabled != is_prev_disabled {
            let index = 0;
            let prev_rect = *self.child(index).rect();
            if is_prev_disabled {
                let prev_filler = Filler::new(prev_rect, WHITE);
                self.children[index] = Box::new(prev_filler) as Box<dyn View>;
            } else {
                let prev_icon = Icon::new("angle-left",
                                          prev_rect,
                                          Event::ResultsPage(CycleDir::Previous));
                self.children[index] = Box::new(prev_icon) as Box<dyn View>;
            }
            self.is_prev_disabled = is_prev_disabled;
            rq.add(RenderData::new(self.id, prev_rect, UpdateMode::Gui));
        }

        let is_next_disabled = pages_count < 2 || current_page == pages_count - 1;

        if self.is_next_disabled != is_next_disabled {
            let index = self.len() - 1;
            let next_rect = *self.child(index).rect();
            if is_next_disabled {
                let next_filler = Filler::new(next_rect, WHITE);
                self.children[index] = Box::new(next_filler) as Box<dyn View>;
            } else {
                let next_icon = Icon::new("angle-right",
                                          next_rect,
                                          Event::ResultsPage(CycleDir::Next));
                self.children[index] = Box::new(next_icon) as Box<dyn View>;
            }
            self.is_next_disabled = is_next_disabled;
            rq.add(RenderData::new(self.id, next_rect, UpdateMode::Gui));
        }
    }
}

impl View for ResultsBar {
    fn handle_event(&mut self, evt: &Event, _hub: &Hub, bus: &mut Bus, _rq: &mut RenderQueue, _context: &mut Context) -> bool {
        match *evt {
            Event::Toggle(ViewId::GoToPage) => {
                bus.push_back(Event::Toggle(ViewId::GoToResultsPage));
                true
            },
            Event::ToggleNear(ViewId::PageMenu, _) => true,
            Event::Gesture(GestureEvent::Tap(center)) |
            Event::Gesture(GestureEvent::HoldFingerShort(center, ..)) if self.rect.includes(center) => true,
            Event::Gesture(GestureEvent::Swipe { start, .. }) if self.rect.includes(start) => true,
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
        self.children[1].resize(rect![pt!(rect.min.x + side, rect.min.y),
                                      pt!(rect.min.x + side + small_half_width, rect.max.y)],
                                hub, rq, context);
        self.children[2].resize(rect![pt!(rect.max.x - side - big_half_width, rect.min.y),
                                      pt!(rect.max.x - side, rect.max.y)],
                                hub, rq, context);
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
