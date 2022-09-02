use std::thread;
use crate::device::CURRENT_DEVICE;
use crate::font::{Fonts, font_from_style, NORMAL_STYLE};
use crate::geom::{Point, Rectangle, CornerSpec, BorderSpec, small_half, big_half};
use crate::gesture::GestureEvent;
use crate::unit::scale_by_dpi;
use crate::color::{BLACK, WHITE, SEPARATOR_NORMAL, SEPARATOR_STRONG};
use crate::framebuffer::{Framebuffer, UpdateMode};
use super::filler::Filler;
use super::menu_entry::MenuEntry;
use super::common::locate_by_id;
use super::{View, Event, Hub, Bus, RenderQueue, RenderData};
use super::{EntryKind, ViewId, Id, ID_FEEDER, CLOSE_IGNITION_DELAY};
use super::{SMALL_BAR_HEIGHT, THICKNESS_MEDIUM, THICKNESS_LARGE, BORDER_RADIUS_MEDIUM};
use crate::context::Context;

pub struct Menu {
    id: Id,
    rect: Rectangle,
    children: Vec<Box<dyn View>>,
    view_id: ViewId,
    kind: MenuKind,
    center: Point,
    root: bool,
    sub_id: u8,
    dir: i32,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum MenuKind {
    DropDown,
    SubMenu,
    Contextual,
}

// TOP MENU       C
//    ───         B
//  ↓  A       ↑  A            
//     B         ───
//     C     BOTTOM MENU

impl Menu {
    pub fn new(target: Rectangle, view_id: ViewId, kind: MenuKind, mut entries: Vec<EntryKind>, context: &mut Context) -> Menu {
        let id = ID_FEEDER.next();
        let mut children = Vec::new();
        let dpi = CURRENT_DEVICE.dpi;
        let (width, height) = context.display.dims;
        let small_height = scale_by_dpi(SMALL_BAR_HEIGHT, dpi) as i32;

        let thickness = scale_by_dpi(THICKNESS_MEDIUM, dpi) as i32;
        let border_thickness = scale_by_dpi(THICKNESS_LARGE, dpi) as i32;
        let border_radius = scale_by_dpi(BORDER_RADIUS_MEDIUM - THICKNESS_LARGE, dpi) as i32;

        let sep_color = if context.fb.monochrome() { SEPARATOR_STRONG } else { SEPARATOR_NORMAL };
        let font = font_from_style(&mut context.fonts, &NORMAL_STYLE, dpi);
        let entry_height = font.x_heights.0 as i32 * 5;
        let padding = 4 * font.em() as i32;

        let north_space = target.min.y;
        let south_space = height as i32 - target.max.y;
        let center = target.center();

        let (dir, y_start): (i32, i32) = if kind == MenuKind::SubMenu {
            if north_space < south_space {
                (1, target.min.y - border_thickness)
            } else {
                (-1, target.max.y + border_thickness)
            }
        } else {
            if north_space < south_space {
                (1, target.max.y)
            } else {
                (-1, target.min.y)
            }
        };

        let top_min = small_height + big_half(thickness);
        let bottom_max = height as i32 - small_height - small_half(thickness);

        let usable_space = if dir.is_positive() {
            bottom_max - y_start
        } else {
            y_start - top_min
        };

        let border_space = if kind == MenuKind::DropDown {
            border_thickness
        } else {
            2 * border_thickness
        };

        let max_entries = ((usable_space - border_space) / entry_height) as usize;
        let total_entries = entries.iter().filter(|e| !e.is_separator()).count();

        if total_entries > max_entries {
            let mut kind_counts = [0, 0];
            for e in &entries {
                kind_counts[e.is_separator() as usize] += 1;
                if kind_counts[0] >= max_entries {
                    break;
                }
            }
            let index = kind_counts[0] + kind_counts[1] - 1;
            let more = entries.drain(index..).collect::<Vec<EntryKind>>();
            entries.push(EntryKind::More(more));
        }

        let mut y_pos = y_start + dir * (border_space - border_thickness);

        let max_width = 2 * width as i32 / 3;
        let free_width = padding + 2 * border_thickness +
                         entries.iter().map(|e| font.plan(e.text(), None, None).width)
                                .max().unwrap();

        let entry_width = free_width.min(max_width);

        let (mut x_min, mut x_max) = if kind == MenuKind::SubMenu {
            let west_space = target.min.x;
            let east_space = width as i32 - target.max.x;
            if west_space > east_space {
                (target.min.x - entry_width, target.min.x)
            } else {
                (target.max.x, target.max.x + entry_width)
            }
        } else {
            (center.x - small_half(entry_width), center.x + big_half(entry_width))
        };

        if x_min < 0 {
            x_max -= x_min;
            x_min = 0;
        }

        if x_max > width as i32 {
            x_min += width as i32 - x_max;
            x_max = width as i32;
        }

        let entries_count = entries.len();

        for i in 0..entries_count {
            if entries[i].is_separator() {
                let rect = rect![x_min + border_thickness, y_pos - small_half(thickness),
                                 x_max - border_thickness, y_pos + big_half(thickness)];
                let separator = Filler::new(rect, sep_color);
                children.push(Box::new(separator) as Box<dyn View>);
            } else {
                let (y_min, y_max) = if dir.is_positive() {
                    (y_pos, y_pos + entry_height)
                } else {
                    (y_pos - entry_height, y_pos)
                };

                let mut rect = rect![x_min + border_thickness, y_min,
                                     x_max - border_thickness, y_max];

                let anchor = rect;

                if i > 0 && entries[i - 1].is_separator() {
                    if dir.is_positive() {
                        rect.min.y += big_half(thickness);
                    } else {
                        rect.max.y -= small_half(thickness);
                    }
                }

                if i < entries_count - 1 && entries[i + 1].is_separator() {
                    if dir.is_positive() {
                        rect.max.y -= small_half(thickness);
                    } else {
                        rect.min.y += big_half(thickness);
                    }
                }

                let corner_spec = if kind != MenuKind::DropDown && entries_count == 1 {
                    Some(CornerSpec::Uniform(border_radius))
                } else if i == entries_count - 1 {
                    if dir.is_positive() {
                        Some(CornerSpec::South(border_radius))
                    } else {
                        Some(CornerSpec::North(border_radius))
                    }
                } else if kind != MenuKind::DropDown && i == 0 {
                    if dir.is_positive() {
                        Some(CornerSpec::North(border_radius))
                    } else {
                        Some(CornerSpec::South(border_radius))
                    }
                } else {
                    None
                };

                let menu_entry = MenuEntry::new(rect, entries[i].clone(), anchor, corner_spec);

                children.push(Box::new(menu_entry) as Box<dyn View>);

                y_pos += dir * entry_height;
            }
        }

        let triangle_space = if kind == MenuKind::Contextual {
            font.x_heights.1 as i32
        } else {
            0
        };

        let total_entries = entries.iter().filter(|e| !e.is_separator()).count();
        let menu_height = total_entries as i32 * entry_height + border_space;

        let (y_min, y_max) = if dir.is_positive() {
            (y_start - triangle_space, y_start + menu_height)
        } else {
            (y_start - menu_height, y_start + triangle_space)
        };

        let rect = rect![x_min, y_min,
                         x_max, y_max];

        Menu {
            id,
            rect,
            children,
            view_id,
            kind,
            center,
            root: true,
            sub_id: 0,
            dir,
        }
    }

