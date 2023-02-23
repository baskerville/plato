use crate::color::{BLACK, WHITE};
use crate::context::Context;
use crate::device::CURRENT_DEVICE;
use crate::font::{font_from_style, Fonts, NORMAL_STYLE};
use crate::framebuffer::Framebuffer;
use crate::geom::{BorderSpec, CornerSpec, Rectangle};
use crate::gesture::GestureEvent;
use crate::unit::scale_by_dpi;
use crate::view::button::Button;
use crate::view::icon::Icon;
use crate::view::label::Label;
use crate::view::slider::Slider;
use crate::view::{Align, Bus, Event, Hub, Id, RenderQueue, View, ID_FEEDER};
use crate::view::{SliderId, ViewId};
use crate::view::{BORDER_RADIUS_MEDIUM, SMALL_BAR_HEIGHT, THICKNESS_LARGE};

use log::debug;

const LABEL_SAVE: &str = "Save";
const LABEL_RESET: &str = "Reset";

pub struct SlidersDialog {
    id: Id,
    view_id: ViewId,
    rect: Rectangle,
    children: Vec<Box<dyn View>>,
    initial_value: Vec<f32>,
}

#[derive(Clone)]
pub struct SlidersConstructors {
    id: SliderId,
    value: f32,
    min: f32,
    max: f32,
}

impl SlidersConstructors {
    pub fn new(id: SliderId, value: f32, min: f32, max: f32) -> SlidersConstructors {
        SlidersConstructors { id, value, min, max }
    }
}

impl SlidersDialog {
    pub fn new(
        context: &mut Context, view_id: ViewId, label: String, sliders: Vec<SlidersConstructors>,
    ) -> SlidersDialog {
        let id = ID_FEEDER.next();
        let mut children = Vec::new();
        let mut initial_value = Vec::new();

        let mut sizes = Self::compute_sizes(sliders.len(), context).into_iter();

        let dpi = CURRENT_DEVICE.dpi;
        let fonts = &mut context.fonts;
        let thickness = scale_by_dpi(THICKNESS_LARGE, dpi) as i32;
        let border_radius = scale_by_dpi(BORDER_RADIUS_MEDIUM, dpi) as i32;

        let padding = {
            let font = font_from_style(fonts, &NORMAL_STYLE, dpi);
            font.em() as i32
        };

        let rect = sizes.next().unwrap();

        for slider in sliders {
            let label = Label::new( sizes.next().unwrap(),
                slider.id.label(),
                Align::Right(padding / 2),
            );
            children.push(Box::new(label) as Box<dyn View>);

            let display_slider = Slider::new(sizes.next().unwrap(),
                slider.id,
                slider.value,
                slider.min,
                slider.max,
            );
            children.push(Box::new(display_slider) as Box<dyn View>);
            initial_value.push(slider.value);
        }

        let corners = CornerSpec::Detailed {
            north_west: 0,
            north_east: border_radius - thickness,
            south_east: 0,
            south_west: 0,
        };
        let close_icon = Icon::new(
            "close", sizes.next().unwrap(),
            Event::Close(view_id),
        )
        .corners(Some(corners));

        children.push(Box::new(close_icon) as Box<dyn View>);

        let label = Label::new(sizes.next().unwrap(),
            label,
            Align::Center,
        );
        children.push(Box::new(label) as Box<dyn View>);

        let button_save = Button::new(sizes.next().unwrap(),
            Event::Save,
            LABEL_SAVE.to_string(),
        );
        children.push(Box::new(button_save) as Box<dyn View>);

        let button_reset = Button::new(sizes.next().unwrap(),
            Event::Reset,
            LABEL_RESET.to_string(),
        );
        children.push(Box::new(button_reset) as Box<dyn View>);

        SlidersDialog {
            id,
            view_id,
            rect,
            children,
            initial_value,
        }
    }

