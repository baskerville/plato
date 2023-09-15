use std::env;
use std::fmt;
use lazy_static::lazy_static;
use crate::input::TouchProto;

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum Model {
    Elipsa2E,
    Clara2E,
    Libra2,
    Sage,
    Elipsa,
    Nia,
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
            Model::Elipsa2E      => write!(f, "Elipsa 2E"),
            Model::Clara2E       => write!(f, "Clara 2E"),
            Model::Libra2        => write!(f, "Libra 2"),
            Model::Sage          => write!(f, "Sage"),
            Model::Elipsa        => write!(f, "Elipsa"),
            Model::Nia           => write!(f, "Nia"),
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
            "luna" => Device {
                model: Model::Nia,
                proto: TouchProto::MultiA,
                dims: (758, 1024),
                dpi: 212,
            },
            "europa" => Device {
                model: Model::Elipsa,
                proto: TouchProto::MultiC,
                dims: (1404, 1872),
                dpi: 227,
            },
            "cadmus" => Device {
                model: Model::Sage,
                proto: TouchProto::MultiC,
                dims: (1440, 1920),
                dpi: 300,
            },
            "io" => Device {
                model: Model::Libra2,
                proto: TouchProto::MultiC,
                dims: (1264, 1680),
                dpi: 300,
            },
            "goldfinch" => Device {
                model: Model::Clara2E,
                proto: TouchProto::MultiB,
                dims: (1072, 1448),
                dpi: 300,
            },
            "condor" => Device {
                model: Model::Elipsa2E,
                proto: TouchProto::MultiC,
                dims: (1404, 1872),
                dpi: 227,
            },
            _ => Device {
                model: if model_number == "320" { Model::TouchC } else { Model::TouchAB },
                proto: TouchProto::Single,
                dims: (600, 800),
                dpi: 167,
            },
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
            Model::LibraH2O |
            Model::Sage |
            Model::Libra2 |
            Model::Clara2E |
            Model::Elipsa2E => FrontlightKind::Premixed,
            _ => FrontlightKind::Standard,
        }
    }

    pub fn has_natural_light(&self) -> bool {
        self.frontlight_kind() != FrontlightKind::Standard
    }

    pub fn has_lightsensor(&self) -> bool {
        matches!(self.model,
                 Model::AuraONE | Model::AuraONELimEd)
    }

    pub fn has_gyroscope(&self) -> bool {
        matches!(self.model,
                 Model::Forma | Model::Forma32GB | Model::LibraH2O |
                 Model::Elipsa | Model::Sage | Model::Libra2 | Model::Elipsa2E)
    }

    pub fn has_page_turn_buttons(&self) -> bool {
        matches!(self.model,
                 Model::Forma | Model::Forma32GB | Model::LibraH2O |
                 Model::Sage | Model::Libra2)
    }

    pub fn has_power_cover(&self) -> bool {
        matches!(self.model, Model::Sage)
    }

    pub fn has_removable_storage(&self) -> bool {
        matches!(self.model,
                 Model::AuraH2O | Model::Aura | Model::AuraHD |
                 Model::Glo | Model::TouchAB | Model::TouchC)
    }

    pub fn should_invert_buttons(&self, rotation: i8) -> bool {
        let sr = self.startup_rotation();
        let (_, dir) = self.mirroring_scheme();

        rotation == (4 + sr - dir) % 4 || rotation == (4 + sr - 2 * dir) % 4
    }

    pub fn orientation(&self, rotation: i8) -> Orientation {
        if self.should_swap_axes(rotation) {
            Orientation::Portrait
        } else {
            Orientation::Landscape
        }
    }

    pub fn mark(&self) -> u8 {
        match self.model {
            Model::Elipsa2E => 11,
            Model::Clara2E => 10,
            Model::Libra2 => 9,
            Model::Sage |
            Model::Elipsa => 8,
            Model::Nia |
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
            Model::AuraH2OEd2V1 |
            Model::LibraH2O |
            Model::Libra2 => (3, 1),
            Model::Sage => (0, 1),
            Model::AuraH2OEd2V2 => (0, -1),
            Model::Forma | Model::Forma32GB => (2, -1),
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

    // The written rotation that makes the screen be in portrait mode
    // with the Kobo logo at the bottom.
    pub fn startup_rotation(&self) -> i8 {
        match self.model {
            Model::LibraH2O => 0,
            Model::AuraH2OEd2V1 |
            Model::Forma | Model::Forma32GB |
            Model::Sage | Model::Libra2 | Model::Elipsa2E => 1,
            _ => 3,
        }
    }

    // Return a device independent rotation value given
    // the device dependent written rotation value *n*.
    pub fn to_canonical(&self, n: i8) -> i8 {
        let (_, dir) = self.mirroring_scheme();
        (4 + dir * (n - self.startup_rotation())) % 4
    }

    // Return a device dependent written rotation value given
    // the device independent rotation value *n*.
    pub fn from_canonical(&self, n: i8) -> i8 {
        let (_, dir) = self.mirroring_scheme();
        (self.startup_rotation() + (4 + dir * n) % 4) % 4
    }

    // Return a device dependent written rotation value given
    // the device dependent read rotation value *n*.
    pub fn transformed_rotation(&self, n: i8) -> i8 {
        match self.model {
            Model::AuraHD | Model::AuraH2O => n ^ 2,
            Model::AuraH2OEd2V2 |
            Model::Forma | Model::Forma32GB => (4 - n) % 4,
            _ => n,
        }
    }

    pub fn transformed_gyroscope_rotation(&self, n: i8) -> i8 {
        match self.model {
            Model::LibraH2O => n ^ 1,
            Model::Libra2 |
            Model::Sage |
            Model::Elipsa2E => (6 - n) % 4,
            Model::Elipsa => (4 - n) % 4,
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
}

#[cfg(test)]
mod tests {
    use super::Device;

    #[test]
    fn test_device_canonical_rotation() {
        let forma = Device::new("frost", "377");
        let aura_one = Device::new("daylight", "373");
        for n in 0..4 {
            assert_eq!(forma.from_canonical(forma.to_canonical(n)), n);
        }
        assert_eq!(aura_one.from_canonical(0), aura_one.startup_rotation());
        assert_eq!(forma.from_canonical(1) - forma.from_canonical(0),
                   aura_one.from_canonical(2) - aura_one.from_canonical(3));
    }
}
