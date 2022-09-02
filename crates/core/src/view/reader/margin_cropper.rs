use crate::framebuffer::{Framebuffer, UpdateMode, Pixmap};
use crate::metadata::Margin;
use crate::gesture::GestureEvent;
use crate::font::Fonts;
use crate::geom::{Rectangle, Point, CornerSpec, BorderSpec};
use crate::view::{View, Event, Hub, Bus, Id, ID_FEEDER, RenderQueue, RenderData, ViewId};
use crate::view::{SMALL_BAR_HEIGHT, THICKNESS_MEDIUM};
use crate::view::rounded_button::RoundedButton;
use crate::unit::scale_by_dpi;
use crate::color::{BLACK, WHITE, GRAY12};
use crate::device::CURRENT_DEVICE;
use crate::context::Context;

pub const BUTTON_DIAMETER: f32 = 30.0;

pub struct MarginCropper {
    id: Id,
    rect: Rectangle,
    children: Vec<Box<dyn View>>,
    pixmap: Pixmap,
    frame: Rectangle,
}

impl MarginCropper {
    pub fn new(rect: Rectangle, pixmap: Pixmap, margin: &Margin, _context: &mut Context) -> MarginCropper {
        let id = ID_FEEDER.next();
        let mut children = Vec::new();

        let pt = pt!((rect.width() as i32 - pixmap.width as i32) / 2,
                     (rect.height() as i32 - pixmap.height as i32) / 2);
        let x_min = pt.x +
                    (margin.left * pixmap.width as f32).round() as i32;
        let y_min = pt.y +
                    (margin.top * pixmap.height as f32).round() as i32;
        let x_max = pt.x + pixmap.width as i32 - (margin.right * pixmap.width as f32).round() as i32;
        let y_max = pt.y + pixmap.height as i32 - (margin.bottom * pixmap.height as f32).round() as i32;
        let frame = rect![x_min, y_min, x_max, y_max];

        let dpi = CURRENT_DEVICE.dpi;
        let small_height = scale_by_dpi(SMALL_BAR_HEIGHT, dpi) as i32;
        let big_button_diameter = small_height;
        let padding = big_button_diameter / 2;

        let cancel_button = RoundedButton::new("close",
                                                rect![rect.min.x + padding,
                                                      rect.max.y - padding - big_button_diameter,
                                                      rect.min.x + padding + big_button_diameter,
                                                      rect.max.y - padding],
                                                Event::Cancel);
        children.push(Box::new(cancel_button) as Box<dyn View>);

        let validate_button = RoundedButton::new("check_mark-large",
                                                 rect![rect.max.x - padding - big_button_diameter,
                                                       rect.max.y - padding - big_button_diameter,
                                                       rect.max.x - padding,
                                                       rect.max.y - padding],
                                                 Event::Validate);
        children.push(Box::new(validate_button) as Box<dyn View>);

        MarginCropper {
            id,
            rect,
            children,
            pixmap,
            frame,
        }
    }

    fn update(&mut self, start: Point, end: Point) {
        let mut nearest = None;
        let mut dmin = u32::MAX;

        for i in 0..3i32 {
            for j in 0..3i32 {
                if i == 1 && j == 1 {
                    continue
                }
                let x = self.frame.min.x + i * self.frame.width() as i32 / 2;
                let y = self.frame.min.y + j * self.frame.height() as i32 / 2;
                let pt = pt!(x, y);
                let d = pt.dist2(start);
                if d < dmin {
                    nearest = Some((i, j));
                    dmin = d;
                }
            }
        }

        if let Some((i, j)) = nearest {
            match (i, j) {
                (0, 0) => self.frame.min = end,
                (1, 0) => self.frame.min.y = end.y,
                (1, 2) => self.frame.max.y = end.y,
                (0, 1) => self.frame.min.x = end.x,
                (2, 1) => self.frame.max.x = end.x,
                (0, 2) => { self.frame.min.x = end.x; self.frame.max.y = end.y },
                (2, 0) => { self.frame.max.x = end.x; self.frame.min.y = end.y },
                (2, 2) => self.frame.max = end,
                _ => (),
            }
        }

        let dpi = CURRENT_DEVICE.dpi;
        let button_radius = scale_by_dpi(BUTTON_DIAMETER / 2.0, dpi) as i32;

        self.frame.min.x = self.frame.min.x.max(self.rect.min.x + button_radius);
        self.frame.min.y = self.frame.min.y.max(self.rect.min.y + button_radius);
        self.frame.max.x = self.frame.max.x.min(self.rect.max.x - button_radius);
        self.frame.max.y = self.frame.max.y.min(self.rect.max.y - button_radius);
    }

