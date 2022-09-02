use crate::device::CURRENT_DEVICE;
use crate::framebuffer::{Framebuffer, UpdateMode};
use crate::input::{DeviceEvent, FingerStatus};
use crate::gesture::GestureEvent;
use super::{View, Event, ViewId, KeyboardEvent, Hub, Bus, Id, ID_FEEDER, RenderQueue, RenderData, TextKind};
use super::BORDER_RADIUS_LARGE;
use super::icon::ICONS_PIXMAPS;
use crate::color::{TEXT_NORMAL, TEXT_INVERTED_HARD, KEYBOARD_BG};
use crate::font::{Fonts, font_from_style, KBD_CHAR, KBD_LABEL};
use crate::geom::{Rectangle, LinearDir, CornerSpec};
use crate::context::Context;
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

use std::fmt;
use serde::{Deserializer, Deserialize};
use serde::de::{self, Visitor};

impl<'de> Deserialize<'de> for KeyKind {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct FieldVisitor;

        const FIELDS: &[&str] = &[
            "Shift", "Sft",
            "Return", "Ret",
            "Alternate", "Alt",
            "Combine", "Cmb",
            "MoveFwd", "MoveF", "MF",
            "MoveBwd", "MoveB", "MB",
            "DelFwd", "DelF", "DF",
            "DelBwd", "DelB", "DB",
            "Space", "Spc",
        ];

        impl<'de> Visitor<'de> for FieldVisitor {
            type Value = KeyKind;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a key name or a single character")
            }

            fn visit_str<E>(self, value: &str) -> Result<KeyKind, E>
            where
                E: de::Error,
            {
                match value {
                    "Shift" | "Sft" => Ok(KeyKind::Shift),
                    "Return" | "Ret" => Ok(KeyKind::Return),
                    "Alternate" | "Alt" => Ok(KeyKind::Alternate),
                    "Combine" | "Cmb" => Ok(KeyKind::Combine),
                    "MoveFwd" | "MoveF" | "MF" => Ok(KeyKind::Move(LinearDir::Forward)),
                    "MoveBwd" | "MoveB" | "MB" => Ok(KeyKind::Move(LinearDir::Backward)),
                    "DelFwd" | "DelF" | "DF" => Ok(KeyKind::Delete(LinearDir::Forward)),
                    "DelBwd" | "DelB" | "DB" => Ok(KeyKind::Delete(LinearDir::Backward)),
                    "Space" | "Spc" => Ok(KeyKind::Output(' ')),
                    _ => {
                        if value.chars().count() != 1 {
                            return Err(serde::de::Error::unknown_field(value, FIELDS));
                        }
                        value.chars().next().map(KeyKind::Output)
                             .ok_or_else(|| serde::de::Error::custom("impossible"))
                    },
                }
            }
        }

        deserializer.deserialize_identifier(FieldVisitor)
    }
}


#[derive(Clone, Debug)]
pub enum KeyLabel {
    Char(char),
    Text(&'static str),
    Icon(&'static str),
}

impl KeyKind {
    pub fn is_variable_output(&self) -> bool {
        matches!(self, KeyKind::Output(ch) if *ch != ' ')
    }

    pub fn label(self, ratio: f32) -> KeyLabel {
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
                    LinearDir::Forward => KeyLabel::Icon(if ratio <= 1.0 { "move-forward-short" } else { "move-forward" }),
                    LinearDir::Backward => KeyLabel::Icon(if ratio <= 1.0 { "move-backward-short" } else { "move-backward"}),
                }
            },
            KeyKind::Shift => if ratio < 2.0 { KeyLabel::Icon("shift") } else { KeyLabel::Text("SHIFT") },
            KeyKind::Return => if ratio < 2.0 { KeyLabel::Icon("return") } else { KeyLabel::Text("RETURN") },
            KeyKind::Combine => if ratio <= 1.0 { KeyLabel::Icon("combine") } else { KeyLabel::Text("CMB") },
            KeyKind::Alternate => if ratio <= 1.0 { KeyLabel::Icon("alternate") } else { KeyLabel::Text("ALT") },
        }
    }
}

