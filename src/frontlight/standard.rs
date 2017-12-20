extern crate libc;

use std::fs::File;
use std::fs::OpenOptions;
use std::os::unix::io::AsRawFd;
use frontlight::{FrontLight, Color};

error_chain!{
    foreign_links {
        Io(::std::io::Error);
    }
}

const CM_FRONT_LIGHT_SET: libc::c_ulong = 241;
const FRONTLIGHT_INTERFACE: &str = "/dev/ntx_io";

pub struct StandardLight {
    value: u8,
    interface: File,
}

impl StandardLight {
    pub fn new(value: Option<u8>) -> Result<StandardLight> {
        let value = value.unwrap_or(0);
        let interface = OpenOptions::new().write(true)
                                    .open(FRONTLIGHT_INTERFACE)?;
        Ok(StandardLight { value, interface })
    }
}

impl FrontLight for StandardLight {
    fn get(&self, c: Color) -> f32 {
        if c == Color::White {
            self.value as f32
        } else {
            0.0
        }
    }

    fn set(&mut self, c: Color, percent: f32) {
        if c != Color::White {
            return;
        }
        let ret = unsafe {
            libc::ioctl(self.interface.as_raw_fd(),
                        CM_FRONT_LIGHT_SET, self.value as libc::c_int)
        };
        if ret != -1 {
            self.value = percent as u8;
        }
    }
}
