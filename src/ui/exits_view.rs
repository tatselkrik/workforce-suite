use super::*;
use crate::exits::{self, CaseDraft, ExitAction, TaskDraft};
#[derive(Default)]
pub(super) struct State {
    pub case_form: Option<CaseDraft>,
    pub task_form: Option<TaskDraft>,
    pub decision: Option<ExitAction>,
    pub pending: Option<ExitAction>,
    pub case_id: Option<Uuid>,
    pub show_completed: bool,
    pub search: String,
}
impl State {
    pub fn editing(&self) -> bool {
        self.case_form.is_some() || self.task_form.is_some() || self.decision.is_some()
    }
    pub fn accept(&mut self, data: &Snapshot) -> bool {
        let Some(a) = self.pending.take() else {
            return true;
        };
        if !a.saved(data) {
            return false;
        }
        if let ExitAction::CaseSave(d) = a {
            self.case_id = Some(d.id);
        }
        self.case_form = None;
        self.task_form = None;
        self.decision = None;
        true
    }
}
fn reason_mut(a: &mut ExitAction) -> &mut String {
    match a {
        ExitAction::CaseSave(d) => &mut d.reason,
        ExitAction::TaskSave(d) => &mut d.reason,
        ExitAction::CaseStatus { reason, .. } | ExitAction::TaskStatus { reason, .. } => reason,
    }
}
fn status_title(status: &str) -> &str {
    match status {
        "cleared" => "Record clearance",
        "finalized" => "Finalize exit and disable sign-in",
        "cancelled" => "Cancel this exit",
        "open" => "Reopen clearance",
        _ => "Save change",
    }
}
impl HrisApp {
    fn send_exit(&mut self, a: ExitAction) {
        self.exit_state.pending = Some(a.clone());
        self.send(Command::Change(Action::Exit(a)));
    }
    fn exit_decision(&mut self, ui: &mut Ui) -> bool {
        let Some(mut a) = self.exit_state.decision.take() else {
            return false;
        };
        let mut save = false;
        let mut cancel = false;
        card(ui, |ui| {
            let title = match &a {
                ExitAction::CaseStatus { status, .. } => status_title(status).to_owned(),
                ExitAction::TaskStatus { status, .. } => format!("Set clearance task to {status}"),
                _ => "Save clearance".into(),
            };
            ui.label(RichText::new(title).strong().size(19.0));
            if let ExitAction::CaseStatus { id, status, .. } = &a {
                if let Ok(c) = exits::case(&self.data, *id) {
                    ui.label(format!(
                        "{} · {} · Last day {}",
                        c.employee_code, c.employee_name, c.last_working_on
                    ));
                }
                if status == "finalized" {
                    ui.label("This disables company sign-in and archives the employee. Their account, documents and history are retained. The server rechecks readiness when saved. Final pay remains a separate human review.");
                }
                if status == "cancelled" {
                    ui.label("Cancelling closes the checklist and preserves its history. It does not change employment status or sign-in access.");
                }
            }
            field(ui, "Reason / evidence reference *", reason_mut(&mut a), "");
            let valid = a.payload(&self.data);
            if let Err(e) = &valid {
                ui.label(RichText::new(e.to_string()).color(muted(ui)));
            }
            ui.horizontal_wrapped(|ui| {
                save = ui
                    .add_enabled(valid.is_ok(), egui::Button::new("Confirm and save"))
                    .clicked();
                cancel = ui.button("Cancel change").clicked();
            });
        });
        if !cancel {
            self.exit_state.decision = Some(a.clone());
        }
        if save {
            self.send_exit(a);
        }
        true
    }
    pub(super) fn exits(&mut self, ui: &mut Ui) {
        if self.exit_decision(ui) {
            return;
        }
        if self.exit_state.case_form.is_some() {
            self.exit_case_form(ui);
            return;
        }
        if self.exit_state.task_form.is_some() {
            self.clearance_task_form(ui);
            return;
        }
        if !self
            .data
            .access
            .allows(crate::staff_roles::Capability::Personnel)
        {
            self.assigned_clearance(ui);
            return;
        }
        if let Some(c) = self
            .exit_state
            .case_id
            .and_then(|id| self.data.exits.cases.iter().find(|c| c.id == id))
            .cloned()
        {
            self.exit_case(ui, &c);
            return;
        }
        card(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.label(RichText::new("Employee exits").strong().size(20.0));
                if ui.button("Start exit checklist").clicked() {
                    self.exit_state.case_form = Some(CaseDraft::new(&self.data, None));
                }
            });
            ui.label("Prepare handover and clearance before an Administrator finalizes the exit. Sign-in stays available until finalization.");
            ui.checkbox(
                &mut self.exit_state.show_completed,
                "Include finalized and cancelled exits",
            );
            ui.add(
                egui::TextEdit::singleline(&mut self.exit_state.search)
                    .hint_text("Search employee code or name")
                    .desired_width(350.0),
            );
        });
        let query = self.exit_state.search.to_lowercase();
        let mut cases: Vec<_> = self
            .data
            .exits
            .cases
            .iter()
            .filter(|c| {
                (self.exit_state.show_completed || ["open", "cleared"].contains(&c.status.as_str()))
                    && format!("{} {}", c.employee_code, c.employee_name)
                        .to_lowercase()
                        .contains(&query)
            })
            .cloned()
            .collect();
        cases.sort_by_key(|c| (c.last_working_on, c.employee_code.clone()));
        if cases.is_empty() {
            empty(ui, "No matching exit records.");
        }
        for c in cases {
            ui.add_space(10.0);
            card(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    if ui
                        .button(format!("{} · {}", c.employee_code, c.employee_name))
                        .clicked()
                    {
                        self.exit_state.case_id = Some(c.id);
                    }
                    badge(ui, &c.status, c.status == "cleared");
                    ui.label(format!("Last working day {}", c.last_working_on));
                });
            });
        }
    }
    fn exit_case(&mut self, ui: &mut Ui, c: &exits::Case) {
        card(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                if ui.button("< All exits").clicked() {
                    self.exit_state.case_id = None;
                }
                if ui.button("Open employee record").clicked() {
                    self.navigate(Page::Employees, ui.ctx());
                    self.open_employee(c.employee_id);
                }
            });
            ui.label(
                RichText::new(format!("{} · {}", c.employee_code, c.employee_name))
                    .strong()
                    .size(20.0),
            );
            badge(ui, &c.status, c.status == "cleared");
            ui.label(format!(
                "Last working day: {} · {}",
                c.last_working_on,
                c.category.replace('_', " ")
            ));
            if !c.private_notes.is_empty() {
                ui.label("Private HR notes");
                ui.label(&c.private_notes);
            }
            ui.horizontal_wrapped(|ui| {
                if c.status == "open" {
                    if ui.button("Edit exit details").clicked() {
                        self.exit_state.case_form = Some(CaseDraft::existing(c));
                    }
                    if ui.button("Add clearance task").clicked() {
                        self.exit_state.task_form = Some(TaskDraft::new(c.id));
                    }
                }
                for status in ["cleared", "open", "cancelled", "finalized"] {
                    let a = ExitAction::CaseStatus {
                        id: c.id,
                        revision: c.revision,
                        status: status.into(),
                        reason: "Preview availability".into(),
                    };
                    if a.payload(&self.data).is_ok() && ui.button(status_title(status)).clicked() {
                        self.exit_state.decision = Some(ExitAction::CaseStatus {
                            id: c.id,
                            revision: c.revision,
                            status: status.into(),
                            reason: String::new(),
                        });
                    }
                }
            });
        });
        if ["open", "cleared"].contains(&c.status.as_str()) {
            ui.add_space(10.0);
            card(ui, |ui| {
                ui.label(
                    RichText::new("Readiness for finalization")
                        .strong()
                        .size(18.0),
                );
                ui.label("Current protected records are checked again when an Administrator finalizes. Refresh after resolving items elsewhere.");
                if let Some(r) = exits::readiness(&self.data, c.id) {
                    let blockers = r.blockers();
                    if blockers.is_empty() {
                        ui.label("All recorded readiness checks pass. Record clearance, then an Administrator can finalize.");
                    } else {
                        for b in blockers {
                            ui.add(egui::Label::new(format!("• {b}")).wrap());
                        }
                    }
                } else {
                    ui.label("Readiness is unavailable. Refresh before finalizing.");
                }
            });
        }
        let tasks: Vec<_> = self
            .data
            .exits
            .tasks
            .iter()
            .filter(|t| t.case_id == c.id)
            .cloned()
            .collect();
        for t in tasks {
            ui.add_space(10.0);
            self.clearance_task_card(ui, &t);
        }
        let events: Vec<_> = self
            .data
            .exits
            .events
            .iter()
            .filter(|e| e.case_id == Some(c.id))
            .cloned()
            .collect();
        ui.add_space(10.0);
        card(ui, |ui| {
            egui::CollapsingHeader::new("Exit and clearance history")
                .default_open(history_preview())
                .show(ui, |ui| self.hiring_events(ui, &events));
        });
    }
    fn exit_case_form(&mut self, ui: &mut Ui) {
        let Some(mut d) = self.exit_state.case_form.take() else {
            return;
        };
        let mut save = false;
        let mut cancel = false;
        card(ui, |ui| {
            ui.label(
                RichText::new(if d.original.is_some() {
                    "Edit exit details"
                } else {
                    "Start exit checklist"
                })
                .strong()
                .size(20.0),
            );
            if d.original.is_none() {
                ui.label("Employee");
                employee_picker(ui, "exit_employee", &self.data, &mut d.employee_id);
            } else if let Some(id) = d.employee_id {
                ui.label(self.data.employee_name(id));
            }
            field(
                ui,
                "Last working date *",
                &mut d.last_working_on,
                "YYYY-MM-DD",
            );
            ui.label("Separation category (private to HR)");
            egui::ComboBox::from_id_salt("exit_category")
                .selected_text(d.category.replace('_', " "))
                .show_ui(ui, |ui| {
                    for value in ["resignation", "end_of_contract", "retirement", "other"] {
                        ui.selectable_value(&mut d.category, value.into(), value.replace('_', " "));
                    }
                });
            ui.label("Private HR notes (up to 2,000 characters)");
            ui.add(
                egui::TextEdit::multiline(&mut d.private_notes)
                    .desired_rows(3)
                    .desired_width(f32::INFINITY),
            );
            field(ui, "Save reason *", &mut d.reason, "");
            ui.label(if d.original.is_none() {"Starts five required clearance tasks. Sign-in stays available until an Administrator finalizes the exit."} else {"Saves exit details and updates the checklist's last-day label. Existing tasks and sign-in access are retained."});
            let valid = ExitAction::CaseSave(d.clone()).payload(&self.data);
            if let Err(e) = &valid {
                ui.label(RichText::new(e.to_string()).color(muted(ui)));
            }
            ui.horizontal_wrapped(|ui| {
                save = ui
                    .add_enabled(valid.is_ok(), egui::Button::new("Save exit details"))
                    .clicked();
                cancel = ui.button("Discard exit draft").clicked();
            });
        });
        if !cancel {
            self.exit_state.case_form = Some(d.clone());
        }
        if save {
            self.send_exit(ExitAction::CaseSave(d));
        }
    }
    fn assigned_clearance(&mut self, ui: &mut Ui) {
        card(ui, |ui| {
            ui.label(RichText::new("Your clearance tasks").strong().size(20.0));
            ui.label("Complete assigned tasks and record a reason. HR manages assignments, waivers and overall completion.");
            ui.checkbox(
                &mut self.exit_state.show_completed,
                "Include resolved / completed clearance",
            );
        });
        let tasks: Vec<_> = self
            .data
            .exits
            .tasks
            .iter()
            .filter(|t| {
                t.assigned_employee_id == self.data.access.employee_id
                    && (self.exit_state.show_completed
                        || (t.status == "open" && t.case_status == "open"))
            })
            .cloned()
            .collect();
        if tasks.is_empty() {
            empty(ui, "No assigned clearance tasks match this view.");
        }
        for t in tasks {
            ui.add_space(10.0);
            self.clearance_task_card(ui, &t);
        }
    }
    fn clearance_task_card(&mut self, ui: &mut Ui, t: &exits::Task) {
        let hr = self
            .data
            .access
            .allows(crate::staff_roles::Capability::Personnel);
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
                    "{} · {} · Last day {}",
                    t.employee_code, t.employee_name, t.last_working_on
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
                        self.exit_state.task_form = Some(TaskDraft::existing(t));
                    }
                    for (status, label) in [
                        ("done", "Mark done"),
                        ("open", "Reopen task"),
                        ("waived", "Waive task"),
                    ] {
                        let a = ExitAction::TaskStatus {
                            id: t.id,
                            revision: t.revision,
                            status: status.into(),
                            reason: "Preview availability".into(),
                        };
                        if a.payload(&self.data).is_ok() && ui.button(label).clicked() {
                            self.exit_state.decision = Some(ExitAction::TaskStatus {
                                id: t.id,
                                revision: t.revision,
                                status: status.into(),
                                reason: String::new(),
                            });
                        }
                    }
                });
            } else {
                ui.label(
                    "This clearance checklist is closed; saved tasks and history remain available.",
                );
            }
            let events: Vec<_> = self
                .data
                .exits
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
    fn clearance_task_form(&mut self, ui: &mut Ui) {
        let Some(mut d) = self.exit_state.task_form.take() else {
            return;
        };
        let mut save = false;
        let mut discard = false;
        card(ui, |ui| {
            ui.label(RichText::new("Clearance task").strong().size(19.0));
            field(ui, "Task title *", &mut d.title, "");
            ui.label("Instructions visible to assigned staff");
            ui.add(
                egui::TextEdit::multiline(&mut d.instructions)
                    .desired_rows(3)
                    .desired_width(f32::INFINITY),
            );
            ui.label("Responsible staff");
            egui::ComboBox::from_id_salt("clearance_assignee")
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
                "Required before clearance can be completed",
            );
            field(ui, "Save reason *", &mut d.reason, "");
            let valid = ExitAction::TaskSave(d.clone()).payload(&self.data);
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
            self.exit_state.task_form = Some(d.clone());
        }
        if save {
            self.send_exit(ExitAction::TaskSave(d));
        }
    }
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

