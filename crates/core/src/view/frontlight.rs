use crate::device::CURRENT_DEVICE;
use crate::framebuffer::{Framebuffer, UpdateMode};
use crate::geom::{Rectangle, CornerSpec, BorderSpec};
use crate::font::{Fonts, font_from_style, NORMAL_STYLE};
use super::{View, Event, Hub, Bus, Id, ID_FEEDER, RenderQueue, RenderData, ViewId, EntryId, SliderId, Align};
use super::{SMALL_BAR_HEIGHT, THICKNESS_LARGE, BORDER_RADIUS_MEDIUM};
use super::label::Label;
use super::button::Button;
use super::slider::Slider;
use super::icon::Icon;
use super::presets_list::PresetsList;
use super::common::shift;
use crate::frontlight::LightLevels;
use crate::gesture::GestureEvent;
use crate::settings::{LightPreset, guess_frontlight};
use crate::color::{BLACK, WHITE};
use crate::unit::scale_by_dpi;
use crate::context::Context;

const LABEL_SAVE: &str = "Save";
const LABEL_GUESS: &str = "Guess";

pub struct FrontlightWindow {
    id: Id,
    rect: Rectangle,
    children: Vec<Box<dyn View>>,
}

impl FrontlightWindow {
    pub fn new(context: &mut Context) -> FrontlightWindow {
        let id = ID_FEEDER.next();
        let fonts = &mut context.fonts;
        let levels = context.frontlight.levels();
        let presets = &context.settings.frontlight_presets;
        let mut children = Vec::new();
        let dpi = CURRENT_DEVICE.dpi;
        let (width, height) = context.display.dims;
        let small_height = scale_by_dpi(SMALL_BAR_HEIGHT, dpi) as i32;
        let thickness = scale_by_dpi(THICKNESS_LARGE, dpi) as i32;
        let border_radius = scale_by_dpi(BORDER_RADIUS_MEDIUM, dpi) as i32;

        let (x_height, padding) = {
            let font = font_from_style(fonts, &NORMAL_STYLE, dpi);
            (font.x_heights.0 as i32, font.em() as i32)
        };

        let window_width = width as i32 - 2 * padding;

        let mut window_height = small_height * 3 + 2 * padding;

        if CURRENT_DEVICE.has_natural_light() {
            window_height += small_height;
        }

        if !presets.is_empty() {
            window_height += small_height;
        }

        let dx = (width as i32 - window_width) / 2;
        let dy = (height as i32 - window_height) / 3;

        let rect = rect![dx, dy, dx + window_width, dy + window_height];

        let corners = CornerSpec::Detailed {
            north_west: 0,
            north_east: border_radius - thickness,
            south_east: 0,
            south_west: 0,
        };

        let close_icon = Icon::new("close",
                                   rect![rect.max.x - small_height,
                                         rect.min.y + thickness,
                                         rect.max.x - thickness,
                                         rect.min.y + small_height],
                                   Event::Close(ViewId::Frontlight))
                              .corners(Some(corners));

        children.push(Box::new(close_icon) as Box<dyn View>);

        let label = Label::new(rect![rect.min.x + small_height,
                                     rect.min.y + thickness,
                                     rect.max.x - small_height,
                                     rect.min.y + small_height],
                               "Frontlight".to_string(),
                               Align::Center);

        children.push(Box::new(label) as Box<dyn View>);

        let mut button_y = rect.min.y + 2 * small_height;

        if CURRENT_DEVICE.has_natural_light() {
            let max_label_width = {
                let font = font_from_style(fonts, &NORMAL_STYLE, dpi);
                ["Intensity", "Warmth"].iter().map(|t| font.plan(t, None, None).width)
                                                           .max().unwrap() as i32
            };

            for (index, slider_id) in [SliderId::LightIntensity, SliderId::LightWarmth].iter().enumerate() {
                let min_y = rect.min.y + (index + 1) as i32 * small_height;
                let label = Label::new(rect![rect.min.x + padding,
                                             min_y,
                                             rect.min.x + 2 * padding + max_label_width,
                                             min_y + small_height],
                                       slider_id.label(),
                                       Align::Right(padding / 2));
                children.push(Box::new(label) as Box<dyn View>);

                let value = if *slider_id == SliderId::LightIntensity {
                    levels.intensity
                } else {
                    levels.warmth
                };

                let slider = Slider::new(rect![rect.min.x + max_label_width + 3 * padding,
                                               min_y,
                                               rect.max.x - padding,
                                               min_y + small_height],
                                         *slider_id,
                                         value,
                                         0.0,
                                         100.0);
                children.push(Box::new(slider) as Box<dyn View>);
            }

            button_y += small_height;
        } else {
                let min_y = rect.min.y + small_height;
                let slider = Slider::new(rect![rect.min.x + padding,
                                               min_y,
                                               rect.max.x - padding,
                                               min_y + small_height],
                                         SliderId::LightIntensity,
                                         levels.intensity,
                                         0.0,
                                         100.0);
                children.push(Box::new(slider) as Box<dyn View>);
        }

        let max_label_width = {
            let font = font_from_style(fonts, &NORMAL_STYLE, dpi);
            [LABEL_SAVE, LABEL_GUESS].iter().map(|t| font.plan(t, None, None).width)
                                                         .max().unwrap() as i32
        };

        let button_height = 4 * x_height;

        let button_save = Button::new(rect![rect.min.x + 3 * padding,
                                            button_y + small_height - button_height,
                                            rect.min.x + 5 * padding + max_label_width,
                                            button_y + small_height],
                                      Event::Save,
                                      LABEL_SAVE.to_string());
        children.push(Box::new(button_save) as Box<dyn View>);

        let button_guess = Button::new(rect![rect.max.x - 5 * padding - max_label_width,
                                             button_y + small_height - button_height,
                                             rect.max.x - 3 * padding,
                                             button_y + small_height],
                                       Event::Guess,
                                       LABEL_GUESS.to_string()).disabled(presets.len() < 2);
        children.push(Box::new(button_guess) as Box<dyn View>);

        if !presets.is_empty() {
            let presets_rect = rect![rect.min.x + thickness + 4 * padding,
                                     rect.max.y - small_height - 2 * padding,
                                     rect.max.x - thickness - 4 * padding,
                                     rect.max.y - thickness - 2 * padding];
            let mut presets_list = PresetsList::new(presets_rect);
            presets_list.update(presets, &mut RenderQueue::new(), fonts);
            children.push(Box::new(presets_list) as Box<dyn View>);
        }

        FrontlightWindow {
            id,
            rect,
            children,
        }
    }

