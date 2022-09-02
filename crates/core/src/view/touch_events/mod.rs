use crate::device::CURRENT_DEVICE;
use crate::geom::{Rectangle, Region, Dir, CornerSpec};
use crate::view::icon::Icon;
use crate::view::notification::Notification;
use crate::view::{View, Event, Hub, Bus, RenderQueue, RenderData};
use crate::view::{Id, ID_FEEDER};
use crate::view::SMALL_BAR_HEIGHT;
use crate::framebuffer::{Framebuffer, UpdateMode};
use crate::font::Fonts;
use crate::unit::scale_by_dpi;
use crate::color::{BLACK, WHITE, GRAY05, GRAY10};
use crate::context::Context;

pub struct TouchEvents {
    id: Id,
    rect: Rectangle,
    children: Vec<Box<dyn View>>,
    strip_width: f32,
    corner_width: f32,
}

impl TouchEvents {
    pub fn new(rect: Rectangle, rq: &mut RenderQueue, context: &mut Context) -> TouchEvents {
        let id = ID_FEEDER.next();
        let mut children = Vec::new();
        let dpi = CURRENT_DEVICE.dpi;
        let small_height = scale_by_dpi(SMALL_BAR_HEIGHT, dpi) as i32;
        let dx = (rect.width() as i32 - small_height) / 2;
        let dy = (rect.height() as i32 - small_height) / 3;
        let icon_rect = rect![rect.min.x + dx, rect.min.y + dy,
                              rect.min.x + dx + small_height, rect.min.y + dy + small_height];
        let icon = Icon::new("back",
                             icon_rect,
                             Event::Back)
                        .corners(Some(CornerSpec::Uniform(small_height/2)));
        children.push(Box::new(icon) as Box<dyn View>);
        rq.add(RenderData::new(id, rect, UpdateMode::Full));
        let strip_width = context.settings.reader.strip_width;
        let corner_width = context.settings.reader.corner_width;
        TouchEvents {
            id,
            rect,
            children,
            strip_width,
            corner_width,
        }
    }

}

impl View for TouchEvents {
    fn handle_event(&mut self, evt: &Event, hub: &Hub, _bus: &mut Bus, rq: &mut RenderQueue, context: &mut Context) -> bool {
        match *evt {
            Event::Gesture(ge) => {
                let notif = Notification::new(ge.to_string(), hub, rq, context);
                self.children.push(Box::new(notif) as Box<dyn View>);
                true
            },
            _ => false,
        }
    }

    fn render(&self, fb: &mut dyn Framebuffer, rect: Rectangle, _fonts: &mut Fonts) {
        for x in rect.min.x..rect.max.x {
            for y in rect.min.y..rect.max.y {
                let color = match Region::from_point(pt!(x, y), self.rect, self.strip_width, self.corner_width) {
                    Region::Corner(..) => BLACK,
                    Region::Strip(Dir::West) | Region::Strip(Dir::East) => GRAY05,
                    Region::Strip(Dir::South) | Region::Strip(Dir::North) => GRAY10,
                    Region::Center => WHITE,
                };
                fb.set_pixel(x as u32, y as u32, color);
            }
        }
    }

    fn render_rect(&self, rect: &Rectangle) -> Rectangle {
        rect.intersection(&self.rect)
            .unwrap_or(self.rect)
    }

    fn is_background(&self) -> bool {
        true
    }

    fn resize(&mut self, rect: Rectangle, hub: &Hub, rq: &mut RenderQueue, context: &mut Context) {
        let dpi = CURRENT_DEVICE.dpi;
        let small_height = scale_by_dpi(SMALL_BAR_HEIGHT, dpi) as i32;
        let dx = (rect.width() as i32 - small_height) / 2;
        let dy = (rect.height() as i32 - small_height) / 3;
        let icon_rect = rect![rect.min.x + dx, rect.min.y + dy,
                              rect.min.x + dx + small_height, rect.min.y + dy + small_height];
        self.children[0].resize(icon_rect, hub, rq, context);

        // Floating windows.
        for i in 1..self.children.len() {
            self.children[i].resize(rect, hub, rq, context);
        }

        self.rect = rect;
        rq.add(RenderData::new(self.id, self.rect, UpdateMode::Full));
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
