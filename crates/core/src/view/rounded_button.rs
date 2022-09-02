use crate::device::CURRENT_DEVICE;
use crate::framebuffer::{Framebuffer, UpdateMode};
use super::{View, Event, Hub, Bus, Id, ID_FEEDER, RenderQueue, RenderData};
use super::THICKNESS_MEDIUM;
use super::icon::ICONS_PIXMAPS;
use crate::gesture::GestureEvent;
use crate::input::{DeviceEvent, FingerStatus};
use crate::unit::scale_by_dpi;
use crate::font::Fonts;
use crate::context::Context;
use crate::geom::{Rectangle, CornerSpec, BorderSpec};
use crate::color::{TEXT_NORMAL, TEXT_INVERTED_HARD};

pub struct RoundedButton {
    id: Id,
    rect: Rectangle,
    children: Vec<Box<dyn View>>,
    name: String,
    event: Event,
    active: bool,
}

impl RoundedButton {
    pub fn new(name: &str, rect: Rectangle, event: Event) -> RoundedButton {
        RoundedButton {
            id: ID_FEEDER.next(),
            rect,
            children: Vec::new(),
            name: name.to_string(),
            event,
            active: false,
        }
    }
}

impl View for RoundedButton {
    fn handle_event(&mut self, evt: &Event, _hub: &Hub, bus: &mut Bus, rq: &mut RenderQueue, _context: &mut Context) -> bool {
        match *evt {
            Event::Device(DeviceEvent::Finger { status, position, .. }) => {
                match status {
                    FingerStatus::Down if self.rect.includes(position) => {
                        self.active = true;
                        rq.add(RenderData::new(self.id, self.rect, UpdateMode::Fast));
                        true
                    },
                    FingerStatus::Up if self.active => {
                        self.active = false;
                        rq.add(RenderData::new(self.id, self.rect, UpdateMode::Gui));
                        true
                    },
                    _ => false,
                }
            },
            Event::Gesture(GestureEvent::Tap(center)) if self.rect.includes(center) => {
                bus.push_back(self.event.clone());
                true
            },
            _ => false,
        }
    }

    fn render(&self, fb: &mut dyn Framebuffer, _rect: Rectangle, _fonts: &mut Fonts) {
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

        fb.draw_blended_pixmap(pixmap, pt, scheme[1]);
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
