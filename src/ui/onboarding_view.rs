use super::*;
use crate::onboarding::{self, HireDraft, HiringAction, OfferDraft, OfferStatus, TaskDraft};
#[derive(Default)]
pub(super) struct State {
    pub active: bool,
    pub offer_form: Option<OfferDraft>,
    pub hire_form: Option<HireDraft>,
    pub task_form: Option<TaskDraft>,
    pub decision: Option<HiringAction>,
    pub pending: Option<HiringAction>,
    pub case_id: Option<Uuid>,
    pub show_completed: bool,
    pub search: String,
}
impl State {
    pub fn editing(&self) -> bool {
        self.offer_form.is_some()
            || self.hire_form.is_some()
            || self.task_form.is_some()
            || self.decision.is_some()
    }
    pub fn accept(&mut self, data: &Snapshot) -> bool {
        let Some(a) = self.pending.take() else {
            return true;
        };
        if !a.saved(data) {
            return false;
        }
        if let HiringAction::Hire(d) = &a {
            self.case_id = Some(d.id);
        }
        self.offer_form = None;
        self.hire_form = None;
        self.task_form = None;
        self.decision = None;
        true
    }
}
fn reason_mut(a: &mut HiringAction) -> &mut String {
    match a {
        HiringAction::OfferSave(d) => &mut d.reason,
        HiringAction::Hire(d) => &mut d.reason,
        HiringAction::TaskSave(d) => &mut d.reason,
        HiringAction::OfferStatus { reason, .. }
        | HiringAction::TaskStatus { reason, .. }
        | HiringAction::CaseStatus { reason, .. } => reason,
    }
}
fn decision_title(a: &HiringAction) -> String {
    match a {
        HiringAction::OfferStatus { status, .. } => match status {
            OfferStatus::Approved => "Approve and lock these offer terms",
            OfferStatus::Sent => "Record external offer delivery",
            OfferStatus::Accepted => "Record candidate acceptance",
            OfferStatus::Declined => "Record candidate decline",
            OfferStatus::Withdrawn => "Withdraw this offer",
            _ => "Update offer",
        }
        .into(),
        HiringAction::TaskStatus { status, .. } => format!("Set task to {status}"),
        HiringAction::CaseStatus { status, .. } => format!("Set onboarding to {status}"),
        _ => "Save hiring change".into(),
    }
}
impl HrisApp {
    fn send_onboarding(&mut self, a: HiringAction) {
        self.onboarding_state.pending = Some(a.clone());
        self.send(Command::Change(Action::Onboarding(a)));
    }
    fn hiring_decision(&mut self, ui: &mut Ui) -> bool {
        let Some(mut a) = self.onboarding_state.decision.take() else {
            return false;
        };
        let mut discard = false;
        let mut save = false;
        card(ui, |ui| {
            ui.label(RichText::new(decision_title(&a)).strong().size(18.0));
            if matches!(
                a,
                HiringAction::OfferStatus {
                    status: OfferStatus::Sent,
                    ..
                }
            ) {
                ui.label("Record delivery after sending the offer through your company’s chosen channel.");
            }
            if matches!(
                a,
                HiringAction::OfferStatus {
                    status: OfferStatus::Accepted,
                    ..
                }
            ) {
                ui.label("Record acceptance only after receiving the candidate’s confirmation. Hiring is a separate step.");
            }
            field(
                ui,
                "Reason / evidence reference *",
                reason_mut(&mut a),
                "Keep the reason specific; original history is retained",
            );
            let valid = a.payload(&self.data);
            if let Err(e) = &valid {
                ui.label(RichText::new(e.to_string()).color(muted(ui)));
            }
            ui.horizontal_wrapped(|ui| {
                save = ui
                    .add_enabled(valid.is_ok(), egui::Button::new("Confirm and save"))
                    .clicked();
                discard = ui.button("Cancel change").clicked();
            });
        });
        if !discard {
            self.onboarding_state.decision = Some(a.clone());
        }
        if save {
            self.send_onboarding(a);
        }
        true
    }
    pub(super) fn applicant_offers(&mut self, ui: &mut Ui, applicant: Uuid) {
        if !self.data.access.role.manages_hr() {
            return;
        }
        if self.hiring_decision(ui) {
            return;
        }
        if self.onboarding_state.offer_form.is_some() {
            self.offer_form(ui);
            return;
        }
        if self.onboarding_state.hire_form.is_some() {
            self.hire_form(ui);
            return;
        }
        let converted = self
            .data
            .onboarding
            .conversions
            .iter()
            .find(|c| c.applicant_id == applicant)
            .cloned();
        let offers: Vec<_> = self
            .data
            .onboarding
            .offers
            .iter()
            .filter(|r| r.applicant_id == applicant)
            .cloned()
            .collect();
        card(ui, |ui| {
            ui.label(
                RichText::new("Offers and approved hiring")
                    .strong()
                    .size(19.0),
            );
            ui.label("HR prepares terms. An Administrator approves them before delivery. Record delivery and the candidate’s response, then create the employee and onboarding checklist.");
            let available = converted.is_none()
                && offers.iter().all(|r| !r.status.active())
                && self
                    .data
                    .hiring
                    .records
                    .iter()
                    .any(|a| a.id == applicant && !a.archived && !a.stage.terminal());
            if ui
                .add_enabled(available, egui::Button::new("Prepare offer"))
                .clicked()
            {
                self.onboarding_state.offer_form = Some(OfferDraft::new(&self.data, applicant));
            }
            if let Some(c) = &converted {
                ui.label(format!(
                    "Hired · {}",
                    self.data.employee_name(c.employee_id)
                ));
                if ui.button("Open onboarding checklist").clicked() {
                    self.onboarding_state.case_id = Some(c.id);
                    self.hiring_state.tab = super::hiring_view::Tab::Onboarding;
                }
            }
        });
        ui.add_space(10.0);
        if offers.is_empty() {
            empty(ui, "No offer has been prepared for this application.");
        }
        for r in offers.iter().rev() {
            card(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.label(RichText::new(&r.job_title).strong().size(18.0));
                    badge(ui, r.status.label(), r.status.active());
                    ui.label(format!("Revision {}", r.revision));
                });
                egui::Grid::new(("offer_summary", r.id))
                    .num_columns(2)
                    .spacing([25.0, 10.0])
                    .show(ui, |ui| {
                        for (label, value) in [
                            ("Department", r.department.clone()),
                            ("Start date", r.starts_on.to_string()),
                            (
                                "Base amount",
                                format!(
                                    "{} {} / {}",
                                    r.currency,
                                    onboarding::money(r.amount_minor),
                                    r.pay_period
                                ),
                            ),
                            (
                                "Expires",
                                r.expires_on
                                    .map(|d| d.to_string())
                                    .unwrap_or_else(|| "Not set".into()),
                            ),
                            (
                                "Approved",
                                r.approved_at
                                    .map(|d| {
                                        d.with_timezone(&self.data.company.tz())
                                            .format("%b %d, %Y %H:%M")
                                            .to_string()
                                    })
                                    .unwrap_or_else(|| "Awaiting Administrator".into()),
                            ),
                        ] {
                            ui.label(label);
                            ui.label(value);
                            ui.end_row();
                        }
                    });
                if !r.terms.is_empty() {
                    ui.label(RichText::new("Terms and benefits").strong());
                    ui.label(&r.terms);
                }
                if converted.is_none() {
                    ui.horizontal_wrapped(|ui| {
                        if r.status == OfferStatus::Draft && ui.button("Edit draft").clicked() {
                            self.onboarding_state.offer_form = Some(OfferDraft::existing(r));
                        }
                        for (status, label) in [
                            (OfferStatus::Approved, "Approve terms"),
                            (OfferStatus::Sent, "Record delivery"),
                            (OfferStatus::Accepted, "Record acceptance"),
                            (OfferStatus::Declined, "Record decline"),
                            (OfferStatus::Withdrawn, "Withdraw offer"),
                        ] {
                            let a = HiringAction::OfferStatus {
                                id: r.id,
                                revision: r.revision,
                                status,
                                reason: "Preview availability".into(),
                            };
                            if a.payload(&self.data).is_ok() && ui.button(label).clicked() {
                                self.onboarding_state.decision = Some(HiringAction::OfferStatus {
                                    id: r.id,
                                    revision: r.revision,
                                    status,
                                    reason: String::new(),
                                });
                            }
                        }
                        if r.status == OfferStatus::Accepted
                            && ui.button("Hire and start onboarding").clicked()
                        {
                            self.onboarding_state.hire_form = Some(HireDraft::new(r));
                        }
                    });
                }
                let events: Vec<_> = self
                    .data
                    .onboarding
                    .offer_events
                    .iter()
                    .filter(|e| e.offer_id == Some(r.id))
                    .cloned()
                    .collect();
                egui::CollapsingHeader::new(format!("Offer history ({})", events.len()))
                    .id_salt((r.id, "offer_history"))
                    .default_open(history_preview())
                    .show(ui, |ui| {
                        self.hiring_events(ui, &events);
                    });
            });
            ui.add_space(10.0);
        }
    }
    fn offer_form(&mut self, ui: &mut Ui) {
        let Some(mut d) = self.onboarding_state.offer_form.take() else {
            return;
        };
        let mut save = false;
        let mut discard = false;
        card(ui, |ui| {
            ui.label(
                RichText::new(if d.original.is_some() {
                    "Edit draft offer"
                } else {
                    "Prepare offer"
                })
                .strong()
                .size(19.0),
            );
            ui.label("Amounts use the selected currency and pay period. Payroll calculations and statutory deductions are separate.");
            field(ui, "Job title *", &mut d.job_title, "");
            field(ui, "Department", &mut d.department, "");
            ui.columns(2, |c| {
                field(&mut c[0], "Start date *", &mut d.starts_on, "YYYY-MM-DD");
                field(
                    &mut c[1],
                    "Expiry date (optional)",
                    &mut d.expires_on,
                    "YYYY-MM-DD",
                );
            });
            ui.columns(2, |c| {
                field(&mut c[0], "Base amount *", &mut d.amount, "25000.00");
                field(&mut c[1], "Currency *", &mut d.currency, "PHP");
            });
            ui.label("Pay period");
            egui::ComboBox::from_id_salt("offer_pay_period")
                .selected_text(&d.pay_period)
                .show_ui(ui, |ui| {
                    for period in ["monthly", "annual", "hourly"] {
                        ui.selectable_value(&mut d.pay_period, period.into(), period);
                    }
                });
            ui.label("Terms and benefits (up to 4,000 characters)");
            ui.add(
                egui::TextEdit::multiline(&mut d.terms)
                    .desired_width(f32::INFINITY)
                    .desired_rows(4),
            );
            field(ui, "Save reason *", &mut d.reason, "");
            let valid = HiringAction::OfferSave(d.clone()).payload(&self.data);
            if let Err(e) = &valid {
                ui.label(RichText::new(e.to_string()).color(muted(ui)));
            }
            ui.horizontal_wrapped(|ui| {
                save = ui
                    .add_enabled(valid.is_ok(), egui::Button::new("Save offer draft"))
                    .clicked();
                discard = ui.button("Discard offer draft").clicked();
            });
        });
        if !discard {
            self.onboarding_state.offer_form = Some(d.clone());
        }
        if save {
            self.send_onboarding(HiringAction::OfferSave(d));
        }
    }
    fn hire_form(&mut self, ui: &mut Ui) {
        let Some(mut d) = self.onboarding_state.hire_form.take() else {
            return;
        };
        let mut save = false;
        let mut discard = false;
        card(ui, |ui| {
            ui.label(
                RichText::new("Hire and start onboarding")
                    .strong()
                    .size(19.0),
            );
            if let Ok(r) = onboarding::offer(&self.data, d.offer_id) {
                ui.label(format!(
                    "{} · {} · Starts {}",
                    r.applicant_name, r.job_title, r.starts_on
                ));
            }
            ui.label("Creates one employee with the approved job, department and start date, copies applicant contact details and starts the checklist. Desktop/portal access can be provisioned later through Account Manager.");
            field(
                ui,
                "Employee code *",
                &mut d.employee_code,
                "e.g. EMP-00050",
            );
            ui.label("Reports to");
            egui::ComboBox::from_id_salt("hire_reports_to")
                .width(260.0)
                .selected_text(
                    d.reports_to
                        .map(|id| self.data.employee_name(id))
                        .unwrap_or("Not set"),
                )
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut d.reports_to, None, "Not set");
                    for e in self.data.active() {
                        ui.selectable_value(&mut d.reports_to, Some(e.id), &e.full_name);
                    }
                });
            employment_assignment(
                ui,
                EmploymentKind::EmploymentType,
                &self.data,
                &mut d.employment_type_id,
                None,
            );
            employment_assignment(
                ui,
                EmploymentKind::WorkLocation,
                &self.data,
                &mut d.work_location_id,
                None,
            );
            field(ui, "Hiring reason *", &mut d.reason, "");
            let valid = HiringAction::Hire(d.clone()).payload(&self.data);
            if let Err(e) = &valid {
                ui.label(RichText::new(e.to_string()).color(muted(ui)));
            }
            ui.horizontal_wrapped(|ui| {
                save = ui
                    .add_enabled(
                        valid.is_ok(),
                        egui::Button::new("Create employee and onboarding"),
                    )
                    .clicked();
                discard = ui.button("Cancel hiring draft").clicked();
            });
        });
        if !discard {
            self.onboarding_state.hire_form = Some(d.clone());
        }
        if save {
            self.send_onboarding(HiringAction::Hire(d));
        }
    }
    pub(super) fn onboarding(&mut self, ui: &mut Ui, applicant: Option<Uuid>) {
        if self.hiring_decision(ui) {
            return;
        }
        if self.onboarding_state.task_form.is_some() {
            self.onboarding_task_form(ui);
            return;
        }
        let hr = self.data.access.role.manages_hr();
        if !hr {
            self.assigned_onboarding(ui);
            return;
        }
        let app_case = applicant.and_then(|id| {
            self.data
                .onboarding
                .conversions
                .iter()
                .find(|c| c.applicant_id == id)
                .map(|c| c.id)
        });
        if applicant.is_some() && app_case.is_none() {
            empty(
                ui,
                "Onboarding starts after an accepted offer is hired from Offers.",
            );
            return;
        }
        let selected = app_case.or(self.onboarding_state.case_id);
        if let Some(id) = selected {
            if let Some(c) = self
                .data
                .onboarding
                .cases
                .iter()
                .find(|c| c.id == id)
                .cloned()
            {
                card(ui, |ui| {
                    if applicant.is_none() && ui.button("< All onboarding").clicked() {
                        self.onboarding_state.case_id = None;
                    }
                    ui.label(
                        RichText::new(format!("{} · {}", c.employee_code, c.employee_name))
                            .strong()
                            .size(20.0),
                    );
                    badge(ui, &c.status, c.status == "open");
                    ui.label(format!("Start date {}", c.starts_on));
                    ui.horizontal_wrapped(|ui| {
                        if ui
                            .add_enabled(c.status == "open", egui::Button::new("Add task"))
                            .clicked()
                        {
                            self.onboarding_state.task_form = Some(TaskDraft::new(c.id));
                        }
                        let status = if c.status == "open" {
                            "completed"
                        } else {
                            "open"
                        };
                        let a = HiringAction::CaseStatus {
                            id: c.id,
                            revision: c.revision,
                            status: status.into(),
                            reason: "Preview availability".into(),
                        };
                        if ui
                            .add_enabled(
                                a.payload(&self.data).is_ok(),
                                egui::Button::new(if status == "completed" {
                                    "Complete onboarding"
                                } else {
                                    "Reopen onboarding"
                                }),
                            )
                            .clicked()
                        {
                            self.onboarding_state.decision = Some(HiringAction::CaseStatus {
                                id: c.id,
                                revision: c.revision,
                                status: status.into(),
                                reason: String::new(),
                            });
                        }
                    });
                    let all = self
                        .data
                        .onboarding
                        .tasks
                        .iter()
                        .filter(|t| t.case_id == id);
                    let required = all.clone().filter(|t| t.required).count();
                    let done = all.filter(|t| t.required && t.status != "open").count();
                    ui.label(format!("{done} / {required} required tasks resolved"));
                });
                let tasks: Vec<_> = self
                    .data
                    .onboarding
                    .tasks
                    .iter()
                    .filter(|t| t.case_id == id)
                    .cloned()
                    .collect();
                for t in tasks {
                    ui.add_space(10.0);
                    self.onboarding_task_card(ui, &t);
                }
                let events: Vec<_> = self
                    .data
                    .onboarding
                    .events
                    .iter()
                    .filter(|e| e.case_id == Some(id))
                    .cloned()
                    .collect();
                ui.add_space(10.0);
                card(ui, |ui| {
                    egui::CollapsingHeader::new("Onboarding history")
                        .default_open(history_preview())
                        .show(ui, |ui| {
                            self.hiring_events(ui, &events);
                        });
                });
                return;
            }
        }
        card(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                if ui.button("< Applicants").clicked() {
                    self.onboarding_state.active = false;
                }
                ui.label(RichText::new("Onboarding").strong().size(20.0));
                ui.checkbox(
                    &mut self.onboarding_state.show_completed,
                    "Include completed",
                );
            });
            ui.add(
                egui::TextEdit::singleline(&mut self.onboarding_state.search)
                    .hint_text("Search employee code or name")
                    .desired_width(350.0),
            );
        });
        let query = self.onboarding_state.search.to_lowercase();
        let cases: Vec<_> = self
            .data
            .onboarding
            .cases
            .iter()
            .filter(|c| {
                (self.onboarding_state.show_completed || c.status == "open")
                    && format!("{} {}", c.employee_code, c.employee_name)
                        .to_lowercase()
                        .contains(&query)
            })
            .cloned()
            .collect();
        if cases.is_empty() {
            empty(ui, "No matching onboarding cases.");
        }
        for c in cases {
            ui.add_space(10.0);
            card(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    if ui
                        .button(format!("{} · {}", c.employee_code, c.employee_name))
                        .clicked()
                    {
                        self.onboarding_state.case_id = Some(c.id);
                    }
                    badge(ui, &c.status, c.status == "open");
                    ui.label(format!("Starts {}", c.starts_on));
                });
            });
        }
    }
    fn assigned_onboarding(&mut self, ui: &mut Ui) {
        card(ui, |ui| {
            ui.label(RichText::new("Your onboarding tasks").strong().size(20.0));
            ui.label("Complete assigned tasks and record a reason. HR manages assignments, waivers and overall completion.");
            ui.checkbox(
                &mut self.onboarding_state.show_completed,
                "Include resolved / completed onboarding",
            );
        });
        let tasks: Vec<_> = self
            .data
            .onboarding
            .tasks
            .iter()
            .filter(|t| {
                t.assigned_employee_id == self.data.access.employee_id
                    && (self.onboarding_state.show_completed
                        || (t.status == "open" && t.case_status == "open"))
            })
            .cloned()
            .collect();
        if tasks.is_empty() {
            empty(ui, "No assigned onboarding tasks match this view.");
        }
        for t in tasks {
            ui.add_space(10.0);
            self.onboarding_task_card(ui, &t);
        }
    }
    fn onboarding_task_card(&mut self, ui: &mut Ui, t: &onboarding::Task) {
        let hr = self.data.access.role.manages_hr();
        card(ui, |ui| {
            ui.label(RichText::new(&t.title).strong().size(17.0));
            ui.horizontal_wrapped(|ui| {
                badge(ui, &t.status, t.status == "done");
                if t.required {
                    ui.label("Required");
                } else {
                    ui.label("Optional");
                }
            });
            if !hr {
                ui.label(format!(
                    "{} · {} · Starts {}",
                    t.employee_code, t.employee_name, t.starts_on
                ));
            }
            ui.label(format!(
                "Assigned: {} · Due: {}",
                t.assigned_employee_id
                    .map(|id| self.data.employee_name(id))
                    .unwrap_or("Unassigned"),
                t.due_on
                    .map(|d| d.to_string())
                    .unwrap_or_else(|| "Not set".into())
            ));
            if !t.instructions.is_empty() {
                ui.label(&t.instructions);
            }
            if t.case_status == "open" {
                ui.horizontal_wrapped(|ui| {
                    if hr && t.status == "open" && ui.button("Edit / assign").clicked() {
                        self.onboarding_state.task_form = Some(TaskDraft::existing(t));
                    }
                    for (status, label) in [
                        ("done", "Mark done"),
                        ("open", "Reopen task"),
                        ("waived", "Waive task"),
                    ] {
                        let a = HiringAction::TaskStatus {
                            id: t.id,
                            revision: t.revision,
                            status: status.into(),
                            reason: "Preview availability".into(),
                        };
                        if a.payload(&self.data).is_ok() && ui.button(label).clicked() {
                            self.onboarding_state.decision = Some(HiringAction::TaskStatus {
                                id: t.id,
                                revision: t.revision,
                                status: status.into(),
                                reason: String::new(),
                            });
                        }
                    }
                });
            } else {
                ui.label("HR has completed this onboarding checklist.");
            }
            let events: Vec<_> = self
                .data
                .onboarding
                .events
                .iter()
                .filter(|e| e.task_id == Some(t.id))
                .cloned()
                .collect();
            egui::CollapsingHeader::new(format!("Task history ({})", events.len()))
                .id_salt((t.id, "history"))
                .show(ui, |ui| {
                    self.hiring_events(ui, &events);
                });
        });
    }
    fn onboarding_task_form(&mut self, ui: &mut Ui) {
        let Some(mut d) = self.onboarding_state.task_form.take() else {
            return;
        };
        let mut save = false;
        let mut discard = false;
        card(ui, |ui| {
            ui.label(RichText::new("Onboarding task").strong().size(19.0));
            field(ui, "Task title *", &mut d.title, "");
            ui.label("Instructions visible to assigned staff");
            ui.add(
                egui::TextEdit::multiline(&mut d.instructions)
                    .desired_rows(3)
                    .desired_width(f32::INFINITY),
            );
            ui.label("Responsible staff");
            egui::ComboBox::from_id_salt("onboarding_assignee")
                .width(320.0)
                .selected_text(
                    d.assigned_employee_id
                        .and_then(|id| {
                            self.data
                                .recruiting
                                .interviewers
                                .iter()
                                .find(|s| s.employee_id == id)
                                .map(|s| s.full_name.as_str())
                        })
                        .unwrap_or("Unassigned"),
                )
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut d.assigned_employee_id, None, "Unassigned");
                    for s in &self.data.recruiting.interviewers {
                        ui.selectable_value(
                            &mut d.assigned_employee_id,
                            Some(s.employee_id),
                            &s.full_name,
                        );
                    }
                });
            field(ui, "Due date (optional)", &mut d.due_on, "YYYY-MM-DD");
            ui.checkbox(
                &mut d.required,
                "Required before onboarding can be completed",
            );
            field(ui, "Save reason *", &mut d.reason, "");
            let valid = HiringAction::TaskSave(d.clone()).payload(&self.data);
            if let Err(e) = &valid {
                ui.label(RichText::new(e.to_string()).color(muted(ui)));
            }
            ui.horizontal_wrapped(|ui| {
                save = ui
                    .add_enabled(valid.is_ok(), egui::Button::new("Save task"))
                    .clicked();
                discard = ui.button("Discard task draft").clicked();
            });
        });
        if !discard {
            self.onboarding_state.task_form = Some(d.clone());
        }
        if save {
            self.send_onboarding(HiringAction::TaskSave(d));
        }
    }
    pub(super) fn hiring_events(&self, ui: &mut Ui, events: &[onboarding::Event]) {
        if events.is_empty() {
            ui.label("No recorded changes.");
        }
        for e in events.iter().rev() {
            ui.label(
                RichText::new(format!(
                    "{} · {} · {}",
                    e.kind.replace('_', " "),
                    e.actor_name,
                    e.recorded_at
                        .with_timezone(&self.data.company.tz())
                        .format("%b %d, %Y %H:%M")
                ))
                .strong(),
            );
            ui.label(&e.reason);
            egui::CollapsingHeader::new("Saved change details")
                .id_salt(e.id)
                .default_open(history_preview())
                .show(ui, |ui| {
                    change_fields(ui, e);
                });
            ui.separator();
        }
    }
}
fn change_fields(ui: &mut Ui, e: &onboarding::Event) -> egui::Rect {
    record_change_fields(ui, e.id, e.before_record.as_ref(), &e.after_record)
}
pub(super) fn record_change_fields(
    ui: &mut Ui,
    id: Uuid,
    before: Option<&serde_json::Value>,
    after_value: &serde_json::Value,
) -> egui::Rect {
    let after = after_value.as_object();
    if let Some(after) = after {
        let width = if before.is_some() {
            (ui.available_width() - 180.0) / 2.0
        } else {
            ui.available_width() - 170.0
        }
        .clamp(85.0, 320.0);
        return egui::Grid::new(id)
            .num_columns(if before.is_some() { 3 } else { 2 })
            .max_col_width(width)
            .spacing([20.0, 8.0])
            .show(ui, |ui| {
                ui.strong("Field");
                if before.is_some() {
                    ui.strong("Before");
                }
                ui.strong("Saved value");
                ui.end_row();
                for (key, value) in after.iter().filter(|(k, _)| {
                    ![
                        "id",
                        "company_id",
                        "created_by",
                        "created_at",
                        "updated_at",
                        "birth_payload",
                        "approved_by",
                        "assigned_employee_id",
                        "employee_id",
                        "case_id",
                        "applicant_id",
                        "offer_id",
                        "conversion_id",
                        "reviewer_employee_id",
                        "published_by",
                        "cycle_id",
                        "review_id",
                        "note_id",
                        "goal_id",
                        "training_id",
                        "certificate_id",
                        "evidence_id",
                        "document_id",
                    ]
                    .contains(&k.as_str())
                }) {
                    let old = before.and_then(|b| b.get(key));
                    if before.is_some() && old == Some(value) {
                        continue;
                    }
                    ui.vertical(|ui| {
                        ui.set_width(135.0);
                        ui.label(if key == "amount_minor" {
                            "Base amount".into()
                        } else {
                            let label = key.replace('_', " ");
                            let mut chars = label.chars();
                            chars
                                .next()
                                .map(|c| c.to_uppercase().to_string() + chars.as_str())
                                .unwrap_or_default()
                        });
                    });
                    if before.is_some() {
                        ui.vertical(|ui| {
                            ui.set_width(width);
                            ui.add(
                                egui::Label::new(history_value(
                                    key,
                                    old.unwrap_or(&serde_json::Value::Null),
                                ))
                                .wrap(),
                            );
                        });
                    }
                    ui.vertical(|ui| {
                        ui.set_width(width);
                        ui.add(egui::Label::new(history_value(key, value)).wrap());
                    });
                    ui.end_row();
                }
            })
            .response
            .rect;
    }
    egui::Rect::NOTHING
}
fn history_preview() -> bool {
    #[cfg(feature = "qa-screenshot")]
    {
        return std::env::var("HRIS_PREVIEW_FORM").is_ok_and(|s| s.contains("history"));
    }
    #[cfg(not(feature = "qa-screenshot"))]
    {
        false
    }
}
fn history_value(key: &str, v: &serde_json::Value) -> String {
    if key == "goals_snapshot" {
        return format!(
            "{} goals saved at submission",
            v.as_array().map_or(0, Vec::len)
        );
    }
    if key == "amount_minor" {
        if let Some(n) = v.as_i64() {
            return onboarding::money(n);
        }
    }
    json_text(v)
}
fn json_text(v: &serde_json::Value) -> String {
    match v {
        serde_json::Value::Null => "Not set".into(),
        serde_json::Value::String(s) if s.is_empty() => "Not set".into(),
        serde_json::Value::String(s) => s.clone(),
        _ => v.to_string(),
    }
}
#[cfg(feature = "qa-screenshot")]
impl HrisApp {
    pub(super) fn prepare_onboarding_preview(&mut self) {
        let preview = std::env::var("HRIS_PREVIEW_FORM").unwrap_or_default();
        if !preview.starts_with("onboard-") || self.data.mode != Mode::Demo {
            return;
        }
        let access = self.data.access.clone();
        let mut data = self.data.clone();
        data.access.role = Role::Administrator;
        let own = access.employee_id.unwrap();
        data.recruiting.interviewers = vec![crate::recruiting::Interviewer {
            employee_id: own,
            full_name: data.employee_name(own).into(),
            role: access.role,
        }];
        let mut a = crate::hiring::ApplicantDraft::new(data.company.today());
        a.full_name = "Fictional Onboarding Candidate".into();
        a.position = "Operations Associate".into();
        let app = a.id.unwrap();
        crate::hiring::apply_demo(
            &mut data,
            &crate::hiring::HiringAction::Create {
                draft: a,
                reason: "Fictional application received".into(),
            },
            chrono::Utc::now(),
        )
        .unwrap();
        let mut d = OfferDraft::new(&data, app);
        d.department = "Operations".into();
        d.amount = "25000.00".into();
        d.starts_on = (data.company.today() + Duration::days(7)).to_string();
        d.terms="Fictional offer: employment terms and company benefits are recorded here for Administrator review. Statutory payroll calculations are handled separately.".into();
        let id = d.id;
        onboarding::apply_demo(
            &mut data,
            &HiringAction::OfferSave(d.clone()),
            chrono::Utc::now(),
        )
        .unwrap();
        let final_status = if preview == "onboard-offer-form" {
            0
        } else if preview == "onboard-offer-approved" {
            1
        } else if preview == "onboard-offer-decision" {
            2
        } else {
            3
        };
        for (i, status) in [
            OfferStatus::Approved,
            OfferStatus::Sent,
            OfferStatus::Accepted,
        ]
        .into_iter()
        .enumerate()
        {
            if i < final_status {
                let revision = data.onboarding.offers[0].revision;
                onboarding::apply_demo(
                    &mut data,
                    &HiringAction::OfferStatus {
                        id,
                        revision,
                        status,
                        reason: "Fictional documented offer decision".into(),
                    },
                    chrono::Utc::now(),
                )
                .unwrap();
            }
        }
        let mut hire = HireDraft::new(&data.onboarding.offers[0]);
        hire.employee_code = "EMP-FICTIONAL".into();
        hire.reports_to = Some(own);
        if !preview.starts_with("onboard-offer") && preview != "onboard-hire-form" {
            onboarding::apply_demo(
                &mut data,
                &HiringAction::Hire(hire.clone()),
                chrono::Utc::now(),
            )
            .unwrap();
            let mut t = TaskDraft::existing(&data.onboarding.tasks[0]);
            t.assigned_employee_id = Some(own);
            t.instructions="Review the employee’s submitted details, request any missing emergency information through HR, and record completion. Fictional test data only.".into();
            t.reason = "Fictional responsibility assigned".into();
            onboarding::apply_demo(&mut data, &HiringAction::TaskSave(t), chrono::Utc::now())
                .unwrap();
        }
        data.access = access.clone();
        self.data = data;
        if preview.starts_with("onboard-offer") || preview == "onboard-hire-form" {
            self.hiring_state.open(
                self.data
                    .hiring
                    .records
                    .iter()
                    .find(|a| a.id == app)
                    .unwrap()
                    .clone(),
            );
            self.hiring_state.tab = super::hiring_view::Tab::Offers;
            if preview == "onboard-offer-form" {
                let mut edit = OfferDraft::existing(&self.data.onboarding.offers[0]);
                edit.reason = "Fictional terms revised".into();
                self.onboarding_state.offer_form = Some(edit);
            }
            if preview == "onboard-offer-decision" {
                self.onboarding_state.decision = Some(HiringAction::OfferStatus {
                    id,
                    revision: 2,
                    status: OfferStatus::Accepted,
                    reason: "Fictional candidate confirmation received".into(),
                });
            }
            if preview == "onboard-hire-form" {
                self.onboarding_state.hire_form = Some(hire);
            }
        } else {
            self.onboarding_state.active = true;
            if preview != "onboard-list" {
                self.onboarding_state.case_id = Some(hire.id);
            }
            if preview == "onboard-task-form" {
                let mut t = TaskDraft::existing(&self.data.onboarding.tasks[0]);
                t.reason = "Fictional due date revised".into();
                self.onboarding_state.task_form = Some(t);
            }
            if preview == "onboard-task-decision" {
                let t = &self.data.onboarding.tasks[0];
                self.onboarding_state.decision = Some(HiringAction::TaskStatus {
                    id: t.id,
                    revision: t.revision,
                    status: "done".into(),
                    reason: "Fictional task completed and verified".into(),
                });
            }
        }
        if !access.role.manages_hr() {
            self.data.hiring = Default::default();
            self.data.onboarding.offers.clear();
            self.data.onboarding.offer_events.clear();
            self.data.onboarding.conversions.clear();
            self.data.onboarding.cases.clear();
            self.data
                .onboarding
                .tasks
                .retain(|t| t.assigned_employee_id == access.employee_id);
            self.data.onboarding.events.retain(|e| {
                e.task_id
                    .is_some_and(|id| self.data.onboarding.tasks.iter().any(|t| t.id == id))
            });
            self.data.recruiting.interviewers.clear();
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unrelated_saved_record_keeps_offer_and_decision_drafts() {
        let data = DemoStore::new().data;
        let d = OfferDraft::new(&data, Uuid::new_v4());
        let mut state = State {
            offer_form: Some(d.clone()),
            pending: Some(HiringAction::OfferSave(d)),
            ..Default::default()
        };
        assert!(!state.accept(&data));
        assert!(state.offer_form.is_some());
        let a = HiringAction::TaskStatus {
            id: Uuid::new_v4(),
            revision: 0,
            status: "done".into(),
            reason: "Completed".into(),
        };
        state.decision = Some(a.clone());
        state.pending = Some(a);
        assert!(!state.accept(&data));
        assert!(state.decision.is_some());
    }
}

#[cfg(test)]
mod layout_tests {
    use super::*;
    #[test]
    fn long_change_values_wrap_inside_narrow_history_columns() {
        for width in [420.0, 640.0, 950.0] {
            for previous in [false, true] {
                let ctx = egui::Context::default();
                let event = onboarding::Event {
                    id: Uuid::new_v4(),
                    company_id: Uuid::nil(),
                    offer_id: None,
                    case_id: None,
                    task_id: None,
                    kind: "edited".into(),
                    reason: "Synthetic layout check".into(),
                    actor_id: Uuid::nil(),
                    actor_name: "Fictional staff".into(),
                    actor_role: Role::Administrator,
                    before_record: previous.then(|| serde_json::json!({"terms":"Earlier terms"})),
                    after_record: serde_json::json!({"terms":"Fictional employment terms and benefits. ".repeat(30),"status":"approved"}),
                    recorded_at: chrono::Utc::now(),
                };
                let mut measured = egui::Rect::NOTHING;
                let mut output = ctx.run_ui(
                    egui::RawInput {
                        screen_rect: Some(egui::Rect::from_min_size(
                            egui::Pos2::ZERO,
                            egui::vec2(width, 1800.0),
                        )),
                        ..Default::default()
                    },
                    |ui| {
                        egui::CentralPanel::default().show(ui, |ui| {
                            measured = change_fields(ui, &event);
                        });
                    },
                );
                output.textures_delta.clear();
                assert!(measured.right() <= width + 1.0, "{width}: {measured:?}");
                assert!(
                    measured.height() > 80.0,
                    "Long values must wrap to multiple lines"
                );
            }
        }
    }
}
