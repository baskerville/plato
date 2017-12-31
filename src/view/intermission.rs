use device::CURRENT_DEVICE;
use document::pdf::PdfOpener;
use geom::Rectangle;
use font::{Fonts, font_from_style, DISPLAY_STYLE};
use view::{View, Event, Hub, Bus};
use framebuffer::Framebuffer;
use color::{TEXT_NORMAL, TEXT_INVERTED_HARD};
use app::Context;

pub struct Intermission {
    rect: Rectangle,
    children: Vec<Box<View>>,
    text: String,
    halt: bool,
}

impl Intermission {
    pub fn new(rect: Rectangle, text: String, halt: bool) -> Intermission {
        Intermission {
            rect,
            children: vec![],
            text,
            halt,
        }
    }
}

impl View for Intermission {
    fn handle_event(&mut self, evt: &Event, hub: &Hub, bus: &mut Bus, _context: &mut Context) -> bool {
        true
    }

    fn render(&self, fb: &mut Framebuffer, fonts: &mut Fonts) {
        let dpi = CURRENT_DEVICE.dpi;
        
        let scheme = if self.halt {
            TEXT_INVERTED_HARD
        } else {
            TEXT_NORMAL
        };

        let font = font_from_style(fonts, &DISPLAY_STYLE, dpi);
        let padding = font.em();
        let max_width = self.rect.width() - 3 * padding as u32;
        let mut plan = font.plan(&self.text, None, None);

        if plan.width > max_width {
            let scale = max_width as f32 / plan.width as f32;
            let size = (scale * DISPLAY_STYLE.size as f32) as u32;
            font.set_size(size, dpi);
            plan = font.plan(&self.text, None, None);
        }

        let x_height = font.x_heights.0 as i32;

        let dx = (self.rect.width() - plan.width) as i32 / 2;
        let dy = (self.rect.height() as i32) / 3;

        fb.draw_rectangle(&self.rect, scheme[0]);

        font.render(fb, scheme[1], &plan, &pt!(dx, dy));

        let doc = PdfOpener::new().and_then(|o| o.open("icons/dodecahedron.svg")).unwrap();
        let page = doc.page(0).unwrap();
        let (width, height) = page.dims();
        let scale = (plan.width as f32 / width.max(height) as f32) / 4.0;
        let pixmap = page.pixmap(scale).unwrap();
        let dx = (self.rect.width() as i32 - pixmap.width) / 2;
        let dy = dy + 2 * x_height;
        let pt = self.rect.min + pt!(dx, dy);

        fb.draw_blended_pixmap(&pixmap, &pt, scheme[1]);
    }

    fn rect(&self) -> &Rectangle {
        &self.rect
    }

    fn rect_mut(&mut self) -> &mut Rectangle {
        &mut self.rect
    }

    fn children(&self) -> &Vec<Box<View>> {
        &self.children
    }

    fn children_mut(&mut self) -> &mut Vec<Box<View>> {
        &mut self.children
    }
}
