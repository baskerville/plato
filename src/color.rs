#![allow(unused)]

pub const GRAY00: u8 = 0x00;
pub const GRAY01: u8 = 0x11;
pub const GRAY02: u8 = 0x22;
pub const GRAY03: u8 = 0x33;
pub const GRAY04: u8 = 0x44;
pub const GRAY05: u8 = 0x55;
pub const GRAY06: u8 = 0x66;
pub const GRAY07: u8 = 0x77;
pub const GRAY08: u8 = 0x88;
pub const GRAY09: u8 = 0x99;
pub const GRAY10: u8 = 0xAA;
pub const GRAY11: u8 = 0xBB;
pub const GRAY12: u8 = 0xCC;
pub const GRAY13: u8 = 0xDD;
pub const GRAY14: u8 = 0xEE;
pub const GRAY15: u8 = 0xFF;

pub const BLACK: u8 = GRAY00;
pub const WHITE: u8 = GRAY15;

pub const TEXT_NORMAL: [u8; 3] = [WHITE, BLACK, GRAY08];
pub const TEXT_BUMP_SMALL: [u8; 3] = [GRAY14, BLACK, GRAY07];
pub const TEXT_BUMP_LARGE: [u8; 3] = [GRAY11, BLACK, BLACK];

pub const TEXT_INVERTED_SOFT: [u8; 3] = [GRAY05, WHITE, WHITE];
pub const TEXT_INVERTED_HARD: [u8; 3] = [BLACK, WHITE, GRAY06];

pub const SEPARATOR_NORMAL: u8 = GRAY10;
pub const SEPARATOR_STRONG: u8 = GRAY07;

pub const KEYBOARD_BG: u8 = GRAY12;
pub const BATTERY_FILL: u8 = GRAY12;
pub const READING_PROGRESS: u8 = GRAY07;

pub const PROGRESS_FULL: u8 = GRAY05;
pub const PROGRESS_EMPTY: u8 = GRAY13;
pub const PROGRESS_VALUE: u8 = GRAY06;
