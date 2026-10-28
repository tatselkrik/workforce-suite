use super::*;
use crate::overtime::{self, Draft, OvertimeAction, Request, Status};
#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub(super) enum Queue {
    #[default]
    Mine,
    Review,
    History,
}
#[derive(Default)]
pub(super) struct State {
    pub form: Option<Draft>,
    pub detail: Option<Uuid>,
    pub decision: Option<Status>,
    pub minutes: u32,
    pub reason: String,
    pub queue: Queue,
    pub search: String,
    pub saving: bool,
}
impl State {
    pub fn editing(&self) -> bool {
        self.form.is_some() || self.decision.is_some()
    }
}
fn recorded(r: &overtime::Basis) -> String {
    let a = &r.attendance;
    let tz = a
        .calculation_snapshot
        .as_ref()
        .and_then(|s| s.timezone.parse::<chrono_tz::Tz>().ok())
        .unwrap_or(chrono_tz::UTC);
    format!(
        "{} to {} · {}",
        a.checked_in_at
            .with_timezone(&tz)
            .format("%b %d, %Y %H:%M:%S"),
        a.checked_out_at
            .map(|t| t
                .with_timezone(&tz)
                .format("%b %d, %Y %H:%M:%S")
                .to_string())
            .unwrap_or_else(|| "Open".into()),
        tz
    )
}
fn basis_summary(ui: &mut Ui, b: &overtime::Basis) {
    ui.label(recorded(b));
    ui.label(format!("Calculated overtime candidate: {} min", b.minutes));
    if let Some(s) = &b.attendance.calculation_snapshot {
        ui.label(
            RichText::new(format!(
                "Saved rule version {} · {} recorded break(s)",
                s.policy.revision,
                b.breaks.len()
            ))
            .small()
            .color(muted(ui)),
        );
    }
}
impl HrisApp {
    #[cfg(feature = "qa-screenshot")]
    pub(super) fn prepare_overtime_preview(&mut self) {
        use crate::{
            breaks::{BreakAction, BreakDraft},
            calculations::PolicyDraft,
            corrections::CorrectionDraft,
            shifts::{ShiftAssignmentDraft, ShiftTemplateDraft},
        };
        use chrono::{Duration, Utc};
        let preview = std::env::var("HRIS_PREVIEW_FORM").unwrap_or_default();
        if !preview.starts_with("ot-") || self.data.mode != Mode::Demo {
            return;
        }
        let initial = self.data.access.clone();
        let employee = initial.employee_id.unwrap();
        let mut store = DemoStore {
            data: self.data.clone(),
        };
        store.data.access.role = Role::Administrator;
        store.data.attendance.clear();
        store.data.corrections.clear();
        store.data.leave.clear();
        store.data.breaks = Default::default();
        store.data.shifts = Default::default();
        store.data.overtime = Default::default();
        store
            .data
            .employees
            .iter_mut()
            .find(|e| e.id == employee)
            .unwrap()
            .joined_on = None;
        let mut policy = PolicyDraft::new(&store.data.calculations.policy);
        policy.policy.overtime_min_minutes = 30;
        policy.policy.overtime_round_minutes = 15;
        policy.reason = "Fictional overtime rules".into();
        store
            .execute(Action::SaveCalculationPolicy(policy))
            .unwrap();
        store
            .execute(Action::SaveShiftTemplate(ShiftTemplateDraft {
                name: "Fictional overtime shift".into(),
                days: vec![1, 2, 3, 4, 5, 6, 7],
                works_on_holidays: true,
                ..Default::default()
            }))
            .unwrap();
        let day = store.data.company.today() - Duration::days(9);
        let mut a = ShiftAssignmentDraft::new(&store.data, employee, day);
        a.template_id = Some(store.data.shifts.templates[0].id);
        a.expected_template_revision = Some(0);
        a.reason = "Fictional overtime assignment".into();
        store.execute(Action::AssignShift(a)).unwrap();
        let mut parents = Vec::new();
        for offset in 0..6 {
            let date = day + Duration::days(offset);
            let mut d = CorrectionDraft::new(employee, date, None, &store.data);
            d.checked_in = "08:17".into();
            d.checked_out = "18:47".into();
            d.reason = "Fictional attendance for overtime review".into();
            let change = d.change(&store.data, Utc::now()).unwrap();
            let parent = change.record.clone();
            store.execute(Action::CorrectAttendance(change)).unwrap();
            for (name, start, end) in [
                ("Fictional meal", "12:00", "13:00"),
                ("Fictional outside break", "17:15", "17:30"),
            ] {
                let mut b = BreakDraft::new(&store.data, &parent, None);
                b.name = name.into();
                b.started_time = start.into();
                b.ended_time = end.into();
                b.reason = "Fictional actual break".into();
                store
                    .execute(Action::AttendanceBreak(BreakAction::Save(b)))
                    .unwrap();
            }
            if offset < 5 {
                let mut d = Draft::new(&store.data, &parent).unwrap();
                d.minutes = 60;
                d.reason =
                    "Fictional overtime: finished the month-end inventory reconciliation".into();
                store
                    .execute(Action::Overtime(OvertimeAction::Submit(d)))
                    .unwrap();
            }
            parents.push(parent.id);
        }
        let ids: Vec<_> = store.data.overtime.records.iter().map(|r| r.id).collect();
        let reviewer = crate::access::Access {
            company_id: store.data.company.id,
            user_id: Uuid::from_u128(779),
            employee_id: store
                .data
                .employees
                .iter()
                .find(|e| e.id != employee)
                .map(|e| e.id),
            role: Role::Manager,
            direct_reports: vec![employee],
            ..Default::default()
        };
        store.data.access = reviewer.clone();
        for (index, status, minutes) in [
            (1, Status::Approved, Some(45)),
            (2, Status::Rejected, None),
            (4, Status::Approved, Some(60)),
        ] {
            store
                .execute(Action::Overtime(OvertimeAction::Resolve {
                    id: ids[index],
                    expected_revision: 0,
                    status,
                    minutes,
                    reason: "Fictional review: verified work scope and attendance".into(),
                }))
                .unwrap();
        }
        store
            .execute(Action::Overtime(OvertimeAction::Resolve {
                id: ids[4],
                expected_revision: 1,
                status: Status::Revoked,
                minutes: None,
                reason: "Fictional revocation: revised authorization needed".into(),
            }))
            .unwrap();
        store.data.access = initial.clone();
        store
            .execute(Action::Overtime(OvertimeAction::Resolve {
                id: ids[3],
                expected_revision: 0,
                status: Status::Cancelled,
                minutes: None,
                reason: "Fictional requester cancellation".into(),
            }))
            .unwrap();
        self.data = store.data;
        self.data.access = initial;
        self.page = Page::Attendance;
        self.shift_state.tab = shifts_view::Tab::Overtime;
        self.attendance_date = day + Duration::days(5);
        self.calendar_month = month_start(self.attendance_date);
        self.overtime_state = State::default();
        match preview.as_str() {
            "ot-list" => {}
            "ot-form" => {
                let a = self
                    .data
                    .attendance
                    .iter()
                    .find(|a| a.id == parents[5])
                    .unwrap();
                let mut d = Draft::new(&self.data, a).unwrap();
                d.minutes = 60;
                d.reason =
                    "Fictional overtime: completed scheduled inventory reconciliation".into();
                self.overtime_state.form = Some(d);
            }
            "ot-history" => self.overtime_state.queue = Queue::History,
            "ot-approved" => self.overtime_state.detail = Some(ids[1]),
            "ot-revoked" => self.overtime_state.detail = Some(ids[4]),
            "ot-cancel" => {
                self.overtime_state.detail = Some(ids[0]);
                self.overtime_state.decision = Some(Status::Cancelled);
                self.overtime_state.reason =
                    "Fictional cancellation: revising the work details".into();
            }
            "ot-empty" => {
                self.data.overtime = Default::default();
                self.data.attendance.clear();
            }
            _ => {
                self.data.access = reviewer;
                self.overtime_state.detail = Some(ids[0]);
                if preview == "ot-hr" {
                    self.data.access.role = Role::HrGeneralist;
                }
                if preview == "ot-review" || preview == "ot-reject" {
                    self.overtime_state.decision = Some(if preview == "ot-review" {
                        Status::Approved
                    } else {
                        Status::Rejected
                    });
                    self.overtime_state.minutes = 45;
                    self.overtime_state.reason =
                        "Fictional review: 45 minutes verified for the stated work".into();
                }
                if preview == "ot-revoke" {
                    self.overtime_state.detail = Some(ids[1]);
                    self.overtime_state.decision = Some(Status::Revoked);
                    self.overtime_state.reason =
                        "Fictional revocation: attendance requires another review".into();
                }
                if preview == "ot-stale" || preview == "ot-approved-stale" {
                    let index = if preview == "ot-stale" { 0 } else { 1 };
                    self.data
                        .attendance
                        .iter_mut()
                        .find(|a| a.id == parents[index])
                        .unwrap()
                        .revision += 1;
                    self.overtime_state.detail = Some(ids[index]);
                }
            }
        }
    }
    pub(super) fn overtime_approvals(&mut self, ui: &mut Ui) {
        if self.overtime_state.form.is_some() {
            self.overtime_editor(ui);
            return;
        }
        if let Some(id) = self.overtime_state.detail {
            self.overtime_detail(ui, id);
            return;
        }
        ui.horizontal_wrapped(|ui| {
            ui.selectable_value(&mut self.overtime_state.queue, Queue::Mine, "My overtime");
            ui.selectable_value(
                &mut self.overtime_state.queue,
                Queue::Review,
                "Review queue",
            );
            ui.selectable_value(
                &mut self.overtime_state.queue,
                Queue::History,
                "Approval history",
            );
        });
        card(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.label("Your attendance work date");
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
            });
            let a = self
                .data
                .attendance
                .iter()
                .find(|a| {
                    Some(a.employee_id) == self.data.access.employee_id
                        && a.work_date == self.attendance_date
                })
                .cloned();
            if let Some(a) = a {
                if let Some(basis) = overtime::capture(&self.data, &a) {
                    basis_summary(ui, &basis);
                    let active = self.data.overtime.records.iter().any(|r| {
                        r.attendance_id == a.id
                            && matches!(r.status, Status::Pending | Status::Approved)
                    });
                    if active {
                        ui.label("A pending or approved request already exists for this record. Open it below; an approval must be revoked by an authorized reviewer before another request.");
                    } else if self
                        .data
                        .own_employee()
                        .is_some_and(|e| e.status == EmployeeStatus::Active)
                        && primary(ui, "Request overtime")
                    {
                        self.overtime_state.form = Some(Draft {
                            minutes: basis.minutes as u32,
                            basis,
                            reason: String::new(),
                        });
                    }
                } else {
                    ui.label("No overtime candidate is available for this attendance. Review completed times, saved shift/rules, breaks and approved leave in Time calculations.");
                }
                if ui.button("Open time calculations").clicked() {
                    self.shift_state.tab = shifts_view::Tab::Calculations;
                    self.calculation_state.detail = Some(a.id);
                }
            } else {
                ui.label("No own attendance record for this date. Choose a completed work date to request overtime.");
            }
            ui.label(RichText::new("Request minutes from the calculated candidate. A different authorized Manager or HR reviewer decides the amount. Payroll remains a separate module.").small().color(muted(ui)));
        });
        ui.add(
            egui::TextEdit::singleline(&mut self.overtime_state.search)
                .desired_width(420.0)
                .hint_text("Find employee by name or code"),
        );
        let search = self.overtime_state.search.trim().to_lowercase();
        let queue = self.overtime_state.queue;
        let access = &self.data.access;
        let mut rows: Vec<_> = self
            .data
            .overtime
            .records
            .iter()
            .filter(|r| match queue {
                Queue::Mine => r.submitted_by == access.user_id,
                Queue::Review => {
                    r.status == Status::Pending && overtime::can_review_in(&self.data, r)
                }
                Queue::History => r.status != Status::Pending,
            })
            .filter(|r| {
                search.is_empty()
                    || format!("{} {}", r.employee_name, r.employee_code)
                        .to_lowercase()
                        .contains(&search)
            })
            .cloned()
            .collect();
        rows.sort_by_key(|r| std::cmp::Reverse((r.created_at, r.id)));
        if rows.is_empty() {
            ui.add_space(12.0);
            ui.label(RichText::new("No overtime requests in this view.").color(muted(ui)));
        }
        for r in rows {
            card(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.strong(&r.employee_name);
                    badge(ui, &r.employee_code, true);
                    badge(ui, r.status.label(), r.status == Status::Approved);
                    ui.label(r.work_date.to_string());
                });
                ui.label(format!(
                    "Requested {} min · candidate {} min",
                    r.requested_minutes, r.basis.minutes
                ));
                if r.status == Status::Approved {
                    if let Some(m) = overtime::effective_minutes(&self.data, &r) {
                        ui.label(format!("Current approved overtime: {m} min"));
                    } else {
                        ui.colored_label(ui.visuals().error_fg_color,"Saved approval no longer matches current attendance, breaks or leave. Current approved minutes: 0.");
                    }
                }
                if ui.button("Open request / approval history").clicked() {
                    self.overtime_state.detail = Some(r.id);
                }
            });
        }
    }
    fn overtime_editor(&mut self, ui: &mut Ui) {
        let Some(mut draft) = self.overtime_state.form.take() else {
            return;
        };
        let mut cancel = false;
        card(ui, |ui| {
            ui.heading("Request overtime");
            ui.label(
                RichText::new(self.data.employee_name(draft.basis.attendance.employee_id)).strong(),
            );
            basis_summary(ui, &draft.basis);
            ui.add_enabled_ui(!self.busy,|ui|{ui.horizontal_wrapped(|ui|{ui.label("Requested minutes *");ui.add(egui::DragValue::new(&mut draft.minutes).range(1..=draft.basis.minutes as u32).speed(1));});ui.label("Reason *");ui.add_sized([ui.available_width(),85.0],egui::TextEdit::multiline(&mut draft.reason).hint_text("Describe the work performed outside the shift (up to 500 characters)"));ui.label("Actual attendance, saved rules and recorded breaks form the review basis. Approval can authorize fewer minutes than requested. Submitting does not change attendance.");
    if let Some((text,true))=&self.notice{ui.colored_label(ui.visuals().error_fg_color,text);}
    ui.horizontal_wrapped(|ui|{if primary(ui,"Submit overtime request"){let action=OvertimeAction::Submit(draft.clone());match overtime::payload(&action,&self.data){Ok(_)=>{self.overtime_state.saving=true;self.send(Command::Change(Action::Overtime(action)));},Err(e)=>self.notice=Some((e.to_string(),true))}}cancel=ui.button("Cancel").clicked();});
   });
        });
        #[cfg(feature = "qa-screenshot")]
        if std::env::var("HRIS_PREVIEW_SCROLL").as_deref() == Ok("bottom") {
            ui.scroll_to_cursor(Some(Align::BOTTOM));
        }
        if !cancel {
            self.overtime_state.form = Some(draft);
        }
    }
    fn overtime_detail(&mut self, ui: &mut Ui, id: Uuid) {
        if self.overtime_state.decision.is_none() && ui.button("Back to overtime").clicked() {
            self.overtime_state.detail = None;
            return;
        }
        let Some(r) = self
            .data
            .overtime
            .records
            .iter()
            .find(|r| r.id == id)
            .cloned()
        else {
            self.overtime_state.detail = None;
            return;
        };
        let current = overtime::matches_current(&self.data, &r);
        self.delegated_review_history(ui, crate::approvals::Scope::Overtime, id);
        card(ui, |ui| {
            ui.heading(&r.employee_name);
            ui.horizontal_wrapped(|ui| {
                badge(ui, &r.employee_code, true);
                badge(ui, r.status.label(), r.status == Status::Approved);
                ui.label(format!("Work date {}", r.work_date));
            });
            basis_summary(ui, &r.basis);
            ui.columns(2, |c| {
                c[0].strong("Requested overtime");
                c[0].label(format!("{} min", r.requested_minutes));
                c[1].strong("Current approved overtime");
                c[1].label(format!(
                    "{} min",
                    overtime::effective_minutes(&self.data, &r).unwrap_or(0)
                ));
            });
            if let Some(m) = r.approved_minutes {
                ui.label(format!("Saved approval amount: {m} min"));
            }
            ui.strong("Work reason");
            ui.label(&r.reason);
            if !current && matches!(r.status, Status::Pending | Status::Approved) {
                ui.colored_label(ui.visuals().error_fg_color,if r.status==Status::Pending{"Attendance, breaks or approved leave changed. Reject or cancel this proposal, then submit a fresh request."}else{"This approval remains in history but no longer matches current facts. Revoke it before requesting a new authorization."});
            }
            if self.overtime_state.decision.is_none() {
                ui.add_enabled_ui(!self.busy, |ui| {
                    ui.horizontal_wrapped(|ui| {
                        if r.status == Status::Pending {
                            if overtime::can_review_in(&self.data, &r) {
                                if ui
                                    .add_enabled(current, egui::Button::new("Approve minutes"))
                                    .clicked()
                                {
                                    self.overtime_state.decision = Some(Status::Approved);
                                    self.overtime_state.minutes = r.requested_minutes;
                                }
                                if ui.button("Reject with reason").clicked() {
                                    self.overtime_state.decision = Some(Status::Rejected);
                                }
                            }
                            if r.submitted_by == self.data.access.user_id
                                && ui.button("Cancel request").clicked()
                            {
                                self.overtime_state.decision = Some(Status::Cancelled);
                            }
                        }
                        if r.status == Status::Approved
                            && overtime::can_review_in(&self.data, &r)
                            && ui.button("Revoke approval with reason").clicked()
                        {
                            self.overtime_state.decision = Some(Status::Revoked);
                        }
                    });
                });
            }
        });
        if let Some(status) = self.overtime_state.decision {
            self.overtime_decision(ui, &r, status);
        }
        ui.heading("Submission & approval history");
        let mut events: Vec<_> = self
            .data
            .overtime
            .events
            .iter()
            .filter(|e| e.request_id == id)
            .collect();
        events.sort_by_key(|e| (e.created_at, e.id));
        for e in events {
            card(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    badge(ui, e.status.label(), e.status == Status::Approved);
                    ui.strong(&e.actor_name);
                    ui.label(e.actor_role.label());
                });
                ui.label(
                    e.created_at
                        .with_timezone(&self.data.company.tz())
                        .format("%b %d, %Y %H:%M:%S")
                        .to_string(),
                );
                ui.label(&e.reason);
                if let Some(m) = e.approved_minutes {
                    ui.label(format!(
                        "{}: {m} min",
                        if e.status == Status::Revoked {
                            "Previous approval retained in history"
                        } else {
                            "Approved amount"
                        }
                    ));
                }
            });
        }
        #[cfg(feature = "qa-screenshot")]
        if std::env::var("HRIS_PREVIEW_SCROLL").as_deref() == Ok("bottom") {
            ui.scroll_to_cursor(Some(Align::BOTTOM));
        }
    }
    fn overtime_decision(&mut self, ui: &mut Ui, r: &Request, status: Status) {
        let mut cancel = false;
        card(ui, |ui| {
            ui.heading(match status {
                Status::Approved => "Approve overtime minutes",
                Status::Rejected => "Reject overtime request",
                Status::Cancelled => "Cancel your overtime request",
                Status::Revoked => "Revoke overtime approval",
                Status::Pending => unreachable!(),
            });
            ui.label(if status==Status::Revoked{"Revocation removes the current authorization and preserves its earlier amount and reviewer history. Attendance stays unchanged."}else{"This decision records overtime authorization separately from attendance. It does not change punch times or calculate pay."});
            ui.add_enabled_ui(!self.busy, |ui| {
                if status == Status::Approved {
                    ui.horizontal_wrapped(|ui| {
                        ui.label("Approved minutes *");
                        ui.add(
                            egui::DragValue::new(&mut self.overtime_state.minutes)
                                .range(1..=r.requested_minutes),
                        );
                        ui.label(format!("Requested {} min", r.requested_minutes));
                    });
                }
                ui.label("Decision reason *");
                ui.add_sized(
                    [ui.available_width(), 76.0],
                    egui::TextEdit::multiline(&mut self.overtime_state.reason)
                        .hint_text("Explain this decision (up to 500 characters)"),
                );
                if let Some((text, true)) = &self.notice {
                    ui.colored_label(ui.visuals().error_fg_color, text);
                }
                ui.horizontal_wrapped(|ui| {
                    if primary(
                        ui,
                        match status {
                            Status::Approved => "Save approval",
                            Status::Rejected => "Reject request",
                            Status::Cancelled => "Confirm cancellation",
                            Status::Revoked => "Revoke approval",
                            Status::Pending => unreachable!(),
                        },
                    ) {
                        let action = OvertimeAction::Resolve {
                            id: r.id,
                            expected_revision: r.revision,
                            status,
                            minutes: (status == Status::Approved)
                                .then_some(self.overtime_state.minutes),
                            reason: self.overtime_state.reason.clone(),
                        };
                        match overtime::payload(&action, &self.data) {
                            Ok(_) => {
                                self.overtime_state.saving = true;
                                self.send(Command::Change(Action::Overtime(action)));
                            }
                            Err(e) => self.notice = Some((e.to_string(), true)),
                        }
                    }
                    cancel = ui.button("Back").clicked();
                });
            });
        });
        if cancel {
            self.overtime_state.decision = None;
            self.overtime_state.reason.clear();
        }
    }
}
