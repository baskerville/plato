mod standard;
mod natural;
mod fake;

pub use self::standard::StandardFrontlight;
pub use self::natural::NaturalFrontlight;
pub use self::fake::FakeFrontlight;

#[derive(Debug, Copy, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum LightLevels {
    Standard(f32),
    Natural(f32, f32),
}

impl LightLevels {
    pub fn intensity(&self) -> f32 {
        match *self {
            LightLevels::Standard(v) => v,
            LightLevels::Natural(v, _) => v,
        }
    }

    pub fn warmth(&self) -> f32 {
        match *self {
            LightLevels::Standard(_) => 0.0,
            LightLevels::Natural(_, v) => v,
        }
    }
}

pub trait Frontlight {
    fn set_intensity(&mut self, value: f32);
    fn set_warmth(&mut self, value: f32);
    fn intensity(&self) -> f32;
    fn warmth(&self) -> f32;
    fn levels(&self) -> LightLevels;
}
