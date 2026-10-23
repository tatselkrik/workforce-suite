use super::*;
use crate::career::{self, CareerAction as Change, MilestoneDraft, PlanDraft};
#[derive(Default)]
pub(super) struct State {
    pub plan_form: Option<PlanDraft>,
    pub milestone_form: Option<MilestoneDraft>,
    pub decision: Option<Change>,
    pub pending: Option<Change>,
    selected: Option<Uuid>,
    employee: Option<Uuid>,
    search: String,
    resolved: bool,
}
impl State {
    pub fn editing(&self) -> bool {
        self.plan_form.is_some() || self.milestone_form.is_some() || self.decision.is_some()
    }
    pub fn accept(&mut self, data: &Snapshot) -> bool {
        let Some(a) = self.pending.take() else {
            return true;
        };
        if !a.saved(data) {
            return false;
        }
        if let Change::PlanSave(d) = &a {
            self.selected = Some(d.id);
        }
        self.plan_form = None;
        self.milestone_form = None;
        self.decision = None;
        true
    }
}
fn area(ui: &mut Ui, label: &str, text: &mut String, rows: usize) {
    ui.label(label);
    ui.add(
        egui::TextEdit::multiline(text)
            .desired_rows(rows)
            .desired_width(f32::INFINITY),
    );
}
fn controls(ui: &mut Ui, data: &Snapshot, a: &Change, label: &str) -> (bool, bool) {
    let valid = a.payload(data);
    if let Err(e) = &valid {
        ui.label(RichText::new(e.to_string()).color(muted(ui)));
    }
    let mut result = (false, false);
    ui.horizontal_wrapped(|ui| {
        result.0 = ui
            .add_enabled(valid.is_ok(), egui::Button::new(label))
            .clicked();
        result.1 = ui.button("Discard changes").clicked();
    });
    result
}
fn date_or(d: Option<NaiveDate>) -> String {
    d.map(|d| d.to_string()).unwrap_or_else(|| "Not set".into())
}
fn title(a: &Change) -> &str {
    match a {
        Change::Progress { .. } => "Record milestone progress",
        Change::PlanStatus { status, .. } => match status.as_str() {
            "completed" => "Complete career plan",
            "cancelled" => "Cancel career plan",
            _ => "Reopen career plan",
        },
        Change::MilestoneStatus { status, .. } => match status.as_str() {
            "done" => "Review milestone completion",
            "waived" => "Waive milestone",
            _ => "Reopen milestone",
        },
        _ => "Save career change",
    }
}
fn shared(ui: &mut Ui) {
    ui.label(RichText::new("Notes, progress, reasons and history are shared with the employee, current reporting Manager and HR.").color(muted(ui)));
}
impl HrisApp {
    fn send_career(&mut self, a: Change) {
        self.career_state.pending = Some(a.clone());
        self.send(Command::Change(Action::Career(a)));
    }
    pub(super) fn career(&mut self, ui: &mut Ui) {
        if self.career_decision(ui) {
            return;
        }
        if self.career_state.plan_form.is_some() {
            self.career_plan_form(ui);
            return;
        }
        if self.career_state.milestone_form.is_some() {
            self.career_milestone_form(ui);
            return;
        }
        if let Some(id) = self.career_state.selected {
            if let Ok(p) = career::plan(&self.data, id).cloned() {
                self.career_detail(ui, &p);
                return;
            }
            self.career_state.selected = None;
        }
        card(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                if self
                    .data
                    .active()
                    .any(|e| career::can_manage(&self.data, e.id))
                    && ui.button("Create career plan").clicked()
                {
                    let employee = self.career_state.employee.filter(|id| {
                        career::can_manage(&self.data, *id)
                            && self.data.active().any(|e| e.id == *id)
                    });
                    self.career_state.plan_form =
                        Some(PlanDraft::new(employee, self.data.company.today()));
                }
                ui.checkbox(
                    &mut self.career_state.resolved,
                    "Include completed / cancelled",
                );
            });
            ui.horizontal_wrapped(|ui| {
                egui::ComboBox::from_id_salt("career_employee_filter")
                    .width(280.0)
                    .selected_text(
                        self.career_state
                            .employee
                            .map(|id| self.data.employee_name(id))
                            .unwrap_or("All employees in your access"),
                    )
                    .show_ui(ui, |ui| {
                        ui.selectable_value(
                            &mut self.career_state.employee,
                            None,
                            "All employees in your access",
                        );
                        for e in self
                            .data
                            .employees
                            .iter()
                            .filter(|e| career::in_scope(&self.data, e.id))
                        {
                            ui.selectable_value(
                                &mut self.career_state.employee,
                                Some(e.id),
                                format!("{} · {}", e.employee_code, e.full_name),
                            );
                        }
                    });
                ui.add(
                    egui::TextEdit::singleline(&mut self.career_state.search)
                        .hint_text("Search employee, plan, skills or target role")
                        .desired_width(300.0),
                );
            });
            ui.label("Agree on career aspirations and practical milestones. A plan does not change an employee's job, department or salary.");
        });
        ui.add_space(10.0);
        let needle = self.career_state.search.to_lowercase();
        let mut rows: Vec<_> = self
            .data
            .career
            .plans
            .iter()
            .filter(|p| {
                career::in_scope(&self.data, p.employee_id)
                    && self
                        .career_state
                        .employee
                        .is_none_or(|id| p.employee_id == id)
                    && (self.career_state.resolved || p.status == "active")
                    && format!(
                        "{} {} {} {} {}",
                        p.employee_name, p.employee_code, p.title, p.target_role, p.skills_focus
                    )
                    .to_lowercase()
                    .contains(&needle)
            })
            .cloned()
            .collect();
        rows.sort_by_key(|p| (p.employee_name.clone(), p.target_on, p.title.clone()));
        if rows.is_empty() {
            card(ui, |ui| {
                ui.label("No career plans match these filters.");
                ui.label("Another HR user or the current reporting Manager assigns and reviews a plan. Staff can record their own milestone progress.");
            });
        }
        for p in rows {
            card(ui, |ui| {
                ui.label(RichText::new(&p.title).strong().size(18.0));
                ui.label(format!("{} · {}", p.employee_code, p.employee_name));
                ui.label(format!(
                    "{} · Target role: {} · Target date: {}",
                    p.status,
                    if p.target_role.is_empty() {
                        "Not set"
                    } else {
                        &p.target_role
                    },
                    date_or(p.target_on)
                ));
                let tasks: Vec<_> = self
                    .data
                    .career
                    .milestones
                    .iter()
                    .filter(|t| t.company_id == self.data.company.id && t.plan_id == p.id)
                    .collect();
                ui.label(format!(
                    "{} milestones · {} done · {} required still open",
                    tasks.len(),
                    tasks.iter().filter(|t| t.status == "done").count(),
                    tasks
                        .iter()
                        .filter(|t| t.required && t.status == "open")
                        .count()
                ));
                if ui.button("Open plan").clicked() {
                    self.career_state.selected = Some(p.id);
                }
            });
            ui.add_space(10.0);
        }
    }
    fn career_detail(&mut self, ui: &mut Ui, p: &career::Plan) {
        if ui.button("Back to career plans").clicked() {
            self.career_state.selected = None;
            return;
        }
        let manage = career::can_manage(&self.data, p.employee_id);
        let active = self.data.active().any(|e| e.id == p.employee_id);
        let tasks: Vec<_> = self
            .data
            .career
            .milestones
            .iter()
            .filter(|t| t.company_id == self.data.company.id && t.plan_id == p.id)
            .cloned()
            .collect();
        card(ui, |ui| {
            ui.label(RichText::new(&p.title).strong().size(21.0));
            ui.label(format!("{} · {}", p.employee_code, p.employee_name));
            ui.label(format!(
                "Status: {} · Starts: {} · Target: {}",
                p.status,
                p.starts_on,
                date_or(p.target_on)
            ));
            if let Some(e) = self.data.employees.iter().find(|e| e.id == p.employee_id) {
                ui.label(format!(
                    "Current employment: {} · {}",
                    if e.job_title.is_empty() {
                        "Job title not set"
                    } else {
                        &e.job_title
                    },
                    if e.department.is_empty() {
                        "Department not set"
                    } else {
                        &e.department
                    }
                ));
            }
            ui.separator();
            ui.label(RichText::new("Career aspirations").strong());
            ui.label(format!(
                "Target role: {} · Target department: {}",
                if p.target_role.is_empty() {
                    "Not set"
                } else {
                    &p.target_role
                },
                if p.target_department.is_empty() {
                    "Not set"
                } else {
                    &p.target_department
                }
            ));
            if !p.skills_focus.is_empty() {
                ui.label(RichText::new("Skills to develop").strong());
                ui.label(&p.skills_focus);
            }
            if !p.shared_notes.is_empty() {
                ui.label(RichText::new("Shared coaching notes").strong());
                ui.label(&p.shared_notes);
            }
            shared(ui);
            ui.horizontal_wrapped(|ui| {
                if manage && active && p.status == "active" {
                    if ui.button("Edit plan").clicked() {
                        self.career_state.plan_form = Some(PlanDraft::existing(p));
                    }
                    if ui.button("Add milestone").clicked() {
                        self.career_state.milestone_form = Some(MilestoneDraft::new(p.id));
                    }
                    if ui.button("Complete plan").clicked() {
                        self.career_state.decision = Some(Change::PlanStatus {
                            id: p.id,
                            revision: p.revision,
                            status: "completed".into(),
                            completed_on: self.data.company.today().to_string(),
                            summary: String::new(),
                            reason: String::new(),
                        });
                    }
                }
                if manage && p.status == "active" && ui.button("Cancel plan").clicked() {
                    self.career_state.decision = Some(Change::PlanStatus {
                        id: p.id,
                        revision: p.revision,
                        status: "cancelled".into(),
                        completed_on: String::new(),
                        summary: String::new(),
                        reason: String::new(),
                    });
                }
                if manage && active && p.status != "active" && ui.button("Reopen plan").clicked() {
                    self.career_state.decision = Some(Change::PlanStatus {
                        id: p.id,
                        revision: p.revision,
                        status: "active".into(),
                        completed_on: String::new(),
                        summary: String::new(),
                        reason: String::new(),
                    });
                }
            });
        });
        ui.add_space(10.0);
        if p.status == "completed" {
            card(ui, |ui| {
                ui.label(RichText::new(format!("Completed {}", date_or(p.completed_on))).strong());
                ui.label(&p.completion_summary);
                ui.label("These milestone facts were saved at completion. Later changes to related records do not rewrite them.");
                snapshot(ui, &p.milestones_snapshot, (p.id, "career_closure"));
            });
            ui.add_space(10.0);
        }
        ui.label(
            RichText::new(format!(
                "{} milestones ({})",
                if p.status == "completed" {
                    "Current"
                } else {
                    "Plan"
                },
                tasks.len()
            ))
            .strong()
            .size(18.0),
        );
        if tasks.is_empty() {
            ui.label("No milestones yet. Complete at least one and resolve every required milestone before completing this plan.");
        }
        let mut tasks = tasks;
        tasks.sort_by_key(|t| (t.due_on.is_none(), t.due_on, t.created_at, t.id));
        for t in tasks {
            self.career_milestone_card(ui, p, &t, manage && active);
            ui.add_space(8.0);
        }
        card(ui, |ui| self.career_history(ui, p.id));
    }
    fn career_milestone_card(
        &mut self,
        ui: &mut Ui,
        p: &career::Plan,
        t: &career::Milestone,
        manage: bool,
    ) {
        card(ui, |ui| {
            ui.label(RichText::new(&t.title).strong().size(17.0));
            ui.label(format!(
                "{}{} · Due: {}",
                t.status,
                if t.required {
                    " · Required"
                } else {
                    " · Optional"
                },
                date_or(t.due_on)
            ));
            if !t.action_steps.is_empty() {
                ui.label(&t.action_steps);
            }
            if !t.progress_note.is_empty() {
                ui.label(RichText::new("Shared progress").strong());
                ui.label(&t.progress_note);
            }
            if !t.review_result.is_empty() {
                ui.label(format!(
                    "Reviewed result{}",
                    t.completed_on
                        .map(|d| format!(" · {d}"))
                        .unwrap_or_default()
                ));
                ui.label(&t.review_result);
            }
            if let Some((kind, title)) = related_label(&self.data, t) {
                ui.label(format!("Related {kind}: {title} · Context only"));
            }
            if p.status == "active" {
                ui.horizontal_wrapped(|ui| {
                    if manage && t.status == "open" && ui.button("Edit milestone").clicked() {
                        self.career_state.milestone_form = Some(MilestoneDraft::existing(t));
                    }
                    let progress = Change::Progress {
                        id: t.id,
                        revision: t.revision,
                        note: t.progress_note.clone(),
                        reason: "Preview availability".into(),
                    };
                    if progress.payload(&self.data).is_ok()
                        && ui.button("Record progress").clicked()
                    {
                        let mut a = progress;
                        a.reason_mut().clear();
                        self.career_state.decision = Some(a);
                    }
                    if manage {
                        for (status, label) in if t.status == "open" {
                            vec![("done", "Review completion"), ("waived", "Waive")]
                        } else {
                            vec![("open", "Reopen milestone")]
                        } {
                            if status == "waived" && !self.data.access.role.manages_hr() {
                                continue;
                            }
                            if ui.button(label).clicked() {
                                self.career_state.decision = Some(Change::MilestoneStatus {
                                    id: t.id,
                                    revision: t.revision,
                                    status: status.into(),
                                    completed_on: if status == "done" {
                                        self.data.company.today().to_string()
                                    } else {
                                        String::new()
                                    },
                                    result: String::new(),
                                    reason: String::new(),
                                });
                            }
                        }
                    }
                });
            }
        });
    }
    fn career_plan_form(&mut self, ui: &mut Ui) {
        let Some(mut d) = self.career_state.plan_form.take() else {
            return;
        };
        let mut result = (false, false);
        card(ui, |ui| {
            ui.label(
                RichText::new(if d.original.is_some() {
                    "Edit career plan"
                } else {
                    "Create career plan"
                })
                .strong()
                .size(20.0),
            );
            ui.add_enabled_ui(d.original.is_none(), |ui| {
                egui::ComboBox::from_id_salt("career_plan_employee")
                    .width(ui.available_width().min(420.0))
                    .selected_text(
                        d.employee_id
                            .map(|id| self.data.employee_name(id))
                            .unwrap_or("Choose an employee"),
                    )
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut d.employee_id, None, "Choose an employee");
                        for e in self
                            .data
                            .active()
                            .filter(|e| career::can_manage(&self.data, e.id))
                        {
                            ui.selectable_value(
                                &mut d.employee_id,
                                Some(e.id),
                                format!("{} · {}", e.employee_code, e.full_name),
                            );
                        }
                    });
            });
            field(ui, "Plan title *", &mut d.title, "");
            if ui.available_width() >= 520.0 {
                ui.columns(2, |c| {
                    field(
                        &mut c[0],
                        "Target role (aspiration)",
                        &mut d.target_role,
                        "",
                    );
                    field(
                        &mut c[1],
                        "Target department (aspiration)",
                        &mut d.target_department,
                        "",
                    );
                });
                ui.columns(2, |c| {
                    field(&mut c[0], "Starts on *", &mut d.starts_on, "YYYY-MM-DD");
                    field(
                        &mut c[1],
                        "Target date",
                        &mut d.target_on,
                        "YYYY-MM-DD or blank",
                    );
                });
            } else {
                field(ui, "Target role (aspiration)", &mut d.target_role, "");
                field(
                    ui,
                    "Target department (aspiration)",
                    &mut d.target_department,
                    "",
                );
                field(ui, "Starts on *", &mut d.starts_on, "YYYY-MM-DD");
                field(ui, "Target date", &mut d.target_on, "YYYY-MM-DD or blank");
            }
            area(ui, "Skills to develop", &mut d.skills_focus, 3);
            area(ui, "Shared coaching notes", &mut d.shared_notes, 3);
            field(ui, "Save reason *", &mut d.reason, "");
            shared(ui);
            result = controls(
                ui,
                &self.data,
                &Change::PlanSave(d.clone()),
                "Save career plan",
            );
        });
        if !result.1 {
            self.career_state.plan_form = Some(d.clone());
        }
        if result.0 {
            self.send_career(Change::PlanSave(d));
        }
    }
    fn career_milestone_form(&mut self, ui: &mut Ui) {
        let Some(mut d) = self.career_state.milestone_form.take() else {
            return;
        };
        let mut result = (false, false);
        card(ui, |ui| {
            ui.label(
                RichText::new(if d.original.is_some() {
                    "Edit career milestone"
                } else {
                    "Add career milestone"
                })
                .strong()
                .size(20.0),
            );
            if let Ok(p) = career::plan(&self.data, d.plan_id) {
                ui.label(format!("{} · {}", p.employee_name, p.title));
            }
            field(ui, "Milestone title *", &mut d.title, "");
            area(ui, "Actions / practice steps", &mut d.action_steps, 3);
            field(ui, "Due date", &mut d.due_on, "YYYY-MM-DD or blank");
            ui.checkbox(
                &mut d.required,
                "Required before this plan can be completed",
            );
            if let Ok(p) = career::plan(&self.data, d.plan_id) {
                related_choice(ui, &self.data, p.employee_id, &mut d);
            }
            field(ui, "Save reason *", &mut d.reason, "");
            shared(ui);
            result = controls(
                ui,
                &self.data,
                &Change::MilestoneSave(d.clone()),
                "Save milestone",
            );
        });
        if !result.1 {
            self.career_state.milestone_form = Some(d.clone());
        }
        if result.0 {
            self.send_career(Change::MilestoneSave(d));
        }
    }
    fn career_decision(&mut self, ui: &mut Ui) -> bool {
        let Some(mut a) = self.career_state.decision.take() else {
            return false;
        };
        let mut result = (false, false);
        card(ui, |ui| {
            ui.label(RichText::new(title(&a)).strong().size(20.0));
            if a.is_plan() {
                if let Ok(p) = career::plan(&self.data, a.id()) {
                    ui.label(format!("{} · {}", p.employee_name, p.title));
                }
            } else if let Ok(t) = career::milestone(&self.data, a.id()) {
                if let Ok(p) = career::plan(&self.data, t.plan_id) {
                    ui.label(format!("{} · {}", p.employee_name, p.title));
                }
                ui.label(RichText::new(&t.title).strong());
            }
            match &mut a {
                Change::Progress { note, .. } => area(ui, "Shared progress", note, 5),
                Change::PlanStatus {
                    status,
                    completed_on,
                    summary,
                    ..
                } if status == "completed" => {
                    field(ui, "Actual completion date *", completed_on, "YYYY-MM-DD");
                    area(ui, "Development outcome / summary *", summary, 5);
                    ui.label("At least one milestone must be done and every required milestone must be resolved. Completion saves milestone facts and related context in history.");
                }
                Change::MilestoneStatus {
                    status,
                    completed_on,
                    result,
                    ..
                } if status != "open" => {
                    if status == "done" {
                        field(ui, "Actual completion date *", completed_on, "YYYY-MM-DD");
                    }
                    area(
                        ui,
                        if status == "waived" {
                            "Waiver result *"
                        } else {
                            "Review result *"
                        },
                        result,
                        5,
                    );
                }
                Change::PlanStatus { status, .. } if status == "active" => {
                    ui.label("Reopening clears the current completion summary and saved milestone facts. The prior completion remains in recorded history.");
                }
                Change::PlanStatus { .. } => {
                    ui.label("Cancellation retains this plan, milestones and history. No employment details change.");
                }
                Change::MilestoneStatus { .. } => {
                    ui.label("Reopening clears the current review result/date and retains the earlier review in history.");
                }
                _ => {}
            }
            field(ui, "Reason *", a.reason_mut(), "");
            shared(ui);
            result = controls(ui, &self.data, &a, "Save change");
        });
        if !result.1 {
            self.career_state.decision = Some(a.clone());
        }
        if result.0 {
            self.send_career(a);
        }
        true
    }
    fn career_history(&self, ui: &mut Ui, id: Uuid) {
        let events = career::visible_events(&self.data, id);
        egui::CollapsingHeader::new(format!("Recorded history ({})", events.len()))
            .id_salt((id, "career_history"))
            .default_open(history_preview())
            .show(ui, |ui| {
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
                            super::onboarding_view::record_change_fields(
                                ui,
                                e.id,
                                e.before_record.as_ref(),
                                &e.after_record,
                            );
                            if let Some(facts) = e.after_record["milestones_snapshot"]
                                .as_array()
                                .filter(|a| !a.is_empty())
                            {
                                snapshot(ui, facts, (e.id, "career_history_snapshot"));
                            }
                        });
                    ui.separator();
                }
            });
    }
}
fn related_label(data: &Snapshot, t: &career::Milestone) -> Option<(&'static str, String)> {
    if let Some(id) = t.goal_id {
        return Some((
            "goal",
            data.performance
                .goals
                .iter()
                .find(|g| {
                    g.id == id && g.employee_id == t.employee_id && g.company_id == data.company.id
                })
                .map(|g| g.title.clone())
                .unwrap_or_else(|| "Saved goal".into()),
        ));
    }
    if let Some(id) = t.training_id {
        return Some((
            "training",
            data.learning
                .training
                .iter()
                .find(|g| {
                    g.id == id && g.employee_id == t.employee_id && g.company_id == data.company.id
                })
                .map(|g| g.title.clone())
                .unwrap_or_else(|| "Saved training".into()),
        ));
    }
    t.certificate_id.map(|id| {
        (
            "certificate",
            data.learning
                .certificates
                .iter()
                .find(|g| {
                    g.id == id && g.employee_id == t.employee_id && g.company_id == data.company.id
                })
                .map(|g| g.title.clone())
                .unwrap_or_else(|| "Saved certificate".into()),
        )
    })
}
fn related_choice(ui: &mut Ui, data: &Snapshot, employee: Uuid, d: &mut MilestoneDraft) {
    let mut kind = if d.goal_id.is_some() {
        1
    } else if d.training_id.is_some() {
        2
    } else if d.certificate_id.is_some() {
        3
    } else {
        0
    };
    let old = kind;
    egui::ComboBox::from_id_salt("career_related_kind")
        .selected_text(
            [
                "No related record",
                "Related goal",
                "Related training",
                "Related certificate",
            ][kind],
        )
        .show_ui(ui, |ui| {
            for (i, label) in [
                "No related record",
                "Related goal",
                "Related training",
                "Related certificate",
            ]
            .iter()
            .enumerate()
            {
                ui.selectable_value(&mut kind, i, *label);
            }
        });
    if old != kind {
        d.goal_id = None;
        d.training_id = None;
        d.certificate_id = None;
    }
    let rows: Vec<(Uuid, String)> = match kind {
        1 => data
            .performance
            .goals
            .iter()
            .filter(|g| g.company_id == data.company.id && g.employee_id == employee)
            .map(|g| (g.id, g.title.clone()))
            .collect(),
        2 => data
            .learning
            .training
            .iter()
            .filter(|g| g.company_id == data.company.id && g.employee_id == employee)
            .map(|g| (g.id, g.title.clone()))
            .collect(),
        3 => data
            .learning
            .certificates
            .iter()
            .filter(|g| g.company_id == data.company.id && g.employee_id == employee)
            .map(|g| (g.id, g.title.clone()))
            .collect(),
        _ => Vec::new(),
    };
    if kind > 0 {
        let selected = match kind {
            1 => &mut d.goal_id,
            2 => &mut d.training_id,
            _ => &mut d.certificate_id,
        };
        egui::ComboBox::from_id_salt("career_related_record")
            .width(ui.available_width().min(500.0))
            .selected_text(
                selected
                    .and_then(|id| rows.iter().find(|r| r.0 == id).map(|r| r.1.as_str()))
                    .unwrap_or("Choose a related record (optional)"),
            )
            .show_ui(ui, |ui| {
                ui.selectable_value(selected, None, "No related record");
                for (id, title) in rows {
                    ui.selectable_value(selected, Some(id), title);
                }
            });
        ui.label("Context only. Reviewing this milestone does not complete the related record.");
    }
}
fn snapshot(ui: &mut Ui, facts: &[serde_json::Value], id: impl std::hash::Hash + std::fmt::Debug) {
    egui::CollapsingHeader::new(format!(
        "Milestone facts saved at completion ({})",
        facts.len()
    ))
    .id_salt(id)
    .default_open(true)
    .show(ui, |ui| {
        for (i, t) in facts.iter().enumerate() {
            let text = |key: &str| t[key].as_str().unwrap_or("Not set");
            egui::CollapsingHeader::new(format!("{} · {}", text("title"), text("status")))
                .id_salt((i, t["id"].as_str()))
                .default_open(i == 0)
                .show(ui, |ui| {
                    ui.label(format!(
                        "{} · Due: {} · Completed: {}",
                        if t["required"] == true {
                            "Required"
                        } else {
                            "Optional"
                        },
                        text("due_on"),
                        text("completed_on")
                    ));
                    for (key, label) in [
                        ("action_steps", "Actions"),
                        ("progress_note", "Progress"),
                        ("review_result", "Review result"),
                    ] {
                        if t[key].as_str().is_some_and(|s| !s.is_empty()) {
                            ui.strong(label);
                            ui.label(text(key));
                        }
                    }
                    if t["reference"].is_object() {
                        let r = &t["reference"];
                        ui.label(format!(
                            "Saved {} context: {} · {}",
                            r["kind"].as_str().unwrap_or("record"),
                            r["title"].as_str().unwrap_or("Not set"),
                            r["status"].as_str().unwrap_or("Not set")
                        ));
                        if let Some(progress) = r["progress"].as_i64() {
                            ui.label(format!("Progress at completion: {progress}%"));
                        }
                    }
                });
        }
    });
}
fn history_preview() -> bool {
    #[cfg(feature = "qa-screenshot")]
    {
        std::env::var("HRIS_PREVIEW_FORM").is_ok_and(|s| s == "career-history")
    }
    #[cfg(not(feature = "qa-screenshot"))]
    {
        false
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unrelated_refresh_preserves_draft() {
        let data = DemoStore::new().data;
        let mut state = State {
            plan_form: Some(PlanDraft::new(None, data.company.today())),
            ..Default::default()
        };
        assert!(state.accept(&data));
        assert!(state.editing());
    }
    #[test]
    fn changed_receipt_preserves_draft() {
        let mut data = DemoStore::new().data;
        let mut d = PlanDraft::new(Some(data.employees[1].id), data.company.today());
        d.title = "Synthetic plan".into();
        d.reason = "Synthetic assignment".into();
        let a = Change::PlanSave(d.clone());
        career::apply_demo(&mut data, &a, chrono::Utc::now()).unwrap();
        data.career.plans[0].title = "Another session changed this".into();
        let mut state = State {
            plan_form: Some(d),
            pending: Some(a),
            ..Default::default()
        };
        assert!(!state.accept(&data));
        assert!(state.editing());
        assert!(state.pending.is_none());
    }
}
#[cfg(feature = "qa-screenshot")]
impl HrisApp {
    pub(super) fn prepare_career_preview(&mut self) {
        let preview = std::env::var("HRIS_PREVIEW_FORM").unwrap_or_default();
        if !preview.starts_with("career-") || self.data.mode != Mode::Demo {
            return;
        }
        let access = self.data.access.clone();
        let mut data = self.data.clone();
        data.access.role = Role::Administrator;
        let today = data.company.today();
        let now = chrono::Utc::now();
        let mut employee = data.employees[0].clone();
        employee.id = Uuid::new_v4();
        employee.full_name = "Fictional Career Employee".into();
        employee.employee_code = "EMP-CAR-001".into();
        employee.reports_to = access.employee_id;
        employee.status = EmployeeStatus::Active;
        employee.joined_on = Some(today - Duration::days(180));
        let person = employee.id;
        data.employees.push(employee);
        let mut d = PlanDraft::new(Some(person), today - Duration::days(30));
        d.title = "Service leadership and professional development pathway".into();
        d.target_role = "Service team coordinator".into();
        d.target_department = "Operations".into();
        d.skills_focus =
            "Coaching, structured problem solving, accurate records and team communication".into();
        d.shared_notes="Fictional shared plan: agree on practical experience, review progress monthly and discuss career options without changing current employment.".into();
        d.target_on = (today + Duration::days(90)).to_string();
        d.reason = "Fictional career planning discussion".into();
        let pid = d.id;
        career::apply_demo(&mut data, &Change::PlanSave(d), now).unwrap();
        let mut l = crate::learning::TrainingDraft::new(Some(person));
        l.title = "Coaching and structured service handover".into();
        l.provider = "Fictional Learning Centre".into();
        l.reason = "Fictional related learning context".into();
        let learning_id = l.id;
        crate::learning::apply_demo(
            &mut data,
            &crate::learning::LearningAction::TrainingSave(l),
            now,
        )
        .unwrap();
        let mut t = MilestoneDraft::new(pid);
        t.title = "Lead a practice handover and collect coaching feedback".into();
        t.action_steps="Prepare a clear handover, facilitate a fictional team discussion and document feedback with the reporting Manager.".into();
        t.due_on = (today + Duration::days(14)).to_string();
        t.training_id = Some(learning_id);
        t.reason = "Fictional agreed development milestone".into();
        let tid = t.id;
        career::apply_demo(&mut data, &Change::MilestoneSave(t), now).unwrap();
        career::apply_demo(&mut data,&Change::Progress{id:tid,revision:0,note:"Fictional progress: handover outline drafted and first coaching discussion completed.".into(),reason:"Fictional shared progress".into()},now).unwrap();
        if preview == "career-completed" || preview == "career-history" {
            career::apply_demo(
                &mut data,
                &Change::MilestoneStatus {
                    id: tid,
                    revision: 1,
                    status: "done".into(),
                    completed_on: today.to_string(),
                    result:
                        "Fictional review: clear handover and constructive feedback demonstrated."
                            .into(),
                    reason: "Fictional independent milestone review".into(),
                },
                now,
            )
            .unwrap();
            career::apply_demo(&mut data,&Change::PlanStatus{id:pid,revision:0,status:"completed".into(),completed_on:today.to_string(),summary:"Fictional outcome: completed the agreed practical experience and identified the next coaching objective.".into(),reason:"Fictional independent career review".into()},now).unwrap();
        }
        data.access = access;
        self.data = data;
        self.employee_view = EmployeeView::Development;
        self.select_career_tab();
        self.career_state.employee = Some(person);
        self.career_state.resolved = true;
        if preview != "career-list" {
            self.career_state.selected = Some(pid);
        }
        match preview.as_str(){
 "career-plan-form"=>{let mut d=PlanDraft::existing(&self.data.career.plans[0]);d.reason="Fictional skills-focus clarification".into();self.career_state.plan_form=Some(d);},
 "career-milestone-form"=>{let mut d=MilestoneDraft::existing(&self.data.career.milestones[0]);d.reason="Fictional action and context clarification".into();self.career_state.milestone_form=Some(d);},
 "career-progress"=>self.career_state.decision=Some(Change::Progress{id:tid,revision:1,note:"Fictional practice session delivered; feedback review follows.".into(),reason:"Fictional progress update".into()}),
 "career-review"=>self.career_state.decision=Some(Change::MilestoneStatus{id:tid,revision:1,status:"done".into(),completed_on:today.to_string(),result:"Fictional review: agreed handover checklist demonstrated with accurate documentation.".into(),reason:"Fictional independent review".into()}),
 "career-waiver"=>self.career_state.decision=Some(Change::MilestoneStatus{id:tid,revision:1,status:"waived".into(),completed_on:String::new(),result:"Fictional alternative practice covers the agreed objective.".into(),reason:"Fictional HR waiver".into()}),
 "career-completion"=>{let access=self.data.access.clone();self.data.access.role=Role::Administrator;career::apply_demo(&mut self.data,&Change::MilestoneStatus{id:tid,revision:1,status:"done".into(),completed_on:today.to_string(),result:"Fictional reviewed practice accepted.".into(),reason:"Fictional independent review".into()},now).unwrap();self.data.access=access;self.career_state.decision=Some(Change::PlanStatus{id:pid,revision:0,status:"completed".into(),completed_on:today.to_string(),summary:"Fictional review outcome: agreed milestones achieved; next career discussion scheduled.".into(),reason:"Fictional development plan completion".into()});},
 _=>{},}
    }
}
