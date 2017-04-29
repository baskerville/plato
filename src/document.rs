use std::collections::HashMap;

pub struct Rect;
pub type Metadata = HashMap<String, String>;
pub struct Toc;

pub trait Document<T: Page> {
    fn pages(&self) -> Vec<T>;
    fn metadata(&self) -> Metadata;
    fn toc(&self) -> Option<Toc>;
}

pub trait Page {
    fn render(&self, rect: Rect, &mut [u8]);
    fn text(&self) -> Option<String>;
}
