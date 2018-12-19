use std::path::PathBuf;
use crate::device::CURRENT_DEVICE;
use crate::document::pdf::PdfOpener;
use crate::geom::Rectangle;
use crate::font::{Fonts, font_from_style, DISPLAY_STYLE};
use super::{View, Event, Hub, Bus};
use crate::framebuffer::Framebuffer;
use crate::color::{TEXT_NORMAL, TEXT_INVERTED_HARD};
use crate::app::Context;

pub struct Intermission {
    rect: Rectangle,
    children: Vec<Box<dyn View>>,
    message: Message,
    halt: bool,
}

pub enum Message {
    Text(String),
    Image(PathBuf),
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum IntermKind {
    Suspend,
    PowerOff,
    Share,
}

impl IntermKind {
    pub fn text(&self) -> &str {
        match self {
            IntermKind::Suspend => "Sleeping",
            IntermKind::PowerOff => "Powered off",
            IntermKind::Share => "Shared",
        }
    }

    pub fn label(&self) -> &str {
        match self {
            IntermKind::Suspend => "Suspend Image",
            IntermKind::PowerOff => "Power Off Image",
            IntermKind::Share => "Share Image",
        }
    }

    pub fn key(&self) -> &str {
        match self {
            IntermKind::Suspend => "suspend",
            IntermKind::PowerOff => "power-off",
            IntermKind::Share => "share",
        }
    }
}


impl Intermission {
    pub fn new(rect: Rectangle, kind: IntermKind, context: &Context) -> Intermission {
        let message = if let Some(path) = context.settings.intermission_images.get(kind.key()) {
            if path.is_relative() {
                Message::Image(context.settings.library_path.join(path))
            } else {
                Message::Image(path.clone())
            }
        } else {
            Message::Text(kind.text().to_string())
        };
        Intermission {
            rect,
            children: vec![],
            message,
            halt: kind == IntermKind::PowerOff,
        }
    }
}

impl View for Intermission {
    fn handle_event(&mut self, _evt: &Event, _hub: &Hub, _bus: &mut Bus, _context: &mut Context) -> bool {
        true
    }

    fn render(&self, fb: &mut Framebuffer, fonts: &mut Fonts) {
        let scheme = if self.halt {
            TEXT_INVERTED_HARD
        } else {
            TEXT_NORMAL
        };

        fb.draw_rectangle(&self.rect, scheme[0]);

        match self.message {
            Message::Text(ref text) => {
                let dpi = CURRENT_DEVICE.dpi;

                let font = font_from_style(fonts, &DISPLAY_STYLE, dpi);
                let padding = font.em();
                let max_width = self.rect.width() - 3 * padding as u32;
                let mut plan = font.plan(text, None, None);

                if plan.width > max_width {
                    let scale = max_width as f32 / plan.width as f32;
                    let size = (scale * DISPLAY_STYLE.size as f32) as u32;
                    font.set_size(size, dpi);
                    plan = font.plan(text, None, None);
                }

                let x_height = font.x_heights.0 as i32;

                let dx = (self.rect.width() - plan.width) as i32 / 2;
                let dy = (self.rect.height() as i32) / 3;

                font.render(fb, scheme[1], &plan, pt!(dx, dy));

                let doc = PdfOpener::new().and_then(|o| o.open("icons/dodecahedron.svg")).unwrap();
                let page = doc.page(0).unwrap();
                let (width, height) = page.dims();
                let scale = (plan.width as f32 / width.max(height) as f32) / 4.0;
                let pixmap = page.pixmap(scale).unwrap();
                let dx = (self.rect.width() as i32 - pixmap.width as i32) / 2;
                let dy = dy + 2 * x_height;
                let pt = self.rect.min + pt!(dx, dy);

                fb.draw_blended_pixmap(&pixmap, &pt, scheme[1]);
            },
            Message::Image(ref path) => {
                if let Some(doc) = PdfOpener::new().and_then(|o| o.open(path)) {
                    if let Some(page) = doc.page(0) {
                        let (width, height) = page.dims();
                        let w_ratio = self.rect.width() as f32 / width;
                        let h_ratio = self.rect.height() as f32 / height;
                        let scale = w_ratio.min(h_ratio);
                        if let Some(pixmap) = page.pixmap(scale) {
                            let dx = (self.rect.width() as i32 - pixmap.width as i32) / 2;
                            let dy = (self.rect.height() as i32 - pixmap.height as i32) / 2;
                            let pt = self.rect.min + pt!(dx, dy);
                            fb.draw_pixmap(&pixmap, &pt);
                        }
                    }
                }
            },
        }
    }

    fn rect(&self) -> &Rectangle {
        &self.rect
    }

    fn rect_mut(&mut self) -> &mut Rectangle {
        &mut self.rect
    }

    fn children(&self) -> &Vec<Box<dyn View>> {
        &self.children
    }

    fn children_mut(&mut self) -> &mut Vec<Box<dyn View>> {
        &mut self.children
    }
}
