#![allow(unused)]

extern crate libc;

pub const FT_ERR_OK: FtError = 0;

pub const FT_LOAD_DEFAULT: i32 = 0;
pub const FT_LOAD_NO_SCALE: i32 = 0x1 << 0;
pub const FT_LOAD_NO_HINTING: i32 = 0x1 << 1;
pub const FT_LOAD_RENDER: i32 = 0x1 << 2;


pub const FT_GLYPH_BBOX_UNSCALED: GlyphBBoxMode = 0;
pub const FT_GLYPH_BBOX_PIXELS: GlyphBBoxMode = 3;

pub type FtError = libc::c_int;
pub type FtByte = libc::c_uchar;
pub type FtF26Dot6 = libc::c_long;
pub type FtPos = libc::c_long;
pub type FtFixed = libc::c_long;
pub type FtGlyphFormat = libc::c_uint;
pub type GlyphBBoxMode = libc::c_uint;
pub type FtGenericFinalizer = extern fn(*mut libc::c_void);

pub enum FtLibrary {}
pub enum FtCharMap {}
pub enum FtSizeInternal {}
pub enum FtSlotInternal {}
pub enum FtFaceInternal {}
pub enum FtListNode {}
pub enum FtDriver {}
pub enum FtMemory {}
pub enum FtStream {}
pub enum FtNamedStyle {}

#[link(name="freetype")]
extern {
    pub fn FT_Init_FreeType(lib: *mut *mut FtLibrary) -> FtError;
    pub fn FT_Done_FreeType(lib: *mut FtLibrary) -> FtError;
    pub fn FT_New_Face(lib: *mut FtLibrary, path: *const libc::c_char, idx: libc::c_long, face: *mut *mut FtFace) -> FtError;
    pub fn FT_New_Memory_Face(lib: *mut FtLibrary, buf: *const FtByte, len: libc::c_long, idx: libc::c_long, face: *mut *mut FtFace) -> FtError;
    pub fn FT_Done_Face(face: *mut FtFace) -> FtError;
    pub fn FT_Set_Char_Size(face: *mut FtFace, sx: FtF26Dot6, sy: FtF26Dot6, rx: libc::c_uint, ry: libc::c_uint) -> FtError;
    pub fn FT_Load_Glyph(face: *const FtFace, idx: libc::c_uint, flags: i32) -> FtError;
    pub fn FT_Load_Char(face: *const FtFace, code: libc::c_ulong, flags: i32) -> FtError;
    pub fn FT_Get_Char_Index(face: *const FtFace, code: libc::c_ulong) -> libc::c_uint;
    pub fn FT_Get_MM_Var(face: *const FtFace, varia: *mut *mut FtMmVar) -> FtError;
    pub fn FT_Done_MM_Var(lib: *mut FtLibrary, varia: *mut FtMmVar) -> FtError;
    pub fn FT_Set_Var_Design_Coordinates(face: *mut FtFace, num_coords: libc::c_uint, coords: *const FtFixed) -> FtError;
}

#[repr(C)]
#[derive(Debug)]
pub struct FtMmVar {
    pub num_axis: libc::c_uint,
    num_designs: libc::c_uint,
    num_namedstyles: libc::c_uint,
    pub axis: *mut FtVarAxis,
    namedstyle: *mut FtNamedStyle,
}

#[repr(C)]
#[derive(Debug)]
pub struct FtVarAxis {
    name: *mut libc::c_char,
    pub minimum: FtFixed,
    pub def: FtFixed,
    pub maximum: FtFixed,
    pub tag: libc::c_ulong,
    strid: libc::c_uint,
}

#[repr(C)]
#[derive(Debug)]
pub struct FtBitmapSize {
    height: libc::c_short,
    width: libc::c_short,

    size: FtPos,

    x_ppem: FtPos,
    y_ppem: FtPos,
}

#[repr(C)]
pub struct FtSize {
    face: *mut FtFace,
    generic: FtGeneric,
    pub metrics: FtSizeMetrics,
    internal: *mut FtSizeInternal,
}

#[repr(C)]
#[derive(Debug)]
pub struct FtSizeMetrics {
    pub x_ppem: libc::c_ushort,
    pub y_ppem: libc::c_ushort,

