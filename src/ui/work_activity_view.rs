use super::*;
use crate::work_activity::{Context, DailyReport, ReportBody, ReportFollowUps, Task, can_review};
use serde_json::{Value, json};

const REPORT_STATUSES: [(Option<&str>, &str); 5] = [
    (None, "All statuses"),
    (Some("draft"), "Draft"),
    (Some("submitted"), "Submitted"),
    (Some("returned"), "Returned for revision"),
    (Some("approved"), "Approved"),
];

#[derive(Clone, Copy, Default)]
struct ReportFilters {
    from: Option<NaiveDate>,
    through: Option<NaiveDate>,
    status: Option<&'static str>,
    from_month: Option<NaiveDate>,
    through_month: Option<NaiveDate>,
}

impl ReportFilters {
    fn valid_range(self) -> bool {
        self.from
            .zip(self.through)
            .is_none_or(|(from, through)| from <= through)
    }

    fn matches(self, report: &DailyReport, employee: Option<Uuid>) -> bool {
        employee.is_none_or(|id| report.employee_id == id)
            && self.from.is_none_or(|from| report.work_date >= from)
            && self.through.is_none_or(|through| report.work_date <= through)
            && self.status.is_none_or(|status| report.status == status)
    }
}

#[derive(Default)]
pub(super) struct State {
    pub context: Option<Context>,
    pub follow_ups: Option<ReportFollowUps>,
    pub form: Option<Form>,
    pub pending: Option<Value>,
    pub rejected: bool,
    pub employee: Option<Uuid>,
    pub evidence_target: Option<Uuid>,
    textures: std::collections::HashMap<Uuid, egui::TextureHandle>,
    report_tab: bool,
    report_filters: ReportFilters,
}
pub(super) struct Form {
    operation: &'static str,
    id: Uuid,
    revision: i64,
    employee: Option<Uuid>,
    date: String,
    end: String,
    title: String,
    outcome: String,
    choice: String,
    note: String,
    enabled: bool,
    office_days: [bool; 7],
    body: ReportBody,
    task_ids: Vec<Uuid>,
}
impl Form {
    fn new(operation: &'static str, today: NaiveDate) -> Self {
        Self {
            operation,
            id: Uuid::new_v4(),
            revision: -1,
            employee: None,
            date: today.to_string(),
            end: String::new(),
            title: String::new(),
            outcome: String::new(),
            choice: if operation == "policy_save" {
                "remote"
            } else {
                "normal"
            }
            .into(),
            note: String::new(),
            enabled: false,
            office_days: [true, true, true, true, true, false, false],
            body: Default::default(),
            task_ids: vec![],
        }
    }
    fn request(&self, submitting: bool) -> anyhow::Result<Value> {
        let mut v = json!({"request_id":Uuid::new_v4(),"id":self.id,"operation":self.operation,"expected_revision":self.revision});
        match self.operation {
            "policy_save" => {
                let day = parse_date(&self.date)?;
                let end = if self.end.trim().is_empty() {
                    None
                } else {
                    Some(parse_date(&self.end)?)
                };
                anyhow::ensure!(
                    self.employee.is_some()
                        && (1..=500).contains(&self.note.trim().chars().count()),
                    "Choose an employee and give a policy reason."
                );
                v["employee_id"] = json!(self.employee);
                v["starts_on"] = json!(day);
                v["ends_on"] = json!(end);
                v["arrangement"] = json!(self.choice);
                v["office_days"] = json!(
                    self.office_days
                        .iter()
                        .enumerate()
                        .filter_map(|(i, yes)| yes.then_some(i + 1))
                        .collect::<Vec<_>>()
                );
                v["enabled"] = json!(self.enabled);
                v["reason"] = json!(self.note.trim());
            }
            "task_save" => {
                anyhow::ensure!(
                    self.employee.is_some()
                        && !self.title.trim().is_empty()
                        && !self.outcome.trim().is_empty(),
                    "Choose an employee, task and expected outcome."
                );
                v["employee_id"] = json!(self.employee);
                v["title"] = json!(self.title.trim());
                v["outcome"] = json!(self.outcome.trim());
                v["due_on"] = json!(parse_date(&self.date)?);
                v["priority"] = json!(self.choice);
            }
            "station_save" => {
                v["name"] = json!(self.title.trim());
                v["active"] = json!(true);
            }
            "station_pin_reset" => {
                v["employee_id"] = json!(self.employee);
                v["pin"] = json!(self.note);
            }
            "report_save" => {
                self.body.validate(submitting)?;
                v["operation"] = json!(if submitting {
                    "report_submit"
                } else {
                    "report_save"
                });
                v["work_date"] = json!(parse_date(&self.date)?);
                v["body"] = json!(self.body);
                v["task_ids"] = json!(self.task_ids);
            }
            _ => {
                v["status"] = json!(self.choice);
                v["note"] = json!(self.note.trim());
            }
        }
        Ok(v)
    }
}
impl State {
    pub fn editing(&self) -> bool {
        self.form.is_some() || self.pending.is_some()
    }
    pub fn clear(&mut self) {
        *self = Self::default();
    }
    pub fn accept(&mut self, c: Context, data: &Snapshot) -> anyhow::Result<()> {
        c.validate(data)?;
        if self.pending.as_ref().is_some_and(|p| {
            c.receipt
                .as_ref()
                .is_some_and(|r| r["request_id"] == p["request_id"])
        }) {
            self.pending = None;
            self.form = None;
            self.rejected = false;
        }
        self.textures.clear();
        self.context = Some(c);
        Ok(())
    }
}
fn editor_text(ui: &mut Ui, label: &str, value: &mut String, multiline: bool, max: usize) {
    ui.label(label);
    let id = egui::Id::unique(format!("work-{label}"));
    if multiline {
        ui.add(
            egui::TextEdit::multiline(value)
                .id(id)
                .desired_rows(3)
                .desired_width(f32::INFINITY)
                .char_limit(max),
        );
    } else {
        ui.add(
            egui::TextEdit::singleline(value)
                .id(id)
                .desired_width(f32::INFINITY)
                .char_limit(max),
        );
    }
    ui.add_space(6.0);
}

