const MILLIMETERS_PER_INCH: f32 = 25.4;
const POINTS_PER_INCH: f32 = 72.0;
const BASE_DPI: f32 = 300.0;

#[inline]
pub fn mm_to_in(mm: f32) -> f32 {
    mm / MILLIMETERS_PER_INCH
}

#[inline]
pub fn scale_by_dpi_raw(x: f32, dpi: u16) -> f32 {
    x * (dpi as f32) / BASE_DPI
}

#[inline]
pub fn scale_by_dpi(x: f32, dpi: u16) -> f32 {
    scale_by_dpi_raw(x, dpi).round().max(1.0)
}
