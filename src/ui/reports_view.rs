use super::*;
use crate::report_views::{self, Period, View};
use crate::reports::{self, Filter, Kind, Report};

pub(super) struct State {
    pub filter: Filter,
    pub result: Option<Report>,
    pub start_month: NaiveDate,
    pub end_month: NaiveDate,
    pub page: usize,
    pub selected_view: Option<Uuid>,
    pub view_draft: Option<View>,
    pub pending_views: Option<Vec<View>>,
    pub views_requested: bool,
}
impl State {
    pub fn new(today: NaiveDate) -> Self {
        Self {
            filter: Filter::new(today),
            result: None,
            start_month: month_start(today),
            end_month: month_start(today),
            page: 0,
            selected_view: None,
            view_draft: None,
            pending_views: None,
            views_requested: false,
        }
    }
    pub fn accept_account(&mut self, views: &[View]) {
        if self.pending_views.as_deref() == Some(views) {
            self.view_draft = None;
            self.pending_views = None;
            if !views.iter().any(|v| Some(v.id) == self.selected_view) {
                self.selected_view = None;
            }
        }
    }
    fn apply(&mut self, filter: Filter) {
        self.start_month = month_start(filter.starts_on);
        self.end_month = month_start(filter.ends_on);
        self.filter = filter;
        self.result = None;
        self.page = 0;
    }
}
fn date_filter(
    ui: &mut Ui,
    id: &str,
    label: &str,
    date: &mut NaiveDate,
    month: &mut NaiveDate,
    data: &Snapshot,
) {
    ui.push_id(id, |ui| {
        ui.label(label);
        let selected = ui.button(date.to_string());
        egui::Popup::from_toggle_button_response(&selected).show(|ui| {
            if calendar_control(
                ui,
                date,
                month,
                data.company.today(),
                data.company.today() + Duration::days(366),
                data,
            ) {
                ui.close();
            }
        });
    });
}
fn report_cell(ui: &mut Ui, text: &str, strong: bool, height: f32) -> egui::Rect {
    ui.vertical(|ui| {
        ui.set_width(180.0);
        ui.set_min_height(height);
        let text = if strong {
            RichText::new(text).strong()
        } else {
            RichText::new(text)
        };
        ui.add(egui::Label::new(text).wrap());
    })
    .response
    .rect
}
impl HrisApp {
    fn save_report_views(&mut self, views: Vec<View>) {
        match report_views::validate_collection(&views) {
            Ok(()) => {
                self.report_state.pending_views = Some(views.clone());
                let revision = self.account.as_ref().map_or(0, |a| a.report_views_revision);
                self.send(Command::Account(AccountAction::SaveReportViews {
                    company_id: self.data.company.id,
                    expected_revision: revision,
                    views,
                }));
            }
            Err(e) => self.notice = Some((e.to_string(), true)),
        }
    }

