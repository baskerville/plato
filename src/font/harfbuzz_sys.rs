#![allow(unused)]

extern crate libc;

use std::mem;
use super::freetype_sys::FtFace;

pub const HB_DIRECTION_LTR: libc::c_uint = 4;

pub type HbDirection = libc::c_uint;
pub type HbTag = libc::uint32_t;
pub type HbBool = libc::c_int;

pub enum HbBuffer {}
pub enum HbFont {}

#[link(name="harfbuzz")]
extern {
    pub fn hb_ft_font_create(face: *mut FtFace, destroy: *const libc::c_void) -> *mut HbFont;
    pub fn hb_ft_font_changed(font: *mut HbFont);
    pub fn hb_font_destroy(font: *mut HbFont);
    pub fn hb_buffer_create() -> *mut HbBuffer;
    pub fn hb_buffer_destroy(buf: *mut HbBuffer);
    pub fn hb_buffer_add_utf8(buf: *mut HbBuffer, txt: *const libc::c_char, len: libc::c_int, offset: libc::c_uint, ilen: libc::c_int);
    pub fn hb_buffer_set_direction(buf: *mut HbBuffer, dir: HbDirection);
    pub fn hb_buffer_guess_segment_properties(buf: *mut HbBuffer);
    pub fn hb_shape(font: *mut HbFont, buf: *mut HbBuffer, features: *const HbFeature, features_count: libc::c_uint);
    pub fn hb_feature_from_string(s: *const libc::c_char, len: libc::c_int, feature: *mut HbFeature) -> HbBool;
    pub fn hb_buffer_get_length(buf: *mut HbBuffer) -> libc::c_uint;
    pub fn hb_buffer_get_glyph_infos(buf: *mut HbBuffer, len: *mut libc::c_uint) -> *mut HbGlyphInfo;
    pub fn hb_buffer_get_glyph_positions(buf: *mut HbBuffer, len: *mut libc::c_uint) -> *mut HbGlyphPosition;
}

#[repr(C)]
#[derive(Debug)]
pub struct HbGlyphInfo {
    pub codepoint: u32,
    mask: u32,
    cluster: u32,
    var1: u32,
    var2: u32,
}

#[repr(C)]
#[derive(Debug)]
pub struct HbGlyphPosition {
    pub x_advance: i32,
    pub y_advance: i32,
    pub x_offset: i32,
    pub y_offset: i32,
    var: u32,
}

#[repr(C)]
pub struct HbFeature {
    tag: HbTag,
    value: libc::uint32_t,
    start: libc::c_uint,
    end: libc::c_uint,
}

impl Default for HbFeature {
    fn default() -> Self {
        unsafe { mem::zeroed() }
    }
}
