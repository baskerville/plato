use std::sync::mpsc::Sender;
use std::time::Duration;
use std::thread;
use geom::Rectangle;
use view::{View, ChildEvent};

const REFRESH_INTERVAL_MS: u64 = 60*60*1000;
const EM_WIDTH: f32 = 4.0;

struct Clock {
    rect: Rectangle,
}

impl Clock {
    pub fn new(rect: Rectangle, bus: &Sender<ChildEvent>) -> Clock {
        let bus2 = bus.clone();
        thread::spawn(move || {
            loop {
                thread::sleep(Duration::from_millis(REFRESH_INTERVAL_MS));
                bus2.send(ChildEvent::ClockTick).unwrap();
            }
        });
        Clock {
            rect: rect,
        }
    }
}
