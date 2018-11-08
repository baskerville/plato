use device::CURRENT_DEVICE;
use framebuffer::Framebuffer;
use settings::ReaderSettings;
use view::{View, Event, Hub, Bus, SliderId, ViewId, THICKNESS_MEDIUM};
use view::common::locate;
use view::filler::Filler;
use view::slider::Slider;
use view::icon::Icon;
use view::labeled_icon::LabeledIcon;
use metadata::ReaderInfo;
use gesture::GestureEvent;
use input::DeviceEvent;
use unit::scale_by_dpi;
use geom::Rectangle;
use font::Fonts;
use color::{WHITE, SEPARATOR_NORMAL};
use app::Context;

#[derive(Debug)]
pub struct ToolBar {
    rect: Rectangle,
    children: Vec<Box<View>>,
    is_reflowable: bool,
}

impl ToolBar {
    pub fn new(rect: Rectangle, is_reflowable: bool, reader_info: Option<&ReaderInfo>, reader_settings: &ReaderSettings) -> ToolBar {
        let mut children = Vec::new();
        let dpi = CURRENT_DEVICE.dpi;
        let thickness = scale_by_dpi(THICKNESS_MEDIUM, dpi) as i32;
        let side = if is_reflowable {
            (rect.height() as i32 + thickness) / 2 - thickness
        } else {
            rect.height() as i32
        };

        if is_reflowable {
            let mut remaining_width = rect.width() as i32 - 3 * side;
            let font_family_label_width = remaining_width / 2;
            remaining_width -= font_family_label_width;
            let margin_label_width = remaining_width / 2;
            let line_height_label_width = remaining_width - margin_label_width;

            // First row.

            let mut x_offset = rect.min.x;
            let margin_width = reader_info.and_then(|r| r.margin_width)
                                          .unwrap_or(reader_settings.margin_width);
            let margin_icon = LabeledIcon::new("margin",
                                               rect![x_offset, rect.min.y,
                                                     x_offset + side + margin_label_width, rect.min.y + side],
                                               Event::Show(ViewId::MarginWidthMenu),
                                               format!("{} mm", margin_width));
            children.push(Box::new(margin_icon) as Box<View>);
            x_offset += side + margin_label_width;

            let font_family = reader_info.and_then(|r| r.font_family.clone())
                                         .unwrap_or_else(|| reader_settings.font_family.clone());
            let font_family_icon = LabeledIcon::new("font_family",
                                                    rect![x_offset, rect.min.y,
                                                          x_offset + side + font_family_label_width, rect.min.y + side],
                                                    Event::Show(ViewId::FontFamilyMenu),
                                                    font_family);
            children.push(Box::new(font_family_icon) as Box<View>);
            x_offset += side + font_family_label_width;

            let line_height = reader_info.and_then(|r| r.line_height)
                                         .unwrap_or(reader_settings.line_height);
            let line_height_icon = LabeledIcon::new("line_height",
                                                    rect![x_offset, rect.min.y,
                                                          x_offset + side + line_height_label_width, rect.min.y + side],
                                                    Event::Show(ViewId::LineHeightMenu),
                                                    format!("{:.1} em", line_height));
            children.push(Box::new(line_height_icon) as Box<View>);

            // Separator.
            let separator = Filler::new(rect![rect.min.x, rect.min.y + side,
                                              rect.max.x, rect.max.y - side],
                                        SEPARATOR_NORMAL);
            children.push(Box::new(separator) as Box<View>);

            // Start of second row.
            let font_size = reader_info.and_then(|r| r.font_size)
                                       .unwrap_or(reader_settings.font_size);
            let font_size_rect = rect![rect.min.x, rect.max.y - side,
                                       rect.min.x + side, rect.max.y];
            let font_size_icon = Icon::new("font_size",
                                           font_size_rect,
                                           Event::ToggleNear(ViewId::FontSizeMenu, font_size_rect));
            children.push(Box::new(font_size_icon) as Box<View>);

            let slider = Slider::new(rect![rect.min.x + side, rect.max.y - side,
                                           rect.max.x - 2 * side, rect.max.y],
                                     SliderId::FontSize,
                                     font_size,
                                     reader_settings.font_size / 2.0,
                                     3.0 * reader_settings.font_size / 2.0);
            children.push(Box::new(slider) as Box<View>);
        } else {
            // Alternate start of second row.
            let crop_icon = Icon::new("crop",
                                      rect![rect.min.x, rect.max.y - side,
                                            rect.min.x + side, rect.max.y],
                                      Event::Show(ViewId::MarginCropper));
            children.push(Box::new(crop_icon) as Box<View>);

            let filler = Filler::new(rect![rect.min.x + side, rect.max.y - side,
                                           rect.max.x - 2 * side, rect.max.y],
                                     WHITE);
            children.push(Box::new(filler) as Box<View>);
        }

        // End of second row.

        let search_icon = Icon::new("search",
                                    rect![rect.max.x - 2 * side, rect.max.y - side,
                                          rect.max.x - side, rect.max.y],
                                    Event::Show(ViewId::SearchBar));
        children.push(Box::new(search_icon) as Box<View>);

        let toc_icon = Icon::new("toc",
                                 rect![rect.max.x - side, rect.max.y - side,
                                       rect.max.x, rect.max.y],
                                 Event::Show(ViewId::TableOfContents));
        children.push(Box::new(toc_icon) as Box<View>);

        ToolBar {
            rect,
            children,
            is_reflowable,
        }
    }

