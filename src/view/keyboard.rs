use fxhash::FxHashMap;
use lazy_static::lazy_static;
use serde::Deserialize;
use crate::device::CURRENT_DEVICE;
use crate::framebuffer::{Framebuffer, UpdateMode};
use crate::gesture::GestureEvent;
use crate::input::DeviceEvent;
use super::{View, Event, Hub, Bus, Id, ID_FEEDER, RenderQueue, RenderData, KeyboardEvent, EntryId, TextKind};
use super::key::{Key, KeyKind};
use super::BIG_BAR_HEIGHT;
use crate::color::KEYBOARD_BG;
use crate::font::Fonts;
use crate::app::Context;
use crate::geom::Rectangle;
use crate::unit::scale_by_dpi;

const PADDING_RATIO: f32 = 0.06;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Layout {
    pub name: String,
    pub outputs: [Vec<Vec<char>>; 4],
    pub keys: Vec<Vec<KeyKind>>,
    pub widths: Vec<Vec<f32>>,
}

#[derive(Default, Debug)]
pub struct State {
    shift: u8,
    alternate: u8,
    combine: bool,
}

pub struct Keyboard {
    id: Id,
    rect: Rectangle,
    children: Vec<Box<dyn View>>,
    layout: Layout,
    state: State,
    combine_buffer: String,
}

impl Keyboard {
    pub fn new(rect: &mut Rectangle, number: bool, context: &mut Context) -> Keyboard {
        let id = ID_FEEDER.next();
        let mut children = Vec::new();
        let dpi = CURRENT_DEVICE.dpi;

        let layout = context.keyboard_layouts[&context.settings.keyboard_layout].clone();

        let mut state = State::default();

        if number {
            state.alternate = 2;
        }

        let mut level = 0;

        if state.shift > 0 {
            level += 1;
        }

        if state.alternate > 0 {
            level += 2;
        }

        let max_width = layout.widths.iter().map(|row| (row.len() + 1) as f32 * PADDING_RATIO + row.iter().sum::<f32>())
                              .max_by(|a, b| a.partial_cmp(&b).expect("Found NaNs"))
                              .expect("Missing row widths");

        let kh_1 = (rect.width() as f32) / max_width;
        let rows_count = layout.keys.len();
        let kh_2 = (rect.height() as f32) / (rows_count as f32 + PADDING_RATIO * (rows_count + 1) as f32);
        let key_height = kh_1.min(kh_2);
        let padding = PADDING_RATIO * key_height;

        let rows_height = key_height * rows_count as f32 + padding * (rows_count + 1) as f32;
        let big_height = scale_by_dpi(BIG_BAR_HEIGHT, dpi) as i32;
        let height_gap = (rect.height() - rows_height.round() as u32) / big_height as u32;
        rect.min.y += height_gap as i32 * big_height;
        context.kb_rect = *rect;

        let start_y = rect.min.y as f32 + padding + (rect.height() as f32 - rows_height) / 2.0;

        for (i, row) in layout.keys.iter().enumerate() {
            let y = start_y + i as f32 * (padding + key_height);
            let row_width = (layout.widths[i].len() + 1) as f32 * padding + layout.widths[i].iter().sum::<f32>() * key_height;
            let start_x = rect.min.x as f32 + padding + (rect.width() as f32 - row_width) / 2.0;
            let mut dx = 0.0;
            let mut dj = 0;

            for (j, kind) in row.iter().enumerate() {
                let key_width = layout.widths[i][j] * key_height;
                let x = start_x + dx;
                dx += key_width + padding;
                let key_rect = rect![x.round() as i32,
                                     y.round() as i32,
                                     (x + key_width).round() as i32,
                                     (y + key_height).round() as i32];
                let kind = match kind {
                    KeyKind::Output(c) if *c != ' ' => KeyKind::Output(layout.outputs[level][i][j-dj]),
                    _ => { dj = j + 1; *kind },
                };
                let mut key = Key::new(key_rect, kind);
                if number && kind == KeyKind::Alternate {
                    key.lock();
                }
                children.push(Box::new(key) as Box<dyn View>);
            }
        }

        Keyboard {
            id,
            rect: *rect,
            children,
            layout,
            state,
            combine_buffer: String::new(),
        }
    }

