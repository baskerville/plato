use crate::device::CURRENT_DEVICE;
use crate::framebuffer::{Framebuffer, UpdateMode};
use crate::settings::ReaderSettings;
use crate::metadata::{ReaderInfo, TextAlign};
use crate::metadata::{DEFAULT_CONTRAST_EXPONENT, DEFAULT_CONTRAST_GRAY};
use crate::view::{View, Event, Hub, Bus, SliderId, ViewId, THICKNESS_MEDIUM};
use crate::view::filler::Filler;
use crate::view::slider::Slider;
use crate::view::icon::Icon;
use crate::view::labeled_icon::LabeledIcon;
use crate::gesture::GestureEvent;
use crate::input::DeviceEvent;
use crate::unit::scale_by_dpi;
use crate::geom::Rectangle;
use crate::font::Fonts;
use crate::color::{SEPARATOR_NORMAL, WHITE};
use crate::app::Context;

#[derive(Debug)]
pub struct ToolBar {
    rect: Rectangle,
    children: Vec<Box<dyn View>>,
    reflowable: bool,
}

impl ToolBar {
    pub fn new(rect: Rectangle, reflowable: bool, reader_info: Option<&ReaderInfo>, reader_settings: &ReaderSettings) -> ToolBar {
        let mut children = Vec::new();
        let dpi = CURRENT_DEVICE.dpi;
        let thickness = scale_by_dpi(THICKNESS_MEDIUM, dpi) as i32;
        let side = (rect.height() as i32 + thickness) / 2 - thickness;

        if reflowable {
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
            children.push(Box::new(margin_icon) as Box<dyn View>);
            x_offset += side + margin_label_width;

            let font_family = reader_info.and_then(|r| r.font_family.clone())
                                         .unwrap_or_else(|| reader_settings.font_family.clone());
            let font_family_icon = LabeledIcon::new("font_family",
                                                    rect![x_offset, rect.min.y,
                                                          x_offset + side + font_family_label_width, rect.min.y + side],
                                                    Event::Show(ViewId::FontFamilyMenu),
                                                    font_family);
            children.push(Box::new(font_family_icon) as Box<dyn View>);
            x_offset += side + font_family_label_width;

            let line_height = reader_info.and_then(|r| r.line_height)
                                         .unwrap_or(reader_settings.line_height);
            let line_height_icon = LabeledIcon::new("line_height",
                                                    rect![x_offset, rect.min.y,
                                                          x_offset + side + line_height_label_width, rect.min.y + side],
                                                    Event::Show(ViewId::LineHeightMenu),
                                                    format!("{:.1} em", line_height));
            children.push(Box::new(line_height_icon) as Box<dyn View>);

            // Separator.
            let separator = Filler::new(rect![rect.min.x, rect.min.y + side,
                                              rect.max.x, rect.max.y - side],
                                        SEPARATOR_NORMAL);
            children.push(Box::new(separator) as Box<dyn View>);

            // Start of second row.
            let text_align = reader_info.and_then(|r| r.text_align)
                                        .unwrap_or(reader_settings.text_align);
            let text_align_rect = rect![rect.min.x, rect.max.y - side,
                                       rect.min.x + side, rect.max.y];
            let text_align_icon = Icon::new(text_align.icon_name(),
                                           text_align_rect,
                                           Event::ToggleNear(ViewId::TextAlignMenu, text_align_rect));
            children.push(Box::new(text_align_icon) as Box<dyn View>);

            let font_size = reader_info.and_then(|r| r.font_size)
                                       .unwrap_or(reader_settings.font_size);
            let font_size_rect = rect![rect.min.x + side, rect.max.y - side,
                                       rect.min.x + 2 * side, rect.max.y];
            let font_size_icon = Icon::new("font_size",
                                           font_size_rect,
                                           Event::ToggleNear(ViewId::FontSizeMenu, font_size_rect));
            children.push(Box::new(font_size_icon) as Box<dyn View>);

            let slider = Slider::new(rect![rect.min.x + 2 * side, rect.max.y - side,
                                           rect.max.x - 2 * side, rect.max.y],
                                     SliderId::FontSize,
                                     font_size,
                                     reader_settings.font_size / 2.0,
                                     3.0 * reader_settings.font_size / 2.0);
            children.push(Box::new(slider) as Box<dyn View>);
        } else {
            let remaining_width = rect.width() as i32 - 2 * side;
            let slider_width = remaining_width / 2;
            // First row.
            let contrast_icon_rect = rect![rect.min.x, rect.min.y,
                                           rect.min.x + side, rect.min.y + side];
            let contrast_icon = Icon::new("contrast",
                                          contrast_icon_rect,
                                          Event::ToggleNear(ViewId::ContrastExponentMenu, contrast_icon_rect));
            children.push(Box::new(contrast_icon) as Box<dyn View>);

            let contrast_exponent = reader_info.and_then(|r| r.contrast_exponent)
                                               .unwrap_or(DEFAULT_CONTRAST_EXPONENT);
            let slider = Slider::new(rect![rect.min.x + side, rect.min.y,
                                           rect.min.x + side + slider_width, rect.min.y + side],
                                     SliderId::ContrastExponent,
                                     contrast_exponent,
                                     1.0,
                                     5.0);
            children.push(Box::new(slider) as Box<dyn View>);

            let gray_icon_rect = rect![rect.min.x + side + slider_width, rect.min.y,
                                       rect.min.x + 2 * side + slider_width, rect.min.y + side];
            let gray_icon = Icon::new("gray",
                                      gray_icon_rect,
                                      Event::ToggleNear(ViewId::ContrastGrayMenu, gray_icon_rect));
            children.push(Box::new(gray_icon) as Box<dyn View>);

            let contrast_gray = reader_info.and_then(|r| r.contrast_gray)
                                           .unwrap_or(DEFAULT_CONTRAST_GRAY);
            let slider = Slider::new(rect![rect.min.x + 2 * side + slider_width, rect.min.y,
                                           rect.max.x - side / 3, rect.min.y + side],
                                     SliderId::ContrastGray,
                                     contrast_gray,
                                     0.0,
                                     255.0);
            children.push(Box::new(slider) as Box<dyn View>);

            let filler = Filler::new(rect![rect.max.x - side / 3,
                                           rect.min.y,
                                           rect.max.x,
                                           rect.min.y + side],
                                     WHITE);
            children.push(Box::new(filler) as Box<dyn View>);


            // Separator.
            let separator = Filler::new(rect![rect.min.x, rect.min.y + side,
                                              rect.max.x, rect.max.y - side],
                                        SEPARATOR_NORMAL);
            children.push(Box::new(separator) as Box<dyn View>);


            // Start of second row.
            let crop_icon = Icon::new("crop",
                                      rect![rect.min.x, rect.max.y - side,
                                            rect.min.x + side, rect.max.y],
                                      Event::Show(ViewId::MarginCropper));
            children.push(Box::new(crop_icon) as Box<dyn View>);

            let remaining_width = rect.width() as i32 - 3 * side;
            let margin_label_width = (2 * side).min(remaining_width);
            let big_padding = (remaining_width - margin_label_width) / 2;
            let small_padding = remaining_width - margin_label_width - big_padding;

            let filler = Filler::new(rect![rect.min.x + side,
                                           rect.max.y - side,
                                           rect.min.x + side + small_padding,
                                           rect.max.y],
                                     WHITE);
            children.push(Box::new(filler) as Box<dyn View>);

            let margin_width = reader_info.and_then(|r| r.screen_margin_width)
                                          .unwrap_or(0);
            let margin_icon = LabeledIcon::new("margin",
                                               rect![rect.min.x + side + small_padding,
                                                     rect.max.y - side,
                                                     rect.max.x - 2 * side - big_padding,
                                                     rect.max.y],
                                               Event::Show(ViewId::MarginWidthMenu),
                                               format!("{} mm", margin_width));
            children.push(Box::new(margin_icon) as Box<dyn View>);

            let filler = Filler::new(rect![rect.max.x - 2 * side - big_padding,
                                           rect.max.y - side,
                                           rect.max.x - 2 * side,
                                           rect.max.y],
                                     WHITE);
            children.push(Box::new(filler) as Box<dyn View>);

        }

        // End of second row.

        let search_icon = Icon::new("search",
                                    rect![rect.max.x - 2 * side, rect.max.y - side,
                                          rect.max.x - side, rect.max.y],
                                    Event::Show(ViewId::SearchBar));
        children.push(Box::new(search_icon) as Box<dyn View>);

        let toc_icon = Icon::new("toc",
                                 rect![rect.max.x - side, rect.max.y - side,
                                       rect.max.x, rect.max.y],
                                 Event::Show(ViewId::TableOfContents));
        children.push(Box::new(toc_icon) as Box<dyn View>);

        ToolBar {
            rect,
            children,
            reflowable,
        }
    }

