use std::io::Write;
use std::fs::File;
use std::fs::OpenOptions;
use anyhow::Error;
use crate::device::{CURRENT_DEVICE, Model};
use super::{Frontlight, LightLevels};

const FRONTLIGHT_WHITE: &str = "/sys/class/backlight/mxc_msp430.0/brightness";

// Forma
const FRONTLIGHT_ORANGE_A: &str = "/sys/class/backlight/tlc5947_bl/color";
// Libra H₂O, Clara HD, Libra 2
const FRONTLIGHT_ORANGE_B: &str = "/sys/class/backlight/lm3630a_led/color";
// Sage
const FRONTLIGHT_ORANGE_C: &str =  "/sys/class/leds/aw99703-bl_FL1/color";

pub struct PremixedFrontlight {
    intensity: f32,
    warmth: f32,
    white: File,
    orange: File,
}

impl PremixedFrontlight {
    pub fn new(intensity: f32, warmth: f32) -> Result<PremixedFrontlight, Error> {
        let white = OpenOptions::new().write(true).open(FRONTLIGHT_WHITE)?;
        let model = CURRENT_DEVICE.model;
        let orange_path = match model {
            Model::Forma | Model::Forma32GB => FRONTLIGHT_ORANGE_A,
            Model::LibraH2O | Model::ClaraHD | Model::Libra2 => FRONTLIGHT_ORANGE_B,
            Model::Sage => FRONTLIGHT_ORANGE_C,
            _ => FRONTLIGHT_ORANGE_A,
        };
        let orange = OpenOptions::new().write(true).open(orange_path)?;
        Ok(PremixedFrontlight { intensity, warmth, white, orange })
    }
}

impl Frontlight for PremixedFrontlight {
    fn set_intensity(&mut self, intensity: f32) {
        let white = intensity.round() as i16;
        write!(self.white, "{}", white).unwrap();
        self.intensity = intensity;
    }

    fn set_warmth(&mut self, warmth: f32) {
        let mut orange = (warmth / 10.0).round() as i16;
        if CURRENT_DEVICE.mark() != 8 {
            orange = 10 - orange;
        }
        write!(self.orange, "{}", orange).unwrap();
        self.warmth = warmth;
    }

    fn levels(&self) -> LightLevels {
        LightLevels {
            intensity: self.intensity,
            warmth: self.warmth,
        }
    }
}
