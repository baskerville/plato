use std::thread;
use std::sync::mpsc;
use std::sync::mpsc::Sender;
use std::collections::HashMap;
use framebuffer::{Framebuffer, UpdateMode};
use gesture::GestureEvent;
use input::{raw_events, device_events};
use input::{DeviceEvent, ButtonCode, ButtonStatus};
use gesture::gesture_events;
use device::CURRENT_DEVICE;
use font::Fonts;
use color::WHITE;
use view::{View, Event, ChildEvent};
use view::home::Home;
use geom::Rectangle;

pub const APP_NAME: &str = "Plato";

pub fn run() {
    let mut fb = Framebuffer::new("/dev/fb0").unwrap();
    let paths = vec!["/dev/input/event0".to_string(),
                     "/dev/input/event1".to_string()];
    let input = gesture_events(device_events(raw_events(paths), fb.dims()));

    let (tx, rx) = mpsc::channel();
    let tx2 = tx.clone();

    // forward gesture events on the main receiver
    thread::spawn(move || {
        while let Ok(ge) = input.recv() {
            tx.send(Event::GestureEvent(ge)).unwrap();
        }
    });

    let mut fb_rect = fb.rect();
    fb.clear(WHITE);
    let tok = fb.update(&fb_rect, UpdateMode::Full).unwrap();

    let mut fonts = Fonts::default();

    let mut view: Box<View> = Box::new(Home::new(fb_rect, &mut fonts, &tx2));
    render(view.as_ref(), &mut fb_rect, &mut fb, &mut fonts);
    fb.wait(tok).unwrap();
    let tok = fb.update(&fb_rect, UpdateMode::Gui).unwrap();
    fb.wait(tok).unwrap();

    let mut updating = HashMap::new();

    println!("{} is running on a Kobo {}.", APP_NAME,
                                            CURRENT_DEVICE.model);
    println!("The framebuffer resolution is {}x{}.", fb_rect.width(),
                                                     fb_rect.height());

    let mut bus = Vec::with_capacity(1);

    while let Ok(evt) = rx.recv() {
        match evt {
            Event::GestureEvent(ge) => {
                match ge {
                    GestureEvent::Relay(DeviceEvent::Button { code: ButtonCode::Power,
                                                              status: ButtonStatus::Pressed, .. }) => break,
                    _ => { handle_event(view.as_mut(), &evt, &mut bus); },
                }
            },

            Event::ChildEvent(ce) => {
                match ce {
                    ChildEvent::Render(mut rect, mode) => {
                        render(view.as_ref(), &mut rect, &mut fb, &mut fonts);
                        let mut finished = Vec::with_capacity(updating.len());
                        for (tok, urect) in &updating {
                            if rect.overlaps(urect) {
                                fb.wait(*tok).unwrap();
                                finished.push(*tok);
                            }
                        }
                        for tok in &finished {
                            updating.remove(&tok);
                        }
                        if let Ok(tok) = fb.update(&rect, mode) {
                            updating.insert(tok, rect);
                        }
                    },
                    ChildEvent::ReplaceRoot(rk) => {
                    },
                    _ => {
                        handle_event(view.as_mut(), &evt, &mut bus);
                    },
                }
            },
        }

        while let Some(ce) = bus.pop() {
            tx2.send(Event::ChildEvent(ce)).unwrap();
        }
    }
}

// From bottom to top
fn render(view: &View, rect: &mut Rectangle, fb: &mut Framebuffer, fonts: &mut Fonts) {
    if view.len() > 0 {
        for i in 0..view.len() {
            render(view.child(i), rect, fb, fonts);
        }
    } else {
        if view.rect().overlaps(rect) {
            view.render(fb, fonts);
            rect.absorb(view.rect());
        }
    }
}

// From top to bottom
fn handle_event(view: &mut View, evt: &Event, parent_bus: &mut Vec<ChildEvent>) -> bool {
    if view.might_skip(evt) {
        return false;
    }

    let mut child_bus: Vec<ChildEvent> = Vec::with_capacity(1);

    for i in (0..view.len()).rev() {
        if handle_event(view.child_mut(i), evt, &mut child_bus) {
            break;
        }
    }

    for child_evt in child_bus {
        if !view.handle_event(&Event::ChildEvent(child_evt), parent_bus) {
            parent_bus.push(child_evt);
        }
    }

    view.handle_event(evt, parent_bus)
}
