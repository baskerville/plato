mod harfbuzz_sys;
mod freetype_sys;

use self::harfbuzz_sys::*;
use self::freetype_sys::*;

use std::str;
use std::ptr;
use std::slice;
use std::ffi::{CString, CStr};
use std::os::unix::ffi::OsStrExt;
use std::path::Path;
use std::collections::{HashMap, BTreeSet};
use std::rc::Rc;
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

#[link(name="mupdf")]
extern {
    // Extracted from mupdf via `head -n 1 generated/resources/fonts/{droid,noto}/*`
    pub static _binary_DroidSansFallback_ttf: [libc::c_uchar; 3556308];
    pub static _binary_NotoEmoji_Regular_ttf: [libc::c_uchar; 418804];
    pub static _binary_NotoKufiArabic_Regular_ttf: [libc::c_uchar; 62996];
    pub static _binary_NotoMusic_Regular_otf: [libc::c_uchar; 60824];
    pub static _binary_NotoNaskhArabic_Regular_ttf: [libc::c_uchar; 136084];
    pub static _binary_NotoNastaliqUrdu_Regular_otf: [libc::c_uchar; 373208];
    pub static _binary_NotoNastaliqUrdu_Regular_ttf: [libc::c_uchar; 497204];
    pub static _binary_NotoSans_Regular_otf: [libc::c_uchar; 263116];
    pub static _binary_NotoSansAdlam_Regular_otf: [libc::c_uchar; 29848];
    pub static _binary_NotoSansAhom_Regular_otf: [libc::c_uchar; 13852];
    pub static _binary_NotoSansAnatolianHieroglyphs_Regular_otf: [libc::c_uchar; 134528];
    pub static _binary_NotoSansArabic_Regular_otf: [libc::c_uchar; 116528];
    pub static _binary_NotoSansAvestan_Regular_otf: [libc::c_uchar; 9308];
    pub static _binary_NotoSansBamum_Regular_otf: [libc::c_uchar; 104284];
    pub static _binary_NotoSansBassaVah_Regular_otf: [libc::c_uchar; 6256];
    pub static _binary_NotoSansBatak_Regular_otf: [libc::c_uchar; 11108];
    pub static _binary_NotoSansBengali_Regular_otf: [libc::c_uchar; 79944];
    pub static _binary_NotoSansBhaiksuki_Regular_otf: [libc::c_uchar; 99816];
    pub static _binary_NotoSansBrahmi_Regular_otf: [libc::c_uchar; 27364];
    pub static _binary_NotoSansBuginese_Regular_otf: [libc::c_uchar; 6248];
    pub static _binary_NotoSansBuhid_Regular_otf: [libc::c_uchar; 5040];
    pub static _binary_NotoSansCanadianAboriginal_Regular_otf: [libc::c_uchar; 38100];
    pub static _binary_NotoSansCarian_Regular_otf: [libc::c_uchar; 5592];
    pub static _binary_NotoSansCaucasianAlbanian_Regular_otf: [libc::c_uchar; 17388];
    pub static _binary_NotoSansChakma_Regular_otf: [libc::c_uchar; 29512];
    pub static _binary_NotoSansCham_Regular_otf: [libc::c_uchar; 21260];
    pub static _binary_NotoSansCherokee_Regular_otf: [libc::c_uchar; 56740];
    pub static _binary_NotoSansCoptic_Regular_otf: [libc::c_uchar; 21432];
    pub static _binary_NotoSansCuneiform_Regular_otf: [libc::c_uchar; 416252];
    pub static _binary_NotoSansCypriot_Regular_otf: [libc::c_uchar; 7024];
    pub static _binary_NotoSansDeseret_Regular_otf: [libc::c_uchar; 9016];
    pub static _binary_NotoSansDevanagari_Regular_otf: [libc::c_uchar; 115204];
    pub static _binary_NotoSansDuployan_Regular_otf: [libc::c_uchar; 10336];
    pub static _binary_NotoSansEgyptianHieroglyphs_Regular_otf: [libc::c_uchar; 363244];
    pub static _binary_NotoSansElbasan_Regular_otf: [libc::c_uchar; 8684];
    pub static _binary_NotoSansGlagolitic_Regular_otf: [libc::c_uchar; 17252];
    pub static _binary_NotoSansGothic_Regular_otf: [libc::c_uchar; 5424];
    pub static _binary_NotoSansGrantha_Regular_otf: [libc::c_uchar; 94548];
    pub static _binary_NotoSansHanunoo_Regular_otf: [libc::c_uchar; 6596];
    pub static _binary_NotoSansHatran_Regular_otf: [libc::c_uchar; 4324];
    pub static _binary_NotoSansImperialAramaic_Regular_otf: [libc::c_uchar; 5436];
    pub static _binary_NotoSansInscriptionalPahlavi_Regular_otf: [libc::c_uchar; 5464];
    pub static _binary_NotoSansInscriptionalParthian_Regular_otf: [libc::c_uchar; 6788];
    pub static _binary_NotoSansJavanese_Regular_otf: [libc::c_uchar; 86944];
    pub static _binary_NotoSansJavanese_Regular_ttf: [libc::c_uchar; 40468];
    pub static _binary_NotoSansKaithi_Regular_otf: [libc::c_uchar; 39600];
    pub static _binary_NotoSansKayahLi_Regular_otf: [libc::c_uchar; 7008];
    pub static _binary_NotoSansKharoshthi_Regular_otf: [libc::c_uchar; 19260];
    pub static _binary_NotoSansKhojki_Regular_otf: [libc::c_uchar; 26952];
    pub static _binary_NotoSansKhudawadi_Regular_otf: [libc::c_uchar; 14764];
    pub static _binary_NotoSansLepcha_Regular_otf: [libc::c_uchar; 18832];
    pub static _binary_NotoSansLimbu_Regular_otf: [libc::c_uchar; 10040];
    pub static _binary_NotoSansLinearA_Regular_otf: [libc::c_uchar; 33664];
    pub static _binary_NotoSansLinearB_Regular_otf: [libc::c_uchar; 37124];
    pub static _binary_NotoSansLisu_Regular_otf: [libc::c_uchar; 5400];
    pub static _binary_NotoSansLycian_Regular_otf: [libc::c_uchar; 4108];
    pub static _binary_NotoSansLydian_Regular_otf: [libc::c_uchar; 4088];
    pub static _binary_NotoSansMahajani_Regular_otf: [libc::c_uchar; 10136];
    pub static _binary_NotoSansMalayalam_Regular_otf: [libc::c_uchar; 48048];
    pub static _binary_NotoSansMandaic_Regular_otf: [libc::c_uchar; 13092];
    pub static _binary_NotoSansManichaean_Regular_otf: [libc::c_uchar; 16500];
    pub static _binary_NotoSansMarchen_Regular_otf: [libc::c_uchar; 63576];
    pub static _binary_NotoSansMath_Regular_otf: [libc::c_uchar; 251968];
    pub static _binary_NotoSansMeeteiMayek_Regular_otf: [libc::c_uchar; 11996];
    pub static _binary_NotoSansMendeKikakui_Regular_otf: [libc::c_uchar; 19652];
    pub static _binary_NotoSansMeroitic_Regular_otf: [libc::c_uchar; 19960];
    pub static _binary_NotoSansMiao_Regular_otf: [libc::c_uchar; 22664];
    pub static _binary_NotoSansModi_Regular_otf: [libc::c_uchar; 29412];
    pub static _binary_NotoSansMongolian_Regular_otf: [libc::c_uchar; 102044];
    pub static _binary_NotoSansMongolian_Regular_ttf: [libc::c_uchar; 135484];
    pub static _binary_NotoSansMro_Regular_otf: [libc::c_uchar; 5608];
    pub static _binary_NotoSansMultani_Regular_otf: [libc::c_uchar; 7724];
    pub static _binary_NotoSansNKo_Regular_otf: [libc::c_uchar; 13280];
    pub static _binary_NotoSansNabataean_Regular_otf: [libc::c_uchar; 6544];
    pub static _binary_NotoSansNewTaiLue_Regular_otf: [libc::c_uchar; 11152];
    pub static _binary_NotoSansNewa_Regular_otf: [libc::c_uchar; 152764];
    pub static _binary_NotoSansOgham_Regular_otf: [libc::c_uchar; 3720];
    pub static _binary_NotoSansOlChiki_Regular_otf: [libc::c_uchar; 6824];
    pub static _binary_NotoSansOldHungarian_Regular_otf: [libc::c_uchar; 44660];
    pub static _binary_NotoSansOldItalic_Regular_otf: [libc::c_uchar; 5964];
    pub static _binary_NotoSansOldNorthArabian_Regular_otf: [libc::c_uchar; 6204];
    pub static _binary_NotoSansOldPermic_Regular_otf: [libc::c_uchar; 8544];
    pub static _binary_NotoSansOldPersian_Regular_otf: [libc::c_uchar; 9856];
    pub static _binary_NotoSansOldSouthArabian_Regular_otf: [libc::c_uchar; 4288];
    pub static _binary_NotoSansOldTurkic_Regular_otf: [libc::c_uchar; 6884];
    pub static _binary_NotoSansOriya_Regular_ttf: [libc::c_uchar; 103684];
    pub static _binary_NotoSansOsage_Regular_otf: [libc::c_uchar; 9296];
    pub static _binary_NotoSansOsmanya_Regular_otf: [libc::c_uchar; 6784];
    pub static _binary_NotoSansPahawhHmong_Regular_otf: [libc::c_uchar; 13072];
    pub static _binary_NotoSansPalmyrene_Regular_otf: [libc::c_uchar; 8528];
    pub static _binary_NotoSansPauCinHau_Regular_otf: [libc::c_uchar; 8124];
    pub static _binary_NotoSansPhagsPa_Regular_otf: [libc::c_uchar; 24032];
    pub static _binary_NotoSansPhoenician_Regular_otf: [libc::c_uchar; 5264];
    pub static _binary_NotoSansPsalterPahlavi_Regular_otf: [libc::c_uchar; 12748];
    pub static _binary_NotoSansRejang_Regular_otf: [libc::c_uchar; 6488];
    pub static _binary_NotoSansRunic_Regular_otf: [libc::c_uchar; 7200];
    pub static _binary_NotoSansSamaritan_Regular_otf: [libc::c_uchar; 9076];
    pub static _binary_NotoSansSaurashtra_Regular_otf: [libc::c_uchar; 16020];
    pub static _binary_NotoSansSharada_Regular_otf: [libc::c_uchar; 30300];
    pub static _binary_NotoSansShavian_Regular_otf: [libc::c_uchar; 5468];
    pub static _binary_NotoSansSiddham_Regular_otf: [libc::c_uchar; 92000];
    pub static _binary_NotoSansSoraSompeng_Regular_otf: [libc::c_uchar; 6304];
    pub static _binary_NotoSansSundanese_Regular_otf: [libc::c_uchar; 9308];
    pub static _binary_NotoSansSylotiNagri_Regular_otf: [libc::c_uchar; 13016];
    pub static _binary_NotoSansSymbols_Regular_otf: [libc::c_uchar; 107580];
    pub static _binary_NotoSansSymbols2_Regular_otf: [libc::c_uchar; 318416];
    pub static _binary_NotoSansSyriac_Regular_otf: [libc::c_uchar; 124772];
    pub static _binary_NotoSansSyriacEastern_Regular_ttf: [libc::c_uchar; 50164];
    pub static _binary_NotoSansSyriacEstrangela_Regular_ttf: [libc::c_uchar; 46396];
    pub static _binary_NotoSansSyriacWestern_Regular_ttf: [libc::c_uchar; 52380];
    pub static _binary_NotoSansTagalog_Regular_otf: [libc::c_uchar; 5548];
    pub static _binary_NotoSansTagbanwa_Regular_otf: [libc::c_uchar; 5736];
    pub static _binary_NotoSansTaiLe_Regular_otf: [libc::c_uchar; 8616];
    pub static _binary_NotoSansTaiTham_Regular_ttf: [libc::c_uchar; 51040];
    pub static _binary_NotoSansTaiViet_Regular_otf: [libc::c_uchar; 12328];
    pub static _binary_NotoSansTakri_Regular_otf: [libc::c_uchar; 12708];
    pub static _binary_NotoSansThaana_Regular_otf: [libc::c_uchar; 12484];
    pub static _binary_NotoSansThaana_Regular_ttf: [libc::c_uchar; 15284];
    pub static _binary_NotoSansTibetan_Regular_ttf: [libc::c_uchar; 422408];
    pub static _binary_NotoSansTifinagh_Regular_otf: [libc::c_uchar; 11408];
    pub static _binary_NotoSansTirhuta_Regular_otf: [libc::c_uchar; 52436];
    pub static _binary_NotoSansUgaritic_Regular_otf: [libc::c_uchar; 5336];
    pub static _binary_NotoSansVai_Regular_otf: [libc::c_uchar; 24088];
    pub static _binary_NotoSansWarangCiti_Regular_otf: [libc::c_uchar; 20532];
    pub static _binary_NotoSansYi_Regular_otf: [libc::c_uchar; 92164];
    pub static _binary_NotoSerif_Regular_otf: [libc::c_uchar; 288248];
    pub static _binary_NotoSerifAhom_Regular_otf: [libc::c_uchar; 14368];
    pub static _binary_NotoSerifArmenian_Regular_otf: [libc::c_uchar; 13528];
    pub static _binary_NotoSerifBalinese_Regular_otf: [libc::c_uchar; 32436];
    pub static _binary_NotoSerifBengali_Regular_ttf: [libc::c_uchar; 125676];
    pub static _binary_NotoSerifDevanagari_Regular_ttf: [libc::c_uchar; 86828];
    pub static _binary_NotoSerifEthiopic_Regular_otf: [libc::c_uchar; 112248];
    pub static _binary_NotoSerifGeorgian_Regular_otf: [libc::c_uchar; 21928];
    pub static _binary_NotoSerifGujarati_Regular_otf: [libc::c_uchar; 60732];
    pub static _binary_NotoSerifGurmukhi_Regular_otf: [libc::c_uchar; 27036];
    pub static _binary_NotoSerifHebrew_Regular_otf: [libc::c_uchar; 14616];
    pub static _binary_NotoSerifKannada_Regular_otf: [libc::c_uchar; 78648];
    pub static _binary_NotoSerifKhmer_Regular_otf: [libc::c_uchar; 40440];
    pub static _binary_NotoSerifLao_Regular_otf: [libc::c_uchar; 16096];
    pub static _binary_NotoSerifMalayalam_Regular_ttf: [libc::c_uchar; 52644];
    pub static _binary_NotoSerifMyanmar_Regular_otf: [libc::c_uchar; 137296];
    pub static _binary_NotoSerifSinhala_Regular_otf: [libc::c_uchar; 74676];
    pub static _binary_NotoSerifTamil_Regular_otf: [libc::c_uchar; 31300];
    pub static _binary_NotoSerifTelugu_Regular_ttf: [libc::c_uchar; 157544];
    pub static _binary_NotoSerifThai_Regular_otf: [libc::c_uchar; 17160];
    pub static _binary_NotoSerifTibetan_Regular_otf: [libc::c_uchar; 333516];
}

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

        let mut styles = HashMap::new();

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

