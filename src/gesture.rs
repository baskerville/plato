use std::sync::mpsc::{self, Sender, Receiver};
use std::sync::{Arc, Mutex};
use fnv::FnvHashMap;
use std::time::Duration;
use std::thread;
use unit::mm_to_in;
use input::{DeviceEvent, FingerStatus, ButtonCode, ButtonStatus};
use device::CURRENT_DEVICE;
use geom::{Point, Dir, Axis};

const JITTER_TOLERANCE_MM: f32 = 5.0;
const FINGER_HOLD_DELAY_MS: u64 = 500;
const BUTTON_HOLD_DELAY_MS: u64 = 1500;

#[derive(Debug, Copy, Clone)]
pub enum GestureEvent {
    Tap {
        center: Point,
        fingers_count: usize,
    },
    Swipe {
        dir: Dir,
        start: Point,
        end: Point,
        fingers_count: usize,
    },
    Pinch {
        axis: Axis,
        target: Point,
        strength: u32,
    },
    Spread {
        axis: Axis,
        target: Point,
        strength: u32,
    },
    Rotate {
        angle: f32,
        quarter_turns: i8,
        center: Point,
    },
    Finger {
        id: i32,
        time: f64,
        status: FingerStatus,
        position: Point,
    },
    Button {
        time: f64,
        code: ButtonCode,
        status: ButtonStatus,
    },
    HoldFinger(Point),
    HoldButton(ButtonCode),
}

impl GestureEvent {
    pub fn from_device_event(evt: DeviceEvent) -> GestureEvent {
        match evt {
            DeviceEvent::Finger { id, time, status, position } => {
                GestureEvent::Finger { id, time, status, position }
            },
            DeviceEvent::Button { time, code, status } => {
                GestureEvent::Button { time, code, status }
            }
        }
    }
}

#[derive(Debug)]
pub struct TouchState {
    time: f64,
    initial: Point,
    current: Point,
}

pub fn gesture_events(rx: Receiver<DeviceEvent>) -> Receiver<GestureEvent> {
    let (ty, ry) = mpsc::channel();
    thread::spawn(move || parse_gesture_events(&rx, &ty));
    ry
}