#[cfg(feature = "qa-screenshot")]
impl HrisApp {
    pub(super) fn prepare_exit_preview(&mut self) {
        let preview = std::env::var("HRIS_PREVIEW_FORM").unwrap_or_default();
        if !preview.starts_with("exit-") || self.data.mode != Mode::Demo {
            return;
        }
        let access = self.data.access.clone();
        let mut data = self.data.clone();
        data.access.role = Role::Administrator;
        let mut e = data.employees[0].clone();
        e.id = Uuid::new_v4();
        e.employee_code = "EMP-EXIT-001".into();
        e.full_name = "Fictional Clearance Employee".into();
        e.department = "Operations".into();
        e.reports_to = None;
        e.joined_on = Some("2020-01-01".parse().unwrap());
        e.status = EmployeeStatus::Active;
        let employee = e.id;
        data.employees.push(e);
        let own = access.employee_id.unwrap();
        data.recruiting.interviewers = vec![crate::recruiting::Interviewer {
            employee_id: own,
            full_name: data.employee_name(own).into(),
            role: access.role,
            roles: None,
        }];
        let mut d = CaseDraft::new(&data, Some(employee));
        d.reason = "Fictional exit preparation".into();
        d.private_notes="Fictional HR notes: complete a documented handover and preserve all employment records. Private case notes remain available only to HR and Administrators.".into();
        let id = d.id;
        exits::apply_demo(
            &mut data,
            &ExitAction::CaseSave(d.clone()),
            chrono::Utc::now(),
        )
        .unwrap();
        let task = data.exits.tasks[0].id;
        let mut td = TaskDraft::existing(&data.exits.tasks[0]);
        td.assigned_employee_id = Some(own);
        td.instructions="Confirm that the handover is complete and record the evidence reference. This instruction is visible to the assigned staff member.".into();
        td.reason = "Fictional assignment".into();
        exits::apply_demo(
            &mut data,
            &ExitAction::TaskSave(td.clone()),
            chrono::Utc::now(),
        )
        .unwrap();
        if preview.contains("ready") || preview.contains("final") || preview.contains("history") {
            for t in data.exits.tasks.clone() {
                exits::apply_demo(
                    &mut data,
                    &ExitAction::TaskStatus {
                        id: t.id,
                        revision: t.revision,
                        status: "done".into(),
                        reason: "Fictional clearance completed".into(),
                    },
                    chrono::Utc::now(),
                )
                .unwrap();
            }
            exits::apply_demo(
                &mut data,
                &ExitAction::CaseStatus {
                    id,
                    revision: 0,
                    status: "cleared".into(),
                    reason: "Fictional HR clearance".into(),
                },
                chrono::Utc::now(),
            )
            .unwrap();
        }
        self.data = data;
        self.data.access = access;
        self.page = Page::Exits;
        match preview.as_str() {
            "exit-new" => {
                d.id = Uuid::new_v4();
                d.employee_id = None;
                d.private_notes = String::new();
                d.reason = String::new();
                self.exit_state.case_form = Some(d);
            }
            "exit-edit" => {
                self.exit_state.case_form = Some(CaseDraft::existing(&self.data.exits.cases[0]));
            }
            "exit-task-edit" => {
                let t = self.data.exits.tasks.iter().find(|t| t.id == task).unwrap();
                self.exit_state.task_form = Some(TaskDraft::existing(t));
            }
            "exit-task-decision" => {
                let t = self.data.exits.tasks.iter().find(|t| t.id == task).unwrap();
                self.exit_state.decision = Some(ExitAction::TaskStatus {
                    id: task,
                    revision: t.revision,
                    status: "done".into(),
                    reason: "Fictional handover evidence".into(),
                });
            }
            "exit-finalize" => {
                self.exit_state.decision = Some(ExitAction::CaseStatus {
                    id,
                    revision: 1,
                    status: "finalized".into(),
                    reason: "Fictional finalization evidence".into(),
                });
            }
            "exit-finalized" => {
                self.data.access.role = Role::Administrator;
                exits::apply_demo(
                    &mut self.data,
                    &ExitAction::CaseStatus {
                        id,
                        revision: 1,
                        status: "finalized".into(),
                        reason: "Fictional finalization".into(),
                    },
                    chrono::Utc::now(),
                )
                .unwrap();
                self.exit_state.case_id = Some(id);
            }
            "exit-list" => {}
            _ => {
                self.exit_state.case_id = Some(id);
            }
        }
        if self.data.access.role == Role::Manager {
            self.data.exits.cases.clear();
            self.data.exits.events.retain(|e| e.task_id == Some(task));
            self.data.exits.tasks.retain(|t| t.id == task);
            self.data.exits.readiness.clear();
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn changed_receipt_keeps_unsaved_exit_form() {
        let mut data = DemoStore::new().data;
        let person = data
            .employees
            .iter()
            .find(|e| Some(e.id) != data.access.employee_id)
            .unwrap()
            .id;
        let mut d = CaseDraft::new(&data, Some(person));
        d.reason = "Synthetic save".into();
        let action = ExitAction::CaseSave(d.clone());
        exits::apply_demo(&mut data, &action, chrono::Utc::now()).unwrap();
        data.exits.cases[0].private_notes = "Another edit".into();
        let mut state = State {
            case_form: Some(d),
            pending: Some(action),
            ..Default::default()
        };
        assert!(!state.accept(&data));
        assert!(state.case_form.is_some());
        assert!(state.pending.is_none());
    }
}
