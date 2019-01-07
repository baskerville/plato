mod harfbuzz_sys;
mod freetype_sys;

use crate::font::harfbuzz_sys::*;
use crate::font::freetype_sys::*;

use std::str;
use std::ptr;
use std::slice;
use std::ffi::{CString, CStr};
use std::os::unix::ffi::OsStrExt;
use std::path::Path;
use std::collections::BTreeSet;
use std::rc::Rc;
use fnv::FnvHashMap;
use bitflags::bitflags;
use failure::{Error, Fail, format_err};
use glob::glob;
use crate::geom::Point;
use crate::framebuffer::Framebuffer;

// Font sizes in 1/64th of a point
pub const FONT_SIZES: [u32; 3] = [349, 524, 629];

pub const KEYBOARD_FONT_SIZES: [u32; 2] = [337, 843];

pub const DISPLAY_FONT_SIZE: u32 = 2516;

pub const NORMAL_STYLE: Style = Style {
    family: Family::SansSerif,
    variant: Variant::REGULAR,
    size: FONT_SIZES[1],
};

pub const KBD_CHAR: Style = Style {
    family: Family::Keyboard,
    variant: Variant::REGULAR,
    size: KEYBOARD_FONT_SIZES[1],
};

pub const KBD_LABEL: Style = Style {
    family: Family::Keyboard,
    variant: Variant::REGULAR,
    size: FONT_SIZES[0],
};

pub const DISPLAY_STYLE: Style = Style {
    family: Family::Display,
    variant: Variant::REGULAR,
    size: DISPLAY_FONT_SIZE,
};

pub const MD_TITLE: Style = Style {
    family: Family::Serif,
    variant: Variant::ITALIC,
    size: FONT_SIZES[2],
};

pub const MD_AUTHOR: Style = Style {
    family: Family::Serif,
    variant: Variant::REGULAR,
    size: FONT_SIZES[1],
};

pub const MD_YEAR: Style = NORMAL_STYLE;

pub const MD_KIND: Style = Style {
    family: Family::SansSerif,
    variant: Variant::BOLD,
    size: FONT_SIZES[0],
};

pub const MD_SIZE: Style = Style {
    family: Family::SansSerif,
    variant: Variant::REGULAR,
    size: FONT_SIZES[0],
};

pub const SLIDER_VALUE: Style = MD_SIZE;

const CATEGORY_DEPTH_LIMIT: usize = 5;

pub fn category_font_size(depth: usize) -> u32 {
    let k = (2.0 / 3.0f32).powf(CATEGORY_DEPTH_LIMIT.min(depth) as f32 /
                                CATEGORY_DEPTH_LIMIT as f32);
    (k * FONT_SIZES[1] as f32) as u32
}

pub struct FontFamily {
    pub regular: Font,
    pub italic: Font,
    pub bold: Font,
    pub bold_italic: Font,
}

pub fn family_names<P: AsRef<Path>>(search_path: P) -> Result<BTreeSet<String>, Error> {
    let opener = FontOpener::new()?;
    let end_path = Path::new("**").join("*.[ot]tf");
    let pattern_path = search_path.as_ref().join(&end_path);
    let pattern = pattern_path.to_str().unwrap_or_default();

    let mut families = BTreeSet::new();

    for path in glob(pattern)?.filter_map(Result::ok) {
        let font = opener.open(&path)?;
        if let Some(family_name) = font.family_name() {
            families.insert(family_name.to_string());
        }
    }

    Ok(families)
}

impl FontFamily {
    pub fn from_name<P: AsRef<Path>>(family_name: &str, search_path: P) -> Result<FontFamily, Error> {
        let opener = FontOpener::new()?;
        let end_path = Path::new("**").join("*.[ot]tf");
        let pattern_path = search_path.as_ref().join(&end_path);
        let pattern = pattern_path.to_str().unwrap_or_default();

        let mut styles = FnvHashMap::default();

        for path in glob(pattern)?.filter_map(Result::ok) {
            let font = opener.open(&path)?;
            if font.family_name() == Some(family_name) {
                styles.insert(font.style_name().map(String::from)
                                  .unwrap_or_else(|| "Regular".to_string()),
                              path.clone());
            }
        }

        let regular_path = styles.get("Regular")
                                 .or_else(|| styles.get("Roman"))
                                 .or_else(|| styles.get("Book"))
                                 .ok_or_else(|| format_err!("Can't find regular style."))?;
        let italic_path = styles.get("Italic")
                                .or_else(|| styles.get("Book Italic"))
                                .unwrap_or(regular_path);
        let bold_path = styles.get("Bold")
                              .or_else(|| styles.get("Semibold"))
                              .or_else(|| styles.get("SemiBold"))
                              .or_else(|| styles.get("Medium"))
                              .unwrap_or(regular_path);
        let bold_italic_path = styles.get("Bold Italic")
                                     .or_else(|| styles.get("SemiBold Italic"))
                                     .or_else(|| styles.get("Medium Italic"))
                                     .unwrap_or(italic_path);
        Ok(FontFamily {
            regular: opener.open(regular_path)?,
            italic: opener.open(italic_path)?,
            bold: opener.open(bold_path)?,
            bold_italic: opener.open(bold_italic_path)?,
        })
    }
}

