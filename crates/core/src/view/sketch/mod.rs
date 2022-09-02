use std::fs::{self, File};
use std::path::PathBuf;
use fxhash::FxHashMap;
use chrono::Local;
use walkdir::WalkDir;
use globset::Glob;
use anyhow::Error;
use crate::device::CURRENT_DEVICE;
use crate::geom::{Point, Rectangle, CornerSpec};
use crate::input::{DeviceEvent, FingerStatus};
use crate::view::icon::{Icon, ICONS_PIXMAPS};
use crate::view::notification::Notification;
use crate::view::menu::{Menu, MenuKind};
use crate::view::common::{locate_by_id};
use crate::view::{View, Event, Hub, Bus, RenderQueue, RenderData};
use crate::view::{EntryKind, EntryId, ViewId, Id, ID_FEEDER};
use crate::view::{SMALL_BAR_HEIGHT, BORDER_RADIUS_SMALL};
use crate::framebuffer::{Framebuffer, UpdateMode, Pixmap};
use crate::settings::{ImportSettings, Pen};
use crate::helpers::IsHidden;
use crate::font::Fonts;
use crate::unit::scale_by_dpi;
use crate::color::{BLACK, WHITE};
use crate::context::Context;

const FILENAME_PATTERN: &str = "sketch-%Y%m%d_%H%M%S.png";
const ICON_NAME: &str = "enclosed_menu";
// https://oeis.org/A000041
const PEN_SIZES: [i32; 12] = [1, 2, 3, 5, 7, 11, 15, 22, 30, 42, 56, 77];

struct TouchState {
    pt: Point,
    time: f64,
    radius: f32,
}

impl TouchState {
    fn new(pt: Point, time: f64, radius: f32) -> TouchState {
        TouchState { pt, time, radius }
    }
}

pub struct Sketch {
    id: Id,
    rect: Rectangle,
    children: Vec<Box<dyn View>>,
    pixmap: Pixmap,
    fingers: FxHashMap<i32, TouchState>,
    pen: Pen,
    save_path: PathBuf,
    filename: String,
}

impl Sketch {
    pub fn new(rect: Rectangle, rq: &mut RenderQueue, context: &mut Context) -> Sketch {
        let id = ID_FEEDER.next();
        let mut children = Vec::new();
        let dpi = CURRENT_DEVICE.dpi;
        let small_height = scale_by_dpi(SMALL_BAR_HEIGHT, dpi) as i32;
        let border_radius = scale_by_dpi(BORDER_RADIUS_SMALL, dpi) as i32;
        let pixmap = &ICONS_PIXMAPS[ICON_NAME];
        let icon_padding = (small_height - pixmap.width.max(pixmap.height) as i32) / 2;
        let width = pixmap.width as i32 + icon_padding;
        let height = pixmap.height as i32 + icon_padding;
        let dx = (small_height - width) / 2;
        let dy = (small_height - height) / 2;
        let icon_rect = rect![rect.min.x + dx, rect.max.y - dy - height,
                              rect.min.x + dx + width, rect.max.y - dy];
        let icon = Icon::new(ICON_NAME,
                             icon_rect,
                             Event::ToggleNear(ViewId::TitleMenu, icon_rect))
                        .corners(Some(CornerSpec::Uniform(border_radius)));
        children.push(Box::new(icon) as Box<dyn View>);
        let save_path = context.library.home.join(&context.settings.sketch.save_path);
        rq.add(RenderData::new(id, rect, UpdateMode::Full));
        Sketch {
            id,
            rect,
            children,
            pixmap: Pixmap::new(rect.width(), rect.height()),
            fingers: FxHashMap::default(),
            pen: context.settings.sketch.pen.clone(),
            save_path,
            filename: Local::now().format(FILENAME_PATTERN).to_string(),
        }
    }

