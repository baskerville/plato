extern crate rand;
#[macro_use] extern crate failure;
extern crate serde;
#[macro_use] extern crate serde_derive;
extern crate serde_json;
extern crate toml;
#[macro_use] extern crate lazy_static;
#[macro_use] extern crate bitflags;
#[macro_use] extern crate downcast_rs;
extern crate unicode_normalization;
extern crate paragraph_breaker;
extern crate hyphenation;
extern crate entities;
extern crate libc;
extern crate regex;
extern crate either;
extern crate chrono;
extern crate zip;
extern crate glob;
extern crate sdl2;
extern crate fnv;
extern crate png;
extern crate isbn;
extern crate titlecase;

#[macro_use] mod geom;
mod unit;
mod color;
mod framebuffer;
mod input;
mod gesture;
mod view;
mod battery;
mod device;
mod font;
mod helpers;
mod document;
mod metadata;
mod settings;
mod frontlight;
mod lightsensor;
mod symbolic_path;
mod trash;
mod app;

use std::mem;
use std::process;
use std::thread;
use std::fs::File;
use std::sync::mpsc;
use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::time::Duration;
use failure::{Error, ResultExt};
use fnv::FnvHashMap;
use chrono::Local;
use png::HasParameters;
use sdl2::event::Event as SdlEvent;
use sdl2::keyboard::{Scancode, Keycode};
use sdl2::render::{WindowCanvas, BlendMode};
use sdl2::pixels::{Color as SdlColor, PixelFormatEnum};
use sdl2::rect::Point as SdlPoint;
use sdl2::rect::Rect as SdlRect;
use framebuffer::{Framebuffer, UpdateMode};
use input::{DeviceEvent, FingerStatus};
use view::{View, Event, ViewId, EntryId, EntryKind};
use view::{render, render_no_wait, handle_event, fill_crack};
use view::home::Home;
use view::reader::Reader;
use view::notification::Notification;
use view::frontlight::FrontlightWindow;
use view::keyboard::Keyboard;
use view::menu::{Menu, MenuKind};
use view::common::{locate, locate_by_id, transfer, overlapping_rectangle};
use helpers::{load_json, save_json, load_toml, save_toml};
use metadata::{Metadata, METADATA_FILENAME};
use settings::{Settings, SETTINGS_PATH};
use geom::Rectangle;
use gesture::gesture_events;
use device::CURRENT_DEVICE;
use battery::{Battery, FakeBattery};
use frontlight::{Frontlight, LightLevels};
use lightsensor::LightSensor;
use font::Fonts;
use app::Context;

pub const APP_NAME: &str = "Plato";
const DEFAULT_ROTATION: i8 = 1;

const CLOCK_REFRESH_INTERVAL: Duration = Duration::from_secs(60);

pub fn build_context(fb: &Framebuffer) -> Result<Context, Error> {
    let settings = load_toml::<Settings, _>(SETTINGS_PATH)?;
    let path = settings.library_path.join(METADATA_FILENAME);
    let metadata = load_json::<Metadata, _>(path)?;
    let battery = Box::new(FakeBattery::new()) as Box<Battery>;
    let frontlight = Box::new(LightLevels::default()) as Box<Frontlight>;
    let lightsensor = Box::new(0u16) as Box<LightSensor>;
    let fonts = Fonts::load()?;
    Ok(Context::new(fb, settings, metadata, PathBuf::from(METADATA_FILENAME),
                    fonts, battery, frontlight, lightsensor))
}

#[inline]
fn seconds(timestamp: u32) -> f64 {
    timestamp as f64 / 1000.0
}

#[inline]
pub fn device_event(event: SdlEvent) -> Option<DeviceEvent> {
    match event {
        SdlEvent::MouseButtonDown { timestamp, x, y, .. } =>
            Some(DeviceEvent::Finger { id: 0,
                                       status: FingerStatus::Down,
                                       position: pt!(x, y),
                                       time: seconds(timestamp) }),
        SdlEvent::MouseButtonUp { timestamp, x, y, .. } =>
            Some(DeviceEvent::Finger { id: 0,
                                       status: FingerStatus::Up,
                                       position: pt!(x, y),
                                       time: seconds(timestamp) }),
        SdlEvent::MouseMotion { timestamp, x, y, .. } =>
            Some(DeviceEvent::Finger { id: 0,
                                       status: FingerStatus::Motion,
                                       position: pt!(x, y),
                                       time: seconds(timestamp) }),
        _ => None
    }
}

