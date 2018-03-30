use std::thread;
use std::time::Duration;
use device::{CURRENT_DEVICE, BAR_SIZES};
use framebuffer::{Framebuffer, UpdateMode};
use geom::{Rectangle, CornerSpec, BorderSpec};
use font::{Fonts, font_from_style, NORMAL_STYLE};
use color::{BLACK, WHITE, TEXT_NORMAL};
use view::{View, Event, Hub, Bus, ViewId};
use view::{THICKNESS_LARGE, BORDER_RADIUS_MEDIUM};
use gesture::GestureEvent;
use input::DeviceEvent;
use unit::scale_by_dpi;
use app::Context;

const NOTIFICATION_CLOSE_DELAY: Duration = Duration::from_secs(4);

pub struct Notification {
    rect: Rectangle,
    children: Vec<Box<View>>,
    text: String,
    id: ViewId,
}

impl Notification {
    pub fn new(id: ViewId, text: String, index: &mut u8, fonts: &mut Fonts, hub: &Hub) -> Notification {
        let hub2 = hub.clone();

        thread::spawn(move || {
            thread::sleep(NOTIFICATION_CLOSE_DELAY);
            hub2.send(Event::Close(id)).unwrap();
        });

        let dpi = CURRENT_DEVICE.dpi;
        let (width, height) = CURRENT_DEVICE.dims;
        let &(small_height, _) = BAR_SIZES.get(&(height, dpi)).unwrap();

        let font = font_from_style(fonts, &NORMAL_STYLE, dpi);
        let x_height = font.x_heights.0 as i32;
        let padding = font.em() as i32;

        let max_message_width = width as i32 - 5 * padding;
        let plan = font.plan(&text, Some(max_message_width as u32), None);

        let dialog_width = plan.width as i32 + 3 * padding;
        let dialog_height = 7 * x_height;

        let side = (*index / 3) % 2;
        let dx = if side == 0 {
            width as i32 - dialog_width - padding
        } else {
            padding
        };
        let dy = small_height as i32 + padding + (*index % 3) as i32 * (dialog_height + padding);

        let rect = rect![dx, dy,
                         dx + dialog_width, dy + dialog_height];

        hub.send(Event::Render(rect, UpdateMode::Gui)).unwrap();

        *index = index.wrapping_add(1);

        Notification {
            rect,
            children: vec![],
            text,
            id,
        }
    }
}

impl View for Notification {
    fn handle_event(&mut self, evt: &Event, _hub: &Hub, _bus: &mut Bus, _context: &mut Context) -> bool {
        match *evt {
            Event::Gesture(GestureEvent::Tap { ref center, .. }) if self.rect.includes(center) => true,
            Event::Gesture(GestureEvent::Swipe { ref start, .. }) if self.rect.includes(start) => true,
            Event::Device(DeviceEvent::Finger { ref position, .. }) if self.rect.includes(position) => true,
            _ => false,
        }
    }

    fn render(&self, fb: &mut Framebuffer, fonts: &mut Fonts) {
        let dpi = CURRENT_DEVICE.dpi;

        let border_radius = scale_by_dpi(BORDER_RADIUS_MEDIUM, dpi) as i32;
        let border_thickness = scale_by_dpi(THICKNESS_LARGE, dpi) as u16;

        fb.draw_rounded_rectangle_with_border(&self.rect,
                                              &CornerSpec::Uniform(border_radius),
                                              &BorderSpec { thickness: border_thickness,
                                                            color: BLACK },
                                              &WHITE);

        let font = font_from_style(fonts, &NORMAL_STYLE, dpi);
        let plan = font.plan(&self.text, None, None);
        let x_height = font.x_heights.0 as i32;

        let dx = (self.rect.width() - plan.width) as i32 / 2;
        let dy = (self.rect.height() as i32 - x_height) / 2;
        let pt = pt!(self.rect.min.x + dx, self.rect.max.y - dy);

        font.render(fb, TEXT_NORMAL[1], &plan, &pt);
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

    fn id(&self) -> Option<ViewId> {
        Some(self.id)
    }
}
