use std::path::PathBuf;
use crate::device::CURRENT_DEVICE;
use crate::document::{Location, open};
use crate::geom::Rectangle;
use crate::font::{Fonts, font_from_style, DISPLAY_STYLE};
use super::{View, Event, Hub, Bus, Id, ID_FEEDER, RenderQueue};
use crate::framebuffer::Framebuffer;
use crate::settings::{IntermKind, LOGO_SPECIAL_PATH, COVER_SPECIAL_PATH};
use crate::metadata::{SortMethod, BookQuery, sort};
use crate::color::{TEXT_NORMAL, TEXT_INVERTED_HARD};
use crate::context::Context;

pub struct Intermission {
    id: Id,
    rect: Rectangle,
    children: Vec<Box<dyn View>>,
    message: Message,
    halt: bool,
}

pub enum Message {
    Text(String),
    Image(PathBuf),
    Cover(PathBuf),
}

impl Intermission {
    pub fn new(rect: Rectangle, kind: IntermKind, context: &Context) -> Intermission {
        let path = &context.settings.intermissions[kind];
        let message = match path.to_str() {
            Some(LOGO_SPECIAL_PATH) => Message::Text(kind.text().to_string()),
            Some(COVER_SPECIAL_PATH) => {
                let query = BookQuery {
                    reading: Some(true),
                    .. Default::default()
                };
                let (mut files, _) = context.library.list(&context.library.home, Some(&query), false);
                sort(&mut files, SortMethod::Opened, true);
                if !files.is_empty() {
                    Message::Cover(context.library.home.join(&files[0].file.path))
                } else {
                    Message::Text(kind.text().to_string())
                }
            },
            _ => Message::Image(path.clone()),
        };
        Intermission {
            id: ID_FEEDER.next(),
            rect,
            children: Vec::new(),
            message,
            halt: kind == IntermKind::PowerOff,
        }
    }
}

impl View for Intermission {
    fn handle_event(&mut self, _evt: &Event, _hub: &Hub, _bus: &mut Bus, _rq: &mut RenderQueue, _context: &mut Context) -> bool {
        true
    }

    fn render(&self, fb: &mut dyn Framebuffer, _rect: Rectangle, fonts: &mut Fonts) {
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
                let padding = font.em() as i32;
                let max_width = self.rect.width() as i32 - 3 * padding;
                let mut plan = font.plan(text, None, None);

                if plan.width > max_width {
                    let scale = max_width as f32 / plan.width as f32;
                    let size = (scale * DISPLAY_STYLE.size as f32) as u32;
                    font.set_size(size, dpi);
                    plan = font.plan(text, None, None);
                }

                let x_height = font.x_heights.0 as i32;

                let dx = (self.rect.width() as i32 - plan.width) / 2;
                let dy = (self.rect.height() as i32) / 3;

                font.render(fb, scheme[1], &plan, pt!(dx, dy));

                let mut doc = open("icons/dodecahedron.svg").unwrap();
                let (width, height) = doc.dims(0).unwrap();
                let scale = (plan.width as f32 / width.max(height) as f32) / 4.0;
                let (pixmap, _) = doc.pixmap(Location::Exact(0), scale).unwrap();
                let dx = (self.rect.width() as i32 - pixmap.width as i32) / 2;
                let dy = dy + 2 * x_height;
                let pt = self.rect.min + pt!(dx, dy);

                fb.draw_blended_pixmap(&pixmap, pt, scheme[1]);
            },
            Message::Image(ref path) => {
                if let Some(mut doc) = open(path) {
                    if let Some((width, height)) = doc.dims(0) {
                        let w_ratio = self.rect.width() as f32 / width;
                        let h_ratio = self.rect.height() as f32 / height;
                        let scale = w_ratio.min(h_ratio);
                        if let Some((pixmap, _)) = doc.pixmap(Location::Exact(0), scale) {
                            let dx = (self.rect.width() as i32 - pixmap.width as i32) / 2;
                            let dy = (self.rect.height() as i32 - pixmap.height as i32) / 2;
                            let pt = self.rect.min + pt!(dx, dy);
                            fb.draw_pixmap(&pixmap, pt);
                            if fb.inverted() {
                                let rect = pixmap.rect() + pt;
                                fb.invert_region(&rect);
                            }
                        }
                    }
                }
            },
            Message::Cover(ref path) => {
                if let Some(mut doc) = open(path) {
                    if let Some(pixmap) = doc.preview_pixmap(self.rect.width() as f32, self.rect.height() as f32) {
                        let dx = (self.rect.width() as i32 - pixmap.width as i32) / 2;
                        let dy = (self.rect.height() as i32 - pixmap.height as i32) / 2;
                        let pt = self.rect.min + pt!(dx, dy);
                        fb.draw_pixmap(&pixmap, pt);
                        if fb.inverted() {
                            let rect = pixmap.rect() + pt;
                            fb.invert_region(&rect);
                        }
                    }
                }
            },
        }
    }

    fn might_rotate(&self) -> bool {
        false
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

    fn id(&self) -> Id {
        self.id
    }
}