fn report_date_filter(
    ui: &mut Ui,
    label: &str,
    date: &mut Option<NaiveDate>,
    month: &mut Option<NaiveDate>,
    data: &Snapshot,
) {
    ui.push_id(label, |ui| {
        let value = date.map_or_else(|| "Any date".into(), |d| d.format("%b %d, %Y").to_string());
        let button = ui.add(
            egui::Button::new(format!("{label} · {value}")).wrap_mode(egui::TextWrapMode::Extend),
        );
        egui::Popup::from_toggle_button_response(&button).show(|ui| {
            let today = data.company.today();
            let mut selected = date.unwrap_or(today);
            let month = month.get_or_insert_with(|| month_start(selected));
            if attendance_calendar(ui, &mut selected, month, today, data) {
                *date = Some(selected);
                ui.close();
            }
            if date.is_some() && ui.button("Any date").clicked() {
                *date = None;
                ui.close();
            }
        });
    });
}

impl HrisApp {
    pub(super) fn attendance_photos(&mut self, ui: &mut Ui) {
        let Some(_record) = self.work_state.evidence_target else {
            return;
        };
        let mut close = false;
        card(ui, |ui| {
            ui.horizontal(|ui| {
                ui.strong("Camera evidence");
                close = ui.button("Close photos").clicked();
            });
            if let Some(context) = &self.work_state.context {
                if context.images.is_empty() {
                    ui.label("No retained kiosk photo is available for this attendance entry.");
                }
                for photo in &context.images {
                    ui.label(format!(
                        "{} · {} · {}",
                        photo.operation.replace('_', " "),
                        photo.recorded_at,
                        photo.station
                    ));
                    if !self.work_state.textures.contains_key(&photo.id) {
                        if let Ok(image) = image::load_from_memory(&photo.bytes) {
                            let rgb = image.to_rgb8();
                            let size = [rgb.width() as usize, rgb.height() as usize];
                            self.work_state.textures.insert(
                                photo.id,
                                ui.ctx().load_texture(
                                    format!("photo-{}", photo.id),
                                    egui::ColorImage::from_rgb(size, rgb.as_raw()),
                                    egui::TextureOptions::LINEAR,
                                ),
                            );
                        }
                    }
                    if let Some(texture) = self.work_state.textures.get(&photo.id) {
                        ui.add(
                            egui::Image::from_texture(texture).max_size(egui::vec2(320.0, 240.0)),
                        );
                    }
                }
            } else {
                ui.label("Loading camera evidence…");
            }
        });
        if close {
            self.work_state.evidence_target = None;
            self.work_state.textures.clear();
            if let Some(c) = &mut self.work_state.context {
                c.images.clear();
            }
        }
    }
    pub(super) fn office_stations(&mut self, ui: &mut Ui) {
        ui.heading("Office attendance stations");
        ui.label("Enroll an office PC, then unlock its attendance-only screen with an Administrator account. Station grants expire after 16 hours and stay in memory.");
        if self.work_state.form.is_some() {
            self.work_editor(ui);
            return;
        }
        if self.work_state.context.is_none() {
            if ui.button("Load station settings").clicked() {
                self.send(Command::WorkActivity(None));
            }
            return;
        }
        let context = self.work_state.context.clone().unwrap();
        ui.horizontal_wrapped(|ui|{
            if ui.button("Refresh stations").clicked(){self.send(Command::WorkActivity(None));}
            if self.data.access.allows(crate::staff_roles::Capability::AccountAdministration)&&ui.button("Enroll station").clicked(){self.work_state.form=Some(Form::new("station_save",self.data.company.today()));}
            if context.can_administer&&ui.button("Set or reset an employee PIN").clicked(){self.work_state.form=Some(Form::new("station_pin_reset",self.data.company.today()));}
            if context.can_administer&&ui.button("Clean expired attendance photos").on_hover_text("Deletes images older than 7 days and abandoned uploads; retains punch receipts and photo hashes.").clicked(){self.send(Command::WorkActivity(Some(json!({"operation":"evidence_cleanup","id":Uuid::new_v4(),"request_id":Uuid::new_v4()}))));}
        });
        for station in &context.stations {
            card(ui, |ui| {
                ui.strong(station["name"].as_str().unwrap_or("Office station"));
                let active = station["active"] == true;
                ui.label(if !active {
                    "Disabled"
                } else if station["verified_at"].is_null() {
                    "Enrolled; first verified punch pending"
                } else {
                    "Active; a photo-backed punch has been recorded"
                });
                if let Some(id) = station["id"].as_str().and_then(|v| Uuid::parse_str(v).ok()) {
                    if active && ui.button("Open attendance kiosk").clicked() {
                        if let Err(e) = crate::desktop::open_kiosk(id) {
                            self.notice = Some((e.to_string(), true));
                        }
                    }
                    if ui
                        .button(if active {
                            "Disable station"
                        } else {
                            "Enable station"
                        })
                        .clicked()
                    {
                        self.send(Command::WorkActivity(Some(json!({"operation":"station_save","id":id,"request_id":Uuid::new_v4(),"name":station["name"],"expected_revision":station["revision"],"active":!active}))));
                    }
                }
            });
        }
        ui.label("Webcam snapshots are private attendance evidence. Biometric recognition remains disabled.");
    }
    pub(super) fn work_activity(&mut self, ui: &mut Ui, policies: bool) {
        if policies {
            ui.heading("Work arrangements");
            ui.label("Set dated attendance methods for office, remote and hybrid work.");
        } else {
            ui.label("Assign outcomes, record progress and review end-of-day reports.");
        }
        if self.work_state.form.is_some() {
            self.work_editor(ui);
            return;
        }
        if self.work_state.context.is_none() {
            if !self.busy && ui.button("Load current work records").clicked() {
                self.send(Command::WorkActivity(None));
            }
            return;
        }
        let context = self.work_state.context.clone().unwrap();
        ui.horizontal_wrapped(|ui| {
            if ui.button("Refresh work records").clicked() {
                self.send(Command::WorkActivity(None));
            }
            employee_picker(ui, "work-filter", &self.data, &mut self.work_state.employee);
            if self.work_state.employee.is_some() && ui.button("All permitted employees").clicked()
            {
                self.work_state.employee = None;
            }
        });
        let company_mode = self.data.mode == Mode::Supabase;
        if !company_mode {
            ui.label("Connect to your company to save policies and work reports.");
        }
        if policies {
            let selected_employee = self.work_state.employee;
            if context.can_administer
                && ui
                    .add_enabled(company_mode, egui::Button::new("New policy version"))
                    .clicked()
            {
                let mut f = Form::new("policy_save", self.data.company.today());
                f.employee = self.work_state.employee;
                self.work_state.form = Some(f);
            }
            for p in context
                .policies
                .iter()
                .filter(|p| selected_employee.is_none_or(|id| p.employee_id == id))
            {
                card(ui, |ui| {
                    ui.strong(format!(
                        "{} · {}",
                        self.data.employee_name(p.employee_id),
                        p.arrangement
                    ));
                    ui.label(format!(
                        "{} → {} · {}",
                        p.starts_on,
                        p.ends_on.map(|d| d.to_string()).unwrap_or("Ongoing".into()),
                        if p.enabled {
                            "Active version"
                        } else {
                            "Draft; punches unchanged"
                        }
                    ));
                    ui.label(&p.reason);
                    if context.can_administer
                        && company_mode
                        && ui.button("Copy as new version").clicked()
                    {
                        let mut f = Form::new("policy_save", self.data.company.today());
                        f.employee = Some(p.employee_id);
                        f.choice = p.arrangement.clone();
                        f.note = p.reason.clone();
                        f.office_days =
                            std::array::from_fn(|i| p.office_days.contains(&((i + 1) as u8)));
                        self.work_state.form = Some(f);
                    }
                });
            }
            if context.policies.is_empty() {
                ui.label("No dated policy has been saved. Existing punch methods continue.");
            }
            return;
        }
        ui.horizontal_wrapped(|ui| {
            ui.selectable_value(&mut self.work_state.report_tab, false, "Tasks");
            ui.selectable_value(&mut self.work_state.report_tab, true, "Daily reports");
            if !self.work_state.report_tab
                && (context.can_administer
                    || self.data.access.allows(crate::staff_roles::Capability::TeamReview))
                && ui
                    .add_enabled(company_mode, egui::Button::new("Assign task"))
                    .clicked()
            {
                self.work_state.form = Some(Form::new("task_save", self.data.company.today()));
            }
            if self.work_state.report_tab
                && ui
                    .add_enabled(company_mode, egui::Button::new("My daily report"))
                    .clicked()
            {
                let employee = self.data.access.employee_id;
                let day = self
                    .data
                    .attendance
                    .iter()
                    .find(|a| Some(a.employee_id) == employee && a.checked_out_at.is_none())
                    .map_or_else(|| self.data.company.today(), |a| a.work_date);
                let own = context
                    .reports
                    .iter()
                    .find(|r| Some(r.employee_id) == employee && r.work_date == day);
                match own {
                    Some(report) if matches!(report.status.as_str(), "draft" | "returned") => {
                        self.edit_work_report(report);
                    }
                    Some(report) => {
                        if !self
                            .work_state
                            .report_filters
                            .matches(report, self.work_state.employee)
                        {
                            self.work_state.employee = employee;
                            self.work_state.report_filters = ReportFilters {
                                from: Some(day),
                                through: Some(day),
                                ..Default::default()
                            };
                        }
                        self.notice = Some((
                            format!(
                                "Your report for {day} is {}. Open its submission history below.",
                                report.status
                            ),
                            false,
                        ));
                    }
                    None => self.work_state.form = Some(Form::new("report_save", day)),
                }
            }
        });
        let selected_employee = self.work_state.employee;
        if self.work_state.report_tab {
            self.report_filter_controls(ui);
            if company_mode
                && (context.can_administer
                    || self.data.access.allows(crate::staff_roles::Capability::TeamReview))
            {
                self.report_follow_ups(ui, selected_employee);
            }
            let filters = self.work_state.report_filters;
            if !filters.valid_range() {
                ui.label("Choose a through date on or after the from date.");
                return;
            }
            let reports = context
                .reports
                .iter()
                .filter(|r| filters.matches(r, selected_employee));
            let shown = reports.clone().count();
            if shown == 0 {
                ui.label("No daily reports match this selection.");
                return;
            }
            ui.label(format!(
                "Showing {shown} · {} awaiting review · {} returned for revision",
                reports
                    .clone()
                    .filter(|r| r.status == "submitted" && can_review(&self.data, r.employee_id))
                    .count(),
                reports.clone().filter(|r| r.status == "returned").count()
            ));
            for r in reports {
                card(ui, |ui| {
                    ui.strong(format!(
                        "{} · {} · {}",
                        self.data.employee_name(r.employee_id),
                        r.work_date,
                        r.status
                    ));
                    ui.label(&r.body.summary);
                    if !r.review_note.is_empty() {
                        ui.label(format!("Reviewer: {}", r.review_note));
                    }
                    if Some(r.employee_id) == self.data.access.employee_id
                        && matches!(r.status.as_str(), "draft" | "returned")
                        && ui.button("Edit report").clicked()
                    {
                        self.edit_work_report(r);
                    }
                    if r.status == "submitted"
                        && can_review(&self.data, r.employee_id)
                        && ui.button("Review submission").clicked()
                    {
                        let mut f = Form::new("report_review", r.work_date);
                        f.id = r.id;
                        f.revision = r.revision;
                        f.choice = "approved".into();
                        self.work_state.form = Some(f);
                    }
                    let history = egui::CollapsingHeader::new("Submission and decision history")
                        .id_salt(r.id);
                    history.show(ui, |ui| {
                        for s in context.submissions.iter().filter(|s| s.report_id == r.id) {
                            ui.strong(format!(
                                "Submission {} · {}",
                                s.version,
                                s.submitted_at.format("%Y-%m-%d %H:%M UTC")
                            ));
                            for (label, text) in [
                                ("Summary", &s.body.summary),
                                ("Completed", &s.body.completed),
                                ("Unfinished", &s.body.unfinished),
                                ("Blockers", &s.body.blockers),
                                ("Next actions", &s.body.next_steps),
                            ] {
                                ui.label(format!("{label}: {text}"));
                            }
                            for task in &s.task_snapshot {
                                ui.label(format!(
                                    "Saved task: {} · {} · due {}",
                                    task.title, task.status, task.due_on
                                ));
                                ui.label(format!(
                                    "Outcome: {} · Progress: {}",
                                    task.outcome, task.progress
                                ));
                            }
                        }
                        for e in context
                            .events
                            .iter()
                            .filter(|e| e["record_id"].as_str() == Some(r.id.to_string().as_str()))
                        {
                            if e["kind"] == "report_review" {
                                ui.label(format!(
                                    "{} · {} · {}",
                                    e["created_at"].as_str().unwrap_or(""),
                                    e["body"]["request"]["status"].as_str().unwrap_or(""),
                                    e["body"]["request"]["note"].as_str().unwrap_or("")
                                ));
                            }
                        }
                    });
                });
            }
        } else {
            for t in context
                .tasks
                .iter()
                .filter(|t| selected_employee.is_none_or(|id| t.employee_id == id))
            {
                card(ui, |ui| {
                    ui.strong(&t.title);
                    ui.label(format!(
                        "{} · due {} · {} · {}",
                        self.data.employee_name(t.employee_id),
                        t.due_on,
                        t.priority,
                        t.status
                    ));
                    ui.label(&t.outcome);
                    if !t.progress.is_empty() {
                        ui.label(format!("Progress: {}", t.progress));
                    }
                    if !t.review_note.is_empty() {
                        ui.label(format!("Review: {}", t.review_note));
                    }
                    if Some(t.employee_id) == self.data.access.employee_id
                        && t.status != "accepted"
                        && ui.button("Update progress").clicked()
                    {
                        self.edit_work_task(t, false);
                    }
                    if t.status == "submitted"
                        && can_review(&self.data, t.employee_id)
                        && ui.button("Review task").clicked()
                    {
                        self.edit_work_task(t, true);
                    }
                });
            }
        }
    }

