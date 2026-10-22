use super::*;
use crate::breaks::{self, BreakAction, BreakDraft, BreakRule};

#[derive(Default)]
pub(super) struct State {
    pub detail: Option<Uuid>,
    pub form: Option<BreakDraft>,
    pub void_target: Option<(Uuid, i64, i64)>,
    pub reason: String,
    pub show_voided: bool,
    pub saving: bool,
}
pub(super) fn rules_editor(ui: &mut Ui, rules: &mut Vec<BreakRule>) {
    ui.add_space(10.0);
    ui.label(RichText::new("Break rules").strong());
    ui.label(RichText::new("Set paid/unpaid breaks and expected durations. Required breaks are flagged when missing or short; only recorded unpaid time is excluded from attendance time.").small().color(muted(ui)));
    let mut remove = None;
    for (i, r) in rules.iter_mut().enumerate() {
        ui.push_id(r.id, |ui| {
            Frame::new()
                .stroke(Stroke::new(
                    1.0,
                    ui.visuals().widgets.noninteractive.bg_stroke.color,
                ))
                .corner_radius(8)
                .inner_margin(10)
                .show(ui, |ui| {
                    ui.columns(2, |cols| {
                        field(
                            &mut cols[0],
                            "Break name *",
                            &mut r.name,
                            "Meal break or Rest break",
                        );
                        cols[1].vertical(|ui| {
                            ui.label(RichText::new("Duration (minutes) *").size(13.0));
                            ui.add_sized(
                                [100.0, 34.0],
                                egui::DragValue::new(&mut r.minutes).range(1..=720),
                            );
                        });
                    });
                    ui.horizontal_wrapped(|ui| {
                        ui.checkbox(&mut r.paid, "Paid");
                        ui.checkbox(&mut r.required, "Required");
                        if ui.button("Remove rule").clicked() {
                            remove = Some(i);
                        }
                    });
                });
            ui.add_space(6.0);
        });
    }
    if rules.is_empty() {
        ui.label(RichText::new("No break rules for this shift.").color(muted(ui)));
    }
    if let Some(i) = remove {
        rules.remove(i);
    }
    if ui
        .add_enabled(rules.len() < 8, egui::Button::new("+ Add break rule"))
        .clicked()
    {
        rules.push(BreakRule::default());
    }
    ui.label(RichText::new("Reassign the shift to apply revised break rules to employees. Existing attendance keeps its saved rules.").small().color(muted(ui)));
}
pub(super) fn rules_summary(ui: &mut Ui, rules: &[BreakRule]) {
    for r in rules {
        ui.label(RichText::new(r.label()).small().color(muted(ui)));
    }
}
impl HrisApp {
    pub(super) fn attendance_breaks(&mut self, ui: &mut Ui, id: Uuid) {
        if ui
            .add_enabled(
                self.break_state.form.is_none() && self.break_state.void_target.is_none(),
                egui::Button::new("Back to daily attendance"),
            )
            .clicked()
        {
            self.break_state = State::default();
            return;
        }
        let Some(parent) = self.data.attendance.iter().find(|a| a.id == id).cloned() else {
            empty(
                ui,
                "Attendance is no longer in your current access scope. Refresh the list.",
            );
            return;
        };
        if self.break_state.form.is_some() {
            self.break_editor(ui, &parent);
            return;
        }
        let totals = self.data.breaks.totals(id);
        card(ui, |ui| {
            ui.label(
                RichText::new(self.data.employee_name(parent.employee_id))
                    .size(20.0)
                    .strong(),
            );
            ui.label(format!(
                "{} · {}",
                parent.work_date, self.data.company.timezone
            ));
            ui.label(attendance_times(&parent, &self.data));
            ui.horizontal_wrapped(|ui| {
                ui.label(format!(
                    "Paid breaks: {}",
                    breaks::duration(totals.paid_seconds)
                ));
                ui.label(format!(
                    "Unpaid breaks: {}",
                    breaks::duration(totals.unpaid_seconds)
                ));
            });
            if let Some(net) = totals.time_excluding_unpaid(&parent) {
                ui.label(
                    RichText::new(format!(
                        "Recorded time excluding unpaid breaks: {}",
                        breaks::duration(net)
                    ))
                    .strong(),
                );
            }
            if totals.open {
                ui.label(
                    RichText::new("A break is in progress. End it before checking out.")
                        .color(accent(ui)),
                );
            }
            ui.label(RichText::new("Totals use completed, recorded breaks. Required durations do not create automatic deductions.").small().color(muted(ui)));
        });
        ui.add_space(8.0);
        let rules = parent
            .planned_shift
            .as_ref()
            .map(|p| p.break_rules.clone())
            .unwrap_or_default();
        card(ui, |ui| {
            ui.label(RichText::new("Saved break rules").strong());
            if rules.is_empty() {
                ui.label("No break rules were saved with this attendance. HR can record an unscheduled break with a reason.");
            }
            for rule in &rules {
                ui.horizontal_wrapped(|ui| {
                    ui.label(rule.label());
                    ui.label(
                        RichText::new(breaks::rule_status(&self.data, id, rule))
                            .small()
                            .color(muted(ui)),
                    );
                    if self.data.access.employee_id == Some(parent.employee_id)
                        && parent.checked_out_at.is_none()
                    {
                        let ready = !self.busy
                            && self.data.breaks.open(id).is_none()
                            && !self
                                .data
                                .breaks
                                .for_attendance(id)
                                .any(|b| b.rule_id == Some(rule.id));
                        if ui
                            .add_enabled(ready, egui::Button::new("Start break"))
                            .clicked()
                        {
                            self.send(Command::Change(Action::AttendanceBreak(
                                BreakAction::Start {
                                    attendance_id: id,
                                    parent_revision: parent.revision,
                                    rule_id: rule.id,
                                },
                            )));
                        }
                    }
                });
            }
            if let Some(open) = self.data.breaks.open(id).cloned() {
                if self.data.access.employee_id == Some(parent.employee_id)
                    && parent.checked_out_at.is_none()
                {
                    if ui
                        .add_enabled(!self.busy, egui::Button::new("End break now"))
                        .clicked()
                    {
                        self.send(Command::Change(Action::AttendanceBreak(BreakAction::End {
                            id: open.id,
                            revision: open.revision,
                            parent_revision: parent.revision,
                        })));
                    }
                }
            }
            if self.data.access.role.manages_hr()
                && ui
                    .add_enabled(
                        !self.busy && self.break_state.void_target.is_none(),
                        egui::Button::new("Record break with reason"),
                    )
                    .clicked()
            {
                self.break_state.form = Some(BreakDraft::new(&self.data, &parent, None));
            }
        });
        ui.add_space(8.0);
        if self.data.access.role.manages_hr() {
            ui.checkbox(&mut self.break_state.show_voided, "Show voided breaks");
        }
        let mut records: Vec<_> = self
            .data
            .breaks
            .records
            .iter()
            .filter(|b| b.attendance_id == id && (!b.voided || self.break_state.show_voided))
            .cloned()
            .collect();
        records.sort_by_key(|b| (b.started_at, b.id));
        if records.is_empty() {
            empty(ui, "No recorded breaks for this attendance.");
        }
        for b in records {
            card(ui, |ui| {
                ui.label(
                    RichText::new(format!(
                        "{} · {}",
                        b.name,
                        if b.paid { "Paid" } else { "Unpaid" }
                    ))
                    .strong(),
                );
                ui.label(format!(
                    "{} to {}",
                    b.started_at
                        .with_timezone(&self.data.company.tz())
                        .format("%b %d, %H:%M:%S"),
                    b.ended_at
                        .map(|t| t
                            .with_timezone(&self.data.company.tz())
                            .format("%b %d, %H:%M:%S")
                            .to_string())
                        .unwrap_or_else(|| "In progress".into())
                ));
                if b.voided {
                    badge(ui, "Voided · excluded from totals", false);
                } else if b.ended_at.is_some() {
                    ui.label(format!(
                        "Recorded duration: {}",
                        breaks::duration(b.seconds())
                    ));
                }
                if self.data.access.role.manages_hr() && !b.voided {
                    ui.add_enabled_ui(!self.busy && self.break_state.void_target.is_none(), |ui| {
                        ui.horizontal_wrapped(|ui| {
                            if ui.button("Correct times").clicked() {
                                self.break_state.form =
                                    Some(BreakDraft::new(&self.data, &parent, Some(b.clone())));
                            }
                            if ui.button("Void with reason").clicked() {
                                self.break_state.void_target =
                                    Some((b.id, b.revision, parent.revision));
                                self.break_state.reason.clear();
                            }
                        });
                    });
                }
            });
            ui.add_space(6.0);
        }
        if let Some((bid, revision, parent_revision)) = self.break_state.void_target {
            card(ui, |ui| {
                ui.label(RichText::new("Void recorded break").strong());
                field(
                    ui,
                    "Reason *",
                    &mut self.break_state.reason,
                    "Explain the correction (up to 500 characters)",
                );
                ui.add_enabled_ui(!self.busy, |ui| {
                    ui.horizontal_wrapped(|ui| {
                        if primary(ui, "Void break") {
                            self.break_state.saving = true;
                            self.send(Command::Change(Action::AttendanceBreak(
                                BreakAction::Void {
                                    id: bid,
                                    revision,
                                    parent_revision,
                                    reason: self.break_state.reason.clone(),
                                },
                            )));
                        }
                        if ui.button("Cancel").clicked() {
                            self.break_state.void_target = None;
                            self.break_state.reason.clear();
                        }
                    });
                });
                ui.label(RichText::new("Voiding removes this break from totals and retains its original record and history.").small().color(muted(ui)));
            });
        }
        if self.data.access.role.manages_hr() {
            let history = egui::CollapsingHeader::new("Break history");
            #[cfg(feature = "qa-screenshot")]
            let history = history
                .default_open(std::env::var("HRIS_PREVIEW_FORM").as_deref() == Ok("break-history"));
            history.show(ui, |ui| {
                let mut events: Vec<_> = self
                    .data
                    .breaks
                    .events
                    .iter()
                    .filter(|e| e.attendance_id == id)
                    .collect();
                events.sort_by_key(|e| std::cmp::Reverse((e.created_at, e.id)));
                for e in events {
                    card(ui, |ui| {
                        ui.label(
                            RichText::new(format!(
                                "{} · {} · {}",
                                e.action,
                                e.actor_name,
                                e.created_at
                                    .with_timezone(&self.data.company.tz())
                                    .format("%Y-%m-%d %H:%M:%S")
                            ))
                            .strong(),
                        );
                        ui.label(&e.reason);
                        if let Some(b) = &e.before_record {
                            ui.label(format!(
                                "Before: {}",
                                break_change_summary(b, self.data.company.tz())
                            ));
                        }
                        let b = &e.after_record;
                        ui.label(format!(
                            "After: {}",
                            break_change_summary(b, self.data.company.tz())
                        ));
                    });
                }
            });
        }
    }
    fn break_editor(&mut self, ui: &mut Ui, parent: &Attendance) {
        let Some(mut draft) = self.break_state.form.take() else {
            return;
        };
        let mut save = false;
        let mut cancel = false;
        card(ui, |ui| {
            ui.heading(if draft.original.is_some() {
                "Correct break times"
            } else {
                "Record break with reason"
            });
            ui.label(RichText::new(self.data.employee_name(parent.employee_id)).strong());
            ui.label(format!(
                "Attendance {} · {}",
                parent.work_date, self.data.company.timezone
            ));
            ui.add_enabled_ui(!self.busy,|ui| {
                if draft.original.is_none(){
                    egui::ComboBox::from_id_salt("manual_break_rule").width(ui.available_width().min(430.0)).selected_text(draft.rule_id.and_then(|id|parent.planned_shift.as_ref().and_then(|p|p.break_rules.iter().find(|r|r.id==id))).map(|r|r.label()).unwrap_or_else(||"Unscheduled break — HR classification".into())).show_ui(ui,|ui|{
                        ui.selectable_value(&mut draft.rule_id,None,"Unscheduled break — HR classification");if let Some(p)=&parent.planned_shift{for r in &p.break_rules{ui.selectable_value(&mut draft.rule_id,Some(r.id),r.label());}}
                    });
                    if draft.rule_id.is_none(){field(ui,"Break name *",&mut draft.name,"For example, Meal break");ui.checkbox(&mut draft.paid,"Paid break");}
                }else{ui.label(format!("{} · {}",draft.name,if draft.paid{"Paid"}else{"Unpaid"}));}
                ui.columns(2,|cols|{field(&mut cols[0],"Start date *",&mut draft.started_date,"YYYY-MM-DD");field(&mut cols[1],"Start time *",&mut draft.started_time,"24-hour time, e.g. 12:00");});
                ui.columns(2,|cols|{field(&mut cols[0],"End date",&mut draft.ended_date,"YYYY-MM-DD");field(&mut cols[1],"End time",&mut draft.ended_time,"Leave empty if in progress");});
                field(ui,"Reason *",&mut draft.reason,"Explain why this record is added or corrected");
                ui.label(RichText::new("Breaks must stay inside the recorded attendance period and cannot overlap. Voided records and corrections remain in history.").small().color(muted(ui)));
                ui.separator();ui.horizontal_wrapped(|ui|{save=primary(ui,"Save break");cancel=ui.button("Cancel").clicked();});
            });
        });
        if save {
            let action = BreakAction::Save(draft.clone());
            match action.prepare(&self.data, chrono::Utc::now()) {
                Ok(_) => {
                    self.break_state.saving = true;
                    self.send(Command::Change(Action::AttendanceBreak(action)));
                }
                Err(e) => self.notice = Some((e.to_string(), true)),
            }
        }
        if !cancel {
            self.break_state.form = Some(draft);
        }
    }
    #[cfg(feature = "qa-screenshot")]
    pub(super) fn prepare_break_preview(&mut self) {
        use crate::{
            corrections::CorrectionDraft,
            domain::DemoStore,
            shifts::{ShiftAssignmentDraft, ShiftTemplateDraft},
        };
        use chrono::{Duration, Utc};
        let preview = std::env::var("HRIS_PREVIEW_FORM").unwrap_or_default();
        if !preview.starts_with("break-") || self.data.mode != Mode::Demo {
            return;
        }
        let role = self.data.access.role;
        let mut store = DemoStore {
            data: self.data.clone(),
        };
        store.data.access.role = Role::Administrator;
        store.data.attendance.clear();
        store.data.corrections.clear();
        let person = store.data.employees[0].id;
        store.data.access.employee_id = Some(person);
        store.data.employees[0].joined_on = Some("2020-01-01".parse().unwrap());
        let rules = vec![
            BreakRule {
                name: "Meal break".into(),
                minutes: 60,
                paid: false,
                required: true,
                ..Default::default()
            },
            BreakRule {
                name: "Afternoon rest".into(),
                minutes: 15,
                paid: true,
                required: false,
                ..Default::default()
            },
        ];
        store
            .execute(Action::SaveShiftTemplate(ShiftTemplateDraft {
                name: "Office day shift".into(),
                days: vec![1, 2, 3, 4, 5, 6, 7],
                works_on_holidays: true,
                break_rules: rules.clone(),
                ..Default::default()
            }))
            .expect("fictional break shift");
        let template = store.data.shifts.templates[0].clone();
        let date = store.data.company.today() - Duration::days(5);
        let mut assignment = ShiftAssignmentDraft::new(&store.data, person, date);
        assignment.template_id = Some(template.id);
        assignment.expected_template_revision = Some(0);
        assignment.reason = "Fictional break policy preview".into();
        store
            .execute(Action::AssignShift(assignment))
            .expect("fictional break assignment");
        let mut correction = CorrectionDraft::new(person, date, None, &store.data);
        correction.checked_in = "08:00".into();
        correction.checked_out = "17:00".into();
        correction.reason = "Fictional attendance preview".into();
        let change = correction.change(&store.data, Utc::now()).unwrap();
        store.execute(Action::CorrectAttendance(change)).unwrap();
        let mut parent = store.data.attendance[0].clone();
        if preview != "break-empty" && preview != "break-no-rules" && preview != "break-own-open" {
            let mut draft = BreakDraft::new(&store.data, &parent, None);
            draft.rule_id = Some(rules[0].id);
            draft.started_time = "12:00".into();
            draft.ended_time = "13:00".into();
            draft.reason = "Fictional meal recorded after a missed break entry".into();
            store
                .execute(Action::AttendanceBreak(BreakAction::Save(draft)))
                .unwrap();
            if preview == "break-history" {
                let mut draft = BreakDraft::new(
                    &store.data,
                    &parent,
                    Some(store.data.breaks.records[0].clone()),
                );
                draft.ended_time = "12:45".into();
                draft.reason =
                    "Fictional corrected end time checked against the attendance record".into();
                store
                    .execute(Action::AttendanceBreak(BreakAction::Save(draft)))
                    .unwrap();
            }
        }
        if preview == "break-own-open" {
            parent.work_date = store.data.company.today();
            parent.checked_in_at = Utc::now() - Duration::hours(2);
            parent.checked_out_at = None;
            store.data.attendance[0] = parent.clone();
            store
                .data
                .breaks
                .records
                .push(crate::breaks::AttendanceBreak {
                    id: Uuid::new_v4(),
                    company_id: parent.company_id,
                    attendance_id: parent.id,
                    employee_id: person,
                    rule_id: Some(rules[0].id),
                    name: rules[0].name.clone(),
                    paid: false,
                    required: true,
                    expected_minutes: 60,
                    started_at: Utc::now() - Duration::minutes(22),
                    ended_at: None,
                    voided: false,
                    revision: 0,
                });
        }
        if preview == "break-no-rules" {
            store.data.attendance[0].planned_shift = None;
        }
        self.data = store.data;
        self.data.access.role = role;
        self.page = Page::Attendance;
        self.attendance_date = parent.work_date;
        self.calendar_month = month_start(parent.work_date);
        self.break_state.detail = Some(parent.id);
        if preview == "break-rules" {
            self.break_state.detail = None;
            self.shift_state.tab = shifts_view::Tab::Shifts;
            self.shift_state.template = Some(ShiftTemplateDraft::from(&template));
        }
        if preview == "break-daily" {
            self.break_state.detail = None;
        }
        if matches!(preview.as_str(), "break-record" | "break-correct") {
            self.break_state.form = Some(BreakDraft::new(
                &self.data,
                &parent,
                if preview == "break-correct" {
                    Some(self.data.breaks.records[0].clone())
                } else {
                    None
                },
            ));
        }
        if preview == "break-void" {
            let b = &self.data.breaks.records[0];
            self.break_state.void_target = Some((b.id, b.revision, parent.revision));
            self.break_state.reason =
                "Fictional duplicate entry; original retained for history".into();
        }
    }
}

fn break_change_summary(b: &crate::breaks::AttendanceBreak, zone: chrono_tz::Tz) -> String {
    format!(
        "{} · {} to {} · {} · {}",
        b.name,
        b.started_at
            .with_timezone(&zone)
            .format("%Y-%m-%d %H:%M:%S"),
        b.ended_at
            .map(|t| t
                .with_timezone(&zone)
                .format("%Y-%m-%d %H:%M:%S")
                .to_string())
            .unwrap_or_else(|| "In progress".into()),
        breaks::duration(b.seconds()),
        if b.voided {
            "Voided"
        } else if b.ended_at.is_none() {
            "In progress"
        } else {
            "Recorded"
        }
    )
}
