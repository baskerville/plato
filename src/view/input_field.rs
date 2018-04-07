use device::CURRENT_DEVICE;
use framebuffer::{Framebuffer, UpdateMode};
use view::{View, Event, Hub, Bus, KeyboardEvent, ViewId, TextKind};
use view::THICKNESS_MEDIUM;
use gesture::GestureEvent;
use font::{Fonts, font_from_style, NORMAL_STYLE, FONT_SIZES};
use geom::{Rectangle, LinearDir, BorderSpec, halves};
use color::{TEXT_NORMAL, BLACK};
use app::Context;
use unit::scale_by_dpi;

pub struct InputField {
    pub rect: Rectangle,
    children: Vec<Box<View>>,
    id: ViewId,
    text: String,
    partial: String,
    placeholder: String,
    cursor: usize,
    border: bool,
    focused: bool,
}

fn closest_char_boundary(text: &str, index: usize, dir: LinearDir) -> Option<usize> {
    match dir {
        LinearDir::Backward => {
            if index == 0 {
                return Some(index);
            }
            (0..index).rev().find(|&i| text.is_char_boundary(i))
        },
        LinearDir::Forward => {
            if index == text.len() {
                return Some(index);
            }
            (index+1..text.len()+1).find(|&i| text.is_char_boundary(i))
        },
    }
}

fn char_position(text: &str, index: usize) -> Option<usize> {
    text.char_indices().map(|(i, _)| i).position(|i| i == index)
}

fn word_boundary(text: &str, index: usize, dir: LinearDir) -> usize {
    match dir {
        LinearDir::Backward => {
            if index == 0 {
                return index; 
            }
            text[..index].rfind(|c: char| !c.is_alphanumeric())
                .and_then(|prev_index| closest_char_boundary(text, prev_index, LinearDir::Forward)
                .map(|next_index| {
                    if index != next_index {
                        next_index
                    } else {
                        word_boundary(text, prev_index, dir)
                    }
                })).unwrap_or(0)
        },
        LinearDir::Forward => {
            if index == text.len() {
                return index;
            }
            text[index..].find(|c: char| !c.is_alphanumeric())
                .map(|next_index| {
                    if next_index == 0 {
                        word_boundary(text, index + 1, dir)
                    } else {
                        index + next_index
                    }
                }).unwrap_or_else(|| text.len())
        }
    }
}

// TODO: hidden chars (password…)
impl InputField {
    pub fn new(rect: Rectangle, id: ViewId) -> InputField {
        InputField {
            rect,
            children: vec![],
            id,
            text: "".to_string(),
            partial: "".to_string(),
            placeholder: "".to_string(),
            cursor: 0,
            border: true,
            focused: false,
        }
    }

    pub fn border(mut self, border: bool) -> InputField {
        self.border = border;
        self
    }

    pub fn placeholder(mut self, placeholder: &str) -> InputField {
        self.placeholder = placeholder.to_string();
        self
    }

    pub fn text(mut self, text: &str) -> InputField {
        self.text = text.to_string();
        self.cursor = self.text.len();
        self
    }

    fn char_move(&mut self, dir: LinearDir) {
        if let Some(index) = closest_char_boundary(&self.text, self.cursor, dir) {
            self.cursor = index;
        }
    }

    fn char_delete(&mut self, dir: LinearDir) {
            match dir {
                LinearDir::Backward if self.cursor > 0 => {
                    if let Some(index) = closest_char_boundary(&self.text, self.cursor, dir) {
                        self.cursor = index;
                        self.text.remove(index);
                    }
                },
                LinearDir::Forward if self.cursor < self.text.len() => {
                    self.text.remove(self.cursor);
                },
                _ => (),
            }
    }

    fn word_move(&mut self, dir: LinearDir) {
        self.cursor = word_boundary(&self.text, self.cursor, dir);
    }

    fn word_delete(&mut self, dir: LinearDir) {
        let next_cursor = word_boundary(&self.text, self.cursor, dir);
        match dir {
            LinearDir::Backward => {
                self.text.drain(next_cursor..self.cursor);
                self.cursor = next_cursor;
            },
            LinearDir::Forward => {
                self.text.drain(self.cursor..next_cursor);
            }
        }
    }

    fn extremum_move(&mut self, dir: LinearDir) {
        match dir {
            LinearDir::Backward => self.cursor = 0,
            LinearDir::Forward => self.cursor = self.text.len(),
        }
    }

    fn extremum_delete(&mut self, dir: LinearDir) {
        match dir {
            LinearDir::Backward => {
                self.text.drain(0..self.cursor);
                self.cursor = 0;
            },
            LinearDir::Forward => {
                let len = self.text.len();
                self.text.drain(self.cursor..len);
            },
        }
    }
}

