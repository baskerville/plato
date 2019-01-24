mod input_bar;
mod bottom_bar;
mod code_area;

use std::env;
use std::thread;
use std::path::Path;
use std::sync::mpsc;
use std::collections::VecDeque;
use std::io::Write;
use std::io::{BufRead, BufReader};
use std::process::{Command, Child, Stdio};
use failure::{Error, format_err};
use crate::device::{CURRENT_DEVICE, BAR_SIZES};
use crate::gesture::GestureEvent;
use crate::geom::{Rectangle, CycleDir, halves};
use crate::view::filler::Filler;
use self::input_bar::InputBar;
use self::bottom_bar::BottomBar;
use self::code_area::CodeArea;
use crate::view::top_bar::TopBar;
use crate::view::keyboard::{Keyboard, DEFAULT_LAYOUT};
use crate::view::menu::{Menu, MenuKind};
use crate::view::common::{locate_by_id};
use crate::view::common::{toggle_main_menu, toggle_battery_menu, toggle_clock_menu};
use crate::view::{View, Event, Hub, Bus, EntryKind, EntryId, ViewId};
use crate::view::{THICKNESS_MEDIUM};
use crate::unit::{scale_by_dpi, mm_to_px};
use crate::framebuffer::{Framebuffer, UpdateMode};
use crate::font::Fonts;
use crate::color::BLACK;
use crate::app::Context;

const LIB_PATH: &str = "bin/lib.ivy";

pub struct Calculator {
    rect: Rectangle,
    children: Vec<Box<dyn View>>,
    process: Child,
    data: VecDeque<Line>,
    size: (usize, usize),
    location: (usize, usize),
    history: History,
    margin_width: i32,
    font_size: f32,
}

#[derive(Debug, Clone)]
struct History {
    cursor: usize,
    size: usize,
}

