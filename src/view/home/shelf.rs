use crate::device::{CURRENT_DEVICE, BAR_SIZES};
use crate::view::{View, Event, Hub, Bus, THICKNESS_MEDIUM};
use crate::view::filler::Filler;
use super::book::Book;
use crate::framebuffer::{Framebuffer, UpdateMode};
use crate::font::Fonts;
use crate::metadata::Info;
use crate::settings::SecondColumn;
use crate::geom::{Rectangle, Dir, CycleDir};
use crate::color::{WHITE, SEPARATOR_NORMAL};
use crate::gesture::GestureEvent;
use crate::unit::scale_by_dpi;
use crate::app::Context;

pub struct Shelf {
    pub rect: Rectangle,
    children: Vec<Box<View>>,
    pub max_lines: usize,
    second_column: SecondColumn,
}

impl Shelf {
    pub fn new(rect: Rectangle, second_column: SecondColumn) -> Shelf {
        Shelf {
            rect,
            children: vec![],
            max_lines: 0,
            second_column,
        }
    }

    pub fn set_second_column(&mut self, second_column: SecondColumn) {
        self.second_column = second_column;
    }

    pub fn update(&mut self, metadata: &[Info], hub: &Hub, context: &Context) {
        self.children.clear();
        let dpi = CURRENT_DEVICE.dpi;
        let (_, height) = context.display.dims;
        let &(_, big_height) = BAR_SIZES.get(&(height, dpi)).unwrap();
        let thickness = scale_by_dpi(THICKNESS_MEDIUM, dpi) as i32;
        let max_lines = ((self.rect.height() + thickness as u32) / big_height) as usize;

        for (index, info) in metadata.iter().enumerate() {
            let y_min = self.rect.min.y + index as i32 * big_height as i32;
            let y_max = y_min + big_height as i32 - thickness;
            let book = Book::new(rect![self.rect.min.x, y_min,
                                       self.rect.max.x, y_max],
                                 info.clone(),
                                 index,
                                 self.second_column);
            self.children.push(Box::new(book) as Box<View>);
            if index < max_lines - 1 {
                let separator = Filler::new(rect![self.rect.min.x, y_max,
                                                  self.rect.max.x, y_max + thickness],
                                            SEPARATOR_NORMAL);
                self.children.push(Box::new(separator) as Box<View>);
            }
        }

        if metadata.len() < max_lines {
            let y_min = self.rect.min.y + metadata.len() as i32 * big_height as i32;
            let filler = Filler::new(rect![self.rect.min.x, y_min,
                                           self.rect.max.x, self.rect.max.y],
                                     WHITE);
            self.children.push(Box::new(filler) as Box<View>);
        }

        self.max_lines = max_lines;
        hub.send(Event::Render(self.rect, UpdateMode::Gui)).unwrap();
    }
}

impl View for Shelf {
    fn handle_event(&mut self, evt: &Event, _hub: &Hub, bus: &mut Bus, _context: &mut Context) -> bool {
        match *evt {
            Event::Gesture(GestureEvent::Swipe { dir, start, end, .. }) if self.rect.includes(start) => {
                match dir {
                    Dir::West => {
                        bus.push_back(Event::Page(CycleDir::Next));
                        true
                    },
                    Dir::East => {
                        bus.push_back(Event::Page(CycleDir::Previous));
                        true
                    },
                    Dir::North if !self.rect.includes(end) => {
                        bus.push_back(Event::ResizeSummary(end.y - self.rect.min.y));
                        true
                    },
                    _ => false,
                }
            },
            _ => false,
        }
    }

    fn render(&self, _fb: &mut Framebuffer, _fonts: &mut Fonts) {}

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
