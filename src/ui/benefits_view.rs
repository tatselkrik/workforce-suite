use super::*;
use crate::benefits::{
    self, Category, Change, Context, Dependent, Eligibility, Enrollment, Operation, Plan,
    Relationship, Request, Response, State as Saved, Status,
};
#[derive(Clone, Copy, PartialEq, Eq)]
enum Tab {
    Plans,
    Enrollments,
}
impl Default for Tab {
    fn default() -> Self {
        Self::Plans
    }
}
#[derive(Clone)]
struct DependentDraft {
    value: Dependent,
    birth: String,
    from: String,
    to: String,
}
impl DependentDraft {
    fn new(v: Dependent) -> Self {
        Self {
            birth: v.birth_date.map(|d| d.to_string()).unwrap_or_default(),
            from: v.coverage_from.to_string(),
            to: v.coverage_to.map(|d| d.to_string()).unwrap_or_default(),
            value: v,
        }
    }
    fn value(&self) -> anyhow::Result<Dependent> {
        let mut d = self.value.clone();
        d.birth_date = optional_date(&self.birth)?;
        d.coverage_from = parse_date(&self.from)?;
        d.coverage_to = optional_date(&self.to)?;
        Ok(d)
    }
}
#[derive(Clone)]
enum Draft {
    Plan(Plan),
    Enrollment {
        value: Box<Enrollment>,
        from: String,
        to: String,
        dependents: Vec<DependentDraft>,
        ending: bool,
    },
}
struct Form {
    id: Uuid,
    revision: i64,
    draft: Draft,
    body: String,
    reviewed: bool,
    pending: Option<Request>,
    rejected: bool,
}
impl Form {
    fn plan(id: Uuid, revision: i64, p: Plan) -> Self {
        Self {
            id,
            revision,
            draft: Draft::Plan(p),
            body: String::new(),
            reviewed: false,
            pending: None,
            rejected: false,
        }
    }
    fn enrollment(id: Uuid, revision: i64, e: Enrollment, ending: bool, today: NaiveDate) -> Self {
        let to = if ending {
            e.coverage_to.unwrap_or(today).min(today).to_string()
        } else {
            e.coverage_to.map(|v| v.to_string()).unwrap_or_default()
        };
        let from = e.coverage_from.to_string();
        let dependents = e
            .dependents
            .iter()
            .cloned()
            .map(DependentDraft::new)
            .collect();
        Self {
            id,
            revision,
            draft: Draft::Enrollment {
                value: Box::new(e),
                from,
                to,
                dependents,
                ending,
            },
            body: String::new(),
            reviewed: false,
            pending: None,
            rejected: false,
        }
    }
    fn change(&self) -> anyhow::Result<Change> {
        Ok(match &self.draft {
            Draft::Plan(p) => Change::Plan(p.clone()),
            Draft::Enrollment {
                value,
                from,
                to,
                dependents,
                ending,
            } => {
                let mut e = value.as_ref().clone();
                e.coverage_from = parse_date(from)?;
                e.coverage_to = optional_date(to)?;
                if *ending {
                    let end = e
                        .coverage_to
                        .ok_or_else(|| anyhow::anyhow!("Enter the final coverage date."))?;
                    e.status = Status::Ended;
                    for d in &mut e.dependents {
                        d.coverage_to = Some(d.coverage_to.unwrap_or(end).min(end));
                    }
                } else {
                    e.dependents = dependents
                        .iter()
                        .map(DependentDraft::value)
                        .collect::<anyhow::Result<_>>()?;
                }
                Change::Enrollment(e)
            }
        })
    }
    fn request(&self, c: &Context, data: &Snapshot) -> anyhow::Result<Request> {
        anyhow::ensure!(
            self.reviewed,
            "Review the saved sources and confirm this benefits change."
        );
        let r = Request {
            id: Uuid::new_v4(),
            company_id: data.access.company_id,
            actor_id: data.access.user_id,
            subject_id: self.id,
            expected_revision: self.revision,
            body: self.body.trim().into(),
            change: self.change()?,
        };
        r.validate(c, &data.access)?;
        Ok(r)
    }
    fn review_current(&mut self, c: &Context) -> anyhow::Result<()> {
        match &mut self.draft {
            Draft::Plan(_) => {
                self.revision = c
                    .plans
                    .iter()
                    .find(|v| v.id == self.id)
                    .map(|v| v.revision)
                    .unwrap_or(0)
            }
            Draft::Enrollment { value, ending, .. } => {
                self.revision = c
                    .enrollments
                    .iter()
                    .find(|v| v.id == self.id)
                    .map(|v| v.revision)
                    .unwrap_or(0);
                if *ending {
                    *value = Box::new(c.enrollment(self.id)?.data.clone());
                } else {
                    let p = c.plan(value.plan_id)?;
                    value.plan_revision = p.revision;
                    value.plan_snapshot = p.data.clone();
                    value.employee_source = c
                        .employees
                        .iter()
                        .find(|e| e.id == value.employee_source.id)
                        .ok_or_else(|| anyhow::anyhow!("Current employee source is unavailable."))?
                        .clone();
                }
            }
        }
        self.pending = None;
        self.rejected = false;
        self.reviewed = false;
        Ok(())
    }
}
fn parse_date(s: &str) -> anyhow::Result<NaiveDate> {
    NaiveDate::parse_from_str(s, "%Y-%m-%d")
        .map_err(|_| anyhow::anyhow!("Use a real date in YYYY-MM-DD format."))
}
fn optional_date(s: &str) -> anyhow::Result<Option<NaiveDate>> {
    if s.trim().is_empty() {
        Ok(None)
    } else {
        Ok(Some(parse_date(s)?))
    }
}
#[derive(Default)]
pub(super) struct State {
    context: Option<Context>,
    tab: Tab,
    selected: Option<(bool, Uuid)>,
    form: Option<Form>,
    search: String,
    error: Option<String>,
}
pub(super) fn clear_undo(ctx: &egui::Context) {
    clear_edit_history(
        ctx,
        &[
            "benefits-search",
            "benefits-code",
            "benefits-name",
            "benefits-description",
            "benefits-guidance",
            "benefits-body",
            "benefits-basis",
            "benefits-from",
            "benefits-to",
        ],
    );
    for i in 0..25 {
        for key in ["name", "birth", "from", "to"] {
            let id = egui::Id::unique(format!("benefits-dependent-{i}-{key}"));
            if let Some(mut s) = egui::text_edit::TextEditState::load(ctx, id) {
                s.clear_undoer();
                s.store(ctx, id);
            }
        }
    }
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
    pub fn accept(&mut self, r: Response, data: &Snapshot) -> anyhow::Result<bool> {
        r.context.validate(&data.access)?;
        if let Some(receipt) = r.receipt {
            let pending = self
                .form
                .as_ref()
                .and_then(|f| f.pending.as_ref())
                .ok_or_else(|| anyhow::anyhow!("No matching benefits save is pending."))?;
            receipt.validate(pending)?;
            anyhow::ensure!(
                r.context.events.iter().any(|e| e.id == receipt.id
                    && e.actor_id == receipt.actor_id
                    && e.current.id() == receipt.subject_id
                    && e.current.revision() == receipt.revision
                    && e.body == pending.body
                    && e.created_at == receipt.applied_at
                    && match (&pending.change, &e.current) {
                        (Change::Plan(expected), Saved::Plan(saved)) => expected == &saved.data,
                        (Change::Enrollment(expected), Saved::Enrollment(saved)) =>
                            expected == &saved.data,
                        _ => false,
                    }),
                "Confirmed change is missing from benefits history."
            );
            self.selected = Some((receipt.kind == "plan", receipt.subject_id));
            self.form = None;
            self.context = Some(r.context);
            self.error = None;
            return Ok(true);
        }
        self.context = Some(r.context);
        Ok(false)
    }
    fn show(&mut self, ui: &mut Ui, data: &Snapshot, busy: bool) -> Option<Operation> {
        ui.label(RichText::new("HR-reviewed plans, eligibility, enrollments and dependents. Monetary effects are reviewed separately in Payroll.").small().color(muted(ui)));
        let mut out = None;
        let editing = self.form.is_some();
        ui.horizontal_wrapped(|ui| {
            if ui
                .add_enabled(
                    !busy,
                    egui::Button::new(if self.context.is_some() {
                        "Refresh benefits"
                    } else {
                        "Load benefits"
                    }),
                )
                .clicked()
            {
                out = Some(Operation::Refresh)
            }
            if self.context.is_some() && !editing {
                if ui
                    .add_enabled(!busy, egui::Button::new("New plan"))
                    .clicked()
                {
                    self.form = Some(Form::plan(
                        Uuid::new_v4(),
                        0,
                        Plan {
                            code: String::new(),
                            name: String::new(),
                            category: Category::HealthInsurance,
                            description: String::new(),
                            eligibility_guidance: String::new(),
                            dependent_limit: 0,
                            available: true,
                        },
                    ));
                }
                if ui
                    .add_enabled(!busy, egui::Button::new("New enrollment"))
                    .clicked()
                {
                    let c = self.context.as_ref().unwrap();
                    match (
                        c.plans.iter().find(|p| p.data.available),
                        c.employees.iter().find(|e| e.active),
                    ) {
                        (Some(p), Some(e)) => {
                            self.form = Some(Form::enrollment(
                                Uuid::new_v4(),
                                0,
                                Enrollment {
                                    plan_id: p.id,
                                    plan_revision: p.revision,
                                    plan_snapshot: p.data.clone(),
                                    employee_source: e.clone(),
                                    eligibility: Eligibility::Pending,
                                    eligibility_basis: String::new(),
                                    status: Status::Pending,
                                    coverage_from: c.today,
                                    coverage_to: None,
                                    dependents: vec![],
                                },
                                false,
                                c.today,
                            ))
                        }
                        _ => {
                            self.error =
                                Some("Add an available plan and an active employee first.".into())
                        }
                    }
                }
            }
        });
        if let Some(e) = &self.error {
            ui.colored_label(Color32::from_rgb(190, 70, 55), e);
        }
        let context = self.context.take();
        if let Some(c) = &context {
            if let Some(f) = &mut self.form {
                if f.rejected {
                    ui.colored_label(Color32::from_rgb(190,70,55),"The save was rejected. Review the current saved fields below before preparing another attempt.");
                    current_view(ui, c, matches!(&f.draft, Draft::Plan(_)), f.id);
                    if ui
                        .add_enabled(!busy, egui::Button::new("Review latest saved details"))
                        .clicked()
                    {
                        match f.review_current(c) {
                            Ok(()) => self.error = None,
                            Err(e) => self.error = Some(e.to_string()),
                        }
                    }
                }
                let mut submit = false;
                let mut discard = false;
                let frozen = f.pending.is_some();
                ui.add_enabled_ui(!busy && !frozen, |ui| {
                    fields(ui, f, c);
                    ui.add_space(8.0);
                    input(
                        ui,
                        "Reason for this change",
                        "benefits-body",
                        &mut f.body,
                        true,
                    );
                    ui.checkbox(
                        &mut f.reviewed,
                        "I reviewed these plan, employment and coverage details.",
                    );
                    if ui.button("Save benefits review").clicked() {
                        submit = true;
                    }
                });
                if let Some(pending) = &f.pending {
                    ui.label(if f.rejected { "This attempt was rejected. Review the latest saved details above or discard the draft." } else { "This exact save is retained until a matching receipt arrives. An interrupted response may already have saved." });
                    if !f.rejected
                        && ui
                            .add_enabled(!busy, egui::Button::new("Retry same save"))
                            .clicked()
                    {
                        out = Some(Operation::Save(Box::new(pending.clone())));
                    }
                }
                if ui
                    .add_enabled(
                        !busy && (!frozen || f.rejected),
                        egui::Button::new("Discard draft"),
                    )
                    .clicked()
                {
                    discard = true;
                }
                if submit {
                    match f.request(c, data) {
                        Ok(r) => {
                            f.pending = Some(r.clone());
                            self.error = None;
                            out = Some(Operation::Save(Box::new(r)));
                        }
                        Err(e) => self.error = Some(e.to_string()),
                    }
                }
                if discard {
                    self.form = None;
                    self.error = None;
                    clear_undo(ui.ctx());
                }
            } else {
                ui.horizontal_wrapped(|ui| {
                    ui.selectable_value(&mut self.tab, Tab::Plans, "Plans");
                    ui.selectable_value(&mut self.tab, Tab::Enrollments, "Enrollments");
                });
                input(ui, "Search", "benefits-search", &mut self.search, false);
                let query = self.search.trim().to_lowercase();
                egui::Grid::new("benefits-list")
                    .num_columns(3)
                    .striped(true)
                    .spacing([16.0, 10.0])
                    .show(ui, |ui| {
                        ui.strong(if self.tab == Tab::Plans {
                            "Benefit plan"
                        } else {
                            "Employee / plan"
                        });
                        ui.strong("Status");
                        ui.strong("Review");
                        ui.end_row();
                        if self.tab == Tab::Plans {
                            for p in &c.plans {
                                if !format!("{} {}", p.data.code, p.data.name)
                                    .to_lowercase()
                                    .contains(&query)
                                {
                                    continue;
                                }
                                if ui
                                    .selectable_label(
                                        self.selected == Some((true, p.id)),
                                        format!("{} · {}", p.data.code, p.data.name),
                                    )
                                    .clicked()
                                {
                                    self.selected = Some((true, p.id));
                                }
                                ui.label(if p.data.available {
                                    "Available"
                                } else {
                                    "Unavailable"
                                });
                                ui.label(format!("Revision {}", p.revision));
                                ui.end_row();
                            }
                        } else {
                            for e in &c.enrollments {
                                if !format!(
                                    "{} {} {}",
                                    e.data.employee_source.employee_code,
                                    e.data.employee_source.full_name,
                                    e.data.plan_snapshot.name
                                )
                                .to_lowercase()
                                .contains(&query)
                                {
                                    continue;
                                }
                                if ui
                                    .selectable_label(
                                        self.selected == Some((false, e.id)),
                                        format!(
                                            "{} · {}\n{}",
                                            e.data.employee_source.employee_code,
                                            e.data.employee_source.full_name,
                                            e.data.plan_snapshot.name
                                        ),
                                    )
                                    .clicked()
                                {
                                    self.selected = Some((false, e.id));
                                }
                                ui.label(e.data.status.label());
                                ui.label(e.data.eligibility.label());
                                ui.end_row();
                            }
                        }
                    });
                if let Some((is_plan, id)) = self.selected {
                    ui.separator();
                    current_view(ui, c, is_plan, id);
                    ui.horizontal_wrapped(|ui| {
                        if is_plan {
                            if let Ok(p) = c.plan(id) {
                                if ui
                                    .add_enabled(!busy, egui::Button::new("Edit plan"))
                                    .clicked()
                                {
                                    self.form = Some(Form::plan(id, p.revision, p.data.clone()));
                                }
                            }
                        } else if let Ok(e) = c.enrollment(id) {
                            if e.data.status != Status::Ended
                                && ui
                                    .add_enabled(
                                        !busy,
                                        egui::Button::new("Review / edit enrollment"),
                                    )
                                    .clicked()
                            {
                                self.form = Some(Form::enrollment(
                                    id,
                                    e.revision,
                                    e.data.clone(),
                                    false,
                                    c.today,
                                ));
                            }
                            if e.data.status == Status::Active
                                && ui
                                    .add_enabled(!busy, egui::Button::new("End coverage"))
                                    .clicked()
                            {
                                self.form = Some(Form::enrollment(
                                    id,
                                    e.revision,
                                    e.data.clone(),
                                    true,
                                    c.today,
                                ));
                            }
                        }
                    });
                    ui.add_space(8.0);
                    ui.heading("Retained history");
                    for e in c
                        .events
                        .iter()
                        .filter(|e| {
                            e.current.id() == id
                                && matches!(
                                    (&e.current, is_plan),
                                    (Saved::Plan(_), true) | (Saved::Enrollment(_), false)
                                )
                        })
                        .rev()
                    {
                        ui.label(
                            RichText::new(format!(
                                "Revision {} · {} · {}",
                                e.current.revision(),
                                e.actor_name,
                                e.created_at
                                    .with_timezone(&data.company.tz())
                                    .format("%d %b %Y %I:%M %p")
                            ))
                            .strong(),
                        );
                        ui.label(&e.body);
                        ui.push_id(e.id, |ui| {
                            ui.collapsing("Saved fields", |ui| match &e.current {
                                Saved::Plan(v) => plan_view(ui, &v.data),
                                Saved::Enrollment(v) => enrollment_view(ui, &v.data),
                            });
                        });
                        ui.add_space(6.0);
                    }
                }
            }
        } else {
            ui.label("Load benefits to view company plans and enrollment records.");
            if let Some(f) = &self.form {
                if let Some(pending) = &f.pending {
                    if !f.rejected
                        && ui
                            .add_enabled(!busy, egui::Button::new("Retry same save"))
                            .clicked()
                    {
                        out = Some(Operation::Save(Box::new(pending.clone())));
                    }
                }
            }
        }
        self.context = context;
        out
    }
}
fn input(ui: &mut Ui, label: &str, id: &str, value: &mut String, multi: bool) {
    ui.label(label);
    let edit = if multi {
        egui::TextEdit::multiline(value).desired_rows(3)
    } else {
        egui::TextEdit::singleline(value)
    };
    ui.add(
        edit.id(egui::Id::unique(id))
            .desired_width(ui.available_width()),
    );
}
fn pair(ui: &mut Ui, label: &str, value: impl Into<String>) {
    ui.label(label);
    ui.label(value.into());
    ui.end_row();
}
fn plan_view(ui: &mut Ui, p: &Plan) {
    egui::Grid::new("benefit-plan-details")
        .num_columns(2)
        .spacing([18.0, 8.0])
        .show(ui, |ui| {
            pair(ui, "Code", p.code.clone());
            pair(ui, "Plan", p.name.clone());
            pair(ui, "Category", p.category.label());
            pair(ui, "Dependent limit", p.dependent_limit.to_string());
            pair(
                ui,
                "Availability",
                if p.available {
                    "Available"
                } else {
                    "Unavailable"
                },
            );
        });
    ui.label(&p.description);
    ui.label(RichText::new("Eligibility guidance").strong());
    ui.label(&p.eligibility_guidance);
}
fn enrollment_view(ui: &mut Ui, e: &Enrollment) {
    egui::Grid::new("benefit-enrollment-details")
        .num_columns(2)
        .spacing([18.0, 8.0])
        .show(ui, |ui| {
            pair(
                ui,
                "Employee",
                format!(
                    "{} · {}",
                    e.employee_source.employee_code, e.employee_source.full_name
                ),
            );
            pair(
                ui,
                "Plan",
                format!("{} · Revision {}", e.plan_snapshot.name, e.plan_revision),
            );
            pair(ui, "Department", e.employee_source.department.clone());
            pair(ui, "Position", e.employee_source.job_title.clone());
            pair(ui, "Status", e.status.label());
            pair(ui, "Eligibility", e.eligibility.label());
            pair(
                ui,
                "Coverage",
                format!(
                    "{} to {}",
                    e.coverage_from,
                    e.coverage_to
                        .map(|v| v.to_string())
                        .unwrap_or_else(|| "No recorded end".into())
                ),
            );
        });
    ui.label(&e.eligibility_basis);
    ui.label(RichText::new("Dependents").strong());
    if e.dependents.is_empty() {
        ui.label("None recorded.");
    }
    for d in &e.dependents {
        ui.label(format!(
            "{} · {} · {} to {}",
            d.full_name,
            d.relationship.label(),
            d.coverage_from,
            d.coverage_to
                .map(|v| v.to_string())
                .unwrap_or_else(|| "No recorded end".into())
        ));
        if let Some(birth) = d.birth_date {
            ui.label(format!("Date of birth: {birth}"));
        }
    }
}
fn current_view(ui: &mut Ui, c: &Context, is_plan: bool, id: Uuid) {
    if is_plan {
        if let Ok(p) = c.plan(id) {
            plan_view(ui, &p.data);
        }
    } else if let Ok(e) = c.enrollment(id) {
        enrollment_view(ui, &e.data);
    }
}
fn fields(ui: &mut Ui, f: &mut Form, c: &Context) {
    match &mut f.draft {
        Draft::Plan(p) => {
            ui.heading(if f.revision == 0 {
                "New benefit plan"
            } else {
                "Edit benefit plan"
            });
            input(ui, "Plan code", "benefits-code", &mut p.code, false);
            input(ui, "Plan name", "benefits-name", &mut p.name, false);
            egui::ComboBox::from_id_salt("benefits-category")
                .selected_text(p.category.label())
                .show_ui(ui, |ui| {
                    for v in Category::ALL {
                        ui.selectable_value(&mut p.category, v, v.label());
                    }
                });
            input(
                ui,
                "Description",
                "benefits-description",
                &mut p.description,
                true,
            );
            input(
                ui,
                "Company eligibility guidance",
                "benefits-guidance",
                &mut p.eligibility_guidance,
                true,
            );
            ui.horizontal_wrapped(|ui| {
                ui.label("Maximum dependents");
                ui.add(egui::DragValue::new(&mut p.dependent_limit).range(0..=25));
                ui.checkbox(&mut p.available, "Available for a new review");
            });
        }
        Draft::Enrollment {
            value: e,
            from,
            to,
            dependents,
            ending,
        } => {
            ui.heading(if *ending {
                "End reviewed coverage"
            } else if f.revision == 0 {
                "New enrollment review"
            } else {
                "Review enrollment"
            });
            if f.revision == 0 {
                let mut employee = e.employee_source.id;
                egui::ComboBox::from_id_salt("benefits-employee")
                    .width(ui.available_width().min(450.0))
                    .selected_text(format!(
                        "{} · {}",
                        e.employee_source.employee_code, e.employee_source.full_name
                    ))
                    .show_ui(ui, |ui| {
                        for p in c.employees.iter().filter(|p| p.active) {
                            ui.selectable_value(
                                &mut employee,
                                p.id,
                                format!("{} · {}", p.employee_code, p.full_name),
                            );
                        }
                    });
                if employee != e.employee_source.id {
                    e.employee_source = c
                        .employees
                        .iter()
                        .find(|p| p.id == employee)
                        .unwrap()
                        .clone();
                    f.reviewed = false;
                }
                let mut plan = e.plan_id;
                egui::ComboBox::from_id_salt("benefits-plan")
                    .width(ui.available_width().min(450.0))
                    .selected_text(&e.plan_snapshot.name)
                    .show_ui(ui, |ui| {
                        for p in c.plans.iter().filter(|p| p.data.available) {
                            ui.selectable_value(&mut plan, p.id, &p.data.name);
                        }
                    });
                if plan != e.plan_id {
                    let p = c.plan(plan).unwrap();
                    e.plan_id = plan;
                    e.plan_revision = p.revision;
                    e.plan_snapshot = p.data.clone();
                    f.reviewed = false;
                }
            } else {
                ui.label(format!(
                    "{} · {}",
                    e.employee_source.employee_code, e.employee_source.full_name
                ));
                ui.label(format!(
                    "{} · Captured revision {}",
                    e.plan_snapshot.name, e.plan_revision
                ));
            }
            ui.label(&e.plan_snapshot.eligibility_guidance);
            if *ending {
                input(
                    ui,
                    "Final coverage date (YYYY-MM-DD)",
                    "benefits-to",
                    to,
                    false,
                );
                ui.label("The prior review and dependent identities are retained; existing dependent end dates are clipped to this end date.");
            } else {
                ui.horizontal_wrapped(|ui| {
                    egui::ComboBox::from_id_salt("benefits-eligibility")
                        .selected_text(e.eligibility.label())
                        .show_ui(ui, |ui| {
                            for v in Eligibility::ALL {
                                ui.selectable_value(&mut e.eligibility, v, v.label());
                            }
                        });
                    egui::ComboBox::from_id_salt("benefits-status")
                        .selected_text(e.status.label())
                        .show_ui(ui, |ui| {
                            for v in [Status::Pending, Status::Active, Status::Declined] {
                                ui.selectable_value(&mut e.status, v, v.label());
                            }
                        });
                });
                input(
                    ui,
                    "Basis for recorded eligibility",
                    "benefits-basis",
                    &mut e.eligibility_basis,
                    true,
                );
                input(
                    ui,
                    "Coverage from (YYYY-MM-DD)",
                    "benefits-from",
                    from,
                    false,
                );
                input(ui, "Coverage to (optional)", "benefits-to", to, false);
                ui.label(RichText::new("Dependents").strong());
                let mut remove = None;
                for (i, d) in dependents.iter_mut().enumerate() {
                    ui.push_id(d.value.id, |ui| {
                        ui.separator();
                        input(
                            ui,
                            "Full name",
                            &format!("benefits-dependent-{i}-name"),
                            &mut d.value.full_name,
                            false,
                        );
                        egui::ComboBox::from_id_salt("relationship")
                            .selected_text(d.value.relationship.label())
                            .show_ui(ui, |ui| {
                                for v in Relationship::ALL {
                                    ui.selectable_value(&mut d.value.relationship, v, v.label());
                                }
                            });
                        input(
                            ui,
                            "Date of birth (optional)",
                            &format!("benefits-dependent-{i}-birth"),
                            &mut d.birth,
                            false,
                        );
                        input(
                            ui,
                            "Dependent coverage from",
                            &format!("benefits-dependent-{i}-from"),
                            &mut d.from,
                            false,
                        );
                        input(
                            ui,
                            "Dependent coverage to (optional)",
                            &format!("benefits-dependent-{i}-to"),
                            &mut d.to,
                            false,
                        );
                        if ui.button("Remove from this review").clicked() {
                            remove = Some(i);
                        }
                    });
                }
                if let Some(i) = remove {
                    dependents.remove(i);
                    clear_undo(ui.ctx());
                    f.reviewed = false;
                }
                if ui
                    .add_enabled(
                        dependents.len() < usize::from(e.plan_snapshot.dependent_limit),
                        egui::Button::new("Add dependent"),
                    )
                    .clicked()
                {
                    dependents.push(DependentDraft::new(Dependent {
                        id: Uuid::new_v4(),
                        full_name: String::new(),
                        relationship: Relationship::Child,
                        birth_date: None,
                        coverage_from: parse_date(from).unwrap_or(c.today),
                        coverage_to: optional_date(to).ok().flatten(),
                    }));
                    f.reviewed = false;
                }
                if ui.button("Refresh plan and employment details").clicked() {
                    if let (Ok(p), Some(person)) = (
                        c.plan(e.plan_id),
                        c.employees.iter().find(|p| p.id == e.employee_source.id),
                    ) {
                        e.plan_revision = p.revision;
                        e.plan_snapshot = p.data.clone();
                        e.employee_source = person.clone();
                        f.reviewed = false;
                    }
                }
            }
        }
    }
}
impl HrisApp {
    pub(super) fn benefits(&mut self, ui: &mut Ui) {
        if !benefits::eligible(&self.data.access) {
            self.benefits_state = Default::default();
            clear_undo(ui.ctx());
            return;
        }
        let mut state = std::mem::take(&mut self.benefits_state);
        let op = state.show(ui, &self.data, self.busy);
        self.benefits_state = state;
        if let Some(op) = op {
            self.send(Command::Benefits(op));
        }
    }
}

