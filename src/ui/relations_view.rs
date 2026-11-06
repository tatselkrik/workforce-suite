use super::*;
use crate::relations::{
    self, Category, Change, Context, Operation, Priority, Request, Response, Status,
};
#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Create,
    Note,
    Status,
    Assign,
    FollowUp,
    Complete,
}
struct Form {
    kind: Kind,
    case_id: Uuid,
    source_revision: i64,
    employee_id: Option<Uuid>,
    owner_id: Uuid,
    title: String,
    category: Category,
    priority: Priority,
    status: Status,
    body: String,
    due_on: String,
    follow_up_id: Uuid,
    pending: Option<Request>,
    rejected: bool,
}
impl Form {
    fn new(kind: Kind, c: Option<&relations::Case>, data: &Snapshot) -> Self {
        Self {
            kind,
            case_id: c.map(|v| v.id).unwrap_or_else(Uuid::new_v4),
            source_revision: c.map(|v| v.revision).unwrap_or(0),
            employee_id: None,
            owner_id: c.map(|v| v.owner_id).unwrap_or(data.access.user_id),
            title: String::new(),
            category: Category::WorkplaceConcern,
            priority: Priority::Normal,
            status: Status::InReview,
            body: String::new(),
            due_on: data.company.today().format("%Y-%m-%d").to_string(),
            follow_up_id: Uuid::new_v4(),
            pending: None,
            rejected: false,
        }
    }
    fn change(&self) -> anyhow::Result<Change> {
        let body = self.body.trim().to_owned();
        Ok(match self.kind {
            Kind::Create => Change::Create {
                employee_id: self
                    .employee_id
                    .ok_or_else(|| anyhow::anyhow!("Choose an employee."))?,
                owner_id: self.owner_id,
                title: self.title.trim().into(),
                category: self.category,
                priority: self.priority,
                body,
            },
            Kind::Note => Change::Note { body },
            Kind::Status => Change::Status {
                status: self.status,
                body,
            },
            Kind::Assign => Change::Assign {
                owner_id: self.owner_id,
                body,
            },
            Kind::FollowUp => Change::FollowUp {
                follow_up_id: self.follow_up_id,
                due_on: NaiveDate::parse_from_str(&self.due_on, "%Y-%m-%d")
                    .map_err(|_| anyhow::anyhow!("Use a real due date in YYYY-MM-DD format."))?,
                body,
            },
            Kind::Complete => Change::Complete {
                follow_up_id: self.follow_up_id,
                body,
            },
        })
    }
}
#[derive(Clone, Copy, Default)]
struct FollowUpSummary {
    open: usize,
    overdue: usize,
    today: usize,
    next_due: Option<NaiveDate>,
}

fn follow_up_summaries(
    context: &Context,
    today: NaiveDate,
) -> std::collections::HashMap<Uuid, FollowUpSummary> {
    let cases = context
        .cases
        .iter()
        .filter(|case| case.company_id == context.company_id)
        .map(|case| case.id)
        .collect::<std::collections::HashSet<_>>();
    let mut summaries = std::collections::HashMap::<Uuid, FollowUpSummary>::new();
    for task in context.follow_ups.iter().filter(|task| {
        task.company_id == context.company_id
            && task.completed_at.is_none()
            && cases.contains(&task.case_id)
    }) {
        let summary = summaries.entry(task.case_id).or_default();
        summary.open += 1;
        summary.overdue += usize::from(task.due_on < today);
        summary.today += usize::from(task.due_on == today);
        summary.next_due = Some(
            summary
                .next_due
                .map_or(task.due_on, |date| date.min(task.due_on)),
        );
    }
    summaries
}

#[derive(Clone, Copy, Default, PartialEq, Eq)]
enum FollowUpFilter {
    #[default]
    All,
    Open,
    Overdue,
    Today,
}

impl FollowUpFilter {
    fn label(self) -> &'static str {
        match self {
            Self::All => "Any follow-up status",
            Self::Open => "Open follow-ups",
            Self::Overdue => "Overdue",
            Self::Today => "Due today",
        }
    }

    fn matches(self, summary: FollowUpSummary) -> bool {
        match self {
            Self::All => true,
            Self::Open => summary.open > 0,
            Self::Overdue => summary.overdue > 0,
            Self::Today => summary.today > 0,
        }
    }
}

