use std::fs;
use std::env;
use std::thread;
use std::process::Command;
use std::path::Path;
use std::sync::mpsc::{self, Receiver, Sender};
use std::collections::{BTreeMap, VecDeque};
use std::time::{Duration, Instant};
use anyhow::{Error, Context as ResultExt, format_err};
use fxhash::FxHashMap;
use chrono::Local;
use globset::Glob;
use walkdir::WalkDir;
use rand_core::SeedableRng;
use rand_xoshiro::Xoroshiro128Plus;
use crate::dictionary::{Dictionary, load_dictionary_from_file};
use crate::framebuffer::{Framebuffer, KoboFramebuffer, Display, UpdateMode};
use crate::view::{View, Event, EntryId, EntryKind, ViewId, AppCmd};
use crate::view::{render, render_region, render_no_wait, render_no_wait_region, handle_event, expose};
use crate::view::common::{locate, locate_by_id, transfer_notifications, overlapping_rectangle};
use crate::view::common::{toggle_input_history_menu, toggle_keyboard_layout_menu};
use crate::view::frontlight::FrontlightWindow;
use crate::view::menu::{Menu, MenuKind};
use crate::view::keyboard::{Layout};
use crate::view::dictionary::Dictionary as DictionaryApp;
use crate::view::calculator::Calculator;
use crate::view::sketch::Sketch;
use crate::input::{DeviceEvent, PowerSource, ButtonCode, ButtonStatus, VAL_RELEASE, VAL_PRESS};
use crate::input::{raw_events, device_events, usb_events, display_rotate_event, button_scheme_event};
use crate::gesture::{GestureEvent, gesture_events};
use crate::helpers::{load_json, load_toml, save_toml, IsHidden};
use crate::settings::{ButtonScheme, Settings, SETTINGS_PATH, RotationLock};
use crate::frontlight::{Frontlight, StandardFrontlight, NaturalFrontlight, PremixedFrontlight};
use crate::lightsensor::{LightSensor, KoboLightSensor};
use crate::battery::{Battery, KoboBattery};
use crate::geom::{Rectangle, Edge};
use crate::view::home::Home;
use crate::view::reader::Reader;
use crate::view::confirmation::Confirmation;
use crate::view::intermission::{Intermission, IntermKind};
use crate::view::notification::Notification;
use crate::device::{CURRENT_DEVICE, Orientation, FrontlightKind};
use crate::library::Library;
use crate::font::Fonts;
use crate::rtc::Rtc;

pub const APP_NAME: &str = "Plato";
const FB_DEVICE: &str = "/dev/fb0";
const RTC_DEVICE: &str = "/dev/rtc0";
const EVENT_BUTTONS: &str = "/dev/input/event0";
const EVENT_TOUCH_SCREEN: &str = "/dev/input/event1";
const KOBO_UPDATE_BUNDLE: &str = "/mnt/onboard/.kobo/KoboRoot.tgz";
const KEYBOARD_LAYOUTS_DIRNAME: &str = "keyboard-layouts";
const DICTIONARIES_DIRNAME: &str = "dictionaries";
const INPUT_HISTORY_SIZE: usize = 32;

const CLOCK_REFRESH_INTERVAL: Duration = Duration::from_secs(60);
const BATTERY_REFRESH_INTERVAL: Duration = Duration::from_secs(299);
const AUTO_SUSPEND_REFRESH_INTERVAL: Duration = Duration::from_secs(60);
const SUSPEND_WAIT_DELAY: Duration = Duration::from_secs(15);
const PREPARE_SUSPEND_WAIT_DELAY: Duration = Duration::from_secs(3);

