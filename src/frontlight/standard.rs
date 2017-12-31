extern crate libc;

use std::fs::File;
use std::fs::OpenOptions;
use std::os::unix::io::AsRawFd;
use frontlight::{Frontlight, LightLevels};
use errors::*;

const CM_FRONT_LIGHT_SET: libc::c_ulong = 241;
const FRONTLIGHT_INTERFACE: &str = "/dev/ntx_io";

pub struct StandardFrontlight {
    value: f32,
    interface: File,
}

impl StandardFrontlight {
    pub fn new(value: f32) -> Result<StandardFrontlight> {
        let interface = OpenOptions::new().write(true)
                                    .open(FRONTLIGHT_INTERFACE)?;
        Ok(StandardFrontlight { value, interface })
    }
}

impl Frontlight for StandardFrontlight {
    fn intensity(&self) -> f32 {
        self.value
    }

    fn warmth(&self) -> f32 {
        0.0
    }

    fn set_intensity(&mut self, value: f32) {
        let ret = unsafe {
            libc::ioctl(self.interface.as_raw_fd(),
                        CM_FRONT_LIGHT_SET, value as libc::c_int)
        };
        if ret != -1 {
            self.value = value;
        }
    }

    fn set_warmth(&mut self, _value: f32) {
    }

    fn levels(&self) -> LightLevels {
        LightLevels::Standard(self.value)
    }
}
