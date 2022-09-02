use std::path::{PathBuf, Path};
use std::collections::BTreeSet;
use fxhash::FxHashMap;
use crate::device::CURRENT_DEVICE;
use crate::framebuffer::{Framebuffer, UpdateMode};
use crate::view::{View, Event, Hub, Bus, Id, ID_FEEDER, RenderQueue, RenderData};
use crate::view::{SMALL_BAR_HEIGHT, THICKNESS_MEDIUM};
use crate::unit::scale_by_dpi;
use crate::view::filler::Filler;
use super::directories_bar::DirectoriesBar;
use crate::gesture::GestureEvent;
use crate::color::SEPARATOR_NORMAL;
use crate::context::Context;
use crate::font::{Fonts, font_from_style, NORMAL_STYLE};
use crate::geom::{Point, Rectangle, Dir};

#[derive(Debug)]
pub struct NavigationBar {
    id: Id,
    pub rect: Rectangle,
    children: Vec<Box<dyn View>>,
    path: PathBuf,
    pub vertical_limit: i32,
    max_levels: usize,
}

impl NavigationBar {
    pub fn new(rect: Rectangle, vertical_limit: i32, max_levels: usize) -> NavigationBar {
        NavigationBar {
            id: ID_FEEDER.next(),
            rect,
            children: Vec::new(),
            path: PathBuf::default(),
            vertical_limit,
            max_levels,
        }
    }

    pub fn clear(&mut self) {
        self.children.clear();
    }

