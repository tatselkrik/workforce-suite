//! Shared native visual language, applied independently to HRIS and Payroll contexts.
use super::*;

#[derive(Clone, Copy)]
pub(super) struct Palette {
    pub page: Color32,
    pub sidebar: Color32,
    pub surface: Color32,
    pub input: Color32,
    pub text: Color32,
    pub secondary: Color32,
    pub border: Color32,
    pub accent: Color32,
    pub selected: Color32,
    pub hover: Color32,
    pub positive: Color32,
    pub positive_soft: Color32,
    pub warning: Color32,
    pub warning_soft: Color32,
    pub error: Color32,
    pub error_soft: Color32,
}
impl Palette {
    pub fn for_dark(dark: bool) -> Self {
        if dark {
            Self {
                page: Color32::from_rgb(19, 29, 40),
                sidebar: Color32::from_rgb(23, 36, 49),
                surface: Color32::from_rgb(29, 43, 57),
                input: Color32::from_rgb(24, 37, 50),
                text: Color32::from_rgb(235, 243, 247),
                secondary: Color32::from_rgb(171, 189, 203),
                border: Color32::from_rgb(51, 69, 84),
                accent: Color32::from_rgb(111, 220, 207),
                selected: Color32::from_rgb(30, 66, 74),
                hover: Color32::from_rgb(35, 53, 67),
                positive: Color32::from_rgb(136, 223, 177),
                positive_soft: Color32::from_rgb(28, 63, 52),
                warning: Color32::from_rgb(241, 198, 122),
                warning_soft: Color32::from_rgb(65, 52, 33),
                error: Color32::from_rgb(255, 170, 155),
                error_soft: Color32::from_rgb(69, 41, 44),
            }
        } else {
            Self {
                page: Color32::from_rgb(246, 249, 250),
                sidebar: Color32::from_rgb(240, 246, 247),
                surface: Color32::WHITE,
                input: Color32::from_rgb(247, 250, 251),
                text: Color32::from_rgb(24, 47, 62),
                secondary: Color32::from_rgb(83, 106, 120),
                border: Color32::from_rgb(220, 231, 235),
                accent: Color32::from_rgb(0, 107, 117),
                selected: Color32::from_rgb(216, 239, 240),
                hover: Color32::from_rgb(229, 239, 242),
                positive: Color32::from_rgb(29, 111, 74),
                positive_soft: Color32::from_rgb(231, 245, 236),
                warning: Color32::from_rgb(139, 88, 16),
                warning_soft: Color32::from_rgb(252, 243, 222),
                error: Color32::from_rgb(164, 54, 43),
                error_soft: Color32::from_rgb(255, 237, 232),
            }
        }
    }
}
const KEY: &str = "hris-design-v1";
pub(super) fn enabled(ctx: &egui::Context) -> bool {
    ctx.data(|data| data.get_temp::<bool>(egui::Id::unique(KEY)))
        .unwrap_or(false)
}
pub(super) fn palette(ui: &Ui) -> Palette {
    Palette::for_dark(ui.visuals().dark_mode)
}
pub(super) fn apply(ctx: &egui::Context, product: crate::desktop::Product) {
    let active = matches!(
        product,
        crate::desktop::Product::Hris | crate::desktop::Product::Payroll
    );
    ctx.data_mut(|data| data.insert_temp(egui::Id::unique(KEY), active));
    if !active {
        ctx.set_fonts(egui::FontDefinitions::default());
        return;
    }
    ctx.set_fonts(native_fonts());
    for theme in [egui::Theme::Light, egui::Theme::Dark] {
        let p = Palette::for_dark(theme == egui::Theme::Dark);
        ctx.style_mut_of(theme, |style| {
            let v = &mut style.visuals;
            v.panel_fill = p.page;
            v.window_fill = p.surface;
            v.extreme_bg_color = p.input;
            v.faint_bg_color = if theme == egui::Theme::Dark {
                Color32::from_rgb(33, 49, 63)
            } else {
                Color32::from_rgb(245, 249, 250)
            };
            v.override_text_color = Some(p.text);
            v.weak_text_color = Some(p.secondary);
            v.hyperlink_color = p.accent;
            v.warn_fg_color = p.warning;
            v.error_fg_color = p.error;
            v.text_edit_bg_color = Some(p.input);
            v.selection.bg_fill = p.selected;
            v.selection.stroke = Stroke::new(1.0, p.accent);
            v.window_stroke = Stroke::new(1.0, p.border);
            v.widgets.noninteractive.bg_fill = p.surface;
            v.widgets.noninteractive.weak_bg_fill = p.surface;
            v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, p.border);
            v.widgets.noninteractive.fg_stroke = Stroke::new(1.0, p.text);
            v.widgets.inactive.bg_fill = p.input;
            v.widgets.inactive.weak_bg_fill = p.input;
            v.widgets.inactive.bg_stroke = Stroke::new(1.0, p.border);
            v.widgets.inactive.fg_stroke = Stroke::new(1.0, p.text);
            v.widgets.hovered.bg_fill = p.hover;
            v.widgets.hovered.weak_bg_fill = p.hover;
            v.widgets.hovered.bg_stroke = Stroke::new(1.0, p.accent);
            v.widgets.hovered.fg_stroke = Stroke::new(1.0, p.text);
            v.widgets.active.bg_fill = p.selected;
            v.widgets.active.weak_bg_fill = p.selected;
            v.widgets.active.bg_stroke = Stroke::new(1.5, p.accent);
            v.widgets.active.fg_stroke = Stroke::new(1.0, p.text);
            v.widgets.open.bg_fill = p.hover;
            v.widgets.open.weak_bg_fill = p.hover;
            v.widgets.open.bg_stroke = Stroke::new(1.0, p.border);
            v.widgets.open.fg_stroke = Stroke::new(1.0, p.text);
            for w in [
                &mut v.widgets.inactive,
                &mut v.widgets.hovered,
                &mut v.widgets.active,
                &mut v.widgets.open,
            ] {
                w.corner_radius = egui::CornerRadius::same(7);
            }
            style
                .text_styles
                .insert(egui::TextStyle::Heading, semibold(28.0));
            style.spacing.item_spacing = egui::vec2(12.0, 10.0);
            style.spacing.button_padding = egui::vec2(12.0, 8.0);
            style.spacing.interact_size.y = 34.0;
        });
    }
}