pub struct Context {
    pub fb: Box<dyn Framebuffer>,
    pub rtc: Option<Rtc>,
    pub display: Display,
    pub settings: Settings,
    pub library: Library,
    pub fonts: Fonts,
    pub dictionaries: BTreeMap<String, Dictionary>,
    pub keyboard_layouts: BTreeMap<String, Layout>,
    pub input_history: FxHashMap<ViewId, VecDeque<String>>,
    pub frontlight: Box<dyn Frontlight>,
    pub battery: Box<dyn Battery>,
    pub lightsensor: Box<dyn LightSensor>,
    pub notification_index: u8,
    pub kb_rect: Rectangle,
    pub rng: Xoroshiro128Plus,
    pub plugged: bool,
    pub covered: bool,
    pub shared: bool,
    pub online: bool,
}

impl Context {
    pub fn new(fb: Box<dyn Framebuffer>, rtc: Option<Rtc>, library: Library,
               settings: Settings, fonts: Fonts, battery: Box<dyn Battery>,
               frontlight: Box<dyn Frontlight>, lightsensor: Box<dyn LightSensor>) -> Context {
        let dims = fb.dims();
        let rotation = CURRENT_DEVICE.transformed_rotation(fb.rotation());
        let rng = Xoroshiro128Plus::seed_from_u64(Local::now().timestamp_nanos() as u64);
        Context { fb, rtc, display: Display { dims, rotation },
                  library, settings, fonts, dictionaries: BTreeMap::new(),
                  keyboard_layouts: BTreeMap::new(), input_history: FxHashMap::default(),
                  battery, frontlight, lightsensor, notification_index: 0,
                  kb_rect: Rectangle::default(), rng, plugged: false, covered: false,
                  shared: false, online: false }
    }

    pub fn batch_import(&mut self) {
        let prefix = self.library.home.clone();
        let import_settings = self.settings.import.clone();
        self.library.import(&prefix, &import_settings);
        let selected_library = self.settings.selected_library;
        for (index, library_settings) in self.settings.libraries.iter().enumerate() {
            if index == selected_library {
                continue;
            }
            let mut library = Library::new(&library_settings.path, library_settings.mode);
            library.import(&library_settings.path, &import_settings);
            library.flush();
        }
    }

    pub fn load_keyboard_layouts(&mut self) {
        let glob = Glob::new("**/*.json").unwrap().compile_matcher();
        for entry in WalkDir::new(Path::new(KEYBOARD_LAYOUTS_DIRNAME)).min_depth(1)
                             .into_iter().filter_entry(|e| !e.is_hidden()) {
            if entry.is_err() {
                continue;
            }
            let entry = entry.unwrap();
            let path = entry.path();
            if !glob.is_match(path) {
                continue;
            }
            if let Ok(layout) = load_json::<Layout, _>(path) {
                self.keyboard_layouts.insert(layout.name.clone(), layout);
            }
        }
    }

    pub fn load_dictionaries(&mut self) {
        let glob = Glob::new("**/*.index").unwrap().compile_matcher();
        for entry in WalkDir::new(Path::new(DICTIONARIES_DIRNAME)).min_depth(1)
                             .into_iter().filter_entry(|e| !e.is_hidden()) {
            if entry.is_err() {
                continue;
            }
            let entry = entry.unwrap();
            if !glob.is_match(entry.path()) {
                continue;
            }
            let index_path = entry.path().to_path_buf();
            let mut content_path = index_path.clone();
            content_path.set_extension("dict.dz");
            if !content_path.exists() {
                content_path.set_extension("");
            }
            if let Ok(mut dict) = load_dictionary_from_file(&content_path, &index_path) {
                let name = dict.short_name().ok().unwrap_or_else(|| {
                    index_path.file_stem()
                        .map(|s| s.to_string_lossy().into_owned())
                        .unwrap_or_default()
                });
                self.dictionaries.insert(name, dict);
            }
        }
    }

    pub fn record_input(&mut self, text: &str, id: ViewId) {
        if text.is_empty() {
            return;
        }

        let history = self.input_history.entry(id)
                          .or_insert_with(|| VecDeque::new());

        if history.front().map(String::as_str) != Some(text) {
            history.push_front(text.to_string());
        }

        if history.len() > INPUT_HISTORY_SIZE {
            history.pop_back();
        }
    }

