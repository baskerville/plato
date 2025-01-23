#![allow(unused)]

use std::mem::ManuallyDrop;
use nix::{ioctl_readwrite_bad, ioctl_write_ptr_bad};
ioctl_readwrite_bad!(send_update, DISP_EINK_UPDATE2, SunxiDispEinkUpdate2);
ioctl_write_ptr_bad!(wait_for_update, DISP_EINK_WAIT_FRAME_SYNC_COMPLETE, SunxiDispEinkWaitFrameSyncComplete);

pub const DISP_EINK_UPDATE2: libc::c_ulong = 0x0406;
pub const DISP_EINK_WAIT_FRAME_SYNC_COMPLETE: libc::c_ulong = 0x4014;

pub const LAYER_MODE_BUFFER: libc::c_int = 0;
pub const DISP_FORMAT_8BIT_GRAY: libc::c_int = 0x50;
pub const DISP_GBR_F: libc::c_int = 0x200;
pub const DISP_BF_NORMAL: libc::c_int = 0;
pub const DISP_SCAN_PROGRESSIVE: libc::c_int = 0;
pub const DISP_EOTF_GAMMA22: libc::c_int = 0x004;

#[repr(C)]
#[derive(Debug)]
pub struct AreaInfo {
    pub x_top: libc::c_uint,
    pub y_top: libc::c_uint,
    pub x_bottom: libc::c_uint,
    pub y_bottom: libc::c_uint,
}

#[repr(C)]
#[derive(Debug)]
pub struct DispRect {
    pub x: libc::c_int,
    pub y: libc::c_int,
    pub width: libc::c_uint,
    pub height: libc::c_uint,
}

#[repr(C)]
pub struct DispRect64 {
    pub x: libc::c_longlong,
    pub y: libc::c_longlong,
    pub width: libc::c_longlong,
    pub height: libc::c_longlong,
}

#[repr(C)]
#[derive(Debug)]
pub struct DispRectsz {
    pub width: libc::c_uint,
    pub height: libc::c_uint,
}

#[repr(C)]
pub struct DispLayerConfig2 {
    pub info: DispLayerInfo2,
    pub enable: bool,
    pub channel: libc::c_uint,
    pub layer_id: libc::c_uint,
}

#[repr(C)]
pub struct DispAtwInfo {
    pub used: bool,
    pub mode: libc::c_int,
    pub b_row: libc::c_uint,
    pub b_col: libc::c_uint,
    pub colf_fd: libc::c_int,
}

#[repr(C)]
pub struct DispLayerInfo2 {
    pub mode: libc::c_int,
    pub zorder: libc::c_uchar,
    pub alpha_mode: libc::c_uchar,
    pub alpha_value: libc::c_uchar,
    pub screen_win: DispRect,
    pub b_trd_out: bool,
    pub out_trd_mode: libc::c_int,
    pub color_fb: ColorFb,
    pub id: libc::c_uint,
    pub atw: DispAtwInfo,
}

#[repr(C)]
pub struct DispFbInfo2 {
    pub fd: libc::c_int,
    pub y8_fd: libc::c_int,
    pub size: [DispRectsz; 3],
    pub align: [libc::c_uint; 3],
    pub format: libc::c_int,
    pub color_space: libc::c_int,
    pub trd_right_fd: libc::c_int,
    pub pre_multiply: bool,
    pub crop: DispRect64,
    pub flags: libc::c_int,
    pub scan: libc::c_int,
    pub eotf: libc::c_int,
    pub depth: libc::c_int,
    pub fbd_en: libc::c_uint,
    pub metadata_fd: libc::c_int,
    pub metadata_size: libc::c_uint,
    pub metadata_flag: libc::c_uint,
}

#[repr(C)]
pub union ColorFb {
    pub color: libc::c_uint,
    pub fb: ManuallyDrop<DispFbInfo2>,
}

#[repr(C)]
#[derive(Clone, Debug)]
pub struct SunxiDispEinkUpdate2 {
    pub area: *const AreaInfo,
    pub layer_num: libc::c_ulong,
    pub update_mode: libc::c_ulong,
    pub lyr_cfg2: *const DispLayerConfig2,
    pub frame_id: *mut libc::c_uint,
    pub rotate: *const u32,
    pub cfa_use: libc::c_ulong,
}

#[repr(C)]
#[derive(Clone, Debug)]
pub struct SunxiDispEinkWaitFrameSyncComplete {
    pub frame_id: u32,
}

pub const EINK_INIT_MODE:u32  = 0x01;
pub const EINK_DU_MODE:u32    = 0x02;
pub const EINK_GC16_MODE:u32  = 0x04;
pub const EINK_GC4_MODE:u32   = 0x08;
pub const EINK_A2_MODE:u32    = 0x10;
pub const EINK_GL16_MODE:u32  = 0x20;
pub const EINK_GLR16_MODE:u32 = 0x40;
pub const EINK_GLD16_MODE:u32 = 0x80;
pub const EINK_GU16_MODE:u32  = 0x84;
pub const EINK_GCK16_MODE:u32 = 0x90;
pub const EINK_GLK16_MODE:u32 = 0x94;
pub const EINK_CLEAR_MODE:u32 = 0x88;
pub const EINK_GC4L_MODE:u32  = 0x8c;
pub const EINK_GCC16_MODE:u32 = 0xa0;

pub const EINK_AUTO_MODE:u32        = 0x0000_8000;
pub const EINK_DITHERING_Y1:u32     = 0x0180_0000;
pub const EINK_DITHERING_Y4:u32     = 0x0280_0000;
pub const EINK_DITHERING_SIMPLE:u32 = 0x0480_0000;
pub const EINK_DITHERING_NTX_Y1:u32 = 0x0880_0000;

pub const EINK_GAMMA_CORRECT:u32 = 0x0020_0000;
pub const EINK_MONOCHROME:u32    = 0x0040_0000;
pub const EINK_NEGATIVE_MODE:u32 = 0x0001_0000;
pub const EINK_REGAL_MODE:u32    = 0x0008_0000;
pub const EINK_NO_MERGE:u32      = 0x8000_0000;

pub const EINK_PARTIAL_MODE:u32 = 0x0400;