    fn update(&mut self, rq: &mut RenderQueue) {
        let mut level = 0;

        if self.state.shift > 0 {
            level += 1;
        }

        if self.state.alternate > 0 {
            level += 2;
        }

        let mut index = 0;

        for (i, row) in self.layout.keys.iter().enumerate() {
            let mut dj = 0;

            for (j, kind) in row.iter().enumerate() {
                if kind.is_variable_output() {
                    if let Some(child) = self.children[index].downcast_mut::<Key>() {
                        let ch = self.layout.outputs[level][i][j-dj];
                        child.update(KeyKind::Output(ch), rq);
                    }
                } else {
                    dj = j + 1;
                }
                index += 1;
            }
        }
    }

    fn release_modifiers(&mut self, rq: &mut RenderQueue) {
        if self.state.shift != 1 && self.state.alternate != 1 {
            return;
        }

        if self.state.shift == 1 {
            self.state.shift = 0;
            for child in self.children_mut() {
                if let Some(key) = child.downcast_mut::<Key>() {
                    if *key.kind() == KeyKind::Shift {
                        key.release(rq);
                        break;
                    }
                }
            }
        }

        if self.state.alternate == 1 {
            self.state.alternate = 0;
            for child in self.children_mut() {
                if let Some(key) = child.downcast_mut::<Key>() {
                    if *key.kind() == KeyKind::Alternate {
                        key.release(rq);
                        break;
                    }
                }
            }
        }

        self.update(rq);
    }

    fn release_combine(&mut self, rq: &mut RenderQueue) {
        self.state.combine = false;
        self.combine_buffer.clear();
        for child in self.children_mut() {
            if let Some(key) = child.downcast_mut::<Key>() {
                if *key.kind() == KeyKind::Combine {
                    key.release(rq);
                    break;
                }
            }
        }
    }

}

impl View for Keyboard {
    fn handle_event(&mut self, evt: &Event, hub: &Hub, _bus: &mut Bus, rq: &mut RenderQueue, context: &mut Context) -> bool {
        match *evt {
            Event::Key(k) => {
                match k {
                    KeyKind::Output(ch) => {
                        if self.state.combine {
                            self.combine_buffer.push(ch);
                            hub.send(Event::Keyboard(KeyboardEvent::Partial(ch))).ok();
                            if self.combine_buffer.len() > 1 {
                                if let Some(&ch) = DEFAULT_COMBINATIONS.get(&self.combine_buffer[..]) {
                                    hub.send(Event::Keyboard(KeyboardEvent::Append(ch))).ok();
                                }
                                self.release_combine(rq);
                            }
                        } else {
                            hub.send(Event::Keyboard(KeyboardEvent::Append(ch))).ok();
                        }
                        if ch != ' ' {
                            self.release_modifiers(rq);
                        }
                    }
                    KeyKind::Shift => {
                        self.state.shift = (self.state.shift + 1) % 3;
                        if self.state.shift != 2 {
                            self.update(rq);
                        }
                    },
                    KeyKind::Alternate => {
                        self.state.alternate = (self.state.alternate + 1) % 3;
                        if self.state.alternate != 2 {
                            self.update(rq);
                        }
                    },
                    KeyKind::Delete(dir) => { hub.send(Event::Keyboard(KeyboardEvent::Delete { target: TextKind::Char, dir })).ok(); },
                    KeyKind::Move(dir) => { hub.send(Event::Keyboard(KeyboardEvent::Move { target: TextKind::Char, dir })).ok(); },
                    KeyKind::Combine => self.state.combine = !self.state.combine,
                    KeyKind::Return => {
                        self.release_combine(rq);
                        hub.send(Event::Keyboard(KeyboardEvent::Submit)).ok();
                    }
                };
                true
            },
            Event::Select(EntryId::SetKeyboardLayout(ref name)) => {
                if *name != context.settings.keyboard_layout {
                    context.settings.keyboard_layout = name.to_string();
                    // FIXME: the keyboard's height might change, in which case,
                    // we shall notify the root view.
                    *self = Keyboard::new(&mut self.rect, self.state.alternate == 2, context);
                    rq.add(RenderData::new(self.id, self.rect, UpdateMode::Gui));
                }
                true
            },
            Event::Gesture(GestureEvent::Tap(center)) |
            Event::Gesture(GestureEvent::HoldFingerShort(center, ..)) if self.rect.includes(center) => true,
            Event::Gesture(GestureEvent::Swipe { start, .. }) if self.rect.includes(start) => true,
            Event::Device(DeviceEvent::Finger { position, .. }) if self.rect.includes(position) => true,
            _ => false,
        }
    }