    pub fn update_margin_width(&mut self, margin_width: i32, hub: &Hub) {
        let index = if self.reflowable { 0 } else { 8 };
        if let Some(labeled_icon) = self.children[index].downcast_mut::<LabeledIcon>() {
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

    pub fn update_text_align_icon(&mut self, text_align: TextAlign, hub: &Hub) {
        let icon = self.child_mut(4).downcast_mut::<Icon>().unwrap();
        let name = text_align.icon_name();
        if icon.name != name {
            icon.name = name.to_string();
            hub.send(Event::Render(*icon.rect(), UpdateMode::Gui)).unwrap();
        }
    }

    pub fn update_font_size_slider(&mut self, font_size: f32, hub: &Hub) {
        let slider = self.children[6].as_mut().downcast_mut::<Slider>().unwrap();
        slider.update(font_size, hub);
    }

    pub fn update_contrast_exponent_slider(&mut self, exponent: f32, hub: &Hub) {
        let slider = self.children[1].as_mut().downcast_mut::<Slider>().unwrap();
        slider.update(exponent, hub);
    }

    pub fn update_contrast_gray_slider(&mut self, gray: f32, hub: &Hub) {
        let slider = self.children[3].as_mut().downcast_mut::<Slider>().unwrap();
        slider.update(gray, hub);
    }
}

impl View for ToolBar {
    fn handle_event(&mut self, evt: &Event, _hub: &Hub, _bus: &mut Bus, _context: &mut Context) -> bool {
        match *evt {
            Event::Gesture(GestureEvent::Tap(center)) |
            Event::Gesture(GestureEvent::HoldFingerShort(center, ..)) if self.rect.includes(center) => true,
            Event::Gesture(GestureEvent::Swipe { start, .. }) if self.rect.includes(start) => true,
            Event::Device(DeviceEvent::Finger { position, .. }) if self.rect.includes(position) => true,
            _ => false,
        }
    }

    fn render(&self, _fb: &mut dyn Framebuffer, _rect: Rectangle, _fonts: &mut Fonts) {
    }

    fn resize(&mut self, rect: Rectangle, hub: &Hub, context: &mut Context) {
        let dpi = CURRENT_DEVICE.dpi;
        let thickness = scale_by_dpi(THICKNESS_MEDIUM, dpi) as i32;
        let side = (rect.height() as i32 + thickness) / 2 - thickness;

        let mut index = 0;

        if self.reflowable {
            let mut remaining_width = rect.width() as i32 - 3 * side;
            let font_family_label_width = remaining_width / 2;
            remaining_width -= font_family_label_width;
            let margin_label_width = remaining_width / 2;
            let line_height_label_width = remaining_width - margin_label_width;

            // First row.

            let mut x_offset = rect.min.x;
            self.children[index].resize(rect![x_offset, rect.min.y,
                                              x_offset + side + margin_label_width, rect.min.y + side],
                                        hub, context);
            index += 1;
            x_offset += side + margin_label_width;

            self.children[index].resize(rect![x_offset, rect.min.y,
                                              x_offset + side + font_family_label_width, rect.min.y + side],
                                        hub, context);
            index += 1;
            x_offset += side + font_family_label_width;

            self.children[index].resize(rect![x_offset, rect.min.y,
                                              x_offset + side + line_height_label_width, rect.min.y + side],
                                        hub, context);
            index += 1;

            // Separator.
            self.children[index].resize(rect![rect.min.x, rect.min.y + side,
                                              rect.max.x, rect.max.y - side],
                                        hub, context);
            index += 1;

            // Start of second row.
            let text_align_rect = rect![rect.min.x, rect.max.y - side,
                                        rect.min.x + side, rect.max.y];
            self.children[index].resize(text_align_rect, hub, context);
            index += 1;

            let font_size_rect = rect![rect.min.x + side, rect.max.y - side,
                                       rect.min.x + 2 * side, rect.max.y];
            self.children[index].resize(font_size_rect, hub, context);
            index += 1;

            self.children[index].resize(rect![rect.min.x + 2 * side, rect.max.y - side,
                                              rect.max.x - 2 * side, rect.max.y],
                                        hub, context);
            index += 1;
        } else {
            let remaining_width = rect.width() as i32 - 2 * side;
            let slider_width = remaining_width / 2;

            // First row.
            let contrast_icon_rect = rect![rect.min.x, rect.min.y,
                                           rect.min.x + side, rect.min.y + side];

            self.children[index].resize(contrast_icon_rect, hub, context);
            index += 1;

            self.children[index].resize(rect![rect.min.x + side, rect.min.y,
                                              rect.min.x + side + slider_width, rect.min.y + side],
                                        hub, context);
            index += 1;

            let gray_icon_rect = rect![rect.min.x + side + slider_width, rect.min.y,
                                       rect.min.x + 2 * side + slider_width, rect.min.y + side];

            self.children[index].resize(gray_icon_rect, hub, context);
            index += 1;

            self.children[index].resize(rect![rect.min.x + 2 * side + slider_width, rect.min.y,
                                              rect.max.x - side / 3, rect.min.y + side],
                                        hub, context);
            index += 1;

            self.children[index].resize(rect![rect.max.x - side / 3,
                                              rect.min.y,
                                              rect.max.x,
                                              rect.min.y + side],
                                        hub, context);
            index += 1;

            // Separator.
            self.children[index].resize(rect![rect.min.x, rect.min.y + side,
                                              rect.max.x, rect.max.y - side],
                                        hub, context);
            index += 1;

            // Start of second row.
            self.children[index].resize(rect![rect.min.x, rect.max.y - side,
                                              rect.min.x + side, rect.max.y],
                                        hub, context);
            index += 1;

            let remaining_width = rect.width() as i32 - 3 * side;
            let margin_label_width = self.children[index+1].rect().width() as i32;
            let big_padding = (remaining_width - margin_label_width) / 2;
            let small_padding = remaining_width - margin_label_width - big_padding;

            self.children[index].resize(rect![rect.min.x + side,
                                              rect.max.y - side,
                                              rect.min.x + side + small_padding,
                                              rect.max.y],
                                        hub, context);

            index += 1;
            self.children[index].resize(rect![rect.min.x + side + small_padding,
                                              rect.max.y - side,
                                              rect.max.x - 2 * side - big_padding,
                                              rect.max.y],
                                        hub, context);
            index += 1;
            self.children[index].resize(rect![rect.max.x - 2 * side - big_padding,
                                              rect.max.y - side,
                                              rect.max.x - 2 * side,
                                              rect.max.y],
                                        hub, context);
            index += 1;
        }

        // End of second row.

        self.children[index].resize(rect![rect.max.x - 2 * side, rect.max.y - side,
                                          rect.max.x - side, rect.max.y],
                                    hub, context);
        index += 1;

        self.children[index].resize(rect![rect.max.x - side, rect.max.y - side,
                                         rect.max.x, rect.max.y],
                                    hub, context);
        self.rect = rect;
    }

    fn rect(&self) -> &Rectangle {
        &self.rect
    }

    fn rect_mut(&mut self) -> &mut Rectangle {
        &mut self.rect
    }

    fn children(&self) -> &Vec<Box<dyn View>> {
        &self.children
    }

    fn children_mut(&mut self) -> &mut Vec<Box<dyn View>> {
        &mut self.children
    }
}
