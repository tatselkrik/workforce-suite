use super::*;
use crate::requests::{self, Request, RequestAction, Status};

#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub(super) enum Queue {
    #[default]
    Mine,
    Review,
    History,
}
#[derive(Default)]
pub(super) struct State {
    pub form: Option<CorrectionDraft>,
    pub detail: Option<Uuid>,
    pub resolution: Option<Status>,
    pub reason: String,
    pub queue: Queue,
    pub search: String,
    pub saving: bool,
}
impl State {
    pub fn editing(&self) -> bool {
        self.form.is_some() || self.resolution.is_some()
    }
}
fn times(
    r: &Request,
    start: chrono::DateTime<chrono::Utc>,
    end: Option<chrono::DateTime<chrono::Utc>>,
) -> String {
    let tz = r
        .timezone
        .parse::<chrono_tz::Tz>()
        .unwrap_or(chrono_tz::UTC);
    format!(
        "{} to {}",
        start.with_timezone(&tz).format("%b %d, %Y %H:%M:%S"),
        end.map(|t| t
            .with_timezone(&tz)
            .format("%b %d, %Y %H:%M:%S")
            .to_string())
            .unwrap_or_else(|| "Open shift".into())
    )
}
impl HrisApp {
    #[cfg(feature = "qa-screenshot")]
    pub(super) fn prepare_request_preview(&mut self) {
        use chrono::{Duration, Utc};
        let preview = std::env::var("HRIS_PREVIEW_FORM").unwrap_or_default();
        if !preview.starts_with("req-") || self.data.mode != Mode::Demo {
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
        store.data.requests = Default::default();
        store
            .data
            .employees
            .iter_mut()
            .find(|e| e.id == employee)
            .unwrap()
            .joined_on = None;
        let day = store.data.company.today() - Duration::days(7);
        let mut recorded = CorrectionDraft::new(employee, day, None, &store.data);
        recorded.checked_in = "08:10".into();
        recorded.checked_out = "17:00".into();
        recorded.reason = "Fictional starting attendance".into();
        store
            .execute(Action::CorrectAttendance(
                recorded.change(&store.data, Utc::now()).unwrap(),
            ))
            .unwrap();
        for offset in 0..5 {
            let date = day + Duration::days(offset);
            let old = store
                .data
                .attendance
                .iter()
                .find(|a| a.work_date == date)
                .cloned();
            let mut d = CorrectionDraft::new(employee, date, old, &store.data);
            d.checked_in = "08:25".into();
            d.checked_out = "17:15".into();
            d.reason = "Fictional request: missed punch verified against the shift log".into();
            store
                .execute(Action::AttendanceRequest(RequestAction::Submit(d)))
                .unwrap();
        }
        let ids: Vec<_> = store.data.requests.records.iter().map(|r| r.id).collect();
        let reviewer = crate::access::Access {
            company_id: store.data.company.id,
            user_id: Uuid::from_u128(778),
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
        for (index, status) in [(1, Status::Approved), (2, Status::Rejected)] {
            store
                .execute(Action::AttendanceRequest(RequestAction::Resolve {
                    id: ids[index],
                    expected_revision: 0,
                    status,
                    reason: "Fictional reviewed decision: attendance log checked".into(),
                }))
                .unwrap();
        }
        store.data.access = initial.clone();
        store
            .execute(Action::AttendanceRequest(RequestAction::Resolve {
                id: ids[3],
                expected_revision: 0,
                status: Status::Cancelled,
                reason: "Fictional cancellation: submitting a revised request".into(),
            }))
            .unwrap();
        self.data = store.data;
        self.data.access = initial;
        self.attendance_date = day;
        self.calendar_month = month_start(day);
        self.page = Page::Attendance;
        self.shift_state.tab = shifts_view::Tab::Requests;
        self.request_state = State::default();
        match preview.as_str() {
            "req-form" | "req-form-error" => {
                let old = self.data.attendance[0].clone();
                let mut d = CorrectionDraft::new(employee, day, Some(old), &self.data);
                d.checked_in = "08:30".into();
                d.reason = "Fictional request: arrival confirmed by the duty log".into();
                if preview == "req-form-error" {
                    d.checked_out = "07:00".into();
                    self.notice = Some(("Check-out must be on or after check-in.".into(), true));
                }
                self.request_state.form = Some(d);
            }
            "req-list" => {}
            "req-empty" => {
                self.data.requests = Default::default();
            }
            "req-history" => {
                self.request_state.queue = Queue::History;
            }
            "req-approved" => self.request_state.detail = Some(ids[1]),
            "req-cancel" => {
                self.request_state.detail = Some(ids[0]);
                self.request_state.resolution = Some(Status::Cancelled);
                self.request_state.reason =
                    "Fictional cancellation: I will submit corrected details".into();
            }
            "req-missing" => self.request_state.detail = Some(ids[4]),
            "req-stale" => {
                self.data.attendance[0].revision += 1;
                self.data.access = reviewer;
                self.request_state.detail = Some(ids[0]);
            }
            "req-review" | "req-reject" | "req-manager" | "req-hr" => {
                self.data.access = reviewer;
                if preview == "req-hr" || preview == "req-reject" {
                    self.data.access.role = Role::HrGeneralist;
                }
                self.request_state.detail = Some(ids[0]);
                if preview == "req-review" || preview == "req-reject" {
                    self.request_state.resolution = Some(if preview == "req-review" {
                        Status::Approved
                    } else {
                        Status::Rejected
                    });
                    self.request_state.reason =
                        "Fictional review: the supplied time was checked against the shift log"
                            .into();
                }
            }
            _ => self.request_state.detail = Some(ids[0]),
        }
    }
    fn open_own_request(&mut self) {
        if let Some(employee) = self
            .data
            .own_employee()
            .filter(|e| e.status == EmployeeStatus::Active)
        {
            let original = self
                .data
                .attendance
                .iter()
                .find(|a| a.employee_id == employee.id && a.work_date == self.attendance_date)
                .cloned();
            self.request_state.form = Some(CorrectionDraft::new(
                employee.id,
                self.attendance_date,
                original,
                &self.data,
            ));
            self.request_state.detail = None;
        }
    }
    pub(super) fn attendance_requests(&mut self, ui: &mut Ui) {
        if self.request_state.form.is_some() {
            self.request_editor(ui);
            return;
        }
        if let Some(id) = self.request_state.detail {
            self.request_detail(ui, id);
            return;
        }
        ui.horizontal_wrapped(|ui| {
            ui.selectable_value(&mut self.request_state.queue, Queue::Mine, "My requests");
            ui.selectable_value(&mut self.request_state.queue, Queue::Review, "Review queue");
            ui.selectable_value(
                &mut self.request_state.queue,
                Queue::History,
                "Request history",
            );
        });
        ui.add_space(10.0);
        ui.horizontal_wrapped(|ui| {
            ui.label("Work date for a new request");
            let selected = ui.button(self.attendance_date.to_string());
            let popup = ui.make_persistent_id("request_work_date");
            egui::Popup::from_toggle_button_response(&selected)
                .id(popup)
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
            if self
                .data
                .own_employee()
                .is_some_and(|e| e.status == EmployeeStatus::Active)
                && primary(ui, "+ Request correction")
            {
                self.open_own_request();
            }
        });
        ui.label(RichText::new("Submit your own missing or incorrect attendance for review. A request changes attendance only when another authorized reviewer approves it. Managers review current direct reports; HR/Admin can review company requests.").color(muted(ui)).small());
        ui.add(
            egui::TextEdit::singleline(&mut self.request_state.search)
                .hint_text("Find request by employee name or code")
                .desired_width(420.0),
        );
        let search = self.request_state.search.trim().to_lowercase();
        let access = &self.data.access;
        let queue = self.request_state.queue;
        let mut rows: Vec<_> = self
            .data
            .requests
            .records
            .iter()
            .filter(|r| match queue {
                Queue::Mine => r.submitted_by == access.user_id,
                Queue::Review => requests::can_review(access, r),
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
            ui.add_space(14.0);
            ui.label(RichText::new(match queue {Queue::Mine=>"No requests for your account. Choose a work date to submit your own correction.",Queue::Review=>"No pending requests currently require your review.",Queue::History=>"No resolved requests in your permitted scope."}).color(muted(ui)));
        }
        for r in rows {
            card(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.strong(&r.employee_name);
                    badge(ui, &r.employee_code, true);
                    badge(ui, r.status.label(), r.status == Status::Approved);
                    ui.label(format!("Work date {}", r.work_date));
                });
                ui.label(format!(
                    "Proposed: {}",
                    times(&r, r.proposed_in, r.proposed_out)
                ));
                ui.label(&r.reason);
                if ui.button("Open request / approval history").clicked() {
                    self.request_state.detail = Some(r.id);
                }
            });
        }
    }
    fn request_editor(&mut self, ui: &mut Ui) {
        let Some(mut draft) = self.request_state.form.take() else {
            return;
        };
        let mut cancel = false;
        card(ui, |ui| {
            ui.heading("Request an attendance correction");
            ui.label(RichText::new(self.data.employee_name(draft.employee_id)).strong());
            ui.label(format!(
                "Work date {} · {}",
                draft.work_date, self.data.company.timezone
            ));
            if let Some(old) = &draft.original {
                ui.label(format!("Current: {}", attendance_times(old, &self.data)));
            } else {
                ui.label("No attendance record is currently saved for this date.");
            }
            ui.add_enabled_ui(!self.busy,|ui|{
    ui.columns(2,|c|{field(&mut c[0],"Check-in date *",&mut draft.checked_in_date,"YYYY-MM-DD");field(&mut c[1],"Check-in time *",&mut draft.checked_in,"24-hour time, e.g. 08:30");});
    ui.columns(2,|c|{field(&mut c[0],"Check-out date",&mut draft.checked_out_date,"YYYY-MM-DD");field(&mut c[1],"Check-out time",&mut draft.checked_out,"Empty if still open");});
    ui.label("Reason *");ui.add_sized([ui.available_width(),90.0],egui::TextEdit::multiline(&mut draft.reason).hint_text("Explain the missing or incorrect time. Avoid sensitive personal details."));
    ui.label(RichText::new(format!("{} / 500 characters",draft.reason.trim().chars().count())).small().color(muted(ui)));
    ui.label("Submitting does not change attendance. Keep one pending request per date; cancel and resubmit to revise it. Overnight check-ins may use the next date within the saved shift. An open shift prevents another check-in.");
    if let Some((text,true))=&self.notice{ui.colored_label(ui.visuals().error_fg_color,text);}
    ui.horizontal_wrapped(|ui|{if primary(ui,"Submit request"){let action=RequestAction::Submit(draft.clone());match requests::payload(&action,&self.data,chrono::Utc::now()){Ok(_)=>{self.request_state.saving=true;self.send(Command::Change(Action::AttendanceRequest(action)));},Err(e)=>self.notice=Some((e.to_string(),true))}}cancel=ui.button("Cancel").clicked();});
   });
        });
        #[cfg(feature = "qa-screenshot")]
        if std::env::var("HRIS_PREVIEW_SCROLL").as_deref() == Ok("bottom") {
            ui.scroll_to_cursor(Some(Align::BOTTOM));
        }
        if !cancel {
            self.request_state.form = Some(draft);
        }
    }
    fn request_detail(&mut self, ui: &mut Ui, id: Uuid) {
        if self.request_state.resolution.is_none() && ui.button("Back to requests").clicked() {
            self.request_state.detail = None;
            return;
        }
        let Some(r) = self
            .data
            .requests
            .records
            .iter()
            .find(|r| r.id == id)
            .cloned()
        else {
            self.request_state.detail = None;
            return;
        };
        let stale = r.status == Status::Pending && requests::stale(&self.data, &r);
        card(ui, |ui| {
            ui.heading(&r.employee_name);
            ui.horizontal_wrapped(|ui| {
                badge(ui, &r.employee_code, true);
                badge(ui, r.status.label(), r.status == Status::Approved);
                ui.label(format!("Work date {} · {}", r.work_date, r.timezone));
            });
            ui.columns(2, |c| {
                c[0].strong("Recorded when submitted");
                c[0].label(
                    r.base_record
                        .as_ref()
                        .map(|a| times(&r, a.checked_in_at, a.checked_out_at))
                        .unwrap_or_else(|| "No attendance record".into()),
                );
                c[1].strong("Requested times");
                c[1].label(times(&r, r.proposed_in, r.proposed_out));
            });
            ui.label(RichText::new("Request reason").strong());
            ui.label(&r.reason);
            if stale {
                ui.colored_label(ui.visuals().error_fg_color,"Attendance, time zone or the missing record schedule changed. This proposal cannot be approved. Reject or cancel it, then submit a fresh request.");
            }
            if r.status == Status::Pending && self.request_state.resolution.is_none() {
                ui.add_enabled_ui(!self.busy, |ui| {
                    ui.horizontal_wrapped(|ui| {
                        if requests::can_review(&self.data.access, &r) {
                            if ui
                                .add_enabled(!stale, egui::Button::new("Approve with note"))
                                .clicked()
                            {
                                self.request_state.resolution = Some(Status::Approved);
                            }
                            if ui.button("Reject with reason").clicked() {
                                self.request_state.resolution = Some(Status::Rejected);
                            }
                        }
                        if r.submitted_by == self.data.access.user_id
                            && ui.button("Cancel request").clicked()
                        {
                            self.request_state.resolution = Some(Status::Cancelled);
                        }
                    });
                });
            }
            if r.status == Status::Approved {
                ui.label("The approved correction was applied atomically. Its attendance audit and request history remain saved.");
            }
        });
        if let Some(status) = self.request_state.resolution {
            let mut cancel = false;
            card(ui, |ui| {
                ui.heading(match status {
                    Status::Approved => "Approve and apply correction",
                    Status::Rejected => "Reject request",
                    Status::Cancelled => "Cancel your request",
                    Status::Pending => unreachable!(),
                });
                ui.label(if status==Status::Approved {"The requested times will replace the recorded times. Current attendance and break safeguards are checked again. Recorded shifts, calculation rules and breaks are retained for existing attendance."}else{"Attendance will stay unchanged. The request, reason and decision remain in history."});
                ui.add_enabled_ui(!self.busy, |ui| {
                    ui.label("Decision reason *");
                    ui.add_sized(
                        [ui.available_width(), 76.0],
                        egui::TextEdit::multiline(&mut self.request_state.reason)
                            .hint_text("Explain the review or cancellation (up to 500 characters)"),
                    );
                    if let Some((text, true)) = &self.notice {
                        ui.colored_label(ui.visuals().error_fg_color, text);
                    }
                    ui.horizontal_wrapped(|ui| {
                        if primary(
                            ui,
                            match status {
                                Status::Approved => "Approve & apply",
                                Status::Rejected => "Reject request",
                                Status::Cancelled => "Confirm cancellation",
                                Status::Pending => unreachable!(),
                            },
                        ) {
                            let action = RequestAction::Resolve {
                                id: r.id,
                                expected_revision: r.revision,
                                status,
                                reason: self.request_state.reason.clone(),
                            };
                            match requests::payload(&action, &self.data, chrono::Utc::now()) {
                                Ok(_) => {
                                    self.request_state.saving = true;
                                    self.send(Command::Change(Action::AttendanceRequest(action)));
                                }
                                Err(e) => self.notice = Some((e.to_string(), true)),
                            }
                        }
                        cancel = ui.button("Back").clicked();
                    });
                });
            });
            if cancel {
                self.request_state.resolution = None;
                self.request_state.reason.clear();
            }
        }
        ui.heading("Submission & approval history");
        let mut events: Vec<_> = self
            .data
            .requests
            .events
            .iter()
            .filter(|e| e.request_id == id)
            .collect();
        events.sort_by_key(|e| (e.created_at, e.id));
        for event in events {
            card(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    badge(ui, event.status.label(), event.status == Status::Approved);
                    ui.strong(&event.actor_name);
                    ui.label(event.actor_role.label());
                });
                ui.label(
                    event
                        .created_at
                        .with_timezone(&self.data.company.tz())
                        .format("%b %d, %Y %H:%M:%S")
                        .to_string(),
                );
                ui.label(&event.reason);
                if let Some(a) = &event.after_record {
                    ui.columns(2, |c| {
                        c[0].strong("Before approval");
                        c[0].label(
                            event
                                .before_record
                                .as_ref()
                                .map(|b| times(&r, b.checked_in_at, b.checked_out_at))
                                .unwrap_or_else(|| "No attendance record".into()),
                        );
                        c[1].strong("After approval");
                        c[1].label(times(&r, a.checked_in_at, a.checked_out_at));
                    });
                }
            });
        }
        #[cfg(feature = "qa-screenshot")]
        if std::env::var("HRIS_PREVIEW_SCROLL").as_deref() == Ok("bottom") {
            ui.scroll_to_cursor(Some(Align::BOTTOM));
        }
    }
}