    pub fn root(mut self, root: bool) -> Menu {
        self.root = root;
        self
    }
}

impl View for Menu {
    fn handle_event(&mut self, evt: &Event, hub: &Hub, bus: &mut Bus, rq: &mut RenderQueue, context: &mut Context) -> bool {
        match *evt {
            Event::Select(ref entry_id) if self.root => {
                self.handle_event(&Event::PropagateSelect(entry_id.clone()), hub, bus, rq, context);
                false
            },
            Event::PropagateSelect(..) => {
                for c in &mut self.children {
                    if c.handle_event(evt, hub, bus, rq, context) {
                        break;
                    }
                }
                true
            },
            Event::Validate if self.root => {
                let hub2 = hub.clone();
                let view_id = self.view_id;
                thread::spawn(move || {
                    thread::sleep(CLOSE_IGNITION_DELAY);
                    hub2.send(Event::Close(view_id)).ok();
                });
                true
            },
            Event::Gesture(GestureEvent::Tap(center)) if !self.rect.includes(center) => {
                if self.root {
                    bus.push_back(Event::Close(self.view_id));
                } else {
                    bus.push_back(Event::CloseSub(self.view_id));
                }
                self.root
            },
            Event::Gesture(GestureEvent::HoldFingerShort(center, ..)) if !self.rect.includes(center) => self.root,
            Event::SubMenu(rect, ref entries) => {
                let menu = Menu::new(rect, ViewId::SubMenu(self.sub_id),
                                     MenuKind::SubMenu, entries.clone(), context).root(false);
                rq.add(RenderData::new(menu.id(), *menu.rect(), UpdateMode::Gui));
                self.children.push(Box::new(menu) as Box<dyn View>);
                self.sub_id = self.sub_id.wrapping_add(1);
                true
            },
            Event::CloseSub(id) => {
                if let Some(index) = locate_by_id(self, id) {
                    rq.add(RenderData::expose(*self.children[index].rect(), UpdateMode::Gui));
                    self.children.remove(index);
                }
                true
            },
            Event::Gesture(..) => true,
            _ => false,
        }
    }

