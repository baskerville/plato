use std::f32;
use crate::device::CURRENT_DEVICE;
use std::collections::BTreeSet;
use crate::framebuffer::{Framebuffer, UpdateMode};
use crate::view::{View, Event, Hub, Bus, Align};
use crate::view::icon::{Icon, ICONS_PIXMAPS};
use crate::view::filler::Filler;
use super::category::{Category, Status};
use crate::gesture::GestureEvent;
use crate::color::TEXT_BUMP_SMALL;
use crate::app::Context;
use crate::symbolic_path::SymbolicPath;
use crate::font::{Font, Fonts, font_from_style, category_font_size, NORMAL_STYLE};
use crate::geom::{Rectangle, Dir, CycleDir, divide, small_half, big_half};

#[derive(Debug)]
pub struct Summary {
    pub rect: Rectangle,
    pages: Vec<Vec<Box<dyn View>>>,
    current_page: usize,
}

#[derive(Debug, Clone)]
struct Page<'a> {
    start_index: usize,
    end_index: usize,
    lines: Vec<Line<'a>>,
}

impl<'a> Default for Page<'a> {
    fn default() -> Page<'a> {
        Page {
            start_index: 0,
            end_index: 0,
            lines: vec![],
        }
    }
}

#[derive(Debug, Clone)]
struct Layout {
    x_height: i32,
    padding: i32,
    max_line_width: i32,
    max_lines: usize,
}

#[derive(Debug, Clone)]
struct Line<'a> {
    width: i32,
    labels_count: usize,
    items: Vec<Item<'a>>,
}

impl<'a> Default for Line<'a> {
    fn default() -> Line<'a> {
        Line {
            width: 0,
            labels_count: 0,
            items: vec![],
        }
    }
}

