use fnv::FnvHashMap;
use fnv::FnvHashSet;
use device::CURRENT_DEVICE;
use geom::Point;
use std::mem;
use input::*;
use std::sync::mpsc::{self, Sender, Receiver};


struct TouchState {
    position: Point,
    pressure: i32,
    status: FingerStatus,
    track_id: i32,
}

impl Default for TouchState {
    fn default() -> Self {
        TouchState {
            pressure: 0,
            position: Point::default(),
            status: FingerStatus::Down,
            track_id: 0,
        }
    }
}

static touchscreen_dims: (u32, u32) = (767, 1023);

pub fn remarkable_parse_device_events(rx: &Receiver<InputEvent>, ty: &Sender<DeviceEvent>, dims: (u32, u32)) {
    let (tc_width, tc_height) = touchscreen_dims;
    let (scr_width, scr_height) = dims;
    let mut slot = 0;
    let mut fingers: FnvHashMap<i32, TouchState> = FnvHashMap::default();

    let proto = CURRENT_DEVICE.proto;

    let mut tc = match proto {
        TouchProto::Single => SINGLE_TOUCH_CODES,
        TouchProto::MultiA => MULTI_TOUCH_CODES_A,
        TouchProto::MultiB => MULTI_TOUCH_CODES_B,
    };

    if CURRENT_DEVICE.touchscreen_x_y_swapped {
        mem::swap(&mut tc.x, &mut tc.y);
    }

    while let Ok(evt) = rx.recv() {
//        println!("{:.6} {} {} {}", seconds(evt.time), evt.kind, evt.code, evt.value);

        if evt.kind == EV_ABS {
            if evt.code == ABS_MT_SLOT {
//                println!("ABS_MT_SLOT {}", evt.value);

                slot = evt.value;
                if fingers.contains_key(&slot) {
//                    println!("Finger moves: {}", slot);
                    fingers.get_mut(&slot).unwrap().status = FingerStatus::Motion;
                } else {
                    fingers.insert(slot, TouchState::default());
//                    println!("Finger added: {}", slot);
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
                if slot == 0 {
                    fingers.insert(slot, TouchState::default());
                }
                if fingers.contains_key(&slot) {
                    fingers.get_mut(&slot).unwrap().track_id = evt.value;
                }
            } else {
//                println!("UNKNOWN EV_ABS CODE: {} {}", evt.code, evt.value);
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
                //once we reported down, for one finger, next reports should be up
                if ts.status == FingerStatus::Down {
                    ts.status = FingerStatus::Motion;
                }
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