    pub fn set_frontlight(&mut self, enable: bool) {
        self.settings.frontlight = enable;

        if enable {
            let levels = self.settings.frontlight_levels;
            self.frontlight.set_warmth(levels.warmth);
            self.frontlight.set_intensity(levels.intensity);
        } else {
            self.settings.frontlight_levels = self.frontlight.levels();
            self.frontlight.set_intensity(0.0);
            self.frontlight.set_warmth(0.0);
        }
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
    let rtc = Rtc::new(RTC_DEVICE)
                  .map_err(|e| eprintln!("Can't open RTC device: {}", e))
                  .ok();
    let path = Path::new(SETTINGS_PATH);
    let settings = load_toml::<Settings, _>(path);

    if let Err(ref e) = settings {
        if path.exists() {
            eprintln!("Can't load settings: {}", e);
        }
    }

    let mut settings = settings.unwrap_or_default();

    if settings.libraries.is_empty() {
        return Err(format_err!("No libraries found."));
    }

    if settings.selected_library >= settings.libraries.len() {
        settings.selected_library = 0;
    }

    let library_settings = &settings.libraries[settings.selected_library];
    let library = Library::new(&library_settings.path, library_settings.mode);

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

    Ok(Context::new(fb, rtc, library, settings,
                    fonts, battery, frontlight, lightsensor))
}

fn schedule_task(id: TaskId, event: Event, delay: Duration, hub: &Sender<Event>, tasks: &mut Vec<Task>) {
    let (ty, ry) = mpsc::channel();
    let hub2 = hub.clone();
    tasks.push(Task { id, chan: ry });
    thread::spawn(move || {
        thread::sleep(delay);
        if ty.send(()).is_ok() {
            hub2.send(event).ok();
        }
    });
}

fn resume(id: TaskId, tasks: &mut Vec<Task>, view: &mut dyn View, hub: &Sender<Event>, context: &mut Context) {
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
            hub.send(Event::Expose(rect, UpdateMode::Full)).ok();
        }
        hub.send(Event::ClockTick).ok();
        hub.send(Event::BatteryTick).ok();
    }
}