/// Reuse installed Windows UI fonts; no proprietary font files enter the package.
/// Other platforms and missing fonts retain egui's bundled free fallback.
fn native_fonts() -> egui::FontDefinitions {
    static FONTS: std::sync::OnceLock<egui::FontDefinitions> = std::sync::OnceLock::new();
    FONTS
        .get_or_init(|| {
            let mut fonts = egui::FontDefinitions::default();
            let windows = std::env::var_os("WINDIR").map(std::path::PathBuf::from);
            for (name, file) in [
                ("hris_body", "segoeui.ttf"),
                ("hris_semibold", "seguisb.ttf"),
            ] {
                if let Some(path) = windows.as_ref().map(|p| p.join("Fonts").join(file)) {
                    if std::fs::metadata(&path).is_ok_and(|m| m.len() < 5 * 1024 * 1024) {
                        if let Ok(bytes) = std::fs::read(path) {
                            fonts.font_data.insert(
                                name.into(),
                                std::sync::Arc::new(egui::FontData::from_owned(bytes)),
                            );
                        }
                    }
                }
            }
            if fonts.font_data.contains_key("hris_body") {
                fonts
                    .families
                    .get_mut(&egui::FontFamily::Proportional)
                    .unwrap()
                    .insert(0, "hris_body".into());
            }
            let mut heading = fonts.families[&egui::FontFamily::Proportional].clone();
            if fonts.font_data.contains_key("hris_semibold") {
                heading.insert(0, "hris_semibold".into());
            }
            fonts
                .families
                .insert(egui::FontFamily::Name("hris_semibold".into()), heading);
            fonts
        })
        .clone()
}
pub(super) fn semibold(size: f32) -> FontId {
    FontId::new(size, egui::FontFamily::Name("hris_semibold".into()))
}