#[derive(Default)]
pub(super) struct State {
    context: Option<Context>,
    selected: Option<Uuid>,
    form: Option<Form>,
    search: String,
    status_filter: Option<Status>,
    follow_up_filter: FollowUpFilter,
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
            f.rejected = true
        }
    }
    pub fn accept(&mut self, response: Response, data: &Snapshot) -> anyhow::Result<bool> {
        response.context.validate(&data.access)?;
        let mut revoked = false;
        if let Some(r) = response.receipt {
            let f = self
                .form
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("No case save is pending."))?;
            r.validate(
                f.pending
                    .as_ref()
                    .ok_or_else(|| anyhow::anyhow!("No matching case request."))?,
            )?;
            anyhow::ensure!(
                response.context.events.iter().any(|e| e.id == r.id
                    && e.actor_id == r.actor_id
                    && e.case_id == r.case_id
                    && e.revision == r.revision),
                "The confirmed change is missing from case history."
            );
            self.selected = Some(r.case_id);
            self.form = None;
        }
        if self.form.as_ref().is_some_and(|f| {
            f.kind != Kind::Create && !response.context.cases.iter().any(|c| c.id == f.case_id)
        }) {
            self.form = None;
            revoked = true;
        }
        if self
            .selected
            .is_some_and(|id| !response.context.cases.iter().any(|c| c.id == id))
        {
            self.selected = None;
            revoked = true;
        }
        self.context = Some(response.context);
        self.error = None;
        Ok(revoked)
    }
    fn show(&mut self, ui: &mut Ui, data: &Snapshot, busy: bool) -> Option<Operation> {
        ui.label(
            RichText::new(
                "Administrators oversee all cases. HR Generalists see only their assigned cases.",
            )
            .small()
            .color(muted(ui)),
        );
        let mut output = None;
        ui.horizontal_wrapped(|ui| {
            if ui
                .add_enabled(
                    !busy,
                    egui::Button::new(if self.context.is_some() {
                        "Refresh cases"
                    } else {
                        "Load assigned cases"
                    }),
                )
                .clicked()
            {
                output = Some(Operation::Refresh)
            }
            if self.context.is_some()
                && self.form.is_none()
                && ui
                    .add_enabled(!busy, egui::Button::new("New case"))
                    .clicked()
            {
                self.form = Some(Form::new(Kind::Create, None, data));
                self.error = None;
            }
        });
        if let Some(error) = &self.error {
            ui.colored_label(Color32::from_rgb(190, 70, 55), error);
        }
        let context = self.context.take();
        if self.form.is_some() {
            if let Some(op) = self.show_form(ui, data, context.as_ref(), busy) {
                output = Some(op)
            }
        } else if let Some(c) = &context {
            if let Some(case) = self
                .selected
                .and_then(|id| c.cases.iter().find(|v| v.id == id))
            {
                if ui.button("Back to cases").clicked() {
                    self.selected = None;
                }
                ui.add_space(8.0);
                ui.label(RichText::new(&case.code).small());
                ui.heading(&case.title);
                ui.label(format!(
                    "{} · {} · {} priority",
                    case.status.label(),
                    case.category.label(),
                    case.priority.label()
                ));
                ui.label(format!(
                    "Employee: {}",
                    data.employees
                        .iter()
                        .find(|e| e.id == case.employee_id)
                        .map(|e| format!("{} · {}", e.employee_code, e.full_name))
                        .unwrap_or_else(|| "Retained employee record".into())
                ));
                ui.label(format!(
                    "Handler: {}",
                    c.handlers
                        .iter()
                        .find(|h| h.user_id == case.owner_id)
                        .map(|h| h.full_name.as_str())
                        .unwrap_or("Former/inactive handler — Administrator reassignment needed")
                ));
                ui.label(format!(
                    "Revision {} · Updated {}",
                    case.revision,
                    case.updated_at
                        .with_timezone(&data.company.tz())
                        .format("%b %d, %Y %H:%M")
                ));
                ui.add_space(10.0);
                ui.horizontal_wrapped(|ui| {
                    if ui
                        .add_enabled(
                            !busy && case.status != Status::Closed,
                            egui::Button::new("Add note"),
                        )
                        .clicked()
                    {
                        self.form = Some(Form::new(Kind::Note, Some(case), data));
                    }
                    if ui
                        .add_enabled(
                            !busy && (case.status != Status::Closed || c.can_assign),
                            egui::Button::new("Change status"),
                        )
                        .clicked()
                    {
                        let mut f = Form::new(Kind::Status, Some(case), data);
                        f.status = Status::ALL
                            .into_iter()
                            .find(|s| case.status.permits(*s, c.can_assign))
                            .unwrap_or(Status::InReview);
                        self.form = Some(f);
                    }
                    if ui
                        .add_enabled(
                            !busy && !matches!(case.status, Status::Resolved | Status::Closed),
                            egui::Button::new("Add follow-up"),
                        )
                        .clicked()
                    {
                        self.form = Some(Form::new(Kind::FollowUp, Some(case), data));
                    }
                    if c.can_assign
                        && ui
                            .add_enabled(
                                !busy && case.status != Status::Closed,
                                egui::Button::new("Assign handler"),
                            )
                            .clicked()
                    {
                        self.form = Some(Form::new(Kind::Assign, Some(case), data));
                    }
                });
                ui.add_space(14.0);
                ui.heading("Follow-ups");
                let mut any = false;
                for f in c.follow_ups.iter().filter(|f| f.case_id == case.id) {
                    any = true;
                    Frame::group(ui.style()).show(ui, |ui| {
                        ui.label(RichText::new(&f.task).strong());
                        let due = format!("Due {}", f.due_on.format("%b %d, %Y"));
                        if let Some(at) = f.completed_at {
                            ui.label(format!(
                                "Completed {} · {}",
                                at.with_timezone(&data.company.tz()).format("%b %d, %Y"),
                                due
                            ));
                            if let Some(note) = &f.completion_note {
                                ui.label(note);
                            }
                        } else {
                            ui.label(if f.due_on < data.company.today() {
                                format!("Overdue · {due}")
                            } else {
                                due
                            });
                            if ui
                                .add_enabled(
                                    !busy && case.status != Status::Closed,
                                    egui::Button::new("Record completion"),
                                )
                                .clicked()
                            {
                                let mut form = Form::new(Kind::Complete, Some(case), data);
                                form.follow_up_id = f.id;
                                self.form = Some(form);
                            }
                        }
                    });
                    ui.add_space(6.0);
                }
                if !any {
                    ui.label("No follow-ups recorded.");
                }
                ui.add_space(12.0);
                ui.heading("Case history");
                for event in c.events.iter().filter(|e| e.case_id == case.id).rev() {
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
                        if event.kind == "status" {
                            ui.label(format!(
                                "{} to {}",
                                event
                                    .previous_state
                                    .as_ref()
                                    .map(|s| s.status.label())
                                    .unwrap_or("New"),
                                event.current_state.status.label()
                            ));
                        }
                        if event.kind == "assign" {
                            ui.label(format!(
                                "Assigned to {}",
                                c.handlers
                                    .iter()
                                    .find(|h| h.user_id == event.current_state.owner_id)
                                    .map(|h| h.full_name.as_str())
                                    .unwrap_or("a retained former handler")
                            ));
                        }
                        ui.label(&event.body);
                    });
                    ui.add_space(6.0);
                }
            } else {
                let today = data.company.today();
                let summaries = follow_up_summaries(c, today);
                let overdue = summaries
                    .values()
                    .map(|summary| summary.overdue)
                    .sum::<usize>();
                ui.label(format!(
                    "Visible cases · {}   Overdue follow-ups · {overdue}",
                    c.cases.len()
                ));
                ui.horizontal_wrapped(|ui| {
                    ui.add(
                        egui::TextEdit::singleline(&mut self.search)
                            .hint_text("Case or employee")
                            .desired_width(205.0),
                    );
                    egui::ComboBox::from_id_salt("relation-filter")
                        .selected_text(self.status_filter.map_or("All statuses", Status::label))
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut self.status_filter, None, "All statuses");
                            for status in Status::ALL {
                                ui.selectable_value(
                                    &mut self.status_filter,
                                    Some(status),
                                    status.label(),
                                );
                            }
                        });
                    egui::ComboBox::from_id_salt("relation-follow-up-filter")
                        .selected_text(self.follow_up_filter.label())
                        .show_ui(ui, |ui| {
                            for filter in [
                                FollowUpFilter::All,
                                FollowUpFilter::Open,
                                FollowUpFilter::Overdue,
                                FollowUpFilter::Today,
                            ] {
                                ui.selectable_value(
                                    &mut self.follow_up_filter,
                                    filter,
                                    filter.label(),
                                );
                            }
                        });
                    if (!self.search.is_empty()
                        || self.status_filter.is_some()
                        || self.follow_up_filter != FollowUpFilter::All)
                        && ui.button("Clear filters").clicked()
                    {
                        self.search.clear();
                        self.status_filter = None;
                        self.follow_up_filter = FollowUpFilter::All;
                    }
                });
                let query = self.search.trim().to_lowercase();
                let mut rows = c
                    .cases
                    .iter()
                    .filter(|case| case.company_id == c.company_id)
                    .filter(|case| {
                        self.status_filter
                            .is_none_or(|status| status == case.status)
                    })
                    .filter_map(|case| {
                        let summary = summaries.get(&case.id).copied().unwrap_or_default();
                        if !self.follow_up_filter.matches(summary) {
                            return None;
                        }
                        let name = data
                            .employees
                            .iter()
                            .find(|employee| {
                                employee.id == case.employee_id
                                    && employee.company_id == c.company_id
                            })
                            .map(|employee| {
                                format!("{} {}", employee.employee_code, employee.full_name)
                            })
                            .unwrap_or_else(|| "Retained employee".into());
                        (query.is_empty()
                            || format!("{} {} {}", case.code, case.title, name)
                                .to_lowercase()
                                .contains(&query))
                        .then_some((case, name, summary))
                    })
                    .collect::<Vec<_>>();
                if self.follow_up_filter != FollowUpFilter::All {
                    rows.sort_by_key(|(case, _, summary)| (summary.next_due, case.id));
                }
                if rows.len() != c.cases.len() {
                    ui.small(format!("Showing {} of {}", rows.len(), c.cases.len()));
                }
                if rows.is_empty() {
                    ui.label("No assigned cases match this view.");
                }
                for (case, name, summary) in rows {
                    Frame::group(ui.style()).show(ui, |ui| {
                        if ui
                            .add(egui::Button::new(RichText::new(&case.title).strong()).wrap())
                            .clicked()
                        {
                            self.selected = Some(case.id);
                        }
                        ui.label(format!(
                            "{} · {} · {} priority",
                            name,
                            case.status.label(),
                            case.priority.label()
                        ));
                        ui.small(&case.code);
                        if let Some(next) = summary.next_due {
                            ui.small(format!(
                                "Open follow-ups · {} · Next due {}",
                                summary.open,
                                next.format("%b %d, %Y")
                            ));
                        }
                        if summary.overdue > 0 || summary.today > 0 {
                            ui.horizontal_wrapped(|ui| {
                                if summary.overdue > 0 {
                                    badge(ui, &format!("Overdue · {}", summary.overdue), false);
                                }
                                if summary.today > 0 {
                                    badge(ui, &format!("Due today · {}", summary.today), false);
                                }
                            });
                        }
                    });
                    ui.add_space(6.0);
                }
            }
        } else {
            ui.label("Load current assigned cases to begin. Confidential case details stay in this workspace.");
        }
        self.context = context;
        output
    }
    fn show_form(
        &mut self,
        ui: &mut Ui,
        data: &Snapshot,
        context: Option<&Context>,
        busy: bool,
    ) -> Option<Operation> {
        let mut f = self.form.take().unwrap();
        let mut output = None;
        let mut cancel = false;
        ui.add_space(12.0);
        ui.heading(match f.kind {
            Kind::Create => "New confidential case",
            Kind::Note => "Add case note",
            Kind::Status => "Change case status",
            Kind::Assign => "Assign case handler",
            Kind::FollowUp => "New follow-up",
            Kind::Complete => "Complete follow-up",
        });
        if f.kind != Kind::Create {
            if let Some(c) = context {
                if let Ok(case) = c.case(f.case_id) {
                    Frame::group(ui.style()).show(ui, |ui| {
                        ui.label(RichText::new(&case.title).strong());
                        ui.label(format!(
                            "Employee: {}",
                            data.employees
                                .iter()
                                .find(|e| e.id == case.employee_id)
                                .map(|e| format!("{} · {}", e.employee_code, e.full_name))
                                .unwrap_or_else(|| "Retained employee".into())
                        ));
                        ui.label(format!(
                            "Current status: {} · Revision {}",
                            case.status.label(),
                            case.revision
                        ));
                        if f.kind == Kind::Complete {
                            if let Some(task) = c
                                .follow_ups
                                .iter()
                                .find(|v| v.id == f.follow_up_id && v.case_id == case.id)
                            {
                                ui.label(format!("Follow-up: {}", task.task));
                                ui.label(format!("Due {}", task.due_on.format("%b %d, %Y")));
                            }
                        }
                    });
                    ui.add_space(8.0);
                }
            }
        }
        ui.add_enabled_ui(!busy && f.pending.is_none(), |ui| {
            if f.kind == Kind::Create {
                ui.label("Employee");
                egui::ComboBox::from_id_salt("relations-employee")
                    .selected_text(
                        f.employee_id
                            .and_then(|id| data.employees.iter().find(|e| e.id == id))
                            .map(|e| e.full_name.as_str())
                            .unwrap_or("Choose an employee"),
                    )
                    .width(ui.available_width().min(430.0))
                    .show_ui(ui, |ui| {
                        for e in &data.employees {
                            ui.selectable_value(
                                &mut f.employee_id,
                                Some(e.id),
                                format!("{} · {}", e.employee_code, e.full_name),
                            );
                        }
                    });
                ui.label("Case title");
                ui.add(
                    egui::TextEdit::singleline(&mut f.title)
                        .id(egui::Id::unique("relations-title"))
                        .desired_width(ui.available_width()),
                );
                ui.horizontal_wrapped(|ui| {
                    egui::ComboBox::from_id_salt("relations-category")
                        .selected_text(f.category.label())
                        .show_ui(ui, |ui| {
                            for cat in Category::ALL {
                                ui.selectable_value(&mut f.category, cat, cat.label());
                            }
                        });
                    egui::ComboBox::from_id_salt("relations-priority")
                        .selected_text(f.priority.label())
                        .show_ui(ui, |ui| {
                            for p in [Priority::Normal, Priority::High] {
                                ui.selectable_value(&mut f.priority, p, p.label());
                            }
                        });
                });
            }
            if matches!(f.kind, Kind::Create | Kind::Assign) {
                if let Some(c) = context {
                    if c.can_assign {
                        ui.label("Assigned HR handler");
                        egui::ComboBox::from_id_salt("relations-handler")
                            .selected_text(
                                c.handlers
                                    .iter()
                                    .find(|h| h.user_id == f.owner_id)
                                    .map(|h| h.full_name.as_str())
                                    .unwrap_or("Choose a handler"),
                            )
                            .show_ui(ui, |ui| {
                                for h in &c.handlers {
                                    ui.selectable_value(&mut f.owner_id, h.user_id, &h.full_name);
                                }
                            });
                    } else {
                        ui.label("This case will be assigned to you.");
                    }
                }
            }
            if f.kind == Kind::Status {
                if let Some(c) = context {
                    if let Ok(case) = c.case(f.case_id) {
                        ui.label("New status");
                        egui::ComboBox::from_id_salt("relations-status")
                            .selected_text(f.status.label())
                            .show_ui(ui, |ui| {
                                for s in Status::ALL {
                                    if case.status.permits(s, c.can_assign) {
                                        ui.selectable_value(&mut f.status, s, s.label());
                                    }
                                }
                            });
                    }
                }
            }
            if f.kind == Kind::FollowUp {
                ui.label("Due date (YYYY-MM-DD)");
                ui.add(egui::TextEdit::singleline(&mut f.due_on).desired_width(160.0));
            }
            ui.label(match f.kind {
                Kind::Create => "Initial confidential description",
                Kind::FollowUp => "Follow-up task",
                Kind::Complete => "Completion note",
                Kind::Status | Kind::Assign => "Reason and review notes",
                Kind::Note => "Confidential note",
            });
            ui.add(
                egui::TextEdit::multiline(&mut f.body)
                    .id(egui::Id::unique("relations-body"))
                    .desired_rows(5)
                    .desired_width(ui.available_width()),
            );
        });
        if f.pending.is_some() {
            ui.label(if f.rejected{"The server rejected this change. Refresh, then explicitly review the current version."}else{"This exact save is awaiting confirmation. Retry it without creating a replacement."});
        }
        let stale = context.is_some_and(|c| {
            f.kind != Kind::Create
                && c.case(f.case_id)
                    .is_ok_and(|case| case.revision != f.source_revision)
        });
        if stale && f.pending.is_none() {
            ui.label("The case changed while this draft was open. Review its current version before saving.");
        }
        ui.horizontal_wrapped(|ui| {
            if ui
                .add_enabled(
                    !busy
                        && !f.rejected
                        && (context.is_some() || f.pending.is_some())
                        && (!stale || f.pending.is_some()),
                    egui::Button::new(if f.pending.is_some() {
                        "Retry same save"
                    } else {
                        "Save case change"
                    }),
                )
                .clicked()
            {
                let request = if let Some(r) = &f.pending {
                    r.binding(&data.access).map(|_| r.clone())
                } else {
                    context
                        .ok_or_else(|| anyhow::anyhow!("Refresh assigned cases first."))
                        .and_then(|c| Request::new(&data.access, c, f.case_id, f.change()?))
                };
                match request {
                    Ok(r) => {
                        f.pending = Some(r.clone());
                        output = Some(Operation::Save(Box::new(r)));
                        self.error = None;
                    }
                    Err(e) => self.error = Some(e.to_string()),
                }
            }
            if !busy
                && (stale || f.rejected)
                && context.is_some()
                && ui.button("Review current version").clicked()
            {
                if let Some(c) = context {
                    if let Ok(case) = c.case(f.case_id) {
                        f.source_revision = case.revision;
                        f.pending = None;
                        f.rejected = false;
                        self.error = None;
                    } else if f.kind == Kind::Create && f.rejected {
                        f.pending = None;
                        f.rejected = false;
                    }
                }
            }
            if ui
                .add_enabled(
                    !busy && (f.pending.is_none() || f.rejected),
                    egui::Button::new("Cancel draft"),
                )
                .clicked()
            {
                cancel = true;
                self.error = None;
            }
        });
        if !cancel {
            self.form = Some(f);
        }
        output
    }
}
impl HrisApp {
    pub(super) fn relations(&mut self, ui: &mut Ui) {
        if !relations::eligible(&self.data.access) {
            self.relations_state = State::default();
            return;
        }
        let mut state = std::mem::take(&mut self.relations_state);
        let op = state.show(ui, &self.data, self.busy);
        self.relations_state = state;
        if let Some(op) = op {
            self.send(Command::Relations(op));
        }
    }
}
#[cfg(feature = "qa-screenshot")]
impl HrisApp {
    pub(super) fn prepare_relations_preview(&mut self) {
        let name = std::env::var("HRIS_PREVIEW_FORM").unwrap_or_default();
        if !name.starts_with("er-")
            || self.data.mode != Mode::Demo
            || !relations::eligible(&self.data.access)
        {
            return;
        }
        if let Some(account) = self.data.accounts.first_mut() {
            account.user_id = self.data.access.user_id;
            account.employee_id = self.data.access.employee_id;
            account.role = self.data.access.role;
        }
        let mut repo = relations::Demo::default();
        let mut context = repo
            .execute(Operation::Refresh, &self.data)
            .unwrap()
            .context;
        let mut ids = Vec::new();
        for (index, title) in [
            "Workplace communication concern",
            "Follow-up after a schedule discussion",
            "Completed workplace review",
        ]
        .into_iter()
        .enumerate()
        {
            let id = Uuid::new_v4();
            ids.push(id);
            let r=Request::new(&self.data.access,&context,id,Change::Create{employee_id:self.data.employees[index+1].id,owner_id:self.data.access.user_id,title:title.into(),category:if index==1{Category::Grievance}else{Category::WorkplaceConcern},priority:if index==1{Priority::High}else{Priority::Normal},body:"Fictional demonstration only. HR recorded the concern and arranged a confidential review. The case does not change employee status, attendance, pay or a disciplinary outcome.".into()}).unwrap();
            context = repo
                .execute(Operation::Save(Box::new(r)), &self.data)
                .unwrap()
                .context;
            if index < 2 {
                let r=Request::new(&self.data.access,&context,id,Change::FollowUp{follow_up_id:Uuid::new_v4(),due_on:self.data.company.today()+Duration::days(if index==0{3}else{-2}),body:"Privately check the agreed actions with the people involved and record the outcome.".into()}).unwrap();
                context = repo
                    .execute(Operation::Save(Box::new(r)), &self.data)
                    .unwrap()
                    .context;
            } else {
                for status in [Status::Resolved, Status::Closed] {
                    let r=Request::new(&self.data.access,&context,id,Change::Status{status,body:"Fictional review completed; the outcome and next steps were recorded.".into()}).unwrap();
                    context = repo
                        .execute(Operation::Save(Box::new(r)), &self.data)
                        .unwrap()
                        .context;
                }
            }
        }
        self.page = Page::Employees;
        self.employee_view = EmployeeView::Relations;
        self.relations_state = State::default();
        if name != "er-list" {
            let selected = if name == "er-closed" { ids[2] } else { ids[0] };
            self.relations_state.selected = Some(selected);
            let case = context.case(selected).unwrap();
            let kind = match name.as_str() {
                "er-new" => Some(Kind::Create),
                "er-note" => Some(Kind::Note),
                "er-follow-up" => Some(Kind::FollowUp),
                "er-complete" => Some(Kind::Complete),
                "er-status" => Some(Kind::Status),
                "er-assign" => Some(Kind::Assign),
                _ => None,
            };
            if let Some(kind) = kind {
                let mut form = Form::new(
                    kind,
                    if kind == Kind::Create {
                        None
                    } else {
                        Some(case)
                    },
                    &self.data,
                );
                form.employee_id = Some(self.data.employees[2].id);
                form.title = "Fictional workplace support request".into();
                form.body="Fictional confidential review note. Describe the relevant facts, next steps and the reason for this update.".into();
                if kind == Kind::Complete {
                    form.follow_up_id = context
                        .follow_ups
                        .iter()
                        .find(|f| f.case_id == selected)
                        .unwrap()
                        .id;
                }
                self.relations_state.form = Some(form);
            }
        }
        self.relations_state.context = Some(context);
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    fn fixture() -> (Snapshot, Context, Request, Response) {
        let d = DemoStore::new().data;
        let mut repo = relations::Demo::default();
        let context = repo.execute(Operation::Refresh, &d).unwrap().context;
        let r = Request::new(
            &d.access,
            &context,
            Uuid::new_v4(),
            Change::Create {
                employee_id: d.employees[1].id,
                owner_id: d.access.user_id,
                title: "Fictional pending case".into(),
                category: Category::Other,
                priority: Priority::Normal,
                body: "Fictional initial account".into(),
            },
        )
        .unwrap();
        let saved = repo
            .execute(Operation::Save(Box::new(r.clone())), &d)
            .unwrap();
        (d, context, r, saved)
    }
    #[test]
    fn deadlines_count_only_open_follow_ups_on_visible_cases() {
        let (data, _, _, saved) = fixture();
        let mut context = saved.context;
        let case = &context.cases[0];
        let case_id = case.id;
        let today = data.company.today();
        let overdue = relations::FollowUp {
            id: Uuid::new_v4(),
            case_id,
            company_id: case.company_id,
            task: "Fictional follow-up".into(),
            due_on: today - Duration::days(1),
            created_by: data.access.user_id,
            created_at: Utc::now(),
            completed_by: None,
            completed_at: None,
            completion_note: None,
        };
        context.follow_ups.extend([
            overdue.clone(),
            relations::FollowUp {
                id: Uuid::new_v4(),
                due_on: today,
                ..overdue.clone()
            },
            relations::FollowUp {
                id: Uuid::new_v4(),
                due_on: today - Duration::days(2),
                completed_at: Some(Utc::now()),
                ..overdue.clone()
            },
            relations::FollowUp {
                id: Uuid::new_v4(),
                company_id: Uuid::new_v4(),
                ..overdue.clone()
            },
            relations::FollowUp {
                id: Uuid::new_v4(),
                case_id: Uuid::new_v4(),
                ..overdue
            },
        ]);

        let summaries = follow_up_summaries(&context, today);
        assert_eq!(summaries.len(), 1);
        let summary = summaries[&case_id];
        assert_eq!((summary.open, summary.overdue, summary.today), (2, 1, 1));
        assert_eq!(summary.next_due, Some(today - Duration::days(1)));
        for filter in [
            FollowUpFilter::Open,
            FollowUpFilter::Overdue,
            FollowUpFilter::Today,
        ] {
            assert!(filter.matches(summary));
            assert!(!filter.matches(FollowUpSummary::default()));
        }
        assert!(FollowUpFilter::All.matches(FollowUpSummary::default()));
    }
    #[test]
    fn uncertain_create_preserves_the_exact_request_until_receipt() {
        let (d, c, r, saved) = fixture();
        let mut f = Form::new(Kind::Create, None, &d);
        f.case_id = r.case_id;
        f.pending = Some(r.clone());
        let mut state = State {
            context: Some(c),
            form: Some(f),
            ..Default::default()
        };
        state.clear_context();
        assert_eq!(state.form.as_ref().unwrap().pending.as_ref(), Some(&r));
        state
            .accept(
                Response {
                    context: saved.context.clone(),
                    receipt: None,
                },
                &d,
            )
            .unwrap();
        assert_eq!(state.form.as_ref().unwrap().pending.as_ref(), Some(&r));
        state.accept(saved, &d).unwrap();
        assert!(state.form.is_none());
        assert_eq!(state.selected, Some(r.case_id));
    }
    #[test]
    fn revoked_assignment_clears_case_contents_and_affected_drafts() {
        let (mut d, _, _, saved) = fixture();
        d.access.role = Role::HrGeneralist;
        let mut c = saved.context;
        c.can_assign = false;
        c.validate(&d.access).unwrap();
        let case = &c.cases[0];
        let f = Form::new(Kind::Note, Some(case), &d);
        let mut state = State {
            selected: Some(case.id),
            form: Some(f),
            context: Some(c.clone()),
            ..Default::default()
        };
        c.cases.clear();
        c.events.clear();
        c.follow_ups.clear();
        assert!(
            state
                .accept(
                    Response {
                        context: c,
                        receipt: None
                    },
                    &d
                )
                .unwrap()
        );
        assert!(state.form.is_none() && state.selected.is_none());
    }
    #[test]
    fn definitive_rejection_keeps_the_draft_for_explicit_review() {
        let (d, c, r, _) = fixture();
        let mut f = Form::new(Kind::Create, None, &d);
        f.pending = Some(r.clone());
        f.body = "Preserve this private draft".into();
        let mut state = State {
            context: Some(c),
            form: Some(f),
            ..Default::default()
        };
        state.reject();
        state.clear_context();
        let f = state.form.as_ref().unwrap();
        assert!(f.rejected);
        assert_eq!(f.body, "Preserve this private draft");
        assert_eq!(f.pending.as_ref(), Some(&r));
    }
    #[test]
    fn case_forms_fit_compact_and_wide_light_dark_views() {
        let (d, c, _, saved) = fixture();
        for width in [480.0, 760.0, 1100.0] {
            for dark in [false, true] {
                for kind in [
                    Kind::Create,
                    Kind::Note,
                    Kind::Status,
                    Kind::Assign,
                    Kind::FollowUp,
                    Kind::Complete,
                ] {
                    let ctx = egui::Context::default();
                    ctx.set_visuals(if dark {
                        egui::Visuals::dark()
                    } else {
                        egui::Visuals::light()
                    });
                    let mut form = Form::new(
                        kind,
                        if kind == Kind::Create {
                            None
                        } else {
                            Some(&saved.context.cases[0])
                        },
                        &d,
                    );
                    form.body = "Fictional note to check layout and retained input.".into();
                    let mut state = State {
                        context: Some(if kind == Kind::Create {
                            c.clone()
                        } else {
                            saved.context.clone()
                        }),
                        form: Some(form),
                        ..Default::default()
                    };
                    let mut bounds = egui::Rect::NOTHING;
                    let mut output = ctx.run_ui(
                        egui::RawInput {
                            screen_rect: Some(egui::Rect::from_min_size(
                                egui::Pos2::ZERO,
                                egui::vec2(width, 1200.0),
                            )),
                            ..Default::default()
                        },
                        |ui| {
                            egui::CentralPanel::default().show(ui, |ui| {
                                state.show(ui, &d, false);
                                bounds = ui.min_rect();
                            });
                        },
                    );
                    output.textures_delta.clear();
                    assert!(
                        bounds.right() <= width + 1.0,
                        "Case form overflow: {width} dark={dark}"
                    );
                }
            }
        }
    }
}
