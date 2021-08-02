#![allow(unused)]

use std::mem;
use std::fs::File;
use std::os::unix::io::AsRawFd;
use anyhow::{Error, Context};
use nix::{ioctl_read_bad, ioctl_write_ptr_bad};

ioctl_read_bad!(read_variable_screen_info, FBIOGET_VSCREENINFO, VarScreenInfo);
ioctl_write_ptr_bad!(write_variable_screen_info, FBIOPUT_VSCREENINFO, VarScreenInfo);
ioctl_read_bad!(read_fixed_screen_info, FBIOGET_FSCREENINFO, FixScreenInfo);

pub const FBIOGET_VSCREENINFO: libc::c_ulong = 0x4600;
pub const FBIOPUT_VSCREENINFO: libc::c_ulong = 0x4601;
pub const FBIOGET_FSCREENINFO: libc::c_ulong = 0x4602;

#[repr(C)]
#[derive(Clone, Debug)]
pub struct FixScreenInfo {
    pub id: [u8; 16],
    pub smem_start: usize,
    pub smem_len: u32,
    pub kind: u32,
    pub type_aux: u32,
    pub visual: u32,
    pub xpanstep: u16,
    pub ypanstep: u16,
    pub ywrapstep: u16,
    pub line_length: u32,
    pub mmio_start: usize,
    pub mmio_len: u32,
    pub accel: u32,
    pub capabilities: u16,
    pub reserved: [u16; 2],
}

#[repr(C)]
#[derive(Clone, Debug)]
pub struct VarScreenInfo {
    pub xres: u32,
    pub yres: u32,
    pub xres_virtual: u32,
    pub yres_virtual: u32,
    pub xoffset: u32,
    pub yoffset: u32,
    pub bits_per_pixel: u32,
    pub grayscale: u32,
    pub red: Bitfield,
    pub green: Bitfield,
    pub blue: Bitfield,
    pub transp: Bitfield,
    pub nonstd: u32,
    pub activate: u32,
    pub height: u32,
    pub width: u32,
    pub accel_flags: u32,
    pub pixclock: u32,
    pub left_margin: u32,
    pub right_margin: u32,
    pub upper_margin: u32,
    pub lower_margin: u32,
    pub hsync_len: u32,
    pub vsync_len: u32,
    pub sync: u32,
    pub vmode: u32,
    pub rotate: u32,
    pub colorspace: u32,
    pub reserved: [u32; 4],
}

#[repr(C)]
#[derive(Clone, Debug)]
pub struct Bitfield {
    pub offset: u32,
    pub length: u32,
    pub msb_right: u32,
}

impl Default for Bitfield {
    fn default() -> Self {
        unsafe { mem::zeroed() }
    }
}

impl Default for VarScreenInfo {
    fn default() -> Self {
        unsafe { mem::zeroed() }
    }
}

impl Default for FixScreenInfo {
    fn default() -> Self {
        unsafe { mem::zeroed() }
    }
}

pub fn fix_screen_info(file: &File) -> Result<FixScreenInfo, Error> {
    let mut info: FixScreenInfo = Default::default();
    let result = unsafe {
        read_fixed_screen_info(file.as_raw_fd(), &mut info)
    };
    match result {
        Err(e) => Err(Error::from(e).context("can't get fixed screen info")),
        _ => Ok(info),
    }
}

pub fn var_screen_info(file: &File) -> Result<VarScreenInfo, Error> {
    let mut info: VarScreenInfo = Default::default();
    let result = unsafe {
        read_variable_screen_info(file.as_raw_fd(), &mut info)
    };
    match result {
        Err(e) => Err(Error::from(e).context("can't get variable screen info")),
        _ => Ok(info),
    }
}