    pub fn update_margin_width(&mut self, margin_width: i32, hub: &Hub) {
        if let Some(labeled_icon) = self.children[0].downcast_mut::<LabeledIcon>() {
            labeled_icon.update(format!("{} mm", margin_width), hub);
        }
    }

    pub fn update_font_family(&mut self, font_family: String, hub: &Hub) {
        if let Some(labeled_icon) = self.children[1].downcast_mut::<LabeledIcon>() {
            labeled_icon.update(font_family, hub);
        }
    }

    pub fn update_line_height(&mut self, line_height: f32, hub: &Hub) {
        if let Some(labeled_icon) = self.children[2].downcast_mut::<LabeledIcon>() {
            labeled_icon.update(format!("{:.1} em", line_height), hub);
        }
    }


    pub fn update_slider(&mut self, font_size: f32, hub: &Hub) {
        if let Some(index) = locate::<Slider>(self) {
            let slider = self.children[index].as_mut().downcast_mut::<Slider>().unwrap();
            slider.update(font_size, hub);
        }
    }
}

impl View for ToolBar {
    fn handle_event(&mut self, evt: &Event, _hub: &Hub, _bus: &mut Bus, _context: &mut Context) -> bool {
        match *evt {
            Event::Gesture(GestureEvent::Tap(center)) |
            Event::Gesture(GestureEvent::HoldFinger(center)) if self.rect.includes(center) => true,
            Event::Gesture(GestureEvent::Swipe { start, .. }) if self.rect.includes(start) => true,
            Event::Device(DeviceEvent::Finger { position, .. }) if self.rect.includes(position) => true,
            _ => false,
        }
    }

    fn render(&self, _fb: &mut Framebuffer, _fonts: &mut Fonts) {
    }

    fn resize(&mut self, rect: Rectangle, context: &mut Context) {
        let dpi = CURRENT_DEVICE.dpi;
        let thickness = scale_by_dpi(THICKNESS_MEDIUM, dpi) as i32;

        let side = if self.is_reflowable {
            (rect.height() as i32 + thickness) / 2 - thickness
        } else {
            rect.height() as i32
        };

        let mut index = 0;

        if self.is_reflowable {
            let mut remaining_width = rect.width() as i32 - 3 * side;
            let font_family_label_width = remaining_width / 2;
            remaining_width -= font_family_label_width;
            let margin_label_width = remaining_width / 2;
            let line_height_label_width = remaining_width - margin_label_width;

            // First row.

            let mut x_offset = rect.min.x;
            self.children[index].resize(rect![x_offset, rect.min.y,
                                              x_offset + side + margin_label_width, rect.min.y + side],
                                        context);
            index += 1;
            x_offset += side + margin_label_width;

            self.children[index].resize(rect![x_offset, rect.min.y,
                                              x_offset + side + font_family_label_width, rect.min.y + side],
                                        context);
            index += 1;
            x_offset += side + font_family_label_width;

            self.children[index].resize(rect![x_offset, rect.min.y,
                                              x_offset + side + line_height_label_width, rect.min.y + side],
                                        context);
            index += 1;

            // Separator.
            self.children[index].resize(rect![rect.min.x, rect.min.y + side,
                                              rect.max.x, rect.max.y - side],
                                        context);
            index += 1;

            // Start of second row.
            let font_size_rect = rect![rect.min.x, rect.max.y - side,
                                       rect.min.x + side, rect.max.y];
            self.children[index].resize(font_size_rect, context);
            index += 1;

            self.children[index].resize(rect![rect.min.x + side, rect.max.y - side,
                                              rect.max.x - 2 * side, rect.max.y],
                                        context);
            index += 1;
        } else {
            // Alternate start of second row.
            self.children[index].resize(rect![rect.min.x, rect.max.y - side,
                                              rect.min.x + side, rect.max.y],
                                        context);
            index += 1;

            self.children[index].resize(rect![rect.min.x + side, rect.max.y - side,
                                              rect.max.x - 2 * side, rect.max.y],
                                        context);
            index += 1;
        }

        // End of second row.

        self.children[index].resize(rect![rect.max.x - 2 * side, rect.max.y - side,
                                          rect.max.x - side, rect.max.y],
                                    context);
        index += 1;

        self.children[index].resize(rect![rect.max.x - side, rect.max.y - side,
                                         rect.max.x, rect.max.y],
                                    context);
        self.rect = rect;
    }

    fn rect(&self) -> &Rectangle {
        &self.rect
    }

    fn rect_mut(&mut self) -> &mut Rectangle {
        &mut self.rect
    }

    fn children(&self) -> &Vec<Box<View>> {
        &self.children
    }

    fn children_mut(&mut self) -> &mut Vec<Box<View>> {
        &mut self.children
    }
}
