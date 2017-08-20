extern crate libc;

use std::ptr;
use std::ffi::CString;
use std::os::unix::ffi::OsStrExt;
use std::rc::Rc;
use std::path::Path;
use std::mem;
use geom::Point;
use framebuffer::Framebuffer;

pub const BOLD_THICKNESS_RATIO: f32 = 36.0 / 61.0;

// Font sizes in 1/64th of a point
// 2, 3, and 4 px at 300 DPI for Noto Sans UI
pub const FONT_SIZES: [u32; 3] = [349, 524, 699];

pub const KEYBOARD_FONT_SIZES: [u32; 2] = [337, 843];

pub const KEYBOARD_LETTER_SPACING: i32 = 4;

pub const NORMAL_STYLE: Style = Style {
    family: Family::SansSerif,
    variant: REGULAR,
    size: FONT_SIZES[1],
};

pub const MD_TITLE: Style = Style {
    family: Family::Serif,
    variant: ITALIC,
    size: FONT_SIZES[2],
};

pub const MD_AUTHOR: Style = Style {
    family: Family::Serif,
    variant: REGULAR,
    size: FONT_SIZES[1],
};

pub const MD_YEAR: Style = NORMAL_STYLE;

pub const MD_KIND: Style = Style {
    family: Family::SansSerif,
    variant: BOLD,
    size: FONT_SIZES[0],
};

pub const MD_SIZE: Style = Style {
    family: Family::SansSerif,
    variant: REGULAR,
    size: FONT_SIZES[0],
};

pub struct FontFamily {
    regular: Font,
    italic: Font,
    bold: Font,
    bold_italic: Font,
}

pub struct Fonts {
    sans_serif: FontFamily,
    serif: FontFamily,
    keyboard: Font,
    // fallback: Font,
}

impl Default for Fonts {
    fn default() -> Self {
        let fo = FontOpener::new().unwrap();
        Fonts {
            sans_serif: FontFamily {
                regular: fo.open("fonts/NotoSansUI-Regular.ttf").unwrap(),
                italic: fo.open("fonts/NotoSansUI-Italic.ttf").unwrap(),
                bold: fo.open("fonts/NotoSansUI-Bold.ttf").unwrap(),
                bold_italic: fo.open("fonts/NotoSansUI-BoldItalic.ttf").unwrap(),
            },
            serif: FontFamily {
                regular: fo.open("fonts/NotoSerif-Regular.ttf").unwrap(),
                italic: fo.open("fonts/NotoSerif-Italic.ttf").unwrap(),
                bold: fo.open("fonts/NotoSerif-Bold.ttf").unwrap(),
                bold_italic: fo.open("fonts/NotoSerif-BoldItalic.ttf").unwrap(),
            },
            keyboard: fo.open("fonts/VarelaRound-Regular.ttf").unwrap(),
        }
    }
}

bitflags! {
    pub flags Variant: u8 {
        const REGULAR = 0,
        const ITALIC = 1,
        const BOLD = 2,
    }
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum Family {
    SansSerif,
    Serif,
    Keyboard,
}

pub struct Style {
    family: Family,
    variant: Variant,
    size: u32,
}

pub fn font_from_variant(family: &mut FontFamily, variant: Variant) -> &mut Font {
    if variant.contains(ITALIC | BOLD) {
        &mut family.bold_italic
    } else if variant.contains(ITALIC) {
        &mut family.italic
    } else if variant.contains(BOLD) {
        &mut family.bold
    } else {
        &mut family.regular
    }
}

pub fn font_from_style<'a>(fonts: &'a mut Fonts, style: &Style, dpi: u16) -> &'a mut Font {
    let mut font = match style.family {
        Family::SansSerif => {
            let family = &mut fonts.sans_serif;
            font_from_variant(family, style.variant)
        },
        Family::Serif => {
            let family = &mut fonts.serif;
            font_from_variant(family, style.variant)
        },
        Family::Keyboard => &mut fonts.keyboard
    };
    font.set_size(style.size, dpi);
    font
}

const FT_ERR_OK: FtError = 0;

const FT_LOAD_DEFAULT: i32 = 0;
const FT_LOAD_NO_SCALE: i32 = 0x1 << 0;
const FT_LOAD_NO_HINTING: i32 = 0x1 << 1;
const FT_LOAD_RENDER: i32 = 0x1 << 2;

