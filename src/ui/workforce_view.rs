use super::*;
use crate::{reports::Report, workforce::Analysis};

pub(super) fn charts(ui: &mut Ui, report: &Report) {
    let Some(analysis) = &report.analysis else {
        return;
    };
    ui.add_space(10.0);
    card(ui, |ui| match analysis {
        Analysis::Headcount { as_of, groups } => {
            let anchor = ui.strong("Current workforce snapshot");
            #[cfg(feature = "qa-screenshot")]
            if std::env::var("HRIS_PREVIEW_SCROLL").as_deref() == Ok("analysis") {
                anchor.scroll_to_me(Some(Align::Min));
            }
            let _ = anchor;
            let active: usize = groups.iter().map(|g| g.active).sum();
            let archived: usize = groups.iter().map(|g| g.archived).sum();
            ui.horizontal_wrapped(|ui| {
                badge(ui, &format!("{active} active employee records"), true);
                ui.label(format!(
                    "{archived} archived · {} groups · {as_of}",
                    groups.len()
                ));
            });
            ui.add_space(8.0);
            let mut ranked: Vec<_> = groups.iter().collect();
            ranked.sort_by(|a, b| b.active.cmp(&a.active).then(a.name.cmp(&b.name)));
            let largest = ranked.iter().map(|g| g.active).max().unwrap_or(1).max(1) as f32;
            for group in ranked.iter().take(8) {
                ui.horizontal(|ui| {
                    let label_width = (ui.available_width() * 0.35).min(200.0);
                    ui.add_sized(
                        [label_width, 30.0],
                        egui::Label::new(&group.name).truncate(),
                    )
                    .on_hover_text(&group.name);
                    ui.add_sized(
                        [ui.available_width(), 24.0],
                        egui::ProgressBar::new(group.active as f32 / largest).text(format!(
                            "{} active · {} archived",
                            group.active, group.archived
                        )),
                    );
                });
            }
            ui.label(RichText::new(if groups.len()>8 {"Chart shows the eight largest active groups; the table and CSV include every group."}else{"Bars compare active records; archived counts are shown separately."}).small().color(muted(ui)));
        }
        Analysis::Trends(points) => {
            let anchor = ui.strong("Recorded attendance and leave");
            #[cfg(feature = "qa-screenshot")]
            if std::env::var("HRIS_PREVIEW_SCROLL").as_deref() == Ok("analysis") {
                anchor.scroll_to_me(Some(Align::Min));
            }
            let _ = anchor;
            let recorded: i64 = points.iter().map(|p| p.records).sum();
            let approved: i32 = points.iter().map(|p| p.approved_units).sum();
            let pending: i32 = points.iter().map(|p| p.pending_units).sum();
            ui.horizontal_wrapped(|ui| {
                badge(ui, &format!("{recorded} attendance employee-days"), true);
                ui.label(format!(
                    "{} approved leave days · {} pending leave days",
                    crate::leave::days(approved),
                    crate::leave::days(pending)
                ));
            });
            let colors = [
                TEAL,
                Color32::from_rgb(213, 126, 35),
                Color32::from_rgb(150, 107, 209),
            ];
            ui.horizontal_wrapped(|ui| {
                for (label, color) in [
                    "Attendance employee-days",
                    "Approved leave days",
                    "Pending leave days",
                ]
                .into_iter()
                .zip(colors)
                {
                    ui.colored_label(color, label);
                }
            });
            let (rect, response) = ui
                .allocate_exact_size(Vec2::new(ui.available_width(), 190.0), egui::Sense::hover());
            let plot = rect.shrink2(Vec2::new(12.0, 20.0));
            let maximum = points
                .iter()
                .flat_map(|p| {
                    [
                        p.records as f32,
                        p.approved_units as f32 / 2.0,
                        p.pending_units as f32 / 2.0,
                    ]
                })
                .fold(1.0, f32::max);
            let x = |i: usize| {
                if points.len() < 2 {
                    plot.center().x
                } else {
                    plot.left() + plot.width() * i as f32 / (points.len() - 1) as f32
                }
            };
            for k in 0..=4 {
                let y = plot.bottom() - plot.height() * k as f32 / 4.0;
                ui.painter().line_segment(
                    [egui::pos2(plot.left(), y), egui::pos2(plot.right(), y)],
                    Stroke::new(1.0, ui.visuals().widgets.noninteractive.bg_stroke.color),
                );
            }
            for (series, color) in colors.into_iter().enumerate() {
                let line: Vec<_> = points
                    .iter()
                    .enumerate()
                    .map(|(i, p)| {
                        let value = match series {
                            0 => p.records as f32,
                            1 => p.approved_units as f32 / 2.0,
                            _ => p.pending_units as f32 / 2.0,
                        };
                        egui::pos2(x(i), plot.bottom() - plot.height() * value / maximum)
                    })
                    .collect();
                if line.len() == 1 {
                    ui.painter().circle_filled(line[0], 3.0, color);
                } else {
                    ui.painter()
                        .add(egui::Shape::line(line, Stroke::new(2.0, color)));
                }
            }
            if let (Some(first), Some(last)) = (points.first(), points.last()) {
                ui.painter().text(
                    rect.left_bottom(),
                    egui::Align2::LEFT_BOTTOM,
                    first.start.to_string(),
                    FontId::proportional(11.0),
                    muted(ui),
                );
                ui.painter().text(
                    rect.right_bottom(),
                    egui::Align2::RIGHT_BOTTOM,
                    last.end.to_string(),
                    FontId::proportional(11.0),
                    muted(ui),
                );
                ui.painter().text(
                    rect.left_top(),
                    egui::Align2::LEFT_TOP,
                    format!("Scale 0–{maximum:.1} days"),
                    FontId::proportional(11.0),
                    muted(ui),
                );
            }
            if let Some(pointer) = response.hover_pos() {
                let index = (((pointer.x - plot.left()) / plot.width()).clamp(0.0, 1.0)
                    * points.len().saturating_sub(1) as f32)
                    .round() as usize;
                if let Some(point) = points.get(index) {
                    response.on_hover_ui(|ui| {
                        ui.label(format!("{} to {}", point.start, point.end));
                        ui.label(format!(
                            "{} recorded · {} completed · {} open",
                            point.records, point.completed, point.open
                        ));
                        ui.label(format!(
                            "{} approved leave days · {} pending leave days",
                            crate::leave::days(point.approved_units),
                            crate::leave::days(point.pending_units)
                        ));
                    });
                }
            }
            ui.label(RichText::new("One employee with a record on one work date counts as one attendance day. Recorded and leave days may overlap; these are counts, not attendance rates or inferred absences. Hover a bucket for details.").small().color(muted(ui)));
        }
    });
}
