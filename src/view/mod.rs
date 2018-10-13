//! Views are organized as a tree. A view might receive / send events and render itself.
//!
//! The z-level of the n-th child of a view is less or equal to the z-level of its n+1-th child.
//!
//! Events travel from the root to the leaves, only the leaf views will handle the root events, but
//! any view can send events to its parent. From the events it receives from its children, a view
//! resends the ones it doesn't handle to its own parent. Hence an event sent from a child might
//! bubble up to the root. If it reaches the root without being captured by any view, then it will
//! be written to the main event channel and will be sent to every leaf in one of the next loop
//! iterations.

pub mod common;
pub mod filler;
pub mod icon;
pub mod label;
pub mod button;
pub mod rounded_button;
pub mod slider;
pub mod input_field;
pub mod page_label;
pub mod named_input;
pub mod labeled_icon;
pub mod search_bar;
pub mod confirmation;
pub mod notification;
pub mod intermission;
pub mod frontlight;
pub mod presets_list;
pub mod preset;
pub mod menu;
pub mod menu_entry;
pub mod clock;
pub mod battery;
pub mod keyboard;
pub mod key;
pub mod home;
pub mod reader;

use std::time::Duration;
use std::path::PathBuf;
use std::sync::mpsc::Sender;
use std::collections::VecDeque;
use std::fmt::{self, Debug};
use fnv::FnvHashMap;
use downcast_rs::Downcast;
use font::Fonts;
use document::TocEntry;
use settings::SecondColumn;
use metadata::{Info, SortMethod, PageScheme, Margin};
use framebuffer::{Framebuffer, UpdateMode};
use input::{DeviceEvent, FingerStatus};
use gesture::GestureEvent;
use view::key::KeyKind;
use app::Context;
use geom::{LinearDir, CycleDir, Rectangle};

pub const THICKNESS_SMALL: f32 = 1.0;
pub const THICKNESS_MEDIUM: f32 = 2.0;
pub const THICKNESS_LARGE: f32 = 3.0;

pub const BORDER_RADIUS_SMALL: f32 = 6.0;
pub const BORDER_RADIUS_MEDIUM: f32 = 9.0;
pub const BORDER_RADIUS_LARGE: f32 = 12.0;

pub const CLOSE_IGNITION_DELAY: Duration = Duration::from_millis(150);

type Bus = VecDeque<Event>;
type Hub = Sender<Event>;

pub trait View: Downcast {
    fn handle_event(&mut self, evt: &Event, hub: &Hub, bus: &mut Bus, context: &mut Context) -> bool;
    fn render(&self, fb: &mut Framebuffer, fonts: &mut Fonts);
    fn rect(&self) -> &Rectangle;
    fn rect_mut(&mut self) -> &mut Rectangle;
    fn children(&self) -> &Vec<Box<View>>;
    fn children_mut(&mut self) -> &mut Vec<Box<View>>;

    fn child(&self, index: usize) -> &View {
        self.children()[index].as_ref()
    }

    fn child_mut(&mut self, index: usize) -> &mut View {
        self.children_mut()[index].as_mut()
    }

    fn len(&self) -> usize {
        self.children().len()
    }

    fn might_skip(&self, _evt: &Event) -> bool {
        false
    }

    fn is_background(&self) -> bool {
        false
    }

    fn id(&self) -> Option<ViewId> {
        None
    }
}

impl_downcast!(View);

impl Debug for Box<View> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Box<View>")
    }
}

