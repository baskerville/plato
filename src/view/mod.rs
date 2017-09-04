pub mod filler;
pub mod icon;
pub mod menu;
pub mod clock;
pub mod keyboard;
pub mod key;
pub mod home;
pub mod reader;

use std::fmt::{self, Debug};
use downcast_rs::Downcast;
use font::Fonts;
use framebuffer::{Framebuffer, UpdateMode, Bitmap};
use gesture::GestureEvent;
use view::key::KeyKind;
use geom::{LinearDir, CycleDir, Rectangle};

const THICKNESS_BIG: f32 = 3.0;
const THICKNESS_MEDIUM: f32 = 2.0;
const THICKNESS_SMALL: f32 = 1.0;

#[derive(Debug, Copy, Clone)]
pub enum Event {
    GestureEvent(GestureEvent),
    ChildEvent(ChildEvent),
}

#[derive(Debug, Copy, Clone)]
pub enum RootKind {
    Home,
    Reader,
}

#[derive(Debug, Copy, Clone)]
pub enum PopupSource {
    Sort,
    Menu,
    Frontlight,
}

#[derive(Debug, Copy, Clone)]
pub enum SortMethod {
    Opened,
    Size,
    Type,
    Author,
    Year,
    Title,
    Pages,
}

#[derive(Debug, Copy, Clone)]
pub enum ChildEvent {
    Render(Rectangle, UpdateMode),
    Page(CycleDir),
    GoTo(usize),
    Chapter(CycleDir),
    Open(usize),
    Remove(usize),
    Sort(SortMethod),
    ReplaceRoot(RootKind),
    Popup(PopupSource),
    Keyboard(KeyboardEvent),
    Key(KeyKind),
    ToggleFind,
    ClockTick,
}

#[derive(Debug, Copy, Clone)]
pub enum KeyboardEvent {
    Append(char),
    Partial(char),
    Move { kind: InputKind, dir: LinearDir },
    Delete { kind: InputKind, dir: LinearDir },
    Submit,
}

#[derive(Debug, Copy, Clone)]
pub enum InputKind {
    Char,
    Word,
}

pub trait View: Downcast {
    fn handle_event(&mut self, evt: &Event, bus: &mut Vec<ChildEvent>) -> bool;
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