    fn toggle_presets(&mut self, enable: bool, rq: &mut RenderQueue, context: &mut Context) {
        let dpi = CURRENT_DEVICE.dpi;
        let small_height = scale_by_dpi(SMALL_BAR_HEIGHT, dpi) as i32;

        if enable {
            let thickness = scale_by_dpi(THICKNESS_LARGE, dpi) as i32;
            let padding = {
                let font = font_from_style(&mut context.fonts, &NORMAL_STYLE, dpi);
                font.em() as i32
            };
            shift(self, pt!(0, -(small_height) / 2));
            self.rect.max.y += small_height;
            let presets_rect = rect![self.rect.min.x + thickness + 4 * padding,
                                     self.rect.max.y - small_height - 2 * padding,
                                     self.rect.max.x - thickness - 4 * padding,
                                     self.rect.max.y - thickness - 2 * padding];
            let mut presets_list = PresetsList::new(presets_rect);
            presets_list.update(&context.settings.frontlight_presets, &mut RenderQueue::new(), &mut context.fonts);
            self.children.push(Box::new(presets_list) as Box<dyn View>);
            rq.add(RenderData::new(self.id, self.rect, UpdateMode::Gui));
        } else {
            self.children.pop();
            rq.add(RenderData::expose(self.rect, UpdateMode::Gui));
            shift(self, pt!(0, small_height / 2));
            self.rect.max.y -= small_height;
        }
    }

