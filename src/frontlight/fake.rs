use frontlight::{Frontlight, LightLevels};

pub struct FakeFrontlight {
    intensity: f32,
    warmth: f32,
}

impl FakeFrontlight {
    pub fn new() -> FakeFrontlight {
        FakeFrontlight {
            intensity: 0.0,
            warmth: 0.0,
        }
    }
}

impl Frontlight for FakeFrontlight {
    fn intensity(&self) -> f32 {
        self.intensity
    }

    fn warmth(&self) -> f32 {
        self.warmth
    }

    fn set_intensity(&mut self, value: f32) {
        self.intensity = value;
    }

    fn set_warmth(&mut self, value: f32) {
        self.warmth = value;
    }

    fn levels(&self) -> LightLevels {
        LightLevels::Natural(self.intensity, self.warmth)
    }
}
