use std::env;
use std::fmt;
use std::path::PathBuf;
use std::collections::HashMap;
use lazy_static::lazy_static;
use crate::input::TouchProto;

pub const INTERNAL_CARD_ROOT: &str = "/mnt/onboard";
pub const EXTERNAL_CARD_ROOT: &str = "/mnt/sd";

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum Model {
    LibraH2O,
    Forma32GB,
    Forma,
    ClaraHD,
    AuraH2OEd2V2,
    AuraH2OEd2V1,
    AuraEd2V2,
    AuraEd2V1,
    AuraONELimEd,
    AuraONE,
    Touch2,
    GloHD,
    AuraH2O,
    Aura,
    AuraHD,
    Mini,
    Glo,
    TouchC,
    TouchAB,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum Orientation {
    Portrait,
    Landscape,
}

impl fmt::Display for Model {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Model::LibraH2O      => write!(f, "Libra H₂O"),
            Model::Forma32GB     => write!(f, "Forma 32GB"),
            Model::Forma         => write!(f, "Forma"),
            Model::ClaraHD       => write!(f, "Clara HD"),
            Model::AuraH2OEd2V1  => write!(f, "Aura H₂O Edition 2 Version 1"),
            Model::AuraH2OEd2V2  => write!(f, "Aura H₂O Edition 2 Version 2"),
            Model::AuraEd2V1     => write!(f, "Aura Edition 2 Version 1"),
            Model::AuraEd2V2     => write!(f, "Aura Edition 2 Version 2"),
            Model::AuraONELimEd  => write!(f, "Aura ONE Limited Edition"),
            Model::AuraONE       => write!(f, "Aura ONE"),
            Model::Touch2        => write!(f, "Touch 2.0"),
            Model::GloHD         => write!(f, "Glo HD"),
            Model::AuraH2O       => write!(f, "Aura H₂O"),
            Model::Aura          => write!(f, "Aura"),
            Model::AuraHD        => write!(f, "Aura HD"),
            Model::Mini          => write!(f, "Mini"),
            Model::Glo           => write!(f, "Glo"),
            Model::TouchC        => write!(f, "Touch C"),
            Model::TouchAB       => write!(f, "Touch A/B"),
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

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum FrontlightKind {
    Standard,
    Natural,
    Premixed,
}

impl Device {
    pub fn new(product: &str, model_number: &str) -> Device {
        match product {
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
                model: if model_number == "381" { Model::AuraONELimEd } else { Model::AuraONE },
                proto: TouchProto::MultiA,
                dims: (1404, 1872),
                dpi: 300,
            },
            "star" => Device {
                model: if model_number == "379" { Model::AuraEd2V2 } else { Model::AuraEd2V1 },
                proto: TouchProto::MultiA,
                dims: (758, 1024),
                dpi: 212,
            },
            "snow" => Device {
                model: if model_number == "378" { Model::AuraH2OEd2V2 } else { Model::AuraH2OEd2V1 },
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
                model: if model_number == "380" { Model::Forma32GB } else { Model::Forma },
                proto: TouchProto::MultiB,
                dims: (1440, 1920),
                dpi: 300,
            },
            "storm" => Device {
                model: Model::LibraH2O,
                proto: TouchProto::MultiB,
                dims: (1264, 1680),
                dpi: 300,
            },
            _ => Device {
                model: if model_number == "320" { Model::TouchC } else { Model::TouchAB },
                proto: TouchProto::Single,
                dims: (600, 800),
                dpi: 167,
            },
        }
    }

    pub fn library_path(&self) -> PathBuf {
        match self.model {
            Model::AuraH2O |
            Model::Aura |
            Model::AuraHD |
            Model::Mini |
            Model::Glo |
            Model::TouchAB |
            Model::TouchC => PathBuf::from(EXTERNAL_CARD_ROOT),
            _ => PathBuf::from(INTERNAL_CARD_ROOT),
        }
    }

    pub fn frontlight_kind(&self) -> FrontlightKind {
        match self.model {
            Model::AuraONE |
            Model::AuraONELimEd |
            Model::AuraH2OEd2V1 |
            Model::AuraH2OEd2V2 => FrontlightKind::Natural,
            Model::ClaraHD |
            Model::Forma |
            Model::Forma32GB |
            Model::LibraH2O => FrontlightKind::Premixed,
            _ => FrontlightKind::Standard,
        }
    }

    pub fn has_natural_light(&self) -> bool {
        match self.frontlight_kind() {
            FrontlightKind::Standard => false,
            _ => true,
        }
    }

    pub fn has_lightsensor(&self) -> bool {
        match self.model {
            Model::AuraONE |
            Model::AuraONELimEd => true,
            _ => false,
        }
    }

    pub fn has_gyroscope(&self) -> bool {
        match self.model {
            Model::Forma | Model::Forma32GB | Model::LibraH2O => true,
            _ => false,
        }
    }

    pub fn has_page_turn_buttons(&self) -> bool {
        match self.model {
            Model::Forma | Model::Forma32GB | Model::LibraH2O => true,
            _ => false,
        }
    }

    pub fn should_invert_buttons(&self, rotation: i8) -> bool {
        let sr = self.startup_rotation();
        let (_, dir) = self.mirroring_scheme();

        rotation == (4 + sr - dir) % 4 || rotation == (4 + sr - 2 * dir) % 4
    }

    pub fn orientation(&self, rotation: i8) -> Orientation {
        let discriminant = match self.model {
            Model::LibraH2O => 0,
            _ => 1,
        };
        if rotation % 2 == discriminant {
            Orientation::Portrait
        } else {
            Orientation::Landscape
        }
    }

    pub fn mark(&self) -> u8 {
        match self.model {
            Model::LibraH2O |
            Model::Forma32GB |
            Model::Forma |
            Model::ClaraHD |
            Model::AuraH2OEd2V2 |
            Model::AuraEd2V2 => 7,
            Model::AuraH2OEd2V1 |
            Model::AuraEd2V1 |
            Model::AuraONELimEd |
            Model::AuraONE |
            Model::Touch2 |
            Model::GloHD => 6,
            Model::AuraH2O |
            Model::Aura => 5,
            Model::AuraHD |
            Model::Mini |
            Model::Glo |
            Model::TouchC => 4,
            Model::TouchAB => 3,
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
        match self.model {
            Model::AuraH2OEd2V1 => (3, 1),
            Model::AuraH2OEd2V2 => (0, -1),
            Model::Forma | Model::Forma32GB => (2, -1),
            Model::LibraH2O => (3, 1),
            _ => (2, 1),
        }
    }

    pub fn should_swap_axes(&self, rotation: i8) -> bool {
        rotation % 2 == self.swapping_scheme()
    }

    pub fn swapping_scheme(&self) -> i8 {
        match self.model {
            Model::LibraH2O => 0,
            _ => 1,
        }
    }

    pub fn startup_rotation(&self) -> i8 {
        match self.model {
            Model::LibraH2O => 0,
            Model::AuraH2OEd2V1 => 1,
            Model::Forma | Model::Forma32GB => 1,
            _ => 3,
        }
    }

    pub fn transformed_rotation(&self, n: i8) -> i8 {
        match self.model {
            Model::AuraHD | Model::AuraH2O => n ^ 2,
            Model::AuraH2OEd2V2 => (4 - n) % 4,
            Model::Forma | Model::Forma32GB => (4 - n) % 4,
            _ => n,
        }
    }

    pub fn transformed_gyroscope_rotation(&self, n: i8) -> i8 {
        match self.model {
            Model::LibraH2O => n ^ 1,
            _ => n,
        }
    }
}

lazy_static! {
    pub static ref CURRENT_DEVICE: Device = {
        let product = env::var("PRODUCT").unwrap_or_default();
        let model_number = env::var("MODEL_NUMBER").unwrap_or_default();

        Device::new(&product, &model_number)
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
         ((1264, 300), (123, 179)),
         ((1680, 300), (120, 165)),
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

#[cfg(test)]
mod tests {
    use std::env;
    use std::path::PathBuf;
    use crate::unit::scale_by_dpi;
    use super::{Device, Model, FrontlightKind, Orientation};
    use super::{CURRENT_DEVICE, EXTERNAL_CARD_ROOT, INTERNAL_CARD_ROOT};

    #[test]
    fn test_global_static_current_device() {
        env::set_var("PRODUCT", "frost");
        env::set_var("MODEL_NUMBER", "380");

        assert_eq!(CURRENT_DEVICE.model, Model::Forma32GB);
    }

    #[test]
    fn test_device_library_path() {
        let device = Device::new("frost", "380");
        assert_eq!(device.library_path(), PathBuf::from(INTERNAL_CARD_ROOT));

        let device = Device::new("kraken", "380");
        assert_eq!(device.library_path(), PathBuf::from(EXTERNAL_CARD_ROOT));
    }

    #[test]
    fn test_device_frontlight_kind() {
        let device = Device::new("frost", "380");
        assert_eq!(device.frontlight_kind(), FrontlightKind::Premixed);

        let device = Device::new("snow", "378");
        assert_eq!(device.frontlight_kind(), FrontlightKind::Natural);
    }

    #[test]
    fn test_device_has_natural_light() {
        let device = Device::new("frost", "380");
        assert_eq!(device.has_natural_light(), true);

        let device = Device::new("pika", "378");
        assert_eq!(device.has_natural_light(), false);
    }

    #[test]
    fn test_device_has_light_sensor() {
        let device = Device::new("daylight", "380");
        assert_eq!(device.has_lightsensor(), true);

        let device = Device::new("pika", "378");
        assert_eq!(device.has_lightsensor(), false);
    }

    #[test]
    fn test_device_has_gyroscope() {
        let device = Device::new("frost", "380");
        assert_eq!(device.has_gyroscope(), true);

        let device = Device::new("pika", "378");
        assert_eq!(device.has_gyroscope(), false);
    }

    #[test]
    fn test_device_has_page_turn_buttons() {
        let device = Device::new("frost", "380");
        assert_eq!(device.has_page_turn_buttons(), true);

        let device = Device::new("pika", "378");
        assert_eq!(device.has_page_turn_buttons(), false);
    }

    #[test]
    fn test_device_should_invert_buttons() {
        let device = Device::new("frost", "380");
        assert_eq!(device.should_invert_buttons(0), false);
        assert_eq!(device.should_invert_buttons(1), false);
        assert_eq!(device.should_invert_buttons(2), true);
        assert_eq!(device.should_invert_buttons(3), true);

        let device = Device::new("pika", "378");
        assert_eq!(device.should_invert_buttons(0), false);
        assert_eq!(device.should_invert_buttons(1), true);
        assert_eq!(device.should_invert_buttons(2), true);
        assert_eq!(device.should_invert_buttons(3), false);
    }

    #[test]
    fn test_device_orientation() {
        let device = Device::new("frost", "380");
        assert_eq!(device.orientation(0), Orientation::Landscape);
        assert_eq!(device.orientation(1), Orientation::Portrait);

        let device = Device::new("storm", "378");
        assert_eq!(device.orientation(0), Orientation::Portrait);
        assert_eq!(device.orientation(1), Orientation::Landscape);
    }

    #[test]
    fn test_device_mark() {
        let device = Device::new("frost", "380");
        assert_eq!(device.mark(), 7);

        let device = Device::new("pika", "378");
        assert_eq!(device.mark(), 6);
    }

    #[test]
    fn test_device_mirroring_scheme() {
        let device = Device::new("frost", "380");
        assert_eq!(device.mirroring_scheme(), (2, -1));

        let device = Device::new("pika", "378");
        assert_eq!(device.mirroring_scheme(), (2, 1));
    }

    #[test]
    fn test_device_should_mirror_axes() {
        let device = Device::new("frost", "380");
        assert_eq!(device.should_mirror_axes(0), (false, false));
        assert_eq!(device.should_mirror_axes(1), (true, false));
        assert_eq!(device.should_mirror_axes(2), (true, true));
        assert_eq!(device.should_mirror_axes(3), (false, true));

        let device = Device::new("pika", "378");
        assert_eq!(device.should_mirror_axes(0), (false, false));
        assert_eq!(device.should_mirror_axes(1), (false, true));
        assert_eq!(device.should_mirror_axes(2), (true, true));
        assert_eq!(device.should_mirror_axes(3), (true, false));
    }

    #[test]
    fn test_device_swapping_scheme() {
        let device = Device::new("storm", "378");
        assert_eq!(device.swapping_scheme(), 0);

        let device = Device::new("frost", "380");
        assert_eq!(device.swapping_scheme(), 1);
    }

    #[test]
    fn test_device_should_swap_axes() {
        let device = Device::new("storm", "378");
        assert_eq!(device.should_swap_axes(0), true);
        assert_eq!(device.should_swap_axes(1), false);
        assert_eq!(device.should_swap_axes(2), true);
        assert_eq!(device.should_swap_axes(3), false);

        let device = Device::new("frost", "380");
        assert_eq!(device.should_swap_axes(0), false);
        assert_eq!(device.should_swap_axes(1), true);
        assert_eq!(device.should_swap_axes(2), false);
        assert_eq!(device.should_swap_axes(3), true);
    }

    #[test]
    fn test_device_startup_rotation() {
        let device = Device::new("frost", "380");
        assert_eq!(device.startup_rotation(), 1);

        let device = Device::new("pika", "378");
        assert_eq!(device.startup_rotation(), 3);
    }

    #[test]
    fn test_device_transformed_rotation() {
        let device = Device::new("frost", "380");
        assert_eq!(device.transformed_rotation(0), 0);
        assert_eq!(device.transformed_rotation(1), 3);
        assert_eq!(device.transformed_rotation(2), 2);
        assert_eq!(device.transformed_rotation(3), 1);

        let device = Device::new("pika", "378");
        assert_eq!(device.transformed_rotation(0), 0);
        assert_eq!(device.transformed_rotation(1), 1);
        assert_eq!(device.transformed_rotation(2), 2);
        assert_eq!(device.transformed_rotation(3), 3);
    }

    #[test]
    fn bar_sizes() {
        assert_eq!(optimal_bars_setup(1872, 300), (126, 166));
        assert_eq!(optimal_bars_setup(1448, 300), (121, 155));
    }

    fn optimal_bars_setup(height: u32, dpi: u16) -> (u32, u32) {
        let target_ratio = 83.0 / 63.0;
        let target_small_height = scale_by_dpi(126.0, dpi) as u32;
        let maximum_big_height = 2 * target_small_height;
        let minimum_small_height = 2 * target_small_height / 3;
        let mut max_score = 0;
        let mut result = (0, 0);
        for small_height in minimum_small_height..=target_small_height {
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
}
