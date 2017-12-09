use std::io::prelude::*;
use std::fs::File;
use std::fs::OpenOptions;
use std::path::PathBuf;
use fnv::FnvHashMap;
use frontlight::{FrontLight, Color};

error_chain!{
    foreign_links {
        Io(::std::io::Error);
        Parse(::std::num::ParseIntError);
    }
}

const FRONTLIGHT_INTERFACE: &str = "/sys/class/backlight";

const FRONTLIGHT_WHITE: &str = "lm3630a_led1b";
const FRONTLIGHT_RED: &str = "lm3630a_led1a";
const FRONTLIGHT_GREEN: &str = "lm3630a_ledb";

const FRONTLIGHT_VALUE: &str = "brightness";
const FRONTLIGHT_ACTUAL_VALUE: &str = "actual_brightness";
const FRONTLIGHT_MAX_VALUE: &str = "max_brightness";
const FRONTLIGHT_POWER: &str = "bl_power";

const FRONTLIGHT_POWER_ON: i16 = 31;
const FRONTLIGHT_POWER_OFF: i16 = 0;

lazy_static! {
pub static ref FRONTLIGHT_DIRS: FnvHashMap<Color, &'static str> =
    [(Color::White, FRONTLIGHT_WHITE),
     (Color::Red, FRONTLIGHT_RED),
     (Color::Green, FRONTLIGHT_GREEN)].iter().cloned().collect();
}

pub struct NaturalLight {
    base: PathBuf,
    maxima: FnvHashMap<Color, i16>,
    values: FnvHashMap<Color, File>,
    powers: FnvHashMap<Color, File>,
}

impl NaturalLight {
    pub fn new() -> Result<NaturalLight> {
        let mut maxima = FnvHashMap::default();
        let mut values = FnvHashMap::default();
        let mut powers = FnvHashMap::default();
        let base = PathBuf::from(FRONTLIGHT_INTERFACE);
        for c in [Color::White, Color::Red, Color::Green].iter().cloned() {
            let dir = base.join(FRONTLIGHT_DIRS.get(&c).unwrap());
            let mut buf = String::new();
            let mut file = File::open(dir.join(FRONTLIGHT_MAX_VALUE))?;
            file.read_to_string(&mut buf)?;
            maxima.insert(c, buf.parse()?);
            let file = OpenOptions::new().write(true).open(dir.join(FRONTLIGHT_VALUE))?;
            values.insert(c, file);
            let file = OpenOptions::new().write(true).open(dir.join(FRONTLIGHT_POWER))?;
            powers.insert(c, file);
        }
        Ok(NaturalLight {
            base,
            maxima,
            values,
            powers,
        })
    }

}

// TODO: return result
impl FrontLight for NaturalLight {
    fn get(&self, c: Color) -> f32 {
        let dir = self.base.join(FRONTLIGHT_DIRS.get(&c).unwrap());
        let mut buf = String::new();
        let mut file = File::open(dir.join(FRONTLIGHT_ACTUAL_VALUE)).unwrap();
        file.read_to_string(&mut buf).unwrap();
        let max_value = self.maxima[&c] as f32;
        100.0 * buf.parse::<f32>().unwrap() / max_value
    }

    fn set(&mut self, c: Color, percent: f32) {
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
}
