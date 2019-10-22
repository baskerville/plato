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
pub mod image;
pub mod icon;
pub mod label;
pub mod button;
pub mod rounded_button;
pub mod slider;
pub mod input_field;
pub mod page_label;
pub mod named_input;
pub mod labeled_icon;
pub mod top_bar;
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
pub mod dictionary;
pub mod calculator;
pub mod sketch;

use std::time::Duration;
use std::path::PathBuf;
use std::sync::mpsc::Sender;
use std::collections::VecDeque;
use std::fmt::{self, Debug};
use fnv::FnvHashMap;
use downcast_rs::{Downcast, impl_downcast};
use crate::font::Fonts;
use crate::document::{Location, TextLocation, TocEntry};
use crate::settings::{SecondColumn, RotationLock};
use crate::metadata::{Info, ZoomMode, SortMethod, TextAlign, SimpleStatus, PageScheme, Margin};
use crate::geom::{LinearDir, CycleDir, Rectangle, Boundary};
use crate::framebuffer::{Framebuffer, UpdateMode};
use crate::input::{DeviceEvent, FingerStatus};
use crate::gesture::GestureEvent;
use self::calculator::LineOrigin;
use self::key::KeyKind;
use self::intermission::IntermKind;
use crate::app::Context;

pub const THICKNESS_SMALL: f32 = 1.0;
pub const THICKNESS_MEDIUM: f32 = 2.0;
pub const THICKNESS_LARGE: f32 = 3.0;

pub const BORDER_RADIUS_SMALL: f32 = 6.0;
pub const BORDER_RADIUS_MEDIUM: f32 = 9.0;
pub const BORDER_RADIUS_LARGE: f32 = 12.0;

pub const CLOSE_IGNITION_DELAY: Duration = Duration::from_millis(150);

pub type Bus = VecDeque<Event>;
pub type Hub = Sender<Event>;

pub trait View: Downcast {
    fn handle_event(&mut self, evt: &Event, hub: &Hub, bus: &mut Bus, context: &mut Context) -> bool;
    fn render(&self, fb: &mut dyn Framebuffer, rect: Rectangle, fonts: &mut Fonts);
    fn rect(&self) -> &Rectangle;
    fn rect_mut(&mut self) -> &mut Rectangle;
    fn children(&self) -> &Vec<Box<dyn View>>;
    fn children_mut(&mut self) -> &mut Vec<Box<dyn View>>;

    fn render_rect(&self, _rect: &Rectangle) -> Rectangle {
        *self.rect()
    }

    fn resize(&mut self, rect: Rectangle, _hub: &Hub, _context: &mut Context) {
        *self.rect_mut() = rect;
    }

    fn child(&self, index: usize) -> &dyn View {
        self.children()[index].as_ref()
    }

    fn child_mut(&mut self, index: usize) -> &mut dyn View {
        self.children_mut()[index].as_mut()
    }

    fn len(&self) -> usize {
        self.children().len()
    }

    fn might_skip(&self, _evt: &Event) -> bool {
        false
    }

    fn might_rotate(&self) -> bool {
        true
    }

    fn is_background(&self) -> bool {
        false
    }

    fn id(&self) -> Option<ViewId> {
        None
    }
}

impl_downcast!(View);

impl Debug for Box<dyn View> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Box<dyn View>")
    }
}

// We start delivering events from the highest z-level to prevent views from capturing
// gestures that occurred in higher views.
// The consistency must also be ensured by the views: popups, for example, need to
// capture any tap gesture with a touch point inside their rectangle.
// A child can send events to the main channel through the *hub* or communicate with its parent through the *bus*.
pub fn handle_event(view: &mut dyn View, evt: &Event, hub: &Hub, parent_bus: &mut Bus, context: &mut Context) -> bool {
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

        let mut temp_bus: Bus = VecDeque::with_capacity(1);

        child_bus.retain(|child_evt| !view.handle_event(child_evt, hub, &mut temp_bus, context));

        parent_bus.append(&mut child_bus);
        parent_bus.append(&mut temp_bus);

        captured || view.handle_event(evt, hub, parent_bus, context)
    } else {
        view.handle_event(evt, hub, parent_bus, context)
    }
}

pub fn render(view: &dyn View, rect: &mut Rectangle, fb: &mut dyn Framebuffer, fonts: &mut Fonts, updating: &mut FnvHashMap<u32, Rectangle>) {
    render_aux(view, rect, fb, fonts, &mut false, true, updating);
}