pub struct Fonts {
    pub sans_serif: FontFamily,
    pub serif: FontFamily,
    pub monospace: FontFamily,
    pub keyboard: Font,
    pub display: Font,
}

impl Fonts {
    pub fn load() -> Result<Fonts, Error> {
        let opener = FontOpener::new()?;
        let mut fonts = Fonts {
            sans_serif: FontFamily {
                regular: opener.open("fonts/NotoSans-Regular.ttf")?,
                italic: opener.open("fonts/NotoSans-Italic.ttf")?,
                bold: opener.open("fonts/NotoSans-Bold.ttf")?,
                bold_italic: opener.open("fonts/NotoSans-BoldItalic.ttf")?,
            },
            serif: FontFamily {
                regular: opener.open("fonts/NotoSerif-Regular.ttf")?,
                italic: opener.open("fonts/NotoSerif-Italic.ttf")?,
                bold: opener.open("fonts/NotoSerif-Bold.ttf")?,
                bold_italic: opener.open("fonts/NotoSerif-BoldItalic.ttf")?,
            },
            monospace: FontFamily {
                regular: opener.open("fonts/SourceCodeVariable-Roman.otf")?,
                italic: opener.open("fonts/SourceCodeVariable-Italic.otf")?,
                bold: opener.open("fonts/SourceCodeVariable-Roman.otf")?,
                bold_italic: opener.open("fonts/SourceCodeVariable-Italic.otf")?,
            },
            keyboard: opener.open("fonts/VarelaRound-Regular.ttf")?,
            display: opener.open("fonts/Cormorant-Regular.ttf")?,
        };
        fonts.monospace.bold.set_variations(&["wght=600"]);
        fonts.monospace.bold_italic.set_variations(&["wght=600"]);
        Ok(fonts)
    }
}

bitflags! {
    pub struct Variant: u8 {
        const REGULAR = 0;
        const ITALIC = 1;
        const BOLD = 2;
    }
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum Family {
    SansSerif,
    Serif,
    Monospace,
    Keyboard,
    Display,
}

pub struct Style {
    family: Family,
    variant: Variant,
    pub size: u32,
}

pub fn font_from_variant(family: &mut FontFamily, variant: Variant) -> &mut Font {
    if variant.contains(Variant::ITALIC | Variant::BOLD) {
        &mut family.bold_italic
    } else if variant.contains(Variant::ITALIC) {
        &mut family.italic
    } else if variant.contains(Variant::BOLD) {
        &mut family.bold
    } else {
        &mut family.regular
    }
}

pub fn font_from_style<'a>(fonts: &'a mut Fonts, style: &Style, dpi: u16) -> &'a mut Font {
    let font = match style.family {
        Family::SansSerif => {
            let family = &mut fonts.sans_serif;
            font_from_variant(family, style.variant)
        },
        Family::Serif => {
            let family = &mut fonts.serif;
            font_from_variant(family, style.variant)
        },
        Family::Monospace => {
            let family = &mut fonts.monospace;
            font_from_variant(family, style.variant)
        },
        Family::Keyboard => &mut fonts.keyboard,
        Family::Display => &mut fonts.display,
    };
    font.set_size(style.size, dpi);
    font
}

pub struct FontLibrary(*mut FtLibrary);

pub struct FontOpener(Rc<FontLibrary>);

pub struct Font {
    lib: Rc<FontLibrary>,
    face: *mut FtFace,
    font: *mut HbFont,
    size: u32,
    dpi: u16,
    // used as truncation mark
    pub ellipsis: RenderPlan,
    // lowercase and uppercase x heights
    pub x_heights: (u32, u32),
    space_codepoint: u32,
}

