use super::*;
use crate::work_activity::{Context, DailyReport, ReportBody, ReportFollowUps, Submission, Task, can_review};
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
    pub overview_reports_requested: bool,
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
            "report_review" => {
                anyhow::ensure!(
                    matches!(self.choice.as_str(), "approved" | "returned"),
                    "Choose Approve or Return for revision."
                );
                let note = self.note.trim();
                anyhow::ensure!(
                    (1..=2000).contains(&note.chars().count()),
                    "Enter a review note of 1 to 2,000 characters."
                );
                v["status"] = json!(self.choice);
                v["note"] = json!(note);
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
    pub(super) fn show_reports(&mut self) {
        self.report_tab = true;
    }
    pub(super) fn reviewing_report(&self) -> Option<Uuid> {
        self.form
            .as_ref()
            .filter(|form| form.operation == "report_review")
            .map(|form| form.id)
    }
    pub(super) fn showing_queue_report(&self, id: Uuid) -> bool {
        self.report_tab && (self.form.is_none() || self.reviewing_report() == Some(id))
    }
    pub(super) fn queue_review_has_draft(&self) -> bool {
        self.pending.is_some() || self.form.as_ref().is_some_and(|form| !form.note.is_empty())
    }
    pub(super) fn close_queue_review(&mut self, ctx: &egui::Context) {
        self.form = None;
        clear_edit_history(ctx, &["work-Review note (required)"]);
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
        self.overview_reports_requested = true;
        Ok(())
    }
}

fn review_submission<'a>(
    context: &'a Context,
    data: &Snapshot,
    id: Uuid,
) -> Option<(&'a DailyReport, &'a Submission)> {
    if context.company_id != data.company.id
        || context.user_id != data.access.user_id
        || Some(context.employee_id) != data.access.employee_id
        || context.own_only
    {
        return None;
    }
    let report = context.reports.iter().find(|report| {
        report.id == id
            && report.company_id == data.company.id
            && report.status == "submitted"
            && can_review(data, report.employee_id)
    })?;
    let submission = context.submissions.iter().find(|submission| {
        submission.report_id == report.id
            && submission.version == report.submission
            && submission.employee_id == report.employee_id
            && submission.company_id == report.company_id
    })?;
    Some((report, submission))
}

fn submission_details(ui: &mut Ui, submission: &Submission) {
    for (label, text) in [
        ("Summary", &submission.body.summary),
        ("Completed work", &submission.body.completed),
        ("Unfinished work", &submission.body.unfinished),
        ("Blockers", &submission.body.blockers),
        ("Next actions", &submission.body.next_steps),
    ] {
        if !text.trim().is_empty() {
            ui.strong(label);
            ui.label(text);
        }
    }
    if !submission.task_snapshot.is_empty() {
        ui.strong("Submitted task snapshots");
        for task in &submission.task_snapshot {
            ui.label(format!("{} · {} · due {}", task.title, task.status, task.due_on));
            ui.label(&task.outcome);
            if !task.progress.is_empty() {
                ui.label(&task.progress);
            }
        }
    }
}