const FT_KERNING_DEFAULT: FtKerningMode = 0;

const FT_GLYPH_BBOX_UNSCALED: GlyphBBoxMode = 0;
const FT_GLYPH_BBOX_PIXELS: GlyphBBoxMode = 3;

const HB_DIRECTION_LTR: libc::c_uint = 4;

type FtError = libc::c_int;
type FtByte = libc::c_uchar;
type FtF26Dot6 = libc::c_long;
type FtPos = libc::c_long;
type FtFixed = libc::c_long;
type FtGlyphFormat = libc::c_uint;
type HbDirection = libc::c_uint;
type HbTag = libc::uint32_t;
type HbBool = libc::c_int;
type GlyphBBoxMode = libc::c_uint;
type FtKerningMode = libc::c_uint;
type FtGenericFinalizer = extern fn(*mut libc::c_void);

enum FtLibrary {}
enum FtCharMap {}
enum FtSizeInternal {}
enum FtSlotInternal {}
enum FtFaceInternal {}
#[derive(Debug)]
enum FtSubGlyph {}
enum FtListNode {}
enum FtDriver {}
enum FtMemory {}
enum FtStream {}
enum HbBuffer {}
enum HbFont {}

#[repr(C)]
#[derive(Debug)]
struct FtBitmapSize {
    height: libc::c_short,
    width: libc::c_short,

    size: FtPos,

    x_ppem: FtPos,
    y_ppem: FtPos,
}

#[repr(C)]
struct FtSize {
    face: *mut FtFace,
    generic: FtGeneric,
    metrics: FtSizeMetrics,
    internal: *mut FtSizeInternal,
}

#[repr(C)]
#[derive(Debug)]
struct FtSizeMetrics {
    x_ppem: libc::c_ushort,
    y_ppem: libc::c_ushort,

    x_scale: FtFixed,
    y_scale: FtFixed,

    ascender: FtPos,
    descender: FtPos,
    height: FtPos,
    max_advance: FtPos
}

#[repr(C)]
#[derive(Debug)]
struct FtGeneric {
    data: *mut libc::c_void,
    finalizer: FtGenericFinalizer,
}

#[repr(C)]
#[derive(Debug)]
struct FtVector {
    x: FtPos,
    y: FtPos,
}

#[repr(C)]
#[derive(Debug)]
struct FtBBox {
    x_min: FtPos,
    y_min: FtPos,
    x_max: FtPos,
    y_max: FtPos,
}

#[repr(C)]
#[derive(Debug)]
struct FtBitmap {
    rows: libc::c_int,
    width: libc::c_int,
    pitch: libc::c_int,
    buffer: *mut libc::c_uchar,
    num_grays: libc::c_short,
    pixel_mode: libc::c_char,
    palette_mode: libc::c_char,
    palette: *mut libc::c_void,
}

#[repr(C)]
#[derive(Debug)]
struct FtGlyphMetrics {
    width: FtPos,
    height: FtPos,

    hori_bearing_x: FtPos,
    hori_bearing_y: FtPos,
    hori_advance: FtPos,

    vert_bearing_x: FtPos,
    vert_bearing_y: FtPos,
    vert_advance: FtPos,
}

#[repr(C)]
#[derive(Debug)]
struct FtOutline {
    n_contours: libc::c_short,
    n_points: libc::c_short,

    points: *mut FtVector,
    tags: *mut libc::c_char,
    contours: *mut libc::c_short,

    flags: libc::c_int,
}

#[repr(C)]
#[derive(Debug)]
struct FtGlyphSlot {
    library: *mut FtLibrary,
    face: *mut FtFace,
    next: *mut FtGlyphSlot,
    reserved: libc::c_uint,
    generic: FtGeneric,

    metrics: FtGlyphMetrics,
    linear_hori_advance: FtFixed,
    linear_vert_advance: FtFixed,
    advance: FtVector,

    format: FtGlyphFormat,

    bitmap: FtBitmap,
    bitmap_left: libc::c_int,
    bitmap_top: libc::c_int,

    outline: FtOutline,

    num_subglyphs: libc::c_uint,
    subglyphs: FtSubGlyph,

    control_data: *mut libc::c_void,
    control_len: libc::c_long,

