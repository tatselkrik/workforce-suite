use super::*;
use crate::calculations::{
    self, ApplyPolicy, CalculationPolicy, CalculationSnapshot, CalculationStatus, PolicyDraft,
};

#[derive(Default)]
pub(super) struct State {
    pub detail: Option<Uuid>,
    pub policy: bool,
    pub draft: Option<PolicyDraft>,
    pub apply: Option<ApplyPolicy>,
    pub saving: bool,
    pub search: String,
}
impl State {
    pub fn editing(&self) -> bool {
        self.draft.is_some() || self.apply.is_some()
    }
}
fn policy_summary(ui: &mut Ui, p: &CalculationPolicy) {
    ui.label(format!("Rule version {}", p.revision));
    ui.label(format!(
        "Late arrival: exclude {} min grace · Early departure: exclude {} min grace",
        p.late_grace_minutes, p.undertime_grace_minutes
    ));
    ui.label(format!(
        "Overtime candidate: minimum {} min, rounded down to {} min · {}",
        p.overtime_min_minutes,
        p.overtime_round_minutes,
        if p.include_pre_shift_overtime {
            "Includes before and after shift"
        } else {
            "After shift only"
        }
    ));
}
fn metrics(ui: &mut Ui, c: &calculations::TimeCalculation) {
    for labels in [
        [
            ("Late arrival", calculations::minutes(c.late_seconds)),
            (
                "Undertime / early departure",
                calculations::minutes(c.undertime_seconds),
            ),
        ],
        [
            (
                "Overtime candidate",
                c.overtime_minutes
                    .map(|m| format!("{m} min"))
                    .unwrap_or_else(|| "—".into()),
            ),
            (
                "Time excluding unpaid breaks",
                c.net_seconds
                    .map(crate::breaks::duration)
                    .unwrap_or_else(|| "—".into()),
            ),
        ],
    ] {
        ui.columns(2, |cols| {
            for (i, (label, value)) in labels.iter().enumerate() {
                cols[i].vertical(|ui| {
                    ui.label(RichText::new(*label).small().color(muted(ui)));
                    ui.label(RichText::new(value).size(19.0).strong());
                });
            }
        });
        ui.add_space(6.0);
    }
}
impl HrisApp {
    pub(super) fn time_calculations(&mut self, ui: &mut Ui) {
        if self.calculation_state.policy {
            self.calculation_policy(ui);
            return;
        }
        if let Some(id) = self.calculation_state.detail {
            self.calculation_detail(ui, id);
            return;
        }
        ui.horizontal_wrapped(|ui| {
            ui.label("Attendance date");
            let r = ui.button(self.attendance_date.to_string());
            egui::Popup::menu(&r)
                .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
                .show(|ui| {
                    calendar_control(
                        ui,
                        &mut self.attendance_date,
                        &mut self.calendar_month,
                        self.data.company.today(),
                        self.data.company.today(),
                        &self.data,
                    );
                });
            if ui.button("Today").clicked() {
                self.attendance_date = self.data.company.today();
                self.calendar_month = month_start(self.attendance_date);
            }
            if ui.button("Company calculation rules").clicked() {
                self.calculation_state.policy = true;
            }
        });
        ui.add(
            egui::TextEdit::singleline(&mut self.calculation_state.search)
                .hint_text("Find employee by name or code")
                .desired_width(ui.available_width().min(430.0)),
        );
        ui.label(RichText::new("Whole-minute calculations use each record's saved shift, calendar decision and rules. Overtime candidates require review; these figures are not payroll amounts.").small().color(muted(ui)));
        let search = self.calculation_state.search.trim().to_lowercase();
        let mut rows: Vec<_> = self
            .data
            .attendance
            .iter()
            .filter(|a| a.work_date == self.attendance_date)
            .filter(|a| {
                self.data
                    .employees
                    .iter()
                    .find(|e| e.id == a.employee_id)
                    .is_some_and(|e| {
                        search.is_empty()
                            || format!("{} {}", e.full_name, e.employee_code)
                                .to_lowercase()
                                .contains(&search)
                    })
            })
            .cloned()
            .collect();
        rows.sort_by_key(|a| self.data.employee_name(a.employee_id).to_lowercase());
        if rows.is_empty() {
            empty(
                ui,
                "No attendance records for this date and filter. A missing record is not a calculated absence.",
            );
        }
        let results: Vec<_> = rows
            .iter()
            .map(|a| calculations::calculate(&self.data, a))
            .collect();
        ui.horizontal_wrapped(|ui| {
            badge(ui, &format!("{} recorded", rows.len()), true);
            badge(
                ui,
                &format!(
                    "{} late",
                    results
                        .iter()
                        .filter(|r| r.late_seconds.is_some_and(|m| m > 0))
                        .count()
                ),
                false,
            );
            badge(
                ui,
                &format!(
                    "{} early departure",
                    results
                        .iter()
                        .filter(|r| r.undertime_seconds.is_some_and(|m| m > 0))
                        .count()
                ),
                false,
            );
            badge(
                ui,
                &format!(
                    "{} overtime candidate",
                    results
                        .iter()
                        .filter(|r| r.overtime_minutes.is_some_and(|m| m > 0))
                        .count()
                ),
                false,
            );
        });
        for (a, c) in rows.iter().zip(results.iter()) {
            ui.add_space(8.0);
            card(ui, |ui| {
                ui.label(
                    RichText::new(self.data.employee_name(a.employee_id))
                        .size(18.0)
                        .strong(),
                );
                ui.label(RichText::new(c.status.label()).color(muted(ui)));
                if let Some(p) = &a.planned_shift {
                    ui.label(format!("{} · {} · {}", p.name, p.hours(), p.timezone));
                }
                metrics(ui, c);
                if ui.button("Details / calculation history").clicked() {
                    self.calculation_state.detail = Some(a.id);
                }
            });
        }
    }
    fn calculation_detail(&mut self, ui: &mut Ui, id: Uuid) {
        if ui
            .add_enabled(
                self.calculation_state.apply.is_none(),
                egui::Button::new("Back to time calculations"),
            )
            .clicked()
        {
            self.calculation_state.detail = None;
            return;
        }
        let Some(a) = self.data.attendance.iter().find(|a| a.id == id).cloned() else {
            empty(
                ui,
                "Attendance is unavailable in your current scope. Refresh the list.",
            );
            return;
        };
        let c = calculations::calculate(&self.data, &a);
        ui.add_space(8.0);
        card(ui, |ui| {
            ui.label(
                RichText::new(self.data.employee_name(a.employee_id))
                    .size(20.0)
                    .strong(),
            );
            ui.label(format!("Work date {}", a.work_date));
            ui.label(attendance_times(&a, &self.data));
            badge(ui, c.status.label(), c.status == CalculationStatus::Ready);
            metrics(ui, &c);
            if let Some(raw) = c.raw_overtime_seconds {
                ui.label(format!(
                    "Outside-shift time after recorded unpaid breaks, before minimum/rounding: {}",
                    calculations::minutes(Some(raw))
                ));
            }
            ui.label(RichText::new("Grace minutes are subtracted before displaying whole minutes. Undertime here means early departure. Break length reminders remain in Breaks; overtime is a candidate for review.").small().color(muted(ui)));
        });
        ui.add_space(8.0);
        card(ui, |ui| {
            ui.label(RichText::new("Saved calculation rules").strong());
            if let Some(s) = &a.calculation_snapshot {
                policy_summary(ui, &s.policy);
                ui.label(format!(
                    "Captured time zone: {} · {}",
                    s.timezone,
                    if s.scheduled_workday {
                        "Scheduled workday"
                    } else {
                        "Not a scheduled workday"
                    }
                ));
                if let Some((start, end)) = s.planned_start.zip(s.planned_end) {
                    let zone = s
                        .timezone
                        .parse::<chrono_tz::Tz>()
                        .unwrap_or(self.data.company.tz());
                    ui.label(format!(
                        "Planned: {} to {}",
                        start.with_timezone(&zone).format("%Y-%m-%d %H:%M"),
                        end.with_timezone(&zone).format("%Y-%m-%d %H:%M")
                    ));
                }
            } else {
                ui.label("Earlier attendance has no saved calculation rules. HR may explicitly apply the current rules with a reason when saved shift hours are available.");
            }
            if matches!(c.status, CalculationStatus::LeaveReview) {
                ui.label("Approved leave covers this work date. Time flags are withheld until HR reviews the leave and attendance together; no partial-day hours are assumed.");
            }
            if matches!(c.status, CalculationStatus::MissingShift) {
                ui.label("No earlier shift hours are reconstructed. Dated assignments do not change a recorded attendance plan.");
            }
            if matches!(c.status, CalculationStatus::BoundaryReview) {
                ui.label("The saved schedule boundary needs HR review. No time penalty or overtime is inferred from an unresolved boundary.");
            }
            if self
                .data
                .access
                .allows(crate::staff_roles::Capability::AttendanceAdministration)
            {
                let changed = a.calculation_snapshot.as_ref()
                    != Some(&CalculationSnapshot::capture(&self.data, &a));
                if ui
                    .add_enabled(
                        !self.busy
                            && a.planned_shift.is_some()
                            && changed
                            && self.calculation_state.apply.is_none(),
                        egui::Button::new("Apply current rules with reason"),
                    )
                    .clicked()
                {
                    self.calculation_state.apply = Some(ApplyPolicy {
                        attendance_id: id,
                        expected_revision: a.revision,
                        expected_policy_revision: self.data.calculations.policy.revision,
                        reason: String::new(),
                    });
                }
            }
        });
        if let Some(mut change) = self.calculation_state.apply.take() {
            let mut cancel = false;
            ui.add_space(8.0);
            card(ui, |ui| {
                ui.heading("Apply current calculation rules");
                policy_summary(ui, &self.data.calculations.policy);
                ui.label("This updates only this record's calculation rules and calendar decision. Attendance times, saved shift and breaks stay intact. The previous rules and reason remain in history.");
                field(
                    ui,
                    "Reason *",
                    &mut change.reason,
                    "Explain why the current rules should apply to this record",
                );
                ui.add_enabled_ui(!self.busy, |ui| {
                    ui.horizontal_wrapped(|ui| {
                        if primary(ui, "Apply rules") {
                            match change.payload(&self.data) {
                                Ok(_) => {
                                    self.calculation_state.saving = true;
                                    self.send(Command::Change(Action::ApplyCalculationPolicy(
                                        change.clone(),
                                    )));
                                }
                                Err(e) => self.notice = Some((e.to_string(), true)),
                            }
                        }
                        cancel = ui.button("Cancel").clicked();
                    });
                });
            });
            if !cancel {
                self.calculation_state.apply = Some(change);
            }
        }
        if self
            .data
            .access
            .allows(crate::staff_roles::Capability::AttendanceAdministration)
        {
            self.calculation_history(ui, Some(id));
        }
    }
    fn calculation_policy(&mut self, ui: &mut Ui) {
        if ui
            .add_enabled(
                self.calculation_state.draft.is_none(),
                egui::Button::new("Back to time calculations"),
            )
            .clicked()
        {
            self.calculation_state.policy = false;
            return;
        }
        let Some(mut draft) = self.calculation_state.draft.take() else {
            card(ui, |ui| {
                ui.heading("Company calculation rules");
                policy_summary(ui, &self.data.calculations.policy);
                ui.label("New attendance keeps the rules in effect when recorded. Saving company rules does not rewrite existing attendance. HR can apply revised rules to a saved record with a reason and history.");
                if self.data.access.role.manages_accounts()
                    && ui
                        .add_enabled(!self.busy, egui::Button::new("Edit calculation rules"))
                        .clicked()
                {
                    self.calculation_state.draft =
                        Some(PolicyDraft::new(&self.data.calculations.policy));
                }
            });
            if self
                .data
                .access
                .allows(crate::staff_roles::Capability::AttendanceAdministration)
            {
                self.calculation_history(ui, None);
            }
            return;
        };
        let mut cancel = false;
        ui.add_space(8.0);
        card(ui, |ui| {
            ui.heading("Edit calculation rules");
            ui.label("Grace allowances are excluded from late arrival and early departure. Remaining time is displayed in whole minutes. Overtime candidates exclude recorded unpaid breaks outside the shift.");
            ui.add_enabled_ui(!self.busy,|ui|{
                ui.columns(2,|cols|{minute_field(&mut cols[0],"Late arrival grace (min)",&mut draft.policy.late_grace_minutes,120);minute_field(&mut cols[1],"Early departure grace (min)",&mut draft.policy.undertime_grace_minutes,120);});
                ui.columns(2,|cols|{minute_field(&mut cols[0],"Minimum overtime (min)",&mut draft.policy.overtime_min_minutes,240);cols[1].vertical(|ui|{ui.label(RichText::new("Overtime rounding down (min)").size(13.0));egui::ComboBox::from_id_salt("overtime_rounding").selected_text(draft.policy.overtime_round_minutes.to_string()).show_ui(ui,|ui|{for n in [1,5,10,15,30,60]{ui.selectable_value(&mut draft.policy.overtime_round_minutes,n,n.to_string());}});});});
                ui.checkbox(&mut draft.policy.include_pre_shift_overtime,"Include recorded work before the shift in overtime candidates");
                ui.label(RichText::new("The minimum is checked before rounding down. Off-days and approved leave are flagged for HR review. These rules do not approve overtime or calculate salary.").small().color(muted(ui)));
                field(ui,"Reason *",&mut draft.reason,"Explain this policy change");ui.separator();
                ui.horizontal_wrapped(|ui|{if primary(ui,"Save calculation rules"){match draft.payload(&self.data){Ok(_)=>{self.calculation_state.saving=true;self.send(Command::Change(Action::SaveCalculationPolicy(draft.clone())));},Err(e)=>self.notice=Some((e.to_string(),true))}}cancel=ui.button("Cancel").clicked();});
            });
        });
        if !cancel {
            self.calculation_state.draft = Some(draft);
        }
    }
    #[cfg(feature = "qa-screenshot")]
    pub(super) fn prepare_calculation_preview(&mut self) {
        use crate::{
            breaks::{BreakAction, BreakDraft},
            domain::DemoStore,
            shifts::{ShiftAssignmentDraft, ShiftTemplateDraft},
        };
        use chrono::Utc;
        let preview = std::env::var("HRIS_PREVIEW_FORM").unwrap_or_default();
        if !preview.starts_with("calc-") || self.data.mode != Mode::Demo {
            return;
        }
        let role = self.data.access.role;
        let mut store = DemoStore {
            data: self.data.clone(),
        };
        store.data.access.role = Role::Administrator;
        store.data.attendance.clear();
        store.data.corrections.clear();
        store.data.breaks = Default::default();
        store.data.leave.clear();
        for e in &mut store.data.employees {
            e.joined_on = Some("2020-01-01".parse().unwrap());
        }
        let person = store.data.employees[0].id;
        let date = store.data.company.today() - Duration::days(5);
        let mut p = PolicyDraft::new(&store.data.calculations.policy);
        p.policy.late_grace_minutes = 5;
        p.policy.undertime_grace_minutes = 10;
        p.policy.overtime_min_minutes = 30;
        p.policy.overtime_round_minutes = 15;
        p.reason = "Fictional company rule preview".into();
        store.execute(Action::SaveCalculationPolicy(p)).unwrap();
        store
            .execute(Action::SaveShiftTemplate(ShiftTemplateDraft {
                name: "Office day shift".into(),
                days: vec![1, 2, 3, 4, 5, 6, 7],
                works_on_holidays: true,
                ..Default::default()
            }))
            .unwrap();
        let mut assign = ShiftAssignmentDraft::new(&store.data, person, date);
        assign.template_id = Some(store.data.shifts.templates[0].id);
        assign.expected_template_revision = Some(0);
        assign.reason = "Fictional calculation schedule".into();
        store.execute(Action::AssignShift(assign)).unwrap();
        let mut c = CorrectionDraft::new(person, date, None, &store.data);
        c.checked_in = "08:17".into();
        c.checked_out = "18:47".into();
        c.reason = "Fictional calculation attendance".into();
        let change = c.change(&store.data, Utc::now()).unwrap();
        store.execute(Action::CorrectAttendance(change)).unwrap();
        let parent = store.data.attendance[0].clone();
        for (name, start, end) in [
            ("Meal break", "12:00", "13:00"),
            ("Outside-shift break", "17:15", "17:30"),
        ] {
            let mut b = BreakDraft::new(&store.data, &parent, None);
            b.name = name.into();
            b.started_time = start.into();
            b.ended_time = end.into();
            b.reason = "Fictional actual break preview".into();
            store
                .execute(Action::AttendanceBreak(BreakAction::Save(b)))
                .unwrap();
        }
        if matches!(preview.as_str(), "calc-apply" | "calc-record-history") {
            let mut p = PolicyDraft::new(&store.data.calculations.policy);
            p.policy.late_grace_minutes = 10;
            p.reason = "Fictional revised grace after original attendance".into();
            store.execute(Action::SaveCalculationPolicy(p)).unwrap();
            if preview == "calc-record-history" {
                store
                    .execute(Action::ApplyCalculationPolicy(ApplyPolicy {
                        attendance_id: parent.id,
                        expected_revision: parent.revision,
                        expected_policy_revision: store.data.calculations.policy.revision,
                        reason: "Fictional HR reviewed the revised rule for this record".into(),
                    }))
                    .unwrap();
            }
        }
        if preview == "calc-legacy" {
            store.data.attendance[0].calculation_snapshot = None;
        }
        if preview == "calc-no-shift" {
            store.data.attendance[0].planned_shift = None;
            store.data.attendance[0].calculation_snapshot = Some(CalculationSnapshot::capture(
                &store.data,
                &store.data.attendance[0],
            ));
        }
        if preview == "calc-off-day" {
            store.data.attendance[0]
                .calculation_snapshot
                .as_mut()
                .unwrap()
                .scheduled_workday = false;
        }
        if preview == "calc-open" {
            store.data.attendance[0].checked_out_at = None;
        }
        if preview == "calc-leave" {
            store.data.leave_setup.policy.workweek = vec![1, 2, 3, 4, 5, 6, 7];
            let draft = LeaveDraft {
                employee_id: Some(person),
                starts_on: date.to_string(),
                ends_on: date.to_string(),
                reason: "Fictional leave requiring time review".into(),
                ..Default::default()
            };
            store.execute(Action::RequestLeave(draft)).unwrap();
            store.data.leave[0].status = LeaveStatus::Approved;
        }
        self.data = store.data;
        self.data.access.role = role;
        self.page = Page::Attendance;
        self.shift_state.tab = shifts_view::Tab::Calculations;
        self.attendance_date = date;
        self.calendar_month = month_start(date);
        if preview.starts_with("calc-policy") {
            self.calculation_state.policy = true;
            if preview == "calc-policy-form" {
                self.calculation_state.draft =
                    Some(PolicyDraft::new(&self.data.calculations.policy));
            }
        } else if preview == "calc-empty" {
            self.data.attendance.clear();
        } else if preview == "calc-settings" {
            self.page = Page::Settings;
        } else if preview != "calc-list" {
            self.calculation_state.detail = Some(parent.id);
        }
        if preview == "calc-apply" {
            self.calculation_state.apply = Some(ApplyPolicy {
                attendance_id: parent.id,
                expected_revision: parent.revision,
                expected_policy_revision: self.data.calculations.policy.revision,
                reason: "Fictional reviewed rule application; recorded times stay intact".into(),
            });
        }
    }
    fn calculation_history(&self, ui: &mut Ui, attendance: Option<Uuid>) {
        let header = egui::CollapsingHeader::new(if attendance.is_some() {
            "Calculation rule history"
        } else {
            "Company rule history"
        });
        #[cfg(feature = "qa-screenshot")]
        let header = header
            .default_open(std::env::var("HRIS_PREVIEW_FORM").is_ok_and(|s| s.ends_with("history")));
        header.show(ui, |ui| {
            let mut events: Vec<_> = if let Some(id) = attendance {
                self.data
                    .calculations
                    .attendance_events
                    .iter()
                    .filter(|e| e.attendance_id == Some(id))
                    .collect()
            } else {
                self.data.calculations.policy_events.iter().collect()
            };
            events.sort_by_key(|e| std::cmp::Reverse((e.created_at, e.id)));
            if events.is_empty() {
                empty(ui, "No rule changes recorded here.");
            }
            for e in events {
                card(ui, |ui| {
                    ui.label(
                        RichText::new(format!(
                            "{} · {}",
                            e.actor_name,
                            e.created_at
                                .with_timezone(&self.data.company.tz())
                                .format("%Y-%m-%d %H:%M:%S")
                        ))
                        .strong(),
                    );
                    ui.label(&e.reason);
                    for (label, value) in [
                        ("Before", e.before_snapshot.as_ref()),
                        ("After", Some(&e.after_snapshot)),
                    ] {
                        ui.label(RichText::new(label).strong());
                        if let Some(p) = value.and_then(|v| {
                            serde_json::from_value::<CalculationPolicy>(
                                v.get("policy").unwrap_or(v).clone(),
                            )
                            .ok()
                        }) {
                            policy_summary(ui, &p);
                            if let Some(s) = value.and_then(|v| {
                                serde_json::from_value::<CalculationSnapshot>(v.clone()).ok()
                            }) {
                                ui.label(format!(
                                    "{} · {}",
                                    s.timezone,
                                    if s.scheduled_workday {
                                        "Scheduled workday"
                                    } else {
                                        "Not a scheduled workday"
                                    }
                                ));
                                if let Some((start, end)) = s.planned_start.zip(s.planned_end) {
                                    let zone = s
                                        .timezone
                                        .parse::<chrono_tz::Tz>()
                                        .unwrap_or(self.data.company.tz());
                                    ui.label(format!(
                                        "Planned: {} to {}",
                                        start.with_timezone(&zone).format("%Y-%m-%d %H:%M"),
                                        end.with_timezone(&zone).format("%Y-%m-%d %H:%M")
                                    ));
                                }
                            }
                        } else {
                            ui.label("No saved calculation rules");
                        }
                    }
                });
                ui.add_space(6.0);
            }
        });
    }
}
fn minute_field(ui: &mut Ui, label: &str, value: &mut u32, max: u32) {
    ui.vertical(|ui| {
        ui.label(RichText::new(label).size(13.0));
        ui.add_sized([110.0, 34.0], egui::DragValue::new(value).range(0..=max));
    });
}