#[derive(Debug, Clone)]
pub struct Line {
    origin: LineOrigin,
    content: String,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum LineOrigin {
    Input,
    Output,
    Error,
}

impl Calculator {
    pub fn new(rect: Rectangle, hub: &Hub, context: &mut Context) -> Result<Calculator, Error> {
        let mut process = Command::new("ivy")
                                 .env("PATH", env::current_dir()?.join("bin"))
                                 .stdin(Stdio::piped())
                                 .stdout(Stdio::piped())
                                 .stderr(Stdio::piped())
                                 .spawn()?;
        let stdout = process.stdout.take()
                            .ok_or_else(|| format_err!("Can't take stdout."))?;
        let stderr = process.stderr.take()
                            .ok_or_else(|| format_err!("Can't take stderr."))?;

        let hub2 = hub.clone();
        thread::spawn(move || {
            let reader = BufReader::new(stdout);
            for line_res in reader.lines() {
                if let Ok(line) = line_res {
                    hub2.send(Event::ProcessLine(LineOrigin::Output, line.clone())).unwrap();
                } else {
                    break;
                }
            }
        });

        let hub3 = hub.clone();
        thread::spawn(move || {
            let reader = BufReader::new(stderr);
            for line_res in reader.lines() {
                if let Ok(line) = line_res {
                    hub3.send(Event::ProcessLine(LineOrigin::Error, line.clone())).unwrap();
                } else {
                    break;
                }
            }
        });

        if Path::new(LIB_PATH).exists() {
            if let Some(stdin) = process.stdin.as_mut() {
                writeln!(stdin, ")get '{}'", LIB_PATH);
            }
        }

        let mut children = Vec::new();
        let dpi = CURRENT_DEVICE.dpi;
        let (_, height) = context.display.dims;
        let (small_height, big_height) = *BAR_SIZES.get(&(height, dpi)).unwrap();
        let thickness = scale_by_dpi(THICKNESS_MEDIUM, dpi) as i32;
        let (small_thickness, big_thickness) = halves(thickness);
        let side = small_height as i32;

        let font_size = context.settings.calculator.font_size;
        let margin_width = context.settings.calculator.margin_width;
        let history = History { cursor: 0, size: context.settings.calculator.history_size };

        let top_bar = TopBar::new(rect![rect.min.x, rect.min.y,
                                        rect.max.x, rect.min.y + side - small_thickness],
                                  Event::Back,
                                  "Calculator".to_string(),
                                  context);
        children.push(Box::new(top_bar) as Box<dyn View>);

        let separator = Filler::new(rect![rect.min.x,
                                          rect.min.y + side - small_thickness,
                                          rect.max.x,
                                          rect.min.y + side + big_thickness],
                                    BLACK);
        children.push(Box::new(separator) as Box<dyn View>);

        let mut kb_rect = rect![rect.min.x,
                                rect.max.y - (small_height + 3 * big_height) as i32 + big_thickness,
                                rect.max.x,
                                rect.max.y - small_height as i32 - small_thickness];

        let keyboard = Keyboard::new(&mut kb_rect, DEFAULT_LAYOUT.clone(), true, context);

        let sp_rect = rect![rect.min.x, kb_rect.min.y - thickness,
                            rect.max.x, kb_rect.min.y];

        let input_bar = InputBar::new(rect![rect.min.x, sp_rect.min.y - side + thickness,
                                            rect.max.x, sp_rect.min.y],
                                      "",
                                      "");

        let sp_rect2 = rect![rect.min.x, sp_rect.min.y - side,
                             rect.max.x, sp_rect.min.y - side + thickness];

        let code_area_rect = rect![rect.min.x,
                                   rect.min.y + side + big_thickness,
                                   rect.max.x,
                                   sp_rect2.min.y];
        let code_area = CodeArea::new(code_area_rect, font_size, margin_width);
        children.push(Box::new(code_area) as Box<dyn View>);

        let separator = Filler::new(sp_rect2, BLACK);
        children.push(Box::new(separator) as Box<dyn View>);

        children.push(Box::new(input_bar) as Box<dyn View>);

        let separator = Filler::new(sp_rect, BLACK);
        children.push(Box::new(separator) as Box<dyn View>);

        children.push(Box::new(keyboard) as Box<dyn View>);

        let separator = Filler::new(rect![rect.min.x, rect.max.y - side - small_thickness,
                                          rect.max.x, rect.max.y - side + big_thickness],
                                    BLACK);
        children.push(Box::new(separator) as Box<dyn View>);

        let bottom_bar = BottomBar::new(rect![rect.min.x, rect.max.y - side + big_thickness,
                                              rect.max.x, rect.max.y],
                                        margin_width,
                                        font_size);
        children.push(Box::new(bottom_bar) as Box<dyn View>);

        let font = &mut context.fonts.monospace.regular;
        font.set_size((64.0 * font_size) as u32, dpi);
        let char_width = font.plan(" ", None, None).width as i32;
        let line_height = font.ascender() - font.descender();
        let margin_width_px = mm_to_px(margin_width as f32, dpi) as i32;
        let columns_count = (code_area_rect.width() as i32 - 2 * margin_width_px) / char_width;
        let lines_count = (code_area_rect.height() as i32 - 2 * margin_width_px) / line_height;

        hub.send(Event::Render(rect, UpdateMode::Full)).unwrap();
        hub.send(Event::Focus(Some(ViewId::CalculatorInput))).unwrap();

        Ok(Calculator {
            rect,
            children,
            process,
            data: VecDeque::new(),
            size: (lines_count as usize, columns_count as usize),
            location: (0, 0),
            history,
            font_size,
            margin_width,
        })
    }

