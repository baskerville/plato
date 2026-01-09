use std::sync::Mutex;
use lazy_static::lazy_static;
use super::feed_entry::FeedEntry;
use crate::device::CURRENT_DEVICE;
use crate::opds::Entry;
use crate::view::{View, Event, Hub, Bus, Id, ID_FEEDER, RenderQueue, RenderData};
use crate::view::{BIG_BAR_HEIGHT, THICKNESS_MEDIUM};
use crate::view::filler::Filler;
use crate::framebuffer::{Framebuffer, UpdateMode};
use crate::geom::{Rectangle, Dir, CycleDir, halves};
use crate::color::{WHITE, SEPARATOR_NORMAL};
use crate::gesture::GestureEvent;
use crate::unit::scale_by_dpi;
use crate::geom::divide;
use crate::font::Fonts;
use crate::context::Context;

lazy_static! {
    static ref EXCLUSIVE_ACCESS: Mutex<u8> = Mutex::new(0);
}

pub struct FeedBrowser {
    id: Id,
    pub rect: Rectangle,
    children: Vec<Box<dyn View>>,
    pub max_lines: usize,
}

impl FeedBrowser {
    pub fn new(rect: Rectangle) -> FeedBrowser {
        let dpi = CURRENT_DEVICE.dpi;
        let big_height = scale_by_dpi(BIG_BAR_HEIGHT, dpi) as i32;
        let thickness = scale_by_dpi(THICKNESS_MEDIUM, dpi) as i32;
        let max_lines = ((rect.height() as i32 + thickness) / big_height) as usize;
        FeedBrowser {
            id: ID_FEEDER.next(),
            rect,
            children: Vec::new(),
            max_lines
        }
    }

    pub fn update(&mut self, metadata: &[Entry], _hub: &Hub, rq: &mut RenderQueue, _context: &Context) {
        self.children.clear();
        let dpi = CURRENT_DEVICE.dpi;
        let big_height = scale_by_dpi(BIG_BAR_HEIGHT, dpi) as i32;
        let thickness = scale_by_dpi(THICKNESS_MEDIUM, dpi) as i32;
        let (small_thickness, big_thickness) = halves(thickness);
        let max_lines = ((self.rect.height() as i32 + thickness) / big_height) as usize;
        let book_heights = divide(self.rect.height() as i32, max_lines as i32);
        let mut y_pos = self.rect.min.y;

        for (index, info) in metadata.iter().enumerate() {
            let y_min = y_pos + if index > 0 { big_thickness } else { 0 };
            let y_max = y_pos + book_heights[index] - if index < max_lines - 1 { small_thickness } else { 0 };

            let entry = FeedEntry::new(rect![self.rect.min.x, y_min,
                                       self.rect.max.x, y_max],
                                 info.clone());
            self.children.push(Box::new(entry) as Box<dyn View>);

            if index < max_lines - 1 {
                let separator = Filler::new(rect![self.rect.min.x, y_max,
                                                  self.rect.max.x, y_max + thickness],
                                            SEPARATOR_NORMAL);
                self.children.push(Box::new(separator) as Box<dyn View>);
            }

            y_pos += book_heights[index];
        }

        if metadata.len() < max_lines {
            let y_start = y_pos + if metadata.is_empty() { 0 } else { thickness };
            let filler = Filler::new(rect![self.rect.min.x, y_start,
                                           self.rect.max.x, self.rect.max.y],
                                     WHITE);
            self.children.push(Box::new(filler) as Box<dyn View>);
        }

        self.max_lines = max_lines;
        rq.add(RenderData::new(self.id, self.rect, UpdateMode::Partial));
    }
}

impl View for FeedBrowser {
    fn handle_event(&mut self, evt: &Event, _hub: &Hub, bus: &mut Bus, _rq: &mut RenderQueue, _context: &mut Context) -> bool {
        match *evt {
            Event::Gesture(GestureEvent::Swipe { dir, start, .. }) if self.rect.includes(start) => {
                match dir {
                    Dir::West => {
                        bus.push_back(Event::Page(CycleDir::Next));
                        true
                    },
                    Dir::East => {
                        bus.push_back(Event::Page(CycleDir::Previous));
                        true
                    },
                    _ => false,
                }
            },
            _ => false,
        }
    }

    fn render(&self, _fb: &mut dyn Framebuffer, _rect: Rectangle, _fonts: &mut Fonts) {
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