#[inline]
unsafe fn font_data_from_script(script: HbScript) -> &'static [libc::c_uchar] {
    // Extracted from mupdf in source/fitz/noto.c
    match script {
	HB_SCRIPT_HANGUL |
	HB_SCRIPT_HIRAGANA |
	HB_SCRIPT_KATAKANA |
	HB_SCRIPT_BOPOMOFO |
	HB_SCRIPT_HAN => &_binary_DroidSansFallback_ttf,

	HB_SCRIPT_ARABIC => &_binary_NotoNaskhArabic_Regular_ttf,
	HB_SCRIPT_SYRIAC => &_binary_NotoSansSyriac_Regular_otf,
	HB_SCRIPT_MEROITIC_CURSIVE |
	HB_SCRIPT_MEROITIC_HIEROGLYPHS => &_binary_NotoSansMeroitic_Regular_otf,

	HB_SCRIPT_ADLAM => &_binary_NotoSansAdlam_Regular_otf,
	HB_SCRIPT_AHOM => &_binary_NotoSerifAhom_Regular_otf,
	HB_SCRIPT_ANATOLIAN_HIEROGLYPHS => &_binary_NotoSansAnatolianHieroglyphs_Regular_otf,
	HB_SCRIPT_ARMENIAN => &_binary_NotoSerifArmenian_Regular_otf,
	HB_SCRIPT_AVESTAN => &_binary_NotoSansAvestan_Regular_otf,
	HB_SCRIPT_BALINESE => &_binary_NotoSerifBalinese_Regular_otf,
	HB_SCRIPT_BAMUM => &_binary_NotoSansBamum_Regular_otf,
	HB_SCRIPT_BASSA_VAH => &_binary_NotoSansBassaVah_Regular_otf,
	HB_SCRIPT_BATAK => &_binary_NotoSansBatak_Regular_otf,
	HB_SCRIPT_BENGALI => &_binary_NotoSerifBengali_Regular_ttf,
	HB_SCRIPT_BHAIKSUKI => &_binary_NotoSansBhaiksuki_Regular_otf,
	HB_SCRIPT_BRAHMI => &_binary_NotoSansBrahmi_Regular_otf,
	HB_SCRIPT_BUGINESE => &_binary_NotoSansBuginese_Regular_otf,
	HB_SCRIPT_BUHID => &_binary_NotoSansBuhid_Regular_otf,
	HB_SCRIPT_CANADIAN_SYLLABICS => &_binary_NotoSansCanadianAboriginal_Regular_otf,
	HB_SCRIPT_CARIAN => &_binary_NotoSansCarian_Regular_otf,
	HB_SCRIPT_CAUCASIAN_ALBANIAN => &_binary_NotoSansCaucasianAlbanian_Regular_otf,
	HB_SCRIPT_CHAKMA => &_binary_NotoSansChakma_Regular_otf,
	HB_SCRIPT_CHAM => &_binary_NotoSansCham_Regular_otf,
	HB_SCRIPT_CHEROKEE => &_binary_NotoSansCherokee_Regular_otf,
	HB_SCRIPT_COPTIC => &_binary_NotoSansCoptic_Regular_otf,
	HB_SCRIPT_CUNEIFORM => &_binary_NotoSansCuneiform_Regular_otf,
	HB_SCRIPT_CYPRIOT => &_binary_NotoSansCypriot_Regular_otf,
	HB_SCRIPT_DESERET => &_binary_NotoSansDeseret_Regular_otf,
	HB_SCRIPT_DEVANAGARI => &_binary_NotoSerifDevanagari_Regular_ttf,
	HB_SCRIPT_DUPLOYAN => &_binary_NotoSansDuployan_Regular_otf,
	HB_SCRIPT_EGYPTIAN_HIEROGLYPHS => &_binary_NotoSansEgyptianHieroglyphs_Regular_otf,
	HB_SCRIPT_ELBASAN => &_binary_NotoSansElbasan_Regular_otf,
	HB_SCRIPT_ETHIOPIC => &_binary_NotoSerifEthiopic_Regular_otf,
	HB_SCRIPT_GEORGIAN => &_binary_NotoSerifGeorgian_Regular_otf,
	HB_SCRIPT_GLAGOLITIC => &_binary_NotoSansGlagolitic_Regular_otf,
	HB_SCRIPT_GOTHIC => &_binary_NotoSansGothic_Regular_otf,
	HB_SCRIPT_GRANTHA => &_binary_NotoSansGrantha_Regular_otf,
	HB_SCRIPT_GUJARATI => &_binary_NotoSerifGujarati_Regular_otf,
	HB_SCRIPT_GURMUKHI => &_binary_NotoSerifGurmukhi_Regular_otf,
	HB_SCRIPT_HANUNOO => &_binary_NotoSansHanunoo_Regular_otf,
	HB_SCRIPT_HATRAN => &_binary_NotoSansHatran_Regular_otf,
	HB_SCRIPT_HEBREW => &_binary_NotoSerifHebrew_Regular_otf,
	HB_SCRIPT_IMPERIAL_ARAMAIC => &_binary_NotoSansImperialAramaic_Regular_otf,
	HB_SCRIPT_INSCRIPTIONAL_PAHLAVI => &_binary_NotoSansInscriptionalPahlavi_Regular_otf,
	HB_SCRIPT_INSCRIPTIONAL_PARTHIAN => &_binary_NotoSansInscriptionalParthian_Regular_otf,
	HB_SCRIPT_JAVANESE => &_binary_NotoSansJavanese_Regular_otf,
	HB_SCRIPT_KAITHI => &_binary_NotoSansKaithi_Regular_otf,
	HB_SCRIPT_KANNADA => &_binary_NotoSerifKannada_Regular_otf,
	HB_SCRIPT_KAYAH_LI => &_binary_NotoSansKayahLi_Regular_otf,
	HB_SCRIPT_KHAROSHTHI => &_binary_NotoSansKharoshthi_Regular_otf,
	HB_SCRIPT_KHMER => &_binary_NotoSerifKhmer_Regular_otf,
	HB_SCRIPT_KHOJKI => &_binary_NotoSansKhojki_Regular_otf,
	HB_SCRIPT_KHUDAWADI => &_binary_NotoSansKhudawadi_Regular_otf,
	HB_SCRIPT_LAO => &_binary_NotoSerifLao_Regular_otf,
	HB_SCRIPT_LEPCHA => &_binary_NotoSansLepcha_Regular_otf,
	HB_SCRIPT_LIMBU => &_binary_NotoSansLimbu_Regular_otf,
	HB_SCRIPT_LINEAR_A => &_binary_NotoSansLinearA_Regular_otf,
	HB_SCRIPT_LINEAR_B => &_binary_NotoSansLinearB_Regular_otf,
	HB_SCRIPT_LISU => &_binary_NotoSansLisu_Regular_otf,
	HB_SCRIPT_LYCIAN => &_binary_NotoSansLycian_Regular_otf,
	HB_SCRIPT_LYDIAN => &_binary_NotoSansLydian_Regular_otf,
	HB_SCRIPT_MAHAJANI => &_binary_NotoSansMahajani_Regular_otf,
	HB_SCRIPT_MALAYALAM => &_binary_NotoSerifMalayalam_Regular_ttf,
	HB_SCRIPT_MANDAIC => &_binary_NotoSansMandaic_Regular_otf,
	HB_SCRIPT_MANICHAEAN => &_binary_NotoSansManichaean_Regular_otf,
	HB_SCRIPT_MARCHEN => &_binary_NotoSansMarchen_Regular_otf,
	HB_SCRIPT_MEETEI_MAYEK => &_binary_NotoSansMeeteiMayek_Regular_otf,
	HB_SCRIPT_MENDE_KIKAKUI => &_binary_NotoSansMendeKikakui_Regular_otf,
	HB_SCRIPT_MIAO => &_binary_NotoSansMiao_Regular_otf,
	HB_SCRIPT_MODI => &_binary_NotoSansModi_Regular_otf,
	HB_SCRIPT_MONGOLIAN => &_binary_NotoSansMongolian_Regular_otf,
	HB_SCRIPT_MRO => &_binary_NotoSansMro_Regular_otf,
	HB_SCRIPT_MULTANI => &_binary_NotoSansMultani_Regular_otf,
	HB_SCRIPT_MYANMAR => &_binary_NotoSerifMyanmar_Regular_otf,
	HB_SCRIPT_NABATAEAN => &_binary_NotoSansNabataean_Regular_otf,
	HB_SCRIPT_NEWA => &_binary_NotoSansNewa_Regular_otf,
	HB_SCRIPT_NEW_TAI_LUE => &_binary_NotoSansNewTaiLue_Regular_otf,
	HB_SCRIPT_NKO => &_binary_NotoSansNKo_Regular_otf,
	HB_SCRIPT_OGHAM => &_binary_NotoSansOgham_Regular_otf,
	HB_SCRIPT_OLD_HUNGARIAN => &_binary_NotoSansOldHungarian_Regular_otf,
	HB_SCRIPT_OLD_ITALIC => &_binary_NotoSansOldItalic_Regular_otf,
	HB_SCRIPT_OLD_NORTH_ARABIAN => &_binary_NotoSansOldNorthArabian_Regular_otf,
	HB_SCRIPT_OLD_PERMIC => &_binary_NotoSansOldPermic_Regular_otf,
	HB_SCRIPT_OLD_PERSIAN => &_binary_NotoSansOldPersian_Regular_otf,
	HB_SCRIPT_OLD_SOUTH_ARABIAN => &_binary_NotoSansOldSouthArabian_Regular_otf,
	HB_SCRIPT_OLD_TURKIC => &_binary_NotoSansOldTurkic_Regular_otf,
	HB_SCRIPT_OL_CHIKI => &_binary_NotoSansOlChiki_Regular_otf,
	HB_SCRIPT_ORIYA => &_binary_NotoSansOriya_Regular_ttf,
	HB_SCRIPT_OSAGE => &_binary_NotoSansOsage_Regular_otf,
	HB_SCRIPT_OSMANYA => &_binary_NotoSansOsmanya_Regular_otf,
	HB_SCRIPT_PAHAWH_HMONG => &_binary_NotoSansPahawhHmong_Regular_otf,
	HB_SCRIPT_PALMYRENE => &_binary_NotoSansPalmyrene_Regular_otf,
	HB_SCRIPT_PAU_CIN_HAU => &_binary_NotoSansPauCinHau_Regular_otf,
	HB_SCRIPT_PHAGS_PA => &_binary_NotoSansPhagsPa_Regular_otf,
	HB_SCRIPT_PHOENICIAN => &_binary_NotoSansPhoenician_Regular_otf,
	HB_SCRIPT_PSALTER_PAHLAVI => &_binary_NotoSansPsalterPahlavi_Regular_otf,
	HB_SCRIPT_REJANG => &_binary_NotoSansRejang_Regular_otf,
	HB_SCRIPT_RUNIC => &_binary_NotoSansRunic_Regular_otf,
	HB_SCRIPT_SAMARITAN => &_binary_NotoSansSamaritan_Regular_otf,
	HB_SCRIPT_SAURASHTRA => &_binary_NotoSansSaurashtra_Regular_otf,
	HB_SCRIPT_SHARADA => &_binary_NotoSansSharada_Regular_otf,
	HB_SCRIPT_SHAVIAN => &_binary_NotoSansShavian_Regular_otf,
	HB_SCRIPT_SIDDHAM => &_binary_NotoSansSiddham_Regular_otf,
	HB_SCRIPT_SINHALA => &_binary_NotoSerifSinhala_Regular_otf,
	HB_SCRIPT_SORA_SOMPENG => &_binary_NotoSansSoraSompeng_Regular_otf,
	HB_SCRIPT_SUNDANESE => &_binary_NotoSansSundanese_Regular_otf,
	HB_SCRIPT_SYLOTI_NAGRI => &_binary_NotoSansSylotiNagri_Regular_otf,
	HB_SCRIPT_TAGALOG => &_binary_NotoSansTagalog_Regular_otf,
	HB_SCRIPT_TAGBANWA => &_binary_NotoSansTagbanwa_Regular_otf,
	HB_SCRIPT_TAI_LE => &_binary_NotoSansTaiLe_Regular_otf,
	HB_SCRIPT_TAI_THAM => &_binary_NotoSansTaiTham_Regular_ttf,
	HB_SCRIPT_TAI_VIET => &_binary_NotoSansTaiViet_Regular_otf,
	HB_SCRIPT_TAKRI => &_binary_NotoSansTakri_Regular_otf,
	HB_SCRIPT_TAMIL => &_binary_NotoSerifTamil_Regular_otf,
	HB_SCRIPT_TELUGU => &_binary_NotoSerifTelugu_Regular_ttf,
	HB_SCRIPT_THAANA => &_binary_NotoSansThaana_Regular_otf,
	HB_SCRIPT_THAI => &_binary_NotoSerifThai_Regular_otf,
	HB_SCRIPT_TIBETAN => &_binary_NotoSerifTibetan_Regular_otf,
	HB_SCRIPT_TIFINAGH => &_binary_NotoSansTifinagh_Regular_otf,
	HB_SCRIPT_TIRHUTA => &_binary_NotoSansTirhuta_Regular_otf,
	HB_SCRIPT_UGARITIC => &_binary_NotoSansUgaritic_Regular_otf,
	HB_SCRIPT_VAI => &_binary_NotoSansVai_Regular_otf,
	HB_SCRIPT_WARANG_CITI => &_binary_NotoSansWarangCiti_Regular_otf,
	HB_SCRIPT_YI => &_binary_NotoSansYi_Regular_otf,

	HB_SYMBOL_MATHS => &_binary_NotoSansMath_Regular_otf,
	HB_SYMBOL_MUSIC => &_binary_NotoMusic_Regular_otf,
	HB_SYMBOL_MISC_ONE => &_binary_NotoSansSymbols_Regular_otf,
	HB_SCRIPT_BRAILLE | HB_SYMBOL_MISC_TWO => &_binary_NotoSansSymbols2_Regular_otf,
	HB_SYMBOL_EMOJI => &_binary_NotoEmoji_Regular_ttf,

        _ => &_binary_DroidSansFallback_ttf,
    }
}

