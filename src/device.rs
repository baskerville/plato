extern crate libc;
use std::env;
use std::fmt;
use std::collections::HashMap;
use unit::scale_by_dpi;
use input::{DeviceEvent, TouchProto, InputEvent, raw_events, device_events, remarkable_parse_device_events, kobo_parse_device_events};
use gesture::gesture_events;
use view::Event;
use std::sync::mpsc::{self, Sender, Receiver};
use battery::{Battery, KoboBattery, RemarkableBattery};
use errors::*;

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum Model {
    AuraH2OEdition2,
    AuraEdition2,
    AuraONE,
    Touch2,
    GloHD,
    AuraH2O,
    Aura,
    AuraHD,
    Mini,
    Glo,
    Touch,
    Remarkable,
}

impl fmt::Display for Model {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Model::AuraH2OEdition2 => write!(f, "Aura H₂O Edition 2"),
            Model::AuraEdition2    => write!(f, "Aura Edition 2"),
            Model::AuraONE         => write!(f, "Aura ONE"),
            Model::Touch2          => write!(f, "Touch 2.0"),
            Model::GloHD           => write!(f, "Glo HD"),
            Model::AuraH2O         => write!(f, "Aura H₂O"),
            Model::Aura            => write!(f, "Aura"),
            Model::AuraHD          => write!(f, "Aura HD"),
            Model::Mini            => write!(f, "Mini"),
            Model::Glo             => write!(f, "Glo"),
            Model::Touch           => write!(f, "Touch"),
            Model::Remarkable      => write!(f, "Remarkable"),
        }
    }
}

#[derive(Debug)]
pub struct Device {
    pub model: Model,
    pub proto: TouchProto,
    pub mirrored_x: bool,
    pub dims: (u32, u32),
    pub dpi: u16,
}



impl Device {

    pub fn has_light(&self) -> bool {
        match self.model {
            Model::Remarkable => false,
            _ => true,
        }
    }

    pub fn has_natural_light(&self) -> bool {
        match self.model {
            Model::AuraONE | Model::AuraH2OEdition2 => true,
            _ => false,
        }
    }

    pub fn has_lightsensor(&self) -> bool {
        match self.model {
            Model::AuraONE => true,
            _ => false,
        }
    }

    pub fn create_battery(&self) -> Box<Battery> {
        match self.model {
            Model::Remarkable  => Box::new(RemarkableBattery::new().chain_err(|| "Can't create battery.").unwrap()) as Box<Battery>,
            _                  => Box::new(KoboBattery::new().chain_err(|| "Can't create battery.").unwrap()) as Box<Battery>,
        }
    }

    pub fn create_touchscreen(&self, screen_size: (u32, u32)) -> Receiver<Event> {
        return match self.model {
            Model::Remarkable => {
                let paths = vec!["/dev/input/event1".to_string(), //this is touchscreen
                                            "/dev/input/event2".to_string()]; //this is buttons
                let touch_screen = gesture_events(device_events(raw_events(paths), screen_size));
                touch_screen
            },
            _ => {
                let paths = vec!["/dev/input/event0".to_string(),
                                            "/dev/input/event1".to_string()];
                let touch_screen = gesture_events(device_events(raw_events(paths), screen_size));
                touch_screen
            }
        }
    }

    pub fn parse_device_events(&self, rx: &Receiver<InputEvent>, ty: &Sender<DeviceEvent>, dims: (u32, u32)) {
        match self.model {
            Model::Remarkable   => remarkable_parse_device_events(rx, ty, dims),
            _                   => kobo_parse_device_events(rx, ty, dims),
        }
    }

    pub fn suspend(&self) {
        match self.model {
            Model::Remarkable => {

            },
            _ => {

            },
        }
    }


}

lazy_static! {
    pub static ref CURRENT_DEVICE: Device = {
        Device {
                model: Model::Remarkable,
                proto: TouchProto::MultiB,
                mirrored_x: true,
                dims: (1404, 1872),
                dpi: 226,
            }
    };

// Tuples of the form
// ((HEIGHT, DPI), (SMALL_HEIGHT, BIG_HEIGHT))
// SMALL_HEIGHT and BIG_HEIGHT are choosen such that
// HEIGHT = 3 * SMALL_HEIGHT + k * BIG_HEIGHT where k > 3
// BIG_HEIGHT / SMALL_HEIGHT is as close as possible to 83/63
// SMALL_HEIGHT / DPI * 2.54 is as close as possible to 1 cm
pub static ref BAR_SIZES: HashMap<(u32, u16), (u32, u32)> =
    [
    ((1872, 226), (91, 123)),
    ].iter().cloned().collect();
}

pub fn optimal_bars_setup(height: u32, dpi: u16) -> (u32, u32) {
    let target_ratio = 83.0 / 63.0;
    let target_small_height = scale_by_dpi(126.0, dpi) as u32;
    let maximum_big_height = 2 * target_small_height;
    let minimum_small_height = 2 * target_small_height / 3;
    let mut max_score = 0;
    let mut result = (0, 0);
    for small_height in minimum_small_height..target_small_height+1 {
        let remaining_height = height - 3 * small_height;
        for big_height in small_height..maximum_big_height {
            if remaining_height % big_height == 0 {
                let ratio = big_height as f32 / small_height as f32;
                let drift = if ratio > target_ratio {
                    target_ratio / ratio
                } else {
                    ratio / target_ratio
                };
                let score = (small_height as f32 * drift) as u32;
                if score > max_score {
                    max_score = score;
                    result = (small_height, big_height);
                }
            }
        }
    }
    result
}

pub fn optimal_key_setup(width: u32, height: u32, dpi: u16) -> (u32, u32) {
    let target_side = scale_by_dpi(117.0, dpi) as u32;
    let target_padding = scale_by_dpi(6.0, dpi) as u32;
    let minimum_side = target_side / 3;
    let minimum_padding = 4 * target_padding / 5;
    let mut max_score = 0;
    let mut result = (0, 0);
    for side in minimum_side..target_side+1 {
        for padding in minimum_padding..target_padding+1 {
            let w = 11 * side + 12 * padding;
            let h = 4 * side + 5 * padding;
            if  w <= width && h <= height {
                let score = side + padding;
                if score > max_score {
                    result = (side, padding);
                    max_score = score;
                }
            }
        }
    }
    result
}