    fn append(&mut self, line: Line, context: &mut Context) {
        let (lines_count, columns_count) = self.size;
        let (mut current_line, mut current_column) = self.location;
        let mut screen_lines = 0;

        while screen_lines <= lines_count && current_line < self.data.len() {
            screen_lines += (self.data[current_line].content[current_column..].chars().count() as f32 /
                             columns_count as f32).ceil().max(1.0) as usize;
            current_line += 1;
            current_column = 0;
        }

        if screen_lines <= lines_count {
            let added_lines = (line.content.chars().count() as f32 /
                               columns_count as f32).ceil().max(1.0) as usize;
            if screen_lines + added_lines > lines_count {
                let filled_pages = ((screen_lines + added_lines) as f32 / lines_count as f32).ceil() as usize;
                let chars_count = columns_count * ((filled_pages - 1) * lines_count - screen_lines);
                let current_column = line.content.char_indices().nth(chars_count).map_or(0, |v| v.0);
                self.location = (self.data.len(), current_column);

                if let Some(code_area) = self.children[2].downcast_mut::<CodeArea>() {
                    let last_line = Line { content: line.content[current_column..].to_string(),
                                           origin: line.origin };
                    code_area.set_data(vec![last_line], context);
                }
            } else {
                if let Some(code_area) = self.children[2].downcast_mut::<CodeArea>() {
                    code_area.append(line.clone(), added_lines as i32, screen_lines as i32, context);
                }
            }
        }

        self.data.push_back(line);

        if self.data.len() > self.history.size {
            self.data.pop_front();
            if self.location.0 == 0 {
                self.location = (0, 0);
                self.refresh(context);
            } else {
                self.location.0 -= 1;
            }
        }

        self.history.cursor = self.data.len();
    }

    fn scroll(&mut self, mut delta_lines: i32, context: &mut Context) {
        if delta_lines == 0 || self.data.is_empty() {
            return;
        }

        let (_, columns_count) = self.size;
        let (mut current_line, mut current_column) = self.location;

        if delta_lines < 0 {
            let lines_before = (self.data[current_line].content[..current_column].chars().count() /
                                columns_count) as i32;
            delta_lines += lines_before;
            if delta_lines < 0 && current_line > 0 {
                current_line -= 1;
                loop {
                    let lines_before = (self.data[current_line].content.chars().count() as f32 /
                                        columns_count as f32).ceil().max(1.0) as i32;
                    delta_lines += lines_before;
                    if delta_lines >= 0 || current_line == 0 {
                        break;
                    }
                    current_line -= 1;
                }
            }

            let chars_count = delta_lines.max(0) as usize * columns_count;
            let current_column = self.data[current_line].content.char_indices().nth(chars_count).map_or(0, |v| v.0);
            self.location = (current_line, current_column);
        } else {
            loop {
                let lines_after = (self.data[current_line].content[current_column..].chars().count() as f32 /
                                   columns_count as f32).ceil().max(1.0) as i32;
                delta_lines -= lines_after;
                if delta_lines < 0  || current_line == self.data.len() - 1 {
                    break;
                }
                current_line += 1;
                current_column = 0;
            }

            let chars_count = ((self.data[current_line].content[current_column..].chars().count() as f32 /
                                columns_count as f32).ceil().max(1.0) as i32 + delta_lines.min(-1)) as usize * columns_count;
            current_column += self.data[current_line].content[current_column..]
                                  .char_indices().nth(chars_count).map_or(0, |v| v.0);

            self.location = (current_line, current_column);
        }

        self.refresh(context);
    }

    fn scroll_pixels(&mut self, dy: i32, context: &mut Context) {
        let dpi = CURRENT_DEVICE.dpi;
        let line_height = {
            let font = &mut context.fonts.monospace.regular;
            font.set_size((64.0 * self.font_size) as u32, dpi);
            font.ascender() - font.descender()
        };
        let mut delta_lines = (dy as f32 / line_height as f32).round() as i32;

        self.scroll(delta_lines, context);
    }

    fn scroll_page(&mut self, dir: CycleDir, context: &mut Context) {
        let sgn = if dir == CycleDir::Previous { -1 } else { 1 };
        let delta_lines = sgn * self.size.0 as i32;
        self.scroll(delta_lines, context);
    }

