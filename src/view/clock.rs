use std::sync::mpsc::Sender;
use std::time::Duration;
use std::thread;
use chrono::{Local, DateTime};
use device::CURRENT_DEVICE;
use font::{Fonts, font_from_style, NORMAL_STYLE};
use color::{BLACK, WHITE};
use geom::{Rectangle, CornerSpec};
use framebuffer::{Framebuffer, UpdateMode};
use view::{View, Event, ChildEvent};

const CLOCK_REFRESH_INTERVAL_MS: u64 = 60*60*1000;

pub struct Clock {
    rect: Rectangle,
    time: DateTime<Local>,
    corners: CornerSpec,
}

impl Clock {
    pub fn new(rect: &mut Rectangle, corners: CornerSpec, fonts: &mut Fonts, bus: &Sender<Event>) -> Clock {
        let font = font_from_style(fonts, &NORMAL_STYLE, CURRENT_DEVICE.dpi);
        let width = font.plan("88:88", None, None).width + font.em() as u32;
        rect.min.x = rect.max.x - width as i32;
        let bus2 = bus.clone();
        thread::spawn(move || {
            loop {
                thread::sleep(Duration::from_millis(CLOCK_REFRESH_INTERVAL_MS));
                bus2.send(Event::ChildEvent(ChildEvent::ClockTick)).unwrap();
            }
        });
        Clock {
            rect: *rect,
            time: Local::now(),
            corners: corners,
        }
    }
}

impl View for Clock {
    fn handle_event(&mut self, evt: &Event, bus: &mut Vec<ChildEvent>) -> bool {
        match *evt {
            Event::ChildEvent(ChildEvent::ClockTick) => {
                self.time = Local::now();
                bus.push(ChildEvent::Render(self.rect, UpdateMode::Gui));
                true
            },
            _ => false,
        }
    }

    fn render(&self, fb: &mut Framebuffer, fonts: &mut Fonts) {
        let dpi = CURRENT_DEVICE.dpi;
        let font = font_from_style(fonts, &NORMAL_STYLE, dpi);
        let plan = font.plan(&self.time.format("%H:%M").to_string(), None, None);
        let dx = (self.rect.width() as i32 - plan.width as i32) / 2;
        let dy = (self.rect.height() as i32 - font.x_heights.1 as i32) / 2;
        let pt = pt!(self.rect.min.x + dx, self.rect.max.y - dy);
        fb.draw_rounded_rectangle(&self.rect, &self.corners, WHITE);
        font.render(fb, BLACK, &plan, &pt);
    }

    fn rect(&self) -> &Rectangle {
        &self.rect
    }

    fn len(&self) -> usize {
        0
    }

    fn child(&self, _: usize) -> &View {
        self
    }

    fn child_mut(&mut self, _: usize) -> &mut View {
        self
    }
}
