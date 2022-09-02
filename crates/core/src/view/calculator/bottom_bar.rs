use crate::framebuffer::Framebuffer;
use crate::view::{View, Event, Hub, Bus, Id, ID_FEEDER, RenderQueue, ViewId};
use crate::view::filler::Filler;
use crate::view::labeled_icon::LabeledIcon;
use crate::gesture::GestureEvent;
use crate::input::DeviceEvent;
use crate::geom::{Rectangle, divide};
use crate::font::Fonts;
use crate::color::WHITE;
use crate::context::Context;

#[derive(Debug)]
pub struct BottomBar {
    id: Id,
    rect: Rectangle,
    children: Vec<Box<dyn View>>,
}

impl BottomBar {
    pub fn new(rect: Rectangle, margin_width: i32, font_size: f32) -> BottomBar {
        let id = ID_FEEDER.next();
        let mut children = Vec::new();
        let labelled_icon_width = 5 * rect.height() as i32 / 2;
        let paddings = divide(rect.width() as i32 - 2 * labelled_icon_width, 3);

        let mut x_offset = rect.min.x;
        let filler = Filler::new(rect![x_offset, rect.min.y,
                                       x_offset + paddings[0], rect.max.y],
                                 WHITE);
        x_offset += paddings[0];
        children.push(Box::new(filler) as Box<dyn View>);

        let margin_width_rect = rect![x_offset, rect.min.y,
                                      x_offset + labelled_icon_width, rect.max.y];
        let margin_width_icon = LabeledIcon::new("margin",
                                                 margin_width_rect,
                                                 Event::ToggleNear(ViewId::MarginWidthMenu, margin_width_rect),
                                                 format!("{} mm", margin_width));
        children.push(Box::new(margin_width_icon) as Box<dyn View>);
        x_offset += labelled_icon_width;

        let filler = Filler::new(rect![x_offset, rect.min.y,
                                       x_offset + paddings[1], rect.max.y],
                                 WHITE);
        children.push(Box::new(filler) as Box<dyn View>);
        x_offset += paddings[1];

        let font_size_rect = rect![x_offset, rect.min.y,
                                   x_offset + labelled_icon_width, rect.max.y];
        let font_size_icon = LabeledIcon::new("font_size",
                                              font_size_rect,
                                              Event::ToggleNear(ViewId::FontSizeMenu, font_size_rect),
                                              format!("{:.1} pt", font_size));
        children.push(Box::new(font_size_icon) as Box<dyn View>);
        x_offset += labelled_icon_width;

        let filler = Filler::new(rect![x_offset, rect.min.y,
                                       x_offset + paddings[2], rect.max.y],
                                 WHITE);
        children.push(Box::new(filler) as Box<dyn View>);

        BottomBar {
            id,
            rect,
            children,
        }
    }

    pub fn update_font_size(&mut self, font_size: f32, rq: &mut RenderQueue) {
        if let Some(labeled_icon) = self.children[3].downcast_mut::<LabeledIcon>() {
            labeled_icon.update(&format!("{:.1} pt", font_size), rq);
        }
    }

    pub fn update_margin_width(&mut self, margin_width: i32, rq: &mut RenderQueue) {
        if let Some(labeled_icon) = self.children[1].downcast_mut::<LabeledIcon>() {
            labeled_icon.update(&format!("{} mm", margin_width), rq);
        }
    }
}

impl View for BottomBar {
    fn handle_event(&mut self, evt: &Event, _hub: &Hub, _bus: &mut Bus, _rq: &mut RenderQueue, _context: &mut Context) -> bool {
        match *evt {
            Event::Gesture(GestureEvent::Tap(center)) |
            Event::Gesture(GestureEvent::HoldFingerShort(center, ..)) if self.rect.includes(center) => true,
            Event::Gesture(GestureEvent::Swipe { start, .. }) if self.rect.includes(start) => true,
            Event::Device(DeviceEvent::Finger { position, .. }) if self.rect.includes(position) => true,
            _ => false,
        }
    }

    fn render(&self, _fb: &mut dyn Framebuffer, _rect: Rectangle, _fonts: &mut Fonts) {
    }

    fn resize(&mut self, rect: Rectangle, hub: &Hub, rq: &mut RenderQueue, context: &mut Context) {
        let labelled_icon_width = 5 * rect.height() as i32 / 2;
        let paddings = divide(rect.width() as i32 - 2 * labelled_icon_width, 3);

        let mut x_offset = rect.min.x;

        let filr_rect = rect![x_offset, rect.min.y,
                              x_offset + paddings[0], rect.max.y];
        self.children[0].resize(filr_rect, hub, rq, context);
        x_offset += paddings[0];

        let margin_width_rect = rect![x_offset, rect.min.y,
                                      x_offset + labelled_icon_width, rect.max.y];
        self.children[1].resize(margin_width_rect, hub, rq, context);
        x_offset += labelled_icon_width;

        let filr_rect = rect![x_offset, rect.min.y,
                              x_offset + paddings[1], rect.max.y];
        self.children[2].resize(filr_rect, hub, rq, context);
        x_offset += paddings[1];

        let font_size_rect = rect![x_offset, rect.min.y,
                                   x_offset + labelled_icon_width, rect.max.y];
        self.children[3].resize(font_size_rect, hub, rq, context);
        x_offset += labelled_icon_width;

        let filr_rect = rect![x_offset, rect.min.y,
                              x_offset + paddings[2], rect.max.y];
        self.children[4].resize(filr_rect, hub, rq, context);

        self.rect = rect;
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