    fn refresh(&mut self, context: &mut Context) {
        let mut data = Vec::new();
        let (mut current_line, mut current_column) = self.location;
        let (lines_count, columns_count) = self.size;

        let mut screen_lines = 0;

        while screen_lines < lines_count && current_line < self.data.len() {
            let mut line = Line { content: self.data[current_line].content[current_column..].to_string(),
                                  origin: self.data[current_line].origin };
            screen_lines += (line.content.chars().count() as f32 /
                             columns_count as f32).ceil().max(1.0) as usize;
            if screen_lines > lines_count {
                let delta = screen_lines - lines_count;
                let chars_count = columns_count * ((line.content.chars().count() as f32 /
                                                    columns_count as f32).ceil().max(1.0) as usize - delta);
                let column_cut = line.content.char_indices().nth(chars_count).map_or(0, |v| v.0);
                line.content = line.content[..column_cut].to_string();
            }
            data.push(line);
            current_line += 1;
            current_column = 0;
        }

        if let Some(code_area) = self.children[2].downcast_mut::<CodeArea>() {
            code_area.set_data(data, context);
        }
    }

    fn history_navigate(&mut self, dir: CycleDir, honor_prefix: bool, hub: &Hub) {
        let beginning = if honor_prefix {
            self.children[4].downcast_ref::<InputBar>().unwrap().text_before_cursor()
        } else {
            ""
        };

        let cursor_opt = match dir {
            CycleDir::Previous => {
                self.data.iter().enumerate().rev()
                    .find(|(index, line)| *index < self.history.cursor &&
                                          line.origin == LineOrigin::Input &&
                                          line.content.starts_with(beginning))
                    .map(|(index, _)| index)
            },
            CycleDir::Next => {
                self.data.iter().enumerate()
                    .find(|(index, line)| *index > self.history.cursor &&
                                          line.origin == LineOrigin::Input &&
                                          line.content.starts_with(beginning))
                    .map(|(index, _)| index)
            },
        };

        if let Some(cursor) = cursor_opt {
            let line = self.data[cursor].content.as_str();
            if let Some(input_bar) = self.children[4].downcast_mut::<InputBar>() {
                input_bar.set_text(line, !honor_prefix, hub);
            }
            self.history.cursor = cursor;
        }
    }

    fn update_size(&mut self, hub: &Hub, context: &mut Context) {
        let dpi = CURRENT_DEVICE.dpi;
        let font = &mut context.fonts.monospace.regular;
        font.set_size((64.0 * self.font_size) as u32, dpi);
        let char_width = font.plan(" ", None, None).width as i32;
        let line_height = font.ascender() - font.descender();
        let margin_width_px = mm_to_px(self.margin_width as f32, dpi) as i32;
        if let Some(code_area) = self.children[2].downcast_mut::<CodeArea>() {
            let columns_count = (code_area.rect().width() as i32 - 2 * margin_width_px) / char_width;
            let lines_count = (code_area.rect().height() as i32 - 2 * margin_width_px) / line_height;
            self.size = (lines_count as usize, columns_count as usize);
            code_area.update(self.font_size, self.margin_width);
        }
        if let Some(bottom_bar) = self.children[8].downcast_mut::<BottomBar>() {
            bottom_bar.update_font_size(self.font_size, hub);
            bottom_bar.update_margin_width(self.margin_width, hub);
        }
    }

    fn set_font_size(&mut self, font_size: f32, hub: &Hub, context: &mut Context) {
        self.font_size = font_size;
        self.update_size(hub, context);
        self.refresh(context);
    }

    fn set_margin_width(&mut self, margin_width: i32, hub: &Hub, context: &mut Context) {
        self.margin_width = margin_width;
        self.update_size(hub, context);
        self.refresh(context);
    }

    fn toggle_margin_width_menu(&mut self, rect: Rectangle, enable: Option<bool>, hub: &Hub, context: &mut Context) {
        if let Some(index) = locate_by_id(self, ViewId::MarginWidthMenu) {
            if let Some(true) = enable {
                return;
            }

            hub.send(Event::Expose(*self.child(index).rect(), UpdateMode::Gui)).unwrap();
            self.children.remove(index);
        } else {
            if let Some(false) = enable {
                return;
            }

            let entries = (0..=10).map(|mw| EntryKind::RadioButton(format!("{}", mw),
                                                                  EntryId::SetMarginWidth(mw),
                                                                  mw == self.margin_width)).collect();
            let margin_width_menu = Menu::new(rect, ViewId::MarginWidthMenu, MenuKind::DropDown, entries, context);
            hub.send(Event::Render(*margin_width_menu.rect(), UpdateMode::Gui)).unwrap();
            self.children.push(Box::new(margin_width_menu) as Box<dyn View>);
        }
    }