    fn report_filter_controls(&mut self, ui: &mut Ui) {
        let filters = &mut self.work_state.report_filters;
        ui.strong("Filter the daily report list");
        ui.horizontal_wrapped(|ui| {
            report_date_filter(
                ui,
                "From",
                &mut filters.from,
                &mut filters.from_month,
                &self.data,
            );
            report_date_filter(
                ui,
                "Through",
                &mut filters.through,
                &mut filters.through_month,
                &self.data,
            );
            let status_label = REPORT_STATUSES
                .iter()
                .find(|(status, _)| *status == filters.status)
                .map_or("All statuses", |(_, label)| *label);
            egui::ComboBox::from_id_salt("daily-report-status")
                .selected_text(status_label)
                .show_ui(ui, |ui| {
                    for (status, label) in REPORT_STATUSES {
                        ui.selectable_value(&mut filters.status, status, label);
                    }
                });
            if (filters.from.is_some() || filters.through.is_some() || filters.status.is_some())
                && ui.button("Clear date and status").clicked()
            {
                *filters = ReportFilters::default();
            }
        });
    }

    fn report_follow_ups(&mut self, ui: &mut Ui, selected_employee: Option<Uuid>) {
        card(ui, |ui| {
            ui.strong("Remote report follow-ups");
            ui.label("Review report status for completed remote shifts. Use the shift start date for overnight work.");
            ui.horizontal_wrapped(|ui| {
                let today = self.data.company.today();
                let date = ui.button(self.attendance_date.format("%b %d, %Y").to_string());
                egui::Popup::from_toggle_button_response(&date).show(|ui| {
                    if attendance_calendar(
                        ui,
                        &mut self.attendance_date,
                        &mut self.calendar_month,
                        today,
                        &self.data,
                    ) {
                        self.work_state.follow_ups = None;
                        ui.close();
                    }
                });
                if ui
                    .add_enabled(!self.busy, egui::Button::new("Load follow-ups"))
                    .clicked()
                {
                    self.work_state.follow_ups = None;
                    self.send(Command::WorkReportFollowUps(self.attendance_date));
                }
            });
            if let Some(follow_ups) = self
                .work_state
                .follow_ups
                .as_ref()
                .filter(|result| result.work_date == self.attendance_date)
            {
                let mut rows = follow_ups
                    .rows
                    .iter()
                    .filter(|row| selected_employee.is_none_or(|id| row.employee_id == id))
                    .peekable();
                if rows.peek().is_none() {
                    ui.label("No report follow-ups for this selection.");
                }
                for row in rows {
                    ui.label(format!(
                        "{} · {}",
                        self.data.employee_name(row.employee_id),
                        row.state.label()
                    ));
                }
            }
        });
    }

