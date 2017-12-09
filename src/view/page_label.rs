use device::CURRENT_DEVICE;
use font::{Fonts, font_from_style, NORMAL_STYLE};
use color::{BLACK, WHITE};
use gesture::GestureEvent;
use geom::{Rectangle};
use framebuffer::{Framebuffer, UpdateMode};
use view::{View, Event, Hub, Bus, ViewId};
use app::Context;

pub struct PageLabel {
    rect: Rectangle,
    children: Vec<Box<View>>,
    current_page: usize,
    pages_count: usize,
}

impl PageLabel {
    pub fn new(rect: Rectangle, current_page: usize, pages_count: usize)  -> PageLabel {
        PageLabel {
            rect,
            children: vec![],
            current_page,
            pages_count,
        }
    }

    pub fn update(&mut self, current_page: usize, pages_count: usize, hub: &Hub) {
        self.current_page = current_page;
        self.pages_count = pages_count;
        hub.send(Event::Render(self.rect, UpdateMode::Gui)).unwrap();
    }

    pub fn text(&self) -> String {
        if self.pages_count == 0 {
            "No pages".to_string()
        } else {
            format!("Page {} of {}", self.current_page + 1, self.pages_count)
        }
    }
}


impl View for PageLabel {
    fn handle_event(&mut self, evt: &Event, _hub: &Hub, bus: &mut Bus, _context: &mut Context) -> bool {
        match *evt {
            Event::Gesture(GestureEvent::Tap { ref center, .. }) if self.rect.includes(center) => {
                bus.push_back(Event::Toggle(ViewId::GoToPage));
                true
            },
            _ => false,
        }
    }

    fn render(&self, fb: &mut Framebuffer, fonts: &mut Fonts) {
        let dpi = CURRENT_DEVICE.dpi;
        let font = font_from_style(fonts, &NORMAL_STYLE, dpi);
        let padding = font.em() as i32 / 2;
        let max_width = self.rect.width().saturating_sub(2 * padding as u32) as i32;
        let plan = font.plan(&self.text(), None, None);
        let dx = padding + (max_width - plan.width as i32) / 2;
        let dy = (self.rect.height() as i32 - font.x_heights.0 as i32) / 2;
        let pt = pt!(self.rect.min.x + dx, self.rect.max.y - dy);
        fb.draw_rectangle(&self.rect, WHITE);
        font.render(fb, BLACK, &plan, &pt);
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
