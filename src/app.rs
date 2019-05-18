use std::thread;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender};
use std::process::Command;
use std::collections::VecDeque;
use std::time::{Duration, Instant};
use failure::{Error, ResultExt};
use fnv::FnvHashMap;
use chrono::Local;
use crate::framebuffer::{Framebuffer, KoboFramebuffer, Display, UpdateMode};
use crate::view::{View, Event, EntryId, EntryKind, ViewId, AppId};
use crate::view::{render, render_no_wait, render_no_wait_region, handle_event, expose};
use crate::view::common::{locate, locate_by_id, transfer_notifications, overlapping_rectangle};
use crate::view::frontlight::FrontlightWindow;
use crate::view::menu::{Menu, MenuKind};
use crate::view::sketch::Sketch;
use crate::view::calculator::Calculator;
use crate::input::{DeviceEvent, PowerSource, ButtonCode, ButtonStatus};
use crate::input::{raw_events, device_events, usb_events, display_rotate_event};
use crate::gesture::{GestureEvent, gesture_events};
use crate::helpers::{load_json, save_json, load_toml, save_toml};
use crate::metadata::{Metadata, METADATA_FILENAME, auto_import};
use crate::settings::{Settings, SETTINGS_PATH, RotationLock};
use crate::frontlight::{Frontlight, StandardFrontlight, NaturalFrontlight, PremixedFrontlight};
use crate::lightsensor::{LightSensor, KoboLightSensor};
use crate::battery::{Battery, KoboBattery};
use crate::geom::{Rectangle, Edge};
use crate::view::home::Home;
use crate::view::reader::Reader;
use crate::view::confirmation::Confirmation;
use crate::view::intermission::{Intermission, IntermKind};
use crate::view::notification::Notification;
use crate::device::{CURRENT_DEVICE, FrontlightKind};
use crate::font::Fonts;

pub const APP_NAME: &str = "Plato";

const CLOCK_REFRESH_INTERVAL: Duration = Duration::from_secs(60);
const BATTERY_REFRESH_INTERVAL: Duration = Duration::from_secs(299);
const AUTO_SUSPEND_REFRESH_INTERVAL: Duration = Duration::from_secs(60);
const SUSPEND_WAIT_DELAY: Duration = Duration::from_secs(15);
const PREPARE_SUSPEND_WAIT_DELAY: Duration = Duration::from_secs(3);

pub struct Context {
    pub fb: Box<dyn Framebuffer>,
    pub display: Display,
    pub settings: Settings,
    pub metadata: Metadata,
    pub filename: PathBuf,
    pub fonts: Fonts,
    pub frontlight: Box<dyn Frontlight>,
    pub battery: Box<dyn Battery>,
    pub lightsensor: Box<dyn LightSensor>,
    pub notification_index: u8,
    pub plugged: bool,
    pub covered: bool,
    pub shared: bool,
    pub online: bool,
}

impl Context {
    pub fn new(fb: Box<dyn Framebuffer>, settings: Settings, metadata: Metadata,
               filename: PathBuf, fonts: Fonts, battery: Box<dyn Battery>,
               frontlight: Box<dyn Frontlight>, lightsensor: Box<dyn LightSensor>) -> Context {
        let dims = fb.dims();
        let rotation = CURRENT_DEVICE.transformed_rotation(fb.rotation());
        Context { fb, display: Display { dims, rotation },
                  settings, metadata, filename, fonts,
                  battery, frontlight, lightsensor, notification_index: 0,
                  plugged: false, covered: false, shared: false, online: false }
    }
}

struct Task {
    id: TaskId,
    chan: Receiver<()>,
}

