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
    let (ty, ry) = mpsc::channel();
    let tx2 = tx.clone();

    thread::spawn(move || {
        while let Ok(ge) = input.recv() {
            tx.send(Event::GestureEvent(ge)).unwrap();
        }
    });

    thread::spawn(move || {
        while let Ok(ce) = ry.recv() {
            tx2.send(Event::ChildEvent(ce)).unwrap();
        }
    });

    let mut fb_rect = fb.rect();
    fb.clear(WHITE);
    let tok = fb.update(&fb_rect, UpdateMode::Full).unwrap();

    let mut fonts = Fonts::default();

    let mut view: Box<View> = Box::new(Home::new(fb_rect));
    render(view.as_ref(), &mut fb_rect, &mut fb, &mut fonts);
    fb.wait(tok).unwrap();
    let tok = fb.update(&fb_rect, UpdateMode::Gui).unwrap();
    fb.wait(tok).unwrap();

    let mut updating = HashMap::new();

    println!("{} is running on a Kobo {}.", APP_NAME,
                                            CURRENT_DEVICE.model);
    println!("The framebuffer resolution is {}x{}.", fb_rect.width(),
                                                     fb_rect.height());

    while let Ok(evt) = rx.recv() {
        match evt {
            Event::GestureEvent(ge) => {
                match ge {
                    GestureEvent::Relay(de) => {
                        match de {
                            DeviceEvent::Button {
                                code: ButtonCode::Power,
                                status: ButtonStatus::Pressed, ..
                            } => break,
                            _ => (),
                        }
                    }
                    _ => {
                        handle_gesture(view.as_mut(), &evt, &ty);
                    },
                    
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
                        handle_event(view.as_mut(), &evt, &ty);
                    },
                }
            },
        }
    }
}

// Moving from bottom to top
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

// Moving from top to bottom
fn handle_gesture(view: &mut View, evt: &Event, bus: &Sender<ChildEvent>) -> bool {
    for i in (0..view.len()).rev() {
        if handle_gesture(view.child_mut(i), evt, bus) {
            return true;
        }
    }
    view.handle_event(evt, bus)
}

fn handle_event(view: &mut View, evt: &Event, bus: &Sender<ChildEvent>) -> bool {
    for i in 0..view.len() {
        if handle_event(view.child_mut(i), evt, bus) {
            return true;
        }
    }
    view.handle_event(evt, bus)
}
