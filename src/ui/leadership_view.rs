use super::*;
use crate::leadership::{self, Filter, GroupBy, Metric, Report};
pub(super) struct State {
    pub open: bool,
    pub filter: Filter,
    pub report: Option<Report>,
    pub start_month: NaiveDate,
    pub end_month: NaiveDate,
    pub group: GroupBy,
}
impl State {
    pub fn new(today: NaiveDate) -> Self {
        Self {
            open: false,
            filter: Filter::new(today),
            report: None,
            start_month: month_start(today),
            end_month: month_start(today),
            group: GroupBy::Department,
        }
    }
    pub fn accept(&mut self, report: Report, data: &Snapshot) -> anyhow::Result<()> {
        report.validate(&data.access, &self.filter)?;
        self.report = Some(report);
        Ok(())
    }
}
fn date(
    ui: &mut Ui,
    id: &str,
    label: &str,
    date: &mut NaiveDate,
    month: &mut NaiveDate,
    data: &Snapshot,
) {
    ui.push_id(id, |ui| {
        ui.label(label);
        let button = ui.button(date.to_string());
        egui::Popup::from_toggle_button_response(&button).show(|ui| {
            if calendar_control(
                ui,
                date,
                month,
                data.company.today(),
                data.company.today(),
                data,
            ) {
                ui.close();
            }
        });
    });
}
fn grid_row(ui: &mut Ui, label: &str, value: impl ToString) {
    let width = (ui.available_width() - 115.0).clamp(160.0, 650.0);
    ui.vertical(|ui| {
        ui.set_width(width);
        ui.add(egui::Label::new(label).wrap());
    });
    ui.label(RichText::new(value.to_string()).strong());
    ui.end_row();
}
fn cell(ui: &mut Ui, text: impl ToString, width: f32) {
    ui.vertical(|ui| {
        ui.set_width(width);
        ui.add(egui::Label::new(text.to_string()).wrap());
    });
}
impl HrisApp {
    pub(super) fn leadership(&mut self, ui: &mut Ui) {
        if self.desktop.product != crate::desktop::Product::Hris
            || !leadership::eligible(&self.data.access)
        {
            self.leadership_state.report = None;
            self.leadership_state.open = false;
            return;
        }
        let state = &mut self.leadership_state;
        let before = state.filter;
        card(ui, |ui| {
            ui.heading("HR leadership");
            ui.label("Administrator oversight from recorded facts. Current workforce and workload are separate from selected-period observations.");
            ui.add_space(8.0);
            ui.add_enabled_ui(!self.busy, |ui| {
                ui.horizontal_wrapped(|ui| {
                    date(
                        ui,
                        "leadership-start",
                        "From",
                        &mut state.filter.starts_on,
                        &mut state.start_month,
                        &self.data,
                    );
                    date(
                        ui,
                        "leadership-end",
                        "To",
                        &mut state.filter.ends_on,
                        &mut state.end_month,
                        &self.data,
                    );
                    if ui.button("This month").clicked() {
                        state.filter = Filter::new(self.data.company.today());
                    }
                    if ui.button("Last 30 days").clicked() {
                        state.filter = Filter {
                            starts_on: self.data.company.today() - Duration::days(29),
                            ends_on: self.data.company.today(),
                        };
                    }
                });
            });
            if state.filter != before {
                state.report = None;
                state.start_month = month_start(state.filter.starts_on);
                state.end_month = month_start(state.filter.ends_on);
            }
        });
        ui.add_space(10.0);
        let mut requested = None;
        let mut error = None;
        card(ui, |ui| {
            let state = &self.leadership_state;
            let valid = state.filter.validate(self.data.company.today());
            if let Err(e) = &valid {
                ui.colored_label(Color32::from_rgb(183, 67, 63), e.to_string());
            }
            let enabled = !self.busy && valid.is_ok() && self.data.mode != Mode::Demo;
            ui.horizontal_wrapped(|ui| {
                if ui
                    .add_enabled(enabled, egui::Button::new("Generate leadership report"))
                    .clicked()
                {
                    requested = Some(Command::Leadership {
                        filter: state.filter,
                        destination: None,
                    });
                }
                for ext in ["csv", "xlsx"] {
                    if ui
                        .add_enabled(
                            enabled && state.report.is_some(),
                            egui::Button::new(if ext == "csv" {
                                "Export CSV"
                            } else {
                                "Export Excel"
                            }),
                        )
                        .clicked()
                    {
                        match crate::reports::choose_export(&state.filter.filename(ext)) {
                            Ok(Some(path)) => {
                                requested = Some(Command::Leadership {
                                    filter: state.filter,
                                    destination: Some(path),
                                })
                            }
                            Ok(None) => {}
                            Err(e) => error = Some(e.to_string()),
                        }
                    }
                }
            });
            ui.small("Generation and export read fresh protected totals. Exports save new local files. No salary, confidential case, service-request or benefits details are included.");
            if self.data.mode == Mode::Demo {
                ui.small("Company reports require sign-in. Any leadership sample in this fictional demo is illustrative.");
            }
        });
        if let Some(c) = requested {
            self.leadership_state.report = None;
            self.send(c);
        }
        if let Some(e) = error {
            self.notice = Some((e, true));
        }
        let Some(report) = &self.leadership_state.report else {
            ui.add_space(10.0);
            card(ui, |ui| {
                ui.label("Choose a period and generate the report. Up to 366 dates; end date cannot be after company today.");
            });
            return;
        };
        ui.add_space(10.0);
        card(ui, |ui| {
            ui.label(
                RichText::new(format!(
                    "{} · current snapshot {}",
                    report.company_name, report.today
                ))
                .strong(),
            );
            ui.small(format!(
                "Generated {} UTC · {} · selected {} to {}",
                report.generated_at.format("%Y-%m-%d %H:%M:%S"),
                report.time_zone,
                report.filter.starts_on,
                report.filter.ends_on
            ));
            let w = &report.workforce;
            ui.add_space(8.0);
            egui::Grid::new("leadership-workforce")
                .num_columns(2)
                .spacing([16.0, 8.0])
                .show(ui, |ui| {
                    for (label, n) in [
                        ("Active employee records", w.active),
                        ("Archived employee records", w.archived),
                        (
                            "Recorded joining dates in selected period",
                            w.joined_in_period,
                        ),
                        (
                            "Finalized exit cases in selected period",
                            w.finalized_exits_in_period,
                        ),
                    ] {
                        grid_row(ui, label, n);
                    }
                });
            ui.small("Current recorded status includes future joining dates. Joining dates are current recorded fields; exits use finalized cases and their last-working dates. Archiving alone is not a separation. Historical headcount and turnover are not reconstructed.");
        });
        ui.add_space(10.0);
        card(ui, |ui| {
            ui.label(RichText::new("Current workforce breakdown").strong());
            ui.horizontal_wrapped(|ui| {
                for by in GroupBy::ALL {
                    ui.selectable_value(&mut self.leadership_state.group, by, by.label());
                }
            });
            egui::Grid::new("leadership-groups")
                .striped(true)
                .spacing([16.0, 8.0])
                .show(ui, |ui| {
                    let width = (ui.available_width() - 160.0).clamp(140.0, 600.0);
                    cell(ui, self.leadership_state.group.label(), width);
                    cell(ui, "Active", 55.0);
                    cell(ui, "Archived", 65.0);
                    ui.end_row();
                    for g in report
                        .groups
                        .iter()
                        .filter(|g| g.by == self.leadership_state.group)
                    {
                        cell(ui, g.label(), width);
                        cell(ui, g.active, 55.0);
                        cell(ui, g.archived, 65.0);
                        ui.end_row();
                    }
                });
            ui.small("Current assignments and labels, including inactive option labels. Unassigned records stay visible.");
        });
        ui.add_space(10.0);
        card(ui, |ui| {
            ui.label(RichText::new("Selected-period observations").strong());
            ui.small("Saved work dates and leave charges, with current decision states. Half-day = 0.5 day. Missing attendance is not absence.");
            egui::ScrollArea::horizontal()
                .id_salt("leadership-month-scroll")
                .show(ui, |ui| {
                    egui::Grid::new("leadership-months")
                        .striped(true)
                        .spacing([16.0, 8.0])
                        .show(ui, |ui| {
                            for h in [
                                "Period",
                                "Joining dates",
                                "Finalized exits",
                                "Completed attendance",
                                "Open attendance",
                                "Approved leave (days)",
                                "Pending leave (days)",
                            ] {
                                cell(ui, h, if h == "Period" { 180.0 } else { 105.0 });
                            }
                            ui.end_row();
                            for m in &report.months {
                                cell(ui, format!("{} – {}", m.starts_on, m.ends_on), 180.0);
                                for n in [
                                    m.joined,
                                    m.finalized_exits,
                                    m.attendance_completed,
                                    m.attendance_open,
                                ] {
                                    cell(ui, n, 105.0);
                                }
                                cell(
                                    ui,
                                    format!("{:.1}", f64::from(m.approved_leave_units) / 2.0),
                                    105.0,
                                );
                                cell(
                                    ui,
                                    format!("{:.1}", f64::from(m.pending_leave_units) / 2.0),
                                    105.0,
                                );
                                ui.end_row();
                            }
                        });
                });
        });
        for section in ["Hiring & transitions", "Learning & Performance"] {
            ui.add_space(10.0);
            card(ui, |ui| {
                ui.label(RichText::new(section).strong());
                ui.small("Current saved workflow counts across retained records, including archived employment. Overdue uses company today; only the training completion count uses the selected period.");
                egui::Grid::new(("leadership-workload", section))
                    .num_columns(2)
                    .spacing([16.0, 8.0])
                    .show(ui, |ui| {
                        for m in Metric::ALL.into_iter().filter(|m| m.section() == section) {
                            grid_row(ui, m.label(), report.workload[&m]);
                        }
                    });
                if section == "Learning & Performance" {
                    ui.small("Completion is recorded by HR. Certificate expiry counts exclude archived/revoked and future-issued records. No scores, private notes or competency conclusions are shown.");
                }
            });
        }
        ui.add_space(10.0);
        card(ui, |ui| {
            ui.label(RichText::new("Record completeness").strong());
            let w = &report.workforce;
            egui::Grid::new("leadership-quality")
                .num_columns(2)
                .spacing([16.0, 8.0])
                .show(ui, |ui| {
                    for (label, n) in [
                        ("Unknown joining dates (all records)", w.unknown_join),
                        ("Future joining dates (all records)", w.future_join),
                        (
                            "Active records without department",
                            w.active_without_department,
                        ),
                        ("Active records without location", w.active_without_location),
                        (
                            "Active records without employment type",
                            w.active_without_type,
                        ),
                        (
                            "Active records without a reporting manager",
                            w.active_without_manager,
                        ),
                    ] {
                        grid_row(ui, label, n);
                    }
                });
            ui.small("A missing manager may be intentional. Unknown dates are not assigned estimated hire or separation dates. Exports retain metric definitions and time bases.");
        });
    }
    #[cfg(feature = "qa-screenshot")]
    pub(super) fn prepare_leadership_preview(&mut self) {
        let name = std::env::var("HRIS_PREVIEW_FORM").unwrap_or_default();
        if !name.starts_with("leadership-")
            || self.desktop.product != crate::desktop::Product::Hris
            || self.data.mode != Mode::Demo
            || !leadership::eligible(&self.data.access)
        {
            return;
        }
        if self.data.access.user_id.is_nil() {
            self.data.access.user_id = Uuid::from_u128(10);
        }
        let r = leadership::preview(&self.data);
        self.leadership_state = State::new(r.today);
        self.leadership_state.open = true;
        self.leadership_state.filter = r.filter;
        if name != "leadership-empty" {
            r.validate(&self.data.access, &r.filter).unwrap();
            self.leadership_state.report = Some(r);
        }
        if name == "leadership-location" {
            self.leadership_state.group = GroupBy::Location;
        }
        self.page = Page::Reports;
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn changed_binding_or_filter_cannot_retain_cached_totals() {
        let mut d = crate::domain::DemoStore::new().data;
        if d.access.user_id.is_nil() {
            d.access.user_id = Uuid::new_v4();
        }
        let r = leadership::preview(&d);
        let mut s = State::new(r.today);
        s.filter = r.filter;
        s.accept(r.clone(), &d).unwrap();
        let mut foreign = d.clone();
        foreign.access.user_id = Uuid::new_v4();
        assert!(s.accept(r.clone(), &foreign).is_err());
        s.filter.starts_on += Duration::days(1);
        assert!(s.accept(r, &d).is_err());
    }
}