    fn toggle_font_size_menu(&mut self, rect: Rectangle, enable: Option<bool>, hub: &Hub, context: &mut Context) {
        if let Some(index) = locate_by_id(self, ViewId::FontSizeMenu) {
            if let Some(true) = enable {
                return;
            }

            hub.send(Event::Expose(*self.child(index).rect(), UpdateMode::Gui)).unwrap();
            self.children.remove(index);
        } else {
            if let Some(false) = enable {
                return;
            }

            let entries = (0..=20).map(|v| {
                let fs = 6.0 + v as f32 / 10.0;
                EntryKind::RadioButton(format!("{:.1}", fs),
                                       EntryId::SetFontSize(v),
                                       (fs - self.font_size).abs() < 0.05)
            }).collect();
            let font_size_menu = Menu::new(rect, ViewId::FontSizeMenu, MenuKind::DropDown, entries, context);
            hub.send(Event::Render(*font_size_menu.rect(), UpdateMode::Gui)).unwrap();
            self.children.push(Box::new(font_size_menu) as Box<dyn View>);
        }
    }

    fn reseed(&mut self, hub: &Hub, context: &mut Context) {
        let (tx, _rx) = mpsc::channel();
        if let Some(top_bar) = self.child_mut(0).downcast_mut::<TopBar>() {
            top_bar.update_frontlight_icon(&tx, context);
        }
        hub.send(Event::ClockTick).unwrap();
        hub.send(Event::BatteryTick).unwrap();
        hub.send(Event::Render(self.rect, UpdateMode::Gui)).unwrap();
    }

    fn quit(&mut self, context: &mut Context) {
        self.process.kill().map_err(|e| eprintln!("Can't kill child process: {}.", e)).ok();
        self.process.wait().map_err(|e| eprintln!("Can't wait child process: {}.", e)).ok();
        context.settings.calculator.font_size = self.font_size;
        context.settings.calculator.margin_width = self.margin_width;
    }
}

impl View for Calculator {
    fn handle_event(&mut self, evt: &Event, hub: &Hub, _bus: &mut Bus, context: &mut Context) -> bool {
        match *evt {
            Event::Submit(ViewId::CalculatorInput, ref line) => {
                self.append(Line { origin: LineOrigin::Input, content: line.to_string() }, context);
                if let Some(input_bar) = self.children[4].downcast_mut::<InputBar>() {
                    input_bar.set_text("", true, hub);
                }
                if let Some(stdin) = self.process.stdin.as_mut() {
                    writeln!(stdin, "{}", line);
                }
                true
            },
            Event::Scroll(dy) => {
                self.scroll_pixels(dy, context);
                true
            },
            Event::Page(dir) => {
                self.scroll_page(dir, context);
                true
            },
            Event::ProcessLine(origin, ref line) => {
                self.append(Line { origin, content: line.replace('\t', "    ") }, context);
                true
            },
            Event::History(dir, honor_prefix) => {
                self.history_navigate(dir, honor_prefix, hub);
                true
            },
            Event::Select(EntryId::SetFontSize(v)) => {
                let font_size = 6.0 + v as f32 / 10.0;
                self.set_font_size(font_size, hub, context);
                true
            },
            Event::Select(EntryId::SetMarginWidth(width)) => {
                self.set_margin_width(width, hub, context);
                true
            },
            Event::Gesture(GestureEvent::Rotate { quarter_turns, .. }) if quarter_turns != 0 => {
                let (_, dir) = CURRENT_DEVICE.mirroring_scheme();
                let n = (4 + (context.display.rotation - dir * quarter_turns)) % 4;
                hub.send(Event::Select(EntryId::Rotate(n))).unwrap();
                true
            },
            Event::Gesture(GestureEvent::HoldFinger(center)) if self.rect.includes(center) => {
                hub.send(Event::Render(self.rect, UpdateMode::Full));
                true
            },
            Event::ToggleNear(ViewId::MainMenu, rect) => {
                toggle_main_menu(self, rect, None, hub, context);
                true
            },
            Event::ToggleNear(ViewId::BatteryMenu, rect) => {
                toggle_battery_menu(self, rect, None, hub, context);
                true
            },
            Event::ToggleNear(ViewId::ClockMenu, rect) => {
                toggle_clock_menu(self, rect, None, hub, context);
                true
            },
            Event::ToggleNear(ViewId::MarginWidthMenu, rect) => {
                self.toggle_margin_width_menu(rect, None, hub, context);
                true
            },
            Event::ToggleNear(ViewId::FontSizeMenu, rect) => {
                self.toggle_font_size_menu(rect, None, hub, context);
                true
            },
            Event::Back | Event::Select(EntryId::Quit) => {
                self.quit(context);
                hub.send(Event::Back).unwrap();
                true
            },
            Event::Reseed => {
                self.reseed(hub, context);
                true
            },
            _ => false,
        }
    }