/// Native line art avoids platform-dependent emoji and external font dependencies.
pub(super) fn icon(ui: &Ui, page: Page, rect: egui::Rect, color: Color32) {
    let p = ui.painter();
    let s = Stroke::new(1.4, color);
    let at = |x: f32, y: f32| {
        egui::pos2(
            rect.left() + rect.width() * x,
            rect.top() + rect.height() * y,
        )
    };
    let line = |a: (f32, f32), b: (f32, f32)| {
        p.line_segment([at(a.0, a.1), at(b.0, b.1)], s);
    };
    let box_at = |x: f32, y: f32, w: f32, h: f32| {
        p.rect_stroke(
            egui::Rect::from_min_max(at(x, y), at(x + w, y + h)),
            2,
            s,
            egui::StrokeKind::Inside,
        );
    };
    match page {
        Page::Overview => {
            for (x, y) in [(0.10, 0.10), (0.56, 0.10), (0.10, 0.56), (0.56, 0.56)] {
                box_at(x, y, 0.34, 0.34);
            }
        }
        Page::Employees | Page::Accounts | Page::Profile => {
            p.circle_stroke(at(0.42, 0.29), rect.width() * 0.16, s);
            p.line(
                vec![
                    at(0.10, 0.91),
                    at(0.13, 0.70),
                    at(0.29, 0.59),
                    at(0.56, 0.59),
                    at(0.72, 0.70),
                    at(0.75, 0.91),
                ],
                s,
            );
            if page != Page::Profile {
                p.circle_stroke(at(0.78, 0.33), rect.width() * 0.10, s);
                line((0.81, 0.61), (0.94, 0.74));
                line((0.94, 0.74), (0.94, 0.91));
            }
        }
        Page::Attendance => {
            p.circle_stroke(rect.center(), rect.width() * 0.39, s);
            line((0.50, 0.24), (0.50, 0.50));
            line((0.50, 0.50), (0.71, 0.62));
        }
        Page::Leave => {
            box_at(0.12, 0.20, 0.76, 0.68);
            line((0.12, 0.40), (0.88, 0.40));
            line((0.31, 0.09), (0.31, 0.29));
            line((0.69, 0.09), (0.69, 0.29));
            line((0.29, 0.60), (0.43, 0.75));
            line((0.43, 0.75), (0.72, 0.50));
        }
        Page::Reports => {
            line((0.10, 0.91), (0.91, 0.91));
            box_at(0.16, 0.53, 0.14, 0.32);
            box_at(0.43, 0.29, 0.14, 0.56);
            box_at(0.70, 0.12, 0.14, 0.73);
        }
        Page::Hiring => {
            box_at(0.09, 0.28, 0.82, 0.59);
            box_at(0.34, 0.10, 0.32, 0.20);
            line((0.11, 0.54), (0.90, 0.54));
            line((0.48, 0.47), (0.48, 0.64));
        }
        Page::Exits => {
            box_at(0.13, 0.14, 0.45, 0.73);
            line((0.41, 0.50), (0.94, 0.50));
            line((0.72, 0.28), (0.94, 0.50));
            line((0.72, 0.72), (0.94, 0.50));
        }
        Page::Settings => {
            p.circle_stroke(rect.center(), rect.width() * 0.28, s);
            p.circle_stroke(rect.center(), rect.width() * 0.10, s);
            for i in 0..8 {
                let a = i as f32 * std::f32::consts::TAU / 8.0;
                let d = egui::vec2(a.cos(), a.sin());
                p.line_segment(
                    [
                        rect.center() + d * rect.width() * 0.31,
                        rect.center() + d * rect.width() * 0.43,
                    ],
                    s,
                );
            }
        }
        Page::Services => {
            box_at(0.10, 0.12, 0.80, 0.61);
            line((0.25, 0.73), (0.25, 0.94));
            line((0.25, 0.94), (0.48, 0.73));
            line((0.28, 0.34), (0.73, 0.34));
            line((0.28, 0.51), (0.59, 0.51));
        }
        Page::Notifications => {
            p.line(
                vec![
                    at(0.14, 0.73),
                    at(0.25, 0.58),
                    at(0.25, 0.32),
                    at(0.36, 0.16),
                    at(0.64, 0.16),
                    at(0.75, 0.32),
                    at(0.75, 0.58),
                    at(0.86, 0.73),
                    at(0.14, 0.73),
                ],
                s,
            );
            line((0.39, 0.88), (0.61, 0.88));
        }
        Page::Payroll => {
            p.circle_stroke(rect.center(), rect.width() * 0.39, s);
            line((0.30, 0.36), (0.66, 0.36));
            line((0.30, 0.53), (0.59, 0.53));
            line((0.40, 0.24), (0.40, 0.76));
        }
    }
}

pub(super) fn nav_item(ui: &mut Ui, page: Page, label: &str, selected: bool, height: f32) -> bool {
    nav_item_with_rect(ui, page, label, selected, height).0
}