#[derive(Debug, Clone)]
enum Item<'a> {
    Label { text: &'a str, width: i32, max_width: Option<u32> },
    Icon { name: &'a str, width: i32 },
}

impl<'a> Item<'a> {
    #[inline]
    fn width(&self) -> i32 {
        match *self {
            Item::Label { width, .. } | Item::Icon { width, .. } => width,
        }
    }
}

impl Summary {
    pub fn new(rect: Rectangle) -> Summary {
        Summary {
            rect,
            current_page: 0,
            pages: vec![],
        }
    }

    pub fn set_current_page(&mut self, dir: CycleDir) {
        match dir {
            CycleDir::Next if self.current_page < self.pages.len() - 1 => {
                self.current_page += 1;
            },
            CycleDir::Previous if self.current_page > 0 => {
                self.current_page -= 1;
            },
            _ => (),
        }
    }

    pub fn update(&mut self, visible_categories: &BTreeSet<String>, selected_categories: &BTreeSet<String>, negated_categories: &BTreeSet<String>, was_resized: bool, hub: &Hub, fonts: &mut Fonts) {
        let dpi = CURRENT_DEVICE.dpi;
        let mut start_index = 0;
        let mut font = font_from_style(fonts, &NORMAL_STYLE, dpi);
        let x_height = font.x_heights.0 as i32;
        let padding = font.em() as i32;
        let max_line_width = self.rect.width() as i32 - 2 * padding;
        // Have at least 2 *x* heights between a line's baseline and the next line's mean line.
        let max_lines = ((self.rect.height() as f32 / x_height as f32 - 2.0) / 3.0).max(1.0) as usize;
        let layout = Layout { x_height, padding, max_line_width, max_lines };
        
        let last_pages_count = self.pages.len();
        self.pages.clear();

        loop {
            let (children, end_index) = {
                let page = self.make_page(start_index, &layout, visible_categories, &mut font);
                let children = self.make_children(&page, &layout, visible_categories,
                                                  selected_categories, negated_categories);
                (children, page.end_index)
            };
            self.pages.push(children);
            if end_index == visible_categories.len() {
                break;
            }
            start_index = end_index;
        }

        if was_resized {
            let page_position = if last_pages_count == 0 {
                0.0
            } else {
                self.current_page as f32 / last_pages_count as f32
            };
            let mut page_guess = page_position * self.pages.len() as f32;
            let page_ceil = page_guess.ceil();
            if (page_ceil - page_guess) < f32::EPSILON {
                page_guess = page_ceil;
            }
            self.current_page = (page_guess as usize).min(self.pages.len() - 1);
        } else {
            // TODO: restore current_page so that the last *manipulated* category is visible?
            self.current_page = 0;
        }

        hub.send(Event::Render(self.rect, UpdateMode::Gui)).unwrap();
    }

    fn make_page<'a>(&self, start_index: usize, layout: &Layout, visible_categories: &'a BTreeSet<String>, font: &mut Font) -> Page<'a> {
        let dpi = CURRENT_DEVICE.dpi;
        let Layout { padding, max_line_width, max_lines, .. } = *layout;
        let mut end_index = start_index;
        let mut line = Line::default();
        let mut page = Page::default();

        if start_index > 0 {
            let pixmap = ICONS_PIXMAPS.get("angle-left-small").unwrap();
            line.width += pixmap.width as i32 + padding;
            line.items.push(Item::Icon { name: "angle-left-small",
                                         width: pixmap.width as i32 });
        }

        for categ in visible_categories.iter().skip(start_index) {
            font.set_size(category_font_size(categ.depth()), dpi);
            let mut categ_width = font.plan(categ.last_component(),
                                            None,
                                            None).width as i32;
            let mut max_categ_width = None;

            if categ_width > max_line_width {
                max_categ_width = Some(max_line_width as u32);
                categ_width = max_line_width;
            }

            line.labels_count += 1;
            line.width += categ_width;
            end_index += 1;
            let label = Item::Label { text: categ,
                                      width: categ_width,
                                      max_width: max_categ_width };
            line.items.push(label);

            if line.width >= max_line_width {
                let mut next_line = Line::default();
                if line.width > max_line_width {
                    if line.labels_count > 1 {
                        if let Some(item) = line.items.pop() {
                            line.width -= item.width() + padding;
                            line.labels_count -= 1;
                            next_line.width += item.width() + padding;
                            next_line.items.push(item);
                            next_line.labels_count += 1;
                        }
                    }
                    if line.labels_count == 1 {
                        let occupied_width = line.width - line.items.last().unwrap().width();
                        if let Some(&mut Item::Label { ref mut width,
                                                       ref mut max_width, .. }) = line.items.last_mut() {
                            *width = max_line_width - occupied_width;
                            *max_width = Some(*width as u32);
                        }
                        line.width = max_line_width;
                    }
                }
                page.lines.push(line);
                line = next_line;
                if page.lines.len() >= max_lines {
                    break;
                }
            } else {
                line.width += padding;
            }
        }

        if page.lines.len() < max_lines {
            page.lines.push(line);
        } else {
            end_index -= line.items.len();
        }

        if end_index < visible_categories.len() {

            if let Some(mut line) = page.lines.pop() {
                let pixmap = ICONS_PIXMAPS.get("angle-right-small").unwrap();
                line.width += pixmap.width as i32 + padding;

                if line.labels_count > 1 {
                    while line.width > max_line_width {
                        if let Some(Item::Label { width, .. }) = line.items.pop() {
                            line.width -= width + padding;
                            line.labels_count -= 1;
                            end_index -= 1;
                        } else {
                            break;
                        }
                    }
                } else {
                    let occupied_width = line.width - line.items.last().unwrap().width();
                    if let Some(&mut Item::Label { ref mut width,
                                                   ref mut max_width, .. }) = line.items.last_mut() {
                        *width = max_line_width - occupied_width;
                        *max_width = Some(*width as u32);
                    }
                    line.width = max_line_width;
                }

                line.items.push(Item::Icon { name: "angle-right-small",
                                             width: pixmap.width as i32 });
                page.lines.push(line);
            }
        }

        page.start_index = start_index;
        page.end_index = end_index;
        page
    }

    fn make_children(&self, page: &Page, layout: &Layout, visible_categories: &BTreeSet<String>, selected_categories: &BTreeSet<String>, negated_categories: &BTreeSet<String>) -> Vec<Box<dyn View>> {
        let mut children = vec![];
        let Layout { x_height, padding, max_line_width, max_lines } = *layout;
        let background = TEXT_BUMP_SMALL[0];
        let vertical_space = self.rect.height() as i32 - max_lines as i32 * x_height;
        let baselines = divide(vertical_space, max_lines as i32 + 1);
        let categories_count = visible_categories.len();
        let lines_count = page.lines.len();
        let mut pos = pt!(self.rect.min.x + small_half(padding),
                          self.rect.min.y + small_half(baselines[0]));

        for (line_index, line) in page.lines.iter().enumerate() {

            let paddings = if line_index == lines_count - 1 && page.end_index == categories_count {
                vec![padding; line.items.len() + 1]
            } else {
                let horizontal_space = (line.items.len() as i32 - 1) * padding +
                                       max_line_width - line.width;
                let mut v = divide(horizontal_space, line.items.len() as i32 - 1);
                v.insert(0, padding);
                v.push(padding);
                v
            };

            let rect_height = big_half(baselines[line_index]) +
                              x_height + small_half(baselines[line_index + 1]);

            for (item_index, item) in line.items.iter().enumerate() {
                let left_padding = big_half(paddings[item_index]);
                let right_padding = small_half(paddings[item_index + 1]);
                let rect_width = left_padding + item.width() + right_padding;
                let sop = pos + pt!(rect_width, rect_height);

                match *item {
                    Item::Label { text, max_width, .. } => {
                        let status = if selected_categories.contains(text) {
                            Status::Selected
                        } else if negated_categories.contains(text) {
                            Status::Negated
                        } else {
                            Status::Normal
                        };
                        let child = Category::new(rect![pos, sop],
                                                  text.to_string(),
                                                  status,
                                                  Align::Left(left_padding),
                                                  max_width);
                        children.push(Box::new(child) as Box<dyn View>);
                    },
                    Item::Icon { name, .. } => {
                        let dir = if item_index == 0 { CycleDir::Previous } else { CycleDir::Next };
                        let child = Icon::new(name, rect![pos, sop],
                                              Event::Page(dir))
                                         .background(background)
                                         .align(Align::Left(left_padding));
                        children.push(Box::new(child) as Box<dyn View>);
                    }
                }

                pos.x += rect_width;
            }

            pos.x = self.rect.min.x + small_half(padding);
            pos.y += rect_height;
        }

        if page.end_index == categories_count {
            let last_width = page.lines[lines_count - 1].width;
            let x_offset = last_width + small_half(padding);
            let y_offset = baselines.iter().take(lines_count).sum::<i32>() -
                           big_half(baselines[lines_count - 1]) + (lines_count - 1) as i32 * x_height;
            let height = big_half(baselines[lines_count - 1]) + x_height + small_half(baselines[lines_count]);
            let filler = Filler::new(rect![pt!(self.rect.min.x + x_offset, self.rect.min.y + y_offset),
                                           pt!(self.rect.max.x - big_half(padding), self.rect.min.y + y_offset + height)],
                                     background);
            children.push(Box::new(filler) as Box<dyn View>);
        }

        if lines_count < max_lines {
            let y_offset = baselines.iter().take(lines_count+1).sum::<i32>() -
                           big_half(baselines[lines_count]) + lines_count as i32 * x_height;
            let filler = Filler::new(rect![pt!(self.rect.min.x + small_half(padding),
                                               self.rect.min.y + y_offset),
                                           pt!(self.rect.max.x - big_half(padding),
                                               self.rect.max.y - big_half(baselines[max_lines]))],
                                     background);
            children.push(Box::new(filler) as Box<dyn View>);
        }
        
        let filler = Filler::new(rect![self.rect.min,
                                       pt!(self.rect.max.x,
                                           self.rect.min.y + small_half(baselines[0]))],
                                 background);
        children.push(Box::new(filler) as Box<dyn View>);

        let filler = Filler::new(rect![pt!(self.rect.min.x,
                                           self.rect.min.y + small_half(baselines[0])),
                                       pt!(self.rect.min.x + small_half(padding),
                                           self.rect.max.y - big_half(baselines[max_lines]))],
                                 background);
        children.push(Box::new(filler) as Box<dyn View>);

        let filler = Filler::new(rect![pt!(self.rect.min.x, self.rect.max.y - big_half(baselines[max_lines])),
                                       self.rect.max],
                                 background);
        children.push(Box::new(filler) as Box<dyn View>);

        let filler = Filler::new(rect![pt!(self.rect.max.x - big_half(padding), self.rect.min.y + small_half(baselines[0])),
                                       pt!(self.rect.max.x, self.rect.max.y - big_half(baselines[max_lines]))],
                                 background);
        children.push(Box::new(filler) as Box<dyn View>);
        children
    }
}