    fn render(&self, _fb: &mut Framebuffer, rect: Rectangle, _fonts: &mut Fonts) -> Rectangle {
        rect
    }

    fn resize(&mut self, rect: Rectangle, hub: &Hub, context: &mut Context) {
        let dpi = CURRENT_DEVICE.dpi;
        let (_, height) = context.display.dims;
        let (small_height, big_height) = *BAR_SIZES.get(&(height, dpi)).unwrap();
        let thickness = scale_by_dpi(THICKNESS_MEDIUM, dpi) as i32;
        let (small_thickness, big_thickness) = halves(thickness);
        let side = small_height as i32;
        let (tx, _rx) = mpsc::channel();

        self.children.retain(|child| !child.is::<Menu>());

        // Top bar.
        let top_bar_rect = rect![rect.min.x, rect.min.y,
                                 rect.max.x, rect.min.y + side - small_thickness];
        self.children[0].resize(top_bar_rect, hub, context);

        let separator_rect = rect![rect.min.x,
                                   rect.min.y + side - small_thickness,
                                   rect.max.x,
                                   rect.min.y + side + big_thickness];
        self.children[1].resize(separator_rect, hub, context);

        let kb_rect = rect![rect.min.x,
                            rect.max.y - (small_height + 3 * big_height) as i32 + big_thickness,
                            rect.max.x,
                            rect.max.y - small_height as i32 - small_thickness];
        self.children[6].resize(kb_rect, hub, context);
        let kb_rect = *self.children[6].rect();

        let sp_rect = rect![rect.min.x, kb_rect.min.y - thickness,
                            rect.max.x, kb_rect.min.y];

        let sp_rect2 = rect![rect.min.x, sp_rect.min.y - side,
                             rect.max.x, sp_rect.min.y - side + thickness];

        let input_bar_rect = rect![rect.min.x, sp_rect.min.y - side + thickness,
                                   rect.max.x, sp_rect.min.y];

        let code_area_rect = rect![rect.min.x,
                                   rect.min.y + side + big_thickness,
                                   rect.max.x,
                                   sp_rect2.min.y];

        self.children[2].resize(code_area_rect, hub, context);
        self.children[3].resize(sp_rect2, hub, context);
        self.children[4].resize(input_bar_rect, hub, context);
        self.children[5].resize(sp_rect, hub, context);

        let sp_rect = rect![rect.min.x, rect.max.y - side - small_thickness,
                            rect.max.x, rect.max.y - side + big_thickness];

        self.children[7].resize(sp_rect, hub, context);

        let bottom_bar_rect = rect![rect.min.x, rect.max.y - side + big_thickness,
                                    rect.max.x, rect.max.y];

        self.children[8].resize(bottom_bar_rect, hub, context);

        for i in 9..self.children.len() {
            self.children[i].resize(rect, hub, context);
        }

        self.update_size(&tx, context);
        self.refresh(context);

        self.rect = rect;
        hub.send(Event::Render(self.rect, UpdateMode::Full)).unwrap();
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
