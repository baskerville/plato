use std::thread;
use std::fs::{self, File};
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::process::Command;
use std::collections::VecDeque;
use std::time::{Instant, Duration};
use failure::{Error, ResultExt};
use fnv::FnvHashMap;
use chrono::Local;
use framebuffer::{Framebuffer, KoboFramebuffer, UpdateMode};
use view::{View, Event, EntryId, EntryKind, ViewId};
use view::{render, render_no_wait, handle_event, fill_crack};
use view::common::{locate, locate_by_id, overlapping_rectangle};
use view::frontlight::FrontlightWindow;
use view::menu::{Menu, MenuKind};
use input::{DeviceEvent, ButtonCode, ButtonStatus};
use input::{raw_events, device_events, usb_events};
use gesture::{GestureEvent, gesture_events, BUTTON_HOLD_DELAY};
use helpers::{load_json, save_json, load_toml, save_toml};
use metadata::{Metadata, METADATA_FILENAME, auto_import};
use settings::{Settings, SETTINGS_PATH};
use frontlight::{Frontlight, NaturalFrontlight, StandardFrontlight};
use lightsensor::{LightSensor, KoboLightSensor};
use battery::{Battery, KoboBattery};
use view::home::Home;
use view::reader::Reader;
use view::confirmation::Confirmation;
use view::intermission::Intermission;
use view::notification::Notification;
use device::CURRENT_DEVICE;
use font::Fonts;

pub const APP_NAME: &str = "Plato";

const CLOCK_REFRESH_INTERVAL: Duration = Duration::from_secs(60);
const BATTERY_REFRESH_INTERVAL: Duration = Duration::from_secs(299);

pub struct Context {
    pub settings: Settings,
    pub metadata: Metadata,
    pub filename: PathBuf,
    pub fonts: Fonts,
    pub frontlight: Box<Frontlight>,
    pub battery: Box<Battery>,
    pub lightsensor: Box<LightSensor>,
    pub notification_index: u8,
    pub resumed_at: Instant,
    pub inverted: bool,
    pub monochrome: bool,
    pub suspended: bool,
    pub plugged: bool,
    pub mounted: bool,
}

impl Context {
    pub fn new(settings: Settings, metadata: Metadata,
               filename: PathBuf, fonts: Fonts, battery: Box<Battery>,
               frontlight: Box<Frontlight>, lightsensor: Box<LightSensor>) -> Context {
        Context { settings, metadata, filename, fonts, battery,
                  frontlight, lightsensor, notification_index: 0,
                  resumed_at: Instant::now(), inverted: false,
                  monochrome: false, suspended: false,
                  plugged: false, mounted: false }
    }
}

fn build_context() -> Result<Context, Error> {
    let path = Path::new(SETTINGS_PATH);
    let settings = load_toml::<Settings, _>(path);

    if let Err(ref e) = settings {
        if path.exists() {
            eprintln!("Warning: can't load settings: {}", e);
        }
    }

    let settings = settings.unwrap_or_default();

    let path = settings.library_path.join(METADATA_FILENAME);
    let metadata = load_json::<Metadata, _>(path)
                             .map_err(|e| eprintln!("Can't load metadata: {}", e))
                             .or_else(|_| auto_import(&settings.library_path,
                                                      &Vec::new(),
                                                      &settings.import.allowed_kinds))
                             .unwrap_or_default();
    let fonts = Fonts::load().context("Can't load fonts.")?;

    let battery = Box::new(KoboBattery::new().context("Can't create battery.")?) as Box<Battery>;

    let lightsensor = if CURRENT_DEVICE.has_lightsensor() {
        Box::new(KoboLightSensor::new().context("Can't create light sensor.")?) as Box<LightSensor>
    } else {
        Box::new(0u16) as Box<LightSensor>
    };

    let levels = settings.frontlight_levels;
    let frontlight = if CURRENT_DEVICE.has_natural_light() {
        Box::new(NaturalFrontlight::new(levels.intensity, levels.warmth)
                                   .context("Can't create natural frontlight.")?) as Box<Frontlight>
    } else {
        Box::new(StandardFrontlight::new(levels.intensity)
                                    .context("Can't create standard frontlight.")?) as Box<Frontlight>
    };

    Ok(Context::new(settings, metadata, PathBuf::from(METADATA_FILENAME),
                    fonts, battery, frontlight, lightsensor))
}

