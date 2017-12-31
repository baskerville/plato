use std::thread;
use std::time::Duration;
use device::CURRENT_DEVICE;
use geom::{Rectangle, CornerSpec, BorderSpec};
use font::{Fonts, font_from_style, NORMAL_STYLE};
use view::{View, Event, Hub, Bus, ViewId, Align};
use view::{THICKNESS_LARGE, BORDER_RADIUS_MEDIUM, CLOSE_IGNITION_DELAY_MS};
use view::button::Button;
use view::label::Label;
use framebuffer::Framebuffer;
use gesture::GestureEvent;
use color::{BLACK, WHITE};
use unit::scale_by_dpi;
use app::Context;

const LABEL_VALIDATE: &str = "OK";
const LABEL_CANCEL: &str = "Cancel";

pub struct Confirmation {
    rect: Rectangle,
    children: Vec<Box<View>>,
    id: ViewId,
    event: Event,
    will_close: bool,
}

impl Confirmation {
    pub fn new(id: ViewId, event: Event, text: String, fonts: &mut Fonts) -> Confirmation {
        let mut children = Vec::new();
        let dpi = CURRENT_DEVICE.dpi;
        let (width, height) = CURRENT_DEVICE.dims;

        let font = font_from_style(fonts, &NORMAL_STYLE, dpi);
        let x_height = font.x_heights.0 as i32;
        let padding = font.em() as i32;

        let min_message_width = width as i32 / 2;
        let max_message_width = width as i32 - 3 * padding;
        let max_button_width = width as i32 / 4;
        let button_height = 4 * x_height;

        let plan = font.plan(&text, Some(max_message_width as u32), None);

        let dialog_width = (plan.width as i32).max(min_message_width) + 3 * padding;
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

        children.push(Box::new(label) as Box<View>);

        let plan_cancel = font.plan(LABEL_CANCEL, Some(max_button_width as u32), None);
        let plan_validate = font.plan(LABEL_VALIDATE, Some(max_button_width as u32), None);

        let button_width = plan_validate.width.max(plan_cancel.width) as i32 + padding;

        let rect_cancel = rect![rect.min.x + padding,
                                rect.max.y - button_height - padding,
                                rect.min.x + button_width + 2 * padding,
                                rect.max.y - padding];

        let rect_validate = rect![rect.max.x - button_width - 2 * padding,
                                  rect.max.y - button_height - padding,
                                  rect.max.x - padding,
                                  rect.max.y - padding];

        let button_cancel = Button::new(rect_cancel, Event::Cancel, LABEL_CANCEL.to_string()); 
        children.push(Box::new(button_cancel) as Box<View>);

        let button_validate = Button::new(rect_validate, Event::Validate, LABEL_VALIDATE.to_string()); 
        children.push(Box::new(button_validate) as Box<View>);

        Confirmation {
            rect,
            children,
            id,
            event,
            will_close: false,
        }
    }
}

impl View for Confirmation {
    fn handle_event(&mut self, evt: &Event, hub: &Hub, bus: &mut Bus, _context: &mut Context) -> bool {
        match *evt {
            Event::Validate | Event::Cancel => {
                if self.will_close {
                    return true;
                }
                let hub2 = hub.clone();
                let id = self.id;
                thread::spawn(move || {
                    thread::sleep(Duration::from_millis(CLOSE_IGNITION_DELAY_MS));
                    hub2.send(Event::Close(id)).unwrap();
                });
                if let Event::Validate = *evt {
                    bus.push_back(self.event.clone());
                }
                self.will_close = true;
                true
            },
            Event::Gesture(GestureEvent::Tap { ref center, .. }) if !self.rect.includes(center) => {
                hub.send(Event::Close(self.id)).unwrap();
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

    fn id(&self) -> Option<ViewId> {
        Some(self.id)
    }
}
