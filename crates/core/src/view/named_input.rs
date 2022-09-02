use crate::framebuffer::Framebuffer;
use crate::font::{Fonts, font_from_style, NORMAL_STYLE};
use crate::geom::{Rectangle, CornerSpec, BorderSpec, halves, big_half};
use crate::gesture::GestureEvent;
use super::{View, Event, Hub, Bus, Id, ID_FEEDER, RenderQueue, ViewId, Align};
use super::{THICKNESS_LARGE, BORDER_RADIUS_MEDIUM};
use super::common::shift;
use super::label::Label;
use super::input_field::InputField;
use crate::unit::scale_by_dpi;
use crate::color::{BLACK, WHITE};
use crate::device::CURRENT_DEVICE;
use crate::context::Context;

pub struct NamedInput {
    id: Id,
    rect: Rectangle,
    children: Vec<Box<dyn View>>,
    view_id: ViewId,
}

impl NamedInput {
    pub fn new(text: String, view_id: ViewId, input_id: ViewId, input_size: usize, context: &mut Context) -> NamedInput {
        let id = ID_FEEDER.next();
        let dpi = CURRENT_DEVICE.dpi;
        let (width, height) = context.display.dims;

        let input_size = input_size.max(3);
        let mut children = Vec::new();
        let font = font_from_style(&mut context.fonts, &NORMAL_STYLE, dpi);
        let x_height = font.x_heights.0 as i32;
        let padding = font.em() as i32;

        let mut label_width = font.plan(&text, None, None).width;
        let mut input_width = font.plan(&"0".repeat(input_size), None, None).width;
        let mut total_width = 5 * padding + label_width + input_width;
        let delta = width as i32 - total_width;

        if delta < 0 {
            let label_ratio = label_width as f32 / (label_width + input_width) as f32;
            let label_delta = (delta as f32 * label_ratio) as i32;
            let input_delta = delta - label_delta;
            label_width = (label_width + label_delta).abs();
            input_width = (input_width + input_delta).abs(); 
            total_width += delta;
        }

        let (small_half_width, big_half_width) = halves(total_width);
        let big_half_padding = big_half(padding);

        let anchor = pt!(width as i32 / 2, height as i32 / 3);
        let x_min = anchor.x - small_half_width;
        let x_max = anchor.x + big_half_width;
        let y_min = anchor.y - 4 * x_height;
        let y_max = anchor.y + 4 * x_height;

        let label = Label::new(rect![x_min + big_half_padding,
                                     y_min + x_height,
                                     x_min + big_half_padding + padding + label_width,
                                     y_max - x_height],
                               text,
                               Align::Center);
        children.push(Box::new(label) as Box<dyn View>);

        let input_field = InputField::new(rect![x_max - 3 * padding - input_width,
                                                y_min + 2 * x_height,
                                                x_max - padding,
                                                y_max - 2 * x_height],
                                          input_id);
        children.push(Box::new(input_field) as Box<dyn View>);

        let rect = rect![x_min, y_min,
                         x_max, y_max];
                                          
        NamedInput {
            id,
            rect,
            children,
            view_id,
        }
    }

    pub fn set_text(&mut self, text: &str, rq: &mut RenderQueue, context: &mut Context) {
        if let Some(input_field) = self.children[1].downcast_mut::<InputField>() {
            input_field.set_text(text, true, rq, context);
        }
    }
}

impl View for NamedInput {
    fn handle_event(&mut self, evt: &Event, _hub: &Hub, bus: &mut Bus, _rq: &mut RenderQueue, context: &mut Context) -> bool {
        match *evt {
            Event::Submit(..) => {
                bus.push_back(Event::Close(self.view_id));
                false
            },
            Event::Gesture(GestureEvent::Tap(center)) | Event::Gesture(GestureEvent::HoldFingerShort(center, _)) => {
                if !self.rect.includes(center) && !context.kb_rect.includes(center) {
                    bus.push_back(Event::Close(self.view_id));
                    true
                } else {
                    self.rect.includes(center)
                }
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

    fn resize(&mut self, _rect: Rectangle, _hub: &Hub, _rq: &mut RenderQueue, context: &mut Context) {
        let (width, height) = context.display.dims;
        let dx = (width as i32 - height as i32) / 2;
        let dy = (height as i32 - width as i32) / 3;
        let delta = pt!(dx, dy);
        shift(self, delta);
    }

    fn is_background(&self) -> bool {
        true
    }

    fn view_id(&self) -> Option<ViewId> {
        Some(self.view_id)
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