    fn saved_report_views(&mut self, ui: &mut Ui) {
        let loaded = self.data.mode == Mode::Demo
            || self
                .account
                .as_ref()
                .is_some_and(|a| a.report_views_company == Some(self.data.company.id));
        if !loaded {
            if !self.report_state.views_requested && !self.busy {
                self.report_state.views_requested = true;
                self.send(Command::Account(AccountAction::LoadReportViews {
                    company_id: self.data.company.id,
                }));
            }
            card(ui, |ui| {
                ui.label("Load your saved report views.");
                if ui
                    .add_enabled(!self.busy, egui::Button::new("Load saved views"))
                    .clicked()
                {
                    self.send(Command::Account(AccountAction::LoadReportViews {
                        company_id: self.data.company.id,
                    }));
                }
            });
            return;
        }
        let views = self
            .account
            .as_ref()
            .map(|a| a.report_views.clone())
            .unwrap_or_default();
        let selected = views
            .iter()
            .find(|v| {
                Some(v.id) == self.report_state.selected_view
                    && v.company_id == self.data.company.id
            })
            .cloned();
        card(ui, |ui| {
            ui.label(RichText::new("Your saved views").strong());
            ui.add_space(6.0);
            ui.add_enabled_ui(!self.busy, |ui| {
                ui.horizontal_wrapped(|ui| {
                    egui::ComboBox::from_id_salt("saved-report-view")
                        .width(240.0).truncate()
                        .selected_text(selected.as_ref().map(|v| v.name.as_str()).unwrap_or("Choose a saved view"))
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut self.report_state.selected_view, None, "Choose a saved view");
                            for view in views.iter().filter(|v| v.company_id == self.data.company.id) {
                                ui.selectable_value(&mut self.report_state.selected_view, Some(view.id), &view.name);
                            }
                        });
                    if ui.add_enabled(selected.is_some(), egui::Button::new("Apply view")).clicked() {
                        match selected.as_ref().unwrap().resolve(&self.data, self.data.company.today()) {
                            Ok(filter) => {
                                self.report_state.apply(filter);
                                self.notice = Some(("Saved filters applied. Run report to refresh the results.".into(), false));
                            }
                            Err(e) => self.notice = Some((e.to_string(), true)),
                        }
                    }
                    if ui.add_enabled(self.report_state.view_draft.is_none() && views.len() < report_views::MAX_VIEWS,
                        egui::Button::new("Save current filters…")).clicked() {
                        self.report_state.view_draft = Some(View {
                            id: Uuid::new_v4(), company_id: self.data.company.id, name: String::new(),
                            period: Period::Fixed, filter: self.report_state.filter.clone(),
                        });
                    }
                    if ui.add_enabled(selected.is_some() && self.report_state.view_draft.is_none(), egui::Button::new("Edit view")).clicked() {
                        self.report_state.view_draft = selected.clone();
                    }
                    if ui.add_enabled(selected.is_some() && self.report_state.view_draft.is_none(), egui::Button::new("Delete view")).clicked() {
                        let id = selected.as_ref().unwrap().id;
                        self.save_report_views(views.iter().filter(|v| v.id != id).cloned().collect());
                    }
                    if ui.button("Reload views").clicked(){self.send(Command::Account(AccountAction::LoadReportViews{company_id:self.data.company.id}));}
                });
                if let Some(mut draft) = self.report_state.view_draft.take() {
                    ui.add_space(8.0);
                    egui::Grid::new("saved-report-editor").num_columns(2).spacing(Vec2::new(12.0,8.0)).show(ui, |ui| {
                        ui.label("View name");
                        ui.add(egui::TextEdit::singleline(&mut draft.name).char_limit(80).desired_width(240.0).hint_text("e.g. Monthly team attendance"));
                        ui.end_row();
                        ui.label("Period");
                        if draft.filter.kind==Kind::Headcount {
                            draft.period=Period::Fixed;
                            ui.label("Current snapshot at generation time");
                        } else {
                        egui::ComboBox::from_id_salt("saved-report-period").width(240.0).selected_text(draft.period.label())
                            .show_ui(ui, |ui| {
                                for period in Period::ALL { ui.selectable_value(&mut draft.period, period, period.label()); }
                            });
                        }
                        ui.end_row();
                    });
                    ui.horizontal_wrapped(|ui| {
                        let dates = draft.resolve(&self.data, self.data.company.today()).unwrap_or_else(|_| draft.filter.clone());
                        ui.label(format!("{} · {} to {} · {}", draft.filter.kind.label(), dates.starts_on, dates.ends_on,
                            draft.filter.employee_id.map(|id| self.data.employee_name(id)).unwrap_or("All permitted employees")));
                        if ui.button("Use current report filters").clicked() { draft.filter = self.report_state.filter.clone(); }
                    });
                    if draft.period != Period::Fixed {
                        ui.label(RichText::new("Rolling dates are calculated when you apply the view, using the company time zone.").small().color(muted(ui)));
                    }
                    let mut next = views.clone();
                    if let Some(old) = next.iter_mut().find(|v| v.id == draft.id) { *old = draft.clone(); }
                    else { next.push(draft.clone()); }
                    let valid = report_views::validate_collection(&next).and_then(|_| draft.resolve(&self.data, self.data.company.today()).map(|_| ()));
                    if let Err(e) = &valid { ui.colored_label(ui.visuals().error_fg_color, e.to_string()); }
                    let mut close = false;
                    ui.horizontal(|ui| {
                        if ui.add_enabled(valid.is_ok(), egui::Button::new("Save view")).clicked() {
                            self.report_state.selected_view = Some(draft.id);
                            self.save_report_views(next);
                        }
                        close = ui.button("Cancel").clicked();
                    });
                    if !close { self.report_state.view_draft = Some(draft); }
                }
            });
            ui.label(RichText::new(format!("Private to your account · {} of {} views. Filters only; reports always use current access and fresh records.", views.len(), report_views::MAX_VIEWS)).small().color(muted(ui)));
            if self.data.mode == Mode::Demo {
                ui.label(
                    RichText::new("Demo views last only for this session.")
                        .small()
                        .color(muted(ui)),
                );
            }
        });
    }

    pub(super) fn reports(&mut self, ui: &mut Ui) {
        let before = self.report_state.filter.clone();
        ui.horizontal_wrapped(|ui| {
            for kind in Kind::ALL {
                if ui
                    .selectable_label(
                        !self.import_state.open
                            && !self.attendance_import_state.open
                            && !self.leave_request_import_state.open
                            && !self.leadership_state.open
                            && self.report_state.filter.kind == kind,
                        kind.label(),
                    )
                    .clicked()
                {
                    if self.report_state.filter.kind != kind {
                        self.report_state.filter.columns = None;
                    }
                    self.leadership_state.open=false;
                    self.report_state.filter.kind = kind;
                    if kind == Kind::Headcount {
                        self.report_state.filter.starts_on = self.data.company.today();
                        self.report_state.filter.ends_on = self.data.company.today();
                    }
                    self.import_state.open = false;
                    self.attendance_import_state.open = false;
                    self.leave_request_import_state.open = false;
                }
            }
        });
        if crate::leadership::eligible(&self.data.access) {
            if ui.selectable_label(self.leadership_state.open,"Leadership").clicked() {
                self.leadership_state.open=true;self.import_state.open=false;self.attendance_import_state.open=false;self.leave_request_import_state.open=false;
            }
        } else { self.leadership_state.open=false;self.leadership_state.report=None; }
        ui.horizontal_wrapped(|ui| {
            if self
                .data
                .access
                .allows(crate::staff_roles::Capability::GeneralHrReports)
                && ui
                    .selectable_label(self.import_state.open, "Employee import")
                    .clicked()
            {
                self.leadership_state.open=false;
                self.import_state.open = true;
                self.attendance_import_state.open = false;
                self.leave_request_import_state.open = false;
            }
            if self
                .data
                .access
                .allows(crate::staff_roles::Capability::GeneralHrReports)
                && ui
                    .selectable_label(self.attendance_import_state.open, "Attendance import")
                    .clicked()
            {
                self.import_state.open = false;
                self.leadership_state.open=false;
                self.attendance_import_state.open = true;
                self.leave_request_import_state.open = false;
            }
            if self
                .data
                .access
                .allows(crate::staff_roles::Capability::GeneralHrReports)
                && ui
                    .selectable_label(self.leave_request_import_state.open, "Leave import")
                    .clicked()
            {
                self.import_state.open = false;
                self.attendance_import_state.open = false;
                self.leadership_state.open=false;
                self.leave_request_import_state.open = true;
            }
        });
        if self.leadership_state.open { self.leadership(ui);return; }
        if self.leave_request_import_state.open {
            self.leave_request_import(ui);
            return;
        }
        if self.attendance_import_state.open {
            self.attendance_import(ui);
            return;
        }
        if self.import_state.open
            && self
                .data
                .access
                .allows(crate::staff_roles::Capability::GeneralHrReports)
        {
            self.employee_import(ui);
            return;
        }
        ui.add_space(10.0);
        self.saved_report_views(ui);
        ui.add_space(10.0);
        card(ui, |ui| {
            if self.report_state.filter.kind == Kind::Headcount {
                ui.horizontal_wrapped(|ui| {
                    ui.label("Group current employee records by");
                    for group in crate::workforce::GroupBy::ALL {
                        ui.selectable_value(
                            &mut self.report_state.filter.headcount_by,
                            group,
                            group.label(),
                        );
                    }
                });
                ui.label(RichText::new("Current employment snapshot at generation time. Archived records are shown separately; past headcount is not reconstructed.").small().color(muted(ui)));
            }
            if self.report_state.filter.kind == Kind::Trends {
                ui.horizontal_wrapped(|ui| {
                    ui.label("Trend buckets");
                    for bucket in crate::workforce::Granularity::ALL {
                        ui.selectable_value(
                            &mut self.report_state.filter.trend_bucket,
                            bucket,
                            bucket.label(),
                        );
                    }
                });
            }
            if self.report_state.filter.kind != Kind::Headcount {
                ui.horizontal_wrapped(|ui| {
                    date_filter(
                        ui,
                        "report-from",
                        "From",
                        &mut self.report_state.filter.starts_on,
                        &mut self.report_state.start_month,
                        &self.data,
                    );
                    date_filter(
                        ui,
                        "report-to",
                        "To",
                        &mut self.report_state.filter.ends_on,
                        &mut self.report_state.end_month,
                        &self.data,
                    );
                    if ui.button("This month").clicked() {
                        let today = self.data.company.today();
                        self.report_state.filter.starts_on = month_start(today);
                        self.report_state.filter.ends_on = today;
                        self.report_state.start_month = month_start(today);
                        self.report_state.end_month = month_start(today);
                    }
                    if ui.button("Last 30 days").clicked() {
                        let today = self.data.company.today();
                        self.report_state.filter.starts_on = today - Duration::days(29);
                        self.report_state.filter.ends_on = today;
                        self.report_state.start_month =
                            month_start(self.report_state.filter.starts_on);
                        self.report_state.end_month = month_start(today);
                    }
                });
            }
            let mut selection = self.report_state.filter.clone();
            selection.employee_id = None;
            selection.department.clear();
            selection.include_archived = true;
            let choices = reports::employees(&self.data, &selection);
            ui.add_space(8.0);
            ui.horizontal_wrapped(|ui| {
                ui.label("Employee");
                egui::ComboBox::from_id_salt("report-employee")
                    .width(240.0)
                    .truncate()
                    .selected_text(
                        self.report_state
                            .filter
                            .employee_id
                            .map(|id| self.data.employee_name(id))
                            .unwrap_or("All permitted employees"),
                    )
                    .show_ui(ui, |ui| {
                        ui.selectable_value(
                            &mut self.report_state.filter.employee_id,
                            None,
                            "All permitted employees",
                        );
                        for e in &choices {
                            ui.selectable_value(
                                &mut self.report_state.filter.employee_id,
                                Some(e.id),
                                format!("{} · {}", e.employee_code, e.full_name),
                            );
                        }
                    });
                ui.label("Department");
                let mut departments: Vec<_> = choices
                    .iter()
                    .map(|e| e.department.clone())
                    .filter(|s| !s.is_empty())
                    .collect();
                departments.sort();
                departments.dedup();
                egui::ComboBox::from_id_salt("report-department")
                    .width(200.0)
                    .truncate()
                    .selected_text(if self.report_state.filter.department.is_empty() {
                        "All departments"
                    } else {
                        &self.report_state.filter.department
                    })
                    .show_ui(ui, |ui| {
                        ui.selectable_value(
                            &mut self.report_state.filter.department,
                            String::new(),
                            "All departments",
                        );
                        for name in departments {
                            ui.selectable_value(
                                &mut self.report_state.filter.department,
                                name.clone(),
                                name,
                            );
                        }
                    });
            });
            ui.add_space(8.0);
            ui.horizontal_wrapped(|ui| {
                ui.checkbox(
                    &mut self.report_state.filter.include_archived,
                    "Include archived employees' records",
                );
                if matches!(
                    self.report_state.filter.kind,
                    Kind::Attendance | Kind::Summary
                ) {
                    ui.checkbox(
                        &mut self.report_state.filter.include_missing,
                        "Include days without check-in",
                    );
                }
                if self.report_state.filter.kind == Kind::Leave {
                    ui.label("Status");
                    egui::ComboBox::from_id_salt("report-leave-status")
                        .selected_text(
                            self.report_state
                                .filter
                                .leave_status
                                .map(|s| s.label())
                                .unwrap_or("All statuses"),
                        )
                        .show_ui(ui, |ui| {
                            ui.selectable_value(
                                &mut self.report_state.filter.leave_status,
                                None,
                                "All statuses",
                            );
                            for status in [
                                LeaveStatus::Pending,
                                LeaveStatus::Approved,
                                LeaveStatus::Rejected,
                                LeaveStatus::Cancelled,
                            ] {
                                ui.selectable_value(
                                    &mut self.report_state.filter.leave_status,
                                    Some(status),
                                    status.label(),
                                );
                            }
                        });
                }
            });
            ui.add_space(10.0);
            report_columns::chooser(ui, &mut self.report_state.filter);
            ui.add_space(8.0);
            match self.report_state.filter.validate() {
                Err(e) => {
                    ui.colored_label(ui.visuals().error_fg_color, e.to_string());
                }
                Ok(()) => {
                    ui.horizontal_wrapped(|ui| {
                        if primary(ui, "Run report") {
                            self.send(Command::Report {
                                filter: self.report_state.filter.clone(),
                                destination: None,
                            });
                        }
                        if ui
                            .add_enabled(
                                self.report_state.result.is_some()
                                    && before == self.report_state.filter,
                                egui::Button::new("Export CSV"),
                            )
                            .clicked()
                        {
                            match reports::choose_export(&self.report_state.filter.filename()) {
                                Ok(Some(path)) => self.send(Command::Report {
                                    filter: self.report_state.filter.clone(),
                                    destination: Some(path),
                                }),
                                Err(e) => self.notice = Some((e.to_string(), true)),
                                _ => {}
                            }
                        }
                        if ui
                            .add_enabled(
                                self.report_state.result.is_some()
                                    && before == self.report_state.filter,
                                egui::Button::new("Export Excel"),
                            )
                            .clicked()
                        {
                            let name = format!(
                                "{}.xlsx",
                                self.report_state.filter.filename().trim_end_matches(".csv")
                            );
                            match reports::choose_export(&name) {
                                Ok(Some(path)) => self.send(Command::Report {
                                    filter: self.report_state.filter.clone(),
                                    destination: Some(path),
                                }),
                                Err(e) => self.notice = Some((e.to_string(), true)),
                                _ => {}
                            }
                        }
                    });
                }
            }
            ui.label(RichText::new("CSV and Excel exports generate a fresh report, recheck current access and save to a new file.").small().color(muted(ui)));
        });
        if before != self.report_state.filter {
            self.report_state.result = None;
            self.report_state.page = 0;
        }
        ui.add_space(12.0);
        let Some(report) = self.report_state.result.as_ref() else {
            empty(ui, "Choose the period and filters, then Run report.");
            return;
        };
        ui.horizontal_wrapped(|ui| {
            badge(ui, &format!("{} rows", report.rows.len()), true);
            if report.filter.kind == Kind::Headcount {
                ui.label(format!(
                    "{} permitted employee records · current snapshot",
                    report.employees
                ));
            } else {
                ui.label(format!(
                    "{} employee(s) · {} to {}",
                    report.employees, report.filter.starts_on, report.filter.ends_on
                ));
            }
        });
        ui.label(
            RichText::new(format!(
                "Generated {} · Company time zone {}",
                report
                    .generated_at
                    .with_timezone(&self.data.company.tz())
                    .format("%b %d, %Y %H:%M:%S"),
                report.timezone
            ))
            .small()
            .color(muted(ui)),
        );
        ui.add_space(8.0);
        ui.label(RichText::new(match report.filter.kind {
            Kind::Attendance=>"Punches use each record's saved time zone. Blank totals need completed attendance or saved rules. Missing check-in is a review flag; it does not establish absence.",
            Kind::Leave=>"Each row is one filed charge date inside the period. Rejected/cancelled charges are retained history; they do not consume leave. Private reasons and contacts are excluded.",
            Kind::Summary=>"Minute totals include available recorded/calculated values only. Review incomplete totals and calculation flags before using this summary. Overtime candidates and current approvals remain separate.",
            Kind::Headcount=>"Counts use current employee status and labels in your permitted scope. A historical date range cannot reconstruct past headcount. Active and archived records remain separate.",
            Kind::Trends=>"Attendance follows recorded work dates; leave uses saved charge dates and current approved/pending states. Zero records do not establish absence. Partial first/last buckets contain only the selected dates.",
        }).small().color(muted(ui)));
        if !matches!(report.filter.kind, Kind::Headcount | Kind::Trends) {
            ui.label(RichText::new("Employee fields reflect current records. Unrecorded days use dated shift assignments and the current holiday calendar. Archived employees have no inferred missing days.").small().color(muted(ui)));
        }
        workforce_view::charts(ui, report);
        ui.add_space(10.0);
        if report.rows.is_empty() {
            empty(ui, "No records match this report.");
            return;
        }
        const PAGE_SIZE: usize = 25;
        let pages = report.rows.len().div_ceil(PAGE_SIZE);
        self.report_state.page = self.report_state.page.min(pages - 1);
        ui.horizontal_wrapped(|ui| {
            if ui
                .add_enabled(self.report_state.page > 0, egui::Button::new("Previous"))
                .clicked()
            {
                self.report_state.page -= 1;
            }
            ui.label(format!(
                "Page {} of {} · Export includes all rows",
                self.report_state.page + 1,
                pages
            ));
            if ui
                .add_enabled(
                    self.report_state.page + 1 < pages,
                    egui::Button::new("Next"),
                )
                .clicked()
            {
                self.report_state.page += 1;
            }
        });
        let first = self.report_state.page * PAGE_SIZE;
        let end = (first + PAGE_SIZE).min(report.rows.len());
        card(ui, |ui| {
            let scroll = egui::ScrollArea::both()
                .id_salt("report-results")
                .max_height(420.0)
                .min_scrolled_height(260.0)
                .auto_shrink([false, true])
                .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysVisible);
            #[cfg(feature = "qa-screenshot")]
            let scroll = if std::env::var("HRIS_PREVIEW_REPORT_SCROLL").as_deref() == Ok("right") {
                scroll.horizontal_scroll_offset(3300.0)
            } else {
                scroll
            };
            scroll.show(ui, |ui| {
                ui.set_width(report.headers.len() as f32 * 198.0);
                egui::Grid::new("report-table")
                    .striped(true)
                    .num_columns(report.headers.len())
                    .min_col_width(180.0)
                    .max_col_width(180.0)
                    .spacing(Vec2::new(18.0, 10.0))
                    .show(ui, |ui| {
                        for title in &report.headers {
                            report_cell(ui, title, true, 48.0);
                        }
                        ui.end_row();
                        for row in &report.rows[first..end] {
                            for value in row {
                                report_cell(
                                    ui,
                                    if value.is_empty() { "—" } else { value },
                                    false,
                                    36.0,
                                );
                            }
                            ui.end_row();
                        }
                    });
            });
        });
    }

    #[cfg(feature = "qa-screenshot")]
    pub(super) fn prepare_reports_preview(&mut self) {
        use chrono::TimeZone;
        let form = std::env::var("HRIS_PREVIEW_FORM").unwrap_or_default();
        if !form.starts_with("reports-") || self.data.mode != Mode::Demo {
            return;
        }
        self.page = Page::Reports;
        let today = self.data.company.today();
        let day = today - Duration::days(4);
        let template = crate::shifts::ShiftTemplate {
            id: Uuid::from_u128(991),
            company_id: self.data.company.id,
            name: "Fictional report schedule".into(),
            days: vec![1, 2, 3, 4, 5, 6, 7],
            start_minute: 480,
            end_minute: 1020,
            works_on_holidays: true,
            active: true,
            revision: 0,
            break_rules: vec![],
        };
        let plan = crate::shifts::ShiftPlan::capture(&template, &self.data.company.timezone);
        let tz = self.data.company.tz();
        for a in &mut self.data.attendance {
            a.work_date = day;
            a.checked_in_at = tz
                .from_local_datetime(&day.and_hms_opt(8, 17, 0).unwrap())
                .single()
                .unwrap()
                .with_timezone(&chrono::Utc);
            a.checked_out_at = Some(
                tz.from_local_datetime(&day.and_hms_opt(18, 47, 0).unwrap())
                    .single()
                    .unwrap()
                    .with_timezone(&chrono::Utc),
            );
            a.planned_shift = Some(plan.clone());
        }
        let saved: Vec<_> = self
            .data
            .attendance
            .iter()
            .map(|a| crate::calculations::CalculationSnapshot::capture(&self.data, a))
            .collect();
        for (a, s) in self.data.attendance.iter_mut().zip(saved) {
            a.calculation_snapshot = Some(s);
        }
        let mut f = Filter::new(today);
        f.starts_on = day;
        f.ends_on = today + Duration::days(5);
        if form.contains("leave") {
            f.kind = Kind::Leave;
        }
        if form.contains("summary") {
            f.kind = Kind::Summary;
        }
        if form.contains("headcount") {
            f.kind = Kind::Headcount;
        }
        if form.contains("trends") {
            f.kind = Kind::Trends;
            f.starts_on = today - Duration::days(29);
            f.ends_on = today;
        }
        if form.contains("locations") {
            f.headcount_by = crate::workforce::GroupBy::Location;
        }
        if form.contains("types") {
            f.headcount_by = crate::workforce::GroupBy::EmploymentType;
        }
        if form.contains("weekly") {
            f.trend_bucket = crate::workforce::Granularity::Weekly;
        }
        if form.contains("monthly") {
            f.trend_bucket = crate::workforce::Granularity::Monthly;
        }
        if form == "reports-manager" || form.contains("workforce-manager") {
            let id = self.data.employees[0].id;
            self.data.access.role = Role::Manager;
            self.data.access.employee_id = Some(id);
            self.data.access.direct_reports = vec![self.data.employees[1].id];
        }
        if form == "reports-hr" {
            self.data.access.role = Role::HrGeneralist;
        }
        if form == "reports-filter" {
            f.employee_id = Some(self.data.employees[0].id);
            f.include_missing = false;
        }
        if form == "reports-empty" {
            f.department = "Fictional empty department".into();
        }
        if form.contains("workforce-empty") {
            f.department = "Fictional empty department".into();
        }
        if matches!(f.kind, Kind::Headcount | Kind::Trends) {
            for (i, request) in self.data.leave.iter_mut().enumerate() {
                let charged = day + Duration::days(i as i64);
                request.starts_on = charged;
                request.ends_on = charged;
                request.status = if i % 2 == 0 {
                    LeaveStatus::Approved
                } else {
                    LeaveStatus::Pending
                };
                request.policy.charged_units = if i % 2 == 0 { 1 } else { 2 };
                request.policy.charged_dates = vec![crate::leave::DayCharge {
                    date: charged,
                    units: request.policy.charged_units,
                }];
            }
        }
        if form.contains("columns") {
            f.columns = Some(
                match f.kind {
                    Kind::Headcount => {
                        vec!["Group (current)", "Active employees", "Archived employees"]
                    }
                    Kind::Trends => vec![
                        "Period start",
                        "Attendance employee-days",
                        "Approved leave days",
                        "Pending leave days",
                    ],
                    Kind::Leave => vec![
                        "Charge date",
                        "Full name",
                        "Leave type (filed)",
                        "Filed charge on date (days)",
                    ],
                    Kind::Summary => vec![
                        "Employee code",
                        "Full name",
                        "Attendance records",
                        "Current approved overtime minutes",
                    ],
                    _ => vec![
                        "Work date",
                        "Full name",
                        "Record status",
                        "Current approved overtime minutes",
                    ],
                }
                .into_iter()
                .map(str::to_owned)
                .collect(),
            );
            if form.contains("invalid") {
                f.columns = Some(Vec::new());
            }
        }
        self.report_state = State::new(today);
        self.report_state.filter = f.clone();
        self.report_state.result = if f.validate().is_ok() {
            Some(reports::build(&self.data, &f, chrono::Utc::now()).unwrap())
        } else {
            None
        };
        if form == "reports-next-page" {
            self.report_state.page = 1;
        }
        if form.starts_with("reports-views-") {
            let mut views = vec![
                View::new(
                    self.data.company.id,
                    "Team attendance · last 30 days".into(),
                    f.clone(),
                    Period::Last30,
                )
                .unwrap(),
                View::new(
                    self.data.company.id,
                    "Monthly leave overview".into(),
                    Filter {
                        kind: Kind::Leave,
                        columns: None,
                        ..f.clone()
                    },
                    Period::ThisMonth,
                )
                .unwrap(),
            ];
            if form.contains("manager") {
                let id = self.data.employees[0].id;
                self.data.access.role = Role::Manager;
                self.data.access.employee_id = Some(id);
                self.data.access.direct_reports = vec![self.data.employees[1].id];
            }
            self.report_state.selected_view = Some(views[0].id);
            if form.contains("edit") || form.contains("invalid") {
                self.report_state.view_draft = Some(views[0].clone());
            }
            if form.contains("invalid") {
                self.report_state.view_draft.as_mut().unwrap().name = views[1].name.clone();
            }
            if form.contains("empty") {
                views.clear();
                self.report_state.selected_view = None;
            }
            if form.contains("applied") {
                self.report_state
                    .apply(views[0].resolve(&self.data, today).unwrap());
                self.notice = Some((
                    "Saved filters applied. Run report to refresh the results.".into(),
                    false,
                ));
            }
            self.account.as_mut().unwrap().report_views = views;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn report_cells_keep_wrapped_headers_unjustified_and_reserve_long_row_height() {
        for scale in [1.0, 1.25] {
            let ctx = egui::Context::default();
            configure_theme(&ctx);
            ctx.set_zoom_factor(scale);
            let mut cells = Vec::new();
            let mut output = None;
            for _ in 0..3 {
                cells.clear();
                let mut frame=ctx.run_ui(egui::RawInput{screen_rect:Some(egui::Rect::from_min_size(egui::Pos2::ZERO,Vec2::new(620.0,1000.0))),..Default::default()},|ui| {
                    egui::Grid::new("report-cell-regression").num_columns(2).spacing(Vec2::new(18.0,10.0)).show(ui,|ui| {
                        cells.push(report_cell(ui,"Attendance employee-days",true,48.0));
                        cells.push(report_cell(ui,"Period start",true,48.0));ui.end_row();
                        cells.push(report_cell(ui,"Synthetic department with a long label that must wrap and reserve its complete measured row height",false,36.0));
                        cells.push(report_cell(ui,"1",false,36.0));ui.end_row();
                        cells.push(report_cell(ui,"Next report row",false,36.0));ui.end_row();
                    });
                });
                frame.textures_delta.clear();
                output = Some(frame);
            }
            let shapes = output.unwrap().shapes;
            let text = |label: &str| {
                shapes
                    .iter()
                    .find_map(|s| match &s.shape {
                        egui::epaint::Shape::Text(t) if t.galley.job.text == label => Some(t),
                        _ => None,
                    })
                    .unwrap()
            };
            let header = text("Attendance employee-days");
            let peer = text("Period start");
            assert!(!header.galley.job.justify);
            assert!((header.pos.y - peer.pos.y).abs() < 0.5);
            assert!(cells[2].height() > 36.0);
            assert!(
                cells[4].top() + 0.5 >= cells[2].bottom(),
                "Long cell {:?}; next {:?}",
                cells[2],
                cells[4]
            );
        }
    }

    #[test]
    fn applying_saved_filters_clears_cached_results_and_paging_even_for_identical_filters() {
        let data = DemoStore::new().data;
        let today = data.company.today();
        let mut state = State::new(today);
        state.result = Some(reports::build(&data, &state.filter, chrono::Utc::now()).unwrap());
        state.page = 2;
        state.apply(state.filter.clone());
        assert!(state.result.is_none());
        assert_eq!(state.page, 0);
        assert_eq!(state.start_month, month_start(state.filter.starts_on));
        assert_eq!(state.end_month, month_start(state.filter.ends_on));
    }

    #[test]
    fn view_editor_closes_only_after_matching_account_preferences_arrive() {
        let data = DemoStore::new().data;
        let mut state = State::new(data.company.today());
        let view = View::new(
            data.company.id,
            "Synthetic".into(),
            state.filter.clone(),
            Period::Last7,
        )
        .unwrap();
        state.view_draft = Some(view.clone());
        state.selected_view = Some(view.id);
        state.pending_views = Some(vec![view.clone()]);
        state.accept_account(&[]);
        assert!(state.view_draft.is_some());
        state.accept_account(&[view.clone()]);
        assert!(state.view_draft.is_none());
        assert!(state.pending_views.is_none());
        assert_eq!(state.selected_view, Some(view.id));
        state.pending_views = Some(vec![]);
        state.accept_account(&[]);
        assert!(state.selected_view.is_none());
    }
}
