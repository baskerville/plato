use framebuffer::Framebuffer;
use view::{View, Event, Hub, Bus, SliderId, ViewId, Align};
use view::filler::Filler;
use view::slider::Slider;
use view::icon::Icon;
use gesture::GestureEvent;
use geom::Rectangle;
use font::{Fonts, DEFAULT_FONT_SIZE};
use color::WHITE;
use app::Context;

#[derive(Debug)]
pub struct ToolBar {
    rect: Rectangle,
    children: Vec<Box<View>>,
}

impl ToolBar {
    pub fn new(rect: Rectangle, is_reflowable: bool, font_size: f32) -> ToolBar {
        let mut children = Vec::new();
        let side = rect.height() as i32;

        if is_reflowable {
            let font_size_icon = Icon::new("font_size",
                                           rect![rect.min, rect.min + pt!(side)],
                                           Some(WHITE),
                                           Align::Center,
                                           Event::Show(ViewId::FontSizeMenu));
            children.push(Box::new(font_size_icon) as Box<View>);

            let slider = Slider::new(rect![rect.min.x + side, rect.min.y,
                                           rect.max.x - side, rect.max.y],
                                     SliderId::FontSize,
                                     font_size,
                                     DEFAULT_FONT_SIZE / 2.0,
                                     3.0 * DEFAULT_FONT_SIZE / 2.0);
            children.push(Box::new(slider) as Box<View>);
        } else {
            let crop_icon = Icon::new("crop",
                                      rect![rect.min, rect.min + pt!(side)],
                                      Some(WHITE),
                                      Align::Center,
                                      Event::Show(ViewId::MarginCropper));
            children.push(Box::new(crop_icon) as Box<View>);

            let filler = Filler::new(rect![rect.min.x + side, rect.min.y,
                                           rect.max.x - side, rect.max.y],
                                     WHITE);
            children.push(Box::new(filler) as Box<View>);
        }

        let toc_icon = Icon::new("toc",
                                 rect![rect.max - pt!(side), rect.max],
                                 Some(WHITE),
                                 Align::Center,
                                 Event::Show(ViewId::TableOfContents));
        children.push(Box::new(toc_icon) as Box<View>);

        ToolBar {
            rect,
            children,
        }
    }
}

impl View for ToolBar {
    fn handle_event(&mut self, evt: &Event, _hub: &Hub, _bus: &mut Bus, _context: &mut Context) -> bool {
        match *evt {
            Event::Gesture(GestureEvent::Tap { ref center, .. }) if self.rect.includes(center) => true,
            Event::Gesture(GestureEvent::Swipe { ref start, .. }) if self.rect.includes(start) => true,
            _ => false,
        }
    }

    fn render(&self, _fb: &mut Framebuffer, _fonts: &mut Fonts) {
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
