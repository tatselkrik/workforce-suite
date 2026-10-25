use super::*;
use crate::access::Access;
use crate::leave::request_import::{self as imports, History, Import, Post, Preview};

fn import_cell(ui: &mut Ui, text: &str, width: f32, height: f32, color: Color32) {
    ui.vertical(|ui| {
        ui.set_width(width);
        ui.set_min_height(height);
        ui.add(egui::Label::new(RichText::new(text).color(color)).wrap());
    });
}

#[derive(Default)]
pub(super) struct State {
    pub open: bool,
    scope: Option<Access>,
    pub source: Option<Import>,
    batch: Option<imports::Batch>,
    preview: Option<Preview>,
    post: Option<Post>,
    history: Option<History>,
    history_view: bool,
    waiting: bool,
    error: Option<String>,
    page: usize,
}
impl State {
    pub fn has_draft(&self) -> bool {
        self.source.is_some() || self.post.is_some()
    }
    pub fn sync(&mut self, data: &Snapshot) {
        if !data.access.role.manages_hr() || self.scope.as_ref().is_some_and(|s| s != &data.access)
        {
            *self = Self::default();
        }
        if data.access.role.manages_hr() {
            self.scope = Some(data.access.clone());
        }
    }
    pub fn failed(&mut self, error: &str) {
        if self.waiting {
            self.waiting = false;
            self.error = Some(error.into());
        }
    }
    pub fn accept_preview(&mut self, p: Preview, data: &Snapshot) {
        self.waiting = false;
        let result = self
            .batch
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Reopen the source file after access changed."))
            .and_then(|b| p.validate(data, b));
        match result {
            Ok(()) => {
                self.preview = Some(p);
                self.error = None;
            }
            Err(e) => self.error = Some(e.to_string()),
        }
    }
    pub fn accept_receipt(&mut self, r: imports::Receipt, data: &Snapshot) {
        self.waiting = false;
        if let Some(p) = &self.post {
            match r.validate(data, p) {
                Ok(()) => {
                    self.source = None;
                    self.preview = None;
                    self.batch = None;
                    self.post = None;
                    self.history_view = true;
                    self.page = 0;
                    self.error = None;
                }
                Err(e) => self.error = Some(e.to_string()),
            }
        }
    }
    pub fn accept_history(&mut self, h: History, data: &Snapshot) {
        self.waiting = false;
        if let Err(e) = h.validate(data) {
            self.error = Some(e.to_string());
            return;
        }
        if let Some(post) = &self.post {
            let confirmed = h
                .batches
                .iter()
                .find(|b| b.id == post.batch.id)
                .is_some_and(|b| {
                    b.actor_id == data.access.user_id
                        && b.source_fingerprint == post.preview.fingerprint
                        && b.reason == post.batch.reason
                        && b.reference == post.batch.reference
                        && b.timezone == post.batch.expected_timezone
                        && b.row_count == post.batch.rows.len()
                })
                && post.batch.rows.iter().all(|r| {
                    h.items
                        .iter()
                        .find(|i| i.id == r.id && i.batch_id == post.batch.id)
                        .is_some_and(|i| {
                            i.source_row == r.source_row
                                && i.starts_on == r.starts_on
                                && i.ends_on == r.ends_on
                                && i.day_part == r.day_part
                                && i.type_code == r.type_code
                                && i.reference == r.reference
                        })
                });
            if confirmed {
                self.source = None;
                self.preview = None;
                self.batch = None;
                self.post = None;
                self.history_view = true;
                self.error = None;
            }
        }
        self.history = Some(h);
    }
}
impl HrisApp {
    pub(super) fn leave_request_import(&mut self, ui: &mut Ui) {
        let mut state = std::mem::take(&mut self.leave_request_import_state);
        state.sync(&self.data);
        if !self.data.access.role.manages_hr() {
            empty(ui, "Only HR and Administrators can import leave requests.");
            self.leave_request_import_state = state;
            return;
        }
        let pending = state.post.is_some();
        card(ui, |ui| {
            ui.heading("Leave request import");
            ui.label("Import reviewed leave requests with their normal dated charges, running reservations and configured approval chains.");
            ui.add_space(6.0);
            ui.label(RichText::new(format!("Company time zone: {} · Up to 100 rows per file · CSV or Excel · Source files are read locally and are not uploaded.",self.data.company.timezone)).small().color(muted(ui)));
        });
        ui.add_space(10.0);
        ui.horizontal_wrapped(|ui| {
            ui.selectable_value(&mut state.history_view, false, "Import file");
            ui.selectable_value(&mut state.history_view, true, "Import history");
            if ui
                .add_enabled(!self.busy, egui::Button::new("Reload history"))
                .clicked()
            {
                state.waiting = true;
                self.send(Command::LeaveImportHistory);
            }
        });
        if let Some(error) = &state.error {
            ui.colored_label(ui.visuals().error_fg_color, error);
        }
        if state.history_view
            && ui
                .add_enabled(
                    !self.busy,
                    egui::Button::new("Open Leave to review or cancel requests"),
                )
                .clicked()
        {
            self.page = Page::Leave;
        }
        if state.history_view {
            self.leave_request_import_history_view(ui, &mut state);
        } else {
            card(ui, |ui| {
                ui.add_enabled_ui(!self.busy && !pending, |ui| {
                    ui.horizontal_wrapped(|ui| {
                        for (excel, label, name) in [
                            (false, "Save CSV template", "hris-leave-requests.csv"),
                            (true, "Save Excel template", "hris-leave-requests.xlsx"),
                        ] {
                            if ui.button(label).clicked() {
                                match crate::reports::choose_export(name).and_then(|p| {
                                    p.map(|p| {
                                        imports::template(excel)
                                            .and_then(|b| crate::reports::save_export(&p, &b))
                                    })
                                    .transpose()
                                }) {
                                    Ok(Some(())) => {
                                        self.notice = Some((
                                            "Leave template saved to a new file.".into(),
                                            false,
                                        ))
                                    }
                                    Err(e) => state.error = Some(e.to_string()),
                                    _ => {}
                                }
                            }
                        }
                        if primary(
                            ui,
                            if state.source.is_some() {
                                "Reopen / choose file"
                            } else {
                                "Choose CSV / Excel"
                            },
                        ) {
                            match crate::imports::choose_tabular_source("Preview leave requests")
                                .and_then(|p| p.map(|p| Import::load(&self.data, &p)).transpose())
                            {
                                Ok(Some(source)) => {
                                    state.source = Some(source);
                                    state.batch = None;
                                    state.preview = None;
                                    state.page = 0;
                                    state.error = None;
                                }
                                Err(e) => state.error = Some(e.to_string()),
                                _ => {}
                            }
                        }
                    });
                });
                ui.label("Use employee codes, leave type codes and YYYY-MM-DD start/end dates. day_part is full, am or pm; half-days use one date. Excel dates may be date-only cells. Split requests crossing New Year.");
                ui.label(RichText::new("Row reasons appear in normal employee/reviewer leave history. Import notes/references stay HR/Admin-only. Pending reservations, overlaps, dated policies and unavailable approvers can block a batch. Opening a file or previewing creates no lasting records.").small().color(muted(ui)));
            });
            if let Some(post) = &state.post {
                ui.add_space(8.0);
                card(ui, |ui| {
                    ui.strong("Submission waiting for confirmation");
                    ui.label("Keep this exact submission. Refresh history to see whether it was saved, or retry it without creating another batch.");
                    ui.horizontal_wrapped(|ui| {
                        if ui
                            .add_enabled(!self.busy, egui::Button::new("Retry exact submission"))
                            .clicked()
                        {
                            state.waiting = true;
                            self.send(Command::PostLeaveImport(Box::new(post.clone())));
                        }
                        if ui
                            .add_enabled(!self.busy, egui::Button::new("Refresh to confirm"))
                            .clicked()
                        {
                            state.waiting = true;
                            self.send(Command::LeaveImportHistory);
                        }
                    });
                });
            }
            if let Some(source) = &mut state.source {
                ui.add_space(10.0);
                card(ui, |ui| {
                    ui.heading(&source.filename);
                    ui.label(format!("{} source row(s)", source.rows.len()));
                    let before = (source.reason.clone(), source.reference.clone());
                    ui.add_enabled_ui(!self.busy && !pending, |ui| {
                        egui::Grid::new("attendance-import-note")
                            .num_columns(2)
                            .spacing([18.0, 10.0])
                            .show(ui, |ui| {
                                ui.label("Import note (HR only)");
                                ui.add(
                                    egui::TextEdit::singleline(&mut source.reason)
                                        .desired_width(480.0)
                                        .char_limit(500),
                                );
                                ui.end_row();
                                ui.label("Reference (HR only)");
                                ui.add(
                                    egui::TextEdit::singleline(&mut source.reference)
                                        .desired_width(480.0)
                                        .char_limit(200),
                                );
                                ui.end_row();
                            });
                    });
                    if before != (source.reason.clone(), source.reference.clone()) {
                        state.preview = None;
                        state.batch = None;
                    }
                    let pages = source.rows.len().div_ceil(12).max(1);
                    state.page = state.page.min(pages - 1);
                    if ui.available_width() < 1000.0 {
                        ui.label(
                            RichText::new(
                                "Scroll the row table horizontally to read every column.",
                            )
                            .small()
                            .color(muted(ui)),
                        );
                    }
                    let row_scroll = egui::ScrollArea::horizontal().scroll_bar_visibility(
                        egui::scroll_area::ScrollBarVisibility::AlwaysVisible,
                    );
                    #[cfg(feature = "qa-screenshot")]
                    let row_scroll = if std::env::var("HRIS_PREVIEW_FORM")
                        .is_ok_and(|v| v == "leave-import-errors")
                    {
                        row_scroll.horizontal_scroll_offset(250.0)
                    } else {
                        row_scroll
                    };
                    row_scroll.show(ui, |ui| {
                        egui::Grid::new("attendance-import-rows")
                            .striped(true)
                            .spacing([18.0, 10.0])
                            .show(ui, |ui| {
                                for h in [
                                    "Source row",
                                    "Employee",
                                    "Leave type",
                                    "Dates / part",
                                    "Review",
                                ] {
                                    ui.strong(h);
                                }
                                ui.end_row();
                                for row in source.rows.iter().skip(state.page * 12).take(12) {
                                    let mut errors = row.errors.clone();
                                    if let Some(p) = state.preview.as_ref().and_then(|p| {
                                        p.rows.iter().find(|r| r.source_row == row.number)
                                    }) {
                                        errors.extend(p.errors.clone());
                                    }
                                    let review = if errors.is_empty() {
                                        state
                                            .preview
                                            .as_ref()
                                            .and_then(|p| {
                                                p.rows.iter().find(|r| r.source_row == row.number)
                                            })
                                            .map(|p| {
                                                format!(
                                                    "{} day(s); {} available after request",
                                                    crate::leave::days(
                                                        p.charged_units.unwrap_or(0)
                                                    ),
                                                    p.balance_after
                                                        .as_ref()
                                                        .map(|b| crate::leave::days(
                                                            b.available_units
                                                        ))
                                                        .unwrap_or_else(|| "no limit".into())
                                                )
                                            })
                                            .unwrap_or_else(|| "Ready for protected preview".into())
                                    } else {
                                        errors.join("\n")
                                    };
                                    let clocks = format!(
                                        "{} to {} ({})",
                                        row.cells.get(2).map_or("", String::as_str),
                                        row.cells.get(3).map_or("", String::as_str),
                                        row.cells.get(4).map_or("", String::as_str)
                                    );
                                    let color = ui.visuals().text_color();
                                    let font = egui::TextStyle::Body.resolve(ui.style());
                                    let height = ui.fonts_mut(|f| {
                                        f.layout(review.clone(), font.clone(), color, 300.0)
                                            .size()
                                            .y
                                            .max(
                                                f.layout(clocks.clone(), font, color, 300.0)
                                                    .size()
                                                    .y,
                                            )
                                    });
                                    import_cell(ui, &row.number.to_string(), 60.0, height, color);
                                    import_cell(
                                        ui,
                                        row.cells.first().map_or("", String::as_str),
                                        90.0,
                                        height,
                                        color,
                                    );
                                    import_cell(
                                        ui,
                                        row.cells.get(1).map_or("", String::as_str),
                                        100.0,
                                        height,
                                        color,
                                    );
                                    import_cell(ui, &clocks, 300.0, height, color);
                                    import_cell(
                                        ui,
                                        &review,
                                        300.0,
                                        height,
                                        if errors.is_empty() {
                                            color
                                        } else {
                                            ui.visuals().error_fg_color
                                        },
                                    );
                                    ui.end_row();
                                }
                            });
                    });
                    if pages > 1 {
                        ui.horizontal(|ui| {
                            if ui
                                .add_enabled(state.page > 0, egui::Button::new("Previous"))
                                .clicked()
                            {
                                state.page -= 1;
                            }
                            ui.label(format!("Page {} of {pages}", state.page + 1));
                            if ui
                                .add_enabled(state.page + 1 < pages, egui::Button::new("Next"))
                                .clicked()
                            {
                                state.page += 1;
                            }
                        });
                    }
                    ui.add_space(10.0);
                    ui.horizontal_wrapped(|ui| {
                        if ui
                            .add_enabled(
                                !self.busy && !pending && source.valid_rows(),
                                egui::Button::new("Preview all rows — no changes"),
                            )
                            .clicked()
                        {
                            match source.batch(&self.data) {
                                Ok(b) => {
                                    state.batch = Some(b.clone());
                                    state.preview = None;
                                    state.error = None;
                                    state.waiting = true;
                                    self.send(Command::PreviewLeaveImport(b));
                                }
                                Err(e) => state.error = Some(e.to_string()),
                            }
                        }
                        for (label, name) in [
                            ("Review / errors CSV", "hris-leave-row-review.csv"),
                            ("Review / errors Excel", "hris-leave-row-review.xlsx"),
                        ] {
                            if ui
                                .add_enabled(!self.busy, egui::Button::new(label))
                                .clicked()
                            {
                                match crate::reports::choose_export(name) {
                                    Ok(Some(destination)) => {
                                        self.send(Command::LeaveImportErrors {
                                            source: source.clone(),
                                            preview: state.preview.clone(),
                                            destination,
                                        })
                                    }
                                    Err(e) => state.error = Some(e.to_string()),
                                    _ => {}
                                }
                            }
                        }
                    });
                    if let (Some(batch), Some(preview)) = (&state.batch, &state.preview) {
                        let ready = preview.ready && preview.validate(&self.data, batch).is_ok();
                        if ui
                            .add_enabled(
                                !self.busy && !pending && ready,
                                egui::Button::new(
                                    RichText::new("Submit all leave requests for review")
                                        .color(Color32::WHITE),
                                )
                                .fill(TEAL)
                                .stroke(Stroke::NONE),
                            )
                            .clicked()
                        {
                            let post = Post {
                                scope: self.data.access.clone(),
                                batch: batch.clone(),
                                preview: preview.clone(),
                            };
                            match post.validate(&self.data) {
                                Ok(()) => {
                                    state.post = Some(post.clone());
                                    state.waiting = true;
                                    state.error = None;
                                    self.send(Command::PostLeaveImport(Box::new(post)));
                                }
                                Err(e) => state.error = Some(e.to_string()),
                            }
                        }
                    }
                });
            } else if !pending {
                empty(
                    ui,
                    "Save a template, fill in reviewed source rows, then choose the file. No requests are created by opening or previewing it.",
                );
            }
            if (state.source.is_some() || state.post.is_some())
                && ui
                    .add_enabled(!self.busy, egui::Button::new("Discard local draft"))
                    .clicked()
            {
                state.source = None;
                state.preview = None;
                state.batch = None;
                state.post = None;
                state.error = None;
            }
        }
        self.leave_request_import_state = state;
    }
    fn leave_request_import_history_view(&mut self, ui: &mut Ui, state: &mut State) {
        let Some(h) = &state.history else {
            empty(
                ui,
                "Reload history to see imported batches and current review status.",
            );
            return;
        };
        if h.batches.is_empty() {
            empty(ui, "No leave request batches have been imported.");
            return;
        }
        let pages = h.batches.len().div_ceil(10).max(1);
        state.page = state.page.min(pages - 1);
        for batch in h.batches.iter().skip(state.page * 10).take(10) {
            let items = h
                .items
                .iter()
                .filter(|i| i.batch_id == batch.id)
                .collect::<Vec<_>>();
            let tz = batch
                .timezone
                .parse::<chrono_tz::Tz>()
                .unwrap_or_else(|_| self.data.company.tz());
            let pending = items
                .iter()
                .filter(|i| i.status == LeaveStatus::Pending)
                .count();
            card(ui, |ui| {
                egui::CollapsingHeader::new(format!(
                    "{} · {} rows · {} pending · {}",
                    batch.created_at.with_timezone(&tz).format("%Y-%m-%d %H:%M"),
                    batch.row_count,
                    pending,
                    batch.actor_name
                ))
                .id_salt(batch.id)
                .default_open(true)
                .show(ui, |ui| {
                    ui.label(format!("HR import note: {}", batch.reason));
                    if !batch.reference.is_empty() {
                        ui.label(format!("HR reference: {}", batch.reference));
                    }
                    for item in &items {
                        ui.push_id(item.id, |ui| {
                            ui.separator();
                            ui.horizontal_wrapped(|ui| {
                                ui.strong(format!(
                                    "Row {} · {} · {}",
                                    item.source_row, item.employee_code, item.employee_name
                                ));
                                ui.label(format!("{} to {}", item.starts_on, item.ends_on));
                                badge(
                                    ui,
                                    item.status.label(),
                                    item.status == LeaveStatus::Approved,
                                );
                            });
                            ui.label(format!(
                                "{} · {} · {} day(s) · policy revision {}",
                                item.type_name,
                                item.day_part.label(),
                                crate::leave::days(item.charged_units),
                                item.policy_revision
                            ));
                            ui.label(
                                RichText::new(format!(
                                    "{} of {} approval steps completed",
                                    item.approval_steps
                                        .iter()
                                        .filter(|s| s.approved_by.is_some())
                                        .count(),
                                    item.approval_steps.len()
                                ))
                                .small()
                                .color(muted(ui)),
                            );
                            if !item.reference.is_empty() {
                                ui.label(format!("HR row reference: {}", item.reference));
                            }
                        });
                    }
                });
            });
        }
        if pages > 1 {
            ui.horizontal(|ui| {
                if ui
                    .add_enabled(state.page > 0, egui::Button::new("Previous batches"))
                    .clicked()
                {
                    state.page -= 1;
                }
                ui.label(format!("Page {} of {pages}", state.page + 1));
                if ui
                    .add_enabled(state.page + 1 < pages, egui::Button::new("Next batches"))
                    .clicked()
                {
                    state.page += 1;
                }
            });
        }
    }
    #[cfg(feature = "qa-screenshot")]
    pub(super) fn prepare_leave_request_import_preview(&mut self) {
        let form = std::env::var("HRIS_PREVIEW_FORM").unwrap_or_default();
        if !form.starts_with("leave-import-") || self.data.mode != Mode::Demo {
            return;
        }
        self.page = Page::Reports;
        self.import_state.open = false;
        self.attendance_import_state.open = false;
        self.data.leave.clear();
        self.data.leave_setup.events.clear();
        self.data.leave_setup.dated = None;
        self.data.leave_setup.credits = None;
        for e in &mut self.data.employees {
            e.joined_on = Some("2025-01-01".parse().unwrap());
        }
        let mut state = State {
            open: true,
            ..Default::default()
        };
        state.sync(&self.data);
        if form == "leave-import-entry" {
            self.leave_request_import_state = state;
            return;
        }
        let mut day = self.data.company.today() + Duration::days(1);
        while !crate::leave::dated::policy_on(&self.data, day).working(day) {
            day += Duration::days(1);
        }
        let code = self.data.employees[1].employee_code.clone();
        let second = if form == "leave-import-errors" {
            "UNKNOWN".into()
        } else {
            self.data.employees[2].employee_code.clone()
        };
        let text = format!(
            "{}\n{code},annual,{day},{day},am,Fictional reviewed leave source,Fictional source row2\n{second},annual,{day},{day},full,Fictional reviewed source,Fictional source row3\n",
            imports::HEADERS.join(",")
        );
        let mut source = Import::from_csv(&self.data, "Fictional leave source.csv", &text).unwrap();
        source.reason = "Fictional reviewed source; ordinary leave approvals still apply.".into();
        source.reference = "Fictional leave register".into();
        if source.valid_rows() {
            let batch = source.batch(&self.data).unwrap();
            let preview = imports::Demo::preview(&self.data, &batch).unwrap();
            state.batch = Some(batch.clone());
            state.preview = Some(preview.clone());
            let post = Post {
                scope: self.data.access.clone(),
                batch,
                preview,
            };
            if form == "leave-import-history" {
                let mut demo = imports::Demo::default();
                let receipt = demo.post(&mut self.data, &post).unwrap();
                crate::leave::apply_demo(
                    &mut self.data,
                    Action::ReviewLeave(
                        receipt.items[0].request_id,
                        0,
                        true,
                        "Fictional ordinary review".into(),
                    ),
                )
                .unwrap();
                state.history = Some(demo.history(&self.data).unwrap());
                state.history_view = true;
                self.leave_request_import_state = state;
                return;
            }
            if form == "leave-import-uncertain" {
                state.post = Some(post);
            }
        }
        state.source = Some(source);
        self.leave_request_import_state = state;
    }
}