    fn edit_work_report(&mut self, r: &DailyReport) {
        let mut f = Form::new("report_save", r.work_date);
        f.id = r.id;
        f.revision = r.revision;
        f.body = r.body.clone();
        f.task_ids = r.task_ids.clone();
        self.work_state.form = Some(f);
    }
    fn edit_work_task(&mut self, t: &Task, review: bool) {
        let mut f = Form::new(
            if review {
                "task_review"
            } else {
                "task_progress"
            },
            t.due_on,
        );
        f.id = t.id;
        f.revision = t.revision;
        f.choice = if review { "accepted" } else { "in_progress" }.into();
        f.note = if review {
            String::new()
        } else {
            t.progress.clone()
        };
        self.work_state.form = Some(f);
    }
    fn work_editor(&mut self, ui: &mut Ui) {
        let mut save = false;
        let mut submit = false;
        let mut discard = false;
        let mut retry = false;
        let company_mode = self.data.mode == Mode::Supabase;
        let pending = self.work_state.pending.is_some();
        let form = self.work_state.form.as_mut().unwrap();
        card(ui, |ui| {
            ui.add_enabled_ui(!self.busy && !pending,|ui|{
                if matches!(form.operation,"policy_save"|"task_save"|"station_pin_reset") {employee_picker(ui,"work-editor-person",&self.data,&mut form.employee);}
                match form.operation {
                    "station_save"=>editor_text(ui,"Station name",&mut form.title,false,120),
                    "station_pin_reset"=>{ui.label("New personal attendance PIN (6–12 digits)");ui.add(egui::TextEdit::singleline(&mut form.note).password(true).char_limit(12).id(egui::Id::unique("work-station-pin")));ui.label("Give this PIN to the employee privately. Saving replaces their previous PIN and clears a PIN lock.");},
                    "policy_save" => {
                        egui::ComboBox::from_id_salt("work-arrangement").selected_text(&form.choice).show_ui(ui,|ui|{for v in ["onsite","remote","hybrid"]{ui.selectable_value(&mut form.choice,v.into(),v);}});
                        editor_text(ui,"Effective from (YYYY-MM-DD)",&mut form.date,false,10);editor_text(ui,"Until (optional)",&mut form.end,false,10);
                        if form.choice=="hybrid"{ui.label("Office weekdays");ui.horizontal_wrapped(|ui|{for(i,d)in["Mon","Tue","Wed","Thu","Fri","Sat","Sun"].iter().enumerate(){ui.checkbox(&mut form.office_days[i],*d);}});}
                        editor_text(ui,"Reason for this version",&mut form.note,true,500);
                        ui.checkbox(&mut form.enabled,"Activate this version from its effective date");ui.label("Leave unchecked to save a draft. Office restrictions require a verified registered kiosk.");
                    },
                    "task_save" => {editor_text(ui,"Task",&mut form.title,false,200);editor_text(ui,"Expected outcome",&mut form.outcome,true,4000);editor_text(ui,"Due work date (YYYY-MM-DD)",&mut form.date,false,10);
                        egui::ComboBox::from_id_salt("work-priority").selected_text(&form.choice).show_ui(ui,|ui|{for v in ["normal","high","urgent"]{ui.selectable_value(&mut form.choice,v.into(),v);}});},
                    "report_save" => {ui.add_enabled_ui(form.revision<0,|ui|editor_text(ui,"Work date (shift start date for overnight work)",&mut form.date,false,10));
                        editor_text(ui,"Work summary",&mut form.body.summary,true,4000);editor_text(ui,"Completed work",&mut form.body.completed,true,4000);
                        editor_text(ui,"Unfinished work",&mut form.body.unfinished,true,4000);editor_text(ui,"Blockers",&mut form.body.blockers,true,4000);editor_text(ui,"Next actions",&mut form.body.next_steps,true,4000);
                        ui.collapsing("Include snapshots of my assigned tasks",|ui|{if let Some(c)=&self.work_state.context{for t in c.tasks.iter().filter(|t|Some(t.employee_id)==self.data.access.employee_id){let mut selected=form.task_ids.contains(&t.id);if ui.checkbox(&mut selected,&t.title).changed(){if selected{form.task_ids.push(t.id);}else{form.task_ids.retain(|id|*id!=t.id);}}}}});},
                    op => {let choices:&[&str]=match op{"task_progress"=>&["in_progress","submitted"],"task_review"=>&["accepted","returned"],_=>&["approved","returned"]};
                        egui::ComboBox::from_id_salt("work-decision").selected_text(&form.choice).show_ui(ui,|ui|{for v in choices{ui.selectable_value(&mut form.choice,(*v).into(),*v);}});
                        editor_text(ui,"Progress or review note",&mut form.note,true,2000);},
                }
                ui.horizontal_wrapped(|ui|{save=ui.add_enabled(company_mode,egui::Button::new(if form.operation=="report_save"{"Save draft"}else{"Save"})).clicked();if form.operation=="report_save"{submit=ui.add_enabled(company_mode,egui::Button::new("Submit to manager")).clicked();}});
            });
            if pending && !self.busy {
                ui.label("The previous save is awaiting confirmation. Retry the same request before editing it.");
                retry = ui.button("Retry same save").clicked();
            }
            discard = ui
                .add_enabled(
                    !self.busy && (!pending || self.work_state.rejected),
                    egui::Button::new(if pending {
                        "Rejected: close and refresh"
                    } else {
                        "Discard draft"
                    }),
                )
                .clicked();
            if pending && !self.work_state.rejected {
                ui.label("Keep this request until its receipt is confirmed. It cannot be discarded while delivery is uncertain.");
            }
        });
        let request = if save || submit {
            match form.request(submit) {
                Ok(v) => Some(v),
                Err(e) => {
                    self.notice = Some((e.to_string(), true));
                    None
                }
            }
        } else if retry {
            self.work_state.pending.clone()
        } else {
            None
        };
        if let Some(request) = request {
            self.work_state.rejected = false;
            self.work_state.pending = Some(request.clone());
            self.send(Command::WorkActivity(Some(request)));
        }
        if discard {
            self.work_state.form = None;
            self.work_state.pending = None;
            self.work_state.rejected = false;
            clear_undo(ui.ctx());
            self.send(Command::WorkActivity(None));
        }
    }
}
pub(super) fn clear_undo(ctx: &egui::Context) {
    clear_edit_history(
        ctx,
        &[
            "work-station-pin",
            "work-Station name",
            "work-Effective from (YYYY-MM-DD)",
            "work-Until (optional)",
            "work-Reason for this version",
            "work-Task",
            "work-Expected outcome",
            "work-Due work date (YYYY-MM-DD)",
            "work-Work date (shift start date for overnight work)",
            "work-Work summary",
            "work-Completed work",
            "work-Unfinished work",
            "work-Blockers",
            "work-Next actions",
            "work-Progress or review note",
        ],
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn report_filters_include_both_work_dates_and_compose_with_status_and_employee() {
        let from = NaiveDate::from_ymd_opt(2026, 10, 26).unwrap();
        let through = from + Duration::days(6);
        let employee = Uuid::from_u128(1);
        let other_employee = Uuid::from_u128(2);
        let mut report = DailyReport {
            id: Uuid::from_u128(3),
            company_id: Uuid::from_u128(4),
            employee_id: employee,
            work_date: from,
            status: "submitted".into(),
            revision: 1,
            submission: 1,
            body: ReportBody::default(),
            task_ids: vec![],
            review_note: String::new(),
            updated_at: (through + Duration::days(1)).and_hms_opt(0, 0, 0).unwrap().and_utc(),
        };
        let mut filters = ReportFilters {
            from: Some(from),
            through: Some(through),
            status: Some("submitted"),
            ..Default::default()
        };
        assert!(filters.valid_range());
        assert!(filters.matches(&report, Some(employee)));
        report.work_date = through;
        assert!(filters.matches(&report, Some(employee)));
        assert!(!filters.matches(&report, Some(other_employee)));
        report.work_date = through + Duration::days(1);
        assert!(!filters.matches(&report, Some(employee)));
        filters.through = None;
        assert!(filters.matches(&report, None));
        report.work_date = from - Duration::days(1);
        assert!(!filters.matches(&report, None));
        filters.through = Some(from - Duration::days(1));
        assert!(!filters.valid_range());
        assert!(!filters.matches(&report, None));
        filters.from = None;
        assert!(filters.matches(&report, None));
        report.status = "returned".into();
        assert!(!filters.matches(&report, None));
        assert!(ReportFilters::default().matches(&report, None));
    }
}
