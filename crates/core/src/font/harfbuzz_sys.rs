#![allow(unused)]

use std::mem;
use super::freetype_sys::FtFace;

macro_rules! hb_tag {
    ($c1:expr, $c2:expr, $c3:expr, $c4:expr) => (($c1 as HbTag) << 24 |
                                                 ($c2 as HbTag) << 16 |
                                                 ($c3 as HbTag) << 8 |
                                                 ($c4 as HbTag));
}

pub type HbDirection = libc::c_uint;
pub type HbTag = u32;
pub type HbScript = libc::c_uint;
pub type HbBool = libc::c_int;

pub enum HbLanguage {}
pub enum HbBuffer {}
pub enum HbFont {}

#[link(name="harfbuzz")]
extern {
    pub fn hb_ft_font_create(face: *mut FtFace, destroy: *const libc::c_void) -> *mut HbFont;
    pub fn hb_ft_font_changed(font: *mut HbFont);
    pub fn hb_font_destroy(font: *mut HbFont);
    pub fn hb_buffer_create() -> *mut HbBuffer;
    pub fn hb_buffer_destroy(buf: *mut HbBuffer);
    pub fn hb_buffer_clear_contents(buf: *mut HbBuffer);
    pub fn hb_buffer_add_utf8(buf: *mut HbBuffer, txt: *const libc::c_char, len: libc::c_int, offset: libc::c_uint, ilen: libc::c_int);
    pub fn hb_buffer_set_direction(buf: *mut HbBuffer, dir: HbDirection);
    pub fn hb_buffer_guess_segment_properties(buf: *mut HbBuffer);
    pub fn hb_shape(font: *mut HbFont, buf: *mut HbBuffer, features: *const HbFeature, features_count: libc::c_uint);
    pub fn hb_feature_from_string(s: *const libc::c_char, len: libc::c_int, feature: *mut HbFeature) -> HbBool;
    pub fn hb_buffer_get_length(buf: *mut HbBuffer) -> libc::c_uint;
    pub fn hb_buffer_get_glyph_infos(buf: *mut HbBuffer, len: *mut libc::c_uint) -> *mut HbGlyphInfo;
    pub fn hb_buffer_get_glyph_positions(buf: *mut HbBuffer, len: *mut libc::c_uint) -> *mut HbGlyphPosition;
    pub fn hb_buffer_get_direction(buf: *const HbBuffer) -> HbDirection;
    pub fn hb_buffer_get_language(buf: *const HbBuffer) -> *const HbLanguage;
    pub fn hb_buffer_get_script(buf: *const HbBuffer) -> HbScript;
}

#[repr(C)]
#[derive(Debug)]
pub struct HbGlyphInfo {
    pub codepoint: u32,
    mask: u32,
    pub cluster: u32,
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
    value: u32,
    start: libc::c_uint,
    end: libc::c_uint,
}

impl Default for HbFeature {
    fn default() -> Self {
        unsafe { mem::zeroed() }
    }
}

pub const HB_DIRECTION_LTR: HbDirection = 4;
pub const HB_DIRECTION_RTL: HbDirection = 5;
pub const HB_DIRECTION_TTB: HbDirection = 6;
pub const HB_DIRECTION_BTT: HbDirection = 7;

// Extracted from harfbuzz in src/hb-common.h
pub const HB_SCRIPT_COMMON: HbTag = hb_tag!('Z','y','y','y');
pub const HB_SCRIPT_INHERITED: HbTag = hb_tag!('Z','i','n','h');
pub const HB_SCRIPT_UNKNOWN: HbTag = hb_tag!('Z','z','z','z');
pub const HB_SCRIPT_INVALID: HbTag = 0;

// Custom *scripts*.
pub const HB_SYMBOL_MISC_ONE: HbTag = hb_tag!('Z','s','m','o');
pub const HB_SYMBOL_MISC_TWO: HbTag = hb_tag!('Z','s','m','t');
pub const HB_SYMBOL_MUSIC: HbTag = hb_tag!('Z','s','m','u');
pub const HB_SYMBOL_MATHS: HbTag = hb_tag!('Z','s','m','a');
pub const HB_SYMBOL_EMOJI: HbTag = hb_tag!('Z','s','e','j');

