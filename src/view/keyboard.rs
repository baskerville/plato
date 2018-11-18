use fnv::FnvHashMap;
use device::{CURRENT_DEVICE, BAR_SIZES};
use framebuffer::Framebuffer;
use gesture::GestureEvent;
use input::DeviceEvent;
use super::{View, Event, Hub, Bus, KeyboardEvent, TextKind};
use super::filler::Filler;
use super::key::{Key, KeyKind};
use color::KEYBOARD_BG;
use font::Fonts;
use app::Context;
use geom::{Rectangle, LinearDir, halves};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Layout {
    name: String,
    outputs: [OutputKeys<char>; 4],
}

#[derive(Debug, Copy, Clone, Serialize, Deserialize)]
pub struct OutputKeys<T: Copy> {
    row1: [T; 10],
    row2: [T; 9],
    row3: [T; 7],
}

#[derive(Default)]
pub struct State {
    shift: u8,
    alternate: u8,
    combine: bool,
}

pub struct Keyboard {
    rect: Rectangle,
    children: Vec<Box<View>>,
    layout: Layout,
    state: State,
    combine_buffer: String,
}

use device::optimal_key_setup;

impl Keyboard {
    pub fn new(rect: &mut Rectangle, layout: Layout, number: bool, context: &Context) -> Keyboard {
        let mut children = Vec::new();
        let dpi = CURRENT_DEVICE.dpi;
        let (side, padding) = optimal_key_setup(rect.width(), rect.height(), dpi);

        let (_, height) = context.display.dims;
        let &(_, big_height) = BAR_SIZES.get(&(height, dpi)).unwrap();
        let height_gap = (rect.height() - (4 * side + 5 * padding)) / big_height;
        rect.min.y += (height_gap * big_height) as i32;

        let normal_side = (side + padding) as i32;
        let (small_half_side, big_half_side) = halves(normal_side);
        let (small_medium_length, big_medium_length) = halves(3 * normal_side);
        let large_length = 2 * normal_side;
        let huge_length = 2 * large_length;

        let remaining_width = rect.width() as i32 - 11 * normal_side;
        let remaining_height = rect.height() as i32 - 4 * normal_side;
        let (small_half_remaining_width, big_half_remaining_width) = halves(remaining_width);
        let (small_half_remaining_height, big_half_remaining_height) = halves(remaining_height);

        let mut state = State::default();

        if number {
            state.alternate = 2;
        }

        let mut index = 0;

        if state.shift > 0 {
            index += 1;
        }

        if state.alternate > 0 {
            index += 2;
        }

        // Row 1

        for i in 0..10usize {
            let min_pt = rect.min + pt!(small_half_remaining_width + small_half_side + i as i32 * normal_side,
                                        small_half_remaining_height);
            let ch = layout.outputs[index].row1[i];
            let key = Key::new(rect![min_pt, min_pt + normal_side],
                               KeyKind::Output(ch), padding);
            children.push(Box::new(key) as Box<View>);
        }

        // Row 2

        let min_pt = rect.min + pt!(small_half_remaining_width, small_half_remaining_height + normal_side);
        let key = Key::new(rect![min_pt, min_pt + normal_side], KeyKind::Delete(LinearDir::Backward), padding);
        children.push(Box::new(key) as Box<View>);

        for i in 0..9usize {
            let min_pt = rect.min + pt!(small_half_remaining_width + (i + 1) as i32 * normal_side,
                                        small_half_remaining_height + normal_side);
            let ch = layout.outputs[index].row2[i];
            let key = Key::new(rect![min_pt, min_pt + normal_side],
                               KeyKind::Output(ch), padding);
            children.push(Box::new(key) as Box<View>);
        }

        let min_pt = rect.min + pt!(small_half_remaining_width + 10 * normal_side, small_half_remaining_height + normal_side);
        let key = Key::new(rect![min_pt, min_pt + normal_side], KeyKind::Delete(LinearDir::Forward), padding);
        children.push(Box::new(key) as Box<View>);

        // Row 3

        let min_pt = rect.min + pt!(small_half_remaining_width, small_half_remaining_height + 2 * normal_side);
        let key = Key::new(rect![min_pt, min_pt + pt!(large_length, normal_side)], KeyKind::Shift, padding);
        children.push(Box::new(key) as Box<View>);

        for i in 0..7usize {
            let min_pt = rect.min + pt!(small_half_remaining_width + (i + 2) as i32 * normal_side,
                                        small_half_remaining_height + 2 * normal_side);
            let ch = layout.outputs[index].row3[i];
            let key = Key::new(rect![min_pt, min_pt + normal_side],
                               KeyKind::Output(ch), padding);
            children.push(Box::new(key) as Box<View>);
        }

        let min_pt = rect.min + pt!(small_half_remaining_width + 9 * normal_side, small_half_remaining_height + 2 * normal_side);
        let key = Key::new(rect![min_pt, min_pt + pt!(large_length, normal_side)], KeyKind::Return, padding);
        children.push(Box::new(key) as Box<View>);

        // Row 4

        let min_pt = rect.min + pt!(small_half_remaining_width + small_half_side, small_half_remaining_height + 3 * normal_side);
        let key = Key::new(rect![min_pt, min_pt + pt!(small_medium_length, normal_side)], KeyKind::Move(LinearDir::Backward), padding);
        children.push(Box::new(key) as Box<View>);

        let min_pt = rect.min + pt!(small_half_remaining_width + small_half_side + small_medium_length, small_half_remaining_height + 3 * normal_side);
        let key = Key::new(rect![min_pt, min_pt + pt!(big_medium_length, normal_side)], KeyKind::Combine, padding);
        children.push(Box::new(key) as Box<View>);

        // Space bar
        let min_pt = rect.min + pt!(small_half_remaining_width + small_half_side + 3 * normal_side, small_half_remaining_height + 3 * normal_side);
        let key = Key::new(rect![min_pt, min_pt + pt!(huge_length, normal_side)], KeyKind::Output(' '), padding);
        children.push(Box::new(key) as Box<View>);

        let min_pt = rect.min + pt!(small_half_remaining_width + small_half_side + 7 * normal_side, small_half_remaining_height + 3 * normal_side);
        let mut key = Key::new(rect![min_pt, min_pt + pt!(big_medium_length, normal_side)], KeyKind::Alternate, padding);
        if number {
            key.lock();
        }
        children.push(Box::new(key) as Box<View>);

        let min_pt = rect.min + pt!(small_half_remaining_width + small_half_side + 7 * normal_side + big_medium_length, small_half_remaining_height + 3 * normal_side);
        let key = Key::new(rect![min_pt, min_pt + pt!(small_medium_length, normal_side)], KeyKind::Move(LinearDir::Forward), padding);
        children.push(Box::new(key) as Box<View>);

        // Boundary Fillers
        let filler = Filler::new(rect![rect.min,
                                       pt!(rect.max.x - big_half_remaining_width,
                                           rect.min.y + small_half_remaining_height)],
                                 KEYBOARD_BG);
        children.push(Box::new(filler) as Box<View>);

        let filler = Filler::new(rect![pt!(rect.max.x - big_half_remaining_width,
                                           rect.min.y),
                                       pt!(rect.max.x,
                                           rect.max.y - big_half_remaining_height)],
                                 KEYBOARD_BG);
        children.push(Box::new(filler) as Box<View>);

        let filler = Filler::new(rect![pt!(rect.min.x + small_half_remaining_width,
                                           rect.max.y - big_half_remaining_height),
                                       rect.max],
                                 KEYBOARD_BG);
        children.push(Box::new(filler) as Box<View>);

        let filler = Filler::new(rect![pt!(rect.min.x,
                                           rect.min.y + small_half_remaining_height),
                                       pt!(rect.min.x + small_half_remaining_width,
                                           rect.max.y)],
                                 KEYBOARD_BG);
        children.push(Box::new(filler) as Box<View>);

        // In-between Fillers
        let min_pt = pt!(rect.min.x + small_half_remaining_width,
                         rect.min.y + small_half_remaining_height);
        let filler = Filler::new(rect![min_pt, min_pt + pt!(small_half_side,
                                                            normal_side)],
                                 KEYBOARD_BG);
        children.push(Box::new(filler) as Box<View>);

        let min_pt = pt!(rect.min.x + small_half_remaining_width,
                         rect.max.y - big_half_remaining_height - normal_side);
        let filler = Filler::new(rect![min_pt, min_pt + pt!(small_half_side,
                                                            normal_side)],
                                 KEYBOARD_BG);
        children.push(Box::new(filler) as Box<View>);

        let min_pt = pt!(rect.max.x - big_half_remaining_width - big_half_side,
                         rect.min.y + small_half_remaining_height);
        let filler = Filler::new(rect![min_pt, min_pt + pt!(big_half_side,
                                                            normal_side)],
                                 KEYBOARD_BG);
        children.push(Box::new(filler) as Box<View>);

        let min_pt = pt!(rect.max.x - big_half_remaining_width - big_half_side,
                         rect.max.y - big_half_remaining_height - normal_side);
        let filler = Filler::new(rect![min_pt, min_pt + pt!(big_half_side,
                                                            normal_side)],
                                 KEYBOARD_BG);
        children.push(Box::new(filler) as Box<View>);

        Keyboard {
            rect: *rect,
            children,
            layout,
            state,
            combine_buffer: String::new(),
        }
    }