// We start delivering events from the highest z-level to prevent views from capturing
// gestures that occurred in higher views.
// The consistency must also be ensured by the views: popups, for example, need to
// capture any tap gesture with a touch point inside their rectangle.
// A child can send events to the main channel through the *hub* or communicate with its parent through the *bus*.
pub fn handle_event(view: &mut View, evt: &Event, hub: &Hub, parent_bus: &mut Bus, context: &mut Context) -> bool {
    if view.len() > 0 {
        let mut captured = false;

        if view.might_skip(evt) {
            return captured;
        }

        let mut child_bus: Bus = VecDeque::with_capacity(1);

        for i in (0..view.len()).rev() {
            if handle_event(view.child_mut(i), evt, hub, &mut child_bus, context) {
                captured = true;
                break;
            }
        }

        child_bus.retain(|child_evt| !view.handle_event(child_evt, hub, parent_bus, context));
        parent_bus.append(&mut child_bus);

        captured || view.handle_event(evt, hub, parent_bus, context)
    } else {
        view.handle_event(evt, hub, parent_bus, context)
    }
}

pub fn render(view: &View, rect: &mut Rectangle, fb: &mut Framebuffer, fonts: &mut Fonts, updating: &mut FnvHashMap<u32, Rectangle>) {
    render_aux(view, rect, fb, fonts, &mut false, true, updating);
}

pub fn render_no_wait(view: &View, rect: &mut Rectangle, fb: &mut Framebuffer, fonts: &mut Fonts, updating: &mut FnvHashMap<u32, Rectangle>) {
    render_aux(view, rect, fb, fonts, &mut false, false, updating);
}

// We don't start rendering until we reach the z-level of the view that generated the event.
// Once we reach that z-level, we start comparing the candidate rectangles with the source
// rectangle. If there is an overlap, we render the corresponding view. And update the source
// rectangle by absorbing the candidate rectangle into it.
fn render_aux(view: &View, rect: &mut Rectangle, fb: &mut Framebuffer, fonts: &mut Fonts, above: &mut bool, wait: bool, updating: &mut FnvHashMap<u32, Rectangle>) {
    if !*above && view.rect() == rect {
        *above = true;
    }

    if *above && view.rect().overlaps(rect) {
        if wait {
            updating.retain(|tok, urect| {
                !view.rect().overlaps(urect) || fb.wait(*tok).is_err()
            });
        }
        view.render(fb, fonts);
        rect.absorb(view.rect());
    }

    for i in 0..view.len() {
        render_aux(view.child(i), rect, fb, fonts, above, wait, updating);
    }
}

// When a floating window is destroyed, it leaves a crack underneath.
// Each view intersecting the crack's rectangle needs to be redrawn.
pub fn fill_crack(view: &View, rect: &mut Rectangle, fb: &mut Framebuffer, fonts: &mut Fonts, updating: &mut FnvHashMap<u32, Rectangle>) {
    if (view.len() == 0 || view.is_background()) && view.rect().overlaps(rect) {
        updating.retain(|tok, urect| {
            !view.rect().overlaps(urect) || fb.wait(*tok).is_err()
        });
        view.render(fb, fonts);
        rect.absorb(view.rect());
    }

    for i in 0..view.len() {
        fill_crack(view.child(i), rect, fb, fonts, updating);
    }
}