    fn render(&self, fb: &mut dyn Framebuffer, _rect: Rectangle, fonts: &mut Fonts) {
        let dpi = CURRENT_DEVICE.dpi;
        let border_radius = scale_by_dpi(BORDER_RADIUS_MEDIUM, dpi) as i32;
        let border_thickness = scale_by_dpi(THICKNESS_LARGE, dpi) as u16;

        let corners = if self.kind == MenuKind::DropDown {
            if self.dir.is_positive() {
                CornerSpec::South(border_radius)
            } else {
                CornerSpec::North(border_radius)
            }
        } else {
            CornerSpec::Uniform(border_radius)
        };

        if self.kind == MenuKind::Contextual {
            let font = font_from_style(fonts, &NORMAL_STYLE, dpi);
            let triangle_space = font.x_heights.1 as i32;
            let mut rect = self.rect;

            if self.dir.is_positive() {
                rect.min.y += triangle_space
            } else {
                rect.max.y -= triangle_space
            }

            fb.draw_rounded_rectangle_with_border(&rect,
                                                  &corners,
                                                  &BorderSpec { thickness: border_thickness,
                                                                color: BLACK },
                                                  &WHITE);

            let y_b = if self.dir.is_positive() {
                self.rect.min.y
            } else {
                self.rect.max.y - 1
            };

            let side = triangle_space + border_thickness as i32;
            let x_b = self.center.x.max(rect.min.x + 2 * side)
                                   .min(rect.max.x - 2 * side);

            let mut b = pt!(x_b, y_b);
            let mut a = b + pt!(-side, self.dir * side);
            let mut c = a + pt!(2 * side, 0);

            fb.draw_triangle(&[a, b, c], BLACK);
            let drift = (border_thickness as f32 * ::std::f32::consts::SQRT_2) as i32;

            b += pt!(0, self.dir * drift);
            a += pt!(drift, 0);
            c -= pt!(drift, 0);

            fb.draw_triangle(&[a, b, c], WHITE);
        } else {
            fb.draw_rounded_rectangle_with_border(&self.rect,
                                                  &corners,
                                                  &BorderSpec { thickness: border_thickness,
                                                                color: BLACK },
                                                  &WHITE);
        }
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

    fn view_id(&self) -> Option<ViewId> {
        Some(self.view_id)
    }
}