    fn might_skip(&self, evt: &Event) -> bool {
        !matches!(*evt,
                  Event::Key(..) |
                  Event::Gesture(..) |
                  Event::Device(DeviceEvent::Finger { .. }) |
                  Event::Select(..))
    }

    fn render(&self, fb: &mut dyn Framebuffer, rect: Rectangle, _fonts: &mut Fonts) {
        for child in &self.children {
            if *child.rect() == rect {
                return;
            }
        }

        if let Some(region) = rect.intersection(&self.rect) {
            fb.draw_rectangle(&region, KEYBOARD_BG);
        }
    }

    fn render_rect(&self, rect: &Rectangle) -> Rectangle {
        rect.intersection(&self.rect)
            .unwrap_or(self.rect)
    }

    fn resize(&mut self, mut rect: Rectangle, hub: &Hub, rq: &mut RenderQueue, context: &mut Context) {
        let dpi = CURRENT_DEVICE.dpi;
        let max_width = self.layout.widths.iter().map(|row| (row.len() + 1) as f32 * PADDING_RATIO + row.iter().sum::<f32>())
                            .max_by(|a, b| a.partial_cmp(b).expect("Found NaNs"))
                            .expect("Missing row widths");

        let kh_1 = (rect.width() as f32) / max_width;
        let rows_count = self.layout.keys.len();
        let kh_2 = (rect.height() as f32) / (rows_count as f32 + PADDING_RATIO * (rows_count + 1) as f32);
        let key_height = kh_1.min(kh_2);
        let padding = PADDING_RATIO * key_height;

        let rows_height = key_height * rows_count as f32 + padding * (rows_count + 1) as f32;
        let big_height = scale_by_dpi(BIG_BAR_HEIGHT, dpi) as i32;
        let height_gap = (rect.height() - rows_height.round() as u32) / big_height as u32;
        rect.min.y += height_gap as i32 * big_height;

        let start_y = rect.min.y as f32 + padding + (rect.height() as f32 - rows_height) / 2.0;
        let mut index = 0;

        for (i, row) in self.layout.keys.iter().enumerate() {
            let y = start_y + i as f32 * (padding + key_height);
            let row_width = (self.layout.widths[i].len() + 1) as f32 * padding + self.layout.widths[i].iter().sum::<f32>() * key_height;
            let start_x = rect.min.x as f32 + padding + (rect.width() as f32 - row_width) / 2.0;
            let mut dx = 0.0;

            for j in 0..row.len() {
                let key_width = self.layout.widths[i][j] * key_height;
                let x = start_x + dx;
                dx += key_width + padding;
                let key_rect = rect![x.round() as i32,
                                     y.round() as i32,
                                     (x + key_width).round() as i32,
                                     (y + key_height).round() as i32];
                self.children[index].resize(key_rect, hub, rq, context);
                index += 1;
            }
        }

        self.rect = rect;
        context.kb_rect = rect;
    }

