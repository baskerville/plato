use crate::device::CURRENT_DEVICE;
use crate::font::{Fonts, font_from_style, NORMAL_STYLE};
use crate::framebuffer::{Framebuffer, UpdateMode};
use crate::color::TEXT_NORMAL;
use crate::geom::{Rectangle};
use crate::view::{View, Event, Hub, Bus};
use crate::app::Context;

pub struct ResultsLabel {
    rect: Rectangle,
    children: Vec<Box<dyn View>>,
    count: usize,
    completed: bool,
}

impl ResultsLabel {
    pub fn new(rect: Rectangle, count: usize, completed: bool) -> ResultsLabel {
        ResultsLabel {
            rect,
            children: vec![],
            count,
            completed,
        }
    }

    pub fn update(&mut self, count: usize, hub: &Hub) {
        self.count = count;
        hub.send(Event::Render(self.rect, UpdateMode::Gui)).unwrap();
    }

    fn text(&self) -> String {
        let qualifier = if self.count != 1 {
            "results"
        } else {
            "result"
        };

        if self.count == 0 {
            format!("No {}", qualifier)
        } else {
            format!("{} {}", self.count, qualifier)
        }
    }
}


impl View for ResultsLabel {
    fn handle_event(&mut self, evt: &Event, hub: &Hub, _bus: &mut Bus, _context: &mut Context) -> bool {
        match *evt {
            Event::EndOfSearch => {
                self.completed = true;
                hub.send(Event::Render(self.rect, UpdateMode::Gui)).unwrap();
                false
            },
            _ => false,
        }
    }

    fn render(&self, fb: &mut dyn Framebuffer, _rect: Rectangle, fonts: &mut Fonts) -> Rectangle {
        let dpi = CURRENT_DEVICE.dpi;
        let font = font_from_style(fonts, &NORMAL_STYLE, dpi);
        let padding = font.em() as i32 / 2;
        let max_width = self.rect.width().saturating_sub(2 * padding as u32) as i32;
        let plan = font.plan(&self.text(), Some(max_width as u32), None);
        let dx = padding + (max_width - plan.width as i32) / 2;
        let dy = (self.rect.height() as i32 - font.x_heights.0 as i32) / 2;
        let pt = pt!(self.rect.min.x + dx, self.rect.max.y - dy);
        fb.draw_rectangle(&self.rect, TEXT_NORMAL[0]);
        let color = if self.completed {
            TEXT_NORMAL[1]
        } else {
            TEXT_NORMAL[2]
        };
        font.render(fb, color, &plan, pt);
        self.rect
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