fn power_off(view: &mut dyn View, history: &mut Vec<HistoryItem>, updating: &mut FxHashMap<u32, Rectangle>, context: &mut Context) {
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
    let mut fb = KoboFramebuffer::new(FB_DEVICE).context("Can't create framebuffer.")?;
    let initial_rotation = CURRENT_DEVICE.transformed_rotation(fb.rotation());
    let startup_rotation = CURRENT_DEVICE.startup_rotation();
    if initial_rotation != startup_rotation {
        fb.set_rotation(startup_rotation).ok();
    }

    let mut context = build_context(Box::new(fb)).context("Can't build context.")?;
    if context.settings.import.startup_trigger {
        context.batch_import();
    }
    context.load_dictionaries();
    context.load_keyboard_layouts();

    let paths = vec![EVENT_BUTTONS.to_string(), EVENT_TOUCH_SCREEN.to_string()];
    let (raw_sender, raw_receiver) = raw_events(paths);
    let touch_screen = gesture_events(device_events(raw_receiver, context.display, context.settings.button_scheme));
    let usb_port = usb_events();

    let (tx, rx) = mpsc::channel();
    let tx2 = tx.clone();

    thread::spawn(move || {
        while let Ok(evt) = touch_screen.recv() {
            tx2.send(evt).ok();
        }
    });

    let tx3 = tx.clone();
    thread::spawn(move || {
        while let Ok(evt) = usb_port.recv() {
            tx3.send(Event::Device(evt)).ok();
        }
    });

    let tx4 = tx.clone();
    thread::spawn(move || {
        loop {
            thread::sleep(CLOCK_REFRESH_INTERVAL);
            tx4.send(Event::ClockTick).ok();
        }
    });

    let tx5 = tx.clone();
    thread::spawn(move || {
        loop {
            thread::sleep(BATTERY_REFRESH_INTERVAL);
            tx5.send(Event::BatteryTick).ok();
        }
    });

    if context.settings.auto_suspend > 0 {
        let tx6 = tx.clone();
        thread::spawn(move || {
            loop {
                thread::sleep(AUTO_SUSPEND_REFRESH_INTERVAL);
                tx6.send(Event::MightSuspend).ok();
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

    let mut updating = FxHashMap::default();
    let current_dir = env::current_dir()?;

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
                            tx.send(Event::Render(*interm.rect(), UpdateMode::Full)).ok();
                            schedule_task(TaskId::PrepareSuspend, Event::PrepareSuspend,
                                          PREPARE_SUSPEND_WAIT_DELAY, &tx, &mut tasks);
                            view.children_mut().push(Box::new(interm) as Box<dyn View>);
                        }
                    },
                    DeviceEvent::Button { code: ButtonCode::Light, status: ButtonStatus::Pressed, .. } => {
                        tx.send(Event::ToggleFrontlight).ok();
                    },
                    DeviceEvent::CoverOn => {
                        context.covered = true;

                        if !context.settings.sleep_cover || context.shared ||
                           tasks.iter().any(|task| task.id == TaskId::PrepareSuspend ||
                                                   task.id == TaskId::Suspend) {
                            continue;
                        }

                        let interm = Intermission::new(context.fb.rect(), IntermKind::Suspend, &context);
                        tx.send(Event::Render(*interm.rect(), UpdateMode::Full)).ok();
                        schedule_task(TaskId::PrepareSuspend, Event::PrepareSuspend,
                                      PREPARE_SUSPEND_WAIT_DELAY, &tx, &mut tasks);
                        view.children_mut().push(Box::new(interm) as Box<dyn View>);
                    },
                    DeviceEvent::CoverOff => {
                        context.covered = false;

                        if context.shared {
                            continue;
                        }

                        if !context.settings.sleep_cover {
                            if tasks.iter().any(|task| task.id == TaskId::Suspend && task.has_occurred()) {
                                tasks.retain(|task| task.id != TaskId::Suspend);
                                schedule_task(TaskId::Suspend, Event::Suspend,
                                              SUSPEND_WAIT_DELAY, &tx, &mut tasks);
                            }
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

                                if context.settings.auto_share {
                                    tx.send(Event::PrepareShare).ok();
                                } else {
                                    let confirm = Confirmation::new(ViewId::ConfirmShare,
                                                                    Event::PrepareShare,
                                                                    "Share storage via USB?".to_string(),
                                                                    &mut context);
                                    tx.send(Event::Render(*confirm.rect(), UpdateMode::Gui)).ok();
                                    view.children_mut().push(Box::new(confirm) as Box<dyn View>);
                                }

                                inactive_since = Instant::now();
                            },
                        }

                        tx.send(Event::BatteryTick).ok();
                    },
                    DeviceEvent::Unplug(..) => {
                        if !context.plugged {
                            continue;
                        }

                        if context.shared {
                            context.shared = false;
                            Command::new("scripts/usb-disable.sh").status().ok();
                            env::set_current_dir(&current_dir)
                                .map_err(|e| eprintln!("Unable to set current directory to {}: {}", current_dir.display(), e))
                                .ok();
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
                                tx.send(Event::Expose(rect, UpdateMode::Full)).ok();
                            }
                            if Path::new(KOBO_UPDATE_BUNDLE).exists() {
                                tx.send(Event::Select(EntryId::Reboot)).ok();
                            }
                            context.library.reload();
                            if context.settings.import.unshare_trigger {
                                context.batch_import();
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
                                tx.send(Event::BatteryTick).ok();
                            }
                        }
                    },
                    DeviceEvent::RotateScreen(n) => {
                        if context.shared || tasks.iter().any(|task| task.id == TaskId::PrepareSuspend ||
                                                                     task.id == TaskId::Suspend) {
                            continue;
                        }

                        if let Some(rotation_lock) = context.settings.rotation_lock {
                            let orientation = CURRENT_DEVICE.orientation(n);
                            if rotation_lock == RotationLock::Current ||
                               (rotation_lock == RotationLock::Portrait && orientation == Orientation::Landscape) ||
                               (rotation_lock == RotationLock::Landscape && orientation == Orientation::Portrait) {
                                continue;
                            }
                        }

                        tx.send(Event::Select(EntryId::Rotate(n))).ok();
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
                context.library.flush();

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
                if context.settings.auto_power_off > 0 {
                    context.rtc.iter().for_each(|rtc| {
                        rtc.set_alarm(context.settings.auto_power_off)
                           .map_err(|e| eprintln!("Can't set alarm: {}.", e))
                           .ok();
                    });
                }
                println!("{}", Local::now().format("Went to sleep on %B %-d, %Y at %H:%M."));
                Command::new("scripts/suspend.sh")
                        .status()
                        .ok();
                println!("{}", Local::now().format("Woke up on %B %-d, %Y at %H:%M."));
                Command::new("scripts/resume.sh")
                        .status()
                        .ok();
                inactive_since = Instant::now();
                if context.settings.auto_power_off > 0 {
                    if let Some(enabled) = context.rtc.as_ref()
                                                  .and_then(|rtc| rtc.is_alarm_enabled()
                                                                     .map_err(|e| eprintln!("Can't get alarm: {}", e))
                                                                     .ok()) {
                        if enabled {
                            context.rtc.iter().for_each(|rtc| {
                                rtc.disable_alarm()
                                   .map_err(|e| eprintln!("Can't disable alarm: {}.", e))
                                   .ok();
                            });
                        } else {
                            power_off(view.as_mut(), &mut history, &mut updating, &mut context);
                            exit_status = ExitStatus::PowerOff;
                            break;
                        }
                    }
                }
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
                            raw_sender.send(display_rotate_event(item.rotation)).ok();
                            context.display.rotation = item.rotation;
                            context.display.dims = dims;
                        }
                    }
                    view = item.view;
                }
                let path = Path::new(SETTINGS_PATH);
                save_toml(&context.settings, path).map_err(|e| eprintln!("Can't save settings: {}", e)).ok();
                context.library.flush();

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
                tx.send(Event::Render(*interm.rect(), UpdateMode::Full)).ok();
                view.children_mut().push(Box::new(interm) as Box<dyn View>);
                tx.send(Event::Share).ok();
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
                    GestureEvent::HoldButtonLong(ButtonCode::Power) => {
                        power_off(view.as_mut(), &mut history, &mut updating, &mut context);
                        exit_status = ExitStatus::PowerOff;
                        break;
                    },
                    GestureEvent::MultiTap(mut points) => {
                        let mut rect = context.fb.rect();
                        let w = rect.width() as i32;
                        let h = rect.height() as i32;
                        let m = w.min(h);
                        rect.shrink(&Edge::uniform(m / 12));
                        if points[0].x > points[1].x {
                            points.swap(0, 1);
                        }
                        if points[0].dist2(points[1]) >= rect.diag2() {
                            if points[0].y < points[1].y {
                                tx.send(Event::Select(EntryId::TakeScreenshot)).ok();
                            } else {
                                tx.send(Event::Render(context.fb.rect(), UpdateMode::Full)).ok();
                            }
                        }
                    },
                    _ => {
                        handle_event(view.as_mut(), &evt, &tx, &mut bus, &mut context);
                    },
                }
            },
            Event::ToggleFrontlight => {
                context.set_frontlight(!context.settings.frontlight);
                view.handle_event(&Event::ToggleFrontlight, &tx, &mut bus, &mut context);
            },
            Event::Render(mut rect, mode) => {
                render(view.as_ref(), &mut rect, context.fb.as_mut(), &mut context.fonts, &mut updating);
                if let Ok(tok) = context.fb.update(&rect, mode) {
                    updating.insert(tok, rect);
                }
            },
            Event::RenderRegion(mut rect, mode) => {
                render_region(view.as_ref(), &mut rect, context.fb.as_mut(), &mut context.fonts, &mut updating);
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
                if let Some(n) = info.reader.as_ref()
                                     .and_then(|r| r.rotation.map(|n| CURRENT_DEVICE.from_canonical(n))) {
                    if CURRENT_DEVICE.orientation(n) != CURRENT_DEVICE.orientation(rotation) {
                        updating.retain(|tok, _| context.fb.wait(*tok).is_err());
                        if let Ok(dims) = context.fb.set_rotation(n) {
                            raw_sender.send(display_rotate_event(n)).ok();
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
                    if context.display.rotation != rotation {
                        if let Ok(dims) = context.fb.set_rotation(rotation) {
                            raw_sender.send(display_rotate_event(rotation)).ok();
                            context.display.rotation = rotation;
                            context.display.dims = dims;
                        }
                    }
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
            Event::Select(EntryId::Launch(app_cmd)) => {
                view.children_mut().retain(|child| !child.is::<Menu>());
                let monochrome = context.fb.monochrome();
                let mut next_view: Box<dyn View> = match app_cmd {
                    AppCmd::Sketch => {
                        context.fb.set_monochrome(true);
                        Box::new(Sketch::new(context.fb.rect(), &tx, &mut context))
                    },
                    AppCmd::Calculator => Box::new(Calculator::new(context.fb.rect(), &tx, &mut context)?),
                    AppCmd::Dictionary { ref query, ref language } => Box::new(DictionaryApp::new(context.fb.rect(), query, language, &tx, &mut context)),
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
                    if CURRENT_DEVICE.orientation(item.rotation) != CURRENT_DEVICE.orientation(context.display.rotation) {
                        updating.retain(|tok, _| context.fb.wait(*tok).is_err());
                        if let Ok(dims) = context.fb.set_rotation(item.rotation) {
                            raw_sender.send(display_rotate_event(item.rotation)).ok();
                            context.display.rotation = item.rotation;
                            context.display.dims = dims;
                        }
                    }
                    view.handle_event(&Event::Reseed, &tx, &mut bus, &mut context);
                } else {
                    break;
                }
            },
            Event::TogglePresetMenu(rect, index) => {
                if let Some(index) = locate_by_id(view.as_ref(), ViewId::PresetMenu) {
                    let rect = *view.child(index).rect();
                    view.children_mut().remove(index);
                    tx.send(Event::Expose(rect, UpdateMode::Gui)).ok();
                } else {
                    let preset_menu = Menu::new(rect, ViewId::PresetMenu, MenuKind::Contextual,
                                                vec![EntryKind::Command("Remove".to_string(),
                                                                        EntryId::RemovePreset(index))],
                                                &mut context);
                    tx.send(Event::Render(*preset_menu.rect(), UpdateMode::Gui)).ok();
                    view.children_mut().push(Box::new(preset_menu) as Box<dyn View>);
                }
            },
            Event::Show(ViewId::Frontlight) => {
                if !context.settings.frontlight {
                    context.set_frontlight(true);
                    view.handle_event(&Event::ToggleFrontlight, &tx, &mut bus, &mut context);
                }
                let flw = FrontlightWindow::new(&mut context);
                tx.send(Event::Render(*flw.rect(), UpdateMode::Gui)).ok();
                view.children_mut().push(Box::new(flw) as Box<dyn View>);
            },
            Event::ToggleInputHistoryMenu(id, rect) => {
                toggle_input_history_menu(view.as_mut(), id, rect, None, &tx, &mut context);
            },
            Event::ToggleNear(ViewId::KeyboardLayoutMenu, rect) => {
                toggle_keyboard_layout_menu(view.as_mut(), rect, None, &tx, &mut context);
            },
            Event::Close(ViewId::Frontlight) => {
                if let Some(index) = locate::<FrontlightWindow>(view.as_ref()) {
                    let rect = *view.child(index).rect();
                    view.children_mut().remove(index);
                    tx.send(Event::Expose(rect, UpdateMode::Gui)).ok();
                }
            },
            Event::Close(id) => {
                if let Some(index) = locate_by_id(view.as_ref(), id) {
                    let rect = overlapping_rectangle(view.child(index));
                    tx.send(Event::Expose(rect, UpdateMode::Gui)).ok();
                    view.children_mut().remove(index);
                }
            },
            Event::Select(EntryId::ToggleInverted) => {
                context.fb.toggle_inverted();
                tx.send(Event::Render(context.fb.rect(), UpdateMode::Gui)).ok();
            },
            Event::Select(EntryId::ToggleMonochrome) => {
                context.fb.toggle_monochrome();
                tx.send(Event::Render(context.fb.rect(), UpdateMode::Gui)).ok();
            },
            Event::Select(EntryId::ToggleIntermissionImage(ref kind, ref path)) => {
                let key = kind.key();
                if context.settings.intermission_images.get(key) == Some(path) {
                    context.settings.intermission_images.remove(key);
                } else {
                    context.settings.intermission_images.insert(key.to_string(), path.clone());
                }
            },
            Event::Select(EntryId::Rotate(n)) if n != context.display.rotation && view.might_rotate() => {
                updating.retain(|tok, _| context.fb.wait(*tok).is_err());
                if let Ok(dims) = context.fb.set_rotation(n) {
                    raw_sender.send(display_rotate_event(n)).ok();
                    context.display.rotation = n;
                    let fb_rect = Rectangle::from(dims);
                    if context.display.dims != dims {
                        context.display.dims = dims;
                        view.resize(fb_rect, &tx, &mut context);
                    } else {
                        tx.send(Event::Render(context.fb.rect(), UpdateMode::Full)).ok();
                    }
                }
            },
            Event::Select(EntryId::SetRotationLock(rotation_lock)) => {
                context.settings.rotation_lock = rotation_lock;

            },
            Event::Select(EntryId::SetButtonScheme(button_scheme)) => {
                context.settings.button_scheme = button_scheme;

                // Sending a pseudo event into the raw_events channel toggles the inversion in the device_events channel
                match button_scheme {
                    ButtonScheme::Natural => {
                        raw_sender.send(button_scheme_event(VAL_RELEASE)).ok();
                    },
                    ButtonScheme::Inverted => {
                        raw_sender.send(button_scheme_event(VAL_PRESS)).ok();
                    }
                }
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
            Event::AddDocument(..) => {
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
            Event::Select(EntryId::RebootInNickel) => {
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
                    tx.send(Event::Render(*interm.rect(), UpdateMode::Full)).ok();
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
            tx.send(ce).ok();
        }
    }

    if context.display.rotation != initial_rotation {
        context.fb.set_rotation(initial_rotation).ok();
    }

    if context.settings.frontlight {
        context.settings.frontlight_levels = context.frontlight.levels();
    }

    context.library.flush();

    let path = Path::new(SETTINGS_PATH);
    save_toml(&context.settings, path).context("Can't save settings.")?;

    match exit_status {
        ExitStatus::Reboot => {
            Command::new("sync").status().ok();
            Command::new("reboot").status().ok();
        },
        ExitStatus::PowerOff => {
            Command::new("sync").status().ok();
            Command::new("poweroff").arg("-f").status().ok();
        },
        _ => (),
    }

    Ok(())
}