pub fn render_region(view: &dyn View, rect: &mut Rectangle, fb: &mut dyn Framebuffer, fonts: &mut Fonts, updating: &mut FnvHashMap<u32, Rectangle>) {
    render_aux(view, rect, fb, fonts, &mut true, true, updating);
}

pub fn render_no_wait(view: &dyn View, rect: &mut Rectangle, fb: &mut dyn Framebuffer, fonts: &mut Fonts, updating: &mut FnvHashMap<u32, Rectangle>) {
    render_aux(view, rect, fb, fonts, &mut false, false, updating);
}

pub fn render_no_wait_region(view: &dyn View, rect: &mut Rectangle, fb: &mut dyn Framebuffer, fonts: &mut Fonts, updating: &mut FnvHashMap<u32, Rectangle>) {
    render_aux(view, rect, fb, fonts, &mut true, false, updating);
}

// We don't start rendering until we reach the z-level of the view that generated the event.
// Once we reach that z-level, we start comparing the candidate rectangles with the source
// rectangle. If there is an overlap, we render the corresponding view. And update the source
// rectangle by absorbing the candidate rectangle into it.
fn render_aux(view: &dyn View, rect: &mut Rectangle, fb: &mut dyn Framebuffer, fonts: &mut Fonts, above: &mut bool, wait: bool, updating: &mut FnvHashMap<u32, Rectangle>) {
    // FIXME: rect is used as an identifier.
    if !*above && view.rect() == rect {
        *above = true;
    }

    if *above && (view.len() == 0 || view.is_background()) && view.rect().overlaps(rect) {
        let render_rect = view.render_rect(rect);
        if wait {
            updating.retain(|tok, urect| {
                !render_rect.overlaps(urect) || fb.wait(*tok).is_err()
            });
        }
        view.render(fb, *rect, fonts);
        rect.absorb(&render_rect);
    }

    for i in 0..view.len() {
        render_aux(view.child(i), rect, fb, fonts, above, wait, updating);
    }
}

// When a floating window is destroyed, it leaves a crack underneath.
// Each view intersecting the crack's rectangle needs to be redrawn.
pub fn expose(view: &dyn View, rect: &mut Rectangle, fb: &mut dyn Framebuffer, fonts: &mut Fonts, updating: &mut FnvHashMap<u32, Rectangle>) {
    if (view.len() == 0 || view.is_background()) && view.rect().overlaps(rect) {
        let render_rect = view.render_rect(rect);
        updating.retain(|tok, urect| {
            !render_rect.overlaps(urect) || fb.wait(*tok).is_err()
        });
        view.render(fb, *rect, fonts);
        rect.absorb(&render_rect);
    }

    for i in 0..view.len() {
        expose(view.child(i), rect, fb, fonts, updating);
    }
}

