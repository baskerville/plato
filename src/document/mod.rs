use std::collections::HashMap;
use geom::Rectangle;

pub mod djvu;

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum LayerGrain {
    Page,
    Column,
    Region,
    Paragraph,
    Line,
    Word,
    Character,
}

#[derive(Debug, Clone)]
pub struct TextLayer {
    grain: LayerGrain,
    rect: Rectangle,
    text: Option<String>,
    children: Vec<TextLayer>,
}

#[derive(Debug, Clone)]
pub struct TocEntry {
    title: String,
    page: usize,
    children: Vec<TocEntry>,
}