impl RenderPlan {
    pub fn space_out(&mut self, letter_spacing: u32) {
        if letter_spacing == 0 {
            return;
        }

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
    pub fn new() -> Result<FontOpener, Error> {
        unsafe {
            let mut lib = ptr::null_mut();
            let ret = FT_Init_FreeType(&mut lib);
            if ret != FT_ERR_OK {
                Err(Error::from(FreetypeError::from(ret)))
            } else {
                Ok(FontOpener(Rc::new(FontLibrary(lib))))
            }
        }
    }

    pub fn open<P: AsRef<Path>>(&self, path: P) -> Result<Font, Error> {
        unsafe {
            let mut face = ptr::null_mut();
            let c_path = CString::new(path.as_ref().as_os_str().as_bytes()).unwrap();
            let ret = FT_New_Face((self.0).0, c_path.as_ptr(), 0, &mut face);
            if ret != FT_ERR_OK {
               return Err(Error::from(FreetypeError::from(ret)));
            }
            let font = ptr::null_mut();
            let ellipsis = RenderPlan::default();
            let x_heights = (0, 0);
            let space_codepoint = FT_Get_Char_Index(face, ' ' as libc::c_ulong);
            Ok(Font { lib: self.0.clone(), face, font,
                      size: 0, dpi: 0, ellipsis, x_heights, space_codepoint })
        }
    }

    pub fn open_memory(&self, buf: &[u8]) -> Result<Font, Error> {
        unsafe {
            let mut face = ptr::null_mut();
            let ret = FT_New_Memory_Face((self.0).0, buf.as_ptr() as *const FtByte, buf.len() as libc::c_long, 0, &mut face);
            if ret != FT_ERR_OK {
               return Err(Error::from(FreetypeError::from(ret)));
            }
            let ellipsis = RenderPlan::default();
            let font = ptr::null_mut();
            let x_heights = (0, 0);
            let space_codepoint = FT_Get_Char_Index(face, ' ' as libc::c_ulong);
            Ok(Font { lib: self.0.clone(), face, font,
                      size: 0, dpi: 0, ellipsis, x_heights, space_codepoint })
        }
    }
}

impl Font {
    pub fn family_name(&self) -> Option<&str> {
        unsafe {
            let ptr = (*self.face).family_name;
            if ptr.is_null() {
                return None;
            }
            CStr::from_ptr(ptr).to_str().ok()
        }
    }

    pub fn style_name(&self) -> Option<&str> {
        unsafe {
            let ptr = (*self.face).style_name;
            if ptr.is_null() {
                return None;
            }
            CStr::from_ptr(ptr).to_str().ok()
        }
    }

    pub fn set_size(&mut self, size: u32, dpi: u16) {
        if !self.font.is_null() && self.size == size && self.dpi == dpi {
            return;
        }

        self.size = size;
        self.dpi = dpi;

        unsafe {
            let ret = FT_Set_Char_Size(self.face, size as FtF26Dot6, 0, dpi as libc::c_uint, 0);

            if ret != FT_ERR_OK {
                return;
            }

            if self.font.is_null() {
                self.font = hb_ft_font_create(self.face, ptr::null());
            } else {
                hb_ft_font_changed(self.font);
            }

            self.ellipsis = self.plan("…", None, None);
            self.x_heights = (self.height('x'), self.height('X'));
        }
    }

    pub fn set_variations(&mut self, specs: &[&str]) {
        unsafe {
            let mut varia = ptr::null_mut();
            let ret = FT_Get_MM_Var(self.face, &mut varia);

            if ret != FT_ERR_OK {
                return;
            }

            let axes_count = (*varia).num_axis as usize;
            let mut coords = Vec::with_capacity(axes_count);

            for i in 0..(axes_count as isize) {
                let axis = ((*varia).axis).offset(i);
                coords.push((*axis).def);
            }

            for s in specs {
                let tn = s[..4].as_bytes();
                let tag = tag(tn[0], tn[1], tn[2], tn[3]);
                let value: f32 = s[5..].parse().unwrap_or_default();

                for i in 0..(axes_count as isize) {
                    let axis = ((*varia).axis).offset(i);

                    if (*axis).tag == tag as libc::c_ulong {
                        let scaled_value = ((value * 65536.0) as FtFixed).min((*axis).maximum)
                                                                         .max((*axis).minimum);
                        *coords.get_unchecked_mut(i as usize) = scaled_value;
                        break;
                    }
                }
            }

            let ret = FT_Set_Var_Design_Coordinates(self.face, coords.len() as libc::c_uint, coords.as_ptr());

            if ret == FT_ERR_OK && !self.font.is_null() {
                hb_ft_font_changed(self.font);
            }

            FT_Done_MM_Var(self.lib.0, varia);
        }
    }