#[inline]
fn script_from_code(code: u32) -> HbScript {
    match code {
        0x2032 ..= 0x2037 |
        0x2057 | 0x20D0 ..= 0x20DC | 0x20E1 | 0x20E5 ..= 0x20EF |
        0x2102 | 0x210A ..= 0x210E | 0x2110 ..= 0x2112 |
        0x2115 | 0x2119 ..= 0x211D |
        0x2124 | 0x2128 | 0x212C | 0x212D | 0x212F ..= 0x2131 |
        0x2133 ..= 0x2138 | 0x213C ..= 0x2140 | 0x2145 ..= 0x2149 |
        0x2190 ..= 0x21AE | 0x21B0 ..= 0x21E5 |
        0x21F1 | 0x21F2 | 0x21F4 ..= 0x22FF | 0x2308 ..= 0x230B |
        0x2310 | 0x2319 | 0x231C ..= 0x2321 | 0x2336 ..= 0x237A |
        0x237C | 0x2395 | 0x239B ..= 0x23B6 | 0x23D0 | 0x23DC ..= 0x23E1 |
        0x2474 | 0x2475 | 0x25AF | 0x25B3 | 0x25B7 | 0x25BD | 0x25C1 |
        0x25CA | 0x25CC | 0x25FB | 0x266D ..= 0x266F |
        0x27C0 ..= 0x27FF | 0x2900 ..= 0x2AFF | 0x2B0E ..= 0x2B11 |
        0x2B30 ..= 0x2B4C | 0x2BFE | 0xFF5B | 0xFF5D | 0x1D400 ..= 0x1D454 |
        0x1D456 ..= 0x1D49C | 0x1D49E | 0x1D49F | 0x1D4A2 | 0x1D4A5 |
        0x1D4A6 | 0x1D4A9 ..= 0x1D4AC | 0x1D4AE ..= 0x1D4B9 |
        0x1D4BB | 0x1D4BD ..= 0x1D4C3 | 0x1D4C5 ..= 0x1D505 | 0x1D507 ..= 0x1D50A |
        0x1D50D ..= 0x1D514 | 0x1D516 ..= 0x1D51C | 0x1D51E ..= 0x1D539 |
        0x1D53B ..= 0x1D53E | 0x1D540 ..= 0x1D544 |
        0x1D546 | 0x1D54A ..= 0x1D550 | 0x1D552 ..= 0x1D6A5 |
        0x1D6A8 ..= 0x1D7CB | 0x1D7CE ..= 0x1D7FF | 0x1EE00 ..= 0x1EE03 |
        0x1EE05 ..= 0x1EE1F | 0x1EE21 | 0x1EE22 | 0x1EE24 | 0x1EE27 |
        0x1EE29 ..= 0x1EE32 | 0x1EE34 ..= 0x1EE37 | 0x1EE39 | 0x1EE3B |
        0x1EE42 | 0x1EE47 | 0x1EE49 | 0x1EE4B | 0x1EE4D ..= 0x1EE4F |
        0x1EE51 | 0x1EE52 | 0x1EE54 | 0x1EE57 | 0x1EE59 | 0x1EE5B | 0x1EE5D |
        0x1EE5F | 0x1EE61 | 0x1EE62 | 0x1EE64 | 0x1EE67 ..= 0x1EE6A |
        0x1EE6C ..= 0x1EE72 | 0x1EE74 ..= 0x1EE77 | 0x1EE79 ..= 0x1EE7C |
        0x1EE7E | 0x1EE80 ..= 0x1EE89 | 0x1EE8B ..= 0x1EE9B | 0x1EEA1 ..= 0x1EEA3 |
        0x1EEA5 ..= 0x1EEA9 | 0x1EEAB ..= 0x1EEBB |
        0x1EEF0 | 0x1EEF1 => HB_SYMBOL_MATHS,

        0x1D000 ..= 0x1D0F5 | 0x1D100 ..= 0x1D126 | 0x1D129 ..= 0x1D1E8 |
        0x1D200 ..= 0x1D245 => HB_SYMBOL_MUSIC,

        0x20DD ..= 0x20E0 | 0x20E2 ..= 0x20E4 |
        0x2160 ..= 0x2183 | 0x2185 ..= 0x2188 |
        0x218A | 0x218B |
        0x2300 ..= 0x230F | 0x2311 ..= 0x2315 |
        0x2317 | 0x2322 |
        0x2323 | 0x2329 | 0x232A | 0x232C ..= 0x2335 |
        0x2380 ..= 0x2394 | 0x2396 ..= 0x239A |
        0x23BE ..= 0x23CD | 0x23D0 ..= 0x23DB |
        0x23E2 ..= 0x23E8 | 0x2460 ..= 0x24FF |
        0x260A ..= 0x260D | 0x2613 | 0x2624 ..= 0x262F |
        0x2638 ..= 0x263B | 0x263D ..= 0x2653 | 0x2669 ..= 0x267E |
        0x2690 ..= 0x269D | 0x26A2 ..= 0x26A9 | 0x26AD ..= 0x26BC |
        0x26CE | 0x26E2 ..= 0x26FF | 0x271D ..= 0x2721 |
        0x2776 ..= 0x2793 | 0x1F100 ..= 0x1F10C |
        0x1F110 ..= 0x1F12E | 0x1F130 ..= 0x1F16B |
        0x1F170 ..= 0x1F190 | 0x1F19B ..= 0x1F1AC |
        0x1F546 ..= 0x1F549 | 0x1F54F | 0x1F610 |
        0x1F700 ..= 0x1F773 => HB_SYMBOL_MISC_ONE,

        0x2022 | 0x21AF | 0x21E6 ..= 0x21F0 |
        0x21F3 | 0x2316 | 0x2318 | 0x231A | 0x231B |
        0x2324 ..= 0x2328 | 0x232B | 0x237B | 0x237D ..= 0x237F |
        0x23CE | 0x23CF | 0x23E9 | 0x23EA | 0x23ED ..= 0x23EF |
        0x23F1 ..= 0x23FE | 0x2400 ..= 0x2426 | 0x2440 ..= 0x244A |
        0x25A0 ..= 0x2609 | 0x260E ..= 0x2612 | 0x2614 ..= 0x2623 |
        0x2630 ..= 0x2637 | 0x263C | 0x2654 ..= 0x2668 |
        0x267F ..= 0x268F | 0x269E ..= 0x26A1 | 0x26AA ..= 0x26AC |
        0x26BD ..= 0x26CD | 0x26CF ..= 0x26E1 | 0x2700 ..= 0x2704 |
        0x2706 ..= 0x2709 | 0x270B ..= 0x271C | 0x2722 ..= 0x2727 |
        0x2729 ..= 0x274B | 0x274D | 0x274F ..= 0x2753 | 0x2756 ..= 0x2775 |
        0x2794 | 0x2798 ..= 0x27AF | 0x27B1 ..= 0x27BE | 0x2800 ..= 0x28FF |
        0x2B00 ..= 0x2B0D | 0x2B12 ..= 0x2B2F |
        0x2B4D ..= 0x2B73 | 0x2B76 ..= 0x2B95 | 0x2B98 ..= 0x2BB9 |
        0x2BBD ..= 0x2BC8 | 0x2BCA ..= 0x2BD1 | 0x2BEC ..= 0x2BEF |
        0x4DC0 ..= 0x4DFF | 0xFFF9 ..= 0xFFFB | 0x10140 ..= 0x1018E |
        0x10190 ..= 0x1019B | 0x101A0 | 0x101D0 ..= 0x101FD |
        0x102E0 ..= 0x102FB | 0x10E60 ..= 0x10E7E | 0x1D300 ..= 0x1D356 |
        0x1D360 ..= 0x1D371 | 0x1F000 ..= 0x1F02B | 0x1F030 ..= 0x1F093 |
        0x1F0A0 ..= 0x1F0AE | 0x1F0B1 ..= 0x1F0BF | 0x1F0C1 ..= 0x1F0CF |
        0x1F0D1 ..= 0x1F0F5 | 0x1F30D ..= 0x1F30F | 0x1F315 | 0x1F31C |
        0x1F321 ..= 0x1F32C | 0x1F336 | 0x1F378 | 0x1F37D |
        0x1F393 ..= 0x1F39F | 0x1F3A7 | 0x1F3AC ..= 0x1F3AE | 0x1F3C2 |
        0x1F3C4 | 0x1F3C6 | 0x1F3CA ..= 0x1F3CE | 0x1F3D4 ..= 0x1F3E0 |
        0x1F3ED | 0x1F3F1 ..= 0x1F3F3 | 0x1F3F5 ..= 0x1F3F7 | 0x1F408 |
        0x1F415 | 0x1F41F | 0x1F426 | 0x1F43F | 0x1F441 | 0x1F442 |
        0x1F446 ..= 0x1F449 | 0x1F44C ..= 0x1F44E | 0x1F453 | 0x1F46A |
        0x1F47D | 0x1F4A3 | 0x1F4B0 | 0x1F4B3 | 0x1F4B9 | 0x1F4BB |
        0x1F4BF | 0x1F4C8 ..= 0x1F4CB | 0x1F4DA | 0x1F4DF |
        0x1F4E4 ..= 0x1F4E6 | 0x1F4EA ..= 0x1F4ED | 0x1F4F7 |
        0x1F4F9 ..= 0x1F4FB | 0x1F4FD | 0x1F4FE | 0x1F503 |
        0x1F507 ..= 0x1F50A | 0x1F50D | 0x1F512 | 0x1F513 |
        0x1F53E ..= 0x1F545 | 0x1F54A | 0x1F550 ..= 0x1F579 |
        0x1F57B ..= 0x1F594 | 0x1F597 ..= 0x1F5A3 | 0x1F5A5 ..= 0x1F5FA |
        0x1F650 ..= 0x1F67F | 0x1F687 | 0x1F68D | 0x1F691 | 0x1F694 |
        0x1F698 | 0x1F6AD | 0x1F6B2 | 0x1F6B9 | 0x1F6BA | 0x1F6BC |
        0x1F6C6 ..= 0x1F6CB | 0x1F6CD ..= 0x1F6CF | 0x1F6E0 ..= 0x1F6EA |
        0x1F6F0 ..= 0x1F6F3 | 0x1F780 ..= 0x1F7D4 | 0x1F800 ..= 0x1F80B |
        0x1F810 ..= 0x1F847 | 0x1F850 ..= 0x1F859 | 0x1F860 ..= 0x1F887 |
        0x1F890 ..= 0x1F8AD | 0x1F93B | 0x1F946 => HB_SYMBOL_MISC_TWO,

        0x2049 | 0x2122 | 0x2139 | 0x23EA ..= 0x23EC | 0x23F0 |
        0x2705 | 0x2708 ..= 0x270C | 0x2728 | 0x274C | 0x274E |
        0x2753 ..= 0x2755 | 0x2795 ..= 0x2797 |
        0x27B0 | 0x27BF | 0x3030 | 0x303D | 0x3297 |
        0x3299 | 0xFEFF | 0x1F191 ..= 0x1F19A | 0x1F1E6 ..= 0x1F1FF |
        0x1F201 | 0x1F202 | 0x1F21A | 0x1F22F | 0x1F232 ..= 0x1F23A |
        0x1F250 | 0x1F251 | 0x1F300 ..= 0x1F320 | 0x1F330 ..= 0x1F335 |
        0x1F337 ..= 0x1F37C | 0x1F380 ..= 0x1F393 | 0x1F3A0 ..= 0x1F3C4 |
        0x1F3C6 ..= 0x1F3CA | 0x1F3E0 ..= 0x1F3F0 | 0x1F400 ..= 0x1F429 |
        0x1F42B ..= 0x1F43E | 0x1F440 | 0x1F442 ..= 0x1F4F7 |
        0x1F4F9 ..= 0x1F4FC | 0x1F500 ..= 0x1F53D | 0x1F5FB ..= 0x1F640 |
        0x1F645 ..= 0x1F64F | 0x1F680 ..= 0x1F697 | 0x1F699 ..= 0x1F6C5 |
        0xFE4E5 ..= 0xFE4EE | 0xFE82C | 0xFE82E ..= 0xFE837 => HB_SYMBOL_EMOJI,

        _ => HB_SCRIPT_UNKNOWN,
    }
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

    #[inline]
    unsafe fn patch(&mut self, txt: &str, features: &[HbFeature], render_plan: &mut RenderPlan, missing_glyphs: Vec<(usize, usize)>, buf: *mut HbBuffer) {
        let mut drift = 0;
        for (mut start, mut end) in missing_glyphs.into_iter() {
            start = (start as i32 + drift).max(0) as usize;
            end = (end as i32 + drift).max(0) as usize;
            hb_buffer_clear_contents(buf);
            let start_index = render_plan.glyphs[start].cluster;
            let end_index = render_plan.glyphs.get(end).map(|g| g.cluster)
                                       .unwrap_or_else(|| txt.len());
            let chunk = &txt[start_index..end_index];
            hb_buffer_add_utf8(buf, chunk.as_ptr() as *const libc::c_char,
                               chunk.len() as libc::c_int, 0, -1);
            hb_buffer_guess_segment_properties(buf);
            let mut script = hb_buffer_get_script(buf);
            if script == HB_SCRIPT_INVALID || script == HB_SCRIPT_UNKNOWN {
                if let Some(c) = chunk.chars().next() {
                    script = script_from_code(u32::from(c));
                }
            }
            let font_data = font_data_from_script(script);
            let mut face = ptr::null_mut();
            FT_New_Memory_Face((self.lib).0, font_data.as_ptr() as *const FtByte,
                               font_data.len() as libc::c_long, 0, &mut face);
            FT_Set_Pixel_Sizes(face, (*(*self.face).size).metrics.x_ppem as libc::c_uint, 0);
            let font = hb_ft_font_create(face, ptr::null());
            hb_shape(font, buf, features.as_ptr(), features.len() as libc::c_uint);
            let len = hb_buffer_get_length(buf) as usize;
            let info = hb_buffer_get_glyph_infos(buf, ptr::null_mut());
            let pos = hb_buffer_get_glyph_positions(buf, ptr::null_mut());
            let mut glyphs = Vec::with_capacity(len);

            for i in 0..len {
                let pos_i = &*pos.add(i);
                let info_i = &*info.add(i);
                render_plan.width += (pos_i.x_advance >> 6) as u32;
                glyphs.push(GlyphPlan {
                    codepoint: info_i.codepoint,
                    cluster: start_index + info_i.cluster as usize,
                    advance: pt!(pos_i.x_advance >> 6, pos_i.y_advance >> 6),
                    offset: pt!(pos_i.x_offset >> 6, -pos_i.y_offset >> 6),
                });
                render_plan.scripts.insert(start+i, script);
            }

            render_plan.glyphs.splice(start..end, glyphs.into_iter());
            drift += len as i32 - (end - start) as i32;

            hb_font_destroy(font);
            FT_Done_Face(face);
        }
    }

    pub fn plan(&mut self, txt: &str, max_width: Option<u32>, features: Option<&[String]>) -> RenderPlan {
        unsafe {
            let buf = hb_buffer_create();
            hb_buffer_add_utf8(buf, txt.as_ptr() as *const libc::c_char,
                               txt.len() as libc::c_int, 0, -1);

            // If the direction is RTL, the clusters are given in reverse order.
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
 
            let len = hb_buffer_get_length(buf) as usize;
            let info = hb_buffer_get_glyph_infos(buf, ptr::null_mut());
            let pos = hb_buffer_get_glyph_positions(buf, ptr::null_mut());
            let mut render_plan = RenderPlan::default();
            let mut missing_glyphs = Vec::new();

            for i in 0..len {
                let pos_i = &*pos.add(i);
                let info_i = &*info.add(i);
                if info_i.codepoint == 0 {
                    if let Some((start, end)) = missing_glyphs.pop() {
                        if i == end {
                            missing_glyphs.push((start, end+1));
                        } else {
                            missing_glyphs.push((start, end));
                            missing_glyphs.push((i, i+1));
                        }
                    } else {
                        missing_glyphs.push((i, i+1));
                    }
                } else {
                    render_plan.width += (pos_i.x_advance >> 6) as u32;
                }
                let glyph = GlyphPlan {
                    codepoint: info_i.codepoint,
                    cluster: info_i.cluster as usize,
                    advance: pt!(pos_i.x_advance >> 6, pos_i.y_advance >> 6),
                    offset: pt!(pos_i.x_offset >> 6, -pos_i.y_offset >> 6),
                };
                render_plan.glyphs.push(glyph);
            }

            self.patch(txt, &features_vec, &mut render_plan, missing_glyphs, buf);

            hb_buffer_destroy(buf);

            if let Some(mw) = max_width {
                self.crop_right(&mut render_plan, mw);
            }

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

        let len = render_plan.glyphs.len();
        render_plan.scripts.retain(|&k, _| k < len);
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

        render_plan.scripts.retain(|&k, _| k >= lower_index.max(0) as usize && k <= upper_index);
        if lower_index > 0 {
            render_plan.scripts = render_plan.scripts.drain()
                                             .map(|(k, v)| (k - lower_index as usize + 1, v)).collect();
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

    pub fn render(&mut self, fb: &mut dyn Framebuffer, color: u8, render_plan: &RenderPlan, origin: Point) {
        unsafe {
            let mut pos = origin;
            let mut fallback_faces = HashMap::new();

            for (index, glyph) in render_plan.glyphs.iter().enumerate() {
                let face = if let Some(script) = render_plan.scripts.get(&index) {
                    *fallback_faces.entry(script).or_insert_with(|| {
                        let font_data = font_data_from_script(*script);
                        let mut face = ptr::null_mut();
                        FT_New_Memory_Face((self.lib).0, font_data.as_ptr() as *const FtByte,
                                           font_data.len() as libc::c_long, 0, &mut face);
                        FT_Set_Pixel_Sizes(face, (*(*self.face).size).metrics.x_ppem as libc::c_uint, 0);
                        face
                    })
                } else {
                    self.face
                };

                FT_Load_Glyph(face, glyph.codepoint, FT_LOAD_RENDER | FT_LOAD_NO_HINTING);

                let glyph_slot = (*face).glyph;
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

            let fallback_faces: BTreeSet<*mut FtFace> = fallback_faces.into_iter().map(|(_, v)| v).collect();
            for face in fallback_faces.into_iter() {
                FT_Done_Face(face);
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
struct GlyphPlan {
    codepoint: u32,
    cluster: usize,
    offset: Point,
    advance: Point,
}

#[derive(Debug, Clone)]
pub struct RenderPlan {
    pub width: u32,
    scripts: HashMap<usize, HbScript>,
    glyphs: Vec<GlyphPlan>,
}

impl Default for RenderPlan {
    fn default() -> RenderPlan {
        RenderPlan {
            width: 0,
            scripts: HashMap::new(),
            glyphs: vec![],
        }
    }
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

    pub fn split_off(&mut self, index: usize, width: u32) -> RenderPlan {
        let mut next_scripts = HashMap::new();
        if !self.scripts.is_empty() {
            for i in index..self.glyphs.len() {
                self.scripts.remove_entry(&i)
                    .map(|(k, v)| next_scripts.insert(k - index, v));
            }
        }
        let next_glyphs = self.glyphs.split_off(index);
        let next_width = self.width - width;
        self.width = width;
        RenderPlan {
            width: next_width,
            scripts: next_scripts,
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

    pub fn append(&mut self, other: &mut Self) {
        let next_index = self.glyphs.len();
        self.scripts.extend(other.scripts.iter().map(|(k, v)| (next_index + k, *v)));
        self.glyphs.append(&mut other.glyphs);
        self.width += other.width;
    }

    pub fn total_advance(&self, index: usize) -> i32 {
        self.glyphs.iter().take(index).map(|g| g.advance.x).sum()
    }

    #[inline]
    pub fn glyph_advance(&self, index: usize) -> i32 {
        self.glyphs[index].advance.x
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

#[inline]
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
