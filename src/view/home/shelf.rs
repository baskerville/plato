use super::book::Book;
use crate::device::CURRENT_DEVICE;
use crate::view::{View, Event, Hub, Bus};
use crate::view::{BIG_BAR_HEIGHT, THICKNESS_MEDIUM};
use crate::view::filler::Filler;
use crate::framebuffer::{Framebuffer, UpdateMode};
use crate::settings::{FirstColumn, SecondColumn};
use crate::geom::{Rectangle, Dir, CycleDir, halves};
use crate::color::{WHITE, SEPARATOR_NORMAL};
use crate::gesture::GestureEvent;
use crate::unit::scale_by_dpi;
use crate::metadata::Info;
use crate::geom::divide;
use crate::font::Fonts;
use crate::app::Context;

pub struct Shelf {
    pub rect: Rectangle,
    children: Vec<Box<dyn View>>,
    pub max_lines: usize,
    first_column: FirstColumn,
    second_column: SecondColumn,
}

impl Shelf {
    pub fn new(rect: Rectangle, first_column: FirstColumn, second_column: SecondColumn) -> Shelf {
        let dpi = CURRENT_DEVICE.dpi;
        let big_height = scale_by_dpi(BIG_BAR_HEIGHT, dpi) as i32;
        let thickness = scale_by_dpi(THICKNESS_MEDIUM, dpi) as i32;
        let max_lines = ((rect.height() as i32 + thickness) / big_height) as usize;
        Shelf {
            rect,
            children: vec![],
            max_lines,
            first_column,
            second_column,
        }
    }

    pub fn set_first_column(&mut self, first_column: FirstColumn) {
        self.first_column = first_column;
    }

    pub fn set_second_column(&mut self, second_column: SecondColumn) {
        self.second_column = second_column;
    }

    pub fn update(&mut self, metadata: &[Info], hub: &Hub) {
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
            let book = Book::new(rect![self.rect.min.x, y_min,
                                       self.rect.max.x, y_max],
                                 info.clone(),
                                 index,
                                 self.first_column,
                                 self.second_column);
            self.children.push(Box::new(book) as Box<dyn View>);
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
        hub.send(Event::Render(self.rect, UpdateMode::Partial)).ok();
    }
}

impl View for Shelf {
    fn handle_event(&mut self, evt: &Event, _hub: &Hub, bus: &mut Bus, _context: &mut Context) -> bool {
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
}
