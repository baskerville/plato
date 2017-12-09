use std::rc::Rc;
use framebuffer::{Framebuffer, UpdateMode, Pixmap};
use view::{View, Event, Hub, Bus, ViewId, EntryId};
use gesture::GestureEvent;
use geom::{Rectangle, Dir, CycleDir};
use color::WHITE;
use app::Context;
use font::Fonts;

pub struct Viewer {
    rect: Rectangle,
    children: Vec<Box<View>>,
    frame: Rectangle,
    pixmap: Rc<Pixmap>,
    update_mode: UpdateMode,
}

impl Viewer {
    pub fn new(rect: Rectangle, pixmap: Rc<Pixmap>, frame: Rectangle, update_mode: UpdateMode)-> Viewer {
        Viewer {
            rect,
            children: vec![],
            frame,
            pixmap,
            update_mode,
        }
    }

    pub fn update(&mut self, pixmap: Rc<Pixmap>, frame: Rectangle, update_mode: UpdateMode, hub: &Hub) {
        self.pixmap = pixmap;
        self.frame = frame;
        self.update_mode = update_mode;
        hub.send(Event::Render(self.rect, update_mode)).unwrap();
    }
}

impl View for Viewer {
    fn handle_event(&mut self, evt: &Event, hub: &Hub, bus: &mut Bus, _context: &mut Context) -> bool {
        match *evt {
            Event::Gesture(GestureEvent::Swipe { dir, ref start, .. }) if self.rect.includes(start) => {
                match dir {
                    Dir::West => {
                        bus.push_back(Event::Page(CycleDir::Next));
                    },
                    Dir::East => {
                        bus.push_back(Event::Page(CycleDir::Previous));
                    },
                    _ => (),
                };
                true
            },
            Event::Gesture(GestureEvent::HoldFinger(ref center)) if self.rect.includes(center) => {
                let w = self.rect.width() as i32;
                let x1 = self.rect.min.x + w / 3;
                let x2 = self.rect.max.x - w / 3;
                if center.x < x1 {
                    bus.push_back(Event::Chapter(CycleDir::Previous));
                } else if center.x > x2 {
                    bus.push_back(Event::Chapter(CycleDir::Next));
                } else {
                    hub.send(Event::Select(EntryId::TakeScreenshot)).unwrap();
                }
                true
            },
            Event::Gesture(GestureEvent::Tap { ref center, .. }) if self.rect.includes(center) => {
                let w = self.rect.width() as i32;
                let x1 = self.rect.min.x + w / 3;
                let x2 = self.rect.max.x - w / 3;
                if center.x < x1 {
                    bus.push_back(Event::Page(CycleDir::Previous));
                } else if center.x > x2 {
                    bus.push_back(Event::Page(CycleDir::Next));
                } else {
                    bus.push_back(Event::Toggle(ViewId::TopBottomBars));
                }
                true
            },
            _ => false,
        }
    }

    fn render(&self, fb: &mut Framebuffer, _fonts: &mut Fonts) {
        let dx = (self.rect.width() - self.frame.width()) as i32 / 2;
        let dy = (self.rect.height() - self.frame.height()) as i32 / 2;
        fb.draw_rectangle(&self.rect, WHITE);
        fb.draw_framed_pixmap(&self.pixmap, &self.frame, &pt!(dx, dy));
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