pub(super) fn nav_item_with_rect(
    ui: &mut Ui,
    page: Page,
    label: &str,
    selected: bool,
    height: f32,
) -> (bool, egui::Rect) {
    let p = palette(ui);
    let (rect, response) = ui.allocate_exact_size(
        egui::vec2(ui.available_width(), height),
        egui::Sense::click(),
    );
    response.widget_info(|| {
        let mut info = egui::WidgetInfo::labeled(egui::Role::Button, ui.is_enabled(), label);
        info.selected = Some(selected);
        info
    });
    if selected || response.hovered() {
        ui.painter()
            .rect_filled(rect, 7, if selected { p.selected } else { p.hover });
    }
    if selected {
        ui.painter().rect_filled(
            egui::Rect::from_min_size(
                egui::pos2(rect.left(), rect.top() + 7.0),
                egui::vec2(2.5, rect.height() - 14.0),
            ),
            2,
            p.accent,
        );
    }
    if response.has_focus() {
        ui.painter().rect_stroke(
            rect.shrink(1.0),
            7,
            Stroke::new(1.5, p.accent),
            egui::StrokeKind::Inside,
        );
    }
    let color = if selected { p.accent } else { p.text };
    icon(
        ui,
        page,
        egui::Rect::from_center_size(
            egui::pos2(rect.left() + 21.0, rect.center().y),
            egui::vec2(17.0, 17.0),
        ),
        color,
    );
    let mut child = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(egui::Rect::from_min_max(
                egui::pos2(rect.left() + 41.0, rect.top()),
                egui::pos2(rect.right() - 8.0, rect.bottom()),
            ))
            .layout(Layout::left_to_right(Align::Center)),
    );
    child.add(egui::Label::new(RichText::new(label).size(14.0).color(color)).truncate());
    if response.gained_focus() {
        ui.scroll_to_rect(rect, Some(Align::Center));
    }
    let keyboard = ui.is_enabled()
        && response.has_focus()
        && ui.input(|i| i.key_pressed(egui::Key::Enter) || i.key_pressed(egui::Key::Space));
    let clicked = response
        .on_hover_text(label)
        .on_hover_cursor(egui::CursorIcon::PointingHand)
        .clicked()
        || keyboard;
    (clicked, rect)
}

