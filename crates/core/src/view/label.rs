use crate::device::CURRENT_DEVICE;
use crate::font::{Fonts, font_from_style, NORMAL_STYLE};
use super::{View, Event, Hub, Bus, Id, ID_FEEDER, RenderQueue, RenderData, Align};
use crate::gesture::GestureEvent;
use crate::framebuffer::{Framebuffer, UpdateMode};
use crate::geom::Rectangle;
use crate::color::TEXT_NORMAL;
use crate::context::Context;

pub struct Label {
    id: Id,
    rect: Rectangle,
    children: Vec<Box<dyn View>>,
    text: String,
    align: Align,
    event: Option<Event>,
    hold_event: Option<Event>,
    font_size: u32,
}

impl Label {
    pub fn new(rect: Rectangle, text: String, align: Align) -> Label {
        Label {
            id: ID_FEEDER.next(),
            rect,
            children: Vec::new(),
            text,
            align,
            event: None,
            hold_event: None,
            font_size: NORMAL_STYLE.size,
        }
    }

    pub fn event(mut self, event: Option<Event>) -> Label {
        self.event = event;
        self
    }

    pub fn hold_event(mut self, event: Option<Event>) -> Label {
        self.hold_event = event;
        self
    }

    pub fn update(&mut self, text: &str, rq: &mut RenderQueue) {
        if self.text != text {
            self.text = text.to_string();
            rq.add(RenderData::new(self.id, self.rect, UpdateMode::Gui));
        }
    }

    pub fn set_font_size(&mut self, font_size: f32) {
        self.font_size = (64.0 * font_size) as u32;
    }
}

impl View for Label {
    fn handle_event(&mut self, evt: &Event, _hub: &Hub, bus: &mut Bus, _rq: &mut RenderQueue, _context: &mut Context) -> bool {
        match *evt {
            Event::Gesture(GestureEvent::Tap(center)) if self.rect.includes(center) => {
                if let Some(event) = self.event.clone() {
                    bus.push_back(event);
                }
                true
            },
            Event::Gesture(GestureEvent::HoldFingerShort(center, _)) if self.rect.includes(center) => {
                if let Some(event) = self.hold_event.clone() {
                    bus.push_back(event);
                }
                true
            },
            _ => false,
        }
    }

    fn render(&self, fb: &mut dyn Framebuffer, _rect: Rectangle, fonts: &mut Fonts) {
        let dpi = CURRENT_DEVICE.dpi;

        fb.draw_rectangle(&self.rect, TEXT_NORMAL[0]);

        let mut style = NORMAL_STYLE; style.size = self.font_size;
        let font = font_from_style(fonts, &style, dpi);
        let x_height = font.x_heights.0 as i32;
        let padding = font.em() as i32;
        let max_width = self.rect.width() as i32 - padding;

        let plan = font.plan(&self.text, Some(max_width), None);

        let dx = self.align.offset(plan.width, self.rect.width() as i32);
        let dy = (self.rect.height() as i32 - x_height) / 2;
        let pt = pt!(self.rect.min.x + dx, self.rect.max.y - dy);

        font.render(fb, TEXT_NORMAL[1], &plan, pt);
    }

    fn resize(&mut self, rect: Rectangle, _hub: &Hub, _rq: &mut RenderQueue, _context: &mut Context) {
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