    x_scale: FtFixed,
    y_scale: FtFixed,

    ascender: FtPos,
    descender: FtPos,
    height: FtPos,
    max_advance: FtPos
}

#[repr(C)]
#[derive(Debug)]
pub struct FtGeneric {
    data: *mut libc::c_void,
    finalizer: FtGenericFinalizer,
}

#[repr(C)]
#[derive(Debug)]
pub struct FtVector {
    x: FtPos,
    y: FtPos,
}

#[repr(C)]
#[derive(Debug)]
pub struct FtBBox {
    x_min: FtPos,
    y_min: FtPos,
    x_max: FtPos,
    y_max: FtPos,
}

#[repr(C)]
#[derive(Debug)]
pub struct FtBitmap {
    pub rows: libc::c_int,
    pub width: libc::c_int,
    pub pitch: libc::c_int,
    pub buffer: *mut libc::c_uchar,
    num_grays: libc::c_short,
    pixel_mode: libc::c_char,
    palette_mode: libc::c_char,
    palette: *mut libc::c_void,
}

#[repr(C)]
#[derive(Debug)]
pub struct FtGlyphMetrics {
    pub width: FtPos,
    pub height: FtPos,

    hori_bearing_x: FtPos,
    hori_bearing_y: FtPos,
    hori_advance: FtPos,

    vert_bearing_x: FtPos,
    vert_bearing_y: FtPos,
    vert_advance: FtPos,
}

#[repr(C)]
#[derive(Debug)]
pub struct FtOutline {
    n_contours: libc::c_short,
    n_points: libc::c_short,

    points: *mut FtVector,
    tags: *mut libc::c_char,
    contours: *mut libc::c_short,

    flags: libc::c_int,
}

#[repr(C)]
#[derive(Debug)]
pub struct FtGlyphSlot {
    library: *mut FtLibrary,
    face: *mut FtFace,
    next: *mut FtGlyphSlot,
    reserved: libc::c_uint,
    generic: FtGeneric,

    pub metrics: FtGlyphMetrics,
    linear_hori_advance: FtFixed,
    linear_vert_advance: FtFixed,
    advance: FtVector,

    format: FtGlyphFormat,

    pub bitmap: FtBitmap,
    pub bitmap_left: libc::c_int,
    pub bitmap_top: libc::c_int,

    outline: FtOutline,

    num_subglyphs: libc::c_uint,
    subglyphs: *mut libc::c_void,

    control_data: *mut libc::c_void,
    control_len: libc::c_long,

    lsb_delta: FtPos,
    rsb_delta: FtPos,

    other: *mut libc::c_void,

    internal: *mut FtSlotInternal,
}

#[repr(C)]
#[derive(Debug)]
pub struct FtList {
    head: *mut FtListNode,
    tail: *mut FtListNode,
}

#[repr(C)]
#[derive(Debug)]
pub struct FtFace {
    num_faces: libc::c_long,
    face_index: libc::c_long,

    face_flags: libc::c_long,
    style_flags: libc::c_long,

    num_glyphs: libc::c_long,

    family_name: *mut libc::c_char,
    style_name: *mut libc::c_char,

    num_fixed_sizes: libc::c_int,
    available_sizes: *mut FtBitmapSize,

    num_charmaps: libc::c_int,
    charmaps: *mut FtCharMap,

    generic: FtGeneric,

    bbox: FtBBox,

    units_per_em: libc::c_ushort,
    ascender: libc::c_short,
    descender: libc::c_short,
    height: libc::c_short,

    max_advance_width: libc::c_short,
    max_advance_height: libc::c_short,

    underline_position: libc::c_short,
    underline_thickness: libc::c_short,

    pub glyph: *mut FtGlyphSlot,
    pub size: *mut FtSize,
    charmap: *mut FtCharMap,

    driver: *mut FtDriver,
    memory: *mut FtMemory,
    stream: *mut FtStream,

    sizes_list: FtList,

    autohint: FtGeneric,
    extensions: *mut libc::c_void,

    internal: *mut FtFaceInternal,
}
