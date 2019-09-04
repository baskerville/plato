use crate::device::CURRENT_DEVICE;
use crate::geom::{Rectangle, CornerSpec, BorderSpec};
use crate::font::{Fonts, font_from_style, NORMAL_STYLE};
use super::{View, Event, Hub, Bus};
use super::{THICKNESS_MEDIUM, BORDER_RADIUS_LARGE};
use crate::framebuffer::{Framebuffer, UpdateMode};
use crate::input::{DeviceEvent, FingerStatus};
use crate::gesture::GestureEvent;
use crate::color::{BLACK, TEXT_NORMAL, TEXT_INVERTED_HARD};
use crate::unit::scale_by_dpi;
use crate::app::Context;

pub struct Button {
    rect: Rectangle,
    children: Vec<Box<dyn View>>,
    event: Event,
    text: String,
    active: bool,
    pub disabled: bool,
}

impl Button {
    pub fn new(rect: Rectangle, event: Event, text: String) -> Button {
        Button {
            rect,
            children: vec![],
            event,
            text,
            active: false,
            disabled: false,
        }
    }

    pub fn disabled(mut self, value: bool) -> Button {
        self.disabled = value;
        self
    }
}

impl View for Button {
    fn handle_event(&mut self, evt: &Event, hub: &Hub, bus: &mut Bus, _context: &mut Context) -> bool {
        match *evt {
            Event::Device(DeviceEvent::Finger { status, position, .. }) if !self.disabled => {
                match status {
                    FingerStatus::Down if self.rect.includes(position) => {
                        self.active = true;
                        hub.send(Event::Render(self.rect, UpdateMode::Fast)).unwrap();
                        true
                    },
                    FingerStatus::Up if self.active => {
                        self.active = false;
                        hub.send(Event::Render(self.rect, UpdateMode::Gui)).unwrap();
                        true
                    },
                    _ => false,
                }
            },
            Event::Gesture(GestureEvent::Tap(center)) if self.rect.includes(center) => {
                if !self.disabled {
                    bus.push_back(self.event.clone());
                }
                true
            },
            _ => false,
        }
    }

    fn render(&self, fb: &mut dyn Framebuffer, _rect: Rectangle, fonts: &mut Fonts) {
        let dpi = CURRENT_DEVICE.dpi;

        let scheme = if self.active {
            TEXT_INVERTED_HARD
        } else {
            TEXT_NORMAL
        };

        let border_radius = scale_by_dpi(BORDER_RADIUS_LARGE, dpi) as i32;
        let border_thickness = scale_by_dpi(THICKNESS_MEDIUM, dpi) as u16;

        fb.draw_rounded_rectangle_with_border(&self.rect,
                                              &CornerSpec::Uniform(border_radius),
                                              &BorderSpec { thickness: border_thickness,
                                                            color: BLACK },
                                              &scheme[0]);

        let font = font_from_style(fonts, &NORMAL_STYLE, dpi);
        let x_height = font.x_heights.0 as i32;
        let padding = font.em() as i32;
        let max_width = self.rect.width() as i32 - padding;

        let plan = font.plan(&self.text, Some(max_width as u32), None);

        let dx = ((self.rect.width() - plan.width) / 2) as i32;
        let dy = (self.rect.height() as i32 - x_height) / 2;
        let pt = pt!(self.rect.min.x + dx, self.rect.max.y - dy);

        let foreground = if self.disabled { scheme[2] } else { scheme[1] };
        font.render(fb, foreground, &plan, pt);
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