    pub fn set_variations_from_name(&mut self, name: &str) -> bool {
        let mut found = false;

        unsafe {
            let mut varia = ptr::null_mut();
            let ret = FT_Get_MM_Var(self.face, &mut varia);

            if ret != FT_ERR_OK {
                return found;
            }

            let styles_count = (*varia).num_namedstyles as isize;
            let names_count = FT_Get_Sfnt_Name_Count(self.face);
            let mut sfnt_name = FtSfntName::default();

            'outer: for i in 0..styles_count {
                let style = ((*varia).namedstyle).offset(i);
                let strid = (*style).strid as libc::c_ushort;
                for j in 0..names_count {
                    let ret = FT_Get_Sfnt_Name(self.face, j, &mut sfnt_name);

                    if ret != FT_ERR_OK || sfnt_name.name_id != strid {
                        continue;
                    }

                    if sfnt_name.platform_id != TT_PLATFORM_MICROSOFT ||
                       sfnt_name.encoding_id != TT_MS_ID_UNICODE_CS ||
                       sfnt_name.language_id != TT_MS_LANGID_ENGLISH_UNITED_STATES {
                        continue;
                    }

                    let slice = slice::from_raw_parts(sfnt_name.text, sfnt_name.len as usize);
                    // We're assuming ASCII encoded as UTF_16BE
                    let vec_ascii: Vec<u8> = slice.iter().enumerate().filter_map(|x| {
                        if x.0 % 2 == 0 { None } else { Some(*x.1) }
                    }).collect();

                    if let Ok(name_str) = str::from_utf8(&vec_ascii[..]) {
                        if name.eq_ignore_ascii_case(name_str) {
                            found = true;
                            let ret = FT_Set_Var_Design_Coordinates(self.face, (*varia).num_axis, (*style).coords);
                            if ret == FT_ERR_OK && !self.font.is_null() {
                                hb_ft_font_changed(self.font);
                            }
                            break 'outer;
                        }
                    }
                }
            }

            FT_Done_MM_Var(self.lib.0, varia);
        }

        found
    }

