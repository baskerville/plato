extern crate libc;

use std::mem;
use std::slice;
use std::thread;
use std::io::Read;
use std::fs::File;
use std::sync::mpsc::{self, Sender, Receiver};
use std::os::unix::io::AsRawFd;
use std::ffi::CString;
use fnv::FnvHashMap;
use device::CURRENT_DEVICE;
use geom::Point;
use errors::*;

// Event types
pub const EV_SYN: u16 = 0;
pub const EV_KEY: u16 = 1;
pub const EV_ABS: u16 = 3;
pub const SYN_REPORT: u16 = 0;

// Event codes
pub const ABS_MT_SLOT: u16 = 47;
pub const ABS_MT_TRACKING_ID: u16 = 57;
pub const ABS_MT_POSITION_X: u16 = 53;
pub const ABS_MT_POSITION_Y: u16 = 54;
pub const ABS_MT_PRESSURE: u16 = 58;
pub const ABS_MT_TOUCH_MAJOR: u16 = 48;
pub const ABS_MT_FINGER_COUNT: u16 = 52;
pub const SYN_MT_REPORT: u16 = 2;
pub const ABS_X: u16 = 0;
pub const ABS_Y: u16 = 1;
pub const ABS_PRESSURE: u16 = 24;

pub const KEY_POWER: u16 = 116;
pub const KEY_HOME: u16 = 102;
pub const KEY_LEFT: u16 = 105;
pub const KEY_RIGHT: u16 = 106;
pub const SLEEP_COVER: u16 = 59;

pub const SINGLE_TOUCH_CODES: TouchCodes = TouchCodes {
    pressure: ABS_PRESSURE,
    x: ABS_X,
    y: ABS_Y,
};

pub const MULTI_TOUCH_CODES_A: TouchCodes = TouchCodes {
    pressure: ABS_MT_TOUCH_MAJOR,
    x: ABS_MT_POSITION_X,
    y: ABS_MT_POSITION_Y,
};