    fn is_background(&self) -> bool {
        true
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

lazy_static! {
    // Most of the combination sequences come from X.org.
    // The chosen characters come from the layout described by
    // Robert Bringhurst in *The Elements of Typographic Style*,
    // version 3.1, p. 92.
    pub static ref DEFAULT_COMBINATIONS: FxHashMap<&'static str, char> = {
        let mut m = FxHashMap::default();
        m.insert("oe", 'œ');
        m.insert("Oe", 'Œ');
        m.insert("ae", 'æ');
        m.insert("AE", 'Æ');
        m.insert("c,", 'ç');
        m.insert("C,", 'Ç');
        m.insert("a;", 'ą');
        m.insert("e;", 'ę');
        m.insert("A;", 'Ą');
        m.insert("E;", 'Ę');
        m.insert("a~", 'ã');
        m.insert("o~", 'õ');
        m.insert("n~", 'ñ');
        m.insert("A~", 'Ã');
        m.insert("O~", 'Õ');
        m.insert("N~", 'Ñ');
        m.insert("a'", 'á');
        m.insert("e'", 'é');
        m.insert("i'", 'í');
        m.insert("o'", 'ó');
        m.insert("u'", 'ú');
        m.insert("y'", 'ý');
        m.insert("z'", 'ź');
        m.insert("s'", 'ś');
        m.insert("c'", 'ć');
        m.insert("n'", 'ń');
        m.insert("A'", 'Á');
        m.insert("E'", 'É');
        m.insert("I'", 'Í');
        m.insert("O'", 'Ó');
        m.insert("U'", 'Ú');
        m.insert("Y'", 'Ý');
        m.insert("Z'", 'Ź');
        m.insert("S'", 'Ś');
        m.insert("C'", 'Ć');
        m.insert("N'", 'Ń');
        m.insert("a`", 'à');
        m.insert("e`", 'è');
        m.insert("i`", 'ì');
        m.insert("o`", 'ò');
        m.insert("u`", 'ù');
        m.insert("A`", 'À');
        m.insert("E`", 'È');
        m.insert("I`", 'Ì');
        m.insert("O`", 'Ò');
        m.insert("U`", 'Ù');
        m.insert("a^", 'â');
        m.insert("e^", 'ê');
        m.insert("i^", 'î');
        m.insert("o^", 'ô');
        m.insert("u^", 'û');
        m.insert("w^", 'ŵ');
        m.insert("y^", 'ŷ');
        m.insert("A^", 'Â');
        m.insert("E^", 'Ê');
        m.insert("I^", 'Î');
        m.insert("O^", 'Ô');
        m.insert("U^", 'Û');
        m.insert("W^", 'Ŵ');
        m.insert("Y^", 'Ŷ');
        m.insert("a:", 'ä');
        m.insert("e:", 'ë');
        m.insert("i:", 'ï');
        m.insert("o:", 'ö');
        m.insert("u:", 'ü');
        m.insert("y:", 'ÿ');
        m.insert("A:", 'Ä');
        m.insert("E:", 'Ë');
        m.insert("I:", 'Ï');
        m.insert("O:", 'Ö');
        m.insert("U:", 'Ü');
        m.insert("Y:", 'Ÿ');
        m.insert("u\"", 'ű');
        m.insert("o\"", 'ő');
        m.insert("U\"", 'Ű');
        m.insert("O\"", 'Ő');
        m.insert("z.", 'ż');
        m.insert("Z.", 'Ż');
        m.insert("th", 'þ');
        m.insert("Th", 'Þ');
        m.insert("ao", 'å');
        m.insert("Ao", 'Å');
        m.insert("l/", 'ł');
        m.insert("d/", 'đ');
        m.insert("o/", 'ø');
        m.insert("L/", 'Ł');
        m.insert("D/", 'Đ');
        m.insert("O/", 'Ø');
        m.insert("mu", 'µ');
        m.insert("l-", '£');
        m.insert("pp", '¶');
        m.insert("so", '§');
        m.insert("|-", '†');
        m.insert("|=", '‡');
        m.insert("ss", 'ß');
        m.insert("Ss", 'ẞ');
        m.insert("o_", 'º');
        m.insert("a_", 'ª');
        m.insert("oo", '°');
        m.insert("!!", '¡');
        m.insert("??", '¿');
        m.insert(".-", '·');
        m.insert(".=", '•');
        m.insert(".>", '›');
        m.insert(".<", '‹');
        m.insert("'1", '′');
        m.insert("'2", '″');
        m.insert("[[", '⟦');
        m.insert("]]", '⟧');
        m.insert("+-", '±');
        m.insert("-:", '÷');
        m.insert("<=", '≤');
        m.insert(">=", '≥');
        m.insert("=/", '≠');
        m.insert("-,", '¬');
        m.insert("~~", '≈');
        m.insert("<<", '«');
        m.insert(">>", '»');
        m.insert("12", '½');
        m.insert("13", '⅓');
        m.insert("23", '⅔');
        m.insert("14", '¼');
        m.insert("34", '¾');
        m.insert("15", '⅕');
        m.insert("25", '⅖');
        m.insert("35", '⅗');
        m.insert("45", '⅘');
        m.insert("16", '⅙');
        m.insert("56", '⅚');
        m.insert("18", '⅛');
        m.insert("38", '⅜');
        m.insert("58", '⅝');
        m.insert("78", '⅞');
        m.insert("#f", '♭');
        m.insert("#n", '♮');
        m.insert("#s", '♯');
        m.insert("%o", '‰');
        m.insert("e=", '€');
        m.insert("or", '®');
        m.insert("oc", '©');
        m.insert("op", '℗');
        m.insert("tm", '™');
        m
    };
}
