use device::{CURRENT_DEVICE, BAR_SIZES};
use geom::{Rectangle, CornerSpec, BorderSpec};
use font::{Fonts, font_from_style, NORMAL_STYLE};
use view::{View, Event, Hub, Bus, ViewId, SliderId, Align};
use view::{THICKNESS_LARGE, BORDER_RADIUS_MEDIUM};
use view::label::Label;
use view::slider::Slider;
use view::icon::Icon;
use framebuffer::Framebuffer;
use gesture::GestureEvent;
use input::FingerStatus;
use color::{BLACK, WHITE};
use unit::scale_by_dpi;
use app::Context;

pub struct FrontlightWindow {
    rect: Rectangle,
    children: Vec<Box<View>>,
}

impl FrontlightWindow {
    pub fn new(context: &mut Context) -> FrontlightWindow {
        let fonts = &mut context.fonts;
        let levels = context.frontlight.levels();
        let mut children = Vec::new();
        let dpi = CURRENT_DEVICE.dpi;
        let (width, height) = CURRENT_DEVICE.dims;
        let &(small_height, _) = BAR_SIZES.get(&(height, dpi)).unwrap();
        let thickness = scale_by_dpi(THICKNESS_LARGE, dpi) as i32;
        let border_radius = scale_by_dpi(BORDER_RADIUS_MEDIUM, dpi) as i32;

        let font = font_from_style(fonts, &NORMAL_STYLE, dpi);
        let padding = font.em() as i32;

        let window_width = width as i32 - 2 * padding;

        let window_height = if CURRENT_DEVICE.has_natural_light() {
            small_height as i32 * 3 + 2 * padding
        } else {
            small_height as i32 * 2 + 2 * padding
        };

        let dx = (width as i32 - window_width) / 2;
        let dy = (height as i32 - window_height) / 3;

        let rect = rect![dx, dy, dx + window_width, dy + window_height];

        let close_icon = Icon::new("close",
                                   rect![rect.max.x - small_height as i32,
                                         rect.min.y + thickness,
                                         rect.max.x - thickness,
                                         rect.min.y + small_height as i32],
                                   Event::Close(ViewId::Frontlight))
                              .corners(Some(CornerSpec::Uniform(border_radius - thickness)));

        children.push(Box::new(close_icon) as Box<View>);

        let label = Label::new(rect![rect.min.x + small_height as i32,
                                     rect.min.y + thickness,
                                     rect.max.x - small_height as i32,
                                     rect.min.y + small_height as i32],
                               "Frontlight".to_string(),
                               Align::Center);

        children.push(Box::new(label) as Box<View>);

        if CURRENT_DEVICE.has_natural_light() {
            let max_label_width = 
                ["Intensity", "Warmth"].iter()
                                         .map(|t| font.plan(t, None, None).width).max().unwrap() as i32;


            for (index, slider_id) in [SliderId::LightIntensity, SliderId::LightWarmth].iter().enumerate() {
                let min_y = rect.min.y + (index + 1) as i32 * small_height as i32;
                let label = Label::new(rect![rect.min.x + padding,
                                             min_y,
                                             rect.min.x + 2 * padding + max_label_width,
                                             min_y + small_height as i32],
                                       slider_id.label(),
                                       Align::Right(padding / 2));
                children.push(Box::new(label) as Box<View>);

                let value = if *slider_id == SliderId::LightIntensity {
                    levels.intensity()
                } else {
                    levels.warmth()
                };

                let slider = Slider::new(rect![rect.min.x + max_label_width + 3 * padding,
                                               min_y,
                                               rect.max.x - padding,
                                               min_y + small_height as i32],
                                        *slider_id,
                                        value,
                                        0.0,
                                        100.0);
                children.push(Box::new(slider) as Box<View>);
            }
        } else {
                let min_y = rect.min.y + small_height as i32;
                let slider = Slider::new(rect![rect.min.x + padding,
                                               min_y,
                                               rect.max.x - padding,
                                               min_y + small_height as i32],
                                        SliderId::LightIntensity,
                                        levels.intensity(),
                                        0.0,
                                        100.0);
                children.push(Box::new(slider) as Box<View>);
        }

        FrontlightWindow {
            rect,
            children,
        }
    }
}

impl View for FrontlightWindow {
    fn handle_event(&mut self, evt: &Event, hub: &Hub, _bus: &mut Bus, context: &mut Context) -> bool {
        match *evt {
            Event::Slider(SliderId::LightIntensity, value, FingerStatus::Up) => {
                context.frontlight.set_intensity(value);
                true
            },
            Event::Slider(SliderId::LightWarmth, value, FingerStatus::Up) => {
                context.frontlight.set_warmth(value);
                true
            },
            Event::Gesture(GestureEvent::Tap { ref center, .. }) if !self.rect.includes(center) => {
                hub.send(Event::Close(ViewId::Frontlight)).unwrap();
                true
            },
            Event::Gesture(..) => true,
            _ => false,
        }
    }

    fn render(&self, fb: &mut Framebuffer, _fonts: &mut Fonts) {
        let dpi = CURRENT_DEVICE.dpi;

        let border_radius = scale_by_dpi(BORDER_RADIUS_MEDIUM, dpi) as i32;
        let border_thickness = scale_by_dpi(THICKNESS_LARGE, dpi) as u16;

        fb.draw_rounded_rectangle_with_border(&self.rect,
                                              &CornerSpec::Uniform(border_radius),
                                              &BorderSpec { thickness: border_thickness,
                                                            color: BLACK },
                                              &WHITE);
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

    fn children(&self) -> &Vec<Box<View>> {
        &self.children
    }

    fn children_mut(&mut self) -> &mut Vec<Box<View>> {
        &mut self.children
    }
}