    lsb_delta: FtPos,
    rsb_delta: FtPos,

    other: *mut libc::c_void,

    internal: *mut FtSlotInternal,
}

#[repr(C)]
#[derive(Debug)]
struct FtGlyph {
    library: *mut FtLibrary,
    class: *const libc::c_void,
    format: FtGlyphFormat,
    advance: FtVector,
}

#[repr(C)]
#[derive(Debug)]
struct FtList {
    head: *mut FtListNode,
    tail: *mut FtListNode,
}

#[repr(C)]
#[derive(Debug)]
struct FtFace {
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

    glyph: *mut FtGlyphSlot,
    size: *mut FtSize,
    charmap: *mut FtCharMap,

    driver: *mut FtDriver,
    memory: *mut FtMemory,
    stream: *mut FtStream,

    sizes_list: FtList,

    autohint: FtGeneric,
    extensions: *mut libc::c_void,

    internal: *mut FtFaceInternal,
}

#[repr(C)]
#[derive(Debug)]
struct HbGlyphInfo {
    codepoint: u32,
    mask: u32,
    cluster: u32,
    var1: u32,
    var2: u32,
}

#[repr(C)]
#[derive(Debug)]
struct HbGlyphPosition {
    x_advance: i32,
    y_advance: i32,
    x_offset: i32,
    y_offset: i32,
    var: u32,
}

#[repr(C)]
struct HbFeature {
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

#[link(name="freetype")]
#[link(name="harfbuzz")]
extern {
    fn FT_Init_FreeType(lib: *mut *mut FtLibrary) -> FtError;
    fn FT_Done_FreeType(lib: *mut FtLibrary) -> FtError;
    fn FT_New_Face(lib: *mut FtLibrary, path: *const libc::c_char, idx: libc::c_long, face: *mut *mut FtFace) -> FtError;
    fn FT_New_Memory_Face(lib: *mut FtLibrary, buf: *const FtByte, len: libc::c_long, idx: libc::c_long, face: *mut *mut FtFace) -> FtError;
    fn FT_Done_Face(face: *mut FtFace) -> FtError;
    fn FT_Set_Char_Size(face: *mut FtFace, sx: FtF26Dot6, sy: FtF26Dot6, rx: libc::c_uint, ry: libc::c_uint) -> FtError;
    fn FT_Load_Glyph(face: *const FtFace, idx: libc::c_uint, flags: i32) -> FtError;
    fn FT_Load_Char(face: *const FtFace, code: libc::c_ulong, flags: i32) -> FtError;
    fn FT_Glyph_Get_CBox(glyph: *mut FtGlyph, bbox_mode: GlyphBBoxMode, acbox: *mut FtBBox);
    fn FT_Get_Char_Index(face: *const FtFace, code: libc::c_ulong) -> libc::c_uint;
    fn FT_Get_Kerning(face: *const FtFace, l_glyph: libc::c_uint, r_glyph: libc::c_uint, kern_mode: FtKerningMode, kerning: *mut FtVector) -> FtError;
    fn hb_ft_font_create(face: *mut FtFace, destroy: *const libc::c_void) -> *mut HbFont;
    fn hb_font_destroy(font: *mut HbFont);
    fn hb_buffer_create() -> *mut HbBuffer;
    fn hb_buffer_destroy(buf: *mut HbBuffer);
    fn hb_buffer_add_utf8(buf: *mut HbBuffer, txt: *const libc::c_char, len: libc::c_int, offset: libc::c_uint, ilen: libc::c_int);
    fn hb_buffer_set_direction(buf: *mut HbBuffer, dir: HbDirection);
    fn hb_buffer_guess_segment_properties(buf: *mut HbBuffer);
    fn hb_shape(font: *mut HbFont, buf: *mut HbBuffer, features: *const HbFeature, features_count: libc::c_uint);
    fn hb_feature_from_string(abbr: *const libc::c_char, len: libc::c_int, feature: *mut HbFeature) -> HbBool;
    fn hb_buffer_get_length(buf: *mut HbBuffer) -> libc::c_uint;
    fn hb_buffer_get_glyph_infos(buf: *mut HbBuffer, len: *mut libc::c_uint) -> *mut HbGlyphInfo;
    fn hb_buffer_get_glyph_positions(buf: *mut HbBuffer, len: *mut libc::c_uint) -> *mut HbGlyphPosition;
}


pub struct FontLibrary(*mut FtLibrary);

pub struct FontOpener(Rc<FontLibrary>);

pub struct Font {
    lib: Rc<FontLibrary>,
    face: *mut FtFace,
    font: *mut HbFont,
    // used as truncation mark
    ellipsis: RenderPlan,
    // lowercase and uppercase x heights
    pub x_heights: (u32, u32),
    space_codepoint: u32,
}

impl RenderPlan {
    pub fn space_out(&mut self, letter_spacing: u32) {
        if let Some((_, start)) = self.glyphs.split_last_mut() {
            let len = start.len() as u32;
            for glyph in start {
                glyph.advance.x += letter_spacing as i32;
            }
            self.width += len * letter_spacing;
        }
    }
}

impl FontOpener {
    pub fn new() -> Option<FontOpener> {
        unsafe {
            let mut lib = ptr::null_mut();
            let ret = FT_Init_FreeType(&mut lib);
            if ret != FT_ERR_OK {
                None
            } else {
                Some(FontOpener(Rc::new(FontLibrary(lib))))
            }
        }
    }

