#![allow(unused)]

use std::mem;
use std::ptr;
use nix::{ioctl_write_ptr, ioctl_readwrite};

const MAGIC: u8 = b'F';

ioctl_write_ptr!(send_update_v1, MAGIC, 0x2E, MxcfbUpdateDataV1);
ioctl_write_ptr!(send_update_v2, MAGIC, 0x2E, MxcfbUpdateDataV2);
ioctl_write_ptr!(send_update_v3, MAGIC, 0x2E, HwtConUpdateData);
ioctl_write_ptr!(wait_for_update_v1, MAGIC, 0x2F, u32);
ioctl_readwrite!(wait_for_update_v2, MAGIC, 0x2F, MxcfbUpdateMarkerData);

#[repr(C)]
#[derive(Clone, Debug)]
pub struct MxcfbRect {
    pub top: u32,
    pub left: u32,
    pub width: u32,
    pub height: u32,
}

impl Default for MxcfbRect {
    fn default() -> Self {
        unsafe { mem::zeroed() }
    }
}

#[repr(C)]
#[derive(Clone, Debug)]
pub struct MxcfbAltBufferDataV1 {
    pub virt_addr: *const libc::c_void,
    pub phys_addr: u32,
    pub width: u32,
    pub height: u32,
    pub alt_update_region: MxcfbRect,
}

impl Default for MxcfbAltBufferDataV1 {
    fn default() -> Self {
        MxcfbAltBufferDataV1 {
            virt_addr: ptr::null(),
            phys_addr: 0,
            width: 0,
            height: 0,
            alt_update_region: MxcfbRect::default(),
        }
    }
}

#[repr(C)]
#[derive(Clone, Debug)]
pub struct MxcfbUpdateDataV1 {
    pub update_region: MxcfbRect,
    pub waveform_mode: u32,
    pub update_mode: u32,
    pub update_marker: u32,
    pub temp: libc::c_int,
    pub flags: libc::c_uint,
    pub alt_buffer_data: MxcfbAltBufferDataV1,
}

#[repr(C)]
#[derive(Clone, Debug)]
pub struct MxcfbUpdateMarkerData {
    pub update_marker: u32,
    pub collision_test: u32,
}

#[repr(C)]
#[derive(Clone, Debug)]
pub struct MxcfbAltBufferDataV2 {
    pub phys_addr: u32,
    pub width: u32,
    pub height: u32,
    pub alt_update_region: MxcfbRect,
}

impl Default for MxcfbAltBufferDataV2 {
    fn default() -> Self {
        MxcfbAltBufferDataV2 {
            phys_addr: 0,
            width: 0,
            height: 0,
            alt_update_region: MxcfbRect::default(),
        }
    }
}

#[repr(C)]
#[derive(Clone, Debug)]
pub struct MxcfbUpdateDataV2 {
    pub update_region: MxcfbRect,
    pub waveform_mode: u32,
    pub update_mode: u32,
    pub update_marker: u32,
    pub temp: libc::c_int,
    pub flags: libc::c_uint,
    pub dither_mode: libc::c_int,
    pub quant_bit: libc::c_int,
    pub alt_buffer_data: MxcfbAltBufferDataV2,
}

#[repr(C)]
#[derive(Clone, Debug)]
pub struct HwtConUpdateData {
    pub update_region: MxcfbRect,
    pub waveform_mode: u32,
    pub update_mode: u32,
    pub update_marker: u32,
    pub flags: libc::c_uint,
    pub dither_mode: libc::c_int,
}

pub const WAVEFORM_MODE_AUTO: u32 = 0x101;

pub const NTX_WFM_MODE_INIT: u32   =  0;
pub const NTX_WFM_MODE_DU: u32     =  1;
pub const NTX_WFM_MODE_GC16: u32   =  2;
pub const NTX_WFM_MODE_GC4: u32    =  3;
pub const NTX_WFM_MODE_A2: u32     =  4;
pub const NTX_WFM_MODE_GL16: u32   =  5;
pub const NTX_WFM_MODE_GLR16: u32  =  6;
pub const NTX_WFM_MODE_GLD16: u32  =  7;
/* Mark 9 */
pub const NTX_WFM_MODE_DU4: u32    =  8;
pub const NTX_WFM_MODE_GCK16: u32  =  9;
pub const NTX_WFM_MODE_GLKW16: u32 = 10;

pub const UPDATE_MODE_PARTIAL: u32 = 0x0;
pub const UPDATE_MODE_FULL: u32    = 0x1;

pub const TEMP_USE_AMBIENT: libc::c_int = 0x1000;

pub const EPDC_FLAG_ENABLE_INVERSION: libc::c_uint = 0x01;
pub const EPDC_FLAG_FORCE_MONOCHROME: libc::c_uint = 0x02;

pub const EPDC_FLAG_TEST_COLLISION: libc::c_uint = 0x200;
pub const EPDC_FLAG_GROUP_UPDATE: libc::c_uint = 0x400;

pub const EPDC_FLAG_USE_AAD: libc::c_uint = 0x1000;
pub const EPDC_FLAG_USE_REGAL: libc::c_uint = 0x8000;

pub const EPDC_FLAG_USE_DITHERING_Y1: libc::c_uint = 0x2000;
pub const EPDC_FLAG_USE_DITHERING_Y4: libc::c_uint = 0x4000;
pub const EPDC_FLAG_USE_DITHERING_NTX_D8: libc::c_uint = 0x100000;

pub const EPDC_FLAG_USE_DITHERING_PASSTHROUGH: libc::c_int = 0;
// pub const EPDC_FLAG_USE_DITHERING_FLOYD_STEINBERG: libc::c_int = 1;
// pub const EPDC_FLAG_USE_DITHERING_ATKINSON: libc::c_int = 2;
pub const EPDC_FLAG_USE_DITHERING_ORDERED: libc::c_int = 3;
// pub const EPDC_FLAG_USE_DITHERING_QUANT_ONLY: libc::c_int = 4;

/* Mark 11 */
pub const HWTCON_WAVEFORM_MODE_GL16  :u32 = 3;
pub const HWTCON_WAVEFORM_MODE_GLR16 :u32 = 4;
pub const HWTCON_WAVEFORM_MODE_REAGL :u32 = 4;
pub const HWTCON_WAVEFORM_MODE_A2    :u32 = 6;
pub const HWTCON_WAVEFORM_MODE_GCK16 :u32 = 8;
pub const HWTCON_WAVEFORM_MODE_GLKW16:u32 = 9;

pub const HWTCON_FLAG_USE_DITHERING: libc::c_uint = 0x1;
pub const HWTCON_FLAG_FORCE_A2_OUTPUT: libc::c_uint = 16;

pub const HWTCON_FLAG_USE_DITHERING_Y8_Y1_S: libc::c_int = 770;
pub const HWTCON_FLAG_USE_DITHERING_Y8_Y4_S: libc::c_int = 258;
