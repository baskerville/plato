use std::thread;
use crate::device::CURRENT_DEVICE;
use crate::geom::{Rectangle, CornerSpec, BorderSpec};
use crate::font::{Fonts, font_from_style, NORMAL_STYLE};
use super::{View, Event, Hub, Bus, Id, ID_FEEDER, RenderQueue, ViewId, Align};
use super::{THICKNESS_LARGE, BORDER_RADIUS_MEDIUM, CLOSE_IGNITION_DELAY};
use super::button::Button;
use super::label::Label;
use crate::framebuffer::Framebuffer;
use crate::gesture::GestureEvent;
use crate::color::{BLACK, WHITE};
use crate::unit::scale_by_dpi;
use crate::context::Context;

const LABEL_VALIDATE: &str = "OK";
const LABEL_CANCEL: &str = "Cancel";

pub struct Dialog {
    id: Id,
    rect: Rectangle,
    children: Vec<Box<dyn View>>,
    view_id: ViewId,
    event: Option<Event>,
    will_close: bool,
}

impl Dialog {
    pub fn new(view_id: ViewId, event: Option<Event>, text: String, context: &mut Context) -> Dialog {
        let id = ID_FEEDER.next();
        let mut children = Vec::new();
        let dpi = CURRENT_DEVICE.dpi;
        let (width, height) = context.display.dims;

        let font = font_from_style(&mut context.fonts, &NORMAL_STYLE, dpi);
        let x_height = font.x_heights.0 as i32;
        let padding = font.em() as i32;

        let min_message_width = width as i32 / 2;
        let max_message_width = width as i32 - 3 * padding;
        let max_button_width = width as i32 / 4;
        let button_height = 4 * x_height;

        let plan = font.plan(&text, Some(max_message_width), None);

        let dialog_width = plan.width.max(min_message_width) + 3 * padding;
        let dialog_height = 2 * button_height + 3 * padding;

        let dx = (width as i32 - dialog_width) / 2;
        let dy = (height as i32 - dialog_height) / 2;
        let rect = rect![dx, dy,
                         dx + dialog_width, dy + dialog_height];

        let rect_label = rect![rect.min.x + padding,
                               rect.min.y + padding,
                               rect.max.x - padding,
                               rect.min.y + padding + button_height];

        let label = Label::new(rect_label, text, Align::Center);

        children.push(Box::new(label) as Box<dyn View>);

        let plan_cancel = event.as_ref().map(|_| font.plan(LABEL_CANCEL, Some(max_button_width), None));
        let plan_validate = font.plan(LABEL_VALIDATE, Some(max_button_width), None);

        let button_width = plan_validate.width.max(plan_cancel.map_or(0, |p| p.width)) as i32 + padding;

        if event.is_some() {
            let rect_cancel = rect![rect.min.x + padding,
                                    rect.max.y - button_height - padding,
                                    rect.min.x + button_width + 2 * padding,
                                    rect.max.y - padding];
            let button_cancel = Button::new(rect_cancel, Event::Cancel, LABEL_CANCEL.to_string());
            children.push(Box::new(button_cancel) as Box<dyn View>);
        }

        let rect_validate = rect![rect.max.x - button_width - 2 * padding,
                                  rect.max.y - button_height - padding,
                                  rect.max.x - padding,
                                  rect.max.y - padding];
        let button_validate = Button::new(rect_validate, Event::Validate, LABEL_VALIDATE.to_string()); 
        children.push(Box::new(button_validate) as Box<dyn View>);

        Dialog {
            id,
            rect,
            children,
            view_id,
            event,
            will_close: false,
        }
    }
}

impl View for Dialog {
    fn handle_event(&mut self, evt: &Event, hub: &Hub, bus: &mut Bus, _rq: &mut RenderQueue, _context: &mut Context) -> bool {
        match *evt {
            Event::Validate | Event::Cancel => {
                if self.will_close {
                    return true;
                }
                let hub2 = hub.clone();
                let view_id = self.view_id;
                thread::spawn(move || {
                    thread::sleep(CLOSE_IGNITION_DELAY);
                    hub2.send(Event::Close(view_id)).ok();
                });
                if let Event::Validate = *evt {
                    if let Some(event) = self.event.as_ref() {
                        bus.push_back(event.clone());
                    }
                }
                self.will_close = true;
                true
            },
            Event::Gesture(GestureEvent::Tap(center)) if !self.rect.includes(center) => {
                hub.send(Event::Close(self.view_id)).ok();
                true
            },
            Event::Gesture(..) => true,
            _ => false,
        }
    }

    fn render(&self, fb: &mut dyn Framebuffer, _rect: Rectangle, _fonts: &mut Fonts) {
        let dpi = CURRENT_DEVICE.dpi;

        let border_radius = scale_by_dpi(BORDER_RADIUS_MEDIUM, dpi) as i32;
        let border_thickness = scale_by_dpi(THICKNESS_LARGE, dpi) as u16;

        fb.draw_rounded_rectangle_with_border(&self.rect,
                                              &CornerSpec::Uniform(border_radius),
                                              &BorderSpec { thickness: border_thickness,
                                                            color: BLACK },
                                              &WHITE);
    }

    fn resize(&mut self, _rect: Rectangle, hub: &Hub, rq: &mut RenderQueue, context: &mut Context) {
        let dpi = CURRENT_DEVICE.dpi;
        let (width, height) = context.display.dims;
        let dialog_width = self.rect.width() as i32;
        let dialog_height = self.rect.height() as i32;
        let max_button_width = width as i32 / 4;

        let (x_height, padding, button_width) = {
            let font = font_from_style(&mut context.fonts, &NORMAL_STYLE, dpi);
            let plan_cancel = self.event.as_ref().map(|_| font.plan(LABEL_CANCEL, Some(max_button_width), None));
            let plan_validate = font.plan(LABEL_VALIDATE, Some(max_button_width), None);
            let x_height = font.x_heights.0 as i32;
            let padding = font.em() as i32;
            let button_width = plan_validate.width.max(plan_cancel.map_or(0, |p| p.width)) as i32 + padding;
            (x_height, padding, button_width)
        };
        let button_height = 4 * x_height;

        let dx = (width as i32 - dialog_width) / 2;
        let dy = (height as i32 - dialog_height) / 2;
        let rect = rect![dx, dy,
                         dx + dialog_width, dy + dialog_height];

        let label_rect = rect![rect.min.x + padding,
                               rect.min.y + padding,
                               rect.max.x - padding,
                               rect.min.y + padding + button_height];
        self.children[0].resize(label_rect, hub, rq, context);

        let mut index = 1;
        if self.event.is_some() {
            let cancel_rect = rect![rect.min.x + padding,
                                    rect.max.y - button_height - padding,
                                    rect.min.x + button_width + 2 * padding,
                                    rect.max.y - padding];
            self.children[index].resize(cancel_rect, hub, rq, context);
            index += 1;
        }

        let validate_rect = rect![rect.max.x - button_width - 2 * padding,
                                  rect.max.y - button_height - padding,
                                  rect.max.x - padding,
                                  rect.max.y - padding];
        self.children[index].resize(validate_rect, hub, rq, context);
        self.rect = rect;
    }

    fn is_background(&self) -> bool {
        true
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

    fn view_id(&self) -> Option<ViewId> {
        Some(self.view_id)
    }
}
