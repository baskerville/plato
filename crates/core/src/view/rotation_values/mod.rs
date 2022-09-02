use std::mem;
use crate::device::CURRENT_DEVICE;
use crate::geom::{Point, Rectangle};
use crate::view::{View, Event, Hub, Bus, RenderQueue, RenderData};
use crate::view::{Id, ID_FEEDER};
use crate::gesture::GestureEvent;
use crate::framebuffer::{Framebuffer, UpdateMode};
use crate::font::{Fonts, font_from_style, NORMAL_STYLE, DISPLAY_STYLE};
use crate::color::{BLACK, WHITE, GRAY07};
use crate::context::Context;

const MESSAGE_1: &str = "Hold you device in portrait mode\n\
                         with the Kobo logo at the bottom,\n\
                         and tap each gray corner\n\
                         in clockwise order\n\
                         starting from the top left.";
const MESSAGE_2: &str = "Tap the black corner.";
const CORNERS_COUNT: usize = 4;

pub struct RotationValues {
    id: Id,
    rect: Rectangle,
    children: Vec<Box<dyn View>>,
    mirror_x: bool,
    mirror_y: bool,
    swap_xy: bool,
    width: i32,
    height: i32,
    read_rotation: i8,
    written_rotation: i8,
    finished: bool,
    taps: Vec<Point>,
}

impl RotationValues {
    pub fn new(rect: Rectangle, rq: &mut RenderQueue, context: &mut Context) -> RotationValues {
        let id = ID_FEEDER.next();
        let rotation = context.display.rotation;
        let (width, height) = context.display.dims;
        let (mirror_x, mirror_y) = CURRENT_DEVICE.should_mirror_axes(rotation);
        let swap_xy = CURRENT_DEVICE.should_swap_axes(rotation);
        rq.add(RenderData::new(id, rect, UpdateMode::Full));
        RotationValues {
            id,
            rect,
            children: Vec::new(),
            mirror_x,
            mirror_y,
            swap_xy,
            width: width as i32,
            height: height as i32,
            read_rotation: context.fb.rotation(),
            written_rotation: rotation,
            finished: false,
            taps: Vec::new(),
        }
    }
}

impl View for RotationValues {
    fn handle_event(&mut self, evt: &Event, hub: &Hub, _bus: &mut Bus, rq: &mut RenderQueue, context: &mut Context) -> bool {
        match *evt {
            Event::Gesture(GestureEvent::Tap(mut pt)) if !self.finished => {
                if self.mirror_x {
                    pt.x = self.width - 1 - pt.x;
                }

                if self.mirror_y {
                    pt.y = self.height - 1 - pt.y;
                }

                if self.swap_xy {
                    mem::swap(&mut pt.x, &mut pt.y);
                }

                println!("Tap {} {:?}", pt, context.fb.dims());

                self.taps.push(pt);
                self.finished = self.taps.len() >= 2 * CORNERS_COUNT;

                if self.taps.len() >= CORNERS_COUNT {
                    let rotation = if self.finished {
                        self.written_rotation
                    } else {
                        (self.taps.len() - CORNERS_COUNT) as i8
                    };
                    context.fb.set_rotation(rotation)
                           .map_err(|e| eprintln!("Can't set rotation: {:#}.", e))
                           .ok();
                    if context.fb.rotation() == self.read_rotation {
                        self.written_rotation = rotation;
                    }
                    self.children.clear();
                    self.rect = context.fb.rect();
                }

                if self.finished {
                    // Infer the startup rotation and the mirroring scheme.
                    let first = self.taps[0];
                    let startup_rotation = self.taps[CORNERS_COUNT..2*CORNERS_COUNT].iter()
                                               .enumerate()
                                               .min_by_key(|(_, &pt)| first.dist2(pt))
                                               .map(|(i, _)| i)
                                               .unwrap();
                    let origin = self.taps[CORNERS_COUNT..2*CORNERS_COUNT].iter()
                                     .enumerate()
                                     .min_by_key(|(_, pt)| pt.x + pt.y)
                                     .map(|(i, _)| i)
                                     .unwrap();
                    let center = (origin + 2) % 4;
                    let next = self.taps[CORNERS_COUNT + (center + 1) % 4];
                    let polarity = 2 * ((origin + startup_rotation) as i8 % 2) - 1;
                    let dir = if next.x < next.y { polarity } else { -polarity };
                    println!("Startup rotation: {}.", startup_rotation);
                    println!("Mirroring scheme: ({}, {}).", center, dir);
                    hub.send(Event::Back).ok();
                } else {
                    rq.add(RenderData::new(self.id, self.rect, UpdateMode::Full));
                }

                true
            },
            _ => false,
        }
    }

    fn render(&self, fb: &mut dyn Framebuffer, _rect: Rectangle, fonts: &mut Fonts) {
        let dpi = CURRENT_DEVICE.dpi;
        let width = self.rect.width() as i32;
        let height = self.rect.height() as i32;
        let side = width.min(height) / 4;

        fb.draw_rectangle(&self.rect, WHITE);

        let step = 1 + (self.taps.len() % CORNERS_COUNT);
        let msg = format!("{} / {}", step, CORNERS_COUNT);
        let font = font_from_style(fonts, &DISPLAY_STYLE, dpi);
        let plan = font.plan(msg, None, Some(&["lnum".to_string()]));
        let dx = (width - plan.width as i32) / 2;
        let mut dy = (height - font.x_heights.1 as i32) / 3;

        font.render(fb, BLACK, &plan, self.rect.min + pt!(dx, dy));

        dy += 4 * (font.x_heights.1 as i32) / 3;
        let msg = if self.taps.len() < CORNERS_COUNT {
            MESSAGE_1
        } else {
            MESSAGE_2
        };
        let font = font_from_style(fonts, &NORMAL_STYLE, dpi);

        for line in msg.lines() {
            let plan = font.plan(line, None, None);
            let dx = (width - plan.width as i32) / 2;
            font.render(fb, BLACK, &plan, self.rect.min + pt!(dx, dy));
            dy += 3 * font.x_heights.0 as i32;
        }

        if self.taps.len() < CORNERS_COUNT {
            fb.draw_triangle(&[pt!(0, 0), pt!(side, 0), pt!(0, side)], GRAY07);
            fb.draw_triangle(&[pt!(width - 1, 0), pt!(width - 1, side),
                               pt!(width - 1 - side, 0)], GRAY07);
            fb.draw_triangle(&[pt!(width - 1, height - 1), pt!(width - 1 - side, height - 1),
                               pt!(width - 1, height - 1 - side)], GRAY07);
            fb.draw_triangle(&[pt!(0, height - 1), pt!(0, height - 1 - side),
                               pt!(side, height - 1)], GRAY07);
        } else {
            fb.draw_triangle(&[pt!(0, 0), pt!(side, 0), pt!(0, side)], BLACK);
        }
    }

    fn might_rotate(&self) -> bool {
        false
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
