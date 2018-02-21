extern crate libc;

mod harfbuzz_sys;
mod freetype_sys;

use font::harfbuzz_sys::*;
use font::freetype_sys::*;

use std::ptr;
use std::ffi::CString;
use std::os::unix::ffi::OsStrExt;
use std::path::Path;
use std::rc::Rc;
use geom::Point;
use framebuffer::Framebuffer;

// Default font size in points
pub const DEFAULT_FONT_SIZE: f32 = 11.0;

// Font sizes in 1/64th of a point
// 2, 3, and 4 px at 300 DPI for Noto Sans UI
pub const FONT_SIZES: [u32; 3] = [349, 524, 699];

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
    regular: Font,
    italic: Font,
    bold: Font,
    bold_italic: Font,
}

pub struct Fonts {
    sans_serif: FontFamily,
    serif: FontFamily,
    keyboard: Font,
    display: Font,
}

impl Fonts {
    pub fn load() -> Result<Fonts> {
        let fo = FontOpener::new()?;
        Ok(Fonts {
            sans_serif: FontFamily {
                regular: fo.open("fonts/NotoSans-Regular.ttf")?,
                italic: fo.open("fonts/NotoSans-Italic.ttf")?,
                bold: fo.open("fonts/NotoSans-Bold.ttf")?,
                bold_italic: fo.open("fonts/NotoSans-BoldItalic.ttf")?,
            },
            serif: FontFamily {
                regular: fo.open("fonts/NotoSerif-Regular.ttf")?,
                italic: fo.open("fonts/NotoSerif-Italic.ttf")?,
                bold: fo.open("fonts/NotoSerif-Bold.ttf")?,
                bold_italic: fo.open("fonts/NotoSerif-BoldItalic.ttf")?,
            },
            keyboard: fo.open("fonts/VarelaRound-Regular.ttf")?,
            display: fo.open("fonts/Cormorant-Regular.ttf")?,
        })
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
        Family::Keyboard => &mut fonts.keyboard,
        Family::Display => &mut fonts.display,
    };
    font.set_size(style.size, dpi);
    font
}

pub struct FontLibrary(*mut FtLibrary);

pub struct FontOpener(Rc<FontLibrary>);

pub struct Font {
    _lib: Rc<FontLibrary>,
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
    pub fn new() -> Result<FontOpener> {
        unsafe {
            let mut lib = ptr::null_mut();
            let ret = FT_Init_FreeType(&mut lib);
            if ret != FT_ERR_OK {
                Err(ret.as_error_kind().into())
            } else {
                Ok(FontOpener(Rc::new(FontLibrary(lib))))
            }
        }
    }

    pub fn open<P: AsRef<Path>>(&self, path: P) -> Result<Font> {
        unsafe {
            let mut face = ptr::null_mut();
            let c_path = CString::new(path.as_ref().as_os_str().as_bytes()).unwrap();
            let ret = FT_New_Face((self.0).0, c_path.as_ptr(), 0, &mut face);
            if ret != FT_ERR_OK {
               return Err(ret.as_error_kind().into());
            }
            let font = ptr::null_mut();
            let ellipsis = RenderPlan::default();
            let x_heights = (0, 0);
            let space_codepoint = FT_Get_Char_Index(face, ' ' as libc::c_ulong);
            Ok(Font { _lib: self.0.clone(), face, font,
                      size: 0, dpi: 0, ellipsis, x_heights, space_codepoint })
        }
    }

    pub fn open_memory(&self, buf: &[u8]) -> Result<Font> {
        unsafe {
            let mut face = ptr::null_mut();
            let ret = FT_New_Memory_Face((self.0).0, buf.as_ptr() as *const FtByte, buf.len() as libc::c_long, 0, &mut face);
            if ret != FT_ERR_OK {
               return Err(ret.as_error_kind().into());
            }
            let ellipsis = RenderPlan::default();
            let font = ptr::null_mut();
            let x_heights = (0, 0);
            let space_codepoint = FT_Get_Char_Index(face, ' ' as libc::c_ulong);
            Ok(Font { _lib: self.0.clone(), face, font,
                      size: 0, dpi: 0, ellipsis, x_heights, space_codepoint })
        }
    }
}

