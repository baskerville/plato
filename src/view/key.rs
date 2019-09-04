use crate::device::CURRENT_DEVICE;
use crate::framebuffer::{Framebuffer, UpdateMode};
use crate::input::{DeviceEvent, FingerStatus};
use crate::gesture::GestureEvent;
use super::{View, Event, KeyboardEvent, Hub, Bus, TextKind};
use super::BORDER_RADIUS_LARGE;
use super::icon::ICONS_PIXMAPS;
use crate::color::{TEXT_NORMAL, TEXT_INVERTED_HARD, KEYBOARD_BG};
use crate::font::{Fonts, font_from_style, KBD_CHAR, KBD_LABEL};
use crate::geom::{Rectangle, LinearDir, CornerSpec, halves};
use crate::app::Context;
use crate::unit::scale_by_dpi;

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum KeyKind {
    Output(char),
    Delete(LinearDir),
    Move(LinearDir),
    Shift,
    Return,
    Combine,
    Alternate,
}

#[derive(Clone, Debug)]
pub enum KeyLabel {
    Char(char),
    Text(&'static str),
    Icon(&'static str),
}

impl KeyKind {
    pub fn label(self) -> KeyLabel {
        match self {
            KeyKind::Output(ch) => KeyLabel::Char(ch),
            KeyKind::Delete(dir) => {
                match dir {
                    LinearDir::Forward => KeyLabel::Icon("delete-forward"),
                    LinearDir::Backward => KeyLabel::Icon("delete-backward"),
                }
            },
            KeyKind::Move(dir) => {
                match dir {
                    LinearDir::Forward => KeyLabel::Icon("move-forward"),
                    LinearDir::Backward => KeyLabel::Icon("move-backward"),
                }
            },
            KeyKind::Shift => KeyLabel::Text("SHIFT"),
            KeyKind::Return => KeyLabel::Text("RETURN"),
            KeyKind::Combine => KeyLabel::Text("CMB"),
            KeyKind::Alternate => KeyLabel::Text("ALT"),
        }
    }
}

pub struct Key {
    rect: Rectangle,
    children: Vec<Box<dyn View>>,
    kind: KeyKind,
    padding: u32,
    pressure: u8,
    active: bool,
}

impl Key {
    pub fn new(rect: Rectangle, kind: KeyKind, padding: u32) -> Key {
        Key {
            rect,
            children: vec![],
            kind,
            padding,
            pressure: 0,
            active: false,
        }
    }

    pub fn update(&mut self, kind: KeyKind, hub: &Hub) {
        self.kind = kind;
        hub.send(Event::Render(self.rect, UpdateMode::Gui)).unwrap();
    }

    pub fn release(&mut self, hub: &Hub) {
        self.pressure = 0;
        hub.send(Event::Render(self.rect, UpdateMode::Gui)).unwrap();
    }

    pub fn lock(&mut self) {
        self.pressure = 2;
    }
}

impl View for Key {
    fn handle_event(&mut self, evt: &Event, hub: &Hub, bus: &mut Bus, _context: &mut Context) -> bool {
        match *evt {
            Event::Device(DeviceEvent::Finger { status, position, .. }) => {
                match status {
                    FingerStatus::Down if self.rect.includes(position) => {
                        self.active = true;
                        hub.send(Event::RenderNoWait(self.rect, UpdateMode::Fast)).unwrap();
                        true
                    },
                    FingerStatus::Up if self.active => {
                        self.active = false;
                        hub.send(Event::RenderNoWait(self.rect, UpdateMode::Gui)).unwrap();
                        true
                    },
                    _ => false,
                }
            },
            Event::Gesture(GestureEvent::Tap(center)) if self.rect.includes(center) => {
                match self.kind {
                    KeyKind::Shift |
                    KeyKind::Alternate |
                    KeyKind::Combine => {
                        if self.kind == KeyKind::Combine {
                            self.pressure = (self.pressure + 2) % 4;
                        } else {
                            self.pressure = (self.pressure + 1) % 3;
                        }
                        hub.send(Event::RenderNoWait(self.rect, UpdateMode::Gui)).unwrap();
                    },
                    _ => (),
                }
                bus.push_back(Event::Key(self.kind));
                true
            },
            Event::Gesture(GestureEvent::HoldFingerShort(center, ..)) if self.rect.includes(center) => {
                match self.kind {
                    KeyKind::Delete(dir) => hub.send(Event::Keyboard(KeyboardEvent::Delete { target: TextKind::Word, dir })).unwrap(),
                    KeyKind::Move(dir) => hub.send(Event::Keyboard(KeyboardEvent::Move { target: TextKind::Word, dir })).unwrap(),
                    _ => (),
                };
                true
            },
            _ => false,
        }
    }

    fn render(&self, fb: &mut dyn Framebuffer, _rect: Rectangle, fonts: &mut Fonts) {
        let dpi = CURRENT_DEVICE.dpi;
        fb.draw_rectangle(&self.rect, KEYBOARD_BG);
        let scheme: [u8; 3] = if self.active ^ (self.pressure == 2) {
            TEXT_INVERTED_HARD
        } else {
            TEXT_NORMAL
        };

        let border_radius = scale_by_dpi(BORDER_RADIUS_LARGE, dpi) as i32;
        let (small_half_padding, big_half_padding) = halves(self.padding as i32);
        let key_rect = rect![self.rect.min + big_half_padding, self.rect.max - small_half_padding];
        fb.draw_rounded_rectangle(&key_rect, &CornerSpec::Uniform(border_radius), scheme[0]);

        match self.kind.label() {
            KeyLabel::Char(ch) => {
                let font = font_from_style(fonts, &KBD_CHAR, dpi);
                let plan = font.plan(&ch.to_string(), None, None);
                let dx = (key_rect.width() - plan.width) as i32 / 2;
                let dy = (key_rect.height() - font.x_heights.0) as i32 / 2;
                let pt = pt!(key_rect.min.x + dx, key_rect.max.y - dy);
                font.render(fb, scheme[1], &plan, pt);
            },
            KeyLabel::Text(label) => {
                let font = font_from_style(fonts, &KBD_LABEL, dpi);
                let mut plan = font.plan(label, None, None);
                let letter_spacing = scale_by_dpi(4.0, dpi) as u32;
                plan.space_out(letter_spacing);
                let dx = (key_rect.width() - plan.width) as i32 / 2;
                let dy = (key_rect.height() - font.x_heights.1) as i32 / 2;
                let pt = pt!(key_rect.min.x + dx, key_rect.max.y - dy);
                font.render(fb, scheme[1], &plan, pt);
            },
            KeyLabel::Icon(name) => {
                let pixmap = ICONS_PIXMAPS.get(name).unwrap();
                let dx = (key_rect.width() as i32 - pixmap.width as i32) / 2;
                let dy = (key_rect.height() as i32 - pixmap.height as i32) / 2;
                let pt = key_rect.min + pt!(dx, dy);
                fb.draw_blended_pixmap(pixmap, pt, scheme[1]);
            }
        }
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
