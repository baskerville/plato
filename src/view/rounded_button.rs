use device::CURRENT_DEVICE;
use framebuffer::{Framebuffer, UpdateMode};
use super::{View, Event, Hub, Bus};
use super::THICKNESS_MEDIUM;
use super::icon::ICONS_PIXMAPS;
use gesture::GestureEvent;
use input::{DeviceEvent, FingerStatus};
use unit::scale_by_dpi;
use font::Fonts;
use app::Context;
use geom::{Rectangle, CornerSpec, BorderSpec};
use color::{TEXT_NORMAL, TEXT_INVERTED_HARD};

pub struct RoundedButton {
    rect: Rectangle,
    children: Vec<Box<View>>,
    name: String,
    event: Event,
    active: bool,
}

impl RoundedButton {
    pub fn new(name: &str, rect: Rectangle, event: Event) -> RoundedButton {
        RoundedButton {
            rect,
            children: vec![],
            name: name.to_string(),
            event,
            active: false,
        }
    }
}

impl View for RoundedButton {
    fn handle_event(&mut self, evt: &Event, hub: &Hub, bus: &mut Bus, _context: &mut Context) -> bool {
        match *evt {
            Event::Device(DeviceEvent::Finger { status, ref position, .. }) => {
                match status {
                    FingerStatus::Down if self.rect.includes(*position) => {
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
            Event::Gesture(GestureEvent::Tap(ref center)) if self.rect.includes(*center) => {
                bus.push_back(self.event.clone());
                true
            },
            _ => false,
        }
    }

    fn render(&self, fb: &mut Framebuffer, _fonts: &mut Fonts) {
        let dpi = CURRENT_DEVICE.dpi;
        let thickness = scale_by_dpi(THICKNESS_MEDIUM, dpi) as u16;
        let button_radius = self.rect.height() as i32 / 2;

        let scheme = if self.active {
            TEXT_INVERTED_HARD
        } else {
            TEXT_NORMAL
        };

        let pixmap = ICONS_PIXMAPS.get(&self.name[..]).unwrap();
        let dx = (self.rect.width() as i32 - pixmap.width as i32) / 2;
        let dy = (self.rect.height() as i32 - pixmap.height as i32) / 2;
        let pt = self.rect.min + pt!(dx, dy);

        fb.draw_rounded_rectangle_with_border(&self.rect,
                                              &CornerSpec::Uniform(button_radius),
                                              &BorderSpec { thickness: thickness as u16,
                                                            color: scheme[1] },
                                              &scheme[0]);

        fb.draw_blended_pixmap(pixmap, &pt, scheme[1]);
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
