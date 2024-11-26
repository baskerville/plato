use crate::color::{SEPARATOR_NORMAL, TEXT_BUMP_SMALL};
use crate::context::Context;
use crate::device::CURRENT_DEVICE;
use crate::font::Fonts;
use crate::framebuffer::Framebuffer;
use crate::geom::Rectangle;
use crate::unit::scale_by_dpi;
use crate::view::filler::Filler;
use crate::view::icon::Icon;
use crate::view::label::Label;
use crate::view::{Align, Bus, Event, Hub, Id, RenderQueue, View, ID_FEEDER, THICKNESS_MEDIUM};

#[derive(Debug)]
pub struct NavigationBar {
    id: Id,
    pub rect: Rectangle,
    children: Vec<Box<dyn View>>,
}

impl NavigationBar {
    pub fn new(rect: Rectangle, feed_name: &str) -> NavigationBar {
        let id = ID_FEEDER.next();
        let mut children = Vec::new();
        let dpi = CURRENT_DEVICE.dpi;
        let thickness = scale_by_dpi(THICKNESS_MEDIUM, dpi) as i32;
        let side = rect.height() as i32;

        let home_rect = rect![rect.min, rect.min + side];
        let home_icon =
            Icon::new("home", home_rect, Event::OpdsHome).background(TEXT_BUMP_SMALL[0]);

        children.push(Box::new(home_icon) as Box<dyn View>);

        let separator = Filler::new(
            rect![
                pt!(rect.min.x + side, rect.min.y),
                pt!(rect.min.x + side + thickness, rect.max.y)
            ],
            SEPARATOR_NORMAL,
        );

        children.push(Box::new(separator) as Box<dyn View>);

        let feed_label = Label::new(
            rect![
                pt!(rect.min.x + side + thickness, rect.min.y),
                pt!(rect.max.x, rect.max.y)
            ],
            feed_name.to_string(),
            Align::Center,
        );

        children.push(Box::new(feed_label) as Box<dyn View>);

        NavigationBar { id, rect, children }
    }

    pub fn set_feed_name(&mut self, feed_name: &str, rq: &mut RenderQueue) {
        if let Some(feed_label) = self.children[2].downcast_mut::<Label>() {
            feed_label.update(feed_name, rq);
        }
    }

}

impl View for NavigationBar {
    fn handle_event(
        &mut self,
        evt: &Event,
        _hub: &Hub,
        _bus: &mut Bus,
        _rq: &mut RenderQueue,
        _context: &mut Context,
    ) -> bool {
        match *evt {
            _ => false,
        }
    }

    fn render(&self, _fb: &mut dyn Framebuffer, _rect: Rectangle, _fonts: &mut Fonts) {}

    fn resize(&mut self, rect: Rectangle, hub: &Hub, rq: &mut RenderQueue, context: &mut Context) {
        let dpi = CURRENT_DEVICE.dpi;
        let thickness = scale_by_dpi(THICKNESS_MEDIUM, dpi) as i32;
        let side = rect.height() as i32;
        self.children[0].resize(rect![rect.min, rect.min + side], hub, rq, context);
        self.children[1].resize(
            rect![
                pt!(rect.min.x + side, rect.min.y),
                pt!(rect.min.x + side + thickness, rect.max.y)
            ],
            hub,
            rq,
            context,
        );
        self.children[2].resize(
            rect![
                pt!(rect.min.x + side + thickness, rect.min.y),
                pt!(rect.max.x, rect.max.y)
            ],
            hub,
            rq,
            context,
        );
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
