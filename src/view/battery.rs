use device::CURRENT_DEVICE;
use framebuffer::{Framebuffer, UpdateMode};
use geom::{Rectangle, BorderSpec, CornerSpec};
use color::{BLACK, WHITE, BATTERY_FILL_CHARGING, BATTERY_FILL_CHARGED};
use view::{View, Event, Hub, Bus};
use view::THICKNESS_LARGE;
use view::icon::ICONS_PIXMAPS;
use battery::Status;
use unit::scale_by_dpi;
use font::Fonts;
use app::Context;

const BATTERY_WIDTH: f32 = 58.0;
const BATTERY_HEIGHT: f32 = 28.0;
const BUMP_WIDTH: f32 = 10.0;
const BUMP_HEIGHT: f32 = 14.0;
const EDGE_WIDTH: f32 = 2.0;
const PLUG_OFFSET: f32 = 8.0;
const INNER_PADDING: f32 = 2.0;

pub struct Battery {
    rect: Rectangle,
    children: Vec<Box<View>>,
    status: Status,
    capacity: f32,
}

impl Battery {
    pub fn new(rect: Rectangle, capacity: f32, status: Status) -> Battery {
        Battery {
            rect,
            children: vec![],
            capacity,
            status,
        }
    }
}

impl View for Battery {
    fn handle_event(&mut self, evt: &Event, hub: &Hub, _bus: &mut Bus, context: &mut Context) -> bool {
        match *evt {
            Event::BatteryTick => {
                self.capacity = context.battery.capacity().unwrap_or(self.capacity);
                self.status = context.battery.status().unwrap_or(self.status);
                hub.send(Event::Render(self.rect, UpdateMode::Gui)).unwrap();
                true
            },
            _ => false,
        }
    }

    fn render(&self, fb: &mut Framebuffer, fonts: &mut Fonts) {
        let dpi = CURRENT_DEVICE.dpi;

        let border_radius = scale_by_dpi(THICKNESS_LARGE / 2.0, dpi) as i32;
        let border_thickness = scale_by_dpi(THICKNESS_LARGE, dpi) as i32;

        let batt_width = scale_by_dpi(BATTERY_WIDTH, dpi) as i32;
        let batt_height = scale_by_dpi(BATTERY_HEIGHT, dpi) as i32;

        let bump_width = scale_by_dpi(BUMP_WIDTH, dpi) as i32;
        let bump_height = scale_by_dpi(BUMP_HEIGHT, dpi) as i32;

        let edge_width = scale_by_dpi(EDGE_WIDTH, dpi) as i32;
        let plug_offset = scale_by_dpi(PLUG_OFFSET, dpi) as i32;

        let inner_padding = if self.status == Status::Discharging { 
            scale_by_dpi(INNER_PADDING, dpi) as i32
        } else {
            0
        };

        let background = if self.status == Status::Charged {
            BATTERY_FILL_CHARGED
        } else {
            BATTERY_FILL_CHARGING
        };

        let batt_fill_height = batt_height - 2 * (border_thickness + inner_padding);
        let bump_fill_height = bump_height - 2 * (border_thickness + inner_padding);

        let dx = (self.rect.width() as i32 - (batt_width + bump_width - border_thickness)) / 2;
        let dy = (self.rect.height() as i32 - batt_height) / 2;

        let pt = self.rect.min + pt!(dx, dy);
        let batt_rect = rect![pt, pt + pt!(batt_width, batt_height)];

        fb.draw_rectangle(&self.rect, WHITE);

        fb.draw_rounded_rectangle_with_border(&batt_rect,
                                              &CornerSpec::Uniform(border_radius),
                                              &BorderSpec { thickness: border_thickness as u16,
                                                            color: BLACK },
                                              &WHITE);

        let pt = pt + pt!(batt_width - border_thickness as i32, (batt_height - bump_height) / 2);
        let bump_rect = rect![pt, pt + pt!(bump_width, bump_height)];

        fb.draw_rounded_rectangle_with_border(&bump_rect,
                                              &CornerSpec::Uniform(border_radius),
                                              &BorderSpec { thickness: border_thickness as u16,
                                                            color: BLACK },
                                              &WHITE);

        let pt = pt + pt!(0, border_thickness);
        let hole_rect = rect![pt, pt + pt!(border_thickness,
                                           bump_height - 2 * border_thickness)];

        fb.draw_rectangle(&hole_rect, WHITE);
        
        let max_batt_fill_width = batt_width - 2 * border_thickness - 2 * inner_padding;
        let max_bump_fill_width = bump_width - border_thickness;
        let max_fill_width = max_batt_fill_width + max_bump_fill_width;

        let fill_width = (self.capacity / 100.0 * max_fill_width as f32) as i32;

        let batt_fill_width = fill_width.min(max_batt_fill_width);

        let pt = self.rect.min + pt!(dx, dy) + pt!(border_thickness + inner_padding);
        let fill_rect = rect![pt, pt + pt!(batt_fill_width, batt_fill_height)];
        fb.draw_rectangle(&fill_rect, background);

        if fill_width > max_batt_fill_width {
            let bump_fill_width = fill_width - max_batt_fill_width;
            let pt = pt + pt!(max_batt_fill_width, (batt_fill_height - bump_fill_height) / 2);
            let fill_rect = rect![pt, pt + pt!(bump_fill_width, bump_fill_height)];
            fb.draw_rectangle(&fill_rect, background);
        } else if fill_width > edge_width {
            let pt = pt + pt!(fill_width - edge_width, 0);
            let edge_rect = rect![pt, pt + pt!(edge_width, batt_fill_height)];
            fb.draw_rectangle(&edge_rect, BLACK);
        }

        if self.status != Status::Discharging {
            let foreground = if self.status == Status::Charging { BLACK } else { WHITE };
            let pixmap = ICONS_PIXMAPS.get("plug").unwrap();
            let pt = pt + pt!(plug_offset, (batt_fill_height - pixmap.height) / 2);
            fb.draw_blended_pixmap(pixmap, &pt, foreground);
        }
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
