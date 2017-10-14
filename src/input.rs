extern crate libc;

use std::sync::mpsc::{self, Sender, Receiver};
use std::os::unix::io::AsRawFd;
use std::collections::HashMap;
use std::thread;
use std::io::Read;
use std::fs::File;
use std::slice;
use std::mem;
use std::env;
use device::CURRENT_DEVICE;
use geom::Point;
use errors::*;

// Event types
pub const EV_SYN: u16 = 0;
pub const EV_KEY: u16 = 1;
pub const EV_ABS: u16 = 3;

// Event codes
pub const SYN_MT_REPORT: u16 = 2;
pub const SYN_REPORT: u16 = 0;
pub const ABS_MT_TRACKING_ID: u16 = 57;
pub const ABS_MT_TOUCH_MAJOR: u16 = 48;
pub const ABS_PRESSURE: u16 = 24;
pub const ABS_MT_POSITION_X: u16 = 53;
pub const ABS_MT_POSITION_Y: u16 = 54;
pub const ABS_X: u16 = 0;
pub const ABS_Y: u16 = 1;
pub const KEY_POWER: u16 = 116;
pub const KEY_HOME: u16 = 102;

pub const SINGLE_TOUCH_CODES: TouchCodes = TouchCodes {
    report: SYN_REPORT,
    pressure: ABS_PRESSURE,
    x: ABS_X,
    y: ABS_Y,
};

pub const MULTI_TOUCH_CODES: TouchCodes = TouchCodes {
    report: SYN_MT_REPORT,
    pressure: ABS_MT_TOUCH_MAJOR,
    x: ABS_MT_POSITION_X,
    y: ABS_MT_POSITION_Y,
};

#[repr(C)]
pub struct InputEvent {
    pub time: libc::timeval,
    pub kind: u16, // type
    pub code: u16,
    pub value: i32,
}

// Handle different touch protocols
#[derive(Debug)]
pub struct TouchCodes {
    report: u16,
    pressure: u16,
    x: u16,
    y: u16,
}

#[derive(Debug, Eq, PartialEq)]
pub enum TouchProto {
    Single,
    Multi,
}

#[derive(Debug, Copy, Clone)]
pub enum FingerStatus {
    Down,
    Motion,
    Up,
}

#[derive(Debug, Copy, Clone)]
pub enum ButtonStatus {
    Pressed,
    Released,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
pub enum ButtonCode {
    Power,
    Home,
    Unknown,
}

impl ButtonCode {
    fn from_raw(code: u16) -> ButtonCode {
        if code == KEY_POWER {
            ButtonCode::Power
        } else if code == KEY_HOME {
            ButtonCode::Home
        } else {
            ButtonCode::Unknown
        }
    }
}

#[derive(Debug, Copy, Clone)]
pub enum DeviceEvent {
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
}

pub fn seconds(time: libc::timeval) -> f64 {
    time.tv_sec as f64 + time.tv_usec as f64 / 1e6
}

pub struct Input {
    pub events: Receiver<DeviceEvent>,
    pub dims: (u32, u32),
}

impl Input {
    pub fn new(paths: Vec<String>, dims: (u32, u32)) -> Input {
        let events = device_events(raw_events(paths), dims);
        Input {
            events: events,
            dims: dims,
        }
    }
}

pub fn raw_events(paths: Vec<String>) -> Receiver<InputEvent> {
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || parse_raw_events(paths, tx));
    rx
}

pub fn parse_raw_events(paths: Vec<String>, tx: Sender<InputEvent>) -> Result<()> {
    let mut files = Vec::new();
    let mut pfds = Vec::new();
    for path in paths.iter() {
        let file = File::open(path).chain_err(|| "can't open input file")?;
        let fd = file.as_raw_fd();
        files.push(file);
        pfds.push(libc::pollfd {
            fd: fd,
            events: libc::POLLIN,
            revents: 0,
        });
    }
    loop {
        let ret = unsafe { libc::poll(pfds.as_mut_ptr(), pfds.len() as libc::nfds_t, -1) };
        if ret < 0 {
            break;
        }
        for (pfd, mut file) in pfds.iter().zip(&files) {
            if pfd.revents & libc::POLLIN != 0 {
                let mut input_event: InputEvent = unsafe { mem::uninitialized() };
                unsafe {
                    let event_slice = slice::from_raw_parts_mut(&mut input_event as *mut InputEvent as *mut u8,
                                                                mem::size_of::<InputEvent>());
                    if file.read_exact(event_slice).is_err() {
                        break;
                    }
                }
                tx.send(input_event).unwrap();
            }
        }
    }
    Ok(())
}

pub fn device_events(rx: Receiver<InputEvent>, dims: (u32, u32)) -> Receiver<DeviceEvent> {
    let (ty, ry) = mpsc::channel();
    thread::spawn(move || parse_device_events(rx, ty, dims));
    ry
}

pub fn parse_device_events(rx: Receiver<InputEvent>, ty: Sender<DeviceEvent>, dims: (u32, u32)) {
    let mut id = 0;
    let mut position = Point::default();
    let mut pressure = 0;
    let mut fingers: HashMap<i32, Point> = HashMap::new();
    let mut tc = if CURRENT_DEVICE.proto == TouchProto::Multi { MULTI_TOUCH_CODES } else { SINGLE_TOUCH_CODES };
    // Current hypothesis: width > height implies UNSWAP_XY and UNMIRROR_X
    if env::var("PLATO_UNSWAP_XY").is_err() {
        mem::swap(&mut tc.x, &mut tc.y);
    }
    let mirror_x = env::var("PLATO_UNMIRROR_X").is_err();
    while let Ok(evt) = rx.recv() {
        if evt.kind == EV_ABS {
            if evt.code == tc.pressure {
                pressure = evt.value;
            } else if evt.code == tc.x {
                position.x = if mirror_x { dims.0 as i32 - 1 - evt.value } else { evt.value };
            } else if evt.code == tc.y {
                position.y = evt.value;
            } else if evt.code == ABS_MT_TRACKING_ID {
                id = evt.value;
            }
        } else if evt.kind == EV_SYN {
            if evt.code == tc.report {
                if let Some(&p) = fingers.get(&id) {
                    if pressure > 0 {
                        if p != position {
                            ty.send(DeviceEvent::Finger {
                                id: id,
                                time: seconds(evt.time),
                                status: FingerStatus::Motion,
                                position: position,
                            }).unwrap();
                        }
                    } else {
                        ty.send(DeviceEvent::Finger {
                            id: id,
                            time: seconds(evt.time),
                            status: FingerStatus::Up,
                            position: position,
                        }).unwrap();
                        fingers.remove(&id);
                    }
                } else {
                    ty.send(DeviceEvent::Finger {
                        id: id,
                        time: seconds(evt.time),
                        status: FingerStatus::Down,
                        position: position,
                    }).unwrap();
                    fingers.insert(id, position);
                }
            }
        } else if evt.kind == EV_KEY {
            ty.send(DeviceEvent::Button {
                time: seconds(evt.time),
                code: ButtonCode::from_raw(evt.code),
                status: if evt.value == 1 { ButtonStatus::Pressed } else
                                          { ButtonStatus::Released },
            }).unwrap();
        }
    }
}
