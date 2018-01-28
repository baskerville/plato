use std::thread;
use std::time::Duration;
use device::{CURRENT_DEVICE, BAR_SIZES};
use font::{Fonts, font_from_style, NORMAL_STYLE};
use geom::{Rectangle, CornerSpec, Dir, BorderSpec, small_half, big_half};
use gesture::GestureEvent;
use unit::scale_by_dpi;
use color::{BLACK, WHITE, SEPARATOR_NORMAL};
use framebuffer::{Framebuffer, UpdateMode};
use view::filler::Filler;
use view::menu_entry::MenuEntry;
use view::{View, Event, Hub, Bus, EntryKind, ViewId, CLOSE_IGNITION_DELAY_MS};
use view::{THICKNESS_MEDIUM, THICKNESS_LARGE, BORDER_RADIUS_MEDIUM};
use app::Context;

pub struct Menu {
    rect: Rectangle,
    children: Vec<Box<View>>,
    drop: bool,
    root: bool,
    id: ViewId,
    dir: i32,
}

// TOP MENU       C
//    ───         B
//  ↓  A       ↑  A            
//     B         ───
//     C     BOTTOM MENU

impl Menu {
    pub fn new(target: Rectangle, id: ViewId, drop: bool, mut entries: Vec<EntryKind>, fonts: &mut Fonts) -> Menu {
        let mut children = Vec::new();
        let dpi = CURRENT_DEVICE.dpi;
        let (width, height) = CURRENT_DEVICE.dims;
        let &(small_height, _) = BAR_SIZES.get(&(height, dpi)).unwrap();

        let thickness = scale_by_dpi(THICKNESS_MEDIUM, dpi) as i32;
        let border_thickness = scale_by_dpi(THICKNESS_LARGE, dpi) as i32;
        let font = font_from_style(fonts, &NORMAL_STYLE, dpi);
        let entry_height = font.x_heights.0 as i32 * 5;
        let padding = 4 * font.em() as i32;

        let north_space = target.min.y;
        let south_space = height as i32 - target.max.y;

        let (dir, y_start): (i32, i32) = if drop {
            if north_space < south_space {
                (1, target.max.y)
            } else {
                (-1, target.min.y)
            }
        } else {
            if north_space < south_space {
                (1, target.min.y - border_thickness)
            } else {
                (-1, target.max.y + border_thickness)
            }
        };

        let top_min = small_height as i32 + big_half(thickness);
        let bottom_max = height as i32 - small_height as i32 - small_half(thickness);

        let usable_space = if dir.is_positive() {
            bottom_max - y_start
        } else {
            y_start - top_min
        };

        let border_space = if drop {
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
            entries.push(EntryKind::SubMenu("More".to_string(), more));
        }

        let mut y_pos = y_start + dir * (border_space - border_thickness);

        let max_width = width as i32 / 2;
        let free_width = padding + 2 * border_thickness +
                         entries.iter().map(|e| font.plan(e.text(), None, None).width as i32)
                                .max().unwrap();

        let entry_width = free_width.min(max_width);

        let (mut x_min, mut x_max) = if drop {
            let center = target.center();
            (center.x - small_half(entry_width), center.x + big_half(entry_width))
        } else {
            let west_space = target.min.x;
            let east_space = width as i32 - target.max.x;
            if west_space > east_space {
                (target.min.x - entry_width, target.min.x)
            } else {
                (target.max.x, target.max.x + entry_width)
            }
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
                let separator = Filler::new(rect, SEPARATOR_NORMAL);
                children.push(Box::new(separator) as Box<View>);
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

                let entry_dir = if i == entries_count - 1 {
                    if dir.is_positive() {
                        Some(Dir::South)
                    } else {
                        Some(Dir::North)
                    }
                } else if !drop && i == 0 {
                    if dir.is_positive() {
                        Some(Dir::North)
                    } else {
                        Some(Dir::South)
                    }
                } else {
                    None
                };

                let menu_entry = MenuEntry::new(rect, entries[i].clone(), anchor, entry_dir);

                children.push(Box::new(menu_entry) as Box<View>);

                y_pos += dir * entry_height;
            }
        }

        let total_entries = entries.iter().filter(|e| !e.is_separator()).count();
        let menu_height = total_entries as i32 * entry_height + border_space;

        let (y_min, y_max) = if dir.is_positive() {
            (y_start, y_start + menu_height)
        } else {
            (y_start - menu_height, y_start)
        };

        let rect = rect![x_min, y_min,
                         x_max, y_max];

        Menu {
            rect,
            children,
            drop,
            root: true,
            id,
            dir,
        }
    }

    pub fn root(mut self, root: bool) -> Menu {
        self.root = root;
        self
    }
}

impl View for Menu {
    fn handle_event(&mut self, evt: &Event, hub: &Hub, bus: &mut Bus, context: &mut Context) -> bool {
        match *evt {
            Event::Select(entry_id) if self.root => {
                self.handle_event(&Event::PropagateSelect(entry_id), hub, bus, context);
                false
            },
            Event::PropagateSelect(..) => {
                for c in &mut self.children {
                    if c.handle_event(evt, hub, bus, context) {
                        break;
                    }
                }
                true
            },
            Event::Validate => {
                let hub2 = hub.clone();
                let id = self.id;
                thread::spawn(move || {
                    thread::sleep(Duration::from_millis(CLOSE_IGNITION_DELAY_MS));
                    hub2.send(Event::Close(id)).unwrap();
                });
                self.drop
            },
            Event::Gesture(GestureEvent::Tap { ref center, .. }) if !self.rect.includes(center) => {
                hub.send(Event::Close(self.id)).unwrap();
                false
            },
            Event::Gesture(GestureEvent::HoldFinger(ref center)) if !self.rect.includes(center) => false,
            Event::SubMenu(rect, ref entries) => {
                let menu = Menu::new(rect, self.id, false, entries.clone(), &mut context.fonts).root(false);
                hub.send(Event::Render(*menu.rect(), UpdateMode::Gui)).unwrap();
                self.children.push(Box::new(menu) as Box<View>);
                true
            },
            Event::Gesture(..) => true,
            _ => false,
        }
    }

    fn render(&self, fb: &mut Framebuffer, _fonts: &mut Fonts) {
        let dpi = CURRENT_DEVICE.dpi;
        let border_radius = scale_by_dpi(BORDER_RADIUS_MEDIUM, dpi) as i32;
        let border_thickness = scale_by_dpi(THICKNESS_LARGE, dpi) as u16;
        let corners = if self.drop {
            if self.dir.is_positive() {
                CornerSpec::South(border_radius)
            } else {
                CornerSpec::North(border_radius)
            }
        } else {
            CornerSpec::Uniform(border_radius)
        };
        fb.draw_rounded_rectangle_with_border(&self.rect,
                                              &corners,
                                              &BorderSpec { thickness: border_thickness,
                                                            color: BLACK },
                                              &WHITE);

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

    fn children(&self) -> &Vec<Box<View>> {
        &self.children
    }

    fn children_mut(&mut self) -> &mut Vec<Box<View>> {
        &mut self.children
    }

    fn id(&self) -> Option<ViewId> {
        Some(self.id)
    }
}
