use crate::device::CURRENT_DEVICE;
use crate::font::{Fonts, font_from_style, NORMAL_STYLE};
use crate::color::{BLACK, WHITE};
use crate::gesture::GestureEvent;
use crate::geom::{Rectangle};
use crate::document::BYTES_PER_PAGE;
use crate::framebuffer::{Framebuffer, UpdateMode};
use super::{View, Event, Hub, Bus, ViewId};
use crate::app::Context;

pub struct PageLabel {
    rect: Rectangle,
    children: Vec<Box<dyn View>>,
    current_page: usize,
    pages_count: usize,
    synthetic: bool,
}

impl PageLabel {
    pub fn new(rect: Rectangle, current_page: usize, pages_count: usize, synthetic: bool)  -> PageLabel {
        PageLabel {
            rect,
            children: vec![],
            current_page,
            pages_count,
            synthetic,
        }
    }

    pub fn update(&mut self, current_page: usize, pages_count: usize, hub: &Hub) {
        self.current_page = current_page;
        self.pages_count = pages_count;
        hub.send(Event::Render(self.rect, UpdateMode::Gui)).ok();
    }

    pub fn text(&self, size: u8) -> String {
        if self.pages_count == 0 {
            return "No pages".to_string();
        }
        let (current_page, pages_count, precision) = if self.synthetic {
            (self.current_page as f64 / BYTES_PER_PAGE,
             self.pages_count as f64 / BYTES_PER_PAGE, 1)
        } else {
            (self.current_page as f64 + 1.0,
             self.pages_count as f64, 0)
        };
        let percent = 100.0 * self.current_page as f32 / self.pages_count as f32;
        match size {
            0 => format!("Page {1:.0$} of {2:.0$} ({3:.1}%)", precision, current_page, pages_count, percent),
            1 => format!("P. {1:.0$} of {2:.0$} ({3:.1}%)", precision, current_page, pages_count, percent),
            2 => format!("{1:.0$}/{2:.0$} ({3:.1}%)", precision, current_page, pages_count, percent),
            3 => format!("{1:.0$} ({2:.1}%)", precision, current_page, percent),
            _ => format!("{:.1}%", percent),
        }
    }
}


impl View for PageLabel {
    fn handle_event(&mut self, evt: &Event, _hub: &Hub, bus: &mut Bus, _context: &mut Context) -> bool {
        match *evt {
            Event::Gesture(GestureEvent::Tap(center)) if self.rect.includes(center) => {
                bus.push_back(Event::Toggle(ViewId::GoToPage));
                true
            },
            Event::Gesture(GestureEvent::HoldFingerShort(center, ..)) if self.rect.includes(center) => {
                bus.push_back(Event::ToggleNear(ViewId::PageMenu, self.rect));
                true
            },
            _ => false,
        }
    }

    fn render(&self, fb: &mut dyn Framebuffer, _rect: Rectangle, fonts: &mut Fonts) {
        let dpi = CURRENT_DEVICE.dpi;
        let font = font_from_style(fonts, &NORMAL_STYLE, dpi);
        let padding = font.em() as i32 / 2;
        let max_width = self.rect.width().saturating_sub(2 * padding as u32) as i32;
        let mut plan = font.plan(&self.text(0), None, None);
        for size in 1..=4 {
            if plan.width <= max_width {
                break;
            }
            plan = font.plan(&self.text(size), None, None);
        }
        font.crop_right(&mut plan, max_width);
        let dx = padding + (max_width - plan.width) / 2;
        let dy = (self.rect.height() as i32 - font.x_heights.0 as i32) / 2;
        let pt = pt!(self.rect.min.x + dx, self.rect.max.y - dy);
        fb.draw_rectangle(&self.rect, WHITE);
        font.render(fb, BLACK, &plan, pt);
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