pub(super) fn metric(ui: &mut Ui, page: Page, title: &str, value: usize, caption: &str) {
    let p = palette(ui);
    let tone = match page {
        Page::Attendance => p.positive,
        Page::Leave => p.warning,
        _ => p.accent,
    };
    super::card(ui, |ui| {
        ui.set_min_height(91.0);
        ui.spacing_mut().item_spacing.y = 6.0;
        ui.horizontal(|ui| {
            let (r, _) = ui.allocate_exact_size(egui::vec2(26.0, 26.0), egui::Sense::hover());
            ui.painter().circle_filled(
                r.center(),
                13.0,
                match page {
                    Page::Attendance => p.positive_soft,
                    Page::Leave => p.warning_soft,
                    _ => p.selected,
                },
            );
            icon(ui, page, r.shrink(5.0), tone);
            ui.add(egui::Label::new(RichText::new(title).size(12.0).color(p.secondary)).truncate())
                .on_hover_text(title);
        });
        ui.label(
            RichText::new(value.to_string())
                .font(semibold(31.0))
                .color(tone)
                .strong(),
        );
        ui.add(egui::Label::new(RichText::new(caption).size(11.0).color(p.secondary)).truncate())
            .on_hover_text(caption);
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    fn luminance(c: Color32) -> f32 {
        let rgb = [c.r(), c.g(), c.b()].map(|n| {
            let s = n as f32 / 255.0;
            if s <= 0.04045 {
                s / 12.92
            } else {
                ((s + 0.055) / 1.055).powf(2.4)
            }
        });
        0.2126 * rgb[0] + 0.7152 * rgb[1] + 0.0722 * rgb[2]
    }
    fn contrast(a: Color32, b: Color32) -> f32 {
        let x = luminance(a);
        let y = luminance(b);
        (x.max(y) + 0.05) / (x.min(y) + 0.05)
    }
    #[test]
    fn normal_text_selection_and_statuses_have_accessible_contrast() {
        for dark in [false, true] {
            let p = Palette::for_dark(dark);
            for (fg, bg) in [
                (p.text, p.page),
                (p.text, p.surface),
                (p.text, p.sidebar),
                (p.secondary, p.surface),
                (p.secondary, p.sidebar),
                (p.accent, p.selected),
                (p.positive, p.positive_soft),
                (p.warning, p.warning_soft),
                (p.error, p.error_soft),
            ] {
                assert!(
                    contrast(fg, bg) >= 4.5,
                    "Insufficient contrast {dark}: {fg:?}/{bg:?}"
                );
            }
        }
    }
    #[test]
    fn payroll_uses_the_approved_hris_theme_without_changing_hris_configuration() {
        let hris = egui::Context::default();
        super::super::configure_theme_for(&hris, crate::desktop::Product::Hris);
        let payroll = egui::Context::default();
        super::super::configure_theme_for(&payroll, crate::desktop::Product::Payroll);
        assert!(enabled(&hris) && enabled(&payroll));
        for theme in [egui::Theme::Light, egui::Theme::Dark] {
            let actual = payroll.style_of(theme);
            let mut expected = (*hris.style_of(theme)).clone();
            // Separate Contexts allocate distinct default number formatter callbacks.
            expected.number_formatter = actual.number_formatter.clone();
            assert_eq!(*actual, expected);
            let palette = Palette::for_dark(theme == egui::Theme::Dark);
            assert_eq!(hris.style_of(theme).visuals.panel_fill, palette.page);
            assert_eq!(hris.style_of(theme).visuals.text_color(), palette.text);
        }
    }
    #[test]
    fn sidebar_keyboard_navigation_obeys_disabled_edit_guards() {
        let ctx = egui::Context::default();
        super::super::configure_theme(&ctx);
        let base = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(320.0, 180.0),
            )),
            ..Default::default()
        };
        let draw = |input: egui::RawInput, enabled: bool| {
            let mut clicked = false;
            let mut out = ctx.run_ui(input, |ui| {
                egui::CentralPanel::default().show(ui, |ui| {
                    ui.add_enabled_ui(enabled, |ui| {
                        clicked = nav_item(ui, Page::Overview, "Overview", false, 34.0);
                    });
                });
            });
            out.textures_delta.clear();
            clicked
        };
        assert!(!draw(base.clone(), true));
        let key = |k| {
            let mut input = base.clone();
            input.events.push(egui::Event::Key {
                key: k,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            });
            input
        };
        draw(key(egui::Key::Tab), true);
        draw(base.clone(), true);
        assert!(
            ctx.memory(|m| m.focused()).is_some(),
            "Sidebar must be keyboard reachable"
        );
        assert!(
            draw(key(egui::Key::Enter), true),
            "Focused sidebar opens with Enter"
        );
        assert!(!draw(base.clone(), false));
        assert!(
            !draw(key(egui::Key::Enter), false),
            "Disabled editor navigation must not activate"
        );
    }
}

pub(super) fn attendance_row(ui: &mut Ui, cells: [&str; 4], header: bool, completed: bool) {
    let p = palette(ui);
    let gap = 10.0;
    let total = (ui.available_width() - gap * 3.0).max(220.0);
    let widths = [total * 0.32, total * 0.29, total * 0.16, total * 0.23];
    let result = ui.horizontal_top(|ui| {
        ui.spacing_mut().item_spacing.x = gap;
        for (i, text) in cells.into_iter().enumerate() {
            ui.allocate_ui_with_layout(
                egui::vec2(widths[i], if header { 25.0 } else { 36.0 }),
                Layout::left_to_right(Align::Center),
                |ui| {
                    ui.set_min_width(widths[i]);
                    ui.set_max_width(widths[i]);
                    if i == 3 && !header {
                        super::badge(ui, if completed { "Completed" } else { "Checked in" }, true);
                    } else {
                        ui.add(
                            egui::Label::new(
                                RichText::new(text)
                                    .size(if header { 11.0 } else { 14.0 })
                                    .color(if header { p.secondary } else { p.text }),
                            )
                            .truncate(),
                        )
                        .on_hover_text(text);
                    }
                },
            );
        }
    });
    if !header {
        ui.painter().line_segment(
            [
                result.response.rect.left_bottom(),
                result.response.rect.right_bottom(),
            ],
            Stroke::new(0.5, p.border),
        );
    }
}

