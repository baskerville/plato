use font::{Fonts, font_from_style, NORMAL_STYLE};
use color::{BLACK, WHITE};
use geom::{Rectangle};
use app::Context;
use gesture::GestureEvent;
use framebuffer::{Framebuffer, UpdateMode};
use view::{View, Event, Hub, Bus, ViewId};
use device::CURRENT_DEVICE;

// TODO: use a regular label; active state
pub struct SortLabel {
    rect: Rectangle,
    children: Vec<Box<View>>,
    text: String,
}

impl SortLabel {
    pub fn new(rect: Rectangle, text: &str)  -> SortLabel {
        SortLabel {
            rect,
            children: vec![],
            text: text.to_string(),
        }
    }

    pub fn update(&mut self, text: &str, hub: &Hub) {
        self.text = text.to_string();
        hub.send(Event::Render(self.rect, UpdateMode::Gui)).unwrap();
    }
}

impl View for SortLabel {
    fn handle_event(&mut self, evt: &Event, _hub: &Hub, bus: &mut Bus, _context: &mut Context) -> bool {
        match *evt {
            Event::Gesture(GestureEvent::Tap { ref center, .. }) if self.rect.includes(center) => {
                // TODO: use the actual text rectangle
                bus.push_back(Event::ToggleNear(ViewId::SortMenu, self.rect));
                true
            },
            _ => false,
        }
    }

    fn render(&self, fb: &mut Framebuffer, fonts: &mut Fonts) {
        let dpi = CURRENT_DEVICE.dpi;
        let font = font_from_style(fonts, &NORMAL_STYLE, dpi);
        let padding = font.em() as i32;
        let plan = font.plan(&format!("Sort by: {}", self.text),
                             Some(self.rect.width().saturating_sub(padding as u32)),
                             None);
        let dx = (self.rect.width() - plan.width) as i32 / 2;
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