    fn update(&mut self, hub: &Hub) {
        let mut index = 0;

        if self.state.shift > 0 {
            index += 1;
        }

        if self.state.alternate > 0 {
            index += 2;
        }

        for i in 0..10usize {
            if let Some(child) = self.children[i].as_mut().downcast_mut::<Key>() {
                let ch = self.layout.outputs[index].row1[i];
                child.update(KeyKind::Output(ch), hub);
            }
        }

        for i in 0..9usize {
            if let Some(child) = self.children[i+11].as_mut().downcast_mut::<Key>() {
                let ch = self.layout.outputs[index].row2[i];
                child.update(KeyKind::Output(ch), hub);
            }
        }

        for i in 0..7usize {
            if let Some(child) = self.children[i+22].as_mut().downcast_mut::<Key>() {
                let ch = self.layout.outputs[index].row3[i];
                child.update(KeyKind::Output(ch), hub);
            }
        }
    }

    fn release_modifiers(&mut self, hub: &Hub) {
        if self.state.shift != 1 && self.state.alternate != 1 {
            return;
        }
        if self.state.shift == 1 {
            self.state.shift = 0;
            if let Some(child) = self.children[21].as_mut().downcast_mut::<Key>() {
                child.release(hub);
            }
        }
        if self.state.alternate == 1 {
            self.state.alternate = 0;
            if let Some(child) = self.children[33].as_mut().downcast_mut::<Key>() {
                child.release(hub);
            }
        }
        self.update(hub);
    }