impl HrisApp {
    pub(super) fn design_toolbar(&mut self, ui: &mut Ui) {
        self.notification_state
            .for_product(&self.data, self.desktop.product);
        let compact = ui.available_width() < 660.0;
        ui.horizontal(|ui| {
            ui.label(
                RichText::new(self.desktop.product.label(self.page, &self.data))
                    .size(12.0)
                    .color(muted(ui)),
            );
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.label(
                    RichText::new(format!("v{}", self.desktop.version))
                        .size(11.0)
                        .color(muted(ui)),
                );
                if !compact {
                    self.design_tools(ui);
                }
                ui.label(
                    RichText::new(self.data.company.today().format("%b %d, %Y").to_string())
                        .size(12.0)
                        .color(muted(ui)),
                );
            });
        });
        if compact {
            ui.add_space(6.0);
            ui.horizontal_wrapped(|ui| self.design_tools(ui));
        }
    }
    fn design_tools(&mut self, ui: &mut Ui) {
        if self.busy {
            ui.spinner();
            ui.label(RichText::new("Working…").small().color(muted(ui)));
        } else if ui
            .add(egui::Button::new(RichText::new("Refresh").size(12.0)))
            .clicked()
        {
            self.send(Command::Refresh);
        }
        let unread = self.notification_state.unread;
        if ui
            .add_enabled(
                !self.busy && !self.has_unsaved_edits(),
                egui::Button::new(RichText::new(format!("Notifications ({unread})")).size(12.0)),
            )
            .on_hover_text("Save or close open editors before opening Notifications.")
            .clicked()
        {
            self.navigate(Page::Notifications, ui.ctx());
        }
        if self.desktop.product == crate::desktop::Product::Hris
            && self
                .data
                .access
                .allows(crate::staff_roles::Capability::PayrollPreparation)
            && ui
                .add_enabled(
                    !self.busy && !self.has_unsaved_edits(),
                    egui::Button::new(RichText::new("Open Payroll Desktop").size(12.0)),
                )
                .clicked()
        {
            if let Err(error) = crate::desktop::open_payroll() {
                self.notice = Some((error.to_string(), true));
            }
        }
    }
    pub(super) fn overview_attendance_card(
        &mut self,
        ui: &mut Ui,
        today: NaiveDate,
        attendance: usize,
    ) {
        card(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new("Today's attendance").font(semibold(17.0)));
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if ui.button("View attendance").clicked() {
                        self.page = Page::Attendance;
                    }
                });
            });
            ui.label(
                RichText::new(format!(
                    "{} · {}",
                    today.format("%A, %B %d"),
                    self.data.company.timezone
                ))
                .color(muted(ui))
                .size(12.0),
            );
            ui.add_space(10.0);
            attendance_row(
                ui,
                ["EMPLOYEE", "DEPARTMENT", "CHECK-IN", "STATUS"],
                true,
                false,
            );
            for record in self
                .data
                .attendance
                .iter()
                .filter(|a| a.work_date == today)
                .take(5)
            {
                let department = self
                    .data
                    .employees
                    .iter()
                    .find(|e| e.id == record.employee_id)
                    .map(|e| e.department.as_str())
                    .unwrap_or("—");
                let time = record
                    .checked_in_at
                    .with_timezone(&self.data.company.tz())
                    .format(self.time_format())
                    .to_string();
                attendance_row(
                    ui,
                    [
                        self.data.employee_name(record.employee_id),
                        department,
                        &time,
                        "",
                    ],
                    false,
                    record.checked_out_at.is_some(),
                );
            }
            if attendance == 0 {
                empty(ui, "No check-ins yet today.");
            }
        });
    }
    pub(super) fn overview_leave_card(&mut self, ui: &mut Ui, pending: usize) {
        card(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new("Pending leave").font(semibold(17.0)));
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if ui.button("Open leave").clicked() {
                        self.page = Page::Leave;
                        self.leave_filter = Some(LeaveStatus::Pending);
                    }
                });
            });
            ui.label(
                RichText::new("Requests in your review scope")
                    .size(12.0)
                    .color(muted(ui)),
            );
            ui.add_space(10.0);
            for row in self
                .data
                .leave
                .iter()
                .filter(|r| r.status == LeaveStatus::Pending)
                .take(3)
            {
                ui.separator();
                ui.label(
                    RichText::new(self.data.employee_name(row.employee_id)).font(semibold(14.0)),
                );
                ui.label(
                    RichText::new(format!(
                        "{} · {} to {}",
                        row.type_name(),
                        row.starts_on,
                        row.ends_on
                    ))
                    .size(12.0)
                    .color(muted(ui)),
                );
                let units = row.units();
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(format!("{} workday(s)", crate::leave::days(units)))
                            .size(12.0)
                            .color(muted(ui)),
                    );
                    badge(ui, "Pending", false);
                });
            }
            if pending == 0 {
                empty(ui, "You're all caught up. No pending requests.");
            }
        });
    }
}
