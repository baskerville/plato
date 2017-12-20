mod standard;
mod natural;

pub use self::standard::StandardLight;
pub use self::natural::NaturalLight;

#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub enum Color {
    White,
    Red,
    Green,
}

pub trait FrontLight {
    fn get(&self, color: Color) -> f32;
    fn set(&mut self, color: Color, value: f32);
}