    pub fn open<P: AsRef<Path>>(&self, path: P) -> Option<Font> {
        unsafe {
            let mut face = ptr::null_mut();
            let c_path = CString::new(path.as_ref().as_os_str().as_bytes()).unwrap();
            let ret = FT_New_Face((self.0).0, c_path.as_ptr(), 0, &mut face);
            if ret != FT_ERR_OK {
               return None;
            }
            let font = ptr::null_mut();
            let ellipsis = RenderPlan::default();
            let x_heights = (0, 0);
            let space_codepoint = FT_Get_Char_Index(face, ' ' as libc::c_ulong);
            Some(Font { lib: self.0.clone(), face, font, ellipsis, x_heights, space_codepoint })
        }
    }

    pub fn from_bytes(&self, buf: &[u8]) -> Option<Font> {
        unsafe {
            let mut face = ptr::null_mut();
            let ret = FT_New_Memory_Face((self.0).0, buf.as_ptr() as *const FtByte, buf.len() as libc::c_long, 0, &mut face);
            if ret != FT_ERR_OK {
               return None;
            }
            let ellipsis = RenderPlan::default();
            let font = ptr::null_mut();
            let x_heights = (0, 0);
            let space_codepoint = FT_Get_Char_Index(face, ' ' as libc::c_ulong);
            Some(Font { lib: self.0.clone(), face, font, ellipsis, x_heights, space_codepoint })
        }
    }
}

impl Font {
    pub fn set_size(&mut self, size: u32, dpi: u16) {
        unsafe {
            if !self.font.is_null() {
                hb_font_destroy(self.font);
            }
            FT_Set_Char_Size(self.face, size as FtF26Dot6, 0, dpi as libc::c_uint, 0);
            self.font = hb_ft_font_create(self.face, ptr::null());
            self.ellipsis = self.plan("…", None, None);
            self.x_heights = (self.height('x'), self.height('X'));
        }
    }

    pub fn plan(&mut self, txt: &str, max_width: Option<u32>, features: Option<&str>) -> RenderPlan {
        unsafe {
            let buf = hb_buffer_create();
            hb_buffer_add_utf8(buf,
                               txt.as_ptr() as *const libc::c_char,
                               txt.len() as libc::c_int,
                               0,
                               -1);
            hb_buffer_set_direction(buf, HB_DIRECTION_LTR);
            hb_buffer_guess_segment_properties(buf);

            let features_vec = if let Some(features_txt) = features {
                features_txt.split(' ')
                    .filter_map(|f| {
                        let mut feature = HbFeature::default();
                        let ret = hb_feature_from_string(f.as_ptr() as *const libc::c_char, f.len() as libc::c_int, &mut feature);
                        if ret == 1 {
                            Some(feature)
                        } else {
                            None
                        }
                    }).collect()
            } else {
                vec![]
            };

            hb_shape(self.font, buf, features_vec.as_ptr(), features_vec.len() as libc::c_uint);
 
            let len = hb_buffer_get_length(buf);
            let info = hb_buffer_get_glyph_infos(buf, ptr::null_mut());
            let pos = hb_buffer_get_glyph_positions(buf, ptr::null_mut());
            let mut render_plan = RenderPlan::default();

            for i in 0..len {
                let pos_i = &*pos.offset(i as isize);
                let info_i = &*info.offset(i as isize);
                render_plan.width += (pos_i.x_advance >> 6) as u32;
                let glyph = GlyphPlan {
                    codepoint: info_i.codepoint,
                    advance: pt!(pos_i.x_advance >> 6, pos_i.y_advance >> 6),
                    offset: pt!(pos_i.x_offset >> 6, -pos_i.y_offset >> 6),
                };
                render_plan.glyphs.push(glyph);
            }

            if let Some(mw) = max_width {
                self.crop(&mut render_plan, mw);
            }

            hb_buffer_destroy(buf);
            render_plan
        }
    }

