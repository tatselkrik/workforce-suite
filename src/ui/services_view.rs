use super::*;
use crate::services::{
    self, Audience, Category, Change, Context, Operation, Priority, Request, Response, Status,
    Ticket,
};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Submit,
    Reply,
    Route,
    Progress,
    Cancel,
}
struct Form {
    kind: Kind,
    request_id: Uuid,
    source_revision: i64,
    employee_id: Option<Uuid>,
    assignee: Option<Uuid>,
    due_on: String,
    title: String,
    category: Category,
    priority: Priority,
    status: Status,
    body: String,
    pending: Option<Request>,
    rejected: bool,
}
impl Form {
    fn new(kind: Kind, ticket: Option<&Ticket>, data: &Snapshot) -> Self {
        Self {
            kind,
            request_id: ticket.map(|t| t.id).unwrap_or_else(Uuid::new_v4),
            source_revision: ticket.map(|t| t.revision).unwrap_or(0),
            employee_id: data.access.employee_id,
            assignee: ticket.and_then(|t| t.assignee_user_id),
            due_on: ticket
                .and_then(|t| t.due_on)
                .map(|v| v.to_string())
                .unwrap_or_default(),
            title: String::new(),
            category: Category::GeneralSupport,
            priority: Priority::Normal,
            status: Status::InProgress,
            body: String::new(),
            pending: None,
            rejected: false,
        }
    }
    fn change(&self) -> anyhow::Result<Change> {
        let body = self.body.trim().to_owned();
        Ok(match self.kind {
            Kind::Submit => Change::Submit {
                employee_id: self
                    .employee_id
                    .ok_or_else(|| anyhow::anyhow!("Choose an employee."))?,
                title: self.title.trim().into(),
                category: self.category,
                priority: self.priority,
                body,
            },
            Kind::Reply => Change::Reply { body },
            Kind::Route => {
                Change::Route {
                    assignee_user_id: self.assignee,
                    due_on: if self.due_on.trim().is_empty() {
                        None
                    } else {
                        Some(NaiveDate::parse_from_str(self.due_on.trim(),"%Y-%m-%d").map_err(|_|anyhow::anyhow!("Use a real due date in YYYY-MM-DD format, or leave it blank."))?)
                    },
                    body,
                }
            }
            Kind::Progress => Change::Progress {
                status: self.status,
                body,
            },
            Kind::Cancel => Change::Cancel { body },
        })
    }
}
#[derive(Default)]
pub(super) struct State {
    context: Option<Context>,
    selected: Option<Uuid>,
    form: Option<Form>,
    search: String,
    status_filter: Option<Status>,
    error: Option<String>,
}
impl State {
    pub fn editing(&self) -> bool {
        self.form.is_some()
    }
    pub fn clear_context(&mut self) {
        self.context = None;
        self.selected = None;
    }
    pub fn reject(&mut self) {
        if let Some(f) = &mut self.form {
            f.rejected = true;
        }
    }
    pub fn accept(&mut self, response: Response, data: &Snapshot) -> anyhow::Result<bool> {
        response.context.validate(&data.access)?;
        anyhow::ensure!(
            response.context.audience == Audience::Staff,
            "The desktop needs a staff service context."
        );
        let mut revoked = false;
        if let Some(receipt) = response.receipt {
            let pending = self
                .form
                .as_ref()
                .and_then(|f| f.pending.as_ref())
                .ok_or_else(|| anyhow::anyhow!("No matching service save is pending."))?;
            receipt.validate(pending)?;
            anyhow::ensure!(
                response.context.events.iter().any(|e| e.id == receipt.id
                    && e.request_id == receipt.request_id
                    && e.actor_id == receipt.actor_id
                    && e.revision == receipt.revision),
                "The confirmed service change is missing from retained history."
            );
            self.selected = Some(receipt.request_id);
            self.form = None;
        }
        if self.form.as_ref().is_some_and(|f| {
            f.kind != Kind::Submit && response.context.ticket(f.request_id).is_err()
        }) {
            self.form = None;
            revoked = true;
        }
        if self
            .selected
            .is_some_and(|id| response.context.ticket(id).is_err())
        {
            self.selected = None;
            revoked = true;
        }
        self.context = Some(response.context);
        self.error = None;
        Ok(revoked)
    }
    fn show(&mut self, ui: &mut Ui, data: &Snapshot, busy: bool) -> Option<Operation> {
        ui.label(RichText::new("HR coordinates the queue. Other staff see their own requests and work assigned specifically to them. All replies are shared with the requester.").color(muted(ui)).small());
        let mut output = None;
        ui.horizontal_wrapped(|ui| {
            if ui
                .add_enabled(
                    !busy,
                    egui::Button::new(if self.context.is_some() {
                        "Refresh requests"
                    } else {
                        "Load requests"
                    }),
                )
                .clicked()
            {
                output = Some(Operation::Refresh);
            }
            if self.context.is_some()
                && self.form.is_none()
                && ui
                    .add_enabled(!busy, egui::Button::new("New service request"))
                    .clicked()
            {
                self.form = Some(Form::new(Kind::Submit, None, data));
                self.error = None;
            }
        });
        if let Some(error) = &self.error {
            ui.colored_label(Color32::from_rgb(190, 70, 55), error);
        }
        let context = self.context.take();
        if self.form.is_some() {
            if let Some(op) = self.show_form(ui, data, context.as_ref(), busy) {
                output = Some(op);
            }
        } else if let Some(c) = &context {
            if let Some(ticket) = self
                .selected
                .and_then(|id| c.requests.iter().find(|t| t.id == id))
            {
                if ui.button("Back to requests").clicked() {
                    self.selected = None;
                }
                ui.add_space(8.0);
                ui.label(RichText::new(&ticket.code).small());
                ui.heading(&ticket.title);
                ui.label(format!(
                    "{} · {} · {} priority",
                    ticket.status.label(),
                    ticket.category.label(),
                    ticket.priority.label()
                ));
                ui.label(format!(
                    "Requester: {} · {}",
                    ticket.requester_code, ticket.requester_name
                ));
                ui.label(format!(
                    "Handler: {}",
                    ticket
                        .assignee_name
                        .as_deref()
                        .unwrap_or("HR service queue")
                ));
                if let Some(due) = ticket.due_on {
                    ui.label(format!(
                        "Due {}{}",
                        due.format("%b %d, %Y"),
                        if due < data.company.today() && ticket.status.active() {
                            " · overdue"
                        } else {
                            ""
                        }
                    ));
                }
                ui.label(
                    RichText::new(format!(
                        "Revision {} · Updated {}",
                        ticket.revision,
                        ticket
                            .updated_at
                            .with_timezone(&data.company.tz())
                            .format("%b %d, %Y %H:%M")
                    ))
                    .small(),
                );
                ui.add_space(10.0);
                ui.horizontal_wrapped(|ui| {
                    if ticket.status.active()
                        && ui.add_enabled(!busy, egui::Button::new("Reply")).clicked()
                    {
                        self.form = Some(Form::new(Kind::Reply, Some(ticket), data));
                    }
                    if c.can_route
                        && ticket.status.active()
                        && ui
                            .add_enabled(!busy, egui::Button::new("Route request"))
                            .clicked()
                    {
                        self.form = Some(Form::new(Kind::Route, Some(ticket), data));
                    }
                    if c.handles(ticket, &data.access)
                        && Status::ALL
                            .iter()
                            .any(|s| ticket.status.permits(*s, c.hr(&data.access)))
                        && ui
                            .add_enabled(
                                !busy,
                                egui::Button::new(if ticket.status.active() {
                                    "Change status"
                                } else {
                                    "Reopen request"
                                }),
                            )
                            .clicked()
                    {
                        let mut f = Form::new(Kind::Progress, Some(ticket), data);
                        f.status = Status::ALL
                            .into_iter()
                            .find(|s| ticket.status.permits(*s, c.hr(&data.access)))
                            .unwrap_or(Status::InProgress);
                        self.form = Some(f);
                    }
                    if ticket.status.active()
                        && (c.owns(ticket) || c.hr(&data.access))
                        && ui
                            .add_enabled(!busy, egui::Button::new("Cancel request"))
                            .clicked()
                    {
                        self.form = Some(Form::new(Kind::Cancel, Some(ticket), data));
                    }
                });
                ui.add_space(14.0);
                ui.heading("Shared request history");
                for event in c.events.iter().filter(|e| e.request_id == ticket.id).rev() {
                    Frame::group(ui.style()).show(ui, |ui| {
                        ui.label(
                            RichText::new(format!(
                                "Revision {} · {} · {}",
                                event.revision,
                                event.kind.replace('_', " "),
                                event.actor_name
                            ))
                            .strong(),
                        );
                        ui.label(
                            RichText::new(
                                event
                                    .created_at
                                    .with_timezone(&data.company.tz())
                                    .format("%b %d, %Y %H:%M")
                                    .to_string(),
                            )
                            .small(),
                        );
                        if matches!(event.kind.as_str(), "progress" | "cancel") {
                            ui.label(format!("Status: {}", event.current_state.status.label()));
                        }
                        if event.kind == "route" {
                            ui.label(format!(
                                "Handler: {}",
                                event
                                    .current_state
                                    .assignee_name
                                    .as_deref()
                                    .unwrap_or("HR service queue")
                            ));
                            if let Some(d) = event.current_state.due_on {
                                ui.label(format!("Due {d}"));
                            }
                        }
                        ui.label(&event.body);
                    });
                    ui.add_space(6.0);
                }
            } else {
                let active = c.requests.iter().filter(|t| t.status.active()).count();
                let overdue = c
                    .requests
                    .iter()
                    .filter(|t| {
                        t.status.active() && t.due_on.is_some_and(|d| d < data.company.today())
                    })
                    .count();
                ui.add_space(8.0);
                ui.label(format!(
                    "{} visible requests · {active} active · {overdue} overdue",
                    c.requests.len()
                ));
                ui.horizontal_wrapped(|ui| {
                    ui.add(
                        egui::TextEdit::singleline(&mut self.search)
                            .hint_text("Search request or requester"),
                    );
                    egui::ComboBox::from_id_salt("services-filter")
                        .selected_text(
                            self.status_filter
                                .map(|s| s.label())
                                .unwrap_or("All statuses"),
                        )
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut self.status_filter, None, "All statuses");
                            for s in Status::ALL {
                                ui.selectable_value(&mut self.status_filter, Some(s), s.label());
                            }
                        });
                });
                let query = self.search.trim().to_lowercase();
                let mut found = false;
                for ticket in c
                    .requests
                    .iter()
                    .filter(|t| self.status_filter.is_none_or(|s| s == t.status))
                {
                    if !query.is_empty()
                        && !format!(
                            "{} {} {} {}",
                            ticket.title, ticket.code, ticket.requester_code, ticket.requester_name
                        )
                        .to_lowercase()
                        .contains(&query)
                    {
                        continue;
                    }
                    found = true;
                    Frame::group(ui.style()).show(ui, |ui| {
                        if ui
                            .add(egui::Button::new(RichText::new(&ticket.title).strong()).wrap())
                            .clicked()
                        {
                            self.selected = Some(ticket.id);
                        }
                        ui.label(format!(
                            "{} · {} · {}",
                            ticket.requester_code,
                            ticket.requester_name,
                            ticket.status.label()
                        ));
                        ui.label(format!(
                            "{} · Handler: {}",
                            ticket.category.label(),
                            ticket
                                .assignee_name
                                .as_deref()
                                .unwrap_or("HR service queue")
                        ));
                        ui.label(RichText::new(&ticket.code).small());
                    });
                    ui.add_space(6.0);
                }
                if !found {
                    ui.label("No requests match this view.");
                }
            }
        } else {
            ui.label("Load the current service queue to begin. Ordinary employees submit and track their requests in the local portal.");
        }
        self.context = context;
        output
    }
    fn show_form(
        &mut self,
        ui: &mut Ui,
        data: &Snapshot,
        c: Option<&Context>,
        busy: bool,
    ) -> Option<Operation> {
        let mut f = self.form.take().unwrap();
        let mut output = None;
        let mut discard = false;
        ui.add_space(12.0);
        ui.heading(match f.kind {
            Kind::Submit => "New service request",
            Kind::Reply => "Reply to request",
            Kind::Route => "Route request",
            Kind::Progress => "Change request status",
            Kind::Cancel => "Cancel service request",
        });
        if let Some(ticket) = c.and_then(|c| c.ticket(f.request_id).ok()) {
            Frame::group(ui.style()).show(ui, |ui| {
                ui.label(RichText::new(&ticket.title).strong());
                ui.label(format!(
                    "{} · {}",
                    ticket.requester_code, ticket.requester_name
                ));
                ui.label(format!(
                    "{} · Revision {}",
                    ticket.status.label(),
                    ticket.revision
                ));
            });
        }
        ui.label(RichText::new("Your message and the recorded outcome are visible to the requester and current handling staff.").color(muted(ui)).small());
        ui.add_enabled_ui(!busy && f.pending.is_none(), |ui| {
            if f.kind == Kind::Submit {
                if c.is_some_and(|c| c.can_route) {
                    ui.label("Requester");
                    egui::ComboBox::from_id_salt("services-employee")
                        .selected_text(
                            data.employees
                                .iter()
                                .find(|e| Some(e.id) == f.employee_id)
                                .map(|e| format!("{} · {}", e.employee_code, e.full_name))
                                .unwrap_or_else(|| "Choose an employee".into()),
                        )
                        .show_ui(ui, |ui| {
                            for e in &data.employees {
                                ui.selectable_value(
                                    &mut f.employee_id,
                                    Some(e.id),
                                    format!(
                                        "{} · {}{}",
                                        e.employee_code,
                                        e.full_name,
                                        if e.status == EmployeeStatus::Archived {
                                            " · archived"
                                        } else {
                                            ""
                                        }
                                    ),
                                );
                            }
                        });
                } else {
                    ui.label("This request is for your own employee record.");
                }
                ui.label("Request title");
                ui.add(
                    egui::TextEdit::singleline(&mut f.title)
                        .id(egui::Id::unique("services-title"))
                        .desired_width(ui.available_width()),
                );
                ui.horizontal_wrapped(|ui| {
                    egui::ComboBox::from_id_salt("services-category")
                        .selected_text(f.category.label())
                        .show_ui(ui, |ui| {
                            for category in Category::ALL {
                                ui.selectable_value(&mut f.category, category, category.label());
                            }
                        });
                    egui::ComboBox::from_id_salt("services-priority")
                        .selected_text(f.priority.label())
                        .show_ui(ui, |ui| {
                            for p in [Priority::Normal, Priority::High] {
                                ui.selectable_value(&mut f.priority, p, p.label());
                            }
                        });
                });
            }
            if f.kind == Kind::Route {
                ui.label("Handling staff");
                egui::ComboBox::from_id_salt("services-handler")
                    .selected_text(
                        f.assignee
                            .and_then(|id| {
                                c.and_then(|c| c.handlers.iter().find(|h| h.user_id == id))
                            })
                            .map(|h| h.full_name.as_str())
                            .unwrap_or("HR service queue"),
                    )
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut f.assignee, None, "HR service queue (unassigned)");
                        if let Some(c) = c {
                            for h in &c.handlers {
                                ui.selectable_value(&mut f.assignee, Some(h.user_id), &h.full_name);
                            }
                        }
                    });
                ui.label("Due date (YYYY-MM-DD, optional)");
                ui.add(
                    egui::TextEdit::singleline(&mut f.due_on)
                        .id(egui::Id::unique("services-due"))
                        .desired_width(180.0),
                );
            }
            if f.kind == Kind::Progress {
                ui.label("New status");
                egui::ComboBox::from_id_salt("services-status")
                    .selected_text(f.status.label())
                    .show_ui(ui, |ui| {
                        if let Some(c) = c {
                            if let Ok(t) = c.ticket(f.request_id) {
                                for s in Status::ALL {
                                    if t.status.permits(s, c.hr(&data.access)) {
                                        ui.selectable_value(&mut f.status, s, s.label());
                                    }
                                }
                            }
                        }
                    });
            }
            ui.label(match f.kind {
                Kind::Submit => "Describe your request",
                Kind::Reply => "Shared reply",
                Kind::Route => "Routing reason",
                Kind::Progress => "Outcome and next steps",
                Kind::Cancel => "Cancellation reason",
            });
            ui.add(
                egui::TextEdit::multiline(&mut f.body)
                    .id(egui::Id::unique("services-body"))
                    .desired_rows(5)
                    .desired_width(ui.available_width()),
            );
        });
        let stale = c.is_some_and(|c| {
            f.kind != Kind::Submit
                && c.ticket(f.request_id)
                    .is_ok_and(|t| t.revision != f.source_revision)
        });
        if f.pending.is_some() {
            ui.label(if f.rejected{"The server rejected this change. Refresh and explicitly review before preparing another save."}else{"This exact save is awaiting confirmation. Retry the same save without creating a replacement."});
        } else if stale {
            ui.label("This request changed while the draft was open. Review the current version before saving.");
        }
        ui.horizontal_wrapped(|ui|{
            if ui.add_enabled(!busy && !f.rejected && (c.is_some()||f.pending.is_some()) && (!stale||f.pending.is_some()),egui::Button::new(if f.pending.is_some(){"Retry same save"}else{"Save request"})).clicked(){
                let r=if let Some(pending)=&f.pending{pending.binding(&data.access).and_then(|_|{if let Some(c)=c{pending.current_scope(c,&data.access)?;}Ok(pending.clone())})}else{c.ok_or_else(||anyhow::anyhow!("Refresh requests first.")).and_then(|c|Request::new(&data.access,c,f.request_id,f.change()?))};
                match r{Ok(r)=>{f.pending=Some(r.clone());self.error=None;output=Some(Operation::Save(Box::new(r)));},Err(e)=>self.error=Some(e.to_string())}
            }
            if !busy && (stale||f.rejected) && c.is_some() && ui.button("Review current version").clicked(){
                let c=c.unwrap();
                if f.kind==Kind::Submit && c.ticket(f.request_id).is_ok(){self.error=Some("This request already exists. Keep the saved identity and retry it, or inspect its history before starting another request.".into());}
                else{f.source_revision=if f.kind==Kind::Submit{0}else{c.ticket(f.request_id).map(|t|t.revision).unwrap_or(f.source_revision)};f.pending=None;f.rejected=false;self.error=None;}
            }
            if ui.add_enabled(!busy && (f.pending.is_none()||f.rejected),egui::Button::new("Discard draft")).clicked(){discard=true;self.error=None;}
        });
        if !discard {
            self.form = Some(f);
        }
        output
    }
}
impl HrisApp {
    pub(super) fn services(&mut self, ui: &mut Ui) {
        if self.data.mode != Mode::Demo && !services::admitted(&self.data.access, Audience::Staff) {
            self.services_state = State::default();
            return;
        }
        let mut state = std::mem::take(&mut self.services_state);
        let op = state.show(ui, &self.data, self.busy);
        self.services_state = state;
        if let Some(op) = op {
            self.send(Command::Services(op));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> (Snapshot, Context, Request, Response) {
        let mut d = DemoStore::new().data;
        d.access.user_id = Uuid::from_u128(670);
        d.accounts[0].user_id = d.access.user_id;
        let mut repo = services::Demo::default();
        let c = repo.context(&d, Audience::Staff).unwrap();
        let r = Request::new(
            &d.access,
            &c,
            Uuid::new_v4(),
            Change::Submit {
                employee_id: d.employees[1].id,
                title: "Fictional pending request".into(),
                category: Category::GeneralSupport,
                priority: Priority::Normal,
                body: "Fictional shared description".into(),
            },
        )
        .unwrap();
        let response = repo.save(&d, Audience::Staff, &r).unwrap();
        (d, c, r, response)
    }
    #[test]
    fn confirmed_receipt_clears_only_the_exact_pending_form() {
        let (d, c, r, response) = fixture();
        let mut f = Form::new(Kind::Submit, None, &d);
        f.request_id = r.request_id;
        f.pending = Some(r.clone());
        let mut state = State {
            context: Some(c),
            form: Some(f),
            ..Default::default()
        };
        let mut bad = response.clone();
        bad.receipt.as_mut().unwrap().id = Uuid::new_v4();
        assert!(state.accept(bad, &d).is_err());
        assert!(state.editing());
        assert!(!state.accept(response, &d).unwrap());
        assert!(!state.editing());
        assert_eq!(state.selected, Some(r.request_id));
    }
    #[test]
    fn uncertain_clear_retains_frozen_request_but_revoked_scope_removes_draft() {
        let (d, c, r, response) = fixture();
        let mut f = Form::new(Kind::Submit, None, &d);
        f.request_id = r.request_id;
        f.pending = Some(r.clone());
        f.body = "Retained shared draft".into();
        let mut state = State {
            context: Some(c),
            form: Some(f),
            ..Default::default()
        };
        state.clear_context();
        assert_eq!(state.form.as_ref().unwrap().pending.as_ref(), Some(&r));
        state.reject();
        assert!(state.form.as_ref().unwrap().rejected);
        state.form = Some(Form::new(
            Kind::Reply,
            Some(&response.context.requests[0]),
            &d,
        ));
        state.selected = Some(r.request_id);
        let mut empty = response.context;
        empty.requests.clear();
        empty.events.clear();
        assert!(
            state
                .accept(
                    Response {
                        context: empty,
                        receipt: None
                    },
                    &d
                )
                .unwrap()
        );
        assert!(!state.editing());
        assert!(state.selected.is_none());
    }
    #[test]
    fn shared_forms_fit_compact_and_wide_layouts_with_pending_fields_frozen() {
        let (d, _, _, response) = fixture();
        let c = response.context;
        for width in [480.0, 760.0, 1100.0] {
            for dark in [false, true] {
                for kind in [
                    Kind::Submit,
                    Kind::Reply,
                    Kind::Route,
                    Kind::Progress,
                    Kind::Cancel,
                ] {
                    let mut f = Form::new(
                        kind,
                        if kind == Kind::Submit {
                            None
                        } else {
                            Some(&c.requests[0])
                        },
                        &d,
                    );
                    f.body = "Fictional shared review message".into();
                    let mut state = State {
                        context: Some(c.clone()),
                        form: Some(f),
                        ..Default::default()
                    };
                    let ctx = egui::Context::default();
                    ctx.set_visuals(if dark {
                        egui::Visuals::dark()
                    } else {
                        egui::Visuals::light()
                    });
                    let raw = egui::RawInput {
                        screen_rect: Some(egui::Rect::from_min_size(
                            egui::Pos2::ZERO,
                            egui::vec2(width, 1400.0),
                        )),
                        ..Default::default()
                    };
                    let mut output = ctx.run_ui(raw, |root| {
                        egui::CentralPanel::default().show(root, |ui| {
                            ui.set_max_width(width - 20.0);
                            let start = ui.cursor().left();
                            let _ = state.show(ui, &d, false);
                            assert!(
                                ui.min_rect().right() <= start + width + 1.0,
                                "Form exceeded viewport width {width}"
                            );
                        });
                    });
                    output.textures_delta.clear();
                    assert!(state.editing());
                }
            }
        }
    }
}

#[cfg(feature = "qa-screenshot")]
impl HrisApp {
    pub(super) fn prepare_services_preview(&mut self) {
        let name = std::env::var("HRIS_PREVIEW_FORM").unwrap_or_default();
        if !name.starts_with("sr-")
            || self.data.mode != Mode::Demo
            || self.product != crate::desktop::Product::Hris
        {
            return;
        }
        let mut hr = self.data.clone();
        hr.access = crate::access::Access {
            company_id: hr.company.id,
            user_id: Uuid::from_u128(10),
            employee_id: Some(hr.employees[0].id),
            role: Role::Administrator,
            ..Default::default()
        };
        hr.accounts[0].user_id = hr.access.user_id;
        hr.accounts[0].employee_id = hr.access.employee_id;
        hr.accounts[0].role = Role::Administrator;
        let mut repo = services::Demo::default();
        let mut c = repo.context(&hr, Audience::Staff).unwrap();
        let mut ids = vec![];
        if name != "sr-empty" {
            for (index, title) in [
                "Certificate of employment request",
                "Benefits enrollment question",
                "Contact information update",
            ]
            .into_iter()
            .enumerate()
            {
                let id = Uuid::new_v4();
                ids.push(id);
                let r=Request::new(&hr.access,&c,id,Change::Submit{employee_id:hr.employees[index+1].id,title:title.into(),category:Category::ALL[index],priority:if index==1{Priority::High}else{Priority::Normal},body:"Fictional shared request. Please review the supporting facts and let me know the next steps. This request does not automatically edit employment or payroll records.".into()}).unwrap();
                c = repo.save(&hr, Audience::Staff, &r).unwrap().context;
                if index == 0 {
                    let r=Request::new(&hr.access,&c,id,Change::Route{assignee_user_id:Some(Uuid::from_u128(12)),due_on:Some(hr.company.today()-Duration::days(1)),body:"The responsible staff member will review the document request. A final document is prepared through the company's existing process.".into()}).unwrap();
                    c = repo.save(&hr, Audience::Staff, &r).unwrap().context;
                    let r=Request::new(&hr.access,&c,id,Change::Progress{status:Status::InProgress,body:"The request is under review. We will reply here when the next step is ready.".into()}).unwrap();
                    c = repo.save(&hr, Audience::Staff, &r).unwrap().context;
                }
                if index == 2 {
                    let r=Request::new(&hr.access,&c,id,Change::Progress{status:Status::Completed,body:"The requested update was reviewed and handled through the authorized employee-record workflow. This service history retains the recorded outcome.".into()}).unwrap();
                    c = repo.save(&hr, Audience::Staff, &r).unwrap().context;
                }
            }
        }
        self.data = hr.clone();
        if name == "sr-assigned" {
            self.data.access = crate::access::Access {
                company_id: hr.company.id,
                user_id: Uuid::from_u128(12),
                employee_id: Some(hr.employees[3].id),
                role: Role::Manager,
                ..Default::default()
            };
            c = repo.context(&self.data, Audience::Staff).unwrap();
            let own = self.data.access.employee_id;
            self.data.employees.retain(|e| Some(e.id) == own);
        }
        self.data.workspaces = crate::workspaces::Context::legacy(&self.data.access);
        self.page = Page::Services;
        let mut state = State {
            context: Some(c.clone()),
            ..Default::default()
        };
        if !matches!(name.as_str(), "sr-list" | "sr-empty" | "sr-assigned") {
            state.selected = ids.first().copied();
            let kind = match name.as_str() {
                "sr-submit" => Some(Kind::Submit),
                "sr-reply" | "sr-pending" | "sr-rejected" => Some(Kind::Reply),
                "sr-route" => Some(Kind::Route),
                "sr-progress" => Some(Kind::Progress),
                "sr-cancel" => Some(Kind::Cancel),
                _ => None,
            };
            if let Some(kind) = kind {
                let mut form = Form::new(
                    kind,
                    if kind == Kind::Submit {
                        None
                    } else {
                        Some(&c.requests.iter().find(|t| t.id == ids[0]).unwrap())
                    },
                    &self.data,
                );
                form.title = "Employment document for a personal application".into();
                form.body="Please review this fictional request and record the next steps here. This shared message remains visible in the request's history.".into();
                if kind == Kind::Route {
                    form.assignee = None;
                    form.due_on = (hr.company.today() + Duration::days(3)).to_string();
                }
                if name == "sr-pending" || name == "sr-rejected" {
                    form.pending = Some(
                        Request::new(
                            &self.data.access,
                            &c,
                            form.request_id,
                            form.change().unwrap(),
                        )
                        .unwrap(),
                    );
                    form.rejected = name == "sr-rejected";
                }
                state.form = Some(form);
            }
        }
        self.services_state = state;
    }
}
