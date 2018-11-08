use framebuffer::Framebuffer;
use view::{View, Event, Hub, Bus, Align};
use view::icon::Icon;
use view::label::Label;
use geom::Rectangle;
use font::Fonts;
use app::Context;

#[derive(Debug)]
pub struct LabeledIcon {
    rect: Rectangle,
    children: Vec<Box<View>>,
    event: Event,
}

impl LabeledIcon {
    pub fn new(name: &str, rect: Rectangle, event: Event, text: String) -> LabeledIcon {
        let mut children = Vec::new();
        let side = rect.height() as i32;

        let icon = Icon::new(name,
                             rect![rect.min.x, rect.min.y,
                                   rect.min.x + side, rect.max.y],
                             Event::Validate);
        children.push(Box::new(icon) as Box<View>);

        let label = Label::new(rect![rect.min.x + side, rect.min.y,
                                     rect.max.x, rect.max.y],
                               text,
                               Align::Left(0))
                          .event(Some(Event::Validate));
        children.push(Box::new(label) as Box<View>);

        LabeledIcon {
            rect,
            children,
            event,
        }
    }

    pub fn update(&mut self, text: String, hub: &Hub) {
        if let Some(label) = self.children[1].downcast_mut::<Label>() {
            label.update(text, hub);
        }
    }
}

impl View for LabeledIcon {
    fn handle_event(&mut self, evt: &Event, _hub: &Hub, bus: &mut Bus, _context: &mut Context) -> bool {
        match *evt {
            Event::Validate => {
                if let Event::Show(view_id) = self.event {
                    bus.push_back(Event::ToggleNear(view_id, self.rect));
                } else {
                    bus.push_back(self.event.clone());
                }
                true
            },
            _ => false,
        }
    }

    fn render(&self, _fb: &mut Framebuffer, _fonts: &mut Fonts) {
    }

    fn resize(&mut self, rect: Rectangle, context: &mut Context) {
        let side = rect.height() as i32;
        self.children[0].resize(rect![rect.min.x, rect.min.y,
                                      rect.min.x + side, rect.max.y],
                                context);
        self.children[1].resize(rect![rect.min.x + side, rect.min.y,
                                     rect.max.x, rect.max.y],
                                context);
        self.rect = rect;
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