    fn set_frontlight_levels(&mut self, frontlight_levels: LightLevels, rq: &mut RenderQueue, context: &mut Context) {
        let LightLevels { intensity, warmth } = frontlight_levels;
        context.frontlight.set_intensity(intensity);
        context.frontlight.set_warmth(warmth);
        if CURRENT_DEVICE.has_natural_light() {
            if let Some(slider_intensity) = self.child_mut(3).downcast_mut::<Slider>() {
                slider_intensity.update(intensity, rq);
            }
            if let Some(slider_warmth) = self.child_mut(5).downcast_mut::<Slider>() {
                slider_warmth.update(warmth, rq);
            }
        } else if let Some(slider_intensity) = self.child_mut(2).downcast_mut::<Slider>() {
            slider_intensity.update(intensity, rq);
        }
    }

    fn update_presets(&mut self, rq: &mut RenderQueue, context: &mut Context) {
        let len = self.len();
        if let Some(presets_list) = self.child_mut(len - 1).downcast_mut::<PresetsList>() {
            presets_list.update(&context.settings.frontlight_presets, rq, &mut context.fonts);
        }
    }
}

impl View for FrontlightWindow {
    fn handle_event(&mut self, evt: &Event, hub: &Hub, _bus: &mut Bus, rq: &mut RenderQueue, context: &mut Context) -> bool {
        match *evt {
            Event::Slider(SliderId::LightIntensity, value, _) => {
                context.frontlight.set_intensity(value);
                true
            },
            Event::Slider(SliderId::LightWarmth, value, _) => {
                context.frontlight.set_warmth(value);
                true
            },
            Event::Gesture(GestureEvent::Tap(center)) if !self.rect.includes(center) => {
                hub.send(Event::Close(ViewId::Frontlight)).ok();
                true
            },
            Event::Gesture(..) => true,
            Event::Save => {
                let lightsensor_level = if CURRENT_DEVICE.has_lightsensor() {
                    context.lightsensor.level().ok()
                } else {
                    None
                };
                let light_preset = LightPreset {
                    lightsensor_level,
                    frontlight_levels: context.frontlight.levels(),
                    .. Default::default()
                };
                context.settings.frontlight_presets.push(light_preset);
                context.settings.frontlight_presets.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));
                if context.settings.frontlight_presets.len() == 1 {
                    self.toggle_presets(true, rq, context);
                } else {
                    if context.settings.frontlight_presets.len() == 2 {
                        let index = self.len() - 2;
                        if let Some(button_guess) = self.child_mut(index).downcast_mut::<Button>() {
                            button_guess.disabled = false;
                            rq.add(RenderData::new(button_guess.id(), *button_guess.rect(), UpdateMode::Gui));
                        }
                    }
                    self.update_presets(rq, context);
                }
                true
            },
            Event::Select(EntryId::RemovePreset(index)) => {
                if index < context.settings.frontlight_presets.len() {
                    context.settings.frontlight_presets.remove(index);
                    if context.settings.frontlight_presets.is_empty() {
                        self.toggle_presets(false, rq, context);
                    } else {
                        if context.settings.frontlight_presets.len() == 1 {
                            let index = self.len() - 2;
                            if let Some(button_guess) = self.child_mut(index).downcast_mut::<Button>() {
                                button_guess.disabled = true;
                                rq.add(RenderData::new(button_guess.id(), *button_guess.rect(), UpdateMode::Gui));
                            }
                        }
                        self.update_presets(rq, context);
                    }
                }
                true
            },
            Event::LoadPreset(index) => {
                let frontlight_levels = context.settings.frontlight_presets[index].frontlight_levels;
                self.set_frontlight_levels(frontlight_levels, rq, context);
                true
            },
            Event::Guess => {
                let lightsensor_level = if CURRENT_DEVICE.has_lightsensor() {
                    context.lightsensor.level().ok()
                } else {
                    None
                };
                if let Some(ref frontlight_levels) = guess_frontlight(lightsensor_level, &context.settings.frontlight_presets) {
                    self.set_frontlight_levels(*frontlight_levels, rq, context);
                }
                true
            },
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
        let small_height = scale_by_dpi(SMALL_BAR_HEIGHT, dpi) as i32;
        let thickness = scale_by_dpi(THICKNESS_LARGE, dpi) as i32;