impl View for Summary {
    fn handle_event(&mut self, evt: &Event, hub: &Hub, bus: &mut Bus, _context: &mut Context) -> bool {
        match *evt {
            Event::Gesture(GestureEvent::Swipe { dir, start, end, .. }) if self.rect.includes(start) => {
                match dir {
                    Dir::West => {
                        self.set_current_page(CycleDir::Next);
                        hub.send(Event::Render(self.rect, UpdateMode::Gui)).unwrap();
                        true
                    },
                    Dir::East => {
                        self.set_current_page(CycleDir::Previous);
                        hub.send(Event::Render(self.rect, UpdateMode::Gui)).unwrap();
                        true
                    },
                    Dir::South if !self.rect.includes(end) => {
                        bus.push_back(Event::ResizeSummary(end.y - self.rect.max.y));
                        true
                    },
                    _ => false,
                }
            },
            Event::Page(dir) => {
                self.set_current_page(dir);
                hub.send(Event::Render(self.rect, UpdateMode::Gui)).unwrap();
                true
            },
            _ => false,
        }
    }

    fn render(&self, _fb: &mut Framebuffer, _fonts: &mut Fonts) {}

    fn rect(&self) -> &Rectangle {
        &self.rect
    }

    fn rect_mut(&mut self) -> &mut Rectangle {
        &mut self.rect
    }

    fn children(&self) -> &Vec<Box<dyn View>> {
        &self.pages[self.current_page]
    }

    fn children_mut(&mut self) -> &mut Vec<Box<dyn View>> {
        &mut self.pages[self.current_page]
    }
}
