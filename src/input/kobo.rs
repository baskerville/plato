use fnv::FnvHashMap;
use fnv::FnvHashSet;
use device::CURRENT_DEVICE;
use geom::Point;
use std::mem;
use input::*;
use std::sync::mpsc::{self, Sender, Receiver};


pub fn kobo_parse_device_events(rx: &Receiver<InputEvent>, ty: &Sender<DeviceEvent>, dims: (u32, u32)) {
    let mut id = 0;
    let mut position = Point::default();
    let mut pressure = 0;
    let mut fingers: FnvHashMap<i32, Point> = FnvHashMap::default();
    let mut packet_ids: FnvHashSet<i32> = FnvHashSet::default();
    let proto = CURRENT_DEVICE.proto;

    let mut tc = match proto {
        TouchProto::Single => SINGLE_TOUCH_CODES,
        TouchProto::MultiA => MULTI_TOUCH_CODES_A,
        TouchProto::MultiB => MULTI_TOUCH_CODES_B,
    };

    mem::swap(&mut tc.x, &mut tc.y);

    while let Ok(evt) = rx.recv() {
        if evt.kind == EV_ABS {
            if evt.code == ABS_MT_TRACKING_ID {
                id = evt.value;
                if proto == TouchProto::MultiB {
                    packet_ids.insert(id);
                }
            } else if evt.code == tc.x {
                position.x = if CURRENT_DEVICE.mirrored_x {
                    dims.0 as i32 - 1 - evt.value
                } else {
                    evt.value
                };
            } else if evt.code == tc.y {
                position.y = evt.value;
            } else if evt.code == tc.pressure {
                pressure = evt.value;
            }
        } else if evt.kind == EV_SYN {
            if evt.code == SYN_MT_REPORT || (proto == TouchProto::Single && evt.code == SYN_REPORT) {
                if let Some(&p) = fingers.get(&id) {
                    if pressure > 0 {
                        if p != position {
                            ty.send(DeviceEvent::Finger {
                                id,
                                time: seconds(evt.time),
                                status: FingerStatus::Motion,
                                position,
                            }).unwrap();
                            fingers.insert(id, position);
                        }
                    } else {
                        ty.send(DeviceEvent::Finger {
                            id,
                            time: seconds(evt.time),
                            status: FingerStatus::Up,
                            position,
                        }).unwrap();
                        fingers.remove(&id);
                    }
                } else {
                    ty.send(DeviceEvent::Finger {
                        id,
                        time: seconds(evt.time),
                        status: FingerStatus::Down,
                        position,
                    }).unwrap();
                    fingers.insert(id, position);
                }
            } else if proto == TouchProto::MultiB && evt.code == SYN_REPORT {
                fingers.retain(|other_id, other_position| {
                    packet_ids.contains(other_id) ||
                        ty.send(DeviceEvent::Finger {
                            id: *other_id,
                            time: seconds(evt.time),
                            status: FingerStatus::Up,
                            position: *other_position,
                        }).is_err()
                });
                packet_ids.clear();
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
