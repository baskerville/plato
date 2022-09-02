use crate::device::CURRENT_DEVICE;
use crate::geom::{Rectangle, CornerSpec, CycleDir};
use crate::font::{Fonts, font_from_style, NORMAL_STYLE};
use super::{View, Event, Hub, Bus, Id, ID_FEEDER, RenderQueue, RenderData};
use super::BORDER_RADIUS_MEDIUM;
use crate::framebuffer::{Framebuffer, UpdateMode};
use crate::input::{DeviceEvent, FingerStatus};
use crate::gesture::GestureEvent;
use crate::color::{TEXT_NORMAL, TEXT_INVERTED_HARD};
use crate::unit::scale_by_dpi;
use crate::context::Context;

pub struct Preset {
    id: Id,
    rect: Rectangle,
    children: Vec<Box<dyn View>>,
    kind: PresetKind,
    active: bool,
}

pub enum PresetKind {
    Normal(String, usize),
    Page(CycleDir),
}

impl Preset {
    pub fn new(rect: Rectangle, kind: PresetKind) -> Preset {
        Preset {
            id: ID_FEEDER.next(),
            rect,
            children: Vec::new(),
            kind,
            active: false,
        }
    }
}

impl View for Preset {
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
                match self.kind {
                    PresetKind::Normal(_, index) => bus.push_back(Event::LoadPreset(index)),
                    PresetKind::Page(dir) => bus.push_back(Event::Page(dir)),
                }
                true
            },
            Event::Gesture(GestureEvent::HoldFingerShort(center, ..)) if self.rect.includes(center) => {
                if let PresetKind::Normal(_, index) = self.kind {
                    bus.push_back(Event::TogglePresetMenu(self.rect, index)); 
                }
                true
            },
            _ => false,
        }
    }

    fn render(&self, fb: &mut dyn Framebuffer, _rect: Rectangle, fonts: &mut Fonts) {
        let dpi = CURRENT_DEVICE.dpi;

        let (scheme, border_radius) = if self.active {
            (TEXT_INVERTED_HARD, scale_by_dpi(BORDER_RADIUS_MEDIUM, dpi) as i32)
        } else {
            (TEXT_NORMAL, 0)
        };

        fb.draw_rounded_rectangle(&self.rect, &CornerSpec::Uniform(border_radius), scheme[0]);

        let font = font_from_style(fonts, &NORMAL_STYLE, dpi);
        let x_height = font.x_heights.0 as i32;
        let padding = font.em() as i32;
        let max_width = self.rect.width() as i32 - padding;

        let name = match self.kind {
            PresetKind::Normal(ref text, _) => text,
            _ => "…",
        };

        let plan = font.plan(name, Some(max_width), None);

        let dx = (self.rect.width() as i32 - plan.width) / 2;
        let dy = (self.rect.height() as i32 - x_height) / 2;
        let pt = pt!(self.rect.min.x + dx, self.rect.max.y - dy);

        font.render(fb, scheme[1], &plan, pt);
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
