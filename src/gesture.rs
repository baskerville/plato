use std::sync::mpsc::{self, Sender, Receiver, TryRecvError};
use std::sync::{Arc, Mutex};
use std::collections::HashMap;
use std::time::Duration;
use std::thread;
use unit::mm_to_in;
use input::{DeviceEvent, FingerStatus};
use device::Device;
use geom::{Point, Dir, Axis};

const JITTER_TOLERANCE_MM: f32 = 1.5;
const LONG_PRESS_DELAY_MS: u64 = 1200;

#[derive(Debug)]
pub enum GestureEvent {
    Tap {
        center: Point,
        fingers_count: usize,
    },
    Hold {
        center: Point,
        long: bool,
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
        center: Point,
    },
    Relay(DeviceEvent),
}

#[derive(Debug)]
pub struct TouchState {
    initial: Point,
    current: Point,
}

pub fn gesture_events(rx: Receiver<DeviceEvent>) -> Receiver<GestureEvent> {
    let (ty, ry) = mpsc::channel();
    thread::spawn(move || parse_gesture_events(rx, ty));
    ry
}

pub fn parse_gesture_events(rx: Receiver<DeviceEvent>, ty: Sender<GestureEvent>) {
    let mut contacts: Arc<Mutex<HashMap<i32, TouchState>>> = Arc::new(Mutex::new(HashMap::new()));
    let mut segments: Vec<(Point, Point)> = Vec::new();
    let mut timeouts: HashMap<i32, Sender<()>> = HashMap::new();
    let dpi = Device::current().dpi;
    while let Ok(evt) = rx.recv() {
        ty.send(GestureEvent::Relay(evt)).unwrap();
        match evt {
            DeviceEvent::Finger { status: FingerStatus::Down, position, id, .. } => {
                let mut ct = contacts.lock().unwrap();
                ct.insert(id, TouchState { initial: position, current: position });
                let (tz, rz) = mpsc::channel();
                let ty = ty.clone();
                let contacts = contacts.clone();
                thread::spawn(move || {
                    for i in 0..2 {
                        thread::sleep(Duration::from_millis(LONG_PRESS_DELAY_MS / 2));
                        if let Err(TryRecvError::Empty) = rz.try_recv() {
                            let ct = contacts.lock().unwrap();
                            if let Some(ts) = ct.get(&id) {
                                if (ts.current - position).length() / (dpi as f32) < mm_to_in(JITTER_TOLERANCE_MM) {
                                    ty.send(GestureEvent::Hold {
                                        long: i > 0,
                                        center: position,
                                    }).unwrap();
                                }
                            }
                        } else {
                            break;
                        }
                    }
                });
                timeouts.insert(id, tz);
            },
            DeviceEvent::Finger { status: FingerStatus::Motion, position, id, .. } => {
                let mut ct = contacts.lock().unwrap();
                if let Some(ref mut ts) = ct.get_mut(&id) {
                    ts.current = position;
                }
            },
            DeviceEvent::Finger { status: FingerStatus::Up, position, id, .. } => {
                let mut ct = contacts.lock().unwrap();
                if let Some(TouchState { initial: pt, .. }) = ct.remove(&id) {
                    segments.push((pt, position));
                }
                timeouts.remove(&id);
                if ct.is_empty() && !segments.is_empty() {
                    let len = segments.len();
                    if len == 1 {
                        let ge = interpret_segment(segments.pop().unwrap(), dpi);
                        ty.send(ge).unwrap();
                    } else if len == 2 {
                        let ge1 = interpret_segment(segments.pop().unwrap(), dpi);
                        let ge2 = interpret_segment(segments.pop().unwrap(), dpi);
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
                                let ds = s1.dist2(&s2);
                                let de = e1.dist2(&e2);
                                if ds > de {
                                    ty.send(GestureEvent::Pinch {
                                        axis: d1.axis(),
                                        target: (e1 + e2) / 2,
                                        strength: ds - de,
                                    }).unwrap();
                                } else {
                                    ty.send(GestureEvent::Spread {
                                        axis: d1.axis(),
                                        target: (s1 + s2) / 2,
                                        strength: de - ds,
                                    }).unwrap();
                                }
                            },
                            (GestureEvent::Swipe { start: s, end: e, .. }, GestureEvent::Tap { center: c, .. }) | 
                            (GestureEvent::Tap { center: c, .. }, GestureEvent::Swipe { start: s, end: e, .. }) => {
                                let angle = (s - c).angle() - (e - c).angle();
                                ty.send(GestureEvent::Rotate {
                                    angle: angle,
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
            _ => ()
        }
    }
}

fn interpret_segment((a, b): (Point, Point), dpi: u16) -> GestureEvent {
    let ab = b - a;
    let d = ab.length();
    if d / (dpi as f32) < mm_to_in(JITTER_TOLERANCE_MM) {
        GestureEvent::Tap {
            center: (a + b) / 2,
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
