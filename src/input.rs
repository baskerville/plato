extern crate libc;

use std::sync::mpsc::{self, Sender, Receiver};
use std::os::unix::io::AsRawFd;
use std::thread;
use std::io::Read;
use std::fs::File;
use std::slice;
use std::mem;
use fnv::FnvHashMap;
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
pub const SLEEP_COVER: u16 = 59;

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

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum TouchProto {
    Single,
    Multi,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum FingerStatus {
    Down,
    Motion,
    Up,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum ButtonStatus {
    Pressed,
    Released,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
pub enum ButtonCode {
    Power,
    Home,
    Raw(u16),
}

impl ButtonCode {
    fn from_raw(code: u16) -> ButtonCode {
        if code == KEY_POWER {
            ButtonCode::Power
        } else if code == KEY_HOME {
            ButtonCode::Home
        } else {
            ButtonCode::Raw(code)
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
    Plug,
    Unplug,
    CoverOn,
    CoverOff,
}

pub fn seconds(time: libc::timeval) -> f64 {
    time.tv_sec as f64 + time.tv_usec as f64 / 1e6
}

pub fn raw_events(paths: Vec<String>) -> Receiver<InputEvent> {
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || parse_raw_events(&paths, &tx));
    rx
}

pub fn parse_raw_events(paths: &[String], tx: &Sender<InputEvent>) -> Result<()> {
    let mut files = Vec::new();
    let mut pfds = Vec::new();

    for path in paths.iter() {
        let file = File::open(path).chain_err(|| "Can't open input file.")?;
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

pub fn usb_events() -> Receiver<DeviceEvent> {
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || parse_usb_events(&tx));
    rx
}

fn parse_usb_events(tx: &Sender<DeviceEvent>) {
    let mut file = File::open("/tmp/nickel-hardware-status").unwrap();
    let fd = file.as_raw_fd();

    let mut pfds = [libc::pollfd { fd: fd, events: libc::POLLIN, revents: 0 }];

    loop {
        let ret = unsafe { libc::poll(pfds.as_mut_ptr(), pfds.len() as libc::nfds_t, -1) };

        if ret < 0 {
            break;
        }

        for pfd in pfds.iter() {
            if pfd.revents & libc::POLLIN != 0 {
                let mut buf = String::new();
                if file.read_to_string(&mut buf).is_err() {
                    break;
                }

                let msg = buf.trim_right();
                if msg == "usb plug add" {
                    tx.send(DeviceEvent::Plug).unwrap();
                } else if msg == "usb plug remove" {
                    tx.send(DeviceEvent::Unplug).unwrap();
                }
            }
        }
    }
}

pub fn device_events(rx: Receiver<InputEvent>, dims: (u32, u32)) -> Receiver<DeviceEvent> {
    let (ty, ry) = mpsc::channel();
    thread::spawn(move || parse_device_events(&rx, &ty, dims));
    ry
}

pub fn parse_device_events(rx: &Receiver<InputEvent>, ty: &Sender<DeviceEvent>, dims: (u32, u32)) {
    let mut id = 0;
    let mut position = Point::default();
    let mut pressure = 0;
    let mut fingers: FnvHashMap<i32, Point> = FnvHashMap::default();
    let mut tc = if CURRENT_DEVICE.proto == TouchProto::Multi { MULTI_TOUCH_CODES } else { SINGLE_TOUCH_CODES };
    mem::swap(&mut tc.x, &mut tc.y);
    while let Ok(evt) = rx.recv() {
        if evt.kind == EV_ABS {
            if evt.code == tc.pressure {
                pressure = evt.value;
            } else if evt.code == tc.x {
                position.x = dims.0 as i32 - 1 - evt.value;
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
            if evt.code == SLEEP_COVER {
                if evt.value == 1 {
                    ty.send(DeviceEvent::CoverOn).unwrap();
                } else {
                    ty.send(DeviceEvent::CoverOff).unwrap();
                }
            } else {
                ty.send(DeviceEvent::Button {
                    time: seconds(evt.time),
                    code: ButtonCode::from_raw(evt.code),
                    status: if evt.value == 1 { ButtonStatus::Pressed } else
                                              { ButtonStatus::Released },
                }).unwrap();
            }
        }
    }
}