fn report_decisions(
    ui: &mut Ui,
    context: &Context,
    report_id: Uuid,
    company: &Company,
    time_format: &str,
) {
    let record_id = report_id.to_string();
    let mut decisions = context
        .events
        .iter()
        .filter(|event| {
            event["record_id"].as_str() == Some(record_id.as_str())
                && event["kind"] == "report_review"
        })
        .peekable();
    if decisions.peek().is_none() {
        ui.label("No prior decisions in this view.");
    }
    for decision in decisions {
        let status = match decision["body"]["request"]["status"].as_str() {
            Some("approved") => "Approved",
            Some("returned") => "Returned for revision",
            _ => "Review decision",
        };
        let raw_time = decision["created_at"].as_str().unwrap_or_default();
        let local_time = chrono::DateTime::parse_from_rfc3339(raw_time).map_or_else(
            |_| raw_time.to_owned(),
            |time| {
                time.with_timezone(&company.tz())
                    .format(&format!("%b %d, %Y {time_format}"))
                    .to_string()
            },
        );
        ui.label(format!("{status} · {local_time}"));
        if let Some(note) = decision["body"]["request"]["note"].as_str() {
            ui.label(note);
        }
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
                        self.open_daily_report_review(r.id, ui.ctx());
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
                            submission_details(ui, s);
                        }
                        report_decisions(ui, &context, r.id, &self.data.company, self.time_format());
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
        let today = self.data.company.today();
        ui.strong("Filter the daily report list");
        ui.horizontal_wrapped(|ui| {
            optional_date_filter(
                ui,
                "From",
                &mut filters.from,
                &mut filters.from_month,
                today,
                today,
                Some(&self.data.attendance),
            );
            optional_date_filter(
                ui,
                "Through",
                &mut filters.through,
                &mut filters.through_month,
                today,
                today,
                Some(&self.data.attendance),
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

    pub(super) fn open_daily_report_review(&mut self, id: Uuid, ctx: &egui::Context) {
        if self.busy || self.work_state.editing() {
            return;
        }
        let Some((report, _)) = self
            .work_state
            .context
            .as_ref()
            .and_then(|context| review_submission(context, &self.data, id))
        else {
            self.notice = Some((
                "Refresh work records to see the current submission and review access.".into(),
                true,
            ));
            return;
        };
        let mut form = Form::new("report_review", report.work_date);
        form.id = id;
        form.revision = report.revision;
        self.navigate(Page::Attendance, ctx);
        if self.page == Page::Attendance {
            self.shift_state.tab = shifts_view::Tab::DailyWork;
            self.work_state.show_reports();
            self.work_state.form = Some(form);
            self.notice = None;
        }
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
        let time_format = self.time_format();
        let from_queue = self.work_state.reviewing_report().is_some_and(|id| {
            self.in_review_queue(&crate::notifications::Target::DailyReport(id))
        });
        let form = self.work_state.form.as_mut().unwrap();
        let report_review = form.operation == "report_review";
        let review = self.work_state.context.as_ref().and_then(|context| {
            if !report_review {
                return None;
            }
            review_submission(context, &self.data, form.id)
                .filter(|(report, _)| report.revision == form.revision)
                .map(|(report, submission)| (context, report, submission))
        });
        let review_available = review.is_some();
        card(ui, |ui| {
            if report_review {
                ui.heading("Review daily report");
                if let Some((context, report, submission)) = review {
                    ui.strong(format!(
                        "{} · {}",
                        self.data.employee_name(report.employee_id),
                        report.work_date,
                    ));
                    ui.label(format!(
                        "Submission {} · {}",
                        submission.version,
                        submission.submitted_at.with_timezone(&self.data.company.tz())
                            .format(&format!("%b %d, %Y {time_format}")),
                    ));
                    ui.separator();
                    submission_details(ui, submission);
                    ui.separator();
                    ui.strong("Prior decisions");
                    report_decisions(ui, context, report.id, &self.data.company, time_format);
                    ui.separator();
                } else if !pending {
                    ui.label("Refresh work records to review the current submission.");
                }
            }
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
                    "report_review" => {
                        if review.is_some() {
                            editor_text(ui, "Review note (required)", &mut form.note, true, 2000);
                            ui.horizontal_wrapped(|ui| {
                                let ready = company_mode && !form.note.trim().is_empty();
                                for (label, decision) in [("Approve", "approved"), ("Return for revision", "returned")] {
                                    if ui.add_enabled(ready, egui::Button::new(label)).clicked() {
                                        form.choice = decision.into();
                                        save = true;
                                    }
                                }
                            });
                        }
                    }
                    op => {let choices:&[&str]=match op{"task_progress"=>&["in_progress","submitted"],"task_review"=>&["accepted","returned"],_=>&["approved","returned"]};
                        egui::ComboBox::from_id_salt("work-decision").selected_text(&form.choice).show_ui(ui,|ui|{for v in choices{ui.selectable_value(&mut form.choice,(*v).into(),*v);}});
                        editor_text(ui,"Progress or review note",&mut form.note,true,2000);},
                }
                if !report_review {
                    ui.horizontal_wrapped(|ui|{save=ui.add_enabled(company_mode,egui::Button::new(if form.operation=="report_save"{"Save draft"}else{"Save"})).clicked();if form.operation=="report_save"{submit=ui.add_enabled(company_mode,egui::Button::new("Submit to manager")).clicked();}});
                }
            });
            if pending && !self.busy {
                ui.label("The previous save is awaiting confirmation. Retry the same request before editing it.");
                retry = ui.button("Retry same save").clicked();
            }
            if from_queue && !pending {
                if !form.note.is_empty()
                    && ui.add_enabled(!self.busy, egui::Button::new("Discard review note")).clicked()
                {
                    form.note.clear();
                    clear_edit_history(ui.ctx(), &["work-Review note (required)"]);
                }
            } else {
                discard = ui
                    .add_enabled(
                        !self.busy && (!pending || self.work_state.rejected),
                        egui::Button::new(if pending {
                            "Rejected: close and refresh"
                        } else if report_review {
                            "Back to reports"
                        } else {
                            "Discard draft"
                        }),
                    )
                    .clicked();
            }
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
            let reload = !report_review || !review_available || self.work_state.rejected;
            self.work_state.form = None;
            self.work_state.pending = None;
            self.work_state.rejected = false;
            clear_undo(ui.ctx());
            if reload {
                self.send(Command::WorkActivity(None));
            }
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
            "work-Review note (required)",
        ],
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn review_uses_the_exact_saved_submission_and_current_team_scope() {
        let mut data = DemoStore::new().data;
        let reviewer = data.employees[3].id;
        let employee = data.employees[1].id;
        data.access.role = Role::Manager;
        data.access.employee_id = Some(reviewer);
        data.employees[1].reports_to = Some(reviewer);
        let date = data.company.today();
        let time = date.and_hms_opt(9, 0, 0).unwrap().and_utc();
        let id = Uuid::from_u128(40);
        let mut context = Context::empty(&data);
        context.reports.push(DailyReport {
            id,
            company_id: data.company.id,
            employee_id: employee,
            work_date: date,
            status: "submitted".into(),
            revision: 3,
            submission: 2,
            body: ReportBody { summary: "Editable report".into(), ..Default::default() },
            task_ids: vec![],
            review_note: String::new(),
            updated_at: time,
        });
        let first = Submission {
            report_id: id,
            version: 1,
            company_id: data.company.id,
            employee_id: employee,
            body: ReportBody { summary: "First submission".into(), ..Default::default() },
            task_snapshot: vec![],
            submitted_by: Uuid::from_u128(41),
            submitted_at: time,
        };
        context.submissions.push(first.clone());
        assert!(review_submission(&context, &data, id).is_none());
        context.submissions.push(Submission {
            version: 2,
            body: ReportBody { summary: "Revised submission".into(), ..Default::default() },
            ..first
        });
        let (report, saved) = review_submission(&context, &data, id).unwrap();
        assert_eq!(report.revision, 3);
        assert_eq!(saved.body.summary, "Revised submission");
        context.own_only = true;
        assert!(review_submission(&context, &data, id).is_none());
        context.own_only = false;
        data.employees[1].reports_to = None;
        assert!(review_submission(&context, &data, id).is_none());
    }

    #[test]
    fn review_request_requires_a_decision_and_note_for_the_selected_revision() {
        let date = NaiveDate::from_ymd_opt(2026, 11, 6).unwrap();
        let mut form = Form::new("report_review", date);
        form.revision = 3;
        form.note = "Checked the saved task snapshot".into();
        assert!(form.request(false).is_err());
        for decision in ["approved", "returned"] {
            form.choice = decision.into();
            form.note = "   ".into();
            assert!(form.request(false).is_err());
            form.note = " Checked the saved task snapshot ".into();
            let request = form.request(false).unwrap();
            assert_eq!(request["id"], form.id.to_string());
            assert_eq!(request["expected_revision"], 3);
            assert_eq!(request["operation"], "report_review");
            assert_eq!(request["status"], decision);
            assert_eq!(request["note"], "Checked the saved task snapshot");
        }
    }

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