    fn margin(&self) -> Margin {
        let x_min = (self.rect.width() as i32 - self.pixmap.width as i32) / 2;
        let y_min = (self.rect.height() as i32 - self.pixmap.height as i32) / 2;
        let x_max = x_min + self.pixmap.width as i32;
        let y_max = y_min + self.pixmap.height as i32;

        let top = (self.frame.min.y - y_min).max(0) as f32 / self.pixmap.height as f32;
        let right = (x_max - self.frame.max.x).max(0) as f32 / self.pixmap.width as f32;
        let bottom = (y_max - self.frame.max.y).max(0) as f32 / self.pixmap.height as f32;
        let left = (self.frame.min.x - x_min).max(0) as f32 / self.pixmap.width as f32;

        Margin::new(top, right, bottom, left)
    }
}

impl View for MarginCropper {
    fn handle_event(&mut self, evt: &Event, _hub: &Hub, bus: &mut Bus, rq: &mut RenderQueue, _context: &mut Context) -> bool {
        match *evt {
            Event::Gesture(GestureEvent::Tap(center)) if self.rect.includes(center) => {
                self.update(center, center);
                rq.add(RenderData::new(self.id, self.rect, UpdateMode::Gui));
                true
            },
            Event::Gesture(GestureEvent::Swipe { start, end, .. }) if self.rect.includes(start) => {
                self.update(start, end);
                rq.add(RenderData::new(self.id, self.rect, UpdateMode::Gui));
                true
            },
            Event::Gesture(GestureEvent::HoldFingerShort(center, ..)) if self.rect.includes(center) => true,
            Event::Validate => {
                bus.push_back(Event::CropMargins(Box::new(self.margin())));
                bus.push_back(Event::Close(ViewId::MarginCropper));
                true
            },
            Event::Cancel => {
                bus.push_back(Event::Close(ViewId::MarginCropper));
                true
            },
            _ => false,
        }
    }

    fn render(&self, fb: &mut dyn Framebuffer, _rect: Rectangle, _fonts: &mut Fonts) {
        let dpi = CURRENT_DEVICE.dpi;
        let dx = (self.rect.width() as i32 - self.pixmap.width as i32) / 2;
        let dy = (self.rect.height() as i32 - self.pixmap.height as i32) / 2;

        fb.draw_rectangle(&self.rect, WHITE);
        fb.draw_pixmap(&self.pixmap, pt!(dx, dy));

        let thickness = scale_by_dpi(THICKNESS_MEDIUM, dpi) as u16;

        fb.draw_blended_rectangle(&rect![self.rect.min.x, self.rect.min.y,
                                         self.frame.min.x, self.frame.max.y],
                                  GRAY12,
                                  0.4);
        fb.draw_blended_rectangle(&rect![self.rect.min.x, self.frame.max.y,
                                         self.frame.max.x, self.rect.max.y],
                                  GRAY12,
                                  0.4);
        fb.draw_blended_rectangle(&rect![self.frame.max.x, self.frame.min.y,
                                         self.rect.max.x, self.rect.max.y],
                                  GRAY12,
                                  0.4);
        fb.draw_blended_rectangle(&rect![self.frame.min.x, self.rect.min.y,
                                         self.rect.max.x, self.frame.min.y],
                                  GRAY12,
                                  0.4);

        fb.draw_rectangle_outline(&self.frame,
                                  &BorderSpec { thickness: thickness as u16,
                                                color: BLACK });

        let button_radius = scale_by_dpi(BUTTON_DIAMETER / 2.0, dpi) as i32;

        for i in 0..3i32 {
            for j in 0..3i32 {
                if i == 1 && j == 1 {
                    continue
                }

                let x = self.frame.min.x + i * self.frame.width() as i32 / 2;
                let y = self.frame.min.y + j * self.frame.height() as i32 / 2;
                let button_rect = rect![x - button_radius, y - button_radius,
                                        x + button_radius, y + button_radius];

                fb.draw_rounded_rectangle_with_border(&button_rect,
                                                      &CornerSpec::Uniform(button_radius),
                                                      &BorderSpec { thickness: thickness as u16,
                                                                    color: BLACK },
                                                      &WHITE);
            }
        }
    }

    fn is_background(&self) -> bool {
        true
    }

    fn view_id(&self) -> Option<ViewId> {
        Some(ViewId::MarginCropper)
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
