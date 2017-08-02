use std::sync::mpsc::Sender;
use framebuffer::Framebuffer;
use view::{View, Event, ChildEvent};
use device::{CURRENT_DEVICE, BAR_SIZES};
use font::Fonts;
use geom::Rectangle;

#[derive(Debug)]
pub struct Home {
    rect: Rectangle,
    children: Vec<Box<View>>,
}

impl Home {
    pub fn new(rect: Rectangle) -> Home {
        Home {
            rect: rect,
            children: vec![],
        }
    }
}

impl View for Home {
    fn rect(&self) -> &Rectangle {
        &self.rect
    }
    fn len(&self) -> usize {
        self.children.len()
    }
    fn child(&self, index: usize) -> &View {
        self.children[index].as_ref()
    }
    fn child_mut(&mut self, index: usize) -> &mut View {
        self.children[index].as_mut()
    }
    fn handle_event(&mut self, evt: &Event, bus: &Sender<ChildEvent>) -> bool {
        unimplemented!();
    }
    fn render(&self, fb: &mut Framebuffer, _: &mut Fonts) {
        unimplemented!();
    }
}
