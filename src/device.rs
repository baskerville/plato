extern crate libc;

use std::env;
use std::fmt;
use std::collections::HashMap;
use unit::scale_by_dpi;
use input::TouchProto;

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum Model {
    Forma,
    ClaraHD,
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
}

impl fmt::Display for Model {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Model::Forma           => write!(f, "Forma"),
            Model::ClaraHD         => write!(f, "Clara HD"),
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
        }
    }
}

#[derive(Debug)]
pub struct Device {
    pub model: Model,
    pub proto: TouchProto,
    pub dims: (u32, u32),
    pub dpi: u16,
}

impl Default for Device {
    fn default() -> Device {
        Device {
            model: Model::Touch,
            proto: TouchProto::Single,
            dims: (600, 800),
            dpi: 167,
        }
    }
}

impl Device {
    pub fn has_natural_light(&self) -> bool {
        match self.model {
            Model::AuraONE |
            Model::AuraH2OEdition2 |
            Model::ClaraHD |
            Model::Forma => true,
            _ => false,
        }
    }

    pub fn has_lightsensor(&self) -> bool {
        match self.model {
            Model::AuraONE => true,
            _ => false,
        }
    }

    pub fn should_mirror_axes(&self, rotation: i8) -> (bool, bool) {
        let (mxy, dir) = self.mirroring_scheme();
        let mx = (4 + (mxy + dir)) % 4;
        let my = (4 + (mxy - dir)) % 4;
        let mirror_x = mxy == rotation || mx == rotation;
        let mirror_y = mxy == rotation || my == rotation;
        (mirror_x, mirror_y)
    }

    // Returns the center and direction of the mirroring pattern.
    pub fn mirroring_scheme(&self) -> (i8, i8) {
        let model_number = env::var("MODEL_NUMBER").unwrap_or_default();
        match self.model {
            Model::AuraH2OEdition2 if model_number == "374" => (1, 1),
            Model::AuraH2OEdition2 if model_number == "378" => (0, -1),
            _ => (2, 1),
        }
    }

    pub fn startup_rotation(&self) -> i8 {
        3
    }

    pub fn transformed_rotation(&self, n: i8) -> i8 {
        let model_number = env::var("MODEL_NUMBER").unwrap_or_default();
        match self.model {
            Model::AuraHD | Model::AuraH2O => n ^ 2,
            Model::AuraH2OEdition2 if model_number == "374" => n ^ 2,
            Model::AuraH2OEdition2 if model_number == "378" => (4 - n) % 4,
            Model::Forma => (4 - n) % 4,
            _ => n,
        }
    }
}

lazy_static! {
    pub static ref CURRENT_DEVICE: Device = {
        let product = env::var("PRODUCT").unwrap_or_default();

        match product.as_ref() {
            "kraken" => Device {
                model: Model::Glo,
                proto: TouchProto::Single,
                dims: (758, 1024),
                dpi: 212,
            },
            "pixie" => Device {
                model: Model::Mini,
                proto: TouchProto::Single,
                dims: (600, 800),
                dpi: 200,
            },
            "dragon" => Device {
                model: Model::AuraHD,
                proto: TouchProto::Single,
                dims: (1080, 1440),
                dpi: 265,
            },
            "phoenix" => Device {
                model: Model::Aura,
                proto: TouchProto::MultiA,
                dims: (758, 1024),
                dpi: 212,
            },
            "dahlia" => Device {
                model: Model::AuraH2O,
                proto: TouchProto::MultiA,
                dims: (1080, 1440),
                dpi: 265,
            },
            "alyssum" => Device {
                model: Model::GloHD,
                proto: TouchProto::MultiA,
                dims: (1072, 1448),
                dpi: 300,
            },
            "pika" => Device {
                model: Model::Touch2,
                proto: TouchProto::MultiA,
                dims: (600, 800),
                dpi: 167,
            },
            "daylight" => Device {
                model: Model::AuraONE,
                proto: TouchProto::MultiA,
                dims: (1404, 1872),
                dpi: 300,
            },
            "star" => Device {
                model: Model::AuraEdition2,
                proto: TouchProto::MultiA,
                dims: (758, 1024),
                dpi: 212,
            },
            "snow" => Device {
                model: Model::AuraH2OEdition2,
                proto: TouchProto::MultiB,
                dims: (1080, 1440),
                dpi: 265,
            },
            "nova" => Device {
                model: Model::ClaraHD,
                proto: TouchProto::MultiB,
                dims: (1072, 1448),
                dpi: 300,
            },
            "frost" => Device {
                model: Model::Forma,
                proto: TouchProto::MultiB,
                dims: (1440, 1920),
                dpi: 300,
            },
            _ => Device::default(),
        }
    };

// Tuples of the form
// ((HEIGHT, DPI), (SMALL_HEIGHT, BIG_HEIGHT))
// SMALL_HEIGHT and BIG_HEIGHT are choosen such that
// HEIGHT = 3 * SMALL_HEIGHT + k * BIG_HEIGHT where k > 3
// BIG_HEIGHT / SMALL_HEIGHT is as close as possible to 83/63
// SMALL_HEIGHT / DPI * 2.54 is as close as possible to 1 cm
pub static ref BAR_SIZES: HashMap<(u32, u16), (u32, u32)> =
    [((1920, 300), (120, 156)),
     ((1440, 300), (126, 177)),
     ((1872, 300), (126, 166)),
     ((1404, 300), (126, 171)),
     ((1448, 300), (121, 155)),
     ((1072, 300), (124, 175)),
     ((1440, 265), (104, 141)),
     ((1080, 265), (110, 150)),
     ((1024, 212), ( 87, 109)),
     (( 758, 212), ( 81, 103)),
     (( 800, 167), ( 66,  86)),
     (( 600, 167), ( 65,  81)),
     (( 800, 200), ( 80, 112)),
     (( 600, 200), ( 84, 116))].iter().cloned().collect();
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
