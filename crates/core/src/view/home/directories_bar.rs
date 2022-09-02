use std::path::{PathBuf, Path};
use crate::device::CURRENT_DEVICE;
use std::collections::BTreeSet;
use crate::framebuffer::{Framebuffer, UpdateMode};
use crate::view::{View, Event, Hub, Bus, Id, ID_FEEDER, RenderQueue, RenderData, Align};
use crate::view::icon::{Icon, ICONS_PIXMAPS};
use crate::view::{SMALL_BAR_HEIGHT, THICKNESS_MEDIUM};
use crate::view::filler::Filler;
use super::directory::Directory;
use crate::gesture::GestureEvent;
use crate::font::{Font, Fonts, font_from_style, NORMAL_STYLE};
use crate::geom::{Point, Rectangle, Dir, CycleDir, divide, small_half, big_half};
use crate::color::TEXT_BUMP_SMALL;
use crate::unit::scale_by_dpi;
use crate::context::Context;

#[derive(Debug)]
pub struct DirectoriesBar {
    id: Id,
    pub rect: Rectangle,
    pub path: PathBuf,
    pages: Vec<Vec<Box<dyn View>>>,
    selection_page: Option<usize>,
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
            lines: Vec::new(),
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
            items: Vec::new(),
        }
    }
}

