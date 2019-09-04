use crate::framebuffer::Framebuffer;
use super::{View, Event, Hub, Bus};
use crate::geom::Rectangle;
use crate::app::Context;
use crate::font::Fonts;

pub struct Filler {
    pub rect: Rectangle,
    children: Vec<Box<dyn View>>,
    color: u8,
}

impl Filler {
    pub fn new(rect: Rectangle, color: u8) -> Filler {
        Filler {
            rect,
            children: vec![],
            color,
        }
    }
}

impl View for Filler {
    fn handle_event(&mut self, _evt: &Event, _hub: &Hub, _bus: &mut Bus, _context: &mut Context) -> bool {
        false
    }

    fn render(&self, fb: &mut dyn Framebuffer, _rect: Rectangle, _fonts: &mut Fonts) {
        fb.draw_rectangle(&self.rect, self.color);
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
}