    pub fn set_path<P: AsRef<Path>>(&mut self, path: P, path_dirs: &BTreeSet<PathBuf>, rq: &mut RenderQueue, context: &mut Context) {
        let dpi = CURRENT_DEVICE.dpi;
        let thickness = scale_by_dpi(THICKNESS_MEDIUM, dpi) as i32;
        let min_height = scale_by_dpi(SMALL_BAR_HEIGHT, dpi) as i32 - thickness;
        let font = font_from_style(&mut context.fonts, &NORMAL_STYLE, dpi);
        let x_height = font.x_heights.0 as i32;
        let padding = min_height - x_height;

        let first = self.children.first()
                        .and_then(|child| child.downcast_ref::<DirectoriesBar>())
                        .map(|dirs_bar| dirs_bar.path.clone())
                        .unwrap_or_else(PathBuf::default);
        let mut last = self.children.last()
                           .and_then(|child| child.downcast_ref::<DirectoriesBar>())
                           .map(|dirs_bar| dirs_bar.path.clone())
                           .unwrap_or_else(PathBuf::default);

        // Remove the trailing children.
        if let Some((leftovers_count, ancestor)) = last.ancestors().enumerate()
                                                       .find(|(_, anc)| path.as_ref().starts_with(anc))
                                                       .filter(|(n, _)| *n > 0) {
            self.children.drain(self.children.len().saturating_sub(2*leftovers_count)..);
            last = ancestor.to_path_buf();
        }

        let mut dirs_from_path = FxHashMap::default();
        let mut current: &Path = path.as_ref();
        let mut y_max = self.vertical_limit;
        let mut index = self.children.len();
        let mut levels = 1;

        loop {
            // Re-use and move existing bars.
            if last.starts_with(current) && current.starts_with(&first) {
                let db_index = index - 1;
                let sep_index = index.saturating_sub(2);

                let y_shift = y_max - self.children[db_index].rect().max.y;

                if let Some(dirs_bar) = self.children[db_index].downcast_mut::<DirectoriesBar>() {
                    dirs_bar.shift(pt!(0, y_shift));
                }

                if self.children[db_index].rect().min.y < self.rect.min.y {
                    break;
                }

                levels += 1;

                y_max -= self.children[db_index].rect().height() as i32;

                if sep_index != db_index {
                    let y_shift = y_max - self.children[sep_index].rect().max.y;
                    *self.children[sep_index].rect_mut() += pt!(0, y_shift);
                    y_max -= self.children[sep_index].rect().height() as i32;
                }

                index = sep_index;
            // Or insert new bars.
            } else if current != path.as_ref() || !path_dirs.is_empty() {
                let count = if current == path.as_ref() {
                    guess_bar_size(path_dirs)
                } else {
                    let (_, dirs) = context.library.list(current, None, true);
                    let count = guess_bar_size(&dirs);
                    dirs_from_path.insert(current, dirs);
                    count
                } as i32;

                let height = count * x_height + (count + 1) * padding / 2;
                if y_max - height - thickness < self.rect.min.y {
                    break;
                }

                levels += 1;

                if self.children.get(index).map_or(true, |child| child.is::<Filler>()) {
                    let rect = rect![self.rect.min.x, y_max - height,
                                     self.rect.max.x, y_max];
                    self.children.insert(index, Box::new(DirectoriesBar::new(rect, current)));
                    y_max -= height;

                    let rect = rect![self.rect.min.x, y_max - thickness,
                                     self.rect.max.x, y_max];
                    self.children.insert(index, Box::new(Filler::new(rect, SEPARATOR_NORMAL)));
                    y_max -= thickness;
                } else {
                    let rect = rect![self.rect.min.x, y_max - thickness,
                                     self.rect.max.x, y_max];
                    self.children.insert(index, Box::new(Filler::new(rect, SEPARATOR_NORMAL)));
                    y_max -= thickness;

                    let rect = rect![self.rect.min.x, y_max - height,
                                     self.rect.max.x, y_max];
                    self.children.insert(index, Box::new(DirectoriesBar::new(rect, current)));
                    y_max -= height;
                }
            }

            if levels > self.max_levels || current == context.library.home {
                break;
            }

            if let Some(parent) = current.parent() {
                current = parent;
            } else {
                break;
            }
        }

        self.children.drain(..index);

        if self.children.is_empty() {
            let rect = rect![self.rect.min.x, self.rect.min.y,
                             self.rect.max.x, self.rect.min.y + min_height];
            self.children.push(Box::new(DirectoriesBar::new(rect, path.as_ref())));
        }

        // Remove the extra separator.
        if self.children.len() % 2 == 0 {
            self.children.remove(0);
        }

        // Move and populate the children.
        current = if path_dirs.is_empty() && path.as_ref() != context.library.home {
            path.as_ref().parent().unwrap_or_else(|| Path::new(""))
        } else {
            path.as_ref()
        };

        let y_shift = self.rect.min.y - self.children[0].rect().min.y;
        index = self.children.len() - 1;

        loop {
            if index % 2 == 0 {
                let dirs_bar = self.children[index].downcast_mut::<DirectoriesBar>().unwrap();
                dirs_bar.shift(pt!(0, y_shift));
                if !last.starts_with(current) || !current.starts_with(&first) {
                    if current == path.as_ref() {
                        dirs_bar.update_content(path_dirs, path.as_ref(), &mut context.fonts);
                    } else {
                        if let Some(dirs) = dirs_from_path.remove(&current) {
                            dirs_bar.update_content(&dirs, path.as_ref(), &mut context.fonts);
                        }
                    }
                } else if current == last {
                    dirs_bar.update_selected(path.as_ref());
                }

                if let Some(parent) = current.parent() {
                    current = parent;
                } else {
                    break;
                }
            } else {
                *self.children[index].rect_mut() += pt!(0, y_shift);
            }

            if index == 0 {
                break;
            }

            index -= 1;
        }

        self.rect.max.y = self.children[self.children.len()-1].rect().max.y;
        rq.add(RenderData::new(self.id, self.rect, UpdateMode::Partial));
        self.path = path.as_ref().to_path_buf();
    }

    pub fn shift(&mut self, delta: Point) {
        for child in &mut self.children {
            if let Some(dirs_bar) = child.downcast_mut::<DirectoriesBar>() {
                dirs_bar.shift(delta);
            } else {
                *child.rect_mut() += delta;
            }
        }
        self.rect += delta;
    }


