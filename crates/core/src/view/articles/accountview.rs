use std::sync::Arc;
use std::thread;

use crate::articles;
use crate::color::{BLACK, WHITE};
use crate::context::Context;
use crate::device::CURRENT_DEVICE;
use crate::font::{font_from_style, Fonts, NORMAL_STYLE};
use crate::framebuffer::{Framebuffer, UpdateMode};
use crate::geom::{BorderSpec, CornerSpec, Rectangle};
use crate::gesture::GestureEvent;
use crate::unit::scale_by_dpi;
use crate::view::button::Button;
use crate::view::common::locate;
use crate::view::icon::Icon;
use crate::view::input_field::InputField;
use crate::view::keyboard::Keyboard;
use crate::view::label::Label;
use crate::view::{Align, Bus, Event, Hub, Id, RenderData, RenderQueue, View, ViewId, ID_FEEDER};
use crate::view::{BORDER_RADIUS_MEDIUM, SMALL_BAR_HEIGHT, THICKNESS_LARGE};

const LABEL_CLOSE: &str = "Close";
const LABEL_AUTHENTICATE: &str = "Authenticate";
const LABEL_SERVER: &str = "server: ";
const LABEL_USERNAME: &str = "username: ";
const LABEL_PASSWORD: &str = "password: ";

pub struct AccountWindow {
    id: Id,
    rect: Rectangle,
    children: Vec<Box<dyn View>>,
    api: String,
}

impl AccountWindow {
    pub fn new(context: &mut Context, api: String, server: String, name: String) -> AccountWindow {
        let id = ID_FEEDER.next();
        let fonts = &mut context.fonts;
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
        let window_height = small_height * 6 + 2 * padding;

        let dx = (width as i32 - window_width) / 2;
        let dy = (height as i32 - window_height) / 4;

        let rect = rect![dx, dy, dx + window_width, dy + window_height];

        let close_icon = Icon::new(
            "close",
            rect![
                rect.max.x - small_height,
                rect.min.y + thickness,
                rect.max.x - thickness,
                rect.min.y + small_height
            ],
            Event::Close(ViewId::ArticlesSettings),
        )
        .corners(Some(CornerSpec::Detailed {
            north_west: 0,
            north_east: border_radius - thickness,
            south_east: 0,
            south_west: 0,
        }));
        children.push(Box::new(close_icon) as Box<dyn View>);

        let label = Label::new(
            rect![
                rect.min.x + small_height,
                rect.min.y + thickness,
                rect.max.x - small_height,
                rect.min.y + small_height
            ],
            "Log in to ".to_owned() + &name,
            Align::Center,
        );
        children.push(Box::new(label) as Box<dyn View>);

        let max_table_labels_width = {
            let font = font_from_style(fonts, &NORMAL_STYLE, dpi);
            [LABEL_SERVER, LABEL_USERNAME, LABEL_PASSWORD]
                .iter()
                .map(|t| font.plan(t, None, None).width)
                .max()
                .unwrap() as i32
        };

        let max_button_label_width = {
            let font = font_from_style(fonts, &NORMAL_STYLE, dpi);
            [LABEL_CLOSE, LABEL_AUTHENTICATE]
                .iter()
                .map(|t| font.plan(t, None, None).width)
                .max()
                .unwrap() as i32
        };

        let label_server = Box::new(Label::new(
            rect![
                rect.min.x + small_height,
                rect.min.y + thickness + small_height,
                rect.max.x - small_height,
                rect.min.y + small_height * 2
            ],
            LABEL_SERVER.to_string(),
            Align::Left(0),
        ));
        children.push(label_server as Box<dyn View>);

        let input_server = InputField::new(
            rect![
                rect.min.x + small_height + max_table_labels_width,
                rect.min.y + thickness + small_height,
                rect.max.x - small_height,
                rect.min.y + small_height * 2
            ],
            ViewId::ArticleInputServer,
        )
        .border(true)
        .text(&server, context);
        children.push(Box::new(input_server) as Box<dyn View>);

        let label_username = Label::new(
            rect![
                rect.min.x + small_height,
                rect.min.y + thickness + small_height * 2,
                rect.max.x - small_height,
                rect.min.y + small_height * 3
            ],
            LABEL_USERNAME.to_string(),
            Align::Left(0),
        );
        children.push(Box::new(label_username) as Box<dyn View>);

        let input_username = InputField::new(
            rect![
                rect.min.x + small_height + max_table_labels_width,
                rect.min.y + thickness + small_height * 2,
                rect.max.x - small_height,
                rect.min.y + small_height * 3
            ],
            ViewId::ArticleInputUsername,
        )
        .text("", context)
        .border(true);
        children.push(Box::new(input_username) as Box<dyn View>);

        let label_password = Label::new(
            rect![
                rect.min.x + small_height,
                rect.min.y + thickness + small_height * 3,
                rect.max.x - small_height,
                rect.min.y + small_height * 4
            ],
            LABEL_PASSWORD.to_string(),
            Align::Left(0),
        );
        children.push(Box::new(label_password) as Box<dyn View>);

        let input_password = InputField::new(
            rect![
                rect.min.x + small_height + max_table_labels_width,
                rect.min.y + thickness + small_height * 3,
                rect.max.x - small_height,
                rect.min.y + small_height * 4
            ],
            ViewId::ArticleInputPassword,
        )
        .text("", context)
        .border(true);
        children.push(Box::new(input_password) as Box<dyn View>);

        let button_y = rect.min.y + small_height * 5;
        let button_height = 4 * x_height;

        let button_close = Button::new(
            rect![
                rect.min.x + 3 * padding,
                button_y + small_height - button_height,
                rect.min.x + 5 * padding + max_button_label_width,
                button_y + small_height
            ],
            Event::Close(ViewId::ArticlesSettings),
            LABEL_CLOSE.to_string(),
        );
        children.push(Box::new(button_close) as Box<dyn View>);

        let button_authenticate = Button::new(
            rect![
                rect.max.x - 5 * padding - max_button_label_width,
                button_y + small_height - button_height,
                rect.max.x - 3 * padding,
                button_y + small_height
            ],
            Event::Authenticate,
            LABEL_AUTHENTICATE.to_string(),
        )
        .disabled(false);
        children.push(Box::new(button_authenticate) as Box<dyn View>);

        let label_status = Label::new(
            rect![
                rect.min.x + small_height,
                rect.min.y + thickness + small_height * 4,
                rect.max.x - small_height,
                rect.min.y + small_height * 5
            ],
            "".to_string(),
            Align::Left(0),
        );
        children.push(Box::new(label_status) as Box<dyn View>);

        AccountWindow {
            id,
            rect,
            children,
            api,
        }
    }

