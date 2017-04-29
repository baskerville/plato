extern crate libc;

use input::TouchProto;
use std::env;

#[derive(Debug)]
pub enum Model {
    Touch,
    Glo,
    Mini,
    AuraHD,
    Aura,
    AuraH2O,
    GloHD,
    Touch2,
    AuraONE,
    AuraEdition2,
}

#[derive(Debug)]
pub struct Device {
    pub model: Model,
    pub proto: TouchProto,
    pub dpi: u16,
}

impl Default for Device {
    fn default() -> Device {
        Device {
            model: Model::Touch,
            proto: TouchProto::Single,
            dpi: 167,
        }
    }
}

impl Device {
    pub fn current() -> Device {
        let product = env::var("PRODUCT").unwrap_or("trilogy".to_owned());
        match product.as_ref() {
            "kraken" => Device {
                model: Model::Glo,
                proto: TouchProto::Single,
                dpi: 212,
            },
            "pixie" => Device {
                model: Model::Mini,
                proto: TouchProto::Single,
                dpi: 200,
            },
            "dragon" => Device {
                model: Model::AuraHD,
                proto: TouchProto::Single,
                dpi: 265,
            },
            "phoenix" => Device {
                model: Model::Aura,
                proto: TouchProto::Multi,
                dpi: 212,
            },
            "dahlia" => Device {
                model: Model::AuraH2O,
                proto: TouchProto::Multi,
                dpi: 265,
            },
            "alyssum" => Device {
                model: Model::GloHD,
                proto: TouchProto::Multi,
                dpi: 300,
            },
            "pika" => Device {
                model: Model::Touch2,
                proto: TouchProto::Multi,
                dpi: 167,
            },
            "daylight" => Device {
                model: Model::AuraONE,
                proto: TouchProto::Multi,
                dpi: 300,
            },
            "star" => Device {
                model: Model::AuraEdition2,
                proto: TouchProto::Multi,
                dpi: 212,
            },
            _ => Device::default(),
        }
    }
}
