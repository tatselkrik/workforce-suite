use super::*;
use crate::access::Access;
use crate::attendance_imports::{self as imports, History, Import, Post, Preview, Withdraw};

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
    withdraw: Option<Withdraw>,
    withdrawal_sent: bool,
    history_view: bool,
    waiting: bool,
    error: Option<String>,
    page: usize,
}
impl State {
    pub fn has_draft(&self) -> bool {
        self.source.is_some() || self.post.is_some() || self.withdraw.is_some()
    }
    pub fn sync(&mut self, data: &Snapshot) {
        if !data
            .access
            .allows(crate::staff_roles::Capability::AttendanceAdministration)
            || self.scope.as_ref().is_some_and(|s| s != &data.access)
        {
            *self = Self::default();
        }
        if data
            .access
            .allows(crate::staff_roles::Capability::AttendanceAdministration)
        {
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
                                && i.work_date == r.work_date
                                && i.proposed_in == r.checked_in_at
                                && i.proposed_out == r.checked_out_at
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
        if self.withdraw.as_ref().is_some_and(|w| {
            h.withdrawals.iter().any(|r| {
                r.id == w.id && r.item_id == w.item_id && r.actor_id == data.access.user_id
            })
        }) {
            self.withdraw = None;
            self.withdrawal_sent = false;
            self.error = None;
        }
        self.history = Some(h);
    }
}
impl HrisApp {
    pub(super) fn attendance_import(&mut self, ui: &mut Ui) {
        let mut state = std::mem::take(&mut self.attendance_import_state);
        state.sync(&self.data);
        if !self
            .data
            .access
            .allows(crate::staff_roles::Capability::AttendanceAdministration)
        {
            empty(
                ui,
                "Only HR and Administrators can import attendance correction requests.",
            );
            self.attendance_import_state = state;
            return;
        }
        let pending = state.post.is_some() || state.withdraw.is_some();
        card(ui, |ui| {
            ui.heading("Attendance correction import");
            ui.label("Import reviewed clock proposals as pending correction requests. Attendance changes only after normal review by another authorized HR staff member or the current manager.");
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
                self.send(Command::AttendanceImportHistory);
            }
        });
        if let Some(error) = &state.error {
            ui.colored_label(ui.visuals().error_fg_color, error);
        }
        if state.history_view {
            self.attendance_import_history_view(ui, &mut state);
        } else {
            card(ui, |ui| {
                ui.add_enabled_ui(!self.busy && !pending, |ui| {
                    ui.horizontal_wrapped(|ui| {
                        for (excel, label, name) in [
                            (false, "Save CSV template", "hris-attendance-requests.csv"),
                            (true, "Save Excel template", "hris-attendance-requests.xlsx"),
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
                                            "Attendance template saved to a new file.".into(),
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
                            match crate::imports::choose_tabular_source(
                                "Preview attendance correction requests",
                            )
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
                ui.label("Use YYYY-MM-DD work dates and explicit timestamp offsets, for example 2026-10-25T08:00:00+08:00. Leave checkout blank only for an open shift. Keep employee codes and timestamps as Text in Excel.");
                ui.label(RichText::new("Row reasons appear in the employee/reviewer correction history. Import notes and reference fields remain private to HR/Admin. Duplicate employee/date rows, overlapping proposals, invalid or stale sources block the whole batch.").small().color(muted(ui)));
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
                            self.send(Command::PostAttendanceImport(Box::new(post.clone())));
                        }
                        if ui
                            .add_enabled(!self.busy, egui::Button::new("Refresh to confirm"))
                            .clicked()
                        {
                            state.waiting = true;
                            self.send(Command::AttendanceImportHistory);
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
                        .is_ok_and(|v| v == "attendance-import-errors")
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
                                    "Work date",
                                    "Check-in / checkout",
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
                                        if state.preview.is_some() {
                                            "Protected preview passed".into()
                                        } else {
                                            "Ready for protected preview".into()
                                        }
                                    } else {
                                        errors.join("\n")
                                    };
                                    let clocks = row
                                        .cells
                                        .iter()
                                        .skip(2)
                                        .take(2)
                                        .map(|v| {
                                            if v.is_empty() {
                                                "Open / no checkout"
                                            } else {
                                                v.as_str()
                                            }
                                        })
                                        .collect::<Vec<_>>()
                                        .join("\n");
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
                                    self.send(Command::PreviewAttendanceImport(b));
                                }
                                Err(e) => state.error = Some(e.to_string()),
                            }
                        }
                        for (label, name) in [
                            ("Review / errors CSV", "hris-attendance-row-review.csv"),
                            ("Review / errors Excel", "hris-attendance-row-review.xlsx"),
                        ] {
                            if ui
                                .add_enabled(!self.busy, egui::Button::new(label))
                                .clicked()
                            {
                                match crate::reports::choose_export(name) {
                                    Ok(Some(destination)) => {
                                        self.send(Command::AttendanceImportErrors {
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
                                    RichText::new("Submit all correction requests for review")
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
                                    self.send(Command::PostAttendanceImport(Box::new(post)));
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
        if let Some(change) = &mut state.withdraw {
            ui.add_space(10.0);
            card(ui, |ui| {
                ui.heading("Withdraw pending imported request");
                ui.label("Only its original HR import actor can withdraw it before review. The reason will appear in the employee/reviewer correction history; attendance is unchanged.");
                ui.add_enabled(
                    !self.busy && !state.withdrawal_sent,
                    egui::TextEdit::singleline(&mut change.reason)
                        .desired_width(650.0)
                        .char_limit(500),
                );
                if ui
                    .add_enabled(
                        !self.busy && !change.reason.trim().is_empty(),
                        egui::Button::new("Withdraw / retry exact withdrawal"),
                    )
                    .clicked()
                {
                    state.withdrawal_sent = true;
                    state.waiting = true;
                    self.send(Command::WithdrawAttendanceImport(change.clone()));
                }
            });
            if ui
                .add_enabled(!self.busy, egui::Button::new("Close withdrawal draft"))
                .clicked()
            {
                state.withdraw = None;
                state.withdrawal_sent = false;
            }
        }
        self.attendance_import_state = state;
    }
    fn attendance_import_history_view(&mut self, ui: &mut Ui, state: &mut State) {
        let Some(h) = &state.history else {
            empty(
                ui,
                "Reload history to see imported batches and current review status.",
            );
            return;
        };
        if h.batches.is_empty() {
            empty(ui, "No attendance request batches have been imported.");
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
                .filter(|i| i.status == crate::requests::Status::Pending)
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
                                ui.label(item.work_date.to_string());
                                badge(
                                    ui,
                                    item.status.label(),
                                    item.status == crate::requests::Status::Approved,
                                );
                                if batch.actor_id == self.data.access.user_id
                                    && item.status == crate::requests::Status::Pending
                                    && ui
                                        .add_enabled(
                                            !self.busy
                                                && state.post.is_none()
                                                && state.withdraw.is_none(),
                                            egui::Button::new("Withdraw"),
                                        )
                                        .clicked()
                                {
                                    state.withdrawal_sent = false;
                                    state.withdraw = Some(Withdraw {
                                        id: Uuid::new_v4(),
                                        item_id: item.id,
                                        expected_revision: item.request_revision,
                                        reason: String::new(),
                                    });
                                }
                            });
                            ui.label(format!(
                                "{} to {} ({})",
                                item.proposed_in
                                    .with_timezone(&tz)
                                    .format("%Y-%m-%d %H:%M:%S"),
                                item.proposed_out
                                    .map(|t| t
                                        .with_timezone(&tz)
                                        .format("%Y-%m-%d %H:%M:%S")
                                        .to_string())
                                    .unwrap_or_else(|| "Open / no checkout".into()),
                                batch.timezone
                            ));
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
    pub(super) fn prepare_attendance_import_preview(&mut self) {
        let form = std::env::var("HRIS_PREVIEW_FORM").unwrap_or_default();
        if !form.starts_with("attendance-import-") || self.data.mode != Mode::Demo {
            return;
        }
        self.page = Page::Reports;
        self.import_state.open = false;
        self.data.attendance.clear();
        self.data.requests.records.clear();
        self.data.requests.events.clear();
        self.data.breaks.records.clear();
        self.data.shifts = Default::default();
        for e in &mut self.data.employees {
            e.joined_on = Some("2025-01-01".parse().unwrap());
        }
        let mut state = State {
            open: true,
            ..Default::default()
        };
        state.sync(&self.data);
        if form == "attendance-import-entry" {
            self.attendance_import_state = state;
            return;
        }
        let day = self.data.company.today() - chrono::Duration::days(1);
        let code = self.data.employees[1].employee_code.clone();
        let second = if form == "attendance-import-errors" {
            "UNKNOWN".to_string()
        } else {
            self.data.employees[2].employee_code.clone()
        };
        let text = format!(
            "{}\n{code},{day},{day}T08:00:00+08:00,{day}T17:00:00+08:00,Fictional source clock correction,Fictional source row2\n{second},{day},{day}T08:30:00+08:00,{day}T17:30:00+08:00,Fictional reviewed clock proposal,Fictional source row3\n",
            imports::HEADERS.join(",")
        );
        let mut source =
            Import::from_csv(&self.data, "Fictional attendance source.csv", &text).unwrap();
        source.reason="Fictional clock source reviewed by HR; rows still require independent correction review.".into();
        source.reference = "Fictional source register".into();
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
            if form == "attendance-import-history" {
                let mut demo = imports::Demo::default();
                let receipt = demo.post(&mut self.data, &post).unwrap();
                let original = self.data.access.clone();
                self.data.access.user_id = Uuid::new_v4();
                self.data.access.employee_id = Some(self.data.employees[0].id);
                crate::requests::apply_demo(
                    &mut self.data,
                    crate::requests::RequestAction::Resolve {
                        id: receipt.items[0].request_id,
                        expected_revision: 0,
                        status: crate::requests::Status::Approved,
                        reason: "Fictional independent HR review".into(),
                    },
                )
                .expect("Fictional independent approval must succeed before capturing its history");
                self.data.access = original;
                state.history = Some(demo.history(&self.data).unwrap());
                state.history_view = true;
                state.source = None;
                self.attendance_import_state = state;
                return;
            }
            if form == "attendance-import-uncertain" {
                state.post = Some(post);
            }
        }
        state.source = Some(source);
        self.attendance_import_state = state;
    }
}
