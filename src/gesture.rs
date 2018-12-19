use std::sync::mpsc::{self, Sender, Receiver};
use std::sync::{Arc, Mutex};
use fnv::FnvHashMap;
use std::f64;
use std::time::Duration;
use std::thread;
use crate::unit::mm_to_px;
use crate::input::{DeviceEvent, FingerStatus, ButtonCode, ButtonStatus};
use crate::view::Event;
use crate::device::CURRENT_DEVICE;
use crate::geom::{Point, Dir, Axis};

pub const JITTER_TOLERANCE_MM: f32 = 6.0;
pub const FINGER_HOLD_DELAY: Duration = Duration::from_millis(666);
pub const BUTTON_HOLD_DELAY: Duration = Duration::from_millis(1500);

#[derive(Debug, Copy, Clone)]
pub enum GestureEvent {
    Tap(Point),
    MultiTap([Point; 2]),
    Swipe {
        dir: Dir,
        start: Point,
        end: Point,
    },
    MultiSwipe {
        dir: Dir,
        starts: [Point; 2],
        ends: [Point; 2],
    },
    Pinch {
        axis: Axis,
        starts: [Point; 2],
        ends: [Point; 2],
        strength: u32,
    },
    Spread {
        axis: Axis,
        starts: [Point; 2],
        ends: [Point; 2],
        strength: u32,
    },
    Rotate {
        angle: f32,
        quarter_turns: i8,
        center: Point,
    },
    HoldFinger(Point),
    HoldButton(ButtonCode),
}

#[derive(Debug)]
pub struct TouchState {
    time: f64,
    initial: Point,
    current: Point,
}

pub fn gesture_events(rx: Receiver<DeviceEvent>) -> Receiver<Event> {
    let (ty, ry) = mpsc::channel();
    thread::spawn(move || parse_gesture_events(&rx, &ty));
    ry
}

pub fn parse_gesture_events(rx: &Receiver<DeviceEvent>, ty: &Sender<Event>) {
    let contacts: Arc<Mutex<FnvHashMap<i32, TouchState>>> = Arc::new(Mutex::new(FnvHashMap::default()));
    let buttons: Arc<Mutex<FnvHashMap<ButtonCode, f64>>> = Arc::new(Mutex::new(FnvHashMap::default()));
    let mut segments: Vec<(Point, Point)> = Vec::new();
    let jitter = mm_to_px(JITTER_TOLERANCE_MM, CURRENT_DEVICE.dpi);
    while let Ok(evt) = rx.recv() {
        ty.send(Event::Device(evt)).unwrap();
        match evt {
            DeviceEvent::Finger { status: FingerStatus::Down, position, id, time } => {
                let mut ct = contacts.lock().unwrap();
                ct.insert(id, TouchState { time, initial: position, current: position });
                let ty = ty.clone();
                let contacts = contacts.clone();
                thread::spawn(move || {
                    thread::sleep(FINGER_HOLD_DELAY);
                    let mut ct = contacts.lock().unwrap();
                    // We don't want to interfere with rotation gestures.
                    // A better fix would be to emit multi-hold events?
                    if ct.len() > 1 {
                        return;
                    }
                    let mut will_remove = None;
                    if let Some(ts) = ct.get(&id) {
                        if (ts.time - time).abs() < f64::EPSILON && (ts.current - position).length() < jitter {
                            ty.send(Event::Gesture(GestureEvent::HoldFinger(position))).unwrap();
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
                        ty.send(Event::Gesture(ge)).unwrap();
                    } else if len == 2 {
                        let ge1 = interpret_segment(segments.pop().unwrap(), jitter);
                        let ge2 = interpret_segment(segments.pop().unwrap(), jitter);
                        match (ge1, ge2) {
                            (GestureEvent::Tap(c1), GestureEvent::Tap(c2)) => {
                                ty.send(Event::Gesture(GestureEvent::MultiTap([c1, c2]))).unwrap();
                            },
                            (GestureEvent::Swipe { dir: d1, start: s1, end: e1, .. },
                             GestureEvent::Swipe { dir: d2, start: s2, end: e2, .. }) if d1 == d2 => {
                                ty.send(Event::Gesture(GestureEvent::MultiSwipe {
                                    dir: d1,
                                    starts: [s1, s2],
                                    ends: [e1, e2],
                                })).unwrap();
                            },
                            (GestureEvent::Swipe { dir: d1, start: s1, end: e1, .. },
                             GestureEvent::Swipe { dir: d2, start: s2, end: e2, .. }) if d1 == d2.opposite() => {
                                let ds = (s2 - s1).length();
                                let de = (e2 - e1).length();
                                if ds > de {
                                    ty.send(Event::Gesture(GestureEvent::Pinch {
                                        axis: d1.axis(),
                                        starts: [s1, s2],
                                        ends: [e1, e2],
                                        strength: (ds - de) as u32,
                                    })).unwrap();
                                } else {
                                    ty.send(Event::Gesture(GestureEvent::Spread {
                                        axis: d1.axis(),
                                        starts: [s1, s2],
                                        ends: [e1, e2],
                                        strength: (de - ds) as u32,
                                    })).unwrap();
                                }
                            },
                            (GestureEvent::Swipe { start: s, end: e, .. }, GestureEvent::Tap(c)) |
                            (GestureEvent::Tap(c), GestureEvent::Swipe { start: s, end: e, .. }) => {
                                // Angle are positive in the counter clockwise direction.
                                let angle = ((e - c).angle() - (s - c).angle()).to_degrees();
                                let quarter_turns = (angle / 90.0).round() as i8;
                                ty.send(Event::Gesture(GestureEvent::Rotate {
                                    angle,
                                    quarter_turns,
                                    center: c,
                                })).unwrap();
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
                    thread::sleep(BUTTON_HOLD_DELAY);
                    let bt = buttons.lock().unwrap();
                    if let Some(&initial_time) = bt.get(&code) {
                        if (initial_time - time).abs() < f64::EPSILON {
                            ty.send(Event::Gesture(GestureEvent::HoldButton(code))).unwrap();
                        }
                    }
                });
            },
            DeviceEvent::Button { status: ButtonStatus::Released, code, .. } => {
                let mut bt = buttons.lock().unwrap();
                bt.remove(&code);
            },
            _ => (),
        }
    }
}

fn interpret_segment((a, b): (Point, Point), jitter: f32) -> GestureEvent {
    let ab = b - a;
    if ab.length() < jitter {
        GestureEvent::Tap(a)
    } else {
        GestureEvent::Swipe {
            dir: ab.dir(),
            start: a,
            end: b,
        }
    }
}
