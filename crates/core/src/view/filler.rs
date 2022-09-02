use crate::framebuffer::Framebuffer;
use super::{View, Event, Hub, Bus, Id, ID_FEEDER, RenderQueue};
use crate::geom::Rectangle;
use crate::context::Context;
use crate::font::Fonts;

pub struct Filler {
    id: Id,
    pub rect: Rectangle,
    children: Vec<Box<dyn View>>,
    color: u8,
}

impl Filler {
    pub fn new(rect: Rectangle, color: u8) -> Filler {
        Filler {
            id: ID_FEEDER.next(),
            rect,
            children: Vec::new(),
            color,
        }
    }
}

impl View for Filler {
    fn handle_event(&mut self, _evt: &Event, _hub: &Hub, _bus: &mut Bus, _rq: &mut RenderQueue, _context: &mut Context) -> bool {
        false
    }

    fn render(&self, fb: &mut dyn Framebuffer, rect: Rectangle, _fonts: &mut Fonts) {
        if let Some(r) = self.rect.intersection(&rect) {
            fb.draw_rectangle(&r, self.color);
        }
    }

    fn render_rect(&self, rect: &Rectangle) -> Rectangle {
        rect.intersection(&self.rect)
            .unwrap_or(self.rect)
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