#[derive(Debug, Clone)]
enum Item<'a> {
    Label { path: &'a Path, width: i32, max_width: Option<i32> },
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

impl DirectoriesBar {
    pub fn new<P: AsRef<Path>>(rect: Rectangle, path: P) -> DirectoriesBar {
        DirectoriesBar {
            id: ID_FEEDER.next(),
            rect,
            path: path.as_ref().to_path_buf(),
            current_page: 0,
            selection_page: None,
            pages: vec![Vec::new()],
        }
    }

    pub fn shift(&mut self, delta: Point) {
        for children in &mut self.pages {
            for child in children {
                *child.rect_mut() += delta;
            }
        }
        self.rect += delta;
    }

    pub fn dirs(&self) -> BTreeSet<PathBuf> {
        self.pages.iter().flatten()
            .filter_map(|child| child.downcast_ref::<Directory>())
            .map(|dir| &dir.path)
            .cloned()
            .collect()
    }

    pub fn go_to_page(&mut self, index: usize) {
        self.current_page = index;
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

    pub fn update_selected(&mut self, current_directory: &Path) {
        for (index, children) in self.pages.iter_mut().enumerate() {
            for child in children.iter_mut() {
                if let Some(dir) = child.downcast_mut::<Directory>() {
                    if dir.update_selected(current_directory) {
                        self.current_page = index;
                        self.selection_page = Some(index);
                    }
                }
            }
        }
    }

    pub fn update_content(&mut self, directories: &BTreeSet<PathBuf>, current_directory: &Path, fonts: &mut Fonts) {
        let dpi = CURRENT_DEVICE.dpi;
        let thickness = scale_by_dpi(THICKNESS_MEDIUM, dpi) as i32;
        let min_height = scale_by_dpi(SMALL_BAR_HEIGHT, dpi) as i32 - thickness;
        let mut start_index = 0;
        let mut font = font_from_style(fonts, &NORMAL_STYLE, dpi);
        let x_height = font.x_heights.0 as i32;
        let padding = font.em() as i32;
        let vertical_padding = min_height - x_height;
        let max_line_width = self.rect.width() as i32 - 2 * padding;
        let max_lines = ((self.rect.height() as i32 - vertical_padding / 2) /
                         (x_height + vertical_padding / 2)) as usize;
        let layout = Layout { x_height, padding, max_line_width, max_lines };
        
        let pages_count = self.pages.len();
        self.pages.clear();
        self.selection_page = None;

        loop {
            let mut has_selection = false;
            let (children, end_index) = {
                let page = self.make_page(start_index, &layout, directories, &mut font);
                let children = self.make_children(&page, &layout, current_directory, directories, &mut has_selection);
                (children, page.end_index)
            };
            if has_selection {
                self.selection_page = Some(self.pages.len());
            }
            self.pages.push(children);
            if end_index == directories.len() {
                break;
            }
            start_index = end_index;
        }

        let previous_position = if pages_count > 0 {
            self.current_page as f32 / pages_count as f32
        } else {
            0.0
        };

        self.current_page = self.selection_page.unwrap_or_else(|| {
            (previous_position * self.pages.len() as f32) as usize
        });
    }

    fn make_page<'a>(&self, start_index: usize, layout: &Layout, directories: &'a BTreeSet<PathBuf>, font: &mut Font) -> Page<'a> {
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

        for dir in directories.iter().skip(start_index) {
            let mut dir_width = font.plan(dir.file_name().unwrap().to_string_lossy(),
                                          None,
                                          None).width;
            let mut max_dir_width = None;

            if dir_width > max_line_width {
                max_dir_width = Some(max_line_width);
                dir_width = max_line_width;
            }

            line.labels_count += 1;
            line.width += dir_width;
            end_index += 1;
            let label = Item::Label { path: dir.as_path(),
                                      width: dir_width,
                                      max_width: max_dir_width };
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
                            *max_width = Some(*width);
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

        if end_index < directories.len() {

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
                        *max_width = Some(*width);
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

    fn make_children(&self, page: &Page, layout: &Layout, current_directory: &Path, directories: &BTreeSet<PathBuf>, has_selection: &mut bool) -> Vec<Box<dyn View>> {
        let mut children = Vec::new();
        let Layout { x_height, padding, max_line_width, max_lines } = *layout;
        let background = TEXT_BUMP_SMALL[0];
        let vertical_space = self.rect.height() as i32 - max_lines as i32 * x_height;
        let baselines = divide(vertical_space, max_lines as i32 + 1);
        let directories_count = directories.len();
        let lines_count = page.lines.len();
        let mut pos = pt!(self.rect.min.x + small_half(padding),
                          self.rect.min.y + small_half(baselines[0]));

        // Top filler.
        let filler = Filler::new(rect![self.rect.min,
                                       pt!(self.rect.max.x,
                                           self.rect.min.y + small_half(baselines[0]))],
                                 background);
        children.push(Box::new(filler) as Box<dyn View>);

        // Left filler.
        let filler = Filler::new(rect![pt!(self.rect.min.x,
                                           self.rect.min.y + small_half(baselines[0])),
                                       pt!(self.rect.min.x + small_half(padding),
                                           self.rect.max.y - big_half(baselines[max_lines]))],
                                 background);
        children.push(Box::new(filler) as Box<dyn View>);


        for (line_index, line) in page.lines.iter().enumerate() {

            let paddings = if line_index == lines_count - 1 && page.end_index == directories_count {
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
                    Item::Label { path, max_width, .. } => {
                        let selected = current_directory.starts_with(path);
                        if selected {
                            *has_selection = true;
                        }
                        let child = Directory::new(rect![pos, sop],
                                                   path.to_path_buf(),
                                                   selected,
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

        // End of line filler.
        if page.end_index == directories_count {
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

        // End of page filler.
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
        
        // Right filler.
        let filler = Filler::new(rect![pt!(self.rect.max.x - big_half(padding), self.rect.min.y + small_half(baselines[0])),
                                       pt!(self.rect.max.x, self.rect.max.y - big_half(baselines[max_lines]))],
                                 background);
        children.push(Box::new(filler) as Box<dyn View>);

        // Bottom filler.
        let filler = Filler::new(rect![pt!(self.rect.min.x, self.rect.max.y - big_half(baselines[max_lines])),
                                       self.rect.max],
                                 background);
        children.push(Box::new(filler) as Box<dyn View>);

        children
    }
}

impl View for DirectoriesBar {
    fn handle_event(&mut self, evt: &Event, _hub: &Hub, _bus: &mut Bus, rq: &mut RenderQueue, _context: &mut Context) -> bool {
        match *evt {
            Event::Gesture(GestureEvent::Swipe { dir, start, .. }) if self.rect.includes(start) => {
                match dir {
                    Dir::West => {
                        self.set_current_page(CycleDir::Next);
                        rq.add(RenderData::new(self.id, self.rect, UpdateMode::Gui));
                        true
                    },
                    Dir::East => {
                        self.set_current_page(CycleDir::Previous);
                        rq.add(RenderData::new(self.id, self.rect, UpdateMode::Gui));
                        true
                    },
                    _ => false,
                }
            },
            Event::Page(dir) => {
                let current_page = self.current_page;
                self.set_current_page(dir);
                if self.current_page != current_page {
                    rq.add(RenderData::new(self.id, self.rect, UpdateMode::Gui));
                }
                true
            },
            Event::Chapter(dir) => {
                let pages_count = self.pages.len();
                if pages_count > 1 {
                    let current_page = self.current_page;
                    match dir {
                        CycleDir::Previous => self.go_to_page(0),
                        CycleDir::Next => self.go_to_page(pages_count - 1),
                    }
                    if self.current_page != current_page {
                        let page = &mut self.pages[current_page];
                        let index = match dir {
                            CycleDir::Previous => 2,
                            CycleDir::Next => page.len() - 3,
                        };
                        if let Some(icon) = page[index].downcast_mut::<Icon>() {
                            icon.active = false;
                        } else {
                            println!("oups");
                        }
                        rq.add(RenderData::new(self.id, self.rect, UpdateMode::Gui));
                    }
                }
                true
            },
            _ => false,
        }
    }

    fn render(&self, _fb: &mut dyn Framebuffer, _rect: Rectangle, _fonts: &mut Fonts) {
    }

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

    fn id(&self) -> Id {
        self.id
    }
}