pub const MULTI_TOUCH_CODES_B: TouchCodes = TouchCodes {
    pressure: ABS_MT_PRESSURE,
    .. MULTI_TOUCH_CODES_A
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
    pressure: u16,
    x: u16,
    y: u16,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum TouchProto {
    Single,
    MultiA,
    MultiB,
    MultiRemarkable,
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
    Left,
    Right,
    Raw(u16),
}

impl ButtonCode {
    fn from_raw(code: u16) -> ButtonCode {
        if code == KEY_POWER {
            ButtonCode::Power
        } else if code == KEY_HOME {
            ButtonCode::Home
        } else if code == KEY_LEFT {
            ButtonCode::Left
        } else if code == KEY_RIGHT {
            ButtonCode::Right
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
    NetUp,
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
            fd,
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
    let path = CString::new("/tmp/nickel-hardware-status").unwrap();
    let fd = unsafe { libc::open(path.as_ptr(), libc::O_NONBLOCK | libc::O_RDWR) };

    let mut pfd = libc::pollfd {
        fd,
        events: libc::POLLIN,
        revents: 0,
    };

    const BUF_LEN: usize = 256;

    loop {
        let ret = unsafe { libc::poll(&mut pfd as *mut libc::pollfd, 1, -1) };

        if ret < 0 {
            break;
        }

        let buf = CString::new(vec![1; BUF_LEN]).unwrap();
        let c_buf = buf.into_raw();

        if pfd.revents & libc::POLLIN != 0 {
            let n = unsafe { libc::read(fd, c_buf as *mut libc::c_void, BUF_LEN as libc::size_t) };
            let buf = unsafe { CString::from_raw(c_buf) };
            if n > 0 {
                if let Ok(s) = buf.to_str() {
                    for msg in s[..n as usize].lines() {
                        if msg == "usb plug add" {
                            tx.send(DeviceEvent::Plug).unwrap();
                        } else if msg == "usb plug remove" {
                            tx.send(DeviceEvent::Unplug).unwrap();
                        } else if msg.starts_with("network bound") {
                            tx.send(DeviceEvent::NetUp).unwrap();
                        }
                    }
                }
            } else {
                break;
            }
        }
    }
}

pub fn device_events(rx: Receiver<InputEvent>, dims: (u32, u32), touchscreen_dims: (u32, u32)) -> Receiver<DeviceEvent> {
    let (ty, ry) = mpsc::channel();
    thread::spawn(move || parse_device_events(&rx, &ty, dims, touchscreen_dims));
    ry
}

struct TouchState {
    position: Point,
    pressure: i32,
    status: FingerStatus,
}

impl Default for TouchState {
    fn default() -> Self {
        TouchState {
            pressure: 0,
            position: Point::default(),
            status: FingerStatus::Down,
        }
    }
}

pub fn parse_device_events(rx: &Receiver<InputEvent>, ty: &Sender<DeviceEvent>, dims: (u32, u32), touchscreen_dims: (u32, u32)) {
    let (tc_width, tc_height) = touchscreen_dims;
    let (scr_width, scr_height) = dims;
    let mut slot = 0;
    let mut fingers: FnvHashMap<i32, TouchState> = FnvHashMap::default();
    let proto = CURRENT_DEVICE.proto;

    let mut tc = match proto {
        TouchProto::Single => SINGLE_TOUCH_CODES,
        TouchProto::MultiA => MULTI_TOUCH_CODES_A,
        TouchProto::MultiB => MULTI_TOUCH_CODES_B,
        TouchProto::MultiRemarkable => MULTI_TOUCH_CODES_B,
    };

    if CURRENT_DEVICE.touchscreen_x_y_swapped {
        mem::swap(&mut tc.x, &mut tc.y);
    }

    while let Ok(evt) = rx.recv() {
//        println!("{:.6} {} {} {}", seconds(evt.time), evt.kind, evt.code, evt.value);

        if evt.kind == EV_ABS {
            if evt.code == ABS_MT_SLOT {
                slot = evt.value;
                if fingers.contains_key(&slot) {
//                    println!("Finger moves: {}", slot);
                    fingers.get_mut(&slot).unwrap().status = FingerStatus::Motion;
                } else {
                    fingers.insert(slot, TouchState::default());
                    println!("Finger added: {}", slot);
                }
            } else if evt.code == tc.x {
                if let Some(ts) = fingers.get_mut(&slot) {
                    let pos = if CURRENT_DEVICE.mirrored_x {
                        tc_width as i32 - 1 - evt.value
                    } else {
                        evt.value
                    };
                    ts.position.x = (pos as f32 / tc_width as f32 * scr_width as f32) as i32;
                }
            } else if evt.code == tc.y {
                if let Some(ts) = fingers.get_mut(&slot) {
                    let pos = if CURRENT_DEVICE.mirrored_y {
                        tc_height as i32 - 1 - evt.value
                    } else {
                        evt.value
                    };
                    ts.position.y = (pos as f32 / tc_height as f32 * scr_height as f32) as i32;
                }
            } else if evt.code == tc.pressure {
                if let Some(ts) = fingers.get_mut(&slot) {
                    ts.pressure = evt.value;
                }
            } else if evt.code == ABS_MT_FINGER_COUNT {
                //fixme do nothing
            } else if evt.code == ABS_MT_TRACKING_ID && evt.value == -1 {
//                println!("Finger up: {} (via TRACKING_ID)", slot);
                if let Some(ts) = fingers.get_mut(&slot) {
                    ts.status = FingerStatus::Up;
                }
            } else if evt.code == ABS_MT_TRACKING_ID {
                //we need to reset previous slot
                if fingers.contains_key(&slot) {
                    if fingers.get_mut(&slot).unwrap().status == FingerStatus::Down {
                        fingers.get_mut(&slot).unwrap().status = FingerStatus::Motion;
                    }
                }
                slot = 0;
                if fingers.contains_key(&slot) {
//                    println!("Finger moves: {}", slot);
                    fingers.get_mut(&slot).unwrap().status = FingerStatus::Motion;
                } else {
                    fingers.insert(slot, TouchState::default());
                    println!("Finger added: {} (Trackingid)", slot);
                }
            } else {
                println!("UNKNOWN EV_ABS CODE: {} {}", evt.code, evt.value);
            }
        } else if evt.kind == EV_SYN && evt.code == SYN_REPORT {
//            println!("Finger reporting: #{}", fingers.len());

            fingers.retain(|slot, ts| {

                ty.send(DeviceEvent::Finger {
                    id: *slot,
                    time: seconds(evt.time),
                    status: ts.status,
                    position: ts.position,
                }).unwrap();
                ts.status != FingerStatus::Up
            });
        } else if evt.kind == EV_KEY {
            if evt.code == SLEEP_COVER {
                if evt.value == 1 {
                    ty.send(DeviceEvent::CoverOn).unwrap();
                } else {
                    ty.send(DeviceEvent::CoverOff).unwrap();
                }
            } else {
//                println!("BUTTON PRESSED: CODE {}", evt.code);
                ty.send(DeviceEvent::Button {
                    time: seconds(evt.time),
                    code: ButtonCode::from_raw(evt.code),
                    status: if evt.value == 1 { ButtonStatus::Pressed } else
                                              { ButtonStatus::Released },
                }).unwrap();
            }
        } else {
            println!("UNKNOWN CODE: {} {} {}", evt.kind, evt.code, evt.value);
        }
    }
}