    pub fn shrink(&mut self, delta_y: i32, fonts: &mut Fonts) -> i32 {
        let dpi = CURRENT_DEVICE.dpi;
        let thickness = scale_by_dpi(THICKNESS_MEDIUM, dpi) as i32;
        let min_height = scale_by_dpi(SMALL_BAR_HEIGHT, dpi) as i32 - thickness;
        let bars_count = (self.children.len()+1)/2;
        let mut values = vec![0; bars_count];

        for (i, value) in values.iter_mut().enumerate().take(bars_count) {
            *value = self.children[2*i].rect().height() as i32 - min_height;
        }

        let sum: i32 = values.iter().sum();
        let mut y_shift = 0;

        // Try to proportionally shrink each children.
        if sum > 0 {
            for i in (0..bars_count).rev() {
                let local_delta_y = ((values[i] as f32 / sum as f32) * delta_y as f32) as i32;
                y_shift += self.resize_child(2*i, local_delta_y, fonts);
                if y_shift <= delta_y {
                    break;
                }
            }
        }

        // If it wasn't enough, remove some children at the beginning.
        while self.children.len() > 1 && y_shift > delta_y {
            let mut dy = 0;

            for child in self.children.drain(0..2) {
                dy += child.rect().height() as i32;
            }

            for child in &mut self.children {
                if let Some(dirs_bar) = child.downcast_mut::<DirectoriesBar>() {
                    dirs_bar.shift(pt!(0, -dy));
                } else {
                    *child.rect_mut() += pt!(0, -dy);
                }
            }

            y_shift -= dy;
        }

        self.rect.max.y = self.children[self.children.len()-1].rect().max.y;

        y_shift
    }

    fn resize_child(&mut self, index: usize, delta_y: i32, fonts: &mut Fonts) -> i32 {
        let dpi = CURRENT_DEVICE.dpi;
        let thickness = scale_by_dpi(THICKNESS_MEDIUM, dpi) as i32;
        let min_height = scale_by_dpi(SMALL_BAR_HEIGHT, dpi) as i32 - thickness;
        let font = font_from_style(fonts, &NORMAL_STYLE, dpi);

        let rect = *self.children[index].rect();
        let delta_y_max = (self.vertical_limit - self.rect.max.y).max(0);
        let y_max = (rect.max.y + delta_y.min(delta_y_max)).max(rect.min.y + min_height);
        let x_height = font.x_heights.0 as i32;
        let padding = min_height - x_height;
        let height = y_max - rect.min.y;

        let count = (height - padding / 2) / (x_height + padding / 2);
        let height = count * x_height + (count + 1) * padding / 2;
        let y_max = rect.min.y + height;

        self.children[index].rect_mut().max.y = y_max;
        let y_shift = y_max - rect.max.y;

        let dirs_bar = self.children[index].downcast_mut::<DirectoriesBar>().unwrap();
        let dirs = dirs_bar.dirs();
        dirs_bar.update_content(&dirs, self.path.as_ref(), fonts);

        // Shift all the children after index.
        for i in index+1..self.children.len() {
            if let Some(dirs_bar) = self.children[i].downcast_mut::<DirectoriesBar>() {
                dirs_bar.shift(pt!(0, y_shift));
            } else {
                *self.children[i].rect_mut() += pt!(0, y_shift);
            }
        }

        // Move our own bottom edge.
        self.rect.max.y = self.children[self.children.len()-1].rect().max.y;

        y_shift
    }
}

#[inline]
fn guess_bar_size(dirs: &BTreeSet<PathBuf>) -> usize {
    (dirs.iter().map(|dir| dir.as_os_str().len())
         .sum::<usize>()/300).clamp(1, 4)
}

impl View for NavigationBar {
    fn handle_event(&mut self, evt: &Event, _hub: &Hub, bus: &mut Bus, _rq: &mut RenderQueue, context: &mut Context) -> bool {
        match *evt {
            Event::Gesture(GestureEvent::Swipe { dir, start, end, .. }) if self.rect.includes(start) || self.rect.includes(end) => {
                match dir {
                    Dir::North | Dir::South => {
                        let pt = if dir == Dir::North { end } else { start };
                        if let Some(index) = self.children.iter().position(|child| child.is::<DirectoriesBar>() && child.rect().includes(pt)) {
                            // Move the bottom edge of the child by end.y - start.y.
                            // Shift all the children after the child.
                            let y_shift = self.resize_child(index, end.y - start.y, &mut context.fonts);
                            bus.push_back(Event::NavigationBarResized(y_shift));
                        }
                        true
                    },
                    _ => false,
                }
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
        &self.children
    }

    fn children_mut(&mut self) -> &mut Vec<Box<dyn View>> {
        &mut self.children
    }

    fn id(&self) -> Id {
        self.id
    }
}