    fn toggle_keyboard(
        &mut self,
        enable: bool,
        hub: &Hub,
        rq: &mut RenderQueue,
        context: &mut Context,
    ) {
        if let Some(index) = locate::<Keyboard>(self) {
            if enable {
                return;
            }

            let mut rect = *self.child(index).rect();
            rect.absorb(self.child(index - 1).rect());
            self.children.drain(index - 1..=index);

            context.kb_rect = Rectangle::default();
            rq.add(RenderData::expose(rect, UpdateMode::Gui));
            hub.send(Event::Focus(None)).ok();
        } else {
            if !enable {
                return;
            }

            let (width, height) = context.display.dims;
            let mut kb_rect = rect![0, 0, width as i32, height as i32];

            let keyboard = Keyboard::new(&mut kb_rect, false, context);
            self.children.push(Box::new(keyboard) as Box<dyn View>);

            rq.add(RenderData::expose(kb_rect, UpdateMode::Gui));
        }
    }
}

impl View for AccountWindow {
    fn handle_event(
        &mut self,
        evt: &Event,
        hub: &Hub,
        bus: &mut Bus,
        rq: &mut RenderQueue,
        context: &mut Context,
    ) -> bool {
        match *evt {
            Event::Gesture(GestureEvent::Tap(center)) if !self.rect.includes(center) => {
                bus.push_back(Event::Close(ViewId::ArticlesSettings));
                true
            }
            Event::Gesture(..) => true,
            Event::Authenticate => {
                let server = self.children[3]
                    .downcast_ref::<InputField>()
                    .unwrap()
                    .text_value()
                    .to_string();

                let username = self.children[5]
                    .downcast_ref::<InputField>()
                    .unwrap()
                    .text_value()
                    .to_string();

                let password = self.children[7]
                    .downcast_ref::<InputField>()
                    .unwrap()
                    .text_value()
                    .to_string();

                let btn = self.children[9].as_mut().downcast_mut::<Button>().unwrap();
                btn.update(true, rq);

                let label_status = self.children[10].as_mut().downcast_mut::<Label>().unwrap();
                label_status.update("authenticating...", rq);

                let hub2 = Arc::new(hub.clone());
                let api = self.api.clone();
                thread::spawn(move || {
                    hub2.send(Event::ArticlesAuth(articles::authenticate(
                        api, server, username, password,
                    )))
                    .ok();
                });

                true
            }
            Event::Focus(v) => {
                if v.is_some() {
                    self.toggle_keyboard(true, hub, rq, context);
                }
                true
            }
            Event::ArticlesAuth(ref result) => {
                match result {
                    Ok(_auth) => {
                        // Let the Articles view deal with this event (closing
                        // the auth popup etc).
                        false
                    }
                    Err(msg) => {
                        // Show the error message in the label.
                        let label_status =
                            self.children[10].as_mut().downcast_mut::<Label>().unwrap();
                        label_status.update(msg, rq);

                        // Re-enable the button for the next attempt.
                        let btn = self.children[9].as_mut().downcast_mut::<Button>().unwrap();
                        btn.update(false, rq);

                        // We processed this event, don't send it to the
                        // Articles view.
                        true
                    }
                }
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