impl Framebuffer for WindowCanvas {
    fn set_pixel(&mut self, x: u32, y: u32, color: u8) {
        self.set_draw_color(SdlColor::RGB(color, color, color));
        self.draw_point(SdlPoint::new(x as i32, y as i32)).unwrap();
    }

    fn set_blended_pixel(&mut self, x: u32, y: u32, color: u8, alpha: f32) {
        self.set_draw_color(SdlColor::RGBA(color, color, color, (alpha * 255.0) as u8));
        self.draw_point(SdlPoint::new(x as i32, y as i32)).unwrap();
    }

    fn invert_region(&mut self, rect: &Rectangle) {
        let width = rect.width();
        let s_rect = Some(SdlRect::new(rect.min.x, rect.min.y,
                                       width, rect.height()));
        if let Ok(data) = self.read_pixels(s_rect, PixelFormatEnum::RGB24) {
            for y in rect.min.y..rect.max.y {
                let v = (y - rect.min.y) as u32;
                for x in rect.min.x..rect.max.x {
                    let u = (x - rect.min.x) as u32;
                    let addr = 3 * (v * width + u);
                    let color = 255 - data[addr as usize];
                    self.set_pixel(x as u32, y as u32, color);
                }
            }
        }
    }

    fn update(&mut self, _rect: &Rectangle, _mode: UpdateMode) -> Result<u32, Error> {
        self.present();
        Ok(1)
    }

    fn wait(&self, _: u32) -> Result<i32, Error> {
        Ok(1)
    }

    fn save(&self, path: &str) -> Result<(), Error> {
        let (width, height) = self.dims();
        let file = File::create(path).context("Can't create output file.")?;
        let mut encoder = png::Encoder::new(file, width, height);
        encoder.set(png::ColorType::RGB).set(png::BitDepth::Eight);
        let mut writer = encoder.write_header().context("Can't write header.")?;
        let data = self.read_pixels(self.viewport(), PixelFormatEnum::RGB24).unwrap_or_default();
        writer.write_image_data(&data).context("Can't write data to file.")?;
        Ok(())
    }

    fn rotation(&self) -> i8 {
        DEFAULT_ROTATION
    }

    fn set_rotation(&mut self, n: i8) -> Result<(u32, u32), Error> {
        let (mut width, mut height) = self.dims();
        if (width < height && n % 2 == 0) || (width > height && n % 2 == 1) {
            mem::swap(&mut width, &mut height);
        }
        self.window_mut().set_size(width, height);
        Ok((width, height))
    }

    fn toggle_inverted(&mut self) {}

    fn toggle_monochrome(&mut self) {}

    fn dims(&self) -> (u32, u32) {
        self.window().size()
    }
}