    #[inline]
    pub fn crop(&self, render_plan: &mut RenderPlan, max_width: u32) {
        if render_plan.width <= max_width {
            return;
        }
        render_plan.width += self.ellipsis.width;
        while let Some(gp) = render_plan.glyphs.pop() {
            render_plan.width -= gp.advance.x as u32;
            if render_plan.width <= max_width {
                break;
            }
        }
        render_plan.glyphs.extend_from_slice(&self.ellipsis.glyphs[..]);
    }

    pub fn last_word_before(&self, render_plan: &RenderPlan, max_width: u32) -> (usize, u32) {
        let mut width = render_plan.width;
        let glyphs = &render_plan.glyphs;
        let mut i = glyphs.len() - 1;
        while i > 0 && (width > max_width || glyphs[i].codepoint != self.space_codepoint) {
            width -= glyphs[i].advance.x as u32;
            i -= 1;
        }
        if i > 0 {
            width -= glyphs[i].advance.x as u32;
            i -= 1;
        }
        (i, width)
    }

    pub fn render(&mut self, fb: &mut Framebuffer, color: u8, render_plan: &RenderPlan, origin: &Point) {
        unsafe {
            let mut pos = *origin;
            for glyph in &render_plan.glyphs {
                FT_Load_Glyph(self.face, glyph.codepoint, FT_LOAD_RENDER | FT_LOAD_NO_HINTING);
                // FT_Load_Glyph(self.face, glyph.codepoint, FT_LOAD_RENDER);
                let glyph_slot = (*self.face).glyph;
                let top_left = pos + glyph.offset + pt!((*glyph_slot).bitmap_left, -(*glyph_slot).bitmap_top);
                let bitmap = &(*glyph_slot).bitmap;
                for y in 0..bitmap.rows {
                    for x in 0..bitmap.width {
                        let blackness = *bitmap.buffer.offset((bitmap.pitch * y + x) as isize);
                        let alpha = blackness as f32 / 255.0;
                        let pt = top_left + pt!(x, y);
                        fb.set_blended_pixel(pt.x as u32, pt.y as u32, color, alpha);
                    }
                }
                pos += glyph.advance;
            }
        }
    }

    pub fn height(&self, c: char) -> u32 {
        unsafe {
            FT_Load_Char(self.face, c as libc::c_ulong, FT_LOAD_DEFAULT);
            let metrics = &((*(*self.face).glyph).metrics);
            (metrics.height >> 6) as u32
        }
    }

    pub fn em(&self) -> u16 {
        unsafe {
            ((*(*self.face).size).metrics).x_ppem
        }
    }
}

#[derive(Debug, Copy, Clone)]
pub struct GlyphPlan {
    codepoint: u32,
    offset: Point,
    advance: Point,
}

#[derive(Debug)]
pub struct RenderPlan {
    pub width: u32,
    glyphs: Vec<GlyphPlan>,
}

impl Default for RenderPlan {
    fn default() -> RenderPlan {
        RenderPlan {
            width: 0,
            glyphs: vec![],
        }
    }
}

impl Drop for FontLibrary {
    fn drop(&mut self) {
        unsafe { FT_Done_FreeType(self.0); }
    }
}

impl Drop for Font {
    fn drop(&mut self) {
        unsafe { 
            FT_Done_Face(self.face);
            if !self.font.is_null() {
                hb_font_destroy(self.font);
            }
        }
    }
}