pub fn run() -> Result<(), Error> {
    let mut context = build_context().context("Can't build context.")?;
    let mut fb = KoboFramebuffer::new("/dev/fb0").context("Can't create framebuffer.")?;

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
            thread::sleep(CLOCK_REFRESH_INTERVAL);
            tx4.send(Event::ClockTick).unwrap();
        }
    });

    let tx5 = tx.clone();
    thread::spawn(move || {
        loop {
            thread::sleep(BATTERY_REFRESH_INTERVAL);
            tx5.send(Event::BatteryTick).unwrap();
        }
    });

    let fb_rect = fb.rect();

    if context.settings.wifi {
        Command::new("scripts/wifi-enable.sh").spawn().ok();
    } else {
        Command::new("scripts/wifi-disable.sh").spawn().ok();
    }

    if context.settings.frontlight {
        let levels = context.settings.frontlight_levels;
        context.frontlight.set_intensity(levels.intensity);
        context.frontlight.set_warmth(levels.warmth);
    } else {
        context.frontlight.set_warmth(0.0);
        context.frontlight.set_intensity(0.0);
    }

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
                    DeviceEvent::Button { code: ButtonCode::Power, status: ButtonStatus::Released, .. } |
                    DeviceEvent::CoverOn => {
                        if context.mounted || context.suspended ||
                           context.resumed_at.elapsed() <= BUTTON_HOLD_DELAY {
                            continue;
                        }

                        let interm = Intermission::new(fb_rect, "Sleeping".to_string(), false);
                        tx.send(Event::Render(*interm.rect(), UpdateMode::Gui)).unwrap();
                        tx.send(Event::Suspend).unwrap();
                        view.children_mut().push(Box::new(interm) as Box<View>);
                    },
                    DeviceEvent::NetUp => {
                        let ip = Command::new("scripts/ip.sh").output()
                                         .map(|o| String::from_utf8_lossy(&o.stdout).trim_right().to_string())
                                         .unwrap_or_default();
                        let essid = Command::new("scripts/essid.sh").output()
                                            .map(|o| String::from_utf8_lossy(&o.stdout).trim_right().to_string())
                                            .unwrap_or_default();
                        let notif = Notification::new(ViewId::NetUpNotif,
                                                      format!("Network is up ({}, {}).", ip, essid),
                                                      &mut context.notification_index,
                                                      &mut context.fonts,
                                                      &tx);
                        view.children_mut().push(Box::new(notif) as Box<View>);
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
                            Command::new("scripts/usb-disable.sh").status().ok();
                            context.mounted = false;
                            let path = Path::new(SETTINGS_PATH);
                            if let Ok(settings) = load_toml::<Settings, _>(path)
                                                            .map_err(|e| eprintln!("Can't load settings: {}", e)) {
                                context.settings = settings;
                            }
                            if context.settings.wifi {
                                Command::new("scripts/wifi-enable.sh")
                                        .spawn()
                                        .ok();
                            }
                            if context.settings.frontlight {
                                let levels = context.settings.frontlight_levels;
                                context.frontlight.set_intensity(levels.intensity);
                                context.frontlight.set_warmth(levels.warmth);
                            }
                            if let Some(index) = locate::<Intermission>(view.as_ref()) {
                                let rect = *view.child(index).rect();
                                view.children_mut().remove(index);
                                tx.send(Event::Expose(rect)).unwrap();
                            }
                            if Path::new("/mnt/onboard/.kobo/KoboRoot.tgz").exists() {
                                tx.send(Event::Select(EntryId::Reboot)).unwrap();
                            }
                            let path = context.settings.library_path.join(&context.filename);
                            let metadata = load_json::<Metadata, _>(path)
                                                     .map_err(|e| eprintln!("Can't load metadata: {}", e))
                                                     .unwrap_or_default();
                            if !metadata.is_empty() {
                                context.metadata = metadata;
                            }
                            if context.settings.import.unmount_trigger {
                                let metadata = auto_import(&context.settings.library_path,
                                                           &context.metadata,
                                                           &context.settings.import.allowed_kinds);
                                if metadata.is_ok() {
                                    context.metadata.append(&mut metadata.unwrap());
                                }
                            }
                            view.handle_event(&Event::Reseed, &tx, &mut bus, &mut context);
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
                let path = Path::new(SETTINGS_PATH);
                save_toml(&context.settings, path).map_err(|e| eprintln!("Can't save settings: {}", e)).ok();
                let path = context.settings.library_path.join(&context.filename);
                save_json(&context.metadata, path).map_err(|e| eprintln!("Can't save metadata: {}", e)).ok();
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
                println!("{}", Local::now().format("Went to sleep on %B %d, %Y at %H:%M."));
                Command::new("scripts/suspend.sh")
                        .status()
                        .ok();
                println!("{}", Local::now().format("Woke up on %B %d, %Y at %H:%M."));
                context.resumed_at = Instant::now();
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
                    context.frontlight.set_intensity(levels.intensity);
                    context.frontlight.set_warmth(levels.warmth);
                }
                tx.send(Event::ClockTick).unwrap();
                tx.send(Event::BatteryTick).unwrap();
            },
            Event::Mount => {
                if !context.mounted {
                    while let Some(v) = history.pop() {
                        view.handle_event(&Event::Back, &tx, &mut bus, &mut context);
                        view = v;
                    }
                    let path = context.settings.library_path.join(&context.filename);
                    save_json(&context.metadata, path).map_err(|e| eprintln!("Can't save metadata: {}", e)).ok();
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
                    Command::new("scripts/usb-enable.sh").spawn().ok();
                    context.mounted = true;
                }
            },
            Event::Gesture(ge) => {
                match ge {
                    GestureEvent::HoldButton(ButtonCode::Power) => {
                        let _ = File::create("poweroff").map_err(|e| {
                            eprintln!("Couldn't create the poweroff file: {}", e);
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
                    context.frontlight.set_intensity(levels.intensity);
                    context.frontlight.set_warmth(levels.warmth);
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
            Event::OpenToc(ref toc, current_page) => {
                let r = Reader::from_toc(fb_rect, toc, current_page, &tx, &mut context);
                history.push(view as Box<View>);
                view = Box::new(r) as Box<View>;
            },
            Event::Back => {
                if let Some(v) = history.pop() {
                    view = v;
                    view.handle_event(&Event::Reseed, &tx, &mut bus, &mut context);
                }
            },
            Event::TogglePresetMenu(rect, index) => {
                if let Some(index) = locate_by_id(view.as_ref(), ViewId::PresetMenu) {
                    let rect = *view.child(index).rect();
                    view.children_mut().remove(index);
                    tx.send(Event::Expose(rect)).unwrap();
                } else {
                    let preset_menu = Menu::new(rect, ViewId::PresetMenu, MenuKind::Contextual,
                                                vec![EntryKind::Command("Remove".to_string(),
                                                                        EntryId::RemovePreset(index))],
                                                &mut context.fonts);
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
                    tx.send(Event::Expose(rect)).unwrap();
                }
            },
            Event::Close(id) => {
                if let Some(index) = locate_by_id(view.as_ref(), id) {
                    let rect = overlapping_rectangle(view.child(index));
                    tx.send(Event::Expose(rect)).unwrap();
                    view.children_mut().remove(index);
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
                let name = Local::now().format("screenshot-%Y%m%d_%H%M%S.png");
                let msg = match fb.save(&name.to_string()) {
                    Err(e) => format!("Couldn't take screenshot: {}).", e),
                    Ok(_) => format!("Saved {}.", name),
                };
                let notif = Notification::new(ViewId::TakeScreenshotNotif,
                                              msg,
                                              &mut context.notification_index,
                                              &mut context.fonts,
                                              &tx);
                view.children_mut().push(Box::new(notif) as Box<View>);
            },
            Event::Select(EntryId::Reboot) | Event::Select(EntryId::Quit) => {
                break;
            },
            Event::Select(EntryId::StartNickel) => {
                fs::remove_file("bootlock").map_err(|e| {
                    eprintln!("Couldn't remove the bootlock file: {}", e);
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

    let path = context.settings.library_path.join(&context.filename);
    save_json(&context.metadata, path).context("Can't save metadata.")?;

    let path = Path::new(SETTINGS_PATH);
    save_toml(&context.settings, path).context("Can't save settings.")?;

    Ok(())
}
