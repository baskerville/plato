mod standard;
mod natural;

#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
pub enum Color {
    White,
    Red,
    Green,
}

pub trait FrontLight {
    fn get(&self, color: Color) -> f32;
    fn set(&mut self, color: Color, value: f32);
}
