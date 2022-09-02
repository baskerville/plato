use crate::framebuffer::{Framebuffer, UpdateMode, Pixmap};
use crate::view::{View, Event, Hub, Bus, Id, ID_FEEDER, RenderQueue, RenderData};
use crate::color::WHITE;
use crate::geom::Rectangle;
use crate::context::Context;
use crate::font::Fonts;

pub struct Image {
    id: Id,
    rect: Rectangle,
    children: Vec<Box<dyn View>>,
    pixmap: Pixmap,
}

impl Image {
    pub fn new(rect: Rectangle, pixmap: Pixmap) -> Image {
        Image {
            id: ID_FEEDER.next(),
            rect,
            children: Vec::new(),
            pixmap,
        }
    }

    pub fn update(&mut self, pixmap: Pixmap, rq: &mut RenderQueue) {
        self.pixmap = pixmap;
        rq.add(RenderData::new(self.id, self.rect, UpdateMode::Gui));
    }
}

impl View for Image {
    fn handle_event(&mut self, _evt: &Event, _hub: &Hub, _bus: &mut Bus, _rq: &mut RenderQueue, _context: &mut Context) -> bool {
        false
    }

    fn render(&self, fb: &mut dyn Framebuffer, rect: Rectangle, _fonts: &mut Fonts) {
        let x0 = self.rect.min.x + (self.rect.width() - self.pixmap.width) as i32 / 2;
        let y0 = self.rect.min.y + (self.rect.height() - self.pixmap.height) as i32 / 2;
        let x1 = x0 + self.pixmap.width as i32;
        let y1 = y0 + self.pixmap.height as i32;
        if let Some(r) = rect![self.rect.min, pt!(x1, y0)].intersection(&rect) {
            fb.draw_rectangle(&r, WHITE);
        }
        if let Some(r) = rect![self.rect.min.x, y0, x0, self.rect.max.y].intersection(&rect) {
            fb.draw_rectangle(&r, WHITE);
        }
        if let Some(r) = rect![pt!(x0, y1), self.rect.max].intersection(&rect) {
            fb.draw_rectangle(&r, WHITE);
        }
        if let Some(r) = rect![x1, self.rect.min.y, self.rect.max.x, y1].intersection(&rect) {
            fb.draw_rectangle(&r, WHITE);
        }
        if let Some(r) = rect![x0, y0, x1, y1].intersection(&rect) {
            let frame = r - pt!(x0, y0);
            fb.draw_framed_pixmap(&self.pixmap, &frame, r.min);
        }
    }

    fn render_rect(&self, rect: &Rectangle) -> Rectangle {
        rect.intersection(&self.rect)
            .unwrap_or(self.rect)
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
