use std::io::Read;
use std::io::Write;
use std::fs::File;
use std::fs::OpenOptions;
use std::path::PathBuf;
use fnv::FnvHashMap;
use frontlight::{Frontlight, LightLevels};
use errors::*;

const FRONTLIGHT_INTERFACE: &str = "/sys/class/backlight";

const FRONTLIGHT_WHITE: &str = "lm3630a_led1b";
const FRONTLIGHT_RED: &str = "lm3630a_led1a";
const FRONTLIGHT_GREEN: &str = "lm3630a_ledb";

const FRONTLIGHT_VALUE: &str = "brightness";
const FRONTLIGHT_MAX_VALUE: &str = "max_brightness";
const FRONTLIGHT_POWER: &str = "bl_power";

const FRONTLIGHT_POWER_ON: i16 = 31;
const FRONTLIGHT_POWER_OFF: i16 = 0;

#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub enum LightColor {
    White,
    Green,
    Red,
}

lazy_static! {
pub static ref FRONTLIGHT_DIRS: FnvHashMap<LightColor, &'static str> =
    [(LightColor::White, FRONTLIGHT_WHITE),
     (LightColor::Red, FRONTLIGHT_RED),
     (LightColor::Green, FRONTLIGHT_GREEN)].iter().cloned().collect();
}

pub struct NaturalFrontlight {
    intensity: f32,
    warmth: f32,
    base: PathBuf,
    values: FnvHashMap<LightColor, File>,
    powers: FnvHashMap<LightColor, File>,
    maxima: FnvHashMap<LightColor, i16>,
}

impl NaturalFrontlight {
    pub fn new(intensity: f32, warmth: f32) -> Result<NaturalFrontlight> {
        let mut maxima = FnvHashMap::default();
        let mut values = FnvHashMap::default();
        let mut powers = FnvHashMap::default();
        let base = PathBuf::from(FRONTLIGHT_INTERFACE);
        for c in [LightColor::White, LightColor::Red, LightColor::Green].iter().cloned() {
            let dir = base.join(FRONTLIGHT_DIRS.get(&c).unwrap());
            let mut buf = String::new();
            let mut file = File::open(dir.join(FRONTLIGHT_MAX_VALUE))?;
            file.read_to_string(&mut buf)?;
            maxima.insert(c, buf.trim_right().parse()?);
            let file = OpenOptions::new().write(true).open(dir.join(FRONTLIGHT_VALUE))?;
            values.insert(c, file);
            let file = OpenOptions::new().write(true).open(dir.join(FRONTLIGHT_POWER))?;
            powers.insert(c, file);
        }
        Ok(NaturalFrontlight {
            intensity,
            warmth,
            base,
            maxima,
            values,
            powers,
        })
    }

    fn set(&mut self, c: LightColor, percent: f32) {
        let max_value = self.maxima[&c] as f32;
        let value = (percent.max(0.0).min(100.0) / 100.0 * max_value) as i16;
        let mut file = &self.values[&c];
        write!(file, "{}", value).unwrap();
        let mut file = &self.powers[&c];
        let power = if value > 0 {
            FRONTLIGHT_POWER_ON
        } else {
            FRONTLIGHT_POWER_OFF
        };
        write!(file, "{}", power).unwrap();
    }

    fn update(&mut self, intensity: f32, warmth: f32) {
        let i = intensity / 100.0;
        let w = warmth / 100.0;
        let white = 80.0 * i * (1.0 - w).sqrt();
        let green = 64.0 * (w * i).sqrt();
        let red = if w == 0.0 {
            0.0
        } else {
            green + 20.0 + 7.0 * (1.0 - green / 64.0) + w * 4.0
        };
        self.set(LightColor::White, white);
        self.set(LightColor::Green, green);
        self.set(LightColor::Red, red);
        self.intensity = intensity;
        self.warmth = warmth;
    }
}

impl Frontlight for NaturalFrontlight {
    fn set_intensity(&mut self, value: f32) {
        let warmth = self.warmth;
        self.update(value, warmth);
    }

    fn set_warmth(&mut self, value: f32) {
        let intensity = self.intensity;
        self.update(intensity, value);
    }

    fn intensity(&self) -> f32 {
        self.intensity
    }

    fn warmth(&self) -> f32 {
        self.warmth
    }

    fn levels(&self) -> LightLevels {
        LightLevels::Natural(self.intensity, self.warmth)
    }
}
