use crate::device::CURRENT_DEVICE;
use crate::framebuffer::{Framebuffer, UpdateMode};
use crate::geom::{Rectangle, BorderSpec, CornerSpec};
use crate::color::{BLACK, WHITE, BATTERY_FILL};
use super::{View, ViewId, Event, Hub, Bus, Id, ID_FEEDER, RenderQueue, RenderData};
use super::{THICKNESS_LARGE, THICKNESS_MEDIUM, BORDER_RADIUS_SMALL};
use super::icon::ICONS_PIXMAPS;
use crate::gesture::GestureEvent;
use crate::battery::Status;
use crate::unit::scale_by_dpi;
use crate::font::Fonts;
use crate::context::Context;

const BUMP_HEIGHT: f32 = 5.0 * THICKNESS_LARGE;
const BUMP_WIDTH: f32 = 4.0 * THICKNESS_LARGE;
const BATTERY_HEIGHT: f32 = 11.0 * THICKNESS_LARGE;
const BATTERY_WIDTH: f32 = 2.0 * BATTERY_HEIGHT;

pub struct Battery {
    id: Id,
    rect: Rectangle,
    children: Vec<Box<dyn View>>,
    status: Status,
    capacity: f32,
}

impl Battery {
    pub fn new(rect: Rectangle, capacity: f32, status: Status) -> Battery {
        Battery {
            id: ID_FEEDER.next(),
            rect,
            children: Vec::new(),
            capacity,
            status,
        }
    }

    pub fn update(&mut self, rq: &mut RenderQueue, context: &mut Context) {
        self.capacity = context.battery.capacity().map_or(self.capacity, |v| v[0]);
        self.status = context.battery.status().map_or(self.status, |v| v[0]);
        rq.add(RenderData::new(self.id, self.rect, UpdateMode::Gui));
    }
}

impl View for Battery {
    fn handle_event(&mut self, evt: &Event, _hub: &Hub, bus: &mut Bus, rq: &mut RenderQueue, context: &mut Context) -> bool {
        match *evt {
            Event::BatteryTick => {
                self.update(rq, context);
                true
            },
            Event::Gesture(GestureEvent::Tap(center)) if self.rect.includes(center) => {
                bus.push_back(Event::ToggleNear(ViewId::BatteryMenu, self.rect));
                true
            },
            _ => false,
        }
    }

    fn render(&self, fb: &mut dyn Framebuffer, _rect: Rectangle, _fonts: &mut Fonts) {
        let dpi = CURRENT_DEVICE.dpi;

        let border_radius = scale_by_dpi(BORDER_RADIUS_SMALL, dpi) as i32;
        let border_thickness = scale_by_dpi(THICKNESS_LARGE, dpi) as i32;

        let batt_width = scale_by_dpi(BATTERY_WIDTH, dpi) as i32;
        let batt_height = scale_by_dpi(BATTERY_HEIGHT, dpi) as i32;

        let bump_width = scale_by_dpi(BUMP_WIDTH, dpi) as i32;
        let bump_height = scale_by_dpi(BUMP_HEIGHT, dpi) as i32;

        let edge_width = scale_by_dpi(THICKNESS_MEDIUM, dpi) as i32;

        let dx = (self.rect.width() as i32 - (batt_width + bump_width - border_thickness)) / 2;
        let dy = (self.rect.height() as i32 - batt_height) / 2;

        let mut pt = self.rect.min + pt!(dx, dy);
        let batt_rect = rect![pt, pt + pt!(batt_width, batt_height)];

        fb.draw_rectangle(&self.rect, WHITE);

        let max_fill_width = batt_width - 2 * border_thickness;
        let fill_width = (self.capacity.clamp(0.0, 100.0) / 100.0 * max_fill_width as f32) as i32;
        let fill_height = batt_height - 2 * border_thickness;
        let x_offset_edge = pt.x + border_thickness + fill_width;
        let x_offset_fill = x_offset_edge.saturating_sub(edge_width);

        fb.draw_rounded_rectangle_with_border(&batt_rect,
                                              &CornerSpec::Uniform(border_radius),
                                              &BorderSpec { thickness: border_thickness as u16,
                                                            color: BLACK },
                                              &|x, _| if x <= x_offset_fill { BATTERY_FILL }
                                                      else if x <= x_offset_edge { BLACK }
                                                      else { WHITE });

        pt += pt!(batt_width - border_thickness as i32, (batt_height - bump_height) / 2);
        let bump_rect = rect![pt, pt + pt!(bump_width, bump_height)];

        fb.draw_rounded_rectangle_with_border(&bump_rect,
                                              &CornerSpec::East(border_radius / 2),
                                              &BorderSpec { thickness: border_thickness as u16,
                                                            color: BLACK },
                                              &WHITE);

        pt = self.rect.min + pt!(dx, dy) + pt!(border_thickness);

        if self.status.is_wired() {
            let name = if self.status == Status::Charging { "plug" } else { "check_mark-small" };
            let pixmap = ICONS_PIXMAPS.get(name).unwrap();
            pt += pt!((max_fill_width - pixmap.width as i32) / 2,
                      (fill_height - pixmap.height as i32) / 2);
            fb.draw_blended_pixmap(pixmap, pt, BLACK);
        }
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
