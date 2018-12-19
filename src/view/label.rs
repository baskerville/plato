use crate::device::CURRENT_DEVICE;
use crate::font::{Fonts, font_from_style, NORMAL_STYLE};
use super::{View, Event, Hub, Bus, Align};
use crate::gesture::GestureEvent;
use crate::framebuffer::{Framebuffer, UpdateMode};
use crate::geom::Rectangle;
use crate::color::TEXT_NORMAL;
use crate::app::Context;

pub struct Label {
    rect: Rectangle,
    children: Vec<Box<View>>,
    text: String,
    align: Align,
    event: Option<Event>,
}

impl Label {
    pub fn new(rect: Rectangle, text: String, align: Align) -> Label {
        Label {
            rect,
            children: vec![],
            text,
            align,
            event: None,
        }
    }

    pub fn event(mut self, event: Option<Event>) -> Label {
        self.event = event;
        self
    }

    pub fn update(&mut self, text: String, hub: &Hub) {
        if self.text != text {
            self.text = text;
            hub.send(Event::Render(self.rect, UpdateMode::Gui)).unwrap();
        }
    }
}

impl View for Label {
    fn handle_event(&mut self, evt: &Event, _hub: &Hub, bus: &mut Bus, _context: &mut Context) -> bool {
        if self.event.is_none() {
            return false;
        }
        match *evt {
            Event::Gesture(GestureEvent::Tap(center)) if self.rect.includes(center) => {
                bus.push_back(self.event.clone().unwrap());
                true
            },
            _ => false,
        }
    }

    fn render(&self, fb: &mut Framebuffer, fonts: &mut Fonts) {
        let dpi = CURRENT_DEVICE.dpi;

        fb.draw_rectangle(&self.rect, TEXT_NORMAL[0]);

        let font = font_from_style(fonts, &NORMAL_STYLE, dpi);
        let x_height = font.x_heights.0 as i32;
        let padding = font.em() as i32;
        let max_width = self.rect.width() as i32 - padding;

        let plan = font.plan(&self.text, Some(max_width as u32), None);

        let dx = self.align.offset(plan.width as i32, self.rect.width() as i32);
        let dy = (self.rect.height() as i32 - x_height) / 2;
        let pt = pt!(self.rect.min.x + dx, self.rect.max.y - dy);

        font.render(fb, TEXT_NORMAL[1], &plan, pt);
    }

    fn resize(&mut self, rect: Rectangle, _hub: &Hub, _context: &mut Context) {
        if let Some(Event::ToggleNear(_, ref mut event_rect)) = self.event.as_mut() {
            *event_rect = rect;
        }
        self.rect = rect;
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
