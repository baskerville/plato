pub const GRAY00: u8 = 00 << 4;
pub const GRAY01: u8 = 01 << 4;
pub const GRAY02: u8 = 02 << 4;
pub const GRAY03: u8 = 03 << 4;
pub const GRAY04: u8 = 04 << 4;
pub const GRAY05: u8 = 05 << 4;
pub const GRAY06: u8 = 06 << 4;
pub const GRAY07: u8 = 07 << 4;
pub const GRAY08: u8 = 08 << 4;
pub const GRAY09: u8 = 09 << 4;
pub const GRAY10: u8 = 10 << 4;
pub const GRAY11: u8 = 11 << 4;
pub const GRAY12: u8 = 12 << 4;
pub const GRAY13: u8 = 13 << 4;
pub const GRAY14: u8 = 14 << 4;
pub const GRAY15: u8 = 15 << 4;
pub const BLACK: u8 = GRAY00;
pub const WHITE: u8 = GRAY15;

pub const TEXT_NORMAL: [u8; 3] = [WHITE, BLACK, GRAY06];
pub const TEXT_BUMP_SMALL: [u8; 3] = [GRAY14, BLACK, GRAY05];
pub const TEXT_BUMP_BIG: [u8; 3] = [GRAY11, BLACK, BLACK];
pub const TEXT_INVERTED: [u8; 3] = [GRAY05, WHITE, WHITE];

pub const KEYBOARD_BG: u8 = GRAY13;
pub const SEPARATOR_NORMAL: u8 = GRAY10;
pub const SEPARATOR_STRONG: u8 = GRAY07;

pub const PROGRESS_FULL: u8 = GRAY06;
pub const PROGRESS_BORDER: u8 = GRAY03;
pub const PROGRESS_EMPTY: u8 = GRAY13;

pub const INPUT_BORDER: u8 = GRAY04;