pub struct Key {
    id: Id,
    rect: Rectangle,
    children: Vec<Box<dyn View>>,
    kind: KeyKind,
    pressure: u8,
    active: bool,
}

impl Key {
    pub fn new(rect: Rectangle, kind: KeyKind) -> Key {
        Key {
            id: ID_FEEDER.next(),
            rect,
            children: Vec::new(),
            kind,
            pressure: 0,
            active: false,
        }
    }

    pub fn kind(&self) -> &KeyKind {
        &self.kind
    }

    pub fn update(&mut self, kind: KeyKind, rq: &mut RenderQueue) {
        self.kind = kind;
        rq.add(RenderData::new(self.id, self.rect, UpdateMode::Gui));
    }

    pub fn release(&mut self, rq: &mut RenderQueue) {
        self.pressure = 0;
        rq.add(RenderData::new(self.id, self.rect, UpdateMode::Gui));
    }

    pub fn lock(&mut self) {
        self.pressure = 2;
    }
}

impl View for Key {
    fn handle_event(&mut self, evt: &Event, hub: &Hub, bus: &mut Bus, rq: &mut RenderQueue, _context: &mut Context) -> bool {
        match *evt {
            Event::Device(DeviceEvent::Finger { status, position, .. }) => {
                match status {
                    FingerStatus::Down if self.rect.includes(position) => {
                        self.active = true;
                        rq.add(RenderData::no_wait(self.id, self.rect, UpdateMode::Fast));
                        true
                    },
                    FingerStatus::Up if self.active => {
                        self.active = false;
                        rq.add(RenderData::no_wait(self.id, self.rect, UpdateMode::Gui));
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
                        rq.add(RenderData::no_wait(self.id, self.rect, UpdateMode::Gui));
                    },
                    _ => (),
                }
                bus.push_back(Event::Key(self.kind));
                true
            },
            Event::Gesture(GestureEvent::HoldFingerShort(center, ..)) if self.rect.includes(center) => {
                match self.kind {
                    KeyKind::Delete(dir) => { hub.send(Event::Keyboard(KeyboardEvent::Delete { target: TextKind::Word, dir })).ok(); },
                    KeyKind::Move(dir) => { hub.send(Event::Keyboard(KeyboardEvent::Move { target: TextKind::Word, dir })).ok(); },
                    KeyKind::Output(' ') => { hub.send(Event::ToggleNear(ViewId::KeyboardLayoutMenu, self.rect)).ok(); },
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
        fb.draw_rounded_rectangle(&self.rect, &CornerSpec::Uniform(border_radius), scheme[0]);
        let ratio = self.rect.width() as f32 / self.rect.height() as f32;

        match self.kind.label(ratio) {
            KeyLabel::Char(ch) => {
                let font = font_from_style(fonts, &KBD_CHAR, dpi);
                let plan = font.plan(&ch.to_string(), None, None);
                let dx = (self.rect.width() as i32 - plan.width) / 2;
                let dy = (self.rect.height() - font.x_heights.0) as i32 / 2;
                let pt = pt!(self.rect.min.x + dx, self.rect.max.y - dy);
                font.render(fb, scheme[1], &plan, pt);
            },
            KeyLabel::Text(label) => {
                let font = font_from_style(fonts, &KBD_LABEL, dpi);
                let mut plan = font.plan(label, None, None);
                let letter_spacing = scale_by_dpi(4.0, dpi) as i32;
                plan.space_out(letter_spacing);
                let dx = (self.rect.width() as i32 - plan.width) / 2;
                let dy = (self.rect.height() - font.x_heights.1) as i32 / 2;
                let pt = pt!(self.rect.min.x + dx, self.rect.max.y - dy);
                font.render(fb, scheme[1], &plan, pt);
            },
            KeyLabel::Icon(name) => {
                let pixmap = ICONS_PIXMAPS.get(name).unwrap();
                let dx = (self.rect.width() as i32 - pixmap.width as i32) / 2;
                let dy = (self.rect.height() as i32 - pixmap.height as i32) / 2;
                let pt = self.rect.min + pt!(dx, dy);
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

    fn id(&self) -> Id {
        self.id
    }
}
