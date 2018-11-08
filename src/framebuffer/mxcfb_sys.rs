#![allow(unused)]

extern crate libc;

use std::mem;

pub const FBIOGET_VSCREENINFO: libc::c_ulong = 0x4600;
pub const FBIOPUT_VSCREENINFO: libc::c_ulong = 0x4601;
pub const FBIOGET_FSCREENINFO: libc::c_ulong = 0x4602;

// Platform dependent
pub const MXCFB_SEND_UPDATE: libc::c_ulong = 0x4044_462E;
pub const MXCFB_WAIT_FOR_UPDATE_COMPLETE: libc::c_ulong = 0x4004_462F;

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

#[repr(C)]
#[derive(Clone, Debug)]
pub struct MxcfbRect {
    pub top: u32,
    pub left: u32,
    pub width: u32,
    pub height: u32,
}

#[repr(C)]
#[derive(Clone, Debug)]
pub struct MxcfbAltBufferData {
    pub virt_addr: *const libc::c_void,
    pub phys_addr: u32,
    pub width: u32,
    pub height: u32,
    pub alt_update_region: MxcfbRect,
}

#[repr(C)]
#[derive(Clone, Debug)]
pub struct MxcfbUpdateData {
    pub update_region: MxcfbRect,
    pub waveform_mode: u32,
    pub update_mode: u32,
    pub update_marker: u32,
    pub temp: libc::c_int,
    pub flags: libc::c_uint,
    pub alt_buffer_data: MxcfbAltBufferData,
}

pub const WAVEFORM_MODE_AUTO: u32 = 0x101;

// Table taken from ice40_eink_controller
//
//    Type  │ Initial State │ Final State │ Waveform Period
//    ──────┼───────────────┼─────────────┼────────────────
//    INIT  │      0-F      │      F      │    4000 ms 
//    DU    │      0-F      │     0/F     │     260 ms
//    GC16  │      0-F      │     0-F     │     760 ms
//    GC4   │      0-F      │   0/5/A/F   │     500 ms
//    A2    │      0/F      │     0/F     │     120 ms

// Most of the comments are taken from include/linux/mxcfb.h in
// the kindle's Oasis sources (Kindle_src_5.8.10_3202110019.tar.gz)
pub const NTX_WFM_MODE_INIT: u32  = 0; // Screen goes to white (clears)
pub const NTX_WFM_MODE_DU: u32    = 1; // Grey->white/grey->black
pub const NTX_WFM_MODE_GC16: u32  = 2; // High fidelity (flashing)
pub const NTX_WFM_MODE_GC4: u32   = 3;
pub const NTX_WFM_MODE_A2: u32    = 4; // Fast but low fidelity
pub const NTX_WFM_MODE_GL16: u32  = 5; // High fidelity from white transition
pub const NTX_WFM_MODE_GLR16: u32 = 6; // Used for partial REAGL updates?
pub const NTX_WFM_MODE_GLD16: u32 = 7; // Dithering REAGL?

pub const UPDATE_MODE_PARTIAL: u32 = 0x0;
pub const UPDATE_MODE_FULL: u32    = 0x1;

pub const TEMP_USE_AMBIENT: libc::c_int = 0x1000;

pub const EPDC_FLAG_ENABLE_INVERSION: libc::c_uint = 0x01;
pub const EPDC_FLAG_FORCE_MONOCHROME: libc::c_uint = 0x02;