#[derive(Debug, Clone)]
pub enum Event {
    Render(Rectangle, UpdateMode),
    RenderRegion(Rectangle, UpdateMode),
    RenderNoWait(Rectangle, UpdateMode),
    RenderNoWaitRegion(Rectangle, UpdateMode),
    Expose(Rectangle, UpdateMode),
    Device(DeviceEvent),
    Gesture(GestureEvent),
    Keyboard(KeyboardEvent),
    Key(KeyKind),
    AddDocument(Box<Info>),
    RemoveDocument(PathBuf),
    Open(Box<Info>),
    OpenToc(Vec<TocEntry>, usize),
    LoadPixmap(usize),
    Update(UpdateMode),
    Invalid(Box<Info>),
    Remove(Box<Info>),
    Notify(String),
    Page(CycleDir),
    ResultsPage(CycleDir),
    GoTo(usize),
    GoToLocation(Location),
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
    EditLanguages,
    Define(String),
    Submit(ViewId, String),
    Slider(SliderId, f32, FingerStatus),
    ToggleNear(ViewId, Rectangle),
    ToggleBookMenu(Rectangle, usize),
    ToggleCategoryMenu(Rectangle, String),
    TogglePresetMenu(Rectangle, usize),
    SubMenu(Rectangle, Vec<EntryKind>),
    ProcessLine(LineOrigin, String),
    History(CycleDir, bool),
    Toggle(ViewId),
    Show(ViewId),
    Close(ViewId),
    CloseSub(ViewId),
    Search(String),
    SearchResult(usize, Vec<Boundary>),
    EndOfSearch,
    Finished,
    ClockTick,
    BatteryTick,
    ToggleFrontlight,
    Load(PathBuf),
    LoadPreset(usize),
    Scroll(i32),
    Save,
    Guess,
    CheckBattery,
    SetWifi(bool),
    MightSuspend,
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

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum AppCmd {
    Sketch,
    Calculator,
    Dictionary {
        query: String,
        language: String,
    },
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum ViewId {
    Home,
    Reader,
    SortMenu,
    MainMenu,
    TitleMenu,
    SelectionMenu,
    AnnotationMenu,
    BatteryMenu,
    ClockMenu,
    SearchTargetMenu,
    Frontlight,
    Dictionary,
    FontSizeMenu,
    TextAlignMenu,
    FontFamilyMenu,
    MarginWidthMenu,
    ContrastExponentMenu,
    ContrastGrayMenu,
    LineHeightMenu,
    CategoryMenu,
    BookMenu,
    MatchesMenu,
    PageMenu,
    PresetMenu,
    MarginCropperMenu,
    SearchMenu,
    SketchMenu,
    GoToPage,
    GoToPageInput,
    GoToResultsPage,
    GoToResultsPageInput,
    NamePage,
    NamePageInput,
    EditNote,
    EditNoteInput,
    EditLanguages,
    EditLanguagesInput,
    SaveAs,
    SaveAsInput,
    AddCategories,
    AddCategoriesInput,
    RenameCategory,
    RenameCategoryInput,
    SearchInput,
    CalculatorInput,
    SearchBar,
    Keyboard,
    ConfirmShare,
    MarginCropper,
    TopBottomBars,
    TableOfContents,
    MessageNotif,
    BoundaryNotif,
    TakeScreenshotNotif,
    SaveSketchNotif,
    LoadSketchNotif,
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
    ContrastExponent,
    ContrastGray,
}

impl SliderId {
    pub fn label(self) -> String {
        match self {
            SliderId::LightIntensity => "Intensity".to_string(),
            SliderId::LightWarmth => "Warmth".to_string(),
            SliderId::FontSize => "Font Size".to_string(),
            SliderId::ContrastExponent => "Contrast Exponent".to_string(),
            SliderId::ContrastGray => "Contrast Gray".to_string(),
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
    Message(String),
    Command(String, EntryId),
    CheckBox(String, EntryId, bool),
    RadioButton(String, EntryId, bool),
    SubMenu(String, Vec<EntryKind>),
    Separator,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum EntryId {
    Save,
    SaveAs,
    Import,
    Load(PathBuf),
    Reload,
    CleanUp,
    Sort(SortMethod),
    StatusFilter(Option<SimpleStatus>),
    ReverseOrder,
    EmptyTrash,
    Remove(PathBuf),
    RenameCategory(String),
    RemoveCategory(String),
    AddMatchesCategories,
    ToggleSelectCategory(String),
    AddBookCategories(PathBuf),
    RemoveBookCategory(PathBuf, String),
    SetStatus(PathBuf, SimpleStatus),
    ToggleIntermissionImage(IntermKind, PathBuf),
    RemoveMatches,
    RemovePreset(usize),
    SecondColumn(SecondColumn),
    ApplyCroppings(usize, PageScheme),
    RemoveCroppings,
    SetZoomMode(ZoomMode),
    SetPageName,
    RemovePageName,
    HighlightSelection,
    AnnotateSelection,
    DefineSelection,
    SearchForSelection,
    AdjustSelection,
    RemoveAnnotation([TextLocation; 2]),
    EditAnnotationNote([TextLocation; 2]),
    RemoveAnnotationNote([TextLocation; 2]),
    GoTo(usize),
    GoToSelectedPageName,
    SearchDirection(LinearDir),
    SetFontFamily(String),
    SetFontSize(i32),
    SetTextAlign(TextAlign),
    SetMarginWidth(i32),
    SetLineHeight(i32),
    SetContrastExponent(i32),
    SetContrastGray(i32),
    SetRotationLock(Option<RotationLock>),
    SetSearchTarget(Option<String>),
    ToggleFuzzy,
    ToggleInverted,
    ToggleMonochrome,
    ToggleWifi,
    Rotate(i8),
    Launch(AppCmd),
    SetPenSize(i32),
    SetPenColor(u8),
    TogglePenDynamism,
    ReloadDictionaries,
    New,
    Refresh,
    OpenMetadata,
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
            EntryKind::Message(ref s) |
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
