pub const MILLIMETERS_PER_INCH: f32 = 25.4;
pub const CENTIMETERS_PER_INCH: f32 = 2.54;
pub const POINTS_PER_INCH: f32 = 72.0;
pub const PICAS_PER_INCH: f32 = 6.0;
const BASE_DPI: f32 = 300.0;

#[inline]
pub fn pt_to_px(pt: f32, dpi: u16) -> f32 {
    pt * (dpi as f32 / POINTS_PER_INCH)
}

#[inline]
pub fn pc_to_px(pc: f32, dpi: u16) -> f32 {
    pc * (dpi as f32 / PICAS_PER_INCH)
}

#[inline]
pub fn in_to_px(inc: f32, dpi: u16) -> f32 {
    inc * (dpi as f32)
}

#[inline]
pub fn mm_to_px(mm: f32, dpi: u16) -> f32 {
    mm * (dpi as f32 / MILLIMETERS_PER_INCH)
}

#[inline]
pub fn scale_by_dpi_raw(x: f32, dpi: u16) -> f32 {
    x * (dpi as f32) / BASE_DPI
}

#[inline]
pub fn scale_by_dpi(x: f32, dpi: u16) -> f32 {
    scale_by_dpi_raw(x, dpi).round().max(1.0)
}