pub const HB_SCRIPT_ARABIC: HbTag = hb_tag!('A','r','a','b');
pub const HB_SCRIPT_ARMENIAN: HbTag = hb_tag!('A','r','m','n');
pub const HB_SCRIPT_BENGALI: HbTag = hb_tag!('B','e','n','g');
pub const HB_SCRIPT_CYRILLIC: HbTag = hb_tag!('C','y','r','l');
pub const HB_SCRIPT_DEVANAGARI: HbTag = hb_tag!('D','e','v','a');
pub const HB_SCRIPT_GEORGIAN: HbTag = hb_tag!('G','e','o','r');
pub const HB_SCRIPT_GREEK: HbTag = hb_tag!('G','r','e','k');
pub const HB_SCRIPT_GUJARATI: HbTag = hb_tag!('G','u','j','r');
pub const HB_SCRIPT_GURMUKHI: HbTag = hb_tag!('G','u','r','u');
pub const HB_SCRIPT_HANGUL: HbTag = hb_tag!('H','a','n','g');
pub const HB_SCRIPT_HAN: HbTag = hb_tag!('H','a','n','i');
pub const HB_SCRIPT_HEBREW: HbTag = hb_tag!('H','e','b','r');
pub const HB_SCRIPT_HIRAGANA: HbTag = hb_tag!('H','i','r','a');
pub const HB_SCRIPT_KANNADA: HbTag = hb_tag!('K','n','d','a');
pub const HB_SCRIPT_KATAKANA: HbTag = hb_tag!('K','a','n','a');
pub const HB_SCRIPT_LAO: HbTag = hb_tag!('L','a','o','o');
pub const HB_SCRIPT_LATIN: HbTag = hb_tag!('L','a','t','n');
pub const HB_SCRIPT_MALAYALAM: HbTag = hb_tag!('M','l','y','m');
pub const HB_SCRIPT_ORIYA: HbTag = hb_tag!('O','r','y','a');
pub const HB_SCRIPT_TAMIL: HbTag = hb_tag!('T','a','m','l');
pub const HB_SCRIPT_TELUGU: HbTag = hb_tag!('T','e','l','u');
pub const HB_SCRIPT_THAI: HbTag = hb_tag!('T','h','a','i');
pub const HB_SCRIPT_TIBETAN: HbTag = hb_tag!('T','i','b','t');
pub const HB_SCRIPT_BOPOMOFO: HbTag = hb_tag!('B','o','p','o');
pub const HB_SCRIPT_BRAILLE: HbTag = hb_tag!('B','r','a','i');
pub const HB_SCRIPT_CANADIAN_SYLLABICS: HbTag = hb_tag!('C','a','n','s');
pub const HB_SCRIPT_CHEROKEE: HbTag = hb_tag!('C','h','e','r');
pub const HB_SCRIPT_ETHIOPIC: HbTag = hb_tag!('E','t','h','i');
pub const HB_SCRIPT_KHMER: HbTag = hb_tag!('K','h','m','r');
pub const HB_SCRIPT_MONGOLIAN: HbTag = hb_tag!('M','o','n','g');
pub const HB_SCRIPT_MYANMAR: HbTag = hb_tag!('M','y','m','r');
pub const HB_SCRIPT_OGHAM: HbTag = hb_tag!('O','g','a','m');
pub const HB_SCRIPT_RUNIC: HbTag = hb_tag!('R','u','n','r');
pub const HB_SCRIPT_SINHALA: HbTag = hb_tag!('S','i','n','h');
pub const HB_SCRIPT_SYRIAC: HbTag = hb_tag!('S','y','r','c');
pub const HB_SCRIPT_THAANA: HbTag = hb_tag!('T','h','a','a');
pub const HB_SCRIPT_YI: HbTag = hb_tag!('Y','i','i','i');
pub const HB_SCRIPT_DESERET: HbTag = hb_tag!('D','s','r','t');
pub const HB_SCRIPT_GOTHIC: HbTag = hb_tag!('G','o','t','h');
pub const HB_SCRIPT_OLD_ITALIC: HbTag = hb_tag!('I','t','a','l');
pub const HB_SCRIPT_BUHID: HbTag = hb_tag!('B','u','h','d');
pub const HB_SCRIPT_HANUNOO: HbTag = hb_tag!('H','a','n','o');
pub const HB_SCRIPT_TAGALOG: HbTag = hb_tag!('T','g','l','g');
pub const HB_SCRIPT_TAGBANWA: HbTag = hb_tag!('T','a','g','b');
pub const HB_SCRIPT_CYPRIOT: HbTag = hb_tag!('C','p','r','t');
pub const HB_SCRIPT_LIMBU: HbTag = hb_tag!('L','i','m','b');
pub const HB_SCRIPT_LINEAR_B: HbTag = hb_tag!('L','i','n','b');
pub const HB_SCRIPT_OSMANYA: HbTag = hb_tag!('O','s','m','a');
pub const HB_SCRIPT_SHAVIAN: HbTag = hb_tag!('S','h','a','w');
pub const HB_SCRIPT_TAI_LE: HbTag = hb_tag!('T','a','l','e');
pub const HB_SCRIPT_UGARITIC: HbTag = hb_tag!('U','g','a','r');
pub const HB_SCRIPT_BUGINESE: HbTag = hb_tag!('B','u','g','i');
pub const HB_SCRIPT_COPTIC: HbTag = hb_tag!('C','o','p','t');
pub const HB_SCRIPT_GLAGOLITIC: HbTag = hb_tag!('G','l','a','g');
pub const HB_SCRIPT_KHAROSHTHI: HbTag = hb_tag!('K','h','a','r');
pub const HB_SCRIPT_NEW_TAI_LUE: HbTag = hb_tag!('T','a','l','u');
pub const HB_SCRIPT_OLD_PERSIAN: HbTag = hb_tag!('X','p','e','o');
pub const HB_SCRIPT_SYLOTI_NAGRI: HbTag = hb_tag!('S','y','l','o');
pub const HB_SCRIPT_TIFINAGH: HbTag = hb_tag!('T','f','n','g');
pub const HB_SCRIPT_BALINESE: HbTag = hb_tag!('B','a','l','i');
pub const HB_SCRIPT_CUNEIFORM: HbTag = hb_tag!('X','s','u','x');
pub const HB_SCRIPT_NKO: HbTag = hb_tag!('N','k','o','o');
pub const HB_SCRIPT_PHAGS_PA: HbTag = hb_tag!('P','h','a','g');
pub const HB_SCRIPT_PHOENICIAN: HbTag = hb_tag!('P','h','n','x');
pub const HB_SCRIPT_CARIAN: HbTag = hb_tag!('C','a','r','i');
pub const HB_SCRIPT_CHAM: HbTag = hb_tag!('C','h','a','m');
pub const HB_SCRIPT_KAYAH_LI: HbTag = hb_tag!('K','a','l','i');
pub const HB_SCRIPT_LEPCHA: HbTag = hb_tag!('L','e','p','c');
pub const HB_SCRIPT_LYCIAN: HbTag = hb_tag!('L','y','c','i');
pub const HB_SCRIPT_LYDIAN: HbTag = hb_tag!('L','y','d','i');
pub const HB_SCRIPT_OL_CHIKI: HbTag = hb_tag!('O','l','c','k');
pub const HB_SCRIPT_REJANG: HbTag = hb_tag!('R','j','n','g');
pub const HB_SCRIPT_SAURASHTRA: HbTag = hb_tag!('S','a','u','r');
pub const HB_SCRIPT_SUNDANESE: HbTag = hb_tag!('S','u','n','d');
pub const HB_SCRIPT_VAI: HbTag = hb_tag!('V','a','i','i');
pub const HB_SCRIPT_AVESTAN: HbTag = hb_tag!('A','v','s','t');
pub const HB_SCRIPT_BAMUM: HbTag = hb_tag!('B','a','m','u');
pub const HB_SCRIPT_EGYPTIAN_HIEROGLYPHS: HbTag = hb_tag!('E','g','y','p');
pub const HB_SCRIPT_IMPERIAL_ARAMAIC: HbTag = hb_tag!('A','r','m','i');
pub const HB_SCRIPT_INSCRIPTIONAL_PAHLAVI: HbTag = hb_tag!('P','h','l','i');
pub const HB_SCRIPT_INSCRIPTIONAL_PARTHIAN: HbTag = hb_tag!('P','r','t','i');
pub const HB_SCRIPT_JAVANESE: HbTag = hb_tag!('J','a','v','a');
pub const HB_SCRIPT_KAITHI: HbTag = hb_tag!('K','t','h','i');
pub const HB_SCRIPT_LISU: HbTag = hb_tag!('L','i','s','u');
pub const HB_SCRIPT_MEETEI_MAYEK: HbTag = hb_tag!('M','t','e','i');
pub const HB_SCRIPT_OLD_SOUTH_ARABIAN: HbTag = hb_tag!('S','a','r','b');
pub const HB_SCRIPT_OLD_TURKIC: HbTag = hb_tag!('O','r','k','h');
pub const HB_SCRIPT_SAMARITAN: HbTag = hb_tag!('S','a','m','r');
pub const HB_SCRIPT_TAI_THAM: HbTag = hb_tag!('L','a','n','a');
pub const HB_SCRIPT_TAI_VIET: HbTag = hb_tag!('T','a','v','t');
pub const HB_SCRIPT_BATAK: HbTag = hb_tag!('B','a','t','k');
pub const HB_SCRIPT_BRAHMI: HbTag = hb_tag!('B','r','a','h');
pub const HB_SCRIPT_MANDAIC: HbTag = hb_tag!('M','a','n','d');
pub const HB_SCRIPT_CHAKMA: HbTag = hb_tag!('C','a','k','m');
pub const HB_SCRIPT_MEROITIC_CURSIVE: HbTag = hb_tag!('M','e','r','c');
pub const HB_SCRIPT_MEROITIC_HIEROGLYPHS: HbTag = hb_tag!('M','e','r','o');
pub const HB_SCRIPT_MIAO: HbTag = hb_tag!('P','l','r','d');
pub const HB_SCRIPT_SHARADA: HbTag = hb_tag!('S','h','r','d');
pub const HB_SCRIPT_SORA_SOMPENG: HbTag = hb_tag!('S','o','r','a');
pub const HB_SCRIPT_TAKRI: HbTag = hb_tag!('T','a','k','r');
pub const HB_SCRIPT_BASSA_VAH: HbTag = hb_tag!('B','a','s','s');
pub const HB_SCRIPT_CAUCASIAN_ALBANIAN: HbTag = hb_tag!('A','g','h','b');
pub const HB_SCRIPT_DUPLOYAN: HbTag = hb_tag!('D','u','p','l');
pub const HB_SCRIPT_ELBASAN: HbTag = hb_tag!('E','l','b','a');
pub const HB_SCRIPT_GRANTHA: HbTag = hb_tag!('G','r','a','n');
pub const HB_SCRIPT_KHOJKI: HbTag = hb_tag!('K','h','o','j');
pub const HB_SCRIPT_KHUDAWADI: HbTag = hb_tag!('S','i','n','d');
pub const HB_SCRIPT_LINEAR_A: HbTag = hb_tag!('L','i','n','a');
pub const HB_SCRIPT_MAHAJANI: HbTag = hb_tag!('M','a','h','j');
pub const HB_SCRIPT_MANICHAEAN: HbTag = hb_tag!('M','a','n','i');
pub const HB_SCRIPT_MENDE_KIKAKUI: HbTag = hb_tag!('M','e','n','d');
pub const HB_SCRIPT_MODI: HbTag = hb_tag!('M','o','d','i');
pub const HB_SCRIPT_MRO: HbTag = hb_tag!('M','r','o','o');
pub const HB_SCRIPT_NABATAEAN: HbTag = hb_tag!('N','b','a','t');
pub const HB_SCRIPT_OLD_NORTH_ARABIAN: HbTag = hb_tag!('N','a','r','b');
pub const HB_SCRIPT_OLD_PERMIC: HbTag = hb_tag!('P','e','r','m');
pub const HB_SCRIPT_PAHAWH_HMONG: HbTag = hb_tag!('H','m','n','g');
pub const HB_SCRIPT_PALMYRENE: HbTag = hb_tag!('P','a','l','m');
pub const HB_SCRIPT_PAU_CIN_HAU: HbTag = hb_tag!('P','a','u','c');
pub const HB_SCRIPT_PSALTER_PAHLAVI: HbTag = hb_tag!('P','h','l','p');
pub const HB_SCRIPT_SIDDHAM: HbTag = hb_tag!('S','i','d','d');
pub const HB_SCRIPT_TIRHUTA: HbTag = hb_tag!('T','i','r','h');
pub const HB_SCRIPT_WARANG_CITI: HbTag = hb_tag!('W','a','r','a');
pub const HB_SCRIPT_AHOM: HbTag = hb_tag!('A','h','o','m');
pub const HB_SCRIPT_ANATOLIAN_HIEROGLYPHS: HbTag = hb_tag!('H','l','u','w');
pub const HB_SCRIPT_HATRAN: HbTag = hb_tag!('H','a','t','r');
pub const HB_SCRIPT_MULTANI: HbTag = hb_tag!('M','u','l','t');
pub const HB_SCRIPT_OLD_HUNGARIAN: HbTag = hb_tag!('H','u','n','g');
pub const HB_SCRIPT_SIGNWRITING: HbTag = hb_tag!('S','g','n','w');
pub const HB_SCRIPT_ADLAM: HbTag = hb_tag!('A','d','l','m');
pub const HB_SCRIPT_BHAIKSUKI: HbTag = hb_tag!('B','h','k','s');
pub const HB_SCRIPT_MARCHEN: HbTag = hb_tag!('M','a','r','c');
pub const HB_SCRIPT_OSAGE: HbTag = hb_tag!('O','s','g','e');
pub const HB_SCRIPT_TANGUT: HbTag = hb_tag!('T','a','n','g');
pub const HB_SCRIPT_NEWA: HbTag = hb_tag!('N','e','w','a');
pub const HB_SCRIPT_MASARAM_GONDI: HbTag = hb_tag!('G','o','n','m');
pub const HB_SCRIPT_NUSHU: HbTag = hb_tag!('N','s','h','u');
pub const HB_SCRIPT_SOYOMBO: HbTag = hb_tag!('S','o','y','o');
pub const HB_SCRIPT_ZANABAZAR_SQUARE: HbTag = hb_tag!('Z','a','n','b');
pub const HB_SCRIPT_DOGRA: HbTag = hb_tag!('D','o','g','r');
pub const HB_SCRIPT_GUNJALA_GONDI: HbTag = hb_tag!('G','o','n','g');
pub const HB_SCRIPT_HANIFI_ROHINGYA: HbTag = hb_tag!('R','o','h','g');
pub const HB_SCRIPT_MAKASAR: HbTag = hb_tag!('M','a','k','a');
pub const HB_SCRIPT_MEDEFAIDRIN: HbTag = hb_tag!('M','e','d','f');
pub const HB_SCRIPT_OLD_SOGDIAN: HbTag = hb_tag!('S','o','g','o');
pub const HB_SCRIPT_SOGDIAN: HbTag = hb_tag!('S','o','g','d');
pub const HB_SCRIPT_ELYMAIC: HbTag = hb_tag!('E','l','y','m');
pub const HB_SCRIPT_NANDINAGARI: HbTag = hb_tag!('N','a','n','d');
pub const HB_SCRIPT_NYIAKENG_PUACHUE_HMONG: HbTag = hb_tag!('H','m','n','p');
pub const HB_SCRIPT_WANCHO: HbTag = hb_tag!('W','c','h','o');
pub const HB_SCRIPT_CHORASMIAN: HbTag = hb_tag!('C','h','r','s');
pub const HB_SCRIPT_DIVES_AKURU: HbTag = hb_tag!('D','i','a','k');
pub const HB_SCRIPT_KHITAN_SMALL_SCRIPT: HbTag = hb_tag!('K','i','t','s');
pub const HB_SCRIPT_YEZIDI: HbTag = hb_tag!('Y','e','z','i');
pub const HB_SCRIPT_CYPRO_MINOAN: HbTag = hb_tag!('C','p','m','n');
pub const HB_SCRIPT_OLD_UYGHUR: HbTag = hb_tag!('O','u','g','r');
pub const HB_SCRIPT_TANGSA: HbTag = hb_tag!('T','n','s','a');
pub const HB_SCRIPT_TOTO: HbTag = hb_tag!('T','o','t','o');
pub const HB_SCRIPT_VITHKUQI: HbTag = hb_tag!('V','i','t','h');
pub const HB_SCRIPT_MATH: HbTag = hb_tag!('Z','m','t','h');
pub const HB_SCRIPT_KAWI: HbTag = hb_tag!('K','a','w','i');
pub const HB_SCRIPT_NAG_MUNDARI: HbTag = hb_tag!('N','a','g','m');