    fn toggle_title_menu(&mut self, rect: Rectangle, enable: Option<bool>, rq: &mut RenderQueue, context: &mut Context) {
        if let Some(index) = locate_by_id(self, ViewId::SketchMenu) {
            if let Some(true) = enable {
                return;
            }

            rq.add(RenderData::expose(*self.child(index).rect(), UpdateMode::Gui));
            self.children.remove(index);
        } else {
            if let Some(false) = enable {
                return;
            }

            let glob = Glob::new("**/*.png").unwrap().compile_matcher();
            let mut loadables: Vec<PathBuf> =
                WalkDir::new(&self.save_path).min_depth(1).into_iter()
                        .filter_map(|e| e.ok().filter(|e| !e.is_hidden())
                                         .and_then(|e| e.path().file_name().map(PathBuf::from)))
                        .filter(|p| glob.is_match(p))
                        .collect();
            loadables.sort_by(|a, b| b.cmp(a));

            let mut sizes = vec![
                EntryKind::CheckBox("Dynamic".to_string(),
                                    EntryId::TogglePenDynamism,
                                    self.pen.dynamic),
                EntryKind::Separator,
            ];

            for s in PEN_SIZES.iter() {
                sizes.push(EntryKind::RadioButton(s.to_string(),
                                                  EntryId::SetPenSize(*s),
                                                  self.pen.size == *s));
            }

            let mut colors = vec![
                EntryKind::RadioButton("White".to_string(),
                                       EntryId::SetPenColor(WHITE),
                                       self.pen.color == WHITE),
                EntryKind::RadioButton("Black".to_string(),
                                       EntryId::SetPenColor(BLACK),
                                       self.pen.color == BLACK),
            ];

            for i in 1..=14 {
                let c = i * 17;
                if i % 7 == 1 {
                    colors.push(EntryKind::Separator);
                }
                colors.push(EntryKind::RadioButton(format!("Gray {:02}", i),
                                                   EntryId::SetPenColor(c),
                                                   self.pen.color == c));
            }

            let mut entries = vec![
                EntryKind::SubMenu("Size".to_string(), sizes),
                EntryKind::SubMenu("Color".to_string(), colors),
                EntryKind::Separator,
                EntryKind::Command("Save".to_string(), EntryId::Save),
                EntryKind::Command("Refresh".to_string(), EntryId::Refresh),
                EntryKind::Command("New".to_string(), EntryId::New),
                EntryKind::Command("Quit".to_string(), EntryId::Quit),
            ];

            if !loadables.is_empty() {
                entries.insert(entries.len() - 1, EntryKind::SubMenu("Load".to_string(),
                    loadables.into_iter().map(|e|
                        EntryKind::Command(e.to_string_lossy().into_owned(),
                                           EntryId::Load(e))).collect()));
            }

            let sketch_menu = Menu::new(rect, ViewId::SketchMenu, MenuKind::Contextual, entries, context);
            rq.add(RenderData::new(sketch_menu.id(), *sketch_menu.rect(), UpdateMode::Gui));
            self.children.push(Box::new(sketch_menu) as Box<dyn View>);
        }
    }

    fn load(&mut self, filename: &PathBuf) -> Result<(), Error> {
        let path = self.save_path.join(filename);
        let decoder = png::Decoder::new(File::open(path)?);
        let mut reader = decoder.read_info()?;
        reader.next_frame(self.pixmap.data_mut())?;
        self.filename = filename.to_string_lossy().into_owned();
        Ok(())
    }

    fn save(&self) -> Result<(), Error> {
        if !self.save_path.exists() {
            fs::create_dir_all(&self.save_path)?;
        }
        let path = self.save_path.join(&self.filename);
        self.pixmap.save(&path.to_string_lossy().into_owned())?;
        Ok(())
    }

    fn quit(&self, context: &mut Context) {
        let import_settings = ImportSettings {
            allowed_kinds: ["png".to_string()].iter().cloned().collect(),
            .. Default::default()
        };
        context.library.import(&import_settings);
    }
}

