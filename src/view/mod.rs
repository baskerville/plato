//! Views are organized as a tree. A view might receive events and render itself. The z level of
//! the n-th child of a view is less or equal to the z level of its n+1-th child. Therefore we
//! deliver gesture events (and events in general) starting from the last children to the first.
//! The rectangles of the tiled layer constitute a partition of the screen rectangle.

pub mod filler;
pub mod icon;
pub mod input_field;
pub mod menu;
pub mod clock;
pub mod keyboard;
pub mod key;
pub mod home;
pub mod reader;

use std::path::PathBuf;
use std::fmt::{self, Debug};
use downcast_rs::Downcast;
use font::Fonts;
use framebuffer::{Framebuffer, UpdateMode, Bitmap};
use gesture::GestureEvent;
use metadata::SortMethod;
use view::key::KeyKind;
use geom::{LinearDir, CycleDir, Rectangle};

#[derive(Debug, Clone)]
pub enum Event {
    Gesture(GestureEvent),
    Keyboard(KeyboardEvent),
    Render(Rectangle, UpdateMode),
    Page(CycleDir),
    GoTo(usize),
    Chapter(CycleDir),
    Open(PathBuf),
    Remove(PathBuf),
    Sort(SortMethod),
    ReplaceRoot(ViewId),
    ToggleCategory(String),
    ToggleNegateCategory(String),
    ToggleNegateCategoryChildren(String),
    Focus(Option<ViewId>),
    Submit(ViewId, String),
    Popup(ViewId),
    Close(ViewId),
    Key(KeyKind),
    ToggleFind,
    ToggleKeyboard,
    ClockTick,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum ViewId {
    Home,
    Reader,
    Sort,
    Menu,
    Frontlight,
    Search,
    SearchBar,
}

#[derive(Debug, Clone)]
pub enum Align {
    Left(i32),
    Right(i32),
    Center,
}

impl Align {
    #[inline]
    pub fn offset(&self, width: i32, container_width: i32) -> i32 {
        match *self {
            Align::Left(dx) => dx,
            Align::Right(dx) => container_width - width - dx,
            Align::Center => (container_width - width) / 2,
        }
    }
}

#[derive(Debug, Copy, Clone)]
pub enum KeyboardEvent {
    Append(char),
    Partial(char),
    Move { target: TextKind, dir: LinearDir },
    Delete { target: TextKind, dir: LinearDir },
    Submit,
}

#[derive(Debug, Copy, Clone)]
pub enum TextKind {
    Char,
    Word,
    Extremum,
}

pub trait View: Downcast {
    fn handle_event(&mut self, evt: &Event, bus: &mut Vec<Event>) -> bool;
    fn render(&self, fb: &mut Framebuffer, fonts: &mut Fonts);
    fn rect(&self) -> &Rectangle;
    fn len(&self) -> usize;
    fn child(&self, index: usize) -> &View;
    fn child_mut(&mut self, index: usize) -> &mut View;
    fn might_skip(&self, evt: &Event) -> bool {
        false
    }
}

impl_downcast!(View);

impl Debug for Box<View> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Box<View>")
    }
}

// From bottom to top
pub fn render(view: &View, rect: &mut Rectangle, fb: &mut Framebuffer, fonts: &mut Fonts) {
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
pub fn handle_event(view: &mut View, evt: &Event, parent_bus: &mut Vec<Event>) -> bool {
    if view.might_skip(evt) {
        return false;
    }

    let mut child_bus: Vec<Event> = Vec::with_capacity(1);

    for i in (0..view.len()).rev() {
        if handle_event(view.child_mut(i), evt, &mut child_bus) {
            break;
        }
    }

    while let Some(child_evt) = child_bus.pop() {
        if !view.handle_event(&child_evt.clone(), parent_bus) {
            parent_bus.push(child_evt);
        }
    }

    view.handle_event(evt, parent_bus)
}