pub fn run() -> Result<(), Error> {
    let sdl_context = sdl2::init().unwrap();
    let video_subsystem = sdl_context.video().unwrap();
    let (width, height) = CURRENT_DEVICE.dims;
    let window = video_subsystem
                 .window("Plato Emulator", width, height)
                 .position_centered()
                 .build()
                 .unwrap();

    let mut fb = window.into_canvas().software().build().unwrap();
    fb.set_blend_mode(BlendMode::Blend);

    let mut context = build_context(&fb)?;

    let (tx, rx) = mpsc::channel();
    let (ty, ry) = mpsc::channel();
    let touch_screen = gesture_events(ry);

    let tx2 = tx.clone();
    thread::spawn(move || {
        while let Ok(evt) = touch_screen.recv() {
            tx2.send(evt).unwrap();
        }
    });

    let tx3 = tx.clone();
    thread::spawn(move || {
        loop {
            thread::sleep(CLOCK_REFRESH_INTERVAL);
            tx3.send(Event::ClockTick).unwrap();
        }
    });

    let mut history: Vec<Box<View>> = Vec::new();
    let mut view: Box<View> = Box::new(Home::new(fb.rect(), &tx, &mut context)?);

    let mut updating = FnvHashMap::default();

    if context.settings.frontlight {
        let levels = context.settings.frontlight_levels;
        context.frontlight.set_intensity(levels.intensity);
        context.frontlight.set_warmth(levels.warmth);
    } else {
        context.frontlight.set_warmth(0.0);
        context.frontlight.set_intensity(0.0);
    }

    println!("{} is running on a Kobo {}.", APP_NAME,
                                            CURRENT_DEVICE.model);
    println!("The framebuffer resolution is {} by {}.", fb.rect().width(),
                                                        fb.rect().height());

    let mut bus = VecDeque::with_capacity(4);

    'outer: loop {
        if let Some(sdl_evt) = sdl_context.event_pump().unwrap().wait_event_timeout(20) {
            match sdl_evt {
                SdlEvent::Quit { .. } |
                SdlEvent::KeyDown { keycode: Some(Keycode::Escape), .. } => break,
                SdlEvent::KeyDown { scancode: Some(scancode), .. } => {
                    if let Some(kb_idx) = locate::<Keyboard>(view.as_ref()) {
                        let index = match scancode {
                            Scancode::Backspace => Some(10),
                            Scancode::Delete => Some(20),
                            Scancode::LShift | Scancode::RShift => Some(21),
                            Scancode::Return => Some(29),
                            Scancode::Left => Some(30),
                            Scancode::LGui | Scancode::RGui => Some(31),
                            Scancode::Space => Some(32),
                            Scancode::LAlt | Scancode::RAlt => Some(33),
                            Scancode::Right => Some(34),
                            _ => {
                                let name = scancode.name();
                                if name.len() == 1 {
                                    let c = name.chars().next().unwrap()
                                                .to_lowercase().next().unwrap();
                                    if let Some(i) = "qwertyuiop".find(c) {
                                        Some(i)
                                    } else if let Some(i) = "asdfghjkl".find(c) {
                                        Some(11+i)
                                    } else if let Some(i) = "zxcvbnm".find(c) {
                                        Some(22+i)
                                    } else {
                                        None
                                    }
                                } else {
                                    None
                                }
                            },
                        };
                        if index.is_some() {
                            let position = view.child(kb_idx).child(index.unwrap()).rect().center();
                            ty.send(DeviceEvent::Finger { status: FingerStatus::Down, position, id: 0, time: 0.0}).unwrap();
                            ty.send(DeviceEvent::Finger { status: FingerStatus::Up, position, id: 0, time: 0.0}).unwrap();
                        }
                    }
                },
                _ => {
                    if let Some(dev_evt) = device_event(sdl_evt) {
                        ty.send(dev_evt).unwrap();
                    }
                },
            }
        }

        while let Ok(evt) = rx.recv_timeout(Duration::from_millis(20)) {
            match evt {
                Event::Render(mut rect, mode) => {
                    render(view.as_ref(), &mut rect, &mut fb, &mut context.fonts, &mut updating);
                    if let Ok(tok) = fb.update(&rect, mode) {
                        updating.insert(tok, rect);
                    }
                },
                Event::RenderNoWait(mut rect, mode) => {
                    render_no_wait(view.as_ref(), &mut rect, &mut fb, &mut context.fonts, &mut updating);
                    if let Ok(tok) = fb.update(&rect, mode) {
                        updating.insert(tok, rect);
                    }
                },
                Event::Expose(mut rect, mode) => {
                    fill_crack(view.as_ref(), &mut rect, &mut fb, &mut context.fonts, &mut updating);
                    if let Ok(tok) = fb.update(&rect, mode) {
                        updating.insert(tok, rect);
                    }
                },
                Event::Open(info) => {
                    let rotation = context.display.rotation;
                    if let Some(n) = info.reader.as_ref().and_then(|r| r.rotation) {
                        if n != rotation {
                            if let Ok(dims) = fb.set_rotation(n) {
                                context.display.rotation = n;
                                context.display.dims = dims;
                            }
                        }
                    }
                    let info2 = info.clone();
                    if let Some(r) = Reader::new(fb.rect(), *info, &tx, &mut context) {
                        let mut next_view = Box::new(r) as Box<View>;
                        transfer::<Notification>(view.as_mut(), next_view.as_mut());
                        history.push(view as Box<View>);
                        view = next_view;
                    } else {
                        handle_event(view.as_mut(), &Event::Invalid(info2), &tx, &mut bus, &mut context);
                    }
                },
                Event::OpenToc(ref toc, current_page, next_page) => {
                    let r = Reader::from_toc(fb.rect(), toc, current_page, next_page, &tx, &mut context);
                    let mut next_view = Box::new(r) as Box<View>;
                    transfer::<Notification>(view.as_mut(), next_view.as_mut());
                    history.push(view as Box<View>);
                    view = next_view;
                },
                Event::Back => {
                    if let Some(v) = history.pop() {
                        view = v;
                        if view.is::<Home>() {
                            if context.display.rotation % 2 != 1 {
                                if let Ok(dims) = fb.set_rotation(DEFAULT_ROTATION) {
                                    context.display.rotation = DEFAULT_ROTATION;
                                    context.display.dims = dims;
                                }
                            }
                        }
                        view.handle_event(&Event::Reseed, &tx, &mut bus, &mut context);
                    }
                },
                Event::TogglePresetMenu(rect, index) => {
                    if let Some(index) = locate_by_id(view.as_ref(), ViewId::PresetMenu) {
                        let rect = *view.child(index).rect();
                        view.children_mut().remove(index);
                        tx.send(Event::Expose(rect, UpdateMode::Gui)).unwrap();
                    } else {
                        let preset_menu = Menu::new(rect, ViewId::PresetMenu, MenuKind::Contextual,
                                                    vec![EntryKind::Command("Remove".to_string(),
                                                                            EntryId::RemovePreset(index))],
                                                    &mut context);
                        tx.send(Event::Render(*preset_menu.rect(), UpdateMode::Gui)).unwrap();
                        view.children_mut().push(Box::new(preset_menu) as Box<View>);
                    }
                },
                Event::Show(ViewId::Frontlight) => {
                    if !context.settings.frontlight {
                        continue;
                    }
                    let flw = FrontlightWindow::new(&mut context);
                    tx.send(Event::Render(*flw.rect(), UpdateMode::Gui)).unwrap();
                    view.children_mut().push(Box::new(flw) as Box<View>);
                },
                Event::Close(ViewId::Frontlight) => {
                    if let Some(index) = locate::<FrontlightWindow>(view.as_ref()) {
                        let rect = *view.child(index).rect();
                        view.children_mut().remove(index);
                        tx.send(Event::Expose(rect, UpdateMode::Gui)).unwrap();
                    }
                },
                Event::Close(id) => {
                    if let Some(index) = locate_by_id(view.as_ref(), id) {
                        let rect = overlapping_rectangle(view.child(index));
                        tx.send(Event::Expose(rect, UpdateMode::Gui)).unwrap();
                        view.children_mut().remove(index);
                    }
                },
                Event::Select(EntryId::Rotate(n)) if n != context.display.rotation => {
                    updating.retain(|tok, _| fb.wait(*tok).is_err());
                    if let Ok(dims) = fb.set_rotation(n) {
                        context.display.rotation = n;
                        let fb_rect = Rectangle::from(dims);
                        if context.display.dims != dims {
                            context.display.dims = dims;
                            view.resize(fb_rect, &tx, &mut context);
                        }
                    }
                },
                Event::Select(EntryId::ToggleInverted) => {
                    fb.toggle_inverted();
                    context.inverted = !context.inverted;
                    tx.send(Event::Render(fb.rect(), UpdateMode::Gui)).unwrap();
                },
                Event::Select(EntryId::ToggleMonochrome) => {
                    fb.toggle_monochrome();
                    context.monochrome = !context.monochrome;
                    tx.send(Event::Render(fb.rect(), UpdateMode::Gui)).unwrap();
                },
                Event::Select(EntryId::TakeScreenshot) => {
                    let name = Local::now().format("screenshot-%Y%m%d_%H%M%S.png");
                    let msg = match fb.save(&name.to_string()) {
                        Err(e) => format!("Couldn't take screenshot: {}).", e),
                        Ok(_) => format!("Saved {}.", name),
                    };
                    let notif = Notification::new(ViewId::TakeScreenshotNotif,
                                                  msg, &tx, &mut context);
                    view.children_mut().push(Box::new(notif) as Box<View>);
                },
                Event::Select(EntryId::Quit) => {
                    break 'outer;
                },
                _ => {
                    handle_event(view.as_mut(), &evt, &tx, &mut bus, &mut context);
                },
            }

            while let Some(ce) = bus.pop_front() {
                tx.send(ce).unwrap();
            }
        }
    }

    if !history.is_empty() {
        let (tx, _rx) = mpsc::channel();
        view.handle_event(&Event::Back, &tx, &mut VecDeque::new(), &mut context);
        while let Some(mut view) = history.pop() {
            view.handle_event(&Event::Back, &tx, &mut VecDeque::new(), &mut context);
        }
    }

    if context.settings.frontlight {
        context.settings.frontlight_levels = context.frontlight.levels();
    }

    let path = context.settings.library_path.join(&context.filename);
    save_json(&context.metadata, path).context("Can't save metadata.")?;

    let path = Path::new(SETTINGS_PATH);
    save_toml(&context.settings, path).context("Can't save settings.")?;

    Ok(())
}

fn main() {
    if let Err(e) = run() {
        for e in e.iter_chain() {
            eprintln!("plato-emulator: {}", e);
        }
        process::exit(1);
    }
}