impl Task {
    fn has_occurred(&self) -> bool {
        self.chan.try_recv() == Ok(())
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
enum TaskId {
    CheckBattery,
    PrepareSuspend,
    Suspend,
}

struct HistoryItem {
    view: Box<dyn View>,
    rotation: i8,
    monochrome: bool,
}

fn build_context(fb: Box<dyn Framebuffer>) -> Result<Context, Error> {
    let path = Path::new(SETTINGS_PATH);
    let settings = load_toml::<Settings, _>(path);

    if let Err(ref e) = settings {
        if path.exists() {
            eprintln!("Warning: can't load settings: {}", e);
        }
    }

    let settings = settings.unwrap_or_default();

    let path = settings.library_path.join(METADATA_FILENAME);
    let mut metadata = load_json::<Metadata, _>(path)
                                 .map_err(|e| eprintln!("Can't load metadata: {}", e))
                                 .or_else(|_| auto_import(&settings.library_path,
                                                          &Vec::new(),
                                                          &settings.import))
                                 .unwrap_or_default();

    if settings.import.startup_trigger {
        let imported_metadata = auto_import(&settings.library_path,
                                            &metadata,
                                            &settings.import);
        metadata.append(&mut imported_metadata.unwrap_or_default());
    }

    let fonts = Fonts::load().context("Can't load fonts.")?;

    let battery = Box::new(KoboBattery::new().context("Can't create battery.")?) as Box<dyn Battery>;

    let lightsensor = if CURRENT_DEVICE.has_lightsensor() {
        Box::new(KoboLightSensor::new().context("Can't create light sensor.")?) as Box<dyn LightSensor>
    } else {
        Box::new(0u16) as Box<dyn LightSensor>
    };

    let levels = settings.frontlight_levels;
    let frontlight = match CURRENT_DEVICE.frontlight_kind() {
        FrontlightKind::Standard => Box::new(StandardFrontlight::new(levels.intensity)
                                        .context("Can't create standard frontlight.")?) as Box<dyn Frontlight>,
        FrontlightKind::Natural => Box::new(NaturalFrontlight::new(levels.intensity, levels.warmth)
                                        .context("Can't create natural frontlight.")?) as Box<dyn Frontlight>,
        FrontlightKind::Premixed => Box::new(PremixedFrontlight::new(levels.intensity, levels.warmth)
                                        .context("Can't create premixed frontlight.")?) as Box<dyn Frontlight>,
    };

    Ok(Context::new(fb, settings, metadata, PathBuf::from(METADATA_FILENAME),
                    fonts, battery, frontlight, lightsensor))
}

fn schedule_task(id: TaskId, event: Event, delay: Duration, hub: &Sender<Event>, tasks: &mut Vec<Task>) {
    let (ty, ry) = mpsc::channel();
    let hub2 = hub.clone();
    tasks.push(Task { id, chan: ry });
    thread::spawn(move || {
        thread::sleep(delay);
        if ty.send(()).is_ok() {
            hub2.send(event).unwrap();
        }
    });
}

fn resume(id: TaskId, tasks: &mut Vec<Task>, view: &mut View, hub: &Sender<Event>, context: &mut Context) {
    if id == TaskId::Suspend {
        tasks.retain(|task| task.id != TaskId::Suspend);
        if context.settings.frontlight {
            let levels = context.settings.frontlight_levels;
            context.frontlight.set_warmth(levels.warmth);
            context.frontlight.set_intensity(levels.intensity);
        }
        if context.settings.wifi {
            Command::new("scripts/wifi-enable.sh")
                    .status()
                    .ok();
        }
    }
    if id == TaskId::Suspend || id == TaskId::PrepareSuspend {
        tasks.retain(|task| task.id != TaskId::PrepareSuspend);
        if let Some(index) = locate::<Intermission>(view) {
            let rect = *view.child(index).rect();
            view.children_mut().remove(index);
            hub.send(Event::Expose(rect, UpdateMode::Full)).unwrap();
        }
        hub.send(Event::ClockTick).unwrap();
        hub.send(Event::BatteryTick).unwrap();
    }
}

fn power_off(view: &mut View, history: &mut Vec<HistoryItem>, updating: &mut FnvHashMap<u32, Rectangle>, context: &mut Context) {
    let (tx, _rx) = mpsc::channel();
    view.handle_event(&Event::Back, &tx, &mut VecDeque::new(), context);
    while let Some(mut item) = history.pop() {
        item.view.handle_event(&Event::Back, &tx, &mut VecDeque::new(), context);
    }
    let interm = Intermission::new(context.fb.rect(), IntermKind::PowerOff, context);
    updating.retain(|tok, _| context.fb.wait(*tok).is_err());
    interm.render(context.fb.as_mut(), *interm.rect(), &mut context.fonts);
    context.fb.update(interm.rect(), UpdateMode::Full).ok();
}

fn set_wifi(enable: bool, context: &mut Context) {
    if context.settings.wifi == enable {
        return;
    }
    context.settings.wifi = enable;
    if context.settings.wifi {
        Command::new("scripts/wifi-enable.sh")
                .status()
                .ok();
    } else {
        Command::new("scripts/wifi-disable.sh")
                .status()
                .ok();
        context.online = false;
    }
}

enum ExitStatus {
    Quit,
    Reboot,
    PowerOff,
}

pub fn run() -> Result<(), Error> {
    let mut inactive_since = Instant::now();
    let mut exit_status = ExitStatus::Quit;
    let mut fb = KoboFramebuffer::new("/dev/fb0").context("Can't create framebuffer.")?;
    let initial_rotation = CURRENT_DEVICE.transformed_rotation(fb.rotation());
    let startup_rotation = CURRENT_DEVICE.startup_rotation();
    if initial_rotation != startup_rotation {
        fb.set_rotation(startup_rotation).ok();
    }

    let mut context = build_context(Box::new(fb)).context("Can't build context.")?;

    let paths = vec!["/dev/input/event0".to_string(),
                     "/dev/input/event1".to_string()];
    let (raw_sender, raw_receiver) = raw_events(paths);
    let touch_screen = gesture_events(device_events(raw_receiver, context.display));
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

    if context.settings.auto_suspend > 0 {
        let tx6 = tx.clone();
        thread::spawn(move || {
            loop {
                thread::sleep(AUTO_SUSPEND_REFRESH_INTERVAL);
                tx6.send(Event::MightSuspend).unwrap();
            }
        });
    }

    if context.settings.wifi {
        Command::new("scripts/wifi-enable.sh").status().ok();
    } else {
        Command::new("scripts/wifi-disable.sh").status().ok();
    }

    if context.settings.frontlight {
        let levels = context.settings.frontlight_levels;
        context.frontlight.set_warmth(levels.warmth);
        context.frontlight.set_intensity(levels.intensity);
    } else {
        context.frontlight.set_intensity(0.0);
        context.frontlight.set_warmth(0.0);
    }

    let mut tasks: Vec<Task> = Vec::new();
    let mut history: Vec<HistoryItem> = Vec::new();
    let mut view: Box<dyn View> = Box::new(Home::new(context.fb.rect(), &tx, &mut context)?);

    let mut updating = FnvHashMap::default();

    println!("{} is running on a Kobo {}.", APP_NAME,
                                            CURRENT_DEVICE.model);
    println!("The framebuffer resolution is {} by {}.", context.fb.rect().width(),
                                                        context.fb.rect().height());

    let mut bus = VecDeque::with_capacity(4);

    schedule_task(TaskId::CheckBattery, Event::CheckBattery,
                  BATTERY_REFRESH_INTERVAL, &tx, &mut tasks);

    while let Ok(evt) = rx.recv() {
        match evt {
            Event::Device(de) => {
                match de {
                    DeviceEvent::Button { code: ButtonCode::Power, status: ButtonStatus::Released, .. } => {
                        if context.shared || context.covered {
                            continue;
                        }

                        if tasks.iter().any(|task| task.id == TaskId::PrepareSuspend) {
                            resume(TaskId::PrepareSuspend, &mut tasks, view.as_mut(), &tx, &mut context);
                        } else if tasks.iter().any(|task| task.id == TaskId::Suspend) {
                            resume(TaskId::Suspend, &mut tasks, view.as_mut(), &tx, &mut context);
                        } else {
                            let interm = Intermission::new(context.fb.rect(), IntermKind::Suspend, &context);
                            tx.send(Event::Render(*interm.rect(), UpdateMode::Full)).unwrap();
                            schedule_task(TaskId::PrepareSuspend, Event::PrepareSuspend,
                                          PREPARE_SUSPEND_WAIT_DELAY, &tx, &mut tasks);
                            view.children_mut().push(Box::new(interm) as Box<dyn View>);
                        }
                    },
                    DeviceEvent::Button { code: ButtonCode::Light, status: ButtonStatus::Pressed, .. } => {
                        tx.send(Event::ToggleFrontlight).unwrap();
                    },
                    DeviceEvent::CoverOn => {
                        context.covered = true;

                        if context.shared || tasks.iter().any(|task| task.id == TaskId::PrepareSuspend ||
                                                                      task.id == TaskId::Suspend) {
                            continue;
                        }

                        let interm = Intermission::new(context.fb.rect(), IntermKind::Suspend, &context);
                        tx.send(Event::Render(*interm.rect(), UpdateMode::Full)).unwrap();
                        schedule_task(TaskId::PrepareSuspend, Event::PrepareSuspend,
                                      PREPARE_SUSPEND_WAIT_DELAY, &tx, &mut tasks);
                        view.children_mut().push(Box::new(interm) as Box<dyn View>);
                    },
                    DeviceEvent::CoverOff => {
                        context.covered = false;

                        if context.shared {
                            continue;
                        }

                        if tasks.iter().any(|task| task.id == TaskId::PrepareSuspend) {
                            resume(TaskId::PrepareSuspend, &mut tasks, view.as_mut(), &tx, &mut context);
                        } else if tasks.iter().any(|task| task.id == TaskId::Suspend) {
                            resume(TaskId::Suspend, &mut tasks, view.as_mut(), &tx, &mut context);
                        }
                    },
                    DeviceEvent::NetUp => {
                        if tasks.iter().any(|task| task.id == TaskId::PrepareSuspend ||
                                                   task.id == TaskId::Suspend) {
                            continue;
                        }
                        let ip = Command::new("scripts/ip.sh").output()
                                         .map(|o| String::from_utf8_lossy(&o.stdout).trim_end().to_string())
                                         .unwrap_or_default();
                        let essid = Command::new("scripts/essid.sh").output()
                                            .map(|o| String::from_utf8_lossy(&o.stdout).trim_end().to_string())
                                            .unwrap_or_default();
                        let notif = Notification::new(ViewId::NetUpNotif,
                                                      format!("Network is up ({}, {}).", ip, essid),
                                                      &tx, &mut context);
                        context.online = true;
                        view.children_mut().push(Box::new(notif) as Box<dyn View>);
                        if view.is::<Home>() {
                            view.handle_event(&evt, &tx, &mut bus, &mut context);
                        } else {
                            let (tx, _rx) = mpsc::channel();
                            history[0].view.handle_event(&evt, &tx, &mut VecDeque::new(), &mut context);
                        };
                    },
                    DeviceEvent::Plug(power_source) => {
                        if context.plugged {
                            continue;
                        }

                        context.plugged = true;

                        tasks.retain(|task| task.id != TaskId::CheckBattery);

                        if context.covered {
                            continue;
                        }

                        match power_source {
                            PowerSource::Wall => {
                                if tasks.iter().any(|task| task.id == TaskId::Suspend && task.has_occurred()) {
                                    tasks.retain(|task| task.id != TaskId::Suspend);
                                    schedule_task(TaskId::Suspend, Event::Suspend,
                                                  SUSPEND_WAIT_DELAY, &tx, &mut tasks);
                                    continue;
                                }
                            },
                            PowerSource::Host => {
                                if tasks.iter().any(|task| task.id == TaskId::PrepareSuspend) {
                                    resume(TaskId::PrepareSuspend, &mut tasks, view.as_mut(), &tx, &mut context);
                                } else if tasks.iter().any(|task| task.id == TaskId::Suspend) {
                                    resume(TaskId::Suspend, &mut tasks, view.as_mut(), &tx, &mut context);
                                }

                                let confirm = Confirmation::new(ViewId::ConfirmShare,
                                                                Event::PrepareShare,
                                                                "Share storage via USB?".to_string(),
                                                                &mut context);
                                tx.send(Event::Render(*confirm.rect(), UpdateMode::Gui)).unwrap();
                                view.children_mut().push(Box::new(confirm) as Box<dyn View>);
                                inactive_since = Instant::now();
                            },
                        }

                        tx.send(Event::BatteryTick).unwrap();
                    },
                    DeviceEvent::Unplug(..) => {
                        if !context.plugged {
                            continue;
                        }

                        if context.shared {
                            context.shared = false;
                            Command::new("scripts/usb-disable.sh").status().ok();
                            let path = Path::new(SETTINGS_PATH);
                            if let Ok(settings) = load_toml::<Settings, _>(path)
                                                            .map_err(|e| eprintln!("Can't load settings: {}", e)) {
                                context.settings = settings;
                            }
                            if context.settings.wifi {
                                Command::new("scripts/wifi-enable.sh")
                                        .status()
                                        .ok();
                            }
                            if context.settings.frontlight {
                                let levels = context.settings.frontlight_levels;
                                context.frontlight.set_warmth(levels.warmth);
                                context.frontlight.set_intensity(levels.intensity);
                            }
                            if let Some(index) = locate::<Intermission>(view.as_ref()) {
                                let rect = *view.child(index).rect();
                                view.children_mut().remove(index);
                                tx.send(Event::Expose(rect, UpdateMode::Full)).unwrap();
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
                            if context.settings.import.unshare_trigger {
                                let metadata = auto_import(&context.settings.library_path,
                                                           &context.metadata,
                                                           &context.settings.import);
                                context.metadata.append(&mut metadata.unwrap_or_default());
                            }
                            view.handle_event(&Event::Reseed, &tx, &mut bus, &mut context);
                        } else {
                            context.plugged = false;
                            schedule_task(TaskId::CheckBattery, Event::CheckBattery,
                                          BATTERY_REFRESH_INTERVAL, &tx, &mut tasks);
                            if tasks.iter().any(|task| task.id == TaskId::Suspend && task.has_occurred()) {
                                if context.covered {
                                    tasks.retain(|task| task.id != TaskId::Suspend);
                                    schedule_task(TaskId::Suspend, Event::Suspend,
                                                  SUSPEND_WAIT_DELAY, &tx, &mut tasks);
                                } else {
                                    resume(TaskId::Suspend, &mut tasks, view.as_mut(), &tx, &mut context);
                                }
                            } else {
                                tx.send(Event::BatteryTick).unwrap();
                            }
                        }
                    },
                    DeviceEvent::RotateScreen(n) => {
                        if view.might_rotate() {
                            if let Some(rotation_lock) = context.settings.rotation_lock {
                                let orientation = n % 2;
                                if rotation_lock == RotationLock::Current ||
                                   (rotation_lock == RotationLock::Portrait && orientation == 0) ||
                                   (rotation_lock == RotationLock::Landscape && orientation == 1) {
                                    continue;
                                }
                            }
                            tx.send(Event::Select(EntryId::Rotate(n))).unwrap();
                        }
                    },
                    DeviceEvent::UserActivity if context.settings.auto_suspend > 0 => {
                        inactive_since = Instant::now();
                    },
                    _ => {
                        handle_event(view.as_mut(), &evt, &tx, &mut bus, &mut context);
                    }
                }
            },
            Event::CheckBattery => {
                schedule_task(TaskId::CheckBattery, Event::CheckBattery,
                              BATTERY_REFRESH_INTERVAL, &tx, &mut tasks);
                if tasks.iter().any(|task| task.id == TaskId::PrepareSuspend ||
                                           task.id == TaskId::Suspend) {
                    continue;
                }
                if let Ok(v) = context.battery.capacity() {
                    if v < context.settings.battery.power_off {
                        power_off(view.as_mut(), &mut history, &mut updating, &mut context);
                        exit_status = ExitStatus::PowerOff;
                        break;
                    } else if v < context.settings.battery.warn {
                        let notif = Notification::new(ViewId::LowBatteryNotif,
                                                      "The battery capacity is getting low.".to_string(),
                                                      &tx, &mut context);
                        view.children_mut().push(Box::new(notif) as Box<dyn View>);
                    }
                }
            },
            Event::PrepareSuspend => {
                tasks.retain(|task| task.id != TaskId::PrepareSuspend);
                updating.retain(|tok, _| context.fb.wait(*tok).is_err());
                let path = Path::new(SETTINGS_PATH);
                save_toml(&context.settings, path).map_err(|e| eprintln!("Can't save settings: {}", e)).ok();
                let path = context.settings.library_path.join(&context.filename);
                save_json(&context.metadata, path).map_err(|e| eprintln!("Can't save metadata: {}", e)).ok();
                if context.settings.frontlight {
                    context.settings.frontlight_levels = context.frontlight.levels();
                    context.frontlight.set_intensity(0.0);
                    context.frontlight.set_warmth(0.0);
                }
                if context.settings.wifi {
                    Command::new("scripts/wifi-disable.sh")
                            .status()
                            .ok();
                    context.online = false;
                }
                // https://github.com/koreader/koreader/commit/71afe36
                schedule_task(TaskId::Suspend, Event::Suspend,
                              SUSPEND_WAIT_DELAY, &tx, &mut tasks);
            },
            Event::Suspend => {
                println!("{}", Local::now().format("Went to sleep on %B %-d, %Y at %H:%M."));
                Command::new("scripts/suspend.sh")
                        .status()
                        .ok();
                println!("{}", Local::now().format("Woke up on %B %-d, %Y at %H:%M."));
                Command::new("scripts/resume.sh")
                        .status()
                        .ok();
                inactive_since = Instant::now();
            },
            Event::PrepareShare => {
                if context.shared {
                    continue;
                }

                tasks.clear();
                view.handle_event(&Event::Back, &tx, &mut bus, &mut context);
                while let Some(mut item) = history.pop() {
                    item.view.handle_event(&Event::Back, &tx, &mut bus, &mut context);
                    if item.rotation != context.display.rotation {
                        updating.retain(|tok, _| context.fb.wait(*tok).is_err());
                        if let Ok(dims) = context.fb.set_rotation(item.rotation) {
                            raw_sender.send(display_rotate_event(item.rotation)).unwrap();
                            context.display.rotation = item.rotation;
                            context.display.dims = dims;
                        }
                    }
                    view = item.view;
                }
                let path = Path::new(SETTINGS_PATH);
                save_toml(&context.settings, path).map_err(|e| eprintln!("Can't save settings: {}", e)).ok();
                let path = context.settings.library_path.join(&context.filename);
                save_json(&context.metadata, path).map_err(|e| eprintln!("Can't save metadata: {}", e)).ok();
                if context.settings.frontlight {
                    context.settings.frontlight_levels = context.frontlight.levels();
                    context.frontlight.set_intensity(0.0);
                    context.frontlight.set_warmth(0.0);
                }
                if context.settings.wifi {
                    Command::new("scripts/wifi-disable.sh")
                            .status()
                            .ok();
                    context.online = false;
                }
                let interm = Intermission::new(context.fb.rect(), IntermKind::Share, &context);
                tx.send(Event::Render(*interm.rect(), UpdateMode::Full)).unwrap();
                view.children_mut().push(Box::new(interm) as Box<dyn View>);
                tx.send(Event::Share).unwrap();
            },
            Event::Share => {
                if context.shared {
                    continue;
                }

                context.shared = true;
                Command::new("scripts/usb-enable.sh").status().ok();
            },
            Event::Gesture(ge) => {
                match ge {
                    GestureEvent::HoldButton(ButtonCode::Power) => {
                        power_off(view.as_mut(), &mut history, &mut updating, &mut context);
                        exit_status = ExitStatus::PowerOff;
                        break;
                    },
                    GestureEvent::MultiTap(points) => {
                        let mut rect = context.fb.rect();
                        let w = rect.width() as i32;
                        let h = rect.height() as i32;
                        let m = w.min(h);
                        rect.shrink(&Edge::uniform(m / 12));
                        if points[0].dist2(points[1]) >= rect.diag2() {
                            tx.send(Event::Select(EntryId::TakeScreenshot)).unwrap();
                        }
                    },
                    _ => {
                        handle_event(view.as_mut(), &evt, &tx, &mut bus, &mut context);
                    },
                }
            },
            Event::ToggleFrontlight => {
                context.settings.frontlight = !context.settings.frontlight;
                if context.settings.frontlight {
                    let levels = context.settings.frontlight_levels;
                    context.frontlight.set_warmth(levels.warmth);
                    context.frontlight.set_intensity(levels.intensity);
                } else {
                    context.settings.frontlight_levels = context.frontlight.levels();
                    context.frontlight.set_intensity(0.0);
                    context.frontlight.set_warmth(0.0);
                }
                view.handle_event(&Event::ToggleFrontlight, &tx, &mut bus, &mut context);
            },
            Event::Render(mut rect, mode) => {
                render(view.as_ref(), &mut rect, context.fb.as_mut(), &mut context.fonts, &mut updating);
                if let Ok(tok) = context.fb.update(&rect, mode) {
                    updating.insert(tok, rect);
                }
            },
            Event::RenderNoWait(mut rect, mode) => {
                render_no_wait(view.as_ref(), &mut rect, context.fb.as_mut(), &mut context.fonts, &mut updating);
                if let Ok(tok) = context.fb.update(&rect, mode) {
                    updating.insert(tok, rect);
                }
            },
            Event::RenderNoWaitRegion(mut rect, mode) => {
                render_no_wait_region(view.as_ref(), &mut rect, context.fb.as_mut(), &mut context.fonts, &mut updating);
                if let Ok(tok) = context.fb.update(&rect, mode) {
                    updating.insert(tok, rect);
                }
            },
            Event::Expose(mut rect, mode) => {
                expose(view.as_ref(), &mut rect, context.fb.as_mut(), &mut context.fonts, &mut updating);
                if let Ok(tok) = context.fb.update(&rect, mode) {
                    updating.insert(tok, rect);
                }
            },
            Event::Open(info) => {
                let rotation = context.display.rotation;
                if let Some(n) = info.reader.as_ref().and_then(|r| r.rotation) {
                    if n != rotation {
                        updating.retain(|tok, _| context.fb.wait(*tok).is_err());
                        if let Ok(dims) = context.fb.set_rotation(n) {
                            raw_sender.send(display_rotate_event(n)).unwrap();
                            context.display.rotation = n;
                            context.display.dims = dims;
                        }
                    }
                }
                let info2 = info.clone();
                if let Some(r) = Reader::new(context.fb.rect(), *info, &tx, &mut context) {
                    let mut next_view = Box::new(r) as Box<dyn View>;
                    transfer_notifications(view.as_mut(), next_view.as_mut(), &mut context);
                    history.push(HistoryItem {
                        view,
                        rotation,
                        monochrome: context.fb.monochrome()
                    });
                    view = next_view;
                } else {
                    handle_event(view.as_mut(), &Event::Invalid(info2), &tx, &mut bus, &mut context);
                }
            },
            Event::OpenToc(ref toc, chap_index) => {
                let r = Reader::from_toc(context.fb.rect(), toc, chap_index, &tx, &mut context);
                let mut next_view = Box::new(r) as Box<dyn View>;
                transfer_notifications(view.as_mut(), next_view.as_mut(), &mut context);
                history.push(HistoryItem {
                    view,
                    rotation: context.display.rotation,
                    monochrome: context.fb.monochrome(),
                });
                view = next_view;
            },
            Event::Select(EntryId::Launch(app_id)) => {
                view.children_mut().retain(|child| !child.is::<Menu>());
                let monochrome = context.fb.monochrome();
                let mut next_view: Box<View> = match app_id {
                    AppId::Sketch => {
                        context.fb.set_monochrome(true);
                        Box::new(Sketch::new(context.fb.rect(), &tx, &mut context))
                    },
                    AppId::Calculator => Box::new(Calculator::new(context.fb.rect(), &tx, &mut context)?),
                };
                transfer_notifications(view.as_mut(), next_view.as_mut(), &mut context);
                history.push(HistoryItem {
                    view,
                    rotation: context.display.rotation,
                    monochrome
                });
                view = next_view;
            },
            Event::Back => {
                if let Some(item) = history.pop() {
                    view = item.view;
                    if item.monochrome != context.fb.monochrome() {
                        context.fb.set_monochrome(item.monochrome);
                    }
                    if item.rotation != context.display.rotation {
                        updating.retain(|tok, _| context.fb.wait(*tok).is_err());
                        if let Ok(dims) = context.fb.set_rotation(item.rotation) {
                            raw_sender.send(display_rotate_event(item.rotation)).unwrap();
                            context.display.rotation = item.rotation;
                            context.display.dims = dims;
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
                    view.children_mut().push(Box::new(preset_menu) as Box<dyn View>);
                }
            },
            Event::Show(ViewId::Frontlight) => {
                if !context.settings.frontlight {
                    continue;
                }
                let flw = FrontlightWindow::new(&mut context);
                tx.send(Event::Render(*flw.rect(), UpdateMode::Gui)).unwrap();
                view.children_mut().push(Box::new(flw) as Box<dyn View>);
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
            Event::Select(EntryId::ToggleInverted) => {
                context.fb.toggle_inverted();
                tx.send(Event::Render(context.fb.rect(), UpdateMode::Gui)).unwrap();
            },
            Event::Select(EntryId::ToggleMonochrome) => {
                context.fb.toggle_monochrome();
                tx.send(Event::Render(context.fb.rect(), UpdateMode::Gui)).unwrap();
            },
            Event::Select(EntryId::ToggleIntermissionImage(ref kind, ref path)) => {
                let key = kind.key();
                if context.settings.intermission_images.get(key) == Some(path) {
                    context.settings.intermission_images.remove(key);
                } else {
                    context.settings.intermission_images.insert(key.to_string(), path.clone());
                }
            },
            Event::Select(EntryId::Rotate(n)) if n != context.display.rotation => {
                updating.retain(|tok, _| context.fb.wait(*tok).is_err());
                if let Ok(dims) = context.fb.set_rotation(n) {
                    raw_sender.send(display_rotate_event(n)).unwrap();
                    context.display.rotation = n;
                    let fb_rect = Rectangle::from(dims);
                    if context.display.dims != dims {
                        context.display.dims = dims;
                        view.resize(fb_rect, &tx, &mut context);
                    } else {
                        tx.send(Event::Render(context.fb.rect(), UpdateMode::Full)).unwrap();
                    }
                }
            },
            Event::Select(EntryId::SetRotationLock(rotation_lock)) => {
                context.settings.rotation_lock = rotation_lock;
            },
            Event::SetWifi(enable) => {
                set_wifi(enable, &mut context);
            },
            Event::Select(EntryId::ToggleWifi) => {
                set_wifi(!context.settings.wifi, &mut context);
            },
            Event::Select(EntryId::TakeScreenshot) => {
                let name = Local::now().format("screenshot-%Y%m%d_%H%M%S.png");
                let msg = match context.fb.save(&name.to_string()) {
                    Err(e) => format!("Couldn't take screenshot: {}).", e),
                    Ok(_) => format!("Saved {}.", name),
                };
                let notif = Notification::new(ViewId::TakeScreenshotNotif,
                                              msg, &tx, &mut context);
                view.children_mut().push(Box::new(notif) as Box<dyn View>);
            },
            Event::AddDocument(..) | Event::RemoveDocument(..) => {
                if view.is::<Home>() {
                    view.handle_event(&evt, &tx, &mut bus, &mut context);
                } else {
                    let (tx, _rx) = mpsc::channel();
                    history[0].view.handle_event(&evt, &tx, &mut VecDeque::new(), &mut context);
                };
            },
            Event::Notify(msg) => {
                let notif = Notification::new(ViewId::MessageNotif,
                                              msg, &tx, &mut context);
                view.children_mut().push(Box::new(notif) as Box<dyn View>);
            },
            Event::Select(EntryId::Reboot) => {
                exit_status = ExitStatus::Reboot;
                break;
            },
            Event::Select(EntryId::Quit) => {
                break;
            },
            Event::Select(EntryId::StartNickel) => {
                fs::remove_file("bootlock").map_err(|e| {
                    eprintln!("Couldn't remove the bootlock file: {}", e);
                }).ok();
                exit_status = ExitStatus::Reboot;
                break;
            },
            Event::MightSuspend if context.settings.auto_suspend > 0 => {
                if context.shared || tasks.iter().any(|task| task.id == TaskId::PrepareSuspend ||
                                                             task.id == TaskId::Suspend) {
                    inactive_since = Instant::now();
                    continue;
                }
                let seconds = 60 * context.settings.auto_suspend as u64;
                if inactive_since.elapsed() > Duration::from_secs(seconds) {
                    let interm = Intermission::new(context.fb.rect(), IntermKind::Suspend, &context);
                    tx.send(Event::Render(*interm.rect(), UpdateMode::Full)).unwrap();
                    schedule_task(TaskId::PrepareSuspend, Event::PrepareSuspend,
                                  PREPARE_SUSPEND_WAIT_DELAY, &tx, &mut tasks);
                    view.children_mut().push(Box::new(interm) as Box<dyn View>);
                }
            },
            _ => {
                handle_event(view.as_mut(), &evt, &tx, &mut bus, &mut context);
            },
        }

        while let Some(ce) = bus.pop_front() {
            tx.send(ce).unwrap();
        }
    }

    if context.display.rotation != initial_rotation {
        context.fb.set_rotation(initial_rotation).ok();
    }

    if context.settings.frontlight {
        context.settings.frontlight_levels = context.frontlight.levels();
    }

    let path = context.settings.library_path.join(&context.filename);
    save_json(&context.metadata, path).context("Can't save metadata.")?;

    let path = Path::new(SETTINGS_PATH);
    save_toml(&context.settings, path).context("Can't save settings.")?;

    match exit_status {
        ExitStatus::Reboot => {
            Command::new("sync").status().ok();
            Command::new("reboot").status().ok();
        },
        ExitStatus::PowerOff => {
            Command::new("sync").status().ok();
            Command::new("poweroff").status().ok();
        },
        _ => (),
    }

    Ok(())
}
