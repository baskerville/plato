use std::thread;
use std::fs::{self, File};
use std::path::Path;
use std::sync::mpsc;
use std::collections::VecDeque;
use std::time::Duration;
use fnv::FnvHashMap;
use chrono::Local;
use framebuffer::{Framebuffer, KoboFramebuffer, UpdateMode};
use view::{View, Event, EntryId, render, render_no_wait, handle_event, fill_crack};
use input::{DeviceEvent, ButtonCode};
use input::{raw_events, device_events, usb_events};
use gesture::{GestureEvent, gesture_events};
use helpers::{load_json, save_json};
use device::CURRENT_DEVICE;
use metadata::{Metadata, METADATA_FILENAME, import};
use settings::{Settings, SETTINGS_PATH};
use view::home::Home;
use view::reader::Reader;
use font::Fonts;
use errors::*;

pub const APP_NAME: &str = "Plato";

const CLOCK_REFRESH_INTERVAL_MS: u64 = 60*1000;

pub struct Context {
    pub settings: Settings,
    pub metadata: Metadata,
    pub fonts: Fonts,
    pub inverted: bool,
    pub monochrome: bool,
}

impl Context {
    pub fn new(settings: Settings, metadata: Metadata, fonts: Fonts) -> Context {
        Context { settings, metadata, fonts, inverted: false, monochrome: false }
    }
}

pub fn run() -> Result<()> {
    let path = Path::new(SETTINGS_PATH);

    let settings = load_json::<Settings, _>(path);

    if let Err(ref e) = settings {
        if path.exists() {
            eprintln!("Warning: can't load settings: {}.", e);
        }
    }

    let settings = settings.unwrap_or_default();

    let path = settings.library_path.join(METADATA_FILENAME);
    let metadata = load_json::<Metadata, _>(path)
                             .map_err(|e| eprintln!("Can't load metadata: {}.", e))
                             .or_else(|_| import(&settings.library_path, &vec![]))
                             .unwrap_or_default();

    let mut fb = KoboFramebuffer::new("/dev/fb0").chain_err(|| "Can't create framebuffer.")?;
    let paths = vec!["/dev/input/event0".to_string(),
                     "/dev/input/event1".to_string()];
    let touch_screen = gesture_events(device_events(raw_events(paths), fb.dims()));
    let usb_port = usb_events();

    let (tx, rx) = mpsc::channel();
    let tx2 = tx.clone();

    thread::spawn(move || {
        while let Ok(evt) = touch_screen.recv() {
            tx2.send(evt).unwrap();
        }
    });

    let tx3 = tx.clone();
    thread::spawn(move || {
        while let Ok(evt) = usb_port.recv() {
            tx3.send(Event::Device(evt)).unwrap();
        }
    });

    let tx4 = tx.clone();
    thread::spawn(move || {
        loop {
            thread::sleep(Duration::from_millis(CLOCK_REFRESH_INTERVAL_MS));
            tx4.send(Event::ClockTick).unwrap();
        }
    });

    let fb_rect = fb.rect();

    let fonts = Fonts::load().chain_err(|| "Can't load fonts.")?;

    let mut context = Context::new(settings, metadata, fonts);
    let mut history: Vec<Box<View>> = Vec::new();
    let mut view: Box<View> = Box::new(Home::new(fb_rect, &tx, &mut context)?);

    let mut updating = FnvHashMap::default();

    println!("{} is running on a Kobo {}.", APP_NAME,
                                            CURRENT_DEVICE.model);
    println!("The framebuffer resolution is {} by {}.", fb_rect.width(),
                                                     fb_rect.height());

    let mut bus = VecDeque::with_capacity(4);

    while let Ok(evt) = rx.recv() {
        match evt {
            Event::Device(de) => {
                match de {
                    DeviceEvent::Plug => (),
                    DeviceEvent::Unplug => (),
                    _ => {
                        handle_event(view.as_mut(), &evt, &tx, &mut bus, &mut context);
                    }
                }
            },
            Event::Gesture(ge) => {
                match ge {
                    GestureEvent::HoldButton(ButtonCode::Power) => break,
                    _ => {
                        handle_event(view.as_mut(), &evt, &tx, &mut bus, &mut context);
                    },
                }
            },
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
            Event::Expose(mut rect) => {
                fill_crack(view.as_ref(), &mut rect, &mut fb, &mut context.fonts, &mut updating);
                if let Ok(tok) = fb.update(&rect, UpdateMode::Gui) {
                    updating.insert(tok, rect);
                }
            },
            Event::Open(info) => {
                let info2 = info.clone();
                if let Some(r) = Reader::new(fb_rect, *info, &tx, &mut context) {
                    history.push(view as Box<View>);
                    view = Box::new(r) as Box<View>;
                } else {
                    handle_event(view.as_mut(), &Event::Invalid(info2), &tx, &mut bus, &mut context);
                }
            },
            Event::Back => {
                if let Some(v) = history.pop() {
                    view = v;
                }
                view.handle_event(&evt, &tx, &mut bus, &mut context);
            },
            Event::Select(EntryId::ToggleInverted) => {
                fb.toggle_inverted();
                context.inverted = !context.inverted;
                tx.send(Event::Render(fb_rect, UpdateMode::Gui)).unwrap();
            },
            Event::Select(EntryId::ToggleMonochrome) => {
                fb.toggle_monochrome();
                context.monochrome = !context.monochrome;
                tx.send(Event::Render(fb_rect, UpdateMode::Gui)).unwrap();
            },
            Event::Select(EntryId::TakeScreenshot) => {
                fb.save(&Local::now().format("screenshot-%Y%m%d_%H%M%S.png").to_string())?;
            },
            Event::Select(EntryId::Suspend) => {
            },
            Event::Select(EntryId::PowerOff) => {
                let _ = File::create("poweroff").map_err(|e| {
                    eprintln!("Couldn't create the poweroff file: {}.", e);
                }).ok();
                break;
            },
            Event::Select(EntryId::Reboot) => {
                break;
            },
            Event::Select(EntryId::StartNickel) | Event::Select(EntryId::Quit) => {
                fs::remove_file("bootlock").map_err(|e| {
                    eprintln!("Couldn't remove the bootlock file: {}.", e);
                }).ok();
                break;
            },
            _ => {
                handle_event(view.as_mut(), &evt, &tx, &mut bus, &mut context);
            },
        }

        while let Some(ce) = bus.pop_front() {
            tx.send(ce).unwrap();
        }
    }

    // TODO: create a backup of the metadata file before overwriting?
    let path = context.settings.library_path.join(METADATA_FILENAME);
    save_json(&context.metadata, path).chain_err(|| "Can't save metadata.")?;

    Ok(())
}