    fn release_combine(&mut self, hub: &Hub) {
        self.state.combine = false;
        self.combine_buffer.clear();
        if let Some(child) = self.children[31].as_mut().downcast_mut::<Key>() {
            child.release(hub);
        }
    }

}

impl View for Keyboard {
    fn handle_event(&mut self, evt: &Event, hub: &Hub, _bus: &mut Bus, _context: &mut Context) -> bool {
        match *evt {
            Event::Key(k) => {
                match k {
                    KeyKind::Output(ch) => {
                        if self.state.combine {
                            self.combine_buffer.push(ch);
                            hub.send(Event::Keyboard(KeyboardEvent::Partial(ch))).unwrap();
                            if self.combine_buffer.len() > 1 {
                                if let Some(&ch) = DEFAULT_COMBINATIONS.get(&self.combine_buffer[..]) {
                                    hub.send(Event::Keyboard(KeyboardEvent::Append(ch))).unwrap();
                                }
                                self.release_combine(hub);
                            }
                        } else {
                            hub.send(Event::Keyboard(KeyboardEvent::Append(ch))).unwrap();
                        }
                        self.release_modifiers(hub);
                    }
                    KeyKind::Shift => {
                        self.state.shift = (self.state.shift + 1) % 3;
                        if self.state.shift != 2 {
                            self.update(hub);
                        }
                    },
                    KeyKind::Alternate => {
                        self.state.alternate = (self.state.alternate + 1) % 3;
                        if self.state.alternate != 2 {
                            self.update(hub);
                        }
                    },
                    KeyKind::Delete(dir) => hub.send(Event::Keyboard(KeyboardEvent::Delete { target: TextKind::Char, dir })).unwrap(),
                    KeyKind::Move(dir) => hub.send(Event::Keyboard(KeyboardEvent::Move { target: TextKind::Char, dir })).unwrap(),
                    KeyKind::Combine => self.state.combine = !self.state.combine,
                    KeyKind::Return => {
                        self.release_combine(hub);
                        hub.send(Event::Keyboard(KeyboardEvent::Submit)).unwrap();
                    }
                };
                true
            },
            Event::Gesture(GestureEvent::Tap(center)) |
            Event::Gesture(GestureEvent::HoldFinger(center)) if self.rect.includes(center) => true,
            Event::Gesture(GestureEvent::Swipe { start, .. }) if self.rect.includes(start) => true,
            Event::Device(DeviceEvent::Finger { position, .. }) if self.rect.includes(position) => true,
            _ => false,
        }
    }

