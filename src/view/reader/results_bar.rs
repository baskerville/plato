use framebuffer::{Framebuffer, UpdateMode};
use view::{View, Event, Hub, Bus, ViewId};
use view::icon::Icon;
use view::filler::Filler;
use view::reader::results_label::ResultsLabel;
use view::page_label::PageLabel;
use gesture::GestureEvent;
use input::DeviceEvent;
use geom::{Rectangle, CycleDir, halves};
use color::WHITE;
use app::Context;
use font::Fonts;

#[derive(Debug)]
pub struct ResultsBar {
    rect: Rectangle,
    children: Vec<Box<View>>,
    is_prev_disabled: bool,
    is_next_disabled: bool,
}

impl ResultsBar {
    pub fn new(rect: Rectangle, current_page: usize, pages_count: usize, count: usize, completed: bool) -> ResultsBar {
        let mut children = Vec::new();
        let side = rect.height() as i32;
        let is_prev_disabled = pages_count < 2 || current_page == 0;
        let is_next_disabled = pages_count < 2 || current_page == pages_count - 1;

        let prev_rect = rect![rect.min, rect.min + side];

        if is_prev_disabled {
            let prev_filler = Filler::new(prev_rect, WHITE);
            children.push(Box::new(prev_filler) as Box<View>);
        } else {
            let prev_icon = Icon::new("angle-left",
                                      prev_rect,
                                      Event::ResultsPage(CycleDir::Previous));
            children.push(Box::new(prev_icon) as Box<View>);
        }

        let (small_half_width, big_half_width) = halves(rect.width() as i32 - 2 * side);
        let results_label = ResultsLabel::new(rect![pt!(rect.min.x + side, rect.min.y),
                                                    pt!(rect.min.x + side + small_half_width, rect.max.y)],
                                              count,
                                              completed);
        children.push(Box::new(results_label) as Box<View>);

        let page_label = PageLabel::new(rect![pt!(rect.max.x - side - big_half_width, rect.min.y),
                                              pt!(rect.max.x - side, rect.max.y)],
                                        current_page,
                                        pages_count);
        children.push(Box::new(page_label) as Box<View>);

        let next_rect = rect![rect.max - side, rect.max];

        if is_next_disabled {
            let next_filler = Filler::new(next_rect, WHITE);
            children.push(Box::new(next_filler) as Box<View>);
        } else {
            let next_icon = Icon::new("angle-right",
                                      rect![rect.max - side, rect.max],
                                      Event::ResultsPage(CycleDir::Next));
            children.push(Box::new(next_icon) as Box<View>);
        }

        ResultsBar {
            rect,
            children,
            is_prev_disabled,
            is_next_disabled,
        }
    }

    pub fn update_results_label(&mut self, count: usize, hub: &Hub) {
        let results_label = self.children[1].as_mut().downcast_mut::<ResultsLabel>().unwrap();
        results_label.update(count, hub);
    }

    pub fn update_page_label(&mut self, current_page: usize, pages_count: usize, hub: &Hub) {
        let page_label = self.children[2].as_mut().downcast_mut::<PageLabel>().unwrap();
        page_label.update(current_page, pages_count, hub);
    }

    pub fn update_icons(&mut self, current_page: usize, pages_count: usize, hub: &Hub) {
        let is_prev_disabled = pages_count < 2 || current_page == 0;

        if self.is_prev_disabled != is_prev_disabled {
            let index = 0;
            let prev_rect = *self.child(index).rect();
            if is_prev_disabled {
                let prev_filler = Filler::new(prev_rect, WHITE);
                self.children[index] = Box::new(prev_filler) as Box<View>;
            } else {
                let prev_icon = Icon::new("angle-left",
                                          prev_rect,
                                          Event::ResultsPage(CycleDir::Previous));
                self.children[index] = Box::new(prev_icon) as Box<View>;
            }
            self.is_prev_disabled = is_prev_disabled;
            hub.send(Event::Render(prev_rect, UpdateMode::Gui)).unwrap();
        }

        let is_next_disabled = pages_count < 2 || current_page == pages_count - 1;

        if self.is_next_disabled != is_next_disabled {
            let index = self.len() - 1;
            let next_rect = *self.child(index).rect();
            if is_next_disabled {
                let next_filler = Filler::new(next_rect, WHITE);
                self.children[index] = Box::new(next_filler) as Box<View>;
            } else {
                let next_icon = Icon::new("angle-right",
                                          next_rect,
                                          Event::ResultsPage(CycleDir::Next));
                self.children[index] = Box::new(next_icon) as Box<View>;
            }
            self.is_next_disabled = is_next_disabled;
            hub.send(Event::Render(next_rect, UpdateMode::Gui)).unwrap();
        }
    }
}

impl View for ResultsBar {
    fn handle_event(&mut self, evt: &Event, _hub: &Hub, bus: &mut Bus, _context: &mut Context) -> bool {
        match *evt {
            Event::Toggle(ViewId::GoToPage) => {
                bus.push_back(Event::Toggle(ViewId::GoToResultsPage));
                true
            },
            Event::ToggleNear(ViewId::PageMenu, _) => true,
            Event::Gesture(GestureEvent::Tap(ref center)) |
            Event::Gesture(GestureEvent::HoldFinger(ref center)) if self.rect.includes(center) => true,
            Event::Gesture(GestureEvent::Swipe { ref start, .. }) if self.rect.includes(start) => true,
            Event::Device(DeviceEvent::Finger { ref position, .. }) if self.rect.includes(position) => true,
            _ => false,
        }
    }

    fn render(&self, _fb: &mut Framebuffer, _fonts: &mut Fonts) {
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
