use std::env;
use std::thread;
use std::fs::{self, File};
use std::path::Path;
use std::sync::mpsc;
use std::process::Command;
use std::collections::VecDeque;
use std::time::Duration;
use fnv::FnvHashMap;
use chrono::Local;
use framebuffer::{Framebuffer, KoboFramebuffer, UpdateMode};
use view::{View, Event, EntryId, ViewId};
use view::{render, render_no_wait, handle_event, fill_crack};
use view::common::{locate, locate_by_id};
use view::frontlight::FrontlightWindow;
use input::{DeviceEvent, ButtonCode, ButtonStatus};
use input::{raw_events, device_events, usb_events};
use gesture::{GestureEvent, gesture_events};
use helpers::{load_json, save_json};
use metadata::{Metadata, METADATA_FILENAME, import};
use settings::{Settings, SETTINGS_PATH};
use frontlight::{Frontlight, NaturalFrontlight, StandardFrontlight};
use battery::{Battery, KoboBattery};
use view::home::Home;
use view::reader::Reader;
use view::confirmation::Confirmation;
use view::intermission::Intermission;
use device::CURRENT_DEVICE;
use font::Fonts;
use errors::*;

pub const APP_NAME: &str = "Plato";

const CLOCK_REFRESH_INTERVAL_MS: u64 = 60*1000;
const BATTERY_REFRESH_INTERVAL_MS: u64 = 299*1000;

pub struct Context {
    pub settings: Settings,
    pub metadata: Metadata,
    pub fonts: Fonts,
    pub frontlight: Box<Frontlight>,
    pub battery: Box<Battery>,
    pub inverted: bool,
    pub monochrome: bool,
    pub suspended: bool,
    pub plugged: bool,
    pub mounted: bool,
}

