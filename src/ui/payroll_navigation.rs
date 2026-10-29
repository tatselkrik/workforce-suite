//! Compact financial navigation for the separate app; pages remain shared implementations.
use super::*;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Choice {
    Section(usize),
    Settings,
}
pub(super) fn show(
    ui: &mut Ui,
    page: Page,
    tab: usize,
    height: f32,
    compact: bool,
    follow: bool,
) -> (Option<Choice>, Option<egui::Rect>, egui::Rect) {
    let row = if compact { 26.0 } else { 30.0 };
    let selected = if page == Page::Settings {
        crate::desktop::PAYROLL_TABS.len()
    } else {
        tab.min(crate::desktop::PAYROLL_TABS.len() - 1)
    };
    let offset = (22.0 + selected as f32 * (row + 2.0) - height * 0.4).max(0.0);
    let scroll = egui::ScrollArea::vertical()
        .id_salt("payroll_sections")
        .max_height(height)
        .auto_shrink([false, false]);
    let scroll = if follow {
        scroll.vertical_scroll_offset(offset)
    } else {
        scroll
    };
    let output = scroll.show(ui, |ui| {
        ui.spacing_mut().item_spacing.y = 2.0;
        ui.spacing_mut().interact_size.y = row;
        ui.spacing_mut().button_padding = Vec2::new(8.0, 4.0);
        let mut choice = None;
        let mut selected_rect = None;
        ui.label(
            RichText::new("PAYROLL")
                .size(10.0)
                .color(Color32::from_rgb(140, 160, 179))
                .strong(),
        );
        for (i, label) in crate::desktop::PAYROLL_TABS.iter().enumerate() {
            let active = page == Page::Payroll && tab == i;
            let label = if i == 3 { "Payslips" } else { *label };
            let r = ui.add_sized(
                [ui.available_width(), row],
                egui::Button::new(RichText::new(label).size(13.0).color(if active {
                    Color32::WHITE
                } else {
                    Color32::from_rgb(175, 193, 207)
                }))
                .fill(if active { TEAL } else { Color32::TRANSPARENT })
                .stroke(Stroke::NONE)
                .corner_radius(8),
            );
            if active {
                selected_rect = Some(r.rect);
            }
            if r.clicked() {
                choice = Some(Choice::Section(i));
            }
        }
        ui.add_space(8.0);
        ui.label(
            RichText::new("PREFERENCES")
                .size(10.0)
                .color(Color32::from_rgb(140, 160, 179))
                .strong(),
        );
        let active = page == Page::Settings;
        let r = ui.add_sized(
            [ui.available_width(), row],
            egui::Button::new(RichText::new("Settings").size(13.0).color(if active {
                Color32::WHITE
            } else {
                Color32::from_rgb(175, 193, 207)
            }))
            .fill(if active { TEAL } else { Color32::TRANSPARENT })
            .stroke(Stroke::NONE)
            .corner_radius(8),
        );
        if active {
            selected_rect = Some(r.rect);
        }
        if r.clicked() {
            choice = Some(Choice::Settings);
        }
        (choice, selected_rect)
    });
    (output.inner.0, output.inner.1, output.inner_rect)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn selected_payroll_section_and_settings_stay_visible_in_short_and_tall_sidebars() {
        assert!(!crate::desktop::PAYROLL_TABS.contains(&"Data exchange"));
        for dark in [false, true] {
            for height in [90.0, 180.0, 420.0, 600.0] {
                for tab in 0..=crate::desktop::PAYROLL_TABS.len() {
                    let ctx = egui::Context::default();
                    configure_theme(&ctx);
                    ctx.set_theme(if dark {
                        egui::Theme::Dark
                    } else {
                        egui::Theme::Light
                    });
                    for _ in 0..3 {
                        let mut out = ctx.run_ui(
                            egui::RawInput {
                                screen_rect: Some(egui::Rect::from_min_size(
                                    egui::Pos2::ZERO,
                                    Vec2::new(220.0, 720.0),
                                )),
                                ..Default::default()
                            },
                            |ui| {
                                let (_, rect, viewport) = show(
                                    ui,
                                    if tab == crate::desktop::PAYROLL_TABS.len() {
                                        Page::Settings
                                    } else {
                                        Page::Payroll
                                    },
                                    tab,
                                    height,
                                    true,
                                    true,
                                );
                                let rect = rect.unwrap();
                                assert!(
                                    rect.top() >= viewport.top() - 1.0
                                        && rect.bottom() <= viewport.bottom() + 1.0,
                                    "Selected tab {tab} outside {height}: {rect:?} / {viewport:?}"
                                );
                                assert!(rect.right() <= viewport.right() + 1.0);
                            },
                        );
                        out.textures_delta.clear();
                    }
                }
            }
        }
    }
}
