use crate::framebuffer::{Framebuffer};
use crate::device::CURRENT_DEVICE;
use crate::view::{View, Event, Hub, Bus, Id, ID_FEEDER, RenderQueue, ViewId, THICKNESS_MEDIUM};
use crate::view::icon::Icon;
use crate::view::input_field::InputField;
use crate::view::filler::Filler;
use crate::gesture::GestureEvent;
use crate::input::DeviceEvent;
use crate::color::{TEXT_BUMP_SMALL, SEPARATOR_NORMAL};
use crate::geom::{Rectangle, CycleDir};
use crate::context::Context;
use crate::unit::scale_by_dpi;
use crate::font::Fonts;

#[derive(Debug)]
pub struct InputBar {
    id: Id,
    pub rect: Rectangle,
    children: Vec<Box<dyn View>>,
}

impl InputBar {
    pub fn new(rect: Rectangle, placeholder: &str, text: &str, context: &mut Context) -> InputBar {
        let id = ID_FEEDER.next();
        let mut children = Vec::new();
        let dpi = CURRENT_DEVICE.dpi;
        let thickness = scale_by_dpi(THICKNESS_MEDIUM, dpi) as i32;
        let side = rect.height() as i32;

        let prev_icon = Icon::new("angle-up",
                                  rect![rect.min, rect.min + side],
                                  Event::History(CycleDir::Previous, false))
                             .background(TEXT_BUMP_SMALL[0]);
        children.push(Box::new(prev_icon) as Box<dyn View>);

        let separator = Filler::new(rect![pt!(rect.min.x + side, rect.min.y),
                                          pt!(rect.min.x + side + thickness, rect.max.y)],
                                    SEPARATOR_NORMAL);
        children.push(Box::new(separator) as Box<dyn View>);

        let input_field = InputField::new(rect![pt!(rect.min.x + side + thickness, rect.min.y),
                                                pt!(rect.max.x - side - thickness, rect.max.y)],
                                          ViewId::CalculatorInput)
                                     .border(false)
                                     .text(text, context)
                                     .placeholder(placeholder);
        children.push(Box::new(input_field) as Box<dyn View>);

        let separator = Filler::new(rect![pt!(rect.max.x - side - thickness, rect.min.y),
                                          pt!(rect.max.x - side, rect.max.y)],
                                    SEPARATOR_NORMAL);
        children.push(Box::new(separator) as Box<dyn View>);

        let next_icon = Icon::new("angle-down",
                                   rect![pt!(rect.max.x - side, rect.min.y),
                                         pt!(rect.max.x, rect.max.y)],
                                   Event::History(CycleDir::Next, false))
                              .background(TEXT_BUMP_SMALL[0]);
        children.push(Box::new(next_icon) as Box<dyn View>);

        InputBar {
            id,
            rect,
            children,
        }
    }

    pub fn set_text(&mut self, text: &str, move_cursor: bool, rq: &mut RenderQueue, context: &mut Context) {
        if let Some(input_field) = self.children[2].downcast_mut::<InputField>() {
            input_field.set_text(text, move_cursor, rq, context);
        }
    }

    pub fn text_before_cursor(&self) -> &str {
        self.children[2].downcast_ref::<InputField>().unwrap().text_before_cursor()
    }
}

impl View for InputBar {
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
        let dpi = CURRENT_DEVICE.dpi;
        let thickness = scale_by_dpi(THICKNESS_MEDIUM, dpi) as i32;
        let side = rect.height() as i32;
        self.children[0].resize(rect![rect.min, rect.min + side], hub, rq, context);
        self.children[1].resize(rect![pt!(rect.min.x + side, rect.min.y),
                                      pt!(rect.min.x + side + thickness, rect.max.y)], hub, rq, context);
        self.children[2].resize(rect![pt!(rect.min.x + side + thickness, rect.min.y),
                                      pt!(rect.max.x - side - thickness, rect.max.y)], hub, rq, context);
        self.children[3].resize(rect![pt!(rect.max.x - side - thickness, rect.min.y),
                                      pt!(rect.max.x - side, rect.max.y)], hub, rq, context);
        self.children[4].resize(rect![pt!(rect.max.x - side, rect.min.y),
                                      pt!(rect.max.x, rect.max.y)], hub, rq, context);
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
