use frontlight::{Frontlight, LightLevels};
use errors::*;


pub struct FakeFrontlight {
}

impl FakeFrontlight {
    pub fn new() -> Result<FakeFrontlight> {
        Ok(FakeFrontlight {
        })
    }


}

impl Frontlight for FakeFrontlight {
    fn set_intensity(&mut self, value: f32) {
    }

    fn set_warmth(&mut self, value: f32) {
    }

    fn levels(&self) -> LightLevels {
        LightLevels {
            intensity: 0.0,
            warmth: 0.0,
        }
    }
}