#[cfg(any(test, feature = "qa-screenshot"))]
fn fictional_context(d: &Snapshot) -> (Context, Uuid, Uuid) {
    let mut repo = benefits::Demo::default();
    let mut c = repo.execute(Operation::Refresh, d).unwrap().context;
    let plan_id = Uuid::new_v4();
    for (i, code, name, category, limit) in [
        (
            0,
            "HEALTH-01",
            "Company health coverage",
            Category::HealthInsurance,
            2,
        ),
        (
            1,
            "WELLNESS-01",
            "Annual wellness program",
            Category::Wellness,
            0,
        ),
    ] {
        let p=Plan{code:code.into(),name:name.into(),category,description:"Fictional company benefit for local review. No insurer or paid service is connected.".into(),eligibility_guidance:"HR reviews the saved employment facts and company conditions before recording eligibility. Missing service dates remain unknown.".into(),dependent_limit:limit,available:true};
        let r = Request::new(
            &d.access,
            &c,
            if i == 0 { plan_id } else { Uuid::new_v4() },
            Change::Plan(p),
            "Fictional initial plan review".into(),
        )
        .unwrap();
        c = repo
            .execute(Operation::Save(Box::new(r)), d)
            .unwrap()
            .context;
    }
    let p = c.plan(plan_id).unwrap();
    let employee = c
        .employees
        .iter()
        .filter(|e| e.active)
        .nth(1)
        .unwrap()
        .clone();
    let from = c.today - Duration::days(30);
    let enr_id = Uuid::new_v4();
    let e=Enrollment{plan_id,plan_revision:p.revision,plan_snapshot:p.data.clone(),employee_source:employee,eligibility:Eligibility::Eligible,eligibility_basis:"Fictional HR review of company coverage conditions against the captured employment source. This records no insurer acceptance or statutory conclusion.".into(),status:Status::Active,coverage_from:from,coverage_to:None,dependents:vec![Dependent{id:Uuid::new_v4(),full_name:"Fictional dependent Alex".into(),relationship:Relationship::Child,birth_date:Some(NaiveDate::from_ymd_opt(2014,6,15).unwrap()),coverage_from:from,coverage_to:None}]};
    let r = Request::new(
        &d.access,
        &c,
        enr_id,
        Change::Enrollment(e),
        "Fictional reviewed enrollment; no payroll amount changes".into(),
    )
    .unwrap();
    c = repo
        .execute(Operation::Save(Box::new(r)), d)
        .unwrap()
        .context;
    (c, plan_id, enr_id)
}
#[cfg(feature = "qa-screenshot")]
impl HrisApp {
    pub(super) fn prepare_benefits_preview(&mut self) {
        let name = std::env::var("HRIS_PREVIEW_FORM").unwrap_or_default();
        if !name.starts_with("ben-")
            || self.desktop.product != crate::desktop::Product::Hris
            || self.data.mode != Mode::Demo
            || !benefits::eligible(&self.data.access)
        {
            return;
        }
        if self.data.access.user_id.is_nil() {
            self.data.access.user_id = Uuid::from_u128(10);
        }
        let (c, plan_id, enr_id) = fictional_context(&self.data);
        let mut state = State {
            context: Some(c.clone()),
            ..Default::default()
        };
        let p = c.plan(plan_id).unwrap();
        let enr = c.enrollment(enr_id).unwrap();
        match name.as_str() {
            "ben-empty" => state.context = None,
            "ben-enrollments" => state.tab = Tab::Enrollments,
            "ben-plan" => state.selected = Some((true, plan_id)),
            "ben-enrollment" => {
                state.tab = Tab::Enrollments;
                state.selected = Some((false, enr_id));
            }
            "ben-new-plan" => {
                let mut plan = p.data.clone();
                plan.code = "NEW-COVERAGE".into();
                plan.name = "New company benefit".into();
                state.form = Some(Form::plan(Uuid::new_v4(), 0, plan));
            }
            "ben-new-enrollment" => {
                let mut e = enr.data.clone();
                e.employee_source = c
                    .employees
                    .iter()
                    .find(|e| e.active && e.id != enr.data.employee_source.id)
                    .unwrap()
                    .clone();
                e.eligibility = Eligibility::Pending;
                e.status = Status::Pending;
                e.dependents.clear();
                state.form = Some(Form::enrollment(Uuid::new_v4(), 0, e, false, c.today));
            }
            "ben-dependent" => {
                state.form = Some(Form::enrollment(
                    enr_id,
                    enr.revision,
                    enr.data.clone(),
                    false,
                    c.today,
                ))
            }
            "ben-end" => {
                state.form = Some(Form::enrollment(
                    enr_id,
                    enr.revision,
                    enr.data.clone(),
                    true,
                    c.today,
                ))
            }
            "ben-pending" | "ben-rejected" => {
                let mut plan = p.data.clone();
                plan.description =
                    "Fictional revised plan description prepared for a recorded review.".into();
                let mut f = Form::plan(plan_id, p.revision, plan);
                f.body = "Fictional review kept as one exact pending save.".into();
                f.reviewed = true;
                f.pending = Some(f.request(&c, &self.data).unwrap());
                f.rejected = name == "ben-rejected";
                state.form = Some(f);
            }
            _ => {}
        }
        if let Some(f) = &mut state.form {
            if f.body.is_empty() {
                f.body = "Fictional HR review; no automatic pay or provider change.".into();
            }
        }
        self.page = Page::Employees;
        self.employee_view = EmployeeView::Benefits;
        self.benefits_state = state;
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> (Snapshot, Context, Uuid) {
        let mut d = DemoStore::new().data;
        d.access.user_id = Uuid::from_u128(10);
        let (c, plan_id, _) = fictional_context(&d);
        (d, c, plan_id)
    }
    fn enrollment(c: &Context, plan_id: Uuid) -> Enrollment {
        let p = c.plan(plan_id).unwrap();
        Enrollment {
            plan_id,
            plan_revision: p.revision,
            plan_snapshot: p.data.clone(),
            employee_source: c.employees[0].clone(),
            eligibility: Eligibility::Eligible,
            eligibility_basis: "Fictional review of recorded company conditions".into(),
            status: Status::Active,
            coverage_from: c.today,
            coverage_to: None,
            dependents: vec![Dependent {
                id: Uuid::new_v4(),
                full_name: "Fictional retained dependent name".into(),
                relationship: Relationship::Child,
                birth_date: None,
                coverage_from: c.today,
                coverage_to: None,
            }],
        }
    }
    #[test]
    fn uncertain_request_is_frozen_and_rejection_needs_explicit_review() {
        let (d, c, id) = fixture();
        let mut f = Form::plan(id, 1, c.plan(id).unwrap().data.clone());
        if let Draft::Plan(p) = &mut f.draft {
            p.description = "Explicit fictional draft change".into();
        }
        f.body = "Fictional reviewed UI change".into();
        f.reviewed = true;
        let r = f.request(&c, &d).unwrap();
        f.pending = Some(r.clone());
        let mut s = State {
            context: Some(c.clone()),
            form: Some(f),
            ..Default::default()
        };
        s.clear_context();
        assert_eq!(s.form.as_ref().unwrap().pending, Some(r));
        assert!(s.editing());
        s.reject();
        assert!(s.form.as_ref().unwrap().rejected);
        s.form.as_mut().unwrap().review_current(&c).unwrap();
        assert!(s.form.as_ref().unwrap().pending.is_none());
        assert!(!s.form.as_ref().unwrap().reviewed);
    }
    #[test]
    fn forms_and_lists_fit_compact_and_wide_light_dark_views() {
        let (d, c, id) = fixture();
        let e = enrollment(&c, id);
        for width in [480.0, 760.0, 1100.0] {
            for dark in [false, true] {
                for mode in 0..10 {
                    let mut state = State {
                        context: Some(c.clone()),
                        ..Default::default()
                    };
                    match mode {
                        0 => {}
                        1 => state.selected = Some((true, id)),
                        2 => state.form = Some(Form::plan(id, 1, c.plan(id).unwrap().data.clone())),
                        3 => {
                            state.form = Some(Form::enrollment(
                                Uuid::new_v4(),
                                0,
                                e.clone(),
                                false,
                                c.today,
                            ))
                        }
                        5 => {
                            state.tab = Tab::Enrollments;
                            state.selected = Some((false, c.enrollments[0].id));
                        }
                        6 | 7 => {
                            let mut plan = c.plan(id).unwrap().data.clone();
                            plan.description = "Fictional new review description".into();
                            let mut f = Form::plan(id, 1, plan);
                            f.body = "Fictional pending review".into();
                            f.reviewed = true;
                            f.pending = Some(f.request(&c, &d).unwrap());
                            f.rejected = mode == 7;
                            state.form = Some(f);
                        }
                        8 => {
                            state.context = None;
                        }
                        9 => {
                            let mut plan = c.plan(id).unwrap().data.clone();
                            plan.description = "Fictional uncertain review description".into();
                            let mut f = Form::plan(id, 1, plan);
                            f.body = "Fictional exact pending review".into();
                            f.reviewed = true;
                            f.pending = Some(f.request(&c, &d).unwrap());
                            state.form = Some(f);
                            state.context = None;
                        }
                        _ => {
                            state.form = Some(Form::enrollment(
                                Uuid::new_v4(),
                                1,
                                e.clone(),
                                true,
                                c.today,
                            ))
                        }
                    };
                    let ctx = egui::Context::default();
                    ctx.set_visuals(if dark {
                        egui::Visuals::dark()
                    } else {
                        egui::Visuals::light()
                    });
                    for _ in 0..3 {
                        let mut bounds = egui::Rect::NOTHING;
                        let mut output = ctx.run_ui(
                            egui::RawInput {
                                screen_rect: Some(egui::Rect::from_min_size(
                                    egui::Pos2::ZERO,
                                    egui::vec2(width, 1800.0),
                                )),
                                ..Default::default()
                            },
                            |ctx| {
                                egui::CentralPanel::default().show(ctx, |ui| {
                                    state.show(ui, &d, false);
                                    bounds = ui.min_rect();
                                });
                            },
                        );
                        output.textures_delta.clear();
                        assert!(
                            bounds.right() <= width + 1.0,
                            "Benefit layout overflow width={width} dark={dark} mode={mode} right={}",
                            bounds.right()
                        );
                    }
                }
            }
        }
    }
}