impl Context {
    pub fn new(settings: Settings, metadata: Metadata,
               fonts: Fonts, frontlight: Box<Frontlight>, battery: Box<Battery>) -> Context {
        Context { settings, metadata, fonts, frontlight, battery,
                  inverted: false, monochrome: false,
                  suspended: false, plugged: false,
                  mounted: false }
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
                             .or_else(|_| import(&settings.library_path,
                                                 &vec![],
                                                 &settings.import.allowed_kinds))
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

    let tx5 = tx.clone();
    thread::spawn(move || {
        loop {
            thread::sleep(Duration::from_millis(BATTERY_REFRESH_INTERVAL_MS));
            tx5.send(Event::BatteryTick).unwrap();
        }
    });

    let fb_rect = fb.rect();

    let fonts = Fonts::load().chain_err(|| "Can't load fonts.")?;

    if settings.wifi {
        Command::new("scripts/wifi-enable.sh").spawn().ok();
    } else {
        Command::new("scripts/wifi-disable.sh").spawn().ok();
    }

    let levels = settings.frontlight_levels;
    let mut frontlight = if CURRENT_DEVICE.has_natural_light() {
        Box::new(NaturalFrontlight::new(levels.intensity(), levels.warmth())
                                   .chain_err(|| "Can't create natural frontlight.")?) as Box<Frontlight>
    } else {
        Box::new(StandardFrontlight::new(levels.intensity())
                                    .chain_err(|| "Can't create standard frontlight.")?) as Box<Frontlight>
    };

    if settings.frontlight {
        frontlight.set_intensity(levels.intensity());
        frontlight.set_warmth(levels.warmth());
    } else {
        frontlight.set_warmth(0.0);
        frontlight.set_intensity(0.0);
    }

    let battery = Box::new(KoboBattery::new().chain_err(|| "Can't create battery.")?) as Box<Battery>;

    let mut context = Context::new(settings, metadata, fonts, frontlight, battery);
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
                    DeviceEvent::Button { code: ButtonCode::Power, status: ButtonStatus::Released, .. } => {
                        if context.mounted {
                            continue;
                        }

                        if context.suspended {
                            if let Some(index) = locate::<Intermission>(view.as_ref()) {
                                let rect = *view.child(index).rect();
                                view.children_mut().remove(index);
                                tx.send(Event::Expose(rect)).unwrap();
                            }
                            context.suspended = false;
                            Command::new("scripts/resume.sh")
                                    .status()
                                    .ok();
                            if context.settings.wifi {
                                Command::new("scripts/wifi-enable.sh")
                                        .spawn()
                                        .ok();
                            }
                            if context.settings.frontlight {
                                let levels = context.settings.frontlight_levels;
                                context.frontlight.set_intensity(levels.intensity());
                                context.frontlight.set_warmth(levels.warmth());
                            }
                            tx.send(Event::ClockTick).unwrap();
                            tx.send(Event::BatteryTick).unwrap();
                        } else {
                            let interm = Intermission::new(fb_rect, "Sleeping".to_string(), false);
                            tx.send(Event::Render(*interm.rect(), UpdateMode::Gui)).unwrap();
                            tx.send(Event::Suspend).unwrap();
                            view.children_mut().push(Box::new(interm) as Box<View>);
                        }
                    },
                    DeviceEvent::Plug => {
                        if context.plugged {
                            continue;
                        }

                        context.plugged = true;
                        let confirm = Confirmation::new(ViewId::ConfirmMount,
                                                        Event::Mount,
                                                        "Mount onboard and external cards?".to_string(),
                                                        &mut context.fonts);

                        tx.send(Event::Render(*confirm.rect(), UpdateMode::Gui)).unwrap();
                        tx.send(Event::BatteryTick).unwrap();
                        view.children_mut().push(Box::new(confirm) as Box<View>);
                    },
                    DeviceEvent::Unplug => {
                        if !context.plugged {
                            continue;
                        }

                        if context.mounted {
                            mount(false);
                            context.mounted = false;
                            if context.settings.wifi {
                                Command::new("scripts/wifi-enable.sh")
                                        .spawn()
                                        .ok();
                            }
                            if context.settings.frontlight {
                                let levels = context.settings.frontlight_levels;
                                context.frontlight.set_intensity(levels.intensity());
                                context.frontlight.set_warmth(levels.warmth());
                            }
                            if let Some(index) = locate::<Intermission>(view.as_ref()) {
                                let rect = *view.child(index).rect();
                                view.children_mut().remove(index);
                                tx.send(Event::Expose(rect)).unwrap();
                            }
                            let path = context.settings.library_path.join(METADATA_FILENAME);
                            let metadata = load_json::<Metadata, _>(path)
                                                     .map_err(|e| eprintln!("Can't load metadata: {}.", e))
                                                     .unwrap_or_default();
                            if !metadata.is_empty() {
                                context.metadata = metadata;
                            }
                            if context.settings.import.unmount_trigger {
                                let metadata = import(&context.settings.library_path,
                                                      &context.metadata,
                                                      &context.settings.import.allowed_kinds);
                                if metadata.is_ok() {
                                    context.metadata.append(&mut metadata.unwrap());
                                }
                            }
                            view.handle_event(&Event::Back, &tx, &mut bus, &mut context);
                        } else {
                            context.plugged = false;
                            tx.send(Event::BatteryTick).unwrap();
                        }
                    },
                    _ => {
                        handle_event(view.as_mut(), &evt, &tx, &mut bus, &mut context);
                    }
                }
            },
            Event::Suspend => {
                context.suspended = true;
                updating.retain(|tok, _| fb.wait(*tok).is_err());
                if context.settings.frontlight {
                    context.settings.frontlight_levels = context.frontlight.levels();
                    context.frontlight.set_warmth(0.0);
                    context.frontlight.set_intensity(0.0);
                }
                if context.settings.wifi {
                    Command::new("scripts/wifi-disable.sh")
                            .status()
                            .ok();
                }
                println!("{} suspending.", Local::now().format("%Y%m%d_%H%M%S"));
                Command::new("scripts/suspend.sh")
                        .status()
                        .ok();
                println!("{} woke up from suspend.", Local::now().format("%Y%m%d_%H%M%S"));
            },
            Event::Mount => {
                if !context.mounted {
                    if let Some(v) = history.pop() {
                        view.handle_event(&Event::Back, &tx, &mut bus, &mut context);
                        view = v;
                    }
                    if context.settings.frontlight {
                        context.settings.frontlight_levels = context.frontlight.levels();
                        context.frontlight.set_warmth(0.0);
                        context.frontlight.set_intensity(0.0);
                    }
                    if context.settings.wifi {
                        Command::new("scripts/wifi-disable.sh")
                                .status()
                                .ok();
                    }
                    let interm = Intermission::new(fb_rect, "Mounted".to_string(), false);
                    tx.send(Event::Render(*interm.rect(), UpdateMode::Full)).unwrap();
                    view.children_mut().push(Box::new(interm) as Box<View>);
                    mount(true);
                    context.mounted = true;
                }
            },
            Event::Gesture(ge) => {
                match ge {
                    GestureEvent::HoldButton(ButtonCode::Power) => {
                        let _ = File::create("poweroff").map_err(|e| {
                            eprintln!("Couldn't create the poweroff file: {}.", e);
                        }).ok();
                        let interm = Intermission::new(fb_rect, "Powered off".to_string(), true);
                        updating.retain(|tok, _| fb.wait(*tok).is_err());
                        interm.render(&mut fb, &mut context.fonts);
                        fb.update(interm.rect(), UpdateMode::Full).ok();
                        break;
                    }
                    _ => {
                        handle_event(view.as_mut(), &evt, &tx, &mut bus, &mut context);
                    },
                }
            },
            Event::ToggleFrontlight => {
                context.settings.frontlight = !context.settings.frontlight;
                if context.settings.frontlight {
                    let levels = context.settings.frontlight_levels;
                    context.frontlight.set_intensity(levels.intensity());
                    context.frontlight.set_warmth(levels.warmth());
                } else {
                    context.settings.frontlight_levels = context.frontlight.levels();
                    context.frontlight.set_warmth(0.0);
                    context.frontlight.set_intensity(0.0);
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
                    tx.send(Event::Expose(rect)).unwrap();
                }
            },
            Event::Close(id) => {
                if let Some(index) = locate_by_id(view.as_ref(), id) {
                    let rect = *view.child(index).rect();
                    view.children_mut().remove(index);
                    tx.send(Event::Expose(rect)).unwrap();
                }
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
            Event::Select(EntryId::ToggleWifi) => {
                context.settings.wifi = !context.settings.wifi;
                if context.settings.wifi {
                    Command::new("scripts/wifi-enable.sh")
                            .spawn()
                            .ok();
                } else {
                    Command::new("scripts/wifi-disable.sh")
                            .spawn()
                            .ok();
                }
            },
            Event::Select(EntryId::TakeScreenshot) => {
                fb.save(&Local::now().format("screenshot-%Y%m%d_%H%M%S.png").to_string())?;
            },
            Event::Select(EntryId::Reboot) | Event::Select(EntryId::Quit) => {
                break;
            },
            Event::Select(EntryId::StartNickel) => {
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

    if context.settings.frontlight {
        context.settings.frontlight_levels = context.frontlight.levels();
    }

    let path = context.settings.library_path.join(METADATA_FILENAME);
    save_json(&context.metadata, path).chain_err(|| "Can't save metadata.")?;

    let path = Path::new(SETTINGS_PATH);
    save_json(&context.settings, path).chain_err(|| "Can't save settings.")?;

    Ok(())
}

fn mount(enable: bool) {
    let action = if enable { "add" } else { "remove" };
    let mut cmd = Command::new("/usr/local/Kobo/udev/usb");
    cmd.env("ACTION", action)
       .env("PRODUCT_ID", env::var("PRODUCT_ID").ok().as_ref()
            .map_or("0x6666", String::as_ref))
       .env("VERSION", env::var("FIRMWARE_VERSION").ok().as_ref()
            .map_or("9.8.76543", String::as_ref))
       .env("SN", env::var("SERIAL_NUMBER").ok().as_ref()
            .map_or("N666999666999", String::as_ref));
    if enable {
        cmd.spawn().ok();
    } else {
        cmd.status().ok();
    }
}
