use framebuffer::Framebuffer;
use device::CURRENT_DEVICE;
use view::{View, Event, Hub, Bus, ViewId, THICKNESS_MEDIUM};
use view::icon::Icon;
use view::input_field::InputField;
use view::filler::Filler;
use gesture::GestureEvent;
use input::DeviceEvent;
use color::{TEXT_BUMP_SMALL, SEPARATOR_NORMAL};
use geom::Rectangle;
use app::Context;
use unit::scale_by_dpi;
use font::Fonts;

#[derive(Debug)]
pub struct SearchBar {
    pub rect: Rectangle,
    children: Vec<Box<View>>,
}

impl SearchBar {
    pub fn new(rect: Rectangle, placeholder: &str, text: &str) -> SearchBar {
        let mut children = Vec::new();
        let dpi = CURRENT_DEVICE.dpi;
        let thickness = scale_by_dpi(THICKNESS_MEDIUM, dpi) as i32;
        let side = rect.height() as i32;

        let search_icon = Icon::new("search",
                                    rect![rect.min, rect.min + side],
                                    Event::Focus(Some(ViewId::SearchInput)))
                               .background(TEXT_BUMP_SMALL[0]);

        children.push(Box::new(search_icon) as Box<View>);
        
        let separator = Filler::new(rect![pt!(rect.min.x + side, rect.min.y),
                                          pt!(rect.min.x + side + thickness, rect.max.y)],
                                    SEPARATOR_NORMAL);

        children.push(Box::new(separator) as Box<View>);

        let input_field = InputField::new(rect![pt!(rect.min.x + side + thickness, rect.min.y),
                                                pt!(rect.max.x - side - thickness, rect.max.y)],
                                          ViewId::SearchInput)
                                     .border(false)
                                     .text(text)
                                     .placeholder(placeholder);

        children.push(Box::new(input_field) as Box<View>);

        let separator = Filler::new(rect![pt!(rect.max.x - side - thickness, rect.min.y),
                                          pt!(rect.max.x - side, rect.max.y)],
                                    SEPARATOR_NORMAL);

        children.push(Box::new(separator) as Box<View>);

        let close_icon = Icon::new("close",
                                   rect![pt!(rect.max.x - side, rect.min.y),
                                         pt!(rect.max.x, rect.max.y)],
                                   Event::Close(ViewId::SearchBar))
                              .background(TEXT_BUMP_SMALL[0]);

        children.push(Box::new(close_icon) as Box<View>);

        SearchBar {
            rect,
            children,
        }
    }
}

impl View for SearchBar {
    fn handle_event(&mut self, evt: &Event, _hub: &Hub, _bus: &mut Bus, _context: &mut Context) -> bool {
        match *evt {
            Event::Gesture(GestureEvent::Tap { ref center, .. }) |
            Event::Gesture(GestureEvent::HoldFinger(ref center)) if self.rect.includes(center) => true,
            Event::Gesture(GestureEvent::Swipe { ref start, .. }) if self.rect.includes(start) => true,
            Event::Device(DeviceEvent::Finger { ref position, .. }) if self.rect.includes(position) => true,
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