        let (x_height, padding) = {
            let font = font_from_style(&mut context.fonts, &NORMAL_STYLE, dpi);
            (font.x_heights.0 as i32, font.em() as i32)
        };

        let window_width = width as i32 - 2 * padding;

        let mut window_height = small_height * 3 + 2 * padding;

        if CURRENT_DEVICE.has_natural_light() {
            window_height += small_height;
        }

        if !context.settings.frontlight_presets.is_empty() {
            window_height += small_height;
        }

        let dx = (width as i32 - window_width) / 2;
        let dy = (height as i32 - window_height) / 3;

        let rect = rect![dx, dy, dx + window_width, dy + window_height];

        self.children[0].resize(rect![rect.max.x - small_height,
                                      rect.min.y + thickness,
                                      rect.max.x - thickness,
                                      rect.min.y + small_height],
                                hub, rq, context);
        self.children[1].resize(rect![rect.min.x + small_height,
                                      rect.min.y + thickness,
                                      rect.max.x - small_height,
                                      rect.min.y + small_height],
                                hub, rq, context);

        let mut button_y = rect.min.y + 2 * small_height;
        let mut index = 2;

        if CURRENT_DEVICE.has_natural_light() {
            let max_label_width = {
                let font = font_from_style(&mut context.fonts, &NORMAL_STYLE, dpi);
                ["Intensity", "Warmth"].iter().map(|t| font.plan(t, None, None).width)
                                                           .max().unwrap() as i32
            };
            for i in 0..2usize {
                let min_y = rect.min.y + (i + 1) as i32 * small_height;
                self.children[index].resize(rect![rect.min.x + padding,
                                                  min_y,
                                                  rect.min.x + 2 * padding + max_label_width,
                                                  min_y + small_height],
                                            hub, rq, context);
                self.children[index+1].resize(rect![rect.min.x + max_label_width + 3 * padding,
                                                    min_y,
                                                    rect.max.x - padding,
                                                    min_y + small_height],
                                              hub, rq, context);
                index += 2;
            }
            button_y += small_height;
        } else {
            let min_y = rect.min.y + small_height;
            self.children[2].resize(rect![rect.min.x + padding,
                                          min_y,
                                          rect.max.x - padding,
                                          min_y + small_height],
                                    hub, rq, context);
            index += 1;
        }

        let max_label_width = {
            let font = font_from_style(&mut context.fonts, &NORMAL_STYLE, dpi);
            [LABEL_SAVE, LABEL_GUESS].iter().map(|t| font.plan(t, None, None).width)
                                                         .max().unwrap() as i32
        };

        let button_height = 4 * x_height;

        self.children[index].resize(rect![rect.min.x + 3 * padding,
                                          button_y + small_height - button_height,
                                          rect.min.x + 5 * padding + max_label_width,
                                          button_y + small_height],
                                    hub, rq, context);
        index += 1;

        self.children[index].resize(rect![rect.max.x - 5 * padding - max_label_width,
                                          button_y + small_height - button_height,
                                          rect.max.x - 3 * padding,
                                          button_y + small_height],
                                    hub, rq, context);
        index += 1;

        if !context.settings.frontlight_presets.is_empty() {
            let presets_rect = rect![rect.min.x + thickness + 4 * padding,
                                     rect.max.y - small_height - 2 * padding,
                                     rect.max.x - thickness - 4 * padding,
                                     rect.max.y - thickness - 2 * padding];
            self.children[index].resize(presets_rect, hub, rq, context);
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