impl Font {
    pub fn set_size(&mut self, size: u32, dpi: u16) {
        unsafe {
            if !self.font.is_null() {
                if self.size == size && self.dpi == dpi {
                    return;
                } else {
                    hb_font_destroy(self.font);
                }
            }
            self.size = size;
            self.dpi = dpi;
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

    // This is an approximation of the height of a character.
    // In the cases of *Noto Sans UI* and *Noto Serif*, the value given
    // for the height of the letter *x* is the exact height.
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

    pub fn advance_at(&self, index: usize) -> i32 {
        self.glyphs.iter().take(index).map(|g| g.advance.x).sum()
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

error_chain! {
    errors {
        UnknownError(code: FtError) {
            description("unknown error")
            display("unknown error with code {}", code)
        }

        CannotOpenResource {
            description("cannot open resource")
        }

        UnknownFileFormat {
            description("unknown file format")
        }

        InvalidFileFormat {
            description("broken file")
        }

        InvalidVersion {
            description("invalid FreeType version")
        }

        LowerModuleVersion {
            description("module version is too low")
        }

        InvalidArgument {
            description("invalid argument")
        }

        UnimplementedFeature {
            description("unimplemented feature")
        }

        InvalidTable {
            description("broken table")
        }

        InvalidOffset {
            description("broken offset within table")
        }

        ArrayTooLarge {
            description("array allocation size too large")
        }

        MissingModule {
            description("missing module")
        }

        MissingProperty {
            description("missing property")
        }

        InvalidGlyphIndex {
            description("invalid glyph index")
        }

        InvalidCharacterCode {
            description("invalid character code")
        }

        InvalidGlyphFormat {
            description("unsupported glyph image format")
        }

        CannotRenderGlyph {
            description("cannot render this glyph format")
        }

        InvalidOutline {
            description("invalid outline")
        }

        InvalidComposite {
            description("invalid composite glyph")
        }

        TooManyHints {
            description("too many hints")
        }

        InvalidPixelSize {
            description("invalid pixel size")
        }

        InvalidHandle {
            description("invalid object handle")
        }

        InvalidLibraryHandle {
            description("invalid library handle")
        }

        InvalidDriverHandle {
            description("invalid module handle")
        }

        InvalidFaceHandle {
            description("invalid face handle")
        }

        InvalidSizeHandle {
            description("invalid size handle")
        }

        InvalidSlotHandle {
            description("invalid glyph slot handle")
        }

        InvalidCharMapHandle {
            description("invalid charmap handle")
        }

        InvalidCacheHandle {
            description("invalid cache manager handle")
        }

        InvalidStreamHandle {
            description("invalid stream handle")
        }

        TooManyDrivers {
            description("too many modules")
        }

        TooManyExtensions {
            description("too many extensions")
        }

        OutOfMemory {
            description("out of memory")
        }

        UnlistedObject {
            description("unlisted object")
        }

        CannotOpenStream {
            description("cannot open stream")
        }

        InvalidStreamSeek {
            description("invalid stream seek")
        }

        InvalidStreamSkip {
            description("invalid stream skip")
        }

        InvalidStreamRead {
            description("invalid stream read")
        }

        InvalidStreamOperation {
            description("invalid stream operation")
        }

        InvalidFrameOperation {
            description("invalid frame operation")
        }

        NestedFrameAccess {
            description("nested frame access")
        }

        InvalidFrameRead {
            description("invalid frame read")
        }

        RasterUninitialized {
            description("raster uninitialized")
        }

        RasterCorrupted {
            description("raster corrupted")
        }

        RasterOverflow {
            description("raster overflow")
        }

        RasterNegativeHeight {
            description("negative height while rastering")
        }

        TooManyCaches {
            description("too many registered caches")
        }

        InvalidOpcode {
            description("invalid opcode")
        }

        TooFewArguments {
            description("too few arguments")
        }

        StackOverflow {
            description("stack overflow")
        }

        CodeOverflow {
            description("code overflow")
        }

        BadArgument {
            description("bad argument")
        }

        DivideByZero {
            description("division by zero")
        }

        InvalidReference {
            description("invalid reference")
        }

        DebugOpCode {
            description("found debug opcode")
        }

        ENDFInExecStream {
            description("found ENDF opcode in execution stream")
        }

        NestedDEFS {
            description("nested DEFS")
        }

        InvalidCodeRange {
            description("invalid code range")
        }

        ExecutionTooLong {
            description("execution context too long")
        }

        TooManyFunctionDefs {
            description("too many function definitions")
        }

        TooManyInstructionDefs {
            description("too many instruction definitions")
        }

        TableMissing {
            description("SFNT font table missing")
        }

        HorizHeaderMissing {
            description("horizontal header (hhea) table missing")
        }

        LocationsMissing {
            description("locations (loca) table missing")
        }

        NameTableMissing {
            description("name table missing")
        }

        CMapTableMissing {
            description("character map (cmap) table missing")
        }

        HmtxTableMissing {
            description("horizontal metrics (hmtx) table missing")
        }

        PostTableMissing {
            description("PostScript (post) table missing")
        }

        InvalidHorizMetrics {
            description("invalid horizontal metrics")
        }

        InvalidCharMapFormat {
            description("invalid character map (cmap) format")
        }

        InvalidPPem {
            description("invalid ppem value")
        }

        InvalidVertMetrics {
            description("invalid vertical metrics")
        }

        CouldNotFindContext {
            description("could not find context")
        }

        InvalidPostTableFormat {
            description("invalid PostScript (post) table format")
        }

        InvalidPostTable {
            description("invalid PostScript (post) table")
        }

        DEFInGlyfBytecode {
            description("found FDEF or IDEF opcode in glyf bytecode")
        }

        MissingBitmap {
            description("missing bitmap in strike")
        }

        SyntaxError {
            description("opcode syntax error")
        }

        StackUnderflow {
            description("argument stack underflow")
        }

        Ignore {
            description("ignore")
        }

        NoUnicodeGlyphName {
            description("no Unicode glyph name found")
        }

        GlyphTooBig {
            description("glyph too big for hinting")
        }

        MissingStartfontField {
            description("`STARTFONT' field missing")
        }

        MissingFontField {
            description("`FONT' field missing")
        }

        MissingSizeField {
            description("`SIZE' field missing")
        }

        MissingFontboundingboxField {
            description("`FONTBOUNDINGBOX' field missing")
        }

        MissingCharsField {
            description("`CHARS' field missing")
        }

        MissingStartcharField {
            description("`STARTCHAR' field missing")
        }

        MissingEncodingField {
            description("`ENCODING' field missing")
        }

        MissingBbxField {
            description("`BBX' field missing")
        }

        BbxTooBig {
            description("`BBX' too big")
        }

        CorruptedFontHeader {
            description("Font header corrupted or missing fields")
        }

        CorruptedFontGlyphs {
            description("Font glyphs corrupted or missing fields")
        }
    }
}



trait AsErrorKind {
    fn as_error_kind(&self) -> ErrorKind;
}

impl AsErrorKind for FtError {
    fn as_error_kind(&self) -> ErrorKind {
        match *self {
            0x01 => ErrorKind::CannotOpenResource,
            0x02 => ErrorKind::UnknownFileFormat,
            0x03 => ErrorKind::InvalidFileFormat,
            0x04 => ErrorKind::InvalidVersion,
            0x05 => ErrorKind::LowerModuleVersion,
            0x06 => ErrorKind::InvalidArgument,
            0x07 => ErrorKind::UnimplementedFeature,
            0x08 => ErrorKind::InvalidTable,
            0x09 => ErrorKind::InvalidOffset,
            0x0A => ErrorKind::ArrayTooLarge,
            0x0B => ErrorKind::MissingModule,
            0x0C => ErrorKind::MissingProperty,
            0x10 => ErrorKind::InvalidGlyphIndex,
            0x11 => ErrorKind::InvalidCharacterCode,
            0x12 => ErrorKind::InvalidGlyphFormat,
            0x13 => ErrorKind::CannotRenderGlyph,
            0x14 => ErrorKind::InvalidOutline,
            0x15 => ErrorKind::InvalidComposite,
            0x16 => ErrorKind::TooManyHints,
            0x17 => ErrorKind::InvalidPixelSize,
            0x20 => ErrorKind::InvalidHandle,
            0x21 => ErrorKind::InvalidLibraryHandle,
            0x22 => ErrorKind::InvalidDriverHandle,
            0x23 => ErrorKind::InvalidFaceHandle,
            0x24 => ErrorKind::InvalidSizeHandle,
            0x25 => ErrorKind::InvalidSlotHandle,
            0x26 => ErrorKind::InvalidCharMapHandle,
            0x27 => ErrorKind::InvalidCacheHandle,
            0x28 => ErrorKind::InvalidStreamHandle,
            0x30 => ErrorKind::TooManyDrivers,
            0x31 => ErrorKind::TooManyExtensions,
            0x40 => ErrorKind::OutOfMemory,
            0x41 => ErrorKind::UnlistedObject,
            0x51 => ErrorKind::CannotOpenStream,
            0x52 => ErrorKind::InvalidStreamSeek,
            0x53 => ErrorKind::InvalidStreamSkip,
            0x54 => ErrorKind::InvalidStreamRead,
            0x55 => ErrorKind::InvalidStreamOperation,
            0x56 => ErrorKind::InvalidFrameOperation,
            0x57 => ErrorKind::NestedFrameAccess,
            0x58 => ErrorKind::InvalidFrameRead,
            0x60 => ErrorKind::RasterUninitialized,
            0x61 => ErrorKind::RasterCorrupted,
            0x62 => ErrorKind::RasterOverflow,
            0x63 => ErrorKind::RasterNegativeHeight,
            0x70 => ErrorKind::TooManyCaches,
            0x80 => ErrorKind::InvalidOpcode,
            0x81 => ErrorKind::TooFewArguments,
            0x82 => ErrorKind::StackOverflow,
            0x83 => ErrorKind::CodeOverflow,
            0x84 => ErrorKind::BadArgument,
            0x85 => ErrorKind::DivideByZero,
            0x86 => ErrorKind::InvalidReference,
            0x87 => ErrorKind::DebugOpCode,
            0x88 => ErrorKind::ENDFInExecStream,
            0x89 => ErrorKind::NestedDEFS,
            0x8A => ErrorKind::InvalidCodeRange,
            0x8B => ErrorKind::ExecutionTooLong,
            0x8C => ErrorKind::TooManyFunctionDefs,
            0x8D => ErrorKind::TooManyInstructionDefs,
            0x8E => ErrorKind::TableMissing,
            0x8F => ErrorKind::HorizHeaderMissing,
            0x90 => ErrorKind::LocationsMissing,
            0x91 => ErrorKind::NameTableMissing,
            0x92 => ErrorKind::CMapTableMissing,
            0x93 => ErrorKind::HmtxTableMissing,
            0x94 => ErrorKind::PostTableMissing,
            0x95 => ErrorKind::InvalidHorizMetrics,
            0x96 => ErrorKind::InvalidCharMapFormat,
            0x97 => ErrorKind::InvalidPPem,
            0x98 => ErrorKind::InvalidVertMetrics,
            0x99 => ErrorKind::CouldNotFindContext,
            0x9A => ErrorKind::InvalidPostTableFormat,
            0x9B => ErrorKind::InvalidPostTable,
            0x9C => ErrorKind::DEFInGlyfBytecode,
            0x9D => ErrorKind::MissingBitmap,
            0xA0 => ErrorKind::SyntaxError,
            0xA1 => ErrorKind::StackUnderflow,
            0xA2 => ErrorKind::Ignore,
            0xA3 => ErrorKind::NoUnicodeGlyphName,
            0xA4 => ErrorKind::GlyphTooBig,
            0xB0 => ErrorKind::MissingStartfontField,
            0xB1 => ErrorKind::MissingFontField,
            0xB2 => ErrorKind::MissingSizeField,
            0xB3 => ErrorKind::MissingFontboundingboxField,
            0xB4 => ErrorKind::MissingCharsField,
            0xB5 => ErrorKind::MissingStartcharField,
            0xB6 => ErrorKind::MissingEncodingField,
            0xB7 => ErrorKind::MissingBbxField,
            0xB8 => ErrorKind::BbxTooBig,
            0xB9 => ErrorKind::CorruptedFontHeader,
            0xBA => ErrorKind::CorruptedFontGlyphs,
            code => ErrorKind::UnknownError(code),
        }
    }
}