    pub fn plan(&mut self, txt: &str, max_width: Option<u32>, features: Option<&[String]>) -> RenderPlan {
        unsafe {
            let buf = hb_buffer_create();
            hb_buffer_add_utf8(buf,
                               txt.as_ptr() as *const libc::c_char,
                               txt.len() as libc::c_int,
                               0,
                               -1);
            hb_buffer_set_direction(buf, HB_DIRECTION_LTR);
            hb_buffer_guess_segment_properties(buf);

            let features_vec: Vec<HbFeature> = features.map(|ftr|
                ftr.iter().filter_map(|f| {
                    let mut feature = HbFeature::default();
                    let ret = hb_feature_from_string(f.as_ptr() as *const libc::c_char,
                                                     f.len() as libc::c_int,
                                                     &mut feature);
                    if ret == 1 {
                        Some(feature)
                    } else {
                        None
                    }
                }).collect()
            ).unwrap_or_default();

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
                self.crop_right(&mut render_plan, mw);
            }

            hb_buffer_destroy(buf);
            render_plan
        }
    }

    #[inline]
    pub fn crop_right(&self, render_plan: &mut RenderPlan, max_width: u32) {
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

    #[inline]
    pub fn crop_around(&self, render_plan: &mut RenderPlan, index: usize, max_width: u32) -> usize {
        if render_plan.width <= max_width {
            return 0;
        }

        let len = render_plan.glyphs.len();
        let mut width = 0;
        let mut polarity = 0;
        let mut upper_index = index;
        let mut lower_index = index as i32 - 1;

        loop {
            let next_width;
            if upper_index < len && (polarity % 2 == 0 || lower_index < 0) {
                next_width = width + render_plan.glyphs[upper_index].advance.x as u32;
                if next_width > max_width {
                    break;
                } else {
                    width = next_width;
                }
                upper_index += 1;
            } else if lower_index >= 0 && (polarity % 2 == 1 || upper_index == len) {
                next_width = width + render_plan.glyphs[lower_index as usize].advance.x as u32;
                if next_width > max_width {
                    break;
                } else {
                    width = next_width;
                }
                lower_index -= 1;
            }
            polarity += 1;
        }

        if upper_index < len {
            width += self.ellipsis.width;
            upper_index -= 1;
            while width > max_width && upper_index > (lower_index.max(0) as usize) {
                width -= render_plan.glyphs[upper_index].advance.x as u32;
                upper_index -= 1;
            }
            render_plan.glyphs.truncate(upper_index + 1);
            render_plan.glyphs.extend_from_slice(&self.ellipsis.glyphs[..]);
        }

        if lower_index >= 0 {
            width += self.ellipsis.width;
            lower_index += 1;
            while width > max_width && (lower_index as usize) < upper_index  {
                width -= render_plan.glyphs[lower_index as usize].advance.x as u32;
                lower_index += 1;
            }
            render_plan.glyphs = self.ellipsis.glyphs.iter()
                                 .chain(render_plan.glyphs[lower_index as usize..].iter()).cloned().collect();
        }

        render_plan.width = width;

        if lower_index < 0 {
            0
        } else {
            lower_index as usize
        }
    }

    pub fn cut_point(&self, render_plan: &RenderPlan, max_width: u32) -> (usize, u32) {
        let mut width = render_plan.width;
        let glyphs = &render_plan.glyphs;
        let mut i = glyphs.len() - 1;

        width -= glyphs[i].advance.x as u32;

        while i > 0 && width > max_width {
            i -= 1;
            width -= glyphs[i].advance.x as u32;
        }

        let j = i;
        let last_width = width;

        while i > 0 && glyphs[i].codepoint != self.space_codepoint {
            i -= 1;
            width -= glyphs[i].advance.x as u32;
        }

        if i == 0 {
            i = j;
            width = last_width;
        }

        (i, width)
    }

    pub fn render(&mut self, fb: &mut Framebuffer, color: u8, render_plan: &RenderPlan, origin: Point) {
        unsafe {
            let mut pos = origin;
            for glyph in &render_plan.glyphs {
                FT_Load_Glyph(self.face, glyph.codepoint, FT_LOAD_RENDER | FT_LOAD_NO_HINTING);
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
            (*(*self.face).size).metrics.x_ppem as u16
        }
    }

    pub fn ascender(&self) -> i32 {
        unsafe {
            (*(*self.face).size).metrics.ascender as i32 / 64
        }
    }

    pub fn descender(&self) -> i32 {
        unsafe {
            (*(*self.face).size).metrics.descender as i32 / 64
        }
    }

    pub fn line_height(&self) -> i32 {
        unsafe {
            (*(*self.face).size).metrics.height as i32 / 64
        }
    }
}

#[derive(Debug, Copy, Clone)]
pub struct GlyphPlan {
    codepoint: u32,
    offset: Point,
    advance: Point,
}

#[derive(Debug, Clone)]
pub struct RenderPlan {
    pub width: u32,
    glyphs: Vec<GlyphPlan>,
}

impl RenderPlan {
    pub fn split_off(&mut self, index: usize, width: u32) -> RenderPlan {
        let next_width = self.width - width;
        let next_glyphs = self.glyphs.split_off(index);
        self.width = width;
        RenderPlan {
            width: next_width,
            glyphs: next_glyphs,
        }
    }

    pub fn index_from_advance(&self, advance: i32) -> usize {
        let mut sum = 0;
        let mut index = 0;
        while index < self.glyphs.len() {
            let gad = self.glyph_advance(index);
            sum += gad;
            if sum > advance {
                if sum - advance < advance - sum + gad {
                    index += 1;
                }
                break;
            }
            index += 1;
        }
        index
    }

    pub fn total_advance(&self, index: usize) -> i32 {
        self.glyphs.iter().take(index).map(|g| g.advance.x).sum()
    }

    #[inline]
    pub fn glyph_advance(&self, index: usize) -> i32 {
        self.glyphs[index].advance.x
    }
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

fn tag(c1: u8, c2: u8, c3: u8, c4: u8) -> u32 {
    ((c1 as u32) << 24) | ((c2 as u32) << 16) | ((c3 as u32) << 8) | c4 as u32
}

#[derive(Fail, Debug)]
enum FreetypeError {
    #[fail(display = "Unknown error with code {}.", _0)]
    UnknownError(FtError),

    #[fail(display = "Cannot open resource.")]
    CannotOpenResource,

    #[fail(display = "Unknown file format.")]
    UnknownFileFormat,

    #[fail(display = "Broken file.")]
    InvalidFileFormat,

    #[fail(display = "Invalid FreeType version.")]
    InvalidVersion,

    #[fail(display = "Module version is too low.")]
    LowerModuleVersion,

    #[fail(display = "Invalid argument.")]
    InvalidArgument,

    #[fail(display = "Unimplemented feature.")]
    UnimplementedFeature,

    #[fail(display = "Broken table.")]
    InvalidTable,

    #[fail(display = "Broken offset within table.")]
    InvalidOffset,

    #[fail(display = "Array allocation size too large.")]
    ArrayTooLarge,

    #[fail(display = "Missing module.")]
    MissingModule,

    #[fail(display = "Missing property.")]
    MissingProperty,

    #[fail(display = "Invalid glyph index.")]
    InvalidGlyphIndex,

    #[fail(display = "Invalid character code.")]
    InvalidCharacterCode,

    #[fail(display = "Unsupported glyph image format.")]
    InvalidGlyphFormat,

    #[fail(display = "Cannot render this glyph format.")]
    CannotRenderGlyph,

    #[fail(display = "Invalid outline.")]
    InvalidOutline,

    #[fail(display = "Invalid composite glyph.")]
    InvalidComposite,

    #[fail(display = "Too many hints.")]
    TooManyHints,

    #[fail(display = "Invalid pixel size.")]
    InvalidPixelSize,

    #[fail(display = "Invalid object handle.")]
    InvalidHandle,

    #[fail(display = "Invalid library handle.")]
    InvalidLibraryHandle,

    #[fail(display = "Invalid module handle.")]
    InvalidDriverHandle,

    #[fail(display = "Invalid face handle.")]
    InvalidFaceHandle,

    #[fail(display = "Invalid size handle.")]
    InvalidSizeHandle,

    #[fail(display = "Invalid glyph slot handle.")]
    InvalidSlotHandle,

    #[fail(display = "Invalid charmap handle.")]
    InvalidCharMapHandle,

    #[fail(display = "Invalid cache manager handle.")]
    InvalidCacheHandle,

    #[fail(display = "Invalid stream handle.")]
    InvalidStreamHandle,

    #[fail(display = "Too many modules.")]
    TooManyDrivers,

    #[fail(display = "Too many extensions.")]
    TooManyExtensions,

    #[fail(display = "Out of memory.")]
    OutOfMemory,

    #[fail(display = "Unlisted object.")]
    UnlistedObject,

    #[fail(display = "Cannot open stream.")]
    CannotOpenStream,

    #[fail(display = "Invalid stream seek.")]
    InvalidStreamSeek,

    #[fail(display = "Invalid stream skip.")]
    InvalidStreamSkip,

    #[fail(display = "Invalid stream read.")]
    InvalidStreamRead,

    #[fail(display = "Invalid stream operation.")]
    InvalidStreamOperation,

    #[fail(display = "Invalid frame operation.")]
    InvalidFrameOperation,

    #[fail(display = "Nested frame access.")]
    NestedFrameAccess,

    #[fail(display = "Invalid frame read.")]
    InvalidFrameRead,

    #[fail(display = "Raster uninitialized.")]
    RasterUninitialized,

    #[fail(display = "Raster corrupted.")]
    RasterCorrupted,

    #[fail(display = "Raster overflow.")]
    RasterOverflow,

    #[fail(display = "Negative height while rastering.")]
    RasterNegativeHeight,

    #[fail(display = "Too many registered caches.")]
    TooManyCaches,

    #[fail(display = "Invalid opcode.")]
    InvalidOpcode,

    #[fail(display = "Too few arguments.")]
    TooFewArguments,

    #[fail(display = "Stack overflow.")]
    StackOverflow,

    #[fail(display = "Code overflow.")]
    CodeOverflow,

    #[fail(display = "Bad argument.")]
    BadArgument,

    #[fail(display = "Division by zero.")]
    DivideByZero,

    #[fail(display = "Invalid reference.")]
    InvalidReference,

    #[fail(display = "Found debug opcode.")]
    DebugOpCode,

    #[fail(display = "Found ENDF opcode in execution stream.")]
    ENDFInExecStream,

    #[fail(display = "Nested DEFS.")]
    NestedDEFS,

    #[fail(display = "Invalid code range.")]
    InvalidCodeRange,

    #[fail(display = "Execution context too long.")]
    ExecutionTooLong,

    #[fail(display = "Too many function definitions.")]
    TooManyFunctionDefs,

    #[fail(display = "Too many instruction definitions.")]
    TooManyInstructionDefs,

    #[fail(display = "SFNT font table missing.")]
    TableMissing,

    #[fail(display = "Horizontal header (hhea) table missing.")]
    HorizHeaderMissing,

    #[fail(display = "Locations (loca) table missing.")]
    LocationsMissing,

    #[fail(display = "Name table missing.")]
    NameTableMissing,

    #[fail(display = "Character map (cmap) table missing.")]
    CMapTableMissing,

    #[fail(display = "Horizontal metrics (hmtx) table missing.")]
    HmtxTableMissing,

    #[fail(display = "PostScript (post) table missing.")]
    PostTableMissing,

    #[fail(display = "Invalid horizontal metrics.")]
    InvalidHorizMetrics,

    #[fail(display = "Invalid character map (cmap) format.")]
    InvalidCharMapFormat,

    #[fail(display = "Invalid ppem value.")]
    InvalidPPem,

    #[fail(display = "Invalid vertical metrics.")]
    InvalidVertMetrics,

    #[fail(display = "Could not find context.")]
    CouldNotFindContext,

    #[fail(display = "Invalid PostScript (post) table format.")]
    InvalidPostTableFormat,

    #[fail(display = "Invalid PostScript (post) table.")]
    InvalidPostTable,

    #[fail(display = "Found FDEF or IDEF opcode in glyf bytecode.")]
    DEFInGlyfBytecode,

    #[fail(display = "Missing bitmap in strike.")]
    MissingBitmap,

    #[fail(display = "Opcode syntax error.")]
    SyntaxError,

    #[fail(display = "Argument stack underflow.")]
    StackUnderflow,

    #[fail(display = "Ignore.")]
    Ignore,

    #[fail(display = "No Unicode glyph name found.")]
    NoUnicodeGlyphName,

    #[fail(display = "Glyph too big for hinting.")]
    GlyphTooBig,

    #[fail(display = "`STARTFONT' field missing.")]
    MissingStartfontField,

    #[fail(display = "`FONT' field missing.")]
    MissingFontField,

    #[fail(display = "`SIZE' field missing.")]
    MissingSizeField,

    #[fail(display = "`FONTBOUNDINGBOX' field missing.")]
    MissingFontboundingboxField,

    #[fail(display = "`CHARS' field missing.")]
    MissingCharsField,

    #[fail(display = "`STARTCHAR' field missing.")]
    MissingStartcharField,

    #[fail(display = "`ENCODING' field missing.")]
    MissingEncodingField,

    #[fail(display = "`BBX' field missing.")]
    MissingBbxField,

    #[fail(display = "`BBX' too big.")]
    BbxTooBig,

    #[fail(display = "Font header corrupted or missing fields.")]
    CorruptedFontHeader,

    #[fail(display = "Font glyphs corrupted or missing fields.")]
    CorruptedFontGlyphs,
}

impl From<FtError> for FreetypeError {
    fn from(code: FtError) -> FreetypeError {
        match code {
            0x01 => FreetypeError::CannotOpenResource,
            0x02 => FreetypeError::UnknownFileFormat,
            0x03 => FreetypeError::InvalidFileFormat,
            0x04 => FreetypeError::InvalidVersion,
            0x05 => FreetypeError::LowerModuleVersion,
            0x06 => FreetypeError::InvalidArgument,
            0x07 => FreetypeError::UnimplementedFeature,
            0x08 => FreetypeError::InvalidTable,
            0x09 => FreetypeError::InvalidOffset,
            0x0A => FreetypeError::ArrayTooLarge,
            0x0B => FreetypeError::MissingModule,
            0x0C => FreetypeError::MissingProperty,
            0x10 => FreetypeError::InvalidGlyphIndex,
            0x11 => FreetypeError::InvalidCharacterCode,
            0x12 => FreetypeError::InvalidGlyphFormat,
            0x13 => FreetypeError::CannotRenderGlyph,
            0x14 => FreetypeError::InvalidOutline,
            0x15 => FreetypeError::InvalidComposite,
            0x16 => FreetypeError::TooManyHints,
            0x17 => FreetypeError::InvalidPixelSize,
            0x20 => FreetypeError::InvalidHandle,
            0x21 => FreetypeError::InvalidLibraryHandle,
            0x22 => FreetypeError::InvalidDriverHandle,
            0x23 => FreetypeError::InvalidFaceHandle,
            0x24 => FreetypeError::InvalidSizeHandle,
            0x25 => FreetypeError::InvalidSlotHandle,
            0x26 => FreetypeError::InvalidCharMapHandle,
            0x27 => FreetypeError::InvalidCacheHandle,
            0x28 => FreetypeError::InvalidStreamHandle,
            0x30 => FreetypeError::TooManyDrivers,
            0x31 => FreetypeError::TooManyExtensions,
            0x40 => FreetypeError::OutOfMemory,
            0x41 => FreetypeError::UnlistedObject,
            0x51 => FreetypeError::CannotOpenStream,
            0x52 => FreetypeError::InvalidStreamSeek,
            0x53 => FreetypeError::InvalidStreamSkip,
            0x54 => FreetypeError::InvalidStreamRead,
            0x55 => FreetypeError::InvalidStreamOperation,
            0x56 => FreetypeError::InvalidFrameOperation,
            0x57 => FreetypeError::NestedFrameAccess,
            0x58 => FreetypeError::InvalidFrameRead,
            0x60 => FreetypeError::RasterUninitialized,
            0x61 => FreetypeError::RasterCorrupted,
            0x62 => FreetypeError::RasterOverflow,
            0x63 => FreetypeError::RasterNegativeHeight,
            0x70 => FreetypeError::TooManyCaches,
            0x80 => FreetypeError::InvalidOpcode,
            0x81 => FreetypeError::TooFewArguments,
            0x82 => FreetypeError::StackOverflow,
            0x83 => FreetypeError::CodeOverflow,
            0x84 => FreetypeError::BadArgument,
            0x85 => FreetypeError::DivideByZero,
            0x86 => FreetypeError::InvalidReference,
            0x87 => FreetypeError::DebugOpCode,
            0x88 => FreetypeError::ENDFInExecStream,
            0x89 => FreetypeError::NestedDEFS,
            0x8A => FreetypeError::InvalidCodeRange,
            0x8B => FreetypeError::ExecutionTooLong,
            0x8C => FreetypeError::TooManyFunctionDefs,
            0x8D => FreetypeError::TooManyInstructionDefs,
            0x8E => FreetypeError::TableMissing,
            0x8F => FreetypeError::HorizHeaderMissing,
            0x90 => FreetypeError::LocationsMissing,
            0x91 => FreetypeError::NameTableMissing,
            0x92 => FreetypeError::CMapTableMissing,
            0x93 => FreetypeError::HmtxTableMissing,
            0x94 => FreetypeError::PostTableMissing,
            0x95 => FreetypeError::InvalidHorizMetrics,
            0x96 => FreetypeError::InvalidCharMapFormat,
            0x97 => FreetypeError::InvalidPPem,
            0x98 => FreetypeError::InvalidVertMetrics,
            0x99 => FreetypeError::CouldNotFindContext,
            0x9A => FreetypeError::InvalidPostTableFormat,
            0x9B => FreetypeError::InvalidPostTable,
            0x9C => FreetypeError::DEFInGlyfBytecode,
            0x9D => FreetypeError::MissingBitmap,
            0xA0 => FreetypeError::SyntaxError,
            0xA1 => FreetypeError::StackUnderflow,
            0xA2 => FreetypeError::Ignore,
            0xA3 => FreetypeError::NoUnicodeGlyphName,
            0xA4 => FreetypeError::GlyphTooBig,
            0xB0 => FreetypeError::MissingStartfontField,
            0xB1 => FreetypeError::MissingFontField,
            0xB2 => FreetypeError::MissingSizeField,
            0xB3 => FreetypeError::MissingFontboundingboxField,
            0xB4 => FreetypeError::MissingCharsField,
            0xB5 => FreetypeError::MissingStartcharField,
            0xB6 => FreetypeError::MissingEncodingField,
            0xB7 => FreetypeError::MissingBbxField,
            0xB8 => FreetypeError::BbxTooBig,
            0xB9 => FreetypeError::CorruptedFontHeader,
            0xBA => FreetypeError::CorruptedFontGlyphs,
            _ => FreetypeError::UnknownError(code),
        }
    }
}