pub fn parse_gesture_events(rx: &Receiver<DeviceEvent>, ty: &Sender<GestureEvent>) {
    let contacts: Arc<Mutex<FnvHashMap<i32, TouchState>>> = Arc::new(Mutex::new(FnvHashMap::default()));
    let buttons: Arc<Mutex<FnvHashMap<ButtonCode, f64>>> = Arc::new(Mutex::new(FnvHashMap::default()));
    let mut segments: Vec<(Point, Point)> = Vec::new();
    let jitter = CURRENT_DEVICE.dpi as f32 * mm_to_in(JITTER_TOLERANCE_MM);
    while let Ok(evt) = rx.recv() {
        ty.send(GestureEvent::from_device_event(evt)).unwrap();
        match evt {
            DeviceEvent::Finger { status: FingerStatus::Down, position, id, time } => {
                let mut ct = contacts.lock().unwrap();
                ct.insert(id, TouchState { time, initial: position, current: position });
                let ty = ty.clone();
                let contacts = contacts.clone();
                thread::spawn(move || {
                    thread::sleep(Duration::from_millis(FINGER_HOLD_DELAY_MS));
                    let mut ct = contacts.lock().unwrap();
                    let mut will_remove = None;
                    if let Some(ts) = ct.get(&id) {
                        if ts.time == time && (ts.current - position).length() < jitter {
                            ty.send(GestureEvent::HoldFinger(position)).unwrap();
                            will_remove = Some(id);
                        }
                    }
                    if let Some(id) = will_remove {
                        ct.remove(&id);
                    }
                });
            },
            DeviceEvent::Finger { status: FingerStatus::Motion, position, id, .. } => {
                let mut ct = contacts.lock().unwrap();
                if let Some(ref mut ts) = ct.get_mut(&id) {
                    ts.current = position;
                }
            },
            DeviceEvent::Finger { status: FingerStatus::Up, position, id, .. } => {
                let mut ct = contacts.lock().unwrap();
                if let Some(TouchState { initial, .. }) = ct.remove(&id) {
                    segments.push((initial, position));
                }
                if ct.is_empty() && !segments.is_empty() {
                    let len = segments.len();
                    if len == 1 {
                        let ge = interpret_segment(segments.pop().unwrap(), jitter);
                        ty.send(ge).unwrap();
                    } else if len == 2 {
                        let ge1 = interpret_segment(segments.pop().unwrap(), jitter);
                        let ge2 = interpret_segment(segments.pop().unwrap(), jitter);
                        match (ge1, ge2) {
                            (GestureEvent::Tap { center: c1, .. }, GestureEvent::Tap { center: c2, .. }) => {
                                ty.send(GestureEvent::Tap {
                                    center: (c1 + c2) / 2,
                                    fingers_count: 2,
                                }).unwrap();
                            }
                            (GestureEvent::Swipe { dir: d1, start: s1, end: e1, .. },
                             GestureEvent::Swipe { dir: d2, start: s2, end: e2, .. }) if d1 == d2 => {
                                ty.send(GestureEvent::Swipe {
                                    dir: d1,
                                    start: (s1 + s2) / 2,
                                    end: (e1 + e2) / 2,
                                    fingers_count: 2,
                                }).unwrap();
                            },
                            (GestureEvent::Swipe { dir: d1, start: s1, end: e1, .. },
                             GestureEvent::Swipe { dir: d2, start: s2, end: e2, .. }) if d1 == d2.opposite() => {
                                let ds = (s2 - s1).length();
                                let de = (e2 - e1).length();
                                if ds > de {
                                    ty.send(GestureEvent::Pinch {
                                        axis: d1.axis(),
                                        target: (e1 + e2) / 2,
                                        strength: (ds - de) as u32,
                                    }).unwrap();
                                } else {
                                    ty.send(GestureEvent::Spread {
                                        axis: d1.axis(),
                                        target: (s1 + s2) / 2,
                                        strength: (de - ds) as u32,
                                    }).unwrap();
                                }
                            },
                            (GestureEvent::Swipe { start: s, end: e, .. }, GestureEvent::Tap { center: c, .. }) | 
                            (GestureEvent::Tap { center: c, .. }, GestureEvent::Swipe { start: s, end: e, .. }) => {
                                let angle = ((s - c).angle() - (e - c).angle()).to_degrees();
                                let quarter_turns = (angle.signum() * (angle / 90.0).abs().ceil()) as i8;
                                ty.send(GestureEvent::Rotate {
                                    angle: angle,
                                    quarter_turns: quarter_turns,
                                    center: c,
                                }).unwrap();
                            },
                            _ => (),
                        }
                    } else {
                        segments.clear();
                    }
                }
            },
            DeviceEvent::Button { status: ButtonStatus::Pressed, code, time } => {
                let mut bt = buttons.lock().unwrap();
                bt.insert(code, time);
                let ty = ty.clone();
                let buttons = buttons.clone();
                thread::spawn(move || {
                    thread::sleep(Duration::from_millis(BUTTON_HOLD_DELAY_MS));
                    let bt = buttons.lock().unwrap();
                    if let Some(&initial_time) = bt.get(&code) {
                        if initial_time == time {
                            ty.send(GestureEvent::HoldButton(code)).unwrap();
                        }
                    }
                });
            },
            DeviceEvent::Button { status: ButtonStatus::Released, code, .. } => {
                let mut bt = buttons.lock().unwrap();
                bt.remove(&code);
            },
        }
    }
}

fn interpret_segment((a, b): (Point, Point), jitter: f32) -> GestureEvent {
    let ab = b - a;
    if ab.length() < jitter {
        GestureEvent::Tap {
            center: a,
            fingers_count: 1,
        }
    } else {
        GestureEvent::Swipe {
            dir: ab.dir(),
            start: a,
            end: b,
            fingers_count: 1,
        }
    }
}
