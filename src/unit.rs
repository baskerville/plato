const MILLIMETERS_PER_INCH: f32 = 25.4;

#[inline]
pub fn mm_to_in(mm: f32) -> f32 {
    mm / MILLIMETERS_PER_INCH
}
