use super::*;
use crate::performance::{
    self, AssessmentDraft, AssignmentDraft, CycleDraft, GoalDraft, NoteDraft,
    PerformanceAction as Change,
};
#[derive(Clone, Copy, Default, PartialEq, Eq)]
enum Tab {
    #[default]
    Goals,
    Appraisals,
    Cycles,
    Training,
    Certifications,
    Career,
}
#[derive(Default)]
pub(super) struct State {
    tab: Tab,
    pub goal_form: Option<GoalDraft>,
    pub cycle_form: Option<CycleDraft>,
    pub assignment_form: Option<AssignmentDraft>,
    pub assessment_form: Option<AssessmentDraft>,
    pub note_form: Option<NoteDraft>,
    pub decision: Option<Change>,
    pub pending: Option<Change>,
    goal_id: Option<Uuid>,
    review_id: Option<Uuid>,
    employee: Option<Uuid>,
    search: String,
    goals_resolved: bool,
    reviews_cancelled: bool,
    cycles_closed: bool,
}
impl State {
    pub(super) fn open_notice(&mut self, id: Uuid) {
        self.tab = Tab::Appraisals;
        self.review_id = Some(id);
        self.employee = None;
        self.search.clear();
    }
    pub(super) fn open_learning_notice(&mut self, certificate: bool) {
        self.tab = if certificate {
            Tab::Certifications
        } else {
            Tab::Training
        };
    }
    pub fn editing(&self) -> bool {
        self.goal_form.is_some()
            || self.cycle_form.is_some()
            || self.assignment_form.is_some()
            || self.assessment_form.is_some()
            || self.note_form.is_some()
            || self.decision.is_some()
    }
    pub fn accept(&mut self, data: &Snapshot) -> bool {
        let Some(a) = self.pending.take() else {
            return true;
        };
        if !a.saved(data) {
            return false;
        }
        match a {
            Change::GoalSave(d) => self.goal_id = Some(d.id),
            Change::Assign(d) => self.review_id = Some(d.id),
            _ => {}
        }
        self.goal_form = None;
        self.cycle_form = None;
        self.assignment_form = None;
        self.assessment_form = None;
        self.note_form = None;
        self.decision = None;
        true
    }
}
fn area(ui: &mut Ui, label: &str, value: &mut String, rows: usize) {
    ui.label(label);
    ui.add(
        egui::TextEdit::multiline(value)
            .desired_rows(rows)
            .desired_width(f32::INFINITY),
    );
}
pub(super) fn employee_choice(
    ui: &mut Ui,
    id: &str,
    data: &Snapshot,
    selected: &mut Option<Uuid>,
    review: bool,
) {
    egui::ComboBox::from_id_salt(id)
        .width(ui.available_width().min(350.0))
        .selected_text(
            selected
                .map(|id| data.employee_name(id))
                .unwrap_or("Choose an employee"),
        )
        .show_ui(ui, |ui| {
            ui.selectable_value(selected, None, "Choose an employee");
            for e in data.active().filter(|e| {
                if review {
                    data.access
                        .allows(crate::staff_roles::Capability::DevelopmentAdministration)
                        && Some(e.id) != data.access.employee_id
                } else {
                    performance::manages_goal(data, e.id)
                }
            }) {
                ui.selectable_value(
                    selected,
                    Some(e.id),
                    format!("{} · {}", e.employee_code, e.full_name),
                );
            }
        });
}
fn transition_label(a: &Change) -> String {
    match a {
        Change::Progress { .. } => "Record goal progress".into(),
        Change::GoalStatus { status, .. } => match status.as_str() {
            "completed" => "Complete this goal",
            "cancelled" => "Cancel this goal",
            _ => "Reopen this goal",
        }
        .into(),
        Change::CycleStatus { status, .. } => if status == "closed" {
            "Close this review cycle"
        } else {
            "Reopen this review cycle"
        }
        .into(),
        Change::ReviewStatus { status, .. } => match status.as_str() {
            "submitted" => "Submit this appraisal to HR",
            "published" => "Publish this appraisal",
            "cancelled" => "Cancel this unpublished appraisal",
            _ => "Return this appraisal to draft",
        }
        .into(),
        _ => "Save performance change".into(),
    }
}
impl HrisApp {
    fn send_performance(&mut self, a: Change) {
        self.performance_state.pending = Some(a.clone());
        self.send(Command::Change(Action::Performance(a)));
    }
    #[cfg(feature = "qa-screenshot")]
    pub(super) fn select_learning_tab(&mut self, certificates: bool) {
        self.performance_state.tab = if certificates {
            Tab::Certifications
        } else {
            Tab::Training
        };
    }
    #[cfg(feature = "qa-screenshot")]
    pub(super) fn select_career_tab(&mut self) {
        self.performance_state.tab = Tab::Career;
    }
    pub(super) fn performance(&mut self, ui: &mut Ui) {
        if self.career_state.editing() {
            self.career(ui);
            return;
        }
        if self.learning_state.editing() {
            self.learning(ui, self.learning_state.certificates);
            return;
        }
        if self.performance_decision(ui) {
            return;
        }
        if self.performance_state.goal_form.is_some() {
            self.performance_goal_form(ui);
            return;
        }
        if self.performance_state.cycle_form.is_some() {
            self.performance_cycle_form(ui);
            return;
        }
        if self.performance_state.assignment_form.is_some() {
            self.performance_assignment_form(ui);
            return;
        }
        if self.performance_state.assessment_form.is_some() {
            self.performance_assessment_form(ui);
            return;
        }
        if self.performance_state.note_form.is_some() {
            self.performance_note_form(ui);
            return;
        }
        let hr = self
            .data
            .access
            .allows(crate::staff_roles::Capability::DevelopmentAdministration);
        if !hr && self.performance_state.tab == Tab::Cycles {
            self.performance_state.tab = Tab::Goals;
        }
        let previous = self.performance_state.tab;
        ui.horizontal_wrapped(|ui| {
            ui.selectable_value(&mut self.performance_state.tab, Tab::Goals, "Goals");
            ui.selectable_value(
                &mut self.performance_state.tab,
                Tab::Appraisals,
                "Appraisals",
            );
            ui.selectable_value(&mut self.performance_state.tab, Tab::Training, "Training");
            ui.selectable_value(
                &mut self.performance_state.tab,
                Tab::Certifications,
                "Certifications",
            );
            ui.selectable_value(&mut self.performance_state.tab, Tab::Career, "Career plans");
            if hr {
                ui.selectable_value(
                    &mut self.performance_state.tab,
                    Tab::Cycles,
                    "Review cycles",
                );
            }
        });
        if previous != self.performance_state.tab {
            self.performance_state.search.clear();
            self.performance_state.goal_id = None;
            self.performance_state.review_id = None;
        }

        ui.add_space(10.0);
        match self.performance_state.tab {
            Tab::Goals => self.performance_goals(ui),
            Tab::Appraisals => self.performance_reviews(ui),
            Tab::Cycles => self.performance_cycles(ui),
            Tab::Training => self.learning(ui, false),
            Tab::Certifications => self.learning(ui, true),
            Tab::Career => self.career(ui),
        }
    }
    fn performance_decision(&mut self, ui: &mut Ui) -> bool {
        let Some(mut a) = self.performance_state.decision.take() else {
            return false;
        };
        let mut save = false;
        let mut cancel = false;
        card(ui, |ui| {
            ui.label(RichText::new(transition_label(&a)).strong().size(19.0));
            match &mut a {
                Change::Progress {
                    id, progress, note, ..
                } => {
                    if let Ok(g) = performance::goal(&self.data, *id) {
                        ui.label(format!("{} · {}", g.employee_name, g.title));
                    }
                    ui.add(egui::Slider::new(progress, 0..=100).text("Recorded progress (%)"));
                    area(ui, "Progress note visible with the goal", note, 3);
                }
                Change::ReviewStatus { status, .. } if status == "submitted" => {
                    ui.label("Submission locks this assessment and saves the current goal facts for the review period. HR can return it to draft or publish it.");
                }
                Change::ReviewStatus { status, .. } if status == "published" => {
                    ui.label("Publication makes this review available to the employee in their future portal. The assessment and submitted goal facts become read-only. Corrections require a follow-up cycle; private HR notes stay separate.");
                }
                Change::CycleStatus { status, .. } if status == "closed" => {
                    ui.label("Close only after every appraisal is published or cancelled. Reopening the cycle keeps published reviews unchanged.");
                }
                _ => {}
            }
            field(ui, "Reason / evidence reference *", a.reason_mut(), "");
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
            self.performance_state.decision = Some(a.clone());
        }
        if save {
            self.send_performance(a);
        }
        true
    }
    fn performance_filter(&mut self, ui: &mut Ui) {
        ui.horizontal_wrapped(|ui| {
            egui::ComboBox::from_id_salt("performance_employee_filter")
                .width(280.0)
                .selected_text(
                    self.performance_state
                        .employee
                        .map(|id| self.data.employee_name(id))
                        .unwrap_or("All employees in your access"),
                )
                .show_ui(ui, |ui| {
                    ui.selectable_value(
                        &mut self.performance_state.employee,
                        None,
                        "All employees in your access",
                    );
                    for e in &self.data.employees {
                        ui.selectable_value(
                            &mut self.performance_state.employee,
                            Some(e.id),
                            format!(
                                "{} · {}{}",
                                e.employee_code,
                                e.full_name,
                                if e.status == EmployeeStatus::Archived {
                                    " (archived)"
                                } else {
                                    ""
                                }
                            ),
                        );
                    }
                });
            ui.add(
                egui::TextEdit::singleline(&mut self.performance_state.search)
                    .hint_text("Search employee, title or cycle")
                    .desired_width(260.0),
            );
        });
    }
    fn performance_goals(&mut self, ui: &mut Ui) {
        if let Some(g) = self
            .performance_state
            .goal_id
            .and_then(|id| performance::goal(&self.data, id).ok())
            .cloned()
        {
            card(ui, |ui| {
                if ui.button("< All goals").clicked() {
                    self.performance_state.goal_id = None;
                }
            });
            ui.add_space(10.0);
            self.performance_goal_card(ui, &g, true);
            return;
        }
        card(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                if self
                    .data
                    .active()
                    .any(|e| performance::manages_goal(&self.data, e.id))
                    && ui.button("Add goal").clicked()
                {
                    let person = self
                        .performance_state
                        .employee
                        .filter(|id| performance::manages_goal(&self.data, *id));
                    self.performance_state.goal_form = Some(GoalDraft::new(&self.data, person));
                }
                ui.checkbox(
                    &mut self.performance_state.goals_resolved,
                    "Include completed / cancelled",
                );
            });
            self.performance_filter(ui);
            if !self
                .data
                .access
                .allows(crate::staff_roles::Capability::DevelopmentAdministration)
            {
                ui.label("Your Manager or HR assigns goals. You can record your own progress; current reporting Managers manage their team goals.");
            }
        });
        let q = self.performance_state.search.to_lowercase();
        let mut rows: Vec<_> = self
            .data
            .performance
            .goals
            .iter()
            .filter(|g| {
                performance::goal(&self.data, g.id).is_ok()
                    && (self.performance_state.goals_resolved || g.status == "active")
                    && self
                        .performance_state
                        .employee
                        .is_none_or(|e| e == g.employee_id)
                    && format!("{} {} {}", g.employee_code, g.employee_name, g.title)
                        .to_lowercase()
                        .contains(&q)
            })
            .cloned()
            .collect();
        rows.sort_by_key(|g| (g.due_on, g.employee_code.clone(), g.id));
        if rows.is_empty() {
            empty(ui, "No matching goals.");
        }
        for g in rows {
            ui.add_space(10.0);
            self.performance_goal_card(ui, &g, false);
        }
    }
    fn performance_goal_card(&mut self, ui: &mut Ui, g: &performance::Goal, detail: bool) {
        card(ui, |ui| {
            ui.label(RichText::new(&g.title).strong().size(18.0));
            ui.label(format!("{} · {}", g.employee_code, g.employee_name));
            ui.horizontal_wrapped(|ui| {
                badge(ui, &g.status, g.status == "completed");
                ui.label(format!("{} to {}", g.starts_on, g.due_on));
            });
            ui.add(
                egui::ProgressBar::new(g.progress as f32 / 100.0)
                    .text(format!("{}% recorded progress", g.progress))
                    .desired_width(ui.available_width().min(420.0)),
            );
            if !g.measure.is_empty() {
                ui.label(RichText::new("Measure").strong());
                ui.label(&g.measure);
            }
            if detail && !g.description.is_empty() {
                ui.label(&g.description);
            }
            if !g.progress_note.is_empty() {
                ui.label(&g.progress_note);
            }
            ui.horizontal_wrapped(|ui| {
                if !detail && ui.button("Open goal").clicked() {
                    self.performance_state.goal_id = Some(g.id);
                }
                if performance::manages_goal(&self.data, g.employee_id)
                    && g.status == "active"
                    && ui.button("Edit goal").clicked()
                {
                    self.performance_state.goal_form = Some(GoalDraft::existing(g));
                }
                let progress = Change::Progress {
                    id: g.id,
                    revision: g.revision,
                    progress: g.progress,
                    note: g.progress_note.clone(),
                    reason: "Preview availability".into(),
                };
                if progress.payload(&self.data).is_ok() && ui.button("Record progress").clicked() {
                    let mut a = progress;
                    a.reason_mut().clear();
                    self.performance_state.decision = Some(a);
                }
                for (status, label) in [
                    ("completed", "Mark completed"),
                    ("active", "Reopen goal"),
                    ("cancelled", "Cancel goal"),
                ] {
                    let a = Change::GoalStatus {
                        id: g.id,
                        revision: g.revision,
                        status: status.into(),
                        reason: "Preview availability".into(),
                    };
                    if a.payload(&self.data).is_ok() && ui.button(label).clicked() {
                        let mut a = a;
                        a.reason_mut().clear();
                        self.performance_state.decision = Some(a);
                    }
                }
            });
            if detail {
                let events: Vec<_> = performance::visible_events(&self.data)
                    .into_iter()
                    .filter(|e| e.goal_id == Some(g.id))
                    .cloned()
                    .collect();
                egui::CollapsingHeader::new("Goal history")
                    .default_open(history_preview())
                    .show(ui, |ui| self.performance_history(ui, &events));
            }
        });
    }
    fn performance_reviews(&mut self, ui: &mut Ui) {
        if let Some(r) = self
            .performance_state
            .review_id
            .and_then(|id| performance::review(&self.data, id).ok())
            .cloned()
        {
            self.performance_review_detail(ui, &r);
            return;
        }
        card(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                if self
                    .data
                    .access
                    .allows(crate::staff_roles::Capability::DevelopmentAdministration)
                    && ui.button("Assign appraisal").clicked()
                {
                    let person = self
                        .performance_state
                        .employee
                        .filter(|id| Some(*id) != self.data.access.employee_id);
                    self.performance_state.assignment_form =
                        Some(AssignmentDraft::new(person, None));
                }
                ui.checkbox(
                    &mut self.performance_state.reviews_cancelled,
                    "Include cancelled",
                );
            });
            self.performance_filter(ui);
            if !self
                .data
                .access
                .allows(crate::staff_roles::Capability::DevelopmentAdministration)
            {
                ui.label("Review your assigned current reports and read your own published appraisals. Drafts and private HR notes remain restricted.");
            }
        });
        let q = self.performance_state.search.to_lowercase();
        let mut rows: Vec<_> = self
            .data
            .performance
            .reviews
            .iter()
            .filter(|r| {
                performance::review(&self.data, r.id).is_ok()
                    && (self.performance_state.reviews_cancelled || r.status != "cancelled")
                    && self
                        .performance_state
                        .employee
                        .is_none_or(|e| e == r.employee_id)
                    && format!("{} {} {}", r.employee_code, r.employee_name, r.cycle_name)
                        .to_lowercase()
                        .contains(&q)
            })
            .cloned()
            .collect();
        rows.sort_by_key(|r| (r.due_on, r.employee_code.clone(), r.id));
        if rows.is_empty() {
            empty(ui, "No matching appraisals.");
        }
        for r in rows {
            ui.add_space(10.0);
            card(ui, |ui| {
                ui.label(
                    RichText::new(format!("{} · {}", r.employee_code, r.employee_name))
                        .strong()
                        .size(18.0),
                );
                ui.horizontal_wrapped(|ui| {
                    badge(ui, &r.status, r.status == "published");
                    ui.label(&r.cycle_name);
                });
                ui.label(format!(
                    "Reviewer: {} · Review due {}",
                    r.reviewer_name, r.due_on
                ));
                if ui.button("Open appraisal").clicked() {
                    self.performance_state.review_id = Some(r.id);
                }
            });
        }
    }
    fn performance_review_detail(&mut self, ui: &mut Ui, r: &performance::Review) {
        let hr = self
            .data
            .access
            .allows(crate::staff_roles::Capability::DevelopmentAdministration);
        card(ui, |ui| {
            if ui.button("< All appraisals").clicked() {
                self.performance_state.review_id = None;
            }
            ui.label(
                RichText::new(format!("{} · {}", r.employee_code, r.employee_name))
                    .strong()
                    .size(20.0),
            );
            ui.horizontal_wrapped(|ui| {
                badge(ui, &r.status, r.status == "published");
                ui.label(&r.cycle_name);
            });
            ui.label(format!(
                "Period: {} to {} · Review due {}",
                r.starts_on, r.ends_on, r.due_on
            ));
            ui.label(format!("Assigned reviewer: {}", r.reviewer_name));
            ui.horizontal_wrapped(|ui| {
                let assessment = Change::Assessment(AssessmentDraft {
                    reason: "Preview availability".into(),
                    ..AssessmentDraft::existing(r)
                });
                if assessment.payload(&self.data).is_ok() && ui.button("Edit assessment").clicked()
                {
                    self.performance_state.assessment_form = Some(AssessmentDraft::existing(r));
                }
                if hr
                    && r.status != "published"
                    && Some(r.employee_id) != self.data.access.employee_id
                    && ui.button("Reassign reviewer").clicked()
                {
                    self.performance_state.assignment_form = Some(AssignmentDraft::existing(r));
                }
                for (status, label) in [
                    ("submitted", "Submit to HR"),
                    ("published", "Publish appraisal"),
                    ("draft", "Return to draft"),
                    ("cancelled", "Cancel appraisal"),
                ] {
                    let a = Change::ReviewStatus {
                        id: r.id,
                        revision: r.revision,
                        status: status.into(),
                        reason: "Preview availability".into(),
                    };
                    if a.payload(&self.data).is_ok() && ui.button(label).clicked() {
                        let mut a = a;
                        a.reason_mut().clear();
                        self.performance_state.decision = Some(a);
                    }
                }
            });
            if r.status == "published" {
                ui.label("This published appraisal is read-only. A follow-up cycle records corrections while retaining this review.");
            }
        });
        ui.add_space(10.0);
        card(ui, |ui| {
            ui.label(RichText::new("Assessment and feedback").strong().size(18.0));
            ui.label(
                r.rating
                    .map(|n| format!("Overall rating: {n} / 5"))
                    .unwrap_or_else(|| "Not rated yet".into()),
            );
            for (label, value) in [
                ("Assessment", &r.assessment),
                ("Strengths", &r.strengths),
                ("Development feedback", &r.development),
            ] {
                ui.label(RichText::new(label).strong());
                ui.add(
                    egui::Label::new(if value.is_empty() {
                        "Not recorded"
                    } else {
                        value
                    })
                    .wrap(),
                );
            }
            if let Some(t) = r.submitted_at {
                ui.label(format!(
                    "Submitted {}",
                    t.with_timezone(&self.data.company.tz())
                        .format("%b %d, %Y %H:%M")
                ));
            }
            if let Some(t) = r.published_at {
                ui.label(format!(
                    "Published {}",
                    t.with_timezone(&self.data.company.tz())
                        .format("%b %d, %Y %H:%M")
                ));
            }
        });
        ui.add_space(10.0);
        card(ui, |ui| {
            ui.label(
                RichText::new("Goals recorded at submission")
                    .strong()
                    .size(18.0),
            );
            if r.submitted_at.is_none() {
                ui.label("Submitting captures the current, non-cancelled goals overlapping this review period.");
            } else if r.goals_snapshot.is_empty() {
                ui.label("No goals were recorded for this submitted period.");
            } else {
                ui.label("These saved facts remain unchanged when current goals progress.");
            }
            for g in &r.goals_snapshot {
                ui.separator();
                ui.label(RichText::new(&g.title).strong());
                ui.label(format!(
                    "{}% recorded · {} · {} to {}",
                    g.progress, g.status, g.starts_on, g.due_on
                ));
                if !g.measure.is_empty() {
                    ui.label(&g.measure);
                }
            }
        });
        if self
            .data
            .access
            .allows(crate::staff_roles::Capability::PrivateHrNotes)
        {
            ui.add_space(10.0);
            let note = self
                .data
                .performance
                .notes
                .iter()
                .find(|n| n.review_id == r.id)
                .cloned();
            card(ui, |ui| {
                ui.label(RichText::new("Private HR notes").strong().size(18.0));
                ui.label("Visible only to HR and Administrators; never part of the employee's published review.");
                if let Some(n) = &note {
                    ui.label(if n.note.is_empty() {
                        "No private note recorded"
                    } else {
                        &n.note
                    });
                } else {
                    ui.label("No private note recorded");
                }
                if Some(r.employee_id) != self.data.access.employee_id
                    && ui.button("Edit private note").clicked()
                {
                    self.performance_state.note_form = Some(
                        note.as_ref()
                            .map(NoteDraft::existing)
                            .unwrap_or_else(|| NoteDraft::new(r.id)),
                    );
                }
                let events: Vec<_> = performance::visible_events(&self.data)
                    .into_iter()
                    .filter(|e| e.review_id == Some(r.id) && e.note_id.is_some())
                    .cloned()
                    .collect();
                egui::CollapsingHeader::new("Private note history")
                    .default_open(history_preview())
                    .show(ui, |ui| self.performance_history(ui, &events));
            });
        }
        let events: Vec<_> = performance::visible_events(&self.data)
            .into_iter()
            .filter(|e| e.review_id == Some(r.id) && e.note_id.is_none())
            .cloned()
            .collect();
        ui.add_space(10.0);
        card(ui, |ui| {
            egui::CollapsingHeader::new("Appraisal history")
                .default_open(history_preview())
                .show(ui, |ui| self.performance_history(ui, &events));
        });
    }
    fn performance_cycles(&mut self, ui: &mut Ui) {
        if !self
            .data
            .access
            .allows(crate::staff_roles::Capability::DevelopmentAdministration)
        {
            return;
        }
        card(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                if ui.button("Add review cycle").clicked() {
                    self.performance_state.cycle_form = Some(CycleDraft::new(&self.data));
                }
                ui.checkbox(&mut self.performance_state.cycles_closed, "Include closed");
            });
            ui.add(
                egui::TextEdit::singleline(&mut self.performance_state.search)
                    .hint_text("Search review cycle")
                    .desired_width(320.0),
            );
            ui.label("Cycle dates and labels freeze once an appraisal is assigned. Publish or cancel unfinished reviews before closing.");
        });
        let q = self.performance_state.search.to_lowercase();
        let rows: Vec<_> = self
            .data
            .performance
            .cycles
            .iter()
            .filter(|c| {
                (self.performance_state.cycles_closed || c.status == "open")
                    && c.name.to_lowercase().contains(&q)
            })
            .cloned()
            .collect();
        if rows.is_empty() {
            empty(ui, "No matching review cycles.");
        }
        for c in rows {
            ui.add_space(10.0);
            card(ui, |ui| {
                ui.label(RichText::new(&c.name).strong().size(18.0));
                ui.horizontal_wrapped(|ui| {
                    badge(ui, &c.status, c.status == "open");
                    ui.label(format!(
                        "{} to {} · Review due {}",
                        c.starts_on, c.ends_on, c.due_on
                    ));
                });
                let pending = self
                    .data
                    .performance
                    .reviews
                    .iter()
                    .filter(|r| {
                        r.cycle_id == c.id && ["draft", "submitted"].contains(&r.status.as_str())
                    })
                    .count();
                ui.label(format!("{pending} unfinished appraisals"));
                ui.horizontal_wrapped(|ui| {
                    if c.status == "open" {
                        if !self
                            .data
                            .performance
                            .reviews
                            .iter()
                            .any(|r| r.cycle_id == c.id)
                            && ui.button("Edit cycle").clicked()
                        {
                            self.performance_state.cycle_form = Some(CycleDraft::existing(&c));
                        }
                        if ui.button("Assign employee").clicked() {
                            self.performance_state.assignment_form = Some(AssignmentDraft::new(
                                self.performance_state
                                    .employee
                                    .filter(|id| Some(*id) != self.data.access.employee_id),
                                Some(c.id),
                            ));
                        }
                    }
                    let status = if c.status == "open" { "closed" } else { "open" };
                    let a = Change::CycleStatus {
                        id: c.id,
                        revision: c.revision,
                        status: status.into(),
                        reason: "Preview availability".into(),
                    };
                    if a.payload(&self.data).is_ok()
                        && ui
                            .button(if status == "closed" {
                                "Close cycle"
                            } else {
                                "Reopen cycle"
                            })
                            .clicked()
                    {
                        let mut a = a;
                        a.reason_mut().clear();
                        self.performance_state.decision = Some(a);
                    }
                });
                let events: Vec<_> = performance::visible_events(&self.data)
                    .into_iter()
                    .filter(|e| e.cycle_id == Some(c.id) && e.review_id.is_none())
                    .cloned()
                    .collect();
                egui::CollapsingHeader::new("Cycle history")
                    .id_salt(c.id)
                    .default_open(history_preview())
                    .show(ui, |ui| self.performance_history(ui, &events));
            });
        }
    }
    fn performance_history(&self, ui: &mut Ui, events: &[performance::Event]) {
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
                });
            ui.separator();
        }
    }

    fn performance_goal_form(&mut self, ui: &mut Ui) {
        let Some(mut d) = self.performance_state.goal_form.take() else {
            return;
        };
        let mut save = false;
        let mut cancel = false;
        card(ui, |ui| {
            ui.label(
                RichText::new(if d.original.is_some() {
                    "Edit goal"
                } else {
                    "Assign a goal"
                })
                .strong()
                .size(20.0),
            );
            if d.original.is_none() {
                ui.label("Employee");
                employee_choice(ui, "goal_employee", &self.data, &mut d.employee_id, false);
            } else if let Some(id) = d.employee_id {
                ui.label(self.data.employee_name(id));
            }
            field(ui, "Goal title *", &mut d.title, "");
            area(ui, "Description", &mut d.description, 3);
            area(ui, "How progress is measured", &mut d.measure, 2);
            if ui.available_width() >= 500.0 {
                ui.columns(2, |cols| {
                    field(&mut cols[0], "Starts on *", &mut d.starts_on, "YYYY-MM-DD");
                    field(&mut cols[1], "Due on *", &mut d.due_on, "YYYY-MM-DD");
                });
            } else {
                field(ui, "Starts on *", &mut d.starts_on, "YYYY-MM-DD");
                field(ui, "Due on *", &mut d.due_on, "YYYY-MM-DD");
            }
            field(
                ui,
                "Save reason visible in goal history *",
                &mut d.reason,
                "",
            );
            ui.label(
                "Record progress separately. Completing a goal requires 100% recorded progress.",
            );
            let valid = Change::GoalSave(d.clone()).payload(&self.data);
            if let Err(e) = &valid {
                ui.label(RichText::new(e.to_string()).color(muted(ui)));
            }
            ui.horizontal_wrapped(|ui| {
                save = ui
                    .add_enabled(valid.is_ok(), egui::Button::new("Save goal"))
                    .clicked();
                cancel = ui.button("Discard goal draft").clicked();
            });
        });
        if !cancel {
            self.performance_state.goal_form = Some(d.clone());
        }
        if save {
            self.send_performance(Change::GoalSave(d));
        }
    }
    fn performance_cycle_form(&mut self, ui: &mut Ui) {
        let Some(mut d) = self.performance_state.cycle_form.take() else {
            return;
        };
        let mut save = false;
        let mut cancel = false;
        card(ui, |ui| {
            ui.label(RichText::new("Review cycle").strong().size(20.0));
            field(
                ui,
                "Cycle name *",
                &mut d.name,
                "For example, annual or probation review",
            );
            if ui.available_width() >= 500.0 {
                ui.columns(2, |cols| {
                    field(
                        &mut cols[0],
                        "Period starts *",
                        &mut d.starts_on,
                        "YYYY-MM-DD",
                    );
                    field(&mut cols[1], "Period ends *", &mut d.ends_on, "YYYY-MM-DD");
                });
            } else {
                field(ui, "Period starts *", &mut d.starts_on, "YYYY-MM-DD");
                field(ui, "Period ends *", &mut d.ends_on, "YYYY-MM-DD");
            }
            field(ui, "Review deadline *", &mut d.due_on, "YYYY-MM-DD");
            field(ui, "Save reason *", &mut d.reason, "");
            ui.label("Reviewers are assigned separately. Dates and labels freeze when a review is assigned; published reviews remain unchanged when the cycle is reopened.");
            let valid = Change::CycleSave(d.clone()).payload(&self.data);
            if let Err(e) = &valid {
                ui.label(RichText::new(e.to_string()).color(muted(ui)));
            }
            ui.horizontal_wrapped(|ui| {
                save = ui
                    .add_enabled(valid.is_ok(), egui::Button::new("Save cycle"))
                    .clicked();
                cancel = ui.button("Discard cycle draft").clicked();
            });
        });
        if !cancel {
            self.performance_state.cycle_form = Some(d.clone());
        }
        if save {
            self.send_performance(Change::CycleSave(d));
        }
    }
    fn performance_assignment_form(&mut self, ui: &mut Ui) {
        let Some(mut d) = self.performance_state.assignment_form.take() else {
            return;
        };
        let mut save = false;
        let mut cancel = false;
        card(ui, |ui| {
            ui.label(
                RichText::new(if d.original.is_some() {
                    "Reassign appraisal"
                } else {
                    "Assign appraisal"
                })
                .strong()
                .size(20.0),
            );
            if let Some(r) = &d.original {
                ui.label(format!(
                    "{} · {} · {}",
                    r.employee_code, r.employee_name, r.cycle_name
                ));
                ui.label("Reassignment clears the unfinished assessment. HR retains the previous draft in history; the new Manager cannot read it, and the old reviewer loses access.");
            } else {
                ui.label("Employee");
                let previous = d.employee_id;
                employee_choice(ui, "review_employee", &self.data, &mut d.employee_id, true);
                if previous != d.employee_id {
                    d.reviewer_employee_id = None;
                }
                ui.label("Open review cycle");
                egui::ComboBox::from_id_salt("review_cycle")
                    .width(ui.available_width().min(350.0))
                    .selected_text(
                        d.cycle_id
                            .and_then(|id| self.data.performance.cycles.iter().find(|c| c.id == id))
                            .map(|c| c.name.as_str())
                            .unwrap_or("Choose a cycle"),
                    )
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut d.cycle_id, None, "Choose a cycle");
                        for c in self
                            .data
                            .performance
                            .cycles
                            .iter()
                            .filter(|c| c.status == "open")
                        {
                            ui.selectable_value(&mut d.cycle_id, Some(c.id), &c.name);
                        }
                    });
            }
            ui.label("Assigned reviewer");
            egui::ComboBox::from_id_salt("performance_reviewer")
                .width(ui.available_width().min(350.0))
                .selected_text(
                    d.reviewer_employee_id
                        .and_then(|id| {
                            self.data
                                .recruiting
                                .interviewers
                                .iter()
                                .find(|s| s.employee_id == id)
                        })
                        .map(|s| s.full_name.as_str())
                        .unwrap_or("Choose an eligible reviewer"),
                )
                .show_ui(ui, |ui| {
                    ui.selectable_value(
                        &mut d.reviewer_employee_id,
                        None,
                        "Choose an eligible reviewer",
                    );
                    for s in self.data.recruiting.interviewers.iter().filter(|s| {
                        d.employee_id.is_some_and(|id| {
                            performance::eligible_reviewer(&self.data, id, s.employee_id)
                        })
                    }) {
                        ui.selectable_value(
                            &mut d.reviewer_employee_id,
                            Some(s.employee_id),
                            format!("{} · {}", s.full_name, s.role_label()),
                        );
                    }
                });
            ui.label("Choose the employee's current reporting Manager or another active HR/Administrator. Another HR staff member must manage your own appraisal.");
            field(ui, "Assignment reason *", &mut d.reason, "");
            let valid = Change::Assign(d.clone()).payload(&self.data);
            if let Err(e) = &valid {
                ui.label(RichText::new(e.to_string()).color(muted(ui)));
            }
            ui.horizontal_wrapped(|ui| {
                save = ui
                    .add_enabled(
                        valid.is_ok(),
                        egui::Button::new(if d.original.is_some() {
                            "Save reassignment"
                        } else {
                            "Assign appraisal"
                        }),
                    )
                    .clicked();
                cancel = ui.button("Discard assignment draft").clicked();
            });
        });
        if !cancel {
            self.performance_state.assignment_form = Some(d.clone());
        }
        if save {
            self.send_performance(Change::Assign(d));
        }
    }
    fn performance_assessment_form(&mut self, ui: &mut Ui) {
        let Some(mut d) = self.performance_state.assessment_form.take() else {
            return;
        };
        let mut save = false;
        let mut cancel = false;
        card(ui, |ui| {
            ui.label(
                RichText::new("Draft appraisal assessment")
                    .strong()
                    .size(20.0),
            );
            if let Ok(r) = performance::review(&self.data, d.id) {
                ui.label(format!(
                    "{} · {} · {}",
                    r.employee_code, r.employee_name, r.cycle_name
                ));
            }
            ui.label("Draft feedback is visible to the assigned reviewer and HR. HR publishes it after submission. Use separate private HR notes for restricted context.");
            ui.label("Overall rating (prototype scale: 1 lowest, 5 highest)");
            egui::ComboBox::from_id_salt("performance_rating")
                .selected_text(
                    d.rating
                        .map(|n| format!("{n} / 5"))
                        .unwrap_or_else(|| "Not rated".into()),
                )
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut d.rating, None, "Not rated");
                    for n in 1..=5 {
                        ui.selectable_value(&mut d.rating, Some(n), format!("{n} / 5"));
                    }
                });
            area(
                ui,
                "Assessment (required for submission)",
                &mut d.assessment,
                4,
            );
            area(ui, "Strengths", &mut d.strengths, 3);
            area(
                ui,
                "Development feedback and agreed next steps",
                &mut d.development,
                3,
            );
            field(ui, "Save reason *", &mut d.reason, "");
            let valid = Change::Assessment(d.clone()).payload(&self.data);
            if let Err(e) = &valid {
                ui.label(RichText::new(e.to_string()).color(muted(ui)));
            }
            ui.horizontal_wrapped(|ui| {
                save = ui
                    .add_enabled(valid.is_ok(), egui::Button::new("Save assessment draft"))
                    .clicked();
                cancel = ui.button("Discard assessment edits").clicked();
            });
        });
        if !cancel {
            self.performance_state.assessment_form = Some(d.clone());
        }
        if save {
            self.send_performance(Change::Assessment(d));
        }
    }
    fn performance_note_form(&mut self, ui: &mut Ui) {
        let Some(mut d) = self.performance_state.note_form.take() else {
            return;
        };
        let mut save = false;
        let mut cancel = false;
        card(ui, |ui| {
            ui.label(
                RichText::new("Private HR appraisal note")
                    .strong()
                    .size(20.0),
            );
            if let Ok(r) = performance::review(&self.data, d.review_id) {
                ui.label(format!(
                    "{} · {} · {}",
                    r.employee_code, r.employee_name, r.cycle_name
                ));
            }
            ui.label("Only HR and Administrators receive these notes or their history. Saving does not change the published review.");
            area(ui, "Private note (up to 2,000 characters)", &mut d.note, 5);
            field(ui, "Private change reason *", &mut d.reason, "");
            let valid = Change::NoteSave(d.clone()).payload(&self.data);
            if let Err(e) = &valid {
                ui.label(RichText::new(e.to_string()).color(muted(ui)));
            }
            ui.horizontal_wrapped(|ui| {
                save = ui
                    .add_enabled(valid.is_ok(), egui::Button::new("Save private note"))
                    .clicked();
                cancel = ui.button("Discard note edits").clicked();
            });
        });
        if !cancel {
            self.performance_state.note_form = Some(d.clone());
        }
        if save {
            self.send_performance(Change::NoteSave(d));
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
    pub(super) fn prepare_performance_preview(&mut self) {
        let preview = std::env::var("HRIS_PREVIEW_FORM").unwrap_or_default();
        if !preview.starts_with("perf-") || self.data.mode != Mode::Demo {
            return;
        }
        let access = self.data.access.clone();
        let mut data = self.data.clone();
        data.access.role = Role::Administrator;
        let own = access.employee_id.unwrap();
        let mut e = data.employees[0].clone();
        e.id = Uuid::new_v4();
        e.employee_code = "EMP-DEV-001".into();
        e.full_name = "Fictional Development Employee".into();
        e.reports_to = Some(own);
        e.joined_on = Some("2020-01-01".parse().unwrap());
        e.status = EmployeeStatus::Active;
        let person = e.id;
        data.employees.push(e);
        data.recruiting.interviewers = data
            .accounts
            .iter()
            .filter(|a| a.active && a.role.uses_desktop())
            .filter_map(|a| {
                a.employee_id.map(|id| crate::recruiting::Interviewer {
                    employee_id: id,
                    full_name: data.employee_name(id).into(),
                    role: a.role,
                    roles: None,
                })
            })
            .collect();
        if !data
            .recruiting
            .interviewers
            .iter()
            .any(|s| s.employee_id == own)
        {
            data.recruiting
                .interviewers
                .push(crate::recruiting::Interviewer {
                    employee_id: own,
                    full_name: data.employee_name(own).into(),
                    role: access.role,
                    roles: None,
                });
        }
        let mut gd = GoalDraft::new(&data, Some(person));
        gd.title = "Improve service quality through a documented handover checklist".into();
        gd.description="Fictional objective: agree measurable service standards, review examples each week and record the progress evidence without including private client information.".into();
        gd.measure="Complete the agreed checklist and review delivery examples with the reporting Manager.".into();
        gd.due_on = (data.company.today() + Duration::days(30)).to_string();
        gd.reason = "Fictional goal agreement".into();
        let goal = gd.id;
        performance::apply_demo(&mut data, &Change::GoalSave(gd), chrono::Utc::now()).unwrap();
        performance::apply_demo(
            &mut data,
            &Change::Progress {
                id: goal,
                revision: 0,
                progress: 35,
                note: "Fictional initial checklist review completed.".into(),
                reason: "Fictional measured progress".into(),
            },
            chrono::Utc::now(),
        )
        .unwrap();
        let mut cd = CycleDraft::new(&data);
        cd.name = "Fictional annual development review".into();
        cd.reason = "Fictional review cycle".into();
        let cycle = cd.id;
        performance::apply_demo(&mut data, &Change::CycleSave(cd), chrono::Utc::now()).unwrap();
        let mut ad = AssignmentDraft::new(Some(person), Some(cycle));
        ad.reviewer_employee_id = Some(own);
        ad.reason = "Fictional reviewer assignment".into();
        let review = ad.id;
        performance::apply_demo(&mut data, &Change::Assign(ad), chrono::Utc::now()).unwrap();
        let mut draft = AssessmentDraft::existing(&data.performance.reviews[0]);
        draft.rating = Some(4);
        draft.assessment="Fictional assessment: consistently follows the agreed process, documents work clearly and asks for feedback when priorities change. Discuss the specific examples together before agreeing the next development steps.".into();
        draft.strengths =
            "Clear communication, dependable follow-through and careful documentation.".into();
        draft.development="Continue structured training, review progress monthly and agree the next measurable learning objective.".into();
        draft.reason = "Fictional assessment prepared".into();
        performance::apply_demo(&mut data, &Change::Assessment(draft), chrono::Utc::now()).unwrap();
        let mut nd = NoteDraft::new(review);
        nd.note="Fictional private HR context. These notes and their history stay separate from the employee's published review and the assigned Manager's feedback.".into();
        nd.reason = "Fictional private HR follow-up".into();
        performance::apply_demo(&mut data, &Change::NoteSave(nd), chrono::Utc::now()).unwrap();
        if preview.contains("submitted")
            || preview.contains("published")
            || preview.contains("publish")
            || preview.contains("history")
        {
            let revision = data.performance.reviews[0].revision;
            performance::apply_demo(
                &mut data,
                &Change::ReviewStatus {
                    id: review,
                    revision,
                    status: "submitted".into(),
                    reason: "Fictional submitted appraisal".into(),
                },
                chrono::Utc::now(),
            )
            .unwrap();
        }
        if preview.contains("published") || preview.contains("history") {
            let revision = data.performance.reviews[0].revision;
            performance::apply_demo(
                &mut data,
                &Change::ReviewStatus {
                    id: review,
                    revision,
                    status: "published".into(),
                    reason: "Fictional HR publication".into(),
                },
                chrono::Utc::now(),
            )
            .unwrap();
            performance::apply_demo(
                &mut data,
                &Change::Progress {
                    id: goal,
                    revision: 1,
                    progress: 75,
                    note: "Fictional later progress; the appraisal retains 35%.".into(),
                    reason: "Fictional later progress evidence".into(),
                },
                chrono::Utc::now(),
            )
            .unwrap();
        }
        data.access = access;
        self.data = data;
        self.page = Page::Employees;
        self.employee_view = EmployeeView::Development;
        self.performance_state.employee = Some(person);
        match preview.as_str() {
            "perf-goal-form" => {
                let mut d = GoalDraft::existing(&self.data.performance.goals[0]);
                d.reason = "Fictional objective clarification".into();
                self.performance_state.goal_form = Some(d);
            }
            "perf-goal-detail" => {
                self.performance_state.goal_id = Some(goal);
            }
            "perf-progress" => {
                self.performance_state.decision = Some(Change::Progress {
                    id: goal,
                    revision: self.data.performance.goals[0].revision,
                    progress: 45,
                    note: "Fictional evidence recorded at the weekly check-in.".into(),
                    reason: "Fictional progress update".into(),
                });
            }
            "perf-cycles" => {
                self.performance_state.tab = Tab::Cycles;
            }
            "perf-cycle-form" => {
                self.performance_state.tab = Tab::Cycles;
                let mut d = CycleDraft::new(&self.data);
                d.name = "Fictional follow-up development review".into();
                d.reason = "Fictional future cycle".into();
                self.performance_state.cycle_form = Some(d);
            }
            "perf-assignment" => {
                self.performance_state.tab = Tab::Appraisals;
                let mut d = AssignmentDraft::new(None, Some(cycle));
                d.reason = "Fictional employee review assignment".into();
                self.performance_state.assignment_form = Some(d);
            }
            "perf-assessment" | "perf-manager-assessment" => {
                self.performance_state.tab = Tab::Appraisals;
                self.performance_state.review_id = Some(review);
                let mut d = AssessmentDraft::existing(&self.data.performance.reviews[0]);
                d.reason = "Fictional draft feedback evidence".into();
                self.performance_state.assessment_form = Some(d);
            }
            "perf-private-note" => {
                self.performance_state.tab = Tab::Appraisals;
                self.performance_state.review_id = Some(review);
                let mut d = NoteDraft::existing(&self.data.performance.notes[0]);
                d.reason = "Fictional private context update".into();
                self.performance_state.note_form = Some(d);
            }
            "perf-publish" => {
                self.performance_state.tab = Tab::Appraisals;
                self.performance_state.review_id = Some(review);
                self.performance_state.decision = Some(Change::ReviewStatus {
                    id: review,
                    revision: self.data.performance.reviews[0].revision,
                    status: "published".into(),
                    reason: "Fictional publication approval".into(),
                });
            }
            "perf-appraisals" => {
                self.performance_state.tab = Tab::Appraisals;
            }
            "perf-review"
            | "perf-manager-review"
            | "perf-submitted"
            | "perf-published"
            | "perf-manager-published"
            | "perf-history" => {
                self.performance_state.tab = Tab::Appraisals;
                self.performance_state.review_id = Some(review);
            }
            _ => {}
        }
        if !self
            .data
            .access
            .allows(crate::staff_roles::Capability::PrivateHrNotes)
        {
            self.data.performance.notes.clear();
            let ids: Vec<_> = performance::visible_events(&self.data)
                .into_iter()
                .map(|e| e.id)
                .collect();
            self.data.performance.events.retain(|e| ids.contains(&e.id));
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unrelated_refresh_preserves_an_unsaved_goal() {
        let data = DemoStore::new().data;
        let d = GoalDraft::new(&data, data.access.employee_id);
        let mut state = State {
            goal_form: Some(d),
            ..Default::default()
        };
        assert!(state.accept(&data));
        assert!(state.goal_form.is_some());
    }
    #[test]
    fn mismatched_goal_receipt_keeps_the_form() {
        let mut data = DemoStore::new().data;
        let mut d = GoalDraft::new(&data, data.access.employee_id);
        d.title = "Synthetic own HR goal".into();
        d.reason = "Synthetic agreement".into();
        let a = Change::GoalSave(d.clone());
        performance::apply_demo(&mut data, &a, chrono::Utc::now()).unwrap();
        data.performance.goals[0].description = "Another session changed this field".into();
        let mut state = State {
            goal_form: Some(d),
            pending: Some(a),
            ..Default::default()
        };
        assert!(!state.accept(&data));
        assert!(state.goal_form.is_some());
        assert!(state.pending.is_none());
    }
}