    fn might_skip(&self, evt: &Event) -> bool {
        match *evt {
            Event::Key(..) | Event::Gesture(..) | Event::Device(DeviceEvent::Finger { .. }) => false,
            _ => true,
        }
    }

    // TODO: draw background and remove fillers
    fn render(&self, _fb: &mut Framebuffer, _fonts: &mut Fonts) {
    }

    fn resize(&mut self, mut rect: Rectangle, hub: &Hub, context: &mut Context) {
        let dpi = CURRENT_DEVICE.dpi;
        let (side, padding) = optimal_key_setup(rect.width(), rect.height(), dpi);

        let (_, height) = context.display.dims;
        let &(_, big_height) = BAR_SIZES.get(&(height, dpi)).unwrap();
        let height_gap = (rect.height() - (4 * side + 5 * padding)) / big_height;
        rect.min.y += (height_gap * big_height) as i32;

        let normal_side = (side + padding) as i32;
        let (small_half_side, big_half_side) = halves(normal_side);
        let (small_medium_length, big_medium_length) = halves(3 * normal_side);
        let large_length = 2 * normal_side;
        let huge_length = 2 * large_length;

        let remaining_width = rect.width() as i32 - 11 * normal_side;
        let remaining_height = rect.height() as i32 - 4 * normal_side;
        let (small_half_remaining_width, big_half_remaining_width) = halves(remaining_width);
        let (small_half_remaining_height, big_half_remaining_height) = halves(remaining_height);

        // Row 1

        for i in 0..10usize {
            let min_pt = rect.min + pt!(small_half_remaining_width + small_half_side + i as i32 * normal_side,
                                        small_half_remaining_height);
            self.children[i].resize(rect![min_pt, min_pt + normal_side], hub, context);
        }

        // Row 2

        let min_pt = rect.min + pt!(small_half_remaining_width, small_half_remaining_height + normal_side);
        self.children[10].resize(rect![min_pt, min_pt + normal_side], hub, context);

        for i in 0..9usize {
            let min_pt = rect.min + pt!(small_half_remaining_width + (i + 1) as i32 * normal_side,
                                        small_half_remaining_height + normal_side);
            self.children[11+i].resize(rect![min_pt, min_pt + normal_side], hub, context);
        }

        let min_pt = rect.min + pt!(small_half_remaining_width + 10 * normal_side, small_half_remaining_height + normal_side);
        self.children[20].resize(rect![min_pt, min_pt + normal_side], hub, context);

        // Row 3

        let min_pt = rect.min + pt!(small_half_remaining_width, small_half_remaining_height + 2 * normal_side);
        self.children[21].resize(rect![min_pt, min_pt + pt!(large_length, normal_side)], hub, context);

        for i in 0..7usize {
            let min_pt = rect.min + pt!(small_half_remaining_width + (i + 2) as i32 * normal_side,
                                        small_half_remaining_height + 2 * normal_side);
            self.children[22+i].resize(rect![min_pt, min_pt + normal_side], hub, context);
        }

        let min_pt = rect.min + pt!(small_half_remaining_width + 9 * normal_side, small_half_remaining_height + 2 * normal_side);
        self.children[29].resize(rect![min_pt, min_pt + pt!(large_length, normal_side)], hub, context);

        // Row 4

        let min_pt = rect.min + pt!(small_half_remaining_width + small_half_side, small_half_remaining_height + 3 * normal_side);
        self.children[30].resize(rect![min_pt, min_pt + pt!(small_medium_length, normal_side)], hub, context);

        let min_pt = rect.min + pt!(small_half_remaining_width + small_half_side + small_medium_length, small_half_remaining_height + 3 * normal_side);
        self.children[31].resize(rect![min_pt, min_pt + pt!(big_medium_length, normal_side)], hub, context);

        // Space bar
        let min_pt = rect.min + pt!(small_half_remaining_width + small_half_side + 3 * normal_side, small_half_remaining_height + 3 * normal_side);
        self.children[32].resize(rect![min_pt, min_pt + pt!(huge_length, normal_side)], hub, context);

        let min_pt = rect.min + pt!(small_half_remaining_width + small_half_side + 7 * normal_side, small_half_remaining_height + 3 * normal_side);
        self.children[33].resize(rect![min_pt, min_pt + pt!(big_medium_length, normal_side)], hub, context);

        let min_pt = rect.min + pt!(small_half_remaining_width + small_half_side + 7 * normal_side + big_medium_length, small_half_remaining_height + 3 * normal_side);
        self.children[34].resize(rect![min_pt, min_pt + pt!(small_medium_length, normal_side)], hub, context);

        // Boundary Fillers
        self.children[35].resize(rect![rect.min,
                                       pt!(rect.max.x - big_half_remaining_width,
                                           rect.min.y + small_half_remaining_height)],
                                 hub, context);

        self.children[36].resize(rect![pt!(rect.max.x - big_half_remaining_width,
                                           rect.min.y),
                                       pt!(rect.max.x,
                                           rect.max.y - big_half_remaining_height)],
                                hub, context);
        self.children[37].resize(rect![pt!(rect.min.x + small_half_remaining_width,
                                           rect.max.y - big_half_remaining_height),
                                       rect.max],
                                 hub, context);
        self.children[38].resize(rect![pt!(rect.min.x,
                                           rect.min.y + small_half_remaining_height),
                                       pt!(rect.min.x + small_half_remaining_width,
                                           rect.max.y)],
                                 hub, context);

        // In-between Fillers
        let min_pt = pt!(rect.min.x + small_half_remaining_width,
                         rect.min.y + small_half_remaining_height);
        self.children[39].resize(rect![min_pt, min_pt + pt!(small_half_side,
                                                            normal_side)],
                                 hub, context);

        let min_pt = pt!(rect.min.x + small_half_remaining_width,
                         rect.max.y - big_half_remaining_height - normal_side);
        self.children[40].resize(rect![min_pt, min_pt + pt!(small_half_side,
                                                            normal_side)],
                                 hub, context);

        let min_pt = pt!(rect.max.x - big_half_remaining_width - big_half_side,
                         rect.min.y + small_half_remaining_height);
        self.children[41].resize(rect![min_pt, min_pt + pt!(big_half_side,
                                                            normal_side)],
                                 hub, context);

        let min_pt = pt!(rect.max.x - big_half_remaining_width - big_half_side,
                         rect.max.y - big_half_remaining_height - normal_side);
        self.children[42].resize(rect![min_pt, min_pt + pt!(big_half_side,
                                                            normal_side)],
                                 hub, context);

        self.rect = rect;
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

lazy_static! {
    pub static ref DEFAULT_LAYOUT: Layout = Layout {
        name: "US_en".to_string(),
        outputs: [
            OutputKeys {
                row1: ['q', 'w', 'e', 'r', 't', 'y', 'u', 'i', 'o', 'p'],
                row2:   ['a', 's', 'd', 'f', 'g', 'h', 'j', 'k', 'l'],
                row3:        ['z', 'x', 'c', 'v', 'b', 'n', 'm'],
            },
            OutputKeys {
                row1: ['Q', 'W', 'E', 'R', 'T', 'Y', 'U', 'I', 'O', 'P'],
                row2:   ['A', 'S', 'D', 'F', 'G', 'H', 'J', 'K', 'L'],
                row3:        ['Z', 'X', 'C', 'V', 'B', 'N', 'M'],
            },
            OutputKeys {
                row1: ['1', '2', '3', '4', '5', '6', '7', '8', '9', '0'],
                row2:  ['\\', '@', ',', '`', '"', '\'', '.', '*', '/'],
                row3:        ['!', '-', '(',  ':', ')', '+', '?'],
            },
            OutputKeys {
                row1: ['·', '“', '×', '^', '#', '$', '~', '=', '”', '°'],
                row2:   ['‘', '%', '[', '_', '|', '…', ']', '&', '’'],
                row3:        ['–', '<', '{', ';', '}', '>', '—'],
            },
        ],
    };

    // Most of the combination sequences come from X.org.
    // The chosen characters come from the layout described by
    // Robert Bringhurst in *The Elements of Typographic Style*,
    // version 3.1, p. 92.
    pub static ref DEFAULT_COMBINATIONS: FnvHashMap<&'static str, char> = {
        let mut m = FnvHashMap::default();
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