    fn compute_sizes(len: usize, context: &mut Context) -> Vec<Rectangle> {
        let dpi = CURRENT_DEVICE.dpi;
        let fonts = &mut context.fonts;
        let (width, height) = context.display.dims;
        let small_height = scale_by_dpi(SMALL_BAR_HEIGHT, dpi) as i32;
        let thickness = scale_by_dpi(THICKNESS_LARGE, dpi) as i32;

        let (x_height, padding) = {
            let font = font_from_style(fonts, &NORMAL_STYLE, dpi);
            (font.x_heights.0 as i32, font.em() as i32)
        };
        let window_width = width as i32 - 2 * padding;
        let window_height = 2 * small_height * len as i32 + 2 * padding;
        let dx = (width as i32 - window_width) / 2;
        let dy = (height as i32 - window_height) / 3;

        let mut sizes = Vec::new();
        let rect = rect![dx, dy, dx + window_width, dy + window_height];
        sizes.push(rect);

        let max_label_width = {
            let font = font_from_style(fonts, &NORMAL_STYLE, dpi);
            [SliderId::ChessElo.label(), SliderId::ChessSlow.label()]
                .iter()
                .map(|t| font.plan(t, None, None).width)
                .max()
                .unwrap()
        };
        for index in 0..len {
            let min_y = rect.min.y + (index + 1) as i32 * small_height;
            sizes.push(rect![rect.min.x + padding, min_y,
                                     rect.min.x + 2 * padding + max_label_width, min_y + small_height]);
            sizes.push(rect![rect.min.x + max_label_width + 3 * padding, min_y,
                                         rect.max.x - padding, min_y + small_height]);
        }

        sizes.push(rect![rect.max.x - small_height, rect.min.y + thickness,
                               rect.max.x - thickness, rect.min.y + small_height]);
        sizes.push(rect![rect.min.x + small_height, rect.min.y + thickness,
                                   rect.max.x - small_height, rect.min.y + small_height]);

        let max_label_width = {
            let font = font_from_style(fonts, &NORMAL_STYLE, dpi);
            [LABEL_SAVE, LABEL_RESET]
                .iter()
                .map(|t| font.plan(t, None, None).width)
                .max()
                .unwrap()
        };
        let button_height = 4 * x_height;
        let mut button_y = rect.min.y + 2 * small_height;
        button_y += small_height;

        sizes.push(rect![rect.min.x + 3 * padding, button_y + small_height - button_height,
                                   rect.min.x + 5 * padding + max_label_width, button_y + small_height]);
        sizes.push(rect![rect.max.x - 5 * padding - max_label_width, button_y + small_height - button_height,
                                   rect.max.x - 3 * padding, button_y + small_height]);

        sizes
    }
}

impl View for SlidersDialog {
    fn view_id(&self) -> Option<ViewId> {
        Some(self.view_id)
    }

    fn handle_event(
        &mut self, evt: &Event, hub: &Hub, _bus: &mut Bus, rq: &mut RenderQueue, _context: &mut Context,
    ) -> bool {
        match *evt {
            Event::Gesture(GestureEvent::Tap(center)) if !self.rect.includes(center) => {
                hub.send(Event::Close(self.view_id)).ok();
                true
            }
            Event::Gesture(..) => true,
            Event::Reset => {
                // reset sliders to initial values
                for (index, value) in self.initial_value.clone().iter().enumerate() {
                    if let Some(child) = self.child_mut(index * 2 + 1).downcast_mut::<Slider>() {
                        child.update(*value, rq);
                    }
                }
                true
            }
            Event::Save => {
                let mut values = Vec::new();
                for index in 0..self.initial_value.len() {
                    if let Some(child) = self.child_mut(index * 2 + 1).downcast_mut::<Slider>() {
                        values.push(child.get());
                    }
                }
                hub.send(Event::SaveAll(self.view_id, values)).ok();
                true
            }
            _ => false,
        }
    }

    fn render(&self, fb: &mut dyn Framebuffer, _rect: Rectangle, _fonts: &mut Fonts) {
        let dpi = CURRENT_DEVICE.dpi;

        let border_radius = scale_by_dpi(BORDER_RADIUS_MEDIUM, dpi) as i32;
        let border_thickness = scale_by_dpi(THICKNESS_LARGE, dpi) as u16;

        fb.draw_rounded_rectangle_with_border(
            &self.rect,
            &CornerSpec::Uniform(border_radius),
            &BorderSpec {
                thickness: border_thickness,
                color: BLACK,
            },
            &WHITE,
        );
    }

    fn resize(&mut self, _rect: Rectangle, hub: &Hub, rq: &mut RenderQueue, context: &mut Context) {
        let mut sizes = Self::compute_sizes(self.initial_value.len(), context).into_iter();
        let rect = sizes.next().unwrap();
        debug!("Resizing sliders from {} to {}", self.rect, rect);
        self.rect = rect;

        for (index, size) in sizes.enumerate() {
            self.children[index].resize(size, hub, rq, context);
        }
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
}
