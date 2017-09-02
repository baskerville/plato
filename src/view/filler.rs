use std::sync::mpsc::Sender;
use framebuffer::Framebuffer;
use view::{View, Event, ChildEvent};
use font::Fonts;
use geom::Rectangle;

pub struct Filler {
    rect: Rectangle,
    color: u8,
}

impl Filler {
    pub fn new(rect: Rectangle, color: u8) -> Filler {
        Filler {
            rect,
            color,
        }
    }
}

impl View for Filler {
    fn handle_event(&mut self, evt: &Event, bus: &mut Vec<ChildEvent>) -> bool {
        false
    }

    fn render(&self, fb: &mut Framebuffer, _: &mut Fonts) {
        fb.draw_rectangle(&self.rect, self.color);
    }

    fn rect(&self) -> &Rectangle {
        &self.rect
    }

    fn len(&self) -> usize {
        0
    }

    fn child(&self, _: usize) -> &View {
        self
    }

    fn child_mut(&mut self, _: usize) -> &mut View {
        self
    }
}
