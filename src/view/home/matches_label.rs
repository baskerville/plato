use device::CURRENT_DEVICE;
use font::{Fonts, font_from_style, NORMAL_STYLE};
use framebuffer::{Framebuffer, UpdateMode};
use gesture::GestureEvent;
use color::{BLACK, WHITE};
use geom::{Rectangle};
use view::{View, Event, Hub, Bus, ViewId};
use app::Context;

pub struct MatchesLabel {
    rect: Rectangle,
    children: Vec<Box<View>>,
    count: usize,
    filter: bool,
}

impl MatchesLabel {
    pub fn new(rect: Rectangle, count: usize, filter: bool)  -> MatchesLabel {
        MatchesLabel {
            rect,
            children: vec![],
            count,
            filter,
        }
    }

    pub fn update(&mut self, count: usize, filter: bool, hub: &Hub) {
        self.count = count;
        self.filter = filter;
        hub.send(Event::Render(self.rect, UpdateMode::Gui)).unwrap();
    }

    fn text(&self) -> String {
        let qualifier = if self.filter {
            if self.count != 1 {
                "matches"
            } else {
                "match"
            }
        } else {
            if self.count != 1 {
                "books"
            } else {
                "book"
            }
        };

        if self.count == 0 {
            format!("No {}", qualifier)
        } else {
            format!("{} {}", self.count, qualifier)
        }
    }
}


impl View for MatchesLabel {
    fn handle_event(&mut self, evt: &Event, _hub: &Hub, bus: &mut Bus, _context: &mut Context) -> bool {
        match *evt {
            Event::Gesture(GestureEvent::Tap(ref center)) if self.rect.includes(center) => {
                bus.push_back(Event::ToggleNear(ViewId::MatchesMenu, self.rect));
                true
            },
            _ => false,
        }
    }

    fn render(&self, fb: &mut Framebuffer, fonts: &mut Fonts) {
        let dpi = CURRENT_DEVICE.dpi;
        let font = font_from_style(fonts, &NORMAL_STYLE, dpi);
        let padding = font.em() as i32 / 2;
        let max_width = self.rect.width().saturating_sub(2 * padding as u32) as i32;
        let plan = font.plan(&self.text(), Some(max_width as u32), None);
        let dx = padding + (max_width - plan.width as i32) / 2;
        let dy = (self.rect.height() as i32 - font.x_heights.0 as i32) / 2;
        let pt = pt!(self.rect.min.x + dx, self.rect.max.y - dy);
        fb.draw_rectangle(&self.rect, WHITE);
        font.render(fb, BLACK, &plan, &pt);
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