#[inline]
fn draw_segment(pixmap: &mut Pixmap, ts: &mut TouchState, position: Point, time: f64, pen: &Pen, id: Id, fb_rect: &Rectangle, rq: &mut RenderQueue) {
    let (start_radius, end_radius) = if pen.dynamic {
        if time > ts.time {
            let d = vec2!((position.x - ts.pt.x) as f32,
                          (position.y - ts.pt.y) as f32).length();
            let speed = d / (time - ts.time) as f32;
            let base_radius = pen.size as f32 / 2.0;
            let radius = base_radius * (1.0 + (pen.amplitude/base_radius) * speed.clamp(pen.min_speed, pen.max_speed) / (pen.max_speed - pen.min_speed));
            (ts.radius, radius)
        } else {
            (ts.radius, ts.radius)
        }
    } else {
        let radius = pen.size as f32 / 2.0;
        (radius, radius)
    };

    let rect = Rectangle::from_segment(ts.pt, position,
                                       start_radius.ceil() as i32,
                                       end_radius.ceil() as i32);

    pixmap.draw_segment(ts.pt, position, start_radius, end_radius, pen.color);

    if let Some(render_rect) = rect.intersection(fb_rect) {
        rq.add(RenderData::no_wait(id, render_rect, UpdateMode::FastMono));
    }

    ts.pt = position;
    ts.time = time;
    ts.radius = end_radius;
}

impl View for Sketch {
    fn handle_event(&mut self, evt: &Event, hub: &Hub, _bus: &mut Bus, rq: &mut RenderQueue, context: &mut Context) -> bool {
        match *evt {
            Event::Device(DeviceEvent::Finger { status: FingerStatus::Motion, id, position, time }) => {
                if let Some(ts) = self.fingers.get_mut(&id) {
                    draw_segment(&mut self.pixmap, ts, position, time, &self.pen, self.id, &self.rect, rq);
                }
                true
            },
            Event::Device(DeviceEvent::Finger { status: FingerStatus::Down, id, position, time }) => {
                let radius = self.pen.size as f32 / 2.0;
                self.fingers.insert(id, TouchState::new(position, time, radius));
                true
            },
            Event::Device(DeviceEvent::Finger { status: FingerStatus::Up, id, position, time }) => {
                if let Some(ts) = self.fingers.get_mut(&id) {
                    draw_segment(&mut self.pixmap, ts, position, time, &self.pen, self.id, &self.rect, rq);
                }
                self.fingers.remove(&id);
                true
            },
            Event::ToggleNear(ViewId::TitleMenu, rect) => {
                self.toggle_title_menu(rect, None, rq, context);
                true
            },
            Event::Select(EntryId::SetPenSize(size)) => {
                self.pen.size = size;
                true
            },
            Event::Select(EntryId::SetPenColor(color)) => {
                self.pen.color = color;
                true
            },
            Event::Select(EntryId::TogglePenDynamism) => {
                self.pen.dynamic = !self.pen.dynamic;
                true
            },
            Event::Select(EntryId::Load(ref name)) => {
                if let Err(e) = self.load(name) {
                    let msg = format!("Couldn't load sketch: {}).", e);
                    let notif = Notification::new(msg, hub, rq, context);
                    self.children.push(Box::new(notif) as Box<dyn View>);
                } else {
                    rq.add(RenderData::new(self.id, self.rect, UpdateMode::Gui));
                }
                true
            },
            Event::Select(EntryId::Refresh) => {
                rq.add(RenderData::new(self.id, self.rect, UpdateMode::Full));
                true
            },
            Event::Select(EntryId::New) => {
                self.pixmap.clear(WHITE);
                self.filename = Local::now().format(FILENAME_PATTERN).to_string();
                rq.add(RenderData::new(self.id, self.rect, UpdateMode::Gui));
                true
            },
            Event::Select(EntryId::Save) => {
                let mut msg = match self.save() {
                    Err(e) => Some(format!("Can't save sketch: {}.", e)),
                    Ok(..) => {
                        if context.settings.sketch.notify_success {
                            Some(format!("Saved {}.", self.filename))
                        } else {
                            None
                        }
                    },
                };
                if let Some(msg) = msg.take() {
                    let notif = Notification::new(msg, hub, rq, context);
                    self.children.push(Box::new(notif) as Box<dyn View>);
                }
                true
            },
            Event::Select(EntryId::Quit) => {
                self.quit(context);
                hub.send(Event::Back).ok();
                true
            },
            _ => false,
        }
    }

    fn render(&self, fb: &mut dyn Framebuffer, rect: Rectangle, _fonts: &mut Fonts) {
        fb.draw_framed_pixmap_halftone(&self.pixmap, &rect, rect.min);
    }

    fn render_rect(&self, rect: &Rectangle) -> Rectangle {
        rect.intersection(&self.rect)
            .unwrap_or(self.rect)
    }

    fn might_rotate(&self) -> bool {
        false
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