#[derive(Debug, Clone)]
pub enum Event {
    Render(Rectangle, UpdateMode),
    RenderNoWait(Rectangle, UpdateMode),
    Expose(Rectangle, UpdateMode),
    Device(DeviceEvent),
    Gesture(GestureEvent),
    Keyboard(KeyboardEvent),
    Key(KeyKind),
    Open(Box<Info>),
    OpenToc(Vec<TocEntry>, f64),
    Invalid(Box<Info>),
    Remove(Box<Info>),
    Page(CycleDir),
    ResultsPage(CycleDir),
    GoTo(f64),
    ResultsGoTo(usize),
    CropMargins(Box<Margin>),
    Chapter(CycleDir),
    Sort(SortMethod),
    ToggleSelectCategory(String),
    ToggleNegateCategory(String),
    ToggleNegateCategoryChildren(String),
    ResizeSummary(i32),
    Focus(Option<ViewId>),
    Select(EntryId),
    PropagateSelect(EntryId),
    Submit(ViewId, String),
    Slider(SliderId, f32, FingerStatus),
    ToggleNear(ViewId, Rectangle),
    ToggleBookMenu(Rectangle, usize),
    TogglePresetMenu(Rectangle, usize),
    SubMenu(Rectangle, Vec<EntryKind>),
    Toggle(ViewId),
    Show(ViewId),
    Close(ViewId),
    CloseSub(ViewId),
    SearchResult(f64, Rectangle),
    EndOfSearch,
    Finished,
    ClockTick,
    BatteryTick,
    ToggleFrontlight,
    Load(PathBuf),
    LoadPreset(usize),
    Save,
    Guess,
    CheckBattery,
    PrepareSuspend,
    Suspend,
    Share,
    PrepareShare,
    Validate,
    Cancel,
    Reseed,
    Back,
    Quit,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum ViewId {
    Home,
    Reader,
    SortMenu,
    MainMenu,
    Frontlight,
    FontSizeMenu,
    FontFamilyMenu,
    MarginWidthMenu,
    LineHeightMenu,
    MatchesMenu,
    PageMenu,
    BookMenu,
    PresetMenu,
    MarginCropperMenu,
    SearchMenu,
    GoToPage,
    GoToPageInput,
    GoToResultsPage,
    GoToResultsPageInput,
    ExportAs,
    ExportAsInput,
    AddCategories,
    AddCategoriesInput,
    SearchInput,
    SearchBar,
    Keyboard,
    ConfirmShare,
    MarginCropper,
    TopBottomBars,
    TableOfContents,
    BoundaryNotif,
    TakeScreenshotNotif,
    NoSearchResultsNotif,
    InvalidSearchQueryNotif,
    LowBatteryNotif,
    NetUpNotif,
    SubMenu(u8),
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum SliderId {
    FontSize,
    LightIntensity,
    LightWarmth,
}

impl SliderId {
    pub fn label(self) -> String {
        match self {
            SliderId::LightIntensity => "Intensity".to_string(),
            SliderId::LightWarmth => "Warmth".to_string(),
            SliderId::FontSize => "Font Size".to_string(),
        }
    }
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

#[derive(Debug, Clone)]
pub enum EntryKind {
    Command(String, EntryId),
    CheckBox(String, EntryId, bool),
    RadioButton(String, EntryId, bool),
    SubMenu(String, Vec<EntryKind>),
    Separator,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum EntryId {
    Sort(SortMethod),
    ApplyCroppings(usize, PageScheme),
    SetFontFamily(String),
    SetFontSize(i32),
    SetMarginWidth(i32),
    SetLineHeight(i32),
    RemoveCroppings,
    Remove(PathBuf),
    SearchDirection(LinearDir),
    AddBookCategories(PathBuf),
    RemoveBookCategory(PathBuf, String),
    RemoveMatches,
    RemovePreset(usize),
    AddMatchesCategories,
    RemoveMatchesCategory(String),
    SecondColumn(SecondColumn),
    Load(PathBuf),
    ExportMatches,
    ToggleFirstPage,
    ReverseOrder,
    ToggleInverted,
    ToggleMonochrome,
    ToggleWifi,
    TakeScreenshot,
    StartNickel,
    Reboot,
    Quit,
    Undo,
}

impl EntryKind {
    pub fn is_separator(&self) -> bool {
        match *self {
            EntryKind::Separator => true,
            _ => false,
        }
    }

    pub fn text(&self) -> &str {
        match *self {
            EntryKind::Command(ref s, ..) |
            EntryKind::CheckBox(ref s, ..) |
            EntryKind::RadioButton(ref s, ..) |
            EntryKind::SubMenu(ref s, ..) => s,
            _ => "",
        }
    }

    pub fn get(&self) -> Option<bool> {
        match *self {
            EntryKind::CheckBox(_, _, v) |
            EntryKind::RadioButton(_, _, v) => Some(v),
            _ => None,
        }
    }

    pub fn set(&mut self, value: bool) {
        match *self {
            EntryKind::CheckBox(_, _, ref mut v) |
            EntryKind::RadioButton(_, _, ref mut v) => *v = value,
            _ => (),
        }
    }
}
