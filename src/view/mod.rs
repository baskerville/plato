pub mod icon;
pub mod home;
pub mod clock;
pub mod keyboard;

use std::sync::mpsc::Sender;
use framebuffer::{Framebuffer, UpdateMode};
use std::fmt::{self, Debug};
use font::Fonts;
use gesture::GestureEvent;
use geom::{LinearDir, CycleDir, Rectangle};

#[derive(Debug, Copy, Clone)]
pub enum Event {
    GestureEvent(GestureEvent),
    ChildEvent(ChildEvent),
}

#[derive(Debug, Copy, Clone)]
pub enum RootKind {
    Home,
    Find,
    Reader,
}

#[derive(Debug, Copy, Clone)]
pub enum PopupSource {
    Sort,
    Menu,
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

pub trait View: Send + Sync {
    fn rect(&self) -> &Rectangle;
    fn len(&self) -> usize;
    fn child(&self, index: usize) -> &View;
    fn child_mut(&mut self, index: usize) -> &mut View;
    fn handle_event(&mut self, evt: &Event, bus: &Sender<ChildEvent>) -> bool;
    fn render(&self, fb: &mut Framebuffer, fonts: &mut Fonts);
}

impl Debug for Box<View> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Box<View>")
    }
}
