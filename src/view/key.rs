use std::sync::mpsc::Sender;
use device::CURRENT_DEVICE;
use framebuffer::{Framebuffer, UpdateMode};
use gesture::GestureEvent;
use input::{DeviceEvent, FingerStatus};
use view::{View, Event, ChildEvent, KeyboardEvent, InputKind};
use view::icon::ICONS_BITMAPS;
use color::{TEXT_NORMAL, TEXT_BUMP_BIG, TEXT_INVERTED, KEYBOARD_BG};
use font::{Fonts, font_from_style, KBD_CHAR, KBD_LABEL};
use geom::{Rectangle, LinearDir, CornerSpec, halves};
use unit::scale_by_dpi;

#[derive(Copy, Clone, Debug)]
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
    Char(String),
    Text(String),
    Icon(String),
}

impl KeyKind {
    pub fn label(&self) -> KeyLabel {
        match *self {
            KeyKind::Output(chr) => KeyLabel::Char([chr].iter().collect()),
            KeyKind::Delete(dir) => {
                match dir {
                    LinearDir::Forward => KeyLabel::Icon("delete-forward.svg".to_string()),
                    LinearDir::Backward => KeyLabel::Icon("delete-backward.svg".to_string()),
                }
            },
            KeyKind::Move(dir) => {
                match dir {
                    LinearDir::Forward => KeyLabel::Icon("move-forward.svg".to_string()),
                    LinearDir::Backward => KeyLabel::Icon("move-backward.svg".to_string()),
                }
            },
            KeyKind::Shift => KeyLabel::Text("SHIFT".to_string()),
            KeyKind::Return => KeyLabel::Text("RETURN".to_string()),
            KeyKind::Combine => KeyLabel::Text("CMB".to_string()),
            KeyKind::Alternate => KeyLabel::Text("ALT".to_string()),
        }
    }
}

pub struct Key {
    rect: Rectangle,
    kind: KeyKind,
    padding: u32,
    active: bool,
    pressed: bool,
}

impl Key {
    pub fn new(rect: Rectangle, kind: KeyKind, padding: u32) -> Key {
        Key {
            rect,
            kind,
            padding,
            active: false,
            pressed: false,
        }
    }

    pub fn update(&mut self, kind: KeyKind, bus: &mut Vec<ChildEvent>) {
        self.kind = kind;
        bus.push(ChildEvent::Render(self.rect, UpdateMode::Gui));
    }
}

impl View for Key {
    fn handle_event(&mut self, evt: &Event, bus: &mut Vec<ChildEvent>) -> bool {
        match *evt {
            Event::GestureEvent(GestureEvent::Relay(de)) => {
                match de {
                    DeviceEvent::Finger { status, ref position, .. } => {
                        match status {
                            FingerStatus::Down if self.rect.includes(position) => {
                                self.active = true;
                                bus.push(ChildEvent::Render(self.rect, UpdateMode::Gui));
                                false
                            },
                            FingerStatus::Up => {
                                self.active = false;
                                bus.push(ChildEvent::Render(self.rect, UpdateMode::Gui));
                                false
                            },
                            FingerStatus::Motion => {
                                let active = self.rect.includes(position);
                                if active != self.active {
                                    self.active = active;
                                    bus.push(ChildEvent::Render(self.rect, UpdateMode::Gui));
                                }
                                false
                            }
                            _ => false,
                        }
                    },
                    _ => false,
                }
            },
            Event::GestureEvent(GestureEvent::Tap { ref center, .. }) if self.rect.includes(center) => {
                bus.push(ChildEvent::Key(self.kind));
                match self.kind {
                    KeyKind::Shift | KeyKind::Alternate | KeyKind::Combine => self.pressed = !self.pressed,
                    _ => (),
                }
                true
            },
            _ => false,
        }
    }

    fn render(&self, fb: &mut Framebuffer, fonts: &mut Fonts) {
        let dpi = CURRENT_DEVICE.dpi;
        fb.draw_rectangle(&self.rect, KEYBOARD_BG);
        let scheme: [u8; 3] = if self.active ^ self.pressed {
            TEXT_INVERTED
        } else {
            match self.kind {
                KeyKind::Output(_) => TEXT_NORMAL,
                _ => TEXT_BUMP_BIG,
            }
        };
        let border_radius = scale_by_dpi(12.0, dpi).max(1.0) as i32;
        let (small_half_padding, big_half_padding) = halves(self.padding as i32);
        let key_rect = rect![self.rect.min + small_half_padding, self.rect.max - big_half_padding];
        fb.draw_rounded_rectangle(&key_rect, &CornerSpec::Uniform(border_radius), scheme[0]);
        match self.kind.label() {
            KeyLabel::Char(value) => {
                let font = font_from_style(fonts, &KBD_CHAR, dpi);
                let plan = font.plan(&value, None, None);
                let dx = (key_rect.width() - plan.width) as i32 / 2;
                let dy = (key_rect.height() - font.x_heights.0) as i32 / 2;
                let pt = pt!(key_rect.min.x + dx, key_rect.max.y - dy);
                font.render(fb, scheme[1], &plan, &pt);
            },
            KeyLabel::Text(label) => {
                let font = font_from_style(fonts, &KBD_LABEL, dpi);
                let mut plan = font.plan(&label, None, None);
                let letter_spacing = scale_by_dpi(4.0, dpi).max(1.0) as u32;
                plan.space_out(letter_spacing);
                let dx = (key_rect.width() - plan.width) as i32 / 2;
                let dy = (key_rect.height() - font.x_heights.1) as i32 / 2;
                let pt = pt!(key_rect.min.x + dx, key_rect.max.y - dy);
                font.render(fb, scheme[1], &plan, &pt);
            },
            KeyLabel::Icon(name) => {
                let bitmap = ICONS_BITMAPS.get(&name[..]).unwrap();
                let dx = (key_rect.width() as i32 - bitmap.width) / 2;
                let dy = (key_rect.height() as i32 - bitmap.height) / 2;
                let pt = key_rect.min + pt!(dx, dy);
                fb.draw_blended_bitmap(bitmap, &pt, scheme[1]);
            }
        }
    }

    fn rect(&self) -> &Rectangle {
        &self.rect
    }

    fn len(&self) -> usize {
        0
    }

    fn child(&self, _: usize) -> &View {
        self
    }

    fn child_mut(&mut self, _: usize) -> &mut View {
        self
    }
}