impl View for InputField {
    fn handle_event(&mut self, evt: &Event, hub: &Hub, bus: &mut Bus, _context: &mut Context) -> bool {
        match *evt {
            Event::Gesture(GestureEvent::Tap(ref center)) if self.rect.includes(center) => {
                self.focused = true;
                bus.push_back(Event::Focus(Some(self.id)));
                hub.send(Event::Render(self.rect, UpdateMode::Gui)).unwrap();
                true
            },
            Event::Focus(id_opt) => {
                let focused = id_opt.is_some() && id_opt.unwrap() == self.id;
                if self.focused != focused {
                    self.focused = focused;
                    hub.send(Event::Render(self.rect, UpdateMode::Gui)).unwrap();
                }
                false
            },
            Event::Keyboard(kbd_evt) if self.focused => {
                match kbd_evt {
                    KeyboardEvent::Append(c) => {
                        self.text.insert(self.cursor, c);
                        self.partial.clear();
                        if let Some(index) = closest_char_boundary(&self.text, self.cursor, LinearDir::Forward) {
                            self.cursor = index;
                        }
                    },
                    KeyboardEvent::Partial(c) => {
                        self.partial.push(c);
                    },
                    KeyboardEvent::Move { target, dir } => {
                        match target {
                            TextKind::Char => self.char_move(dir),
                            TextKind::Word => self.word_move(dir),
                            TextKind::Extremum => self.extremum_move(dir),
                        }
                    },
                    KeyboardEvent::Delete { target, dir } => {
                        match target {
                            TextKind::Char => self.char_delete(dir),
                            TextKind::Word => self.word_delete(dir),
                            TextKind::Extremum => self.extremum_delete(dir),
                        }
                    },
                    KeyboardEvent::Submit => {
                        bus.push_back(Event::Submit(self.id, self.text.clone()));
                    },
                };
                hub.send(Event::RenderNoWait(self.rect, UpdateMode::Gui)).unwrap();
                true
            }
            _ => false,
        }
    }

    fn render(&self, fb: &mut Framebuffer, fonts: &mut Fonts) {
        let dpi = CURRENT_DEVICE.dpi;
        let font = font_from_style(fonts, &NORMAL_STYLE, dpi);
        let padding = font.em() as i32;
        let x_height = font.x_heights.0 as i32;
        let cursor_height = 2 * x_height;
        let max_width = self.rect.width().saturating_sub(2 * padding as u32) as i32;

        fb.draw_rectangle(&self.rect, TEXT_NORMAL[0]);

        let thickness = scale_by_dpi(THICKNESS_MEDIUM, dpi) as i32;

        if self.border {
            fb.draw_rectangle_outline(&self.rect,
                                      &BorderSpec { thickness: thickness as u16, color: BLACK });
        }

        let (mut plan, foreground) = if self.text.is_empty() {
            (font.plan(&self.placeholder, Some(max_width as u32), None),
             TEXT_NORMAL[2])
        } else {
            (font.plan(&self.text, None, Some("-liga")),
            TEXT_NORMAL[1])
        };

        let dy = (self.rect.height() as i32 - x_height) / 2;
        let pt = pt!(self.rect.min.x + padding, self.rect.max.y - dy);
        
        let mut index = char_position(&self.text, self.cursor).unwrap_or_else(|| self.text.chars().count());
        let lower_index = font.crop_around(&mut plan, index, max_width as u32);

        font.render(fb, foreground, &plan, &pt);

        if !self.focused {
            return;
        }

        if lower_index > 0 {
            index += 1;
        }

        let mut dx = plan.advance_at(index - lower_index);

        let (small_dy, big_dy) = halves(self.rect.height() as i32 - cursor_height);

        if self.text.is_empty() {
            dx -= 3 * thickness;
        }

        fb.draw_rectangle(&rect![self.rect.min.x + padding + dx,
                                 self.rect.min.y + small_dy,
                                 self.rect.min.x + padding + dx + thickness,
                                 self.rect.max.y - big_dy],
                          BLACK);

        if !self.partial.is_empty() {
            font.set_size(FONT_SIZES[0], dpi);
            let x_height = font.x_heights.0 as i32;
            let plan = font.plan(&self.partial, None, None);
            let pt = pt!(self.rect.min.x + padding + dx + 3 * thickness,
                         self.rect.max.y - big_dy + x_height);
            font.render(fb, TEXT_NORMAL[1], &plan, &pt);
        }
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
