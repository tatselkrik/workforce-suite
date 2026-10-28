use super::payroll_view::{amount, choice, employee_choice, field, footer, note};
use super::*;
use crate::{
    payroll::{self, PayrollAction as Change},
    payroll_thirteenth as benefit,
};
use serde_json::json;
#[derive(Default)]
pub(super) struct State {
    pub(super) selected: Option<Uuid>,
    slip: Option<Uuid>,
    employee: Option<Uuid>,
    form: Option<Form>,
    slips: bool,
}
enum Form {
    Run(RunForm),
    Item(ItemForm),
    Decision(Decision),
}
struct RunForm {
    id: Uuid,
    revision: Option<i64>,
    year: String,
    as_of: String,
    pay: String,
    title: String,
    reason: String,
}
struct ItemForm {
    item: benefit::Item,
    w: benefit::Worksheet,
    openings: Vec<OpeningForm>,
    reason: String,
}
struct OpeningForm {
    start: String,
    end: String,
    amount: i64,
    reference: String,
}
struct Decision {
    id: Uuid,
    revision: i64,
    op: String,
    reason: String,
    confirmed: bool,
    paid: String,
    reference: String,
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn benefit_source_refresh_preserves_unsaved_policy_and_opening_edits_but_resets_reviews() {
        let mut s = DemoStore::new();
        let eid = s.data.employees[1].id;
        let r = Uuid::new_v4();
        let id = Uuid::new_v4();
        for a in [
            Change::new(
                "thirteenth_run_create",
                json!({"id":r,"year":2026,"as_of":"2026-09-30","pay_date":"2026-10-01","title":"Synthetic form","reason":"Synthetic period"}),
            ),
            Change::new(
                "thirteenth_item_add",
                json!({"id":id,"run_id":r,"employee_id":eid,"reason":"Synthetic scope"}),
            ),
        ] {
            s.execute(Action::Payroll(a)).unwrap();
        }
        let i = s.data.payroll.thirteenth.items[0].clone();
        let mut w = i.worksheet.clone();
        w.policy_topup_cents = 12345;
        w.policy_note = "Unsaved policy evidence".into();
        w.reviewed = true;
        w.tax_reviewed = true;
        w.contributions_reviewed = true;
        let mut state = State {
            form: Some(Form::Item(ItemForm {
                item: i,
                w,
                openings: vec![OpeningForm {
                    start: "2026-01-01".into(),
                    end: "2026-01-31".into(),
                    amount: 45678,
                    reference: "Unsaved opening evidence".into(),
                }],
                reason: "Unsaved reason".into(),
            })),
            ..Default::default()
        };
        let a = Change::new(
            "thirteenth_item_refresh",
            json!({"id":id,"expected_revision":0,"reason":"Refresh synthetic source"}),
        );
        s.execute(Action::Payroll(a.clone())).unwrap();
        state.accept(&a, &s.data);
        let Some(Form::Item(f)) = state.form else {
            panic!("Open worksheet was lost")
        };
        assert_eq!(f.w.policy_topup_cents, 12345);
        assert_eq!(f.openings[0].amount, 45678);
        assert!(!f.w.reviewed && !f.w.tax_reviewed && !f.w.contributions_reviewed);
        assert_eq!(f.item.revision, 1);
    }
}
fn openings(w: &benefit::Worksheet) -> Vec<OpeningForm> {
    w.openings
        .iter()
        .map(|o| OpeningForm {
            start: o.starts_on.to_string(),
            end: o.ends_on.to_string(),
            amount: o.basic_cents,
            reference: o.reference.clone(),
        })
        .collect()
}
impl State {
    pub(super) fn open_notice(&mut self,id:Uuid){self.selected=None;self.employee=None;self.slips=true;self.slip=Some(id);}
    pub(super) fn editing(&self) -> bool {
        self.form.is_some()
    }
    pub(super) fn accept(&mut self, a: &Change, data: &Snapshot) {
        if a.operation == "thirteenth_item_refresh" {
            if let Some(Form::Item(f)) = self.form.as_mut() {
                if let Some(i) =
                    data.payroll.thirteenth.items.iter().find(|i| {
                        Some(i.id) == a.record["id"].as_str().and_then(|s| s.parse().ok())
                    })
                {
                    f.item = i.clone();
                    f.w.external_thirteenth_cents =
                        f.w.external_thirteenth_cents
                            .max(i.worksheet.external_thirteenth_cents);
                    f.w.other_benefits_cents =
                        f.w.other_benefits_cents
                            .max(i.worksheet.other_benefits_cents);
                    f.w.reviewed = false;
                    f.w.review_note.clear();
                    f.w.tax_reviewed = false;
                    f.w.contributions_reviewed = false;
                    f.reason.clear();
                    return;
                }
            }
        }
        if a.operation == "thirteenth_run_create" {
            self.selected = a.record["id"].as_str().and_then(|s| s.parse().ok());
        }
        self.form = None;
    }
}
fn totals(ui: &mut Ui, c: &benefit::Calculation, id: impl std::hash::Hash + std::fmt::Debug) {
    egui::Grid::new(id)
        .num_columns(2)
        .spacing([32.0, 7.0])
        .striped(true)
        .show(ui, |ui| {
            for (label, n) in benefit::totals_rows(c) {
                ui.label(label);
                ui.label(RichText::new(payroll::money(n)).strong());
                ui.end_row();
            }
        });
    for g in &c.coverage_gaps {
        ui.colored_label(
            Color32::DARK_RED,
            format!(
                "Missing reviewed coverage: {} to {}",
                g.starts_on, g.ends_on
            ),
        );
    }
}
impl HrisApp {
    #[cfg(feature = "qa-screenshot")]
    pub(super) fn prepare_thirteenth_preview(&mut self, form: &str) {
        let mut store = DemoStore::new();
        let eid = store.data.employees[1].id;
        let rid = Uuid::from_u128(99500);
        let iid = Uuid::from_u128(99510);
        let apply = |s: &mut DemoStore, op: &str, v: serde_json::Value| {
            s.execute(Action::Payroll(Change::new(op, v)))
                .expect("Fictional benefit preview")
        };
        apply(
            &mut store,
            "thirteenth_run_create",
            json!({"id":rid,"year":2026,"as_of":"2026-09-30","pay_date":"2026-10-01","title":"2026 13th-month pay through September","reason":"Fictional annual benefit"}),
        );
        apply(
            &mut store,
            "thirteenth_item_add",
            json!({"id":iid,"run_id":rid,"employee_id":eid,"reason":"Fictional employee"}),
        );
        if !form.ends_with("missing") {
            let src = benefit::source(&store.data.payroll.thirteenth.items[0]).unwrap();
            let w = benefit::Worksheet {
                openings: vec![benefit::Opening {
                    starts_on: src.starts_on,
                    ends_on: src.as_of,
                    basic_cents: 27000000,
                    reference: "Fictional reviewed earning ledger".into(),
                }],
                classification: "rank_and_file".into(),
                eligibility_note: "Fictional duties and service reviewed".into(),
                served_one_month: true,
                other_benefits_cents: 7500000,
                opening_note: "Fictional earlier benefit ledger".into(),
                withholding_cents: 75000,
                tax_reviewed: true,
                tax_note:
                    "Fictional reviewed supplementary withholding; not a statutory tax vector"
                        .into(),
                contributions_reviewed: true,
                contribution_note: "Fictional monthly contribution records reviewed".into(),
                reviewed: true,
                review_note: "Fictional source and payout review".into(),
                ..Default::default()
            };
            apply(
                &mut store,
                "thirteenth_item_save",
                json!({"id":iid,"expected_revision":0,"worksheet":w,"reason":"Fictional reviewed worksheet"}),
            );
        }
        if form.ends_with("saved") || form.ends_with("slip") || form.ends_with("manager") {
            let rev = store.data.payroll.thirteenth.runs[0].revision;
            apply(
                &mut store,
                "thirteenth_run_submit",
                json!({"id":rid,"expected_revision":rev,"scope_reviewed":true,"reason":"Fictional submit"}),
            );
        }
        if form.ends_with("slip") || form.ends_with("manager") {
            store.data.access.user_id = Uuid::from_u128(99600);
            let rev = store.data.payroll.thirteenth.runs[0].revision;
            apply(
                &mut store,
                "thirteenth_run_approve",
                json!({"id":rid,"expected_revision":rev,"reason":"Fictional independent approval"}),
            );
            if let Ok(path) = std::env::var("HRIS_THIRTEENTH_PDF_TO") {
                std::fs::write(
                    path,
                    crate::payroll_pdf::build_thirteenth(&store.data.payroll.thirteenth.slips[0])
                        .unwrap(),
                )
                .unwrap();
            }
        }
        if form.ends_with("manager") {
            store.data.access.role = Role::Manager;
            store.data.access.employee_id = Some(eid);
        }
        self.data = store.data;
        self.payroll_state.thirteenth = State::default();
        let r = self.data.payroll.thirteenth.runs[0].clone();
        let i = self.data.payroll.thirteenth.items[0].clone();
        if form.ends_with("detail") {
            self.payroll_state.thirteenth.selected = Some(rid);
        }
        if form.ends_with("worksheet") || form.ends_with("missing") || form.ends_with("saved") {
            self.payroll_state.thirteenth.form = Some(Form::Item(ItemForm {
                openings: openings(&i.worksheet),
                w: i.worksheet.clone(),
                item: i,
                reason: String::new(),
            }));
        }
        if form.ends_with("approval") {
            self.thirteenth_decide(&r, "run_submit");
        }
        if form.ends_with("create") {
            self.payroll_state.thirteenth.form = Some(Form::Run(RunForm {
                id: Uuid::from_u128(99590),
                revision: None,
                year: "2026".into(),
                as_of: "2026-09-30".into(),
                pay: "2026-10-01".into(),
                title: "2026 13th-month pay".into(),
                reason: String::new(),
            }));
        }
        if form.ends_with("slip") || form.ends_with("manager") {
            self.payroll_state.thirteenth.slips = true;
            self.payroll_state.thirteenth.slip = Some(iid);
        }
    }
    pub(super) fn payroll_thirteenth(&mut self, ui: &mut Ui) {
        let hr = self.data.access.allows(crate::staff_roles::Capability::PayrollPreparation);
        if !hr {
            self.payroll_state.thirteenth.slips = true;
            self.payroll_state.thirteenth.form = None;
        }
        ui.horizontal_wrapped(|ui| {
            if hr {
                ui.selectable_value(
                    &mut self.payroll_state.thirteenth.slips,
                    false,
                    "Benefit runs",
                );
            }
            ui.selectable_value(
                &mut self.payroll_state.thirteenth.slips,
                true,
                if hr {
                    "Issued benefit slips"
                } else {
                    "My 13th-month slips"
                },
            );
        });
        ui.add_space(10.0);
        if self.payroll_state.thirteenth.slips {
            self.thirteenth_slips(ui);
            return;
        }
        if let Some(r) = self
            .data
            .payroll
            .thirteenth
            .runs
            .iter()
            .find(|r| Some(r.id) == self.payroll_state.thirteenth.selected)
            .cloned()
        {
            self.thirteenth_detail(ui, &r);
            return;
        }
        ui.heading("13th-month pay");
        ui.label("Earned current-employer basic ÷ 12, earlier payments and one shared annual benefit exclusion. Eligibility is reviewed from actual employment duties and service, independently of an account's role.");
        ui.label("Review the December 24 deadline. For an employer-funded advance or more favorable benefit, record the explicit policy amount and reason; never present future basic as already earned. Recording payment transfers no money.");
        if ui.button("Create 13th-month payout").clicked() {
            let today = self.data.company.today();
            let end = today.with_day(1).unwrap() - chrono::Duration::days(1);
            self.payroll_state.thirteenth.form = Some(Form::Run(RunForm {
                id: Uuid::new_v4(),
                revision: None,
                year: end.year().to_string(),
                as_of: end.to_string(),
                pay: today.to_string(),
                title: format!("{} 13th-month pay through {}", end.year(), end),
                reason: String::new(),
            }));
        }
        let mut runs = self.data.payroll.thirteenth.runs.clone();
        runs.sort_by_key(|r| std::cmp::Reverse((r.year, r.as_of, r.id)));
        for r in runs {
            card(ui, |ui| {
                if ui.link(&r.title).clicked() {
                    self.payroll_state.thirteenth.selected = Some(r.id);
                }
                ui.label(format!(
                    "{} · Earned through {} · Planned pay {} · {}",
                    r.year, r.as_of, r.pay_date, r.status
                ));
                let items: Vec<_> = self
                    .data
                    .payroll
                    .thirteenth
                    .items
                    .iter()
                    .filter(|i| i.run_id == r.id)
                    .collect();
                ui.label(format!(
                    "{} employee(s) · Net {}",
                    items.len(),
                    payroll::money(items.iter().map(|i| i.totals.net_cents).sum())
                ));
            });
        }
    }
    fn thirteenth_decide(&mut self, r: &benefit::Run, op: &str) {
        self.payroll_state.thirteenth.form = Some(Form::Decision(Decision {
            id: r.id,
            revision: r.revision,
            op: op.into(),
            reason: String::new(),
            confirmed: false,
            paid: self.data.company.today().to_string(),
            reference: String::new(),
        }));
    }
    fn thirteenth_detail(&mut self, ui: &mut Ui, r: &benefit::Run) {
        if ui.button("Back to benefit runs").clicked() {
            self.payroll_state.thirteenth.selected = None;
            return;
        }
        ui.heading(&r.title);
        ui.label(format!(
            "Year {} · Earned through {} · Planned pay {} · {}",
            r.year, r.as_of, r.pay_date, r.status
        ));
        ui.horizontal_wrapped(|ui| {
            if r.status == "draft" {
                if ui.button("Edit title / pay date").clicked() {
                    self.payroll_state.thirteenth.form = Some(Form::Run(RunForm {
                        id: r.id,
                        revision: Some(r.revision),
                        year: r.year.to_string(),
                        as_of: r.as_of.to_string(),
                        pay: r.pay_date.to_string(),
                        title: r.title.clone(),
                        reason: String::new(),
                    }));
                }
                if ui.button("Submit for approval").clicked() {
                    self.thirteenth_decide(r, "run_submit");
                }
            }
            if r.status == "submitted" {
                if self.data.access.role == Role::Administrator
                    && ui.button("Approve and issue slips").clicked()
                {
                    self.thirteenth_decide(r, "run_approve");
                }
                if ui.button("Return for correction").clicked() {
                    self.thirteenth_decide(r, "run_return");
                }
            }
            if ["draft", "submitted"].contains(&r.status.as_str())
                && ui.button("Cancel unissued payout").clicked()
            {
                self.thirteenth_decide(r, "run_cancel");
            }
            if r.status == "approved"
                && self.data.access.role == Role::Administrator
                && ui.button("Record payment").clicked()
            {
                self.thirteenth_decide(r, "run_paid");
            }
            if ui.button("Export benefit register CSV").clicked() {
                self.payroll_export("thirteenth_register", Some(r.id));
            }
        });
        if r.status == "paid" {
            ui.label(format!(
                "Recorded payment {} · {}",
                r.paid_on.map(|d| d.to_string()).unwrap_or_default(),
                r.payment_reference
            ));
        }
        if r.status == "draft" {
            card(ui, |ui| {
                employee_choice(
                    ui,
                    &self.data,
                    "thirteenth_employee",
                    &mut self.payroll_state.thirteenth.employee,
                );
                ui.label("Archived employees remain eligible for a reviewed pro-rata benefit; sign-in access is separate.");
                if let Some(eid) = self.payroll_state.thirteenth.employee {
                    let has = self
                        .data
                        .payroll
                        .thirteenth
                        .items
                        .iter()
                        .any(|i| i.run_id == r.id && i.employee_id == eid);
                    if ui
                        .add_enabled(!has, egui::Button::new("Add employee worksheet"))
                        .clicked()
                    {
                        self.send_payroll(Change::new("thirteenth_item_add",json!({"id":Uuid::new_v4(),"run_id":r.id,"employee_id":eid,"reason":"Add employee to annual benefit scope"})));
                    }
                }
            });
        }
        for i in self
            .data
            .payroll
            .thirteenth
            .items
            .iter()
            .filter(|i| i.run_id == r.id)
            .cloned()
            .collect::<Vec<_>>()
        {
            card(ui, |ui| {
                ui.heading(format!("{} · {}", i.employee_code, i.employee_name));
                ui.label(format!(
                    "{} · Gross {} · Net {}",
                    if i.worksheet.reviewed {
                        "Reviewed"
                    } else {
                        "Needs review"
                    },
                    payroll::money(i.totals.gross_cents),
                    payroll::money(i.totals.net_cents)
                ));
                for g in &i.totals.coverage_gaps {
                    ui.label(format!("Missing basis: {} to {}", g.starts_on, g.ends_on));
                }
                if !i.worksheet.review_note.is_empty() {
                    ui.label(format!("HR review: {}", i.worksheet.review_note));
                }
                {
                    ui.horizontal_wrapped(|ui| {
                        if ui
                            .button(if r.status == "draft" {
                                "Open worksheet"
                            } else {
                                "Review saved worksheet"
                            })
                            .clicked()
                        {
                            self.payroll_state.thirteenth.form = Some(Form::Item(ItemForm {
                                openings: openings(&i.worksheet),
                                w: i.worksheet.clone(),
                                item: i.clone(),
                                reason: String::new(),
                            }));
                        }
                        if r.status == "draft" && ui.button("Remove from draft").clicked() {
                            self.payroll_state.thirteenth.form = Some(Form::Decision(Decision {
                                id: i.id,
                                revision: i.revision,
                                op: "item_remove".into(),
                                reason: String::new(),
                                confirmed: false,
                                paid: String::new(),
                                reference: String::new(),
                            }));
                        }
                    });
                }
            });
        }
        self.payroll_history(ui, r.id);
    }
    pub(super) fn payroll_thirteenth_forms(&mut self, ui: &mut Ui) {
        if !self.data.access.allows(crate::staff_roles::Capability::PayrollPreparation) {
            self.payroll_state.thirteenth.form = None;
            return;
        }
        let Some(mut f) = self.payroll_state.thirteenth.form.take() else {
            return;
        };
        let keep;
        match &mut f {
            Form::Run(f) => {
                ui.heading(if f.revision.is_some() {
                    "Edit draft benefit"
                } else {
                    "Create 13th-month payout"
                });
                ui.add_enabled_ui(f.revision.is_none(), |ui| {
                    field(ui, "Benefit year", &mut f.year);
                    field(ui, "Earned basic through (YYYY-MM-DD)", &mut f.as_of);
                });
                field(ui, "Planned pay date (YYYY-MM-DD)", &mut f.pay);
                field(ui, "Title", &mut f.title);
                note(ui, "Reason", &mut f.reason);
                let record = if f.revision.is_some() {
                    json!({"id":f.id,"expected_revision":f.revision,"pay_date":f.pay,"title":f.title,"reason":f.reason})
                } else {
                    json!({"id":f.id,"year":f.year.parse::<i32>().ok(),"as_of":f.as_of,"pay_date":f.pay,"title":f.title,"reason":f.reason})
                };
                let a = Change::new(
                    if f.revision.is_some() {
                        "thirteenth_run_update"
                    } else {
                        "thirteenth_run_create"
                    },
                    record,
                );
                let (save, cancel) = footer(ui, &a, &self.data, "Save benefit run");
                if save {
                    self.send_payroll(a);
                }
                keep = !cancel;
            }
            Form::Decision(f) => {
                ui.heading(match f.op.as_str() {
                    "run_submit" => "Submit 13th-month pay for approval",
                    "run_approve" => "Approve and issue benefit slips",
                    "run_return" => "Return benefit for correction",
                    "run_cancel" => "Cancel unissued benefit",
                    "run_paid" => "Record benefit payment",
                    _ => "Remove employee from draft",
                });
                ui.label("Changes retain reasons and audit history. Issued slips cannot be overwritten. A different Administrator approves. Recording payment moves no money.");
                if f.op == "run_submit" {
                    ui.checkbox(&mut f.confirmed,"I reviewed included/omitted employees, earned coverage, earlier payments, tax and contribution reconciliation");
                }
                if f.op == "run_paid" {
                    field(ui, "Actual paid date (YYYY-MM-DD)", &mut f.paid);
                    field(ui, "Payment reference", &mut f.reference);
                }
                note(ui, "Reason", &mut f.reason);
                let mut record =
                    json!({"id":f.id,"expected_revision":f.revision,"reason":f.reason});
                if f.op == "run_submit" {
                    record["scope_reviewed"] = json!(f.confirmed);
                }
                if f.op == "run_paid" {
                    record["paid_on"] = json!(f.paid);
                    record["payment_reference"] = json!(f.reference);
                }
                let a = Change::new(&format!("thirteenth_{}", f.op), record);
                let (save, cancel) = footer(ui, &a, &self.data, "Confirm change");
                if save {
                    self.send_payroll(a);
                }
                keep = !cancel;
            }
            Form::Item(f) => {
                keep = self.thirteenth_item_form(ui, f);
            }
        }
        if keep {
            self.payroll_state.thirteenth.form = Some(f);
        }
    }
    fn thirteenth_item_form(&mut self, ui: &mut Ui, f: &mut ItemForm) -> bool {
        ui.heading(format!(
            "{} · {}",
            f.item.employee_code, f.item.employee_name
        ));
        let editable = self
            .data
            .payroll
            .thirteenth
            .runs
            .iter()
            .any(|r| r.id == f.item.run_id && r.status == "draft");
        if !editable {
            ui.label("Saved HR worksheet and source evidence. Return a submitted payout for correction before editing.");
            totals(ui, &f.item.totals, "thirteenth_saved_totals");
            if let Ok(s) = benefit::source(&f.item) {
                ui.label(format!(
                    "Saved earned coverage {} to {}",
                    s.starts_on, s.as_of
                ));
                for p in s.periods {
                    ui.label(format!(
                        "Issued basic {} to {}: {}",
                        p.starts_on,
                        p.ends_on,
                        payroll::money(p.basic_cents)
                    ));
                }
            }
            for o in &f.w.openings {
                ui.label(format!(
                    "Opening {} to {}: {} · {}",
                    o.starts_on,
                    o.ends_on,
                    payroll::money(o.basic_cents),
                    o.reference
                ));
            }
            ui.label(format!(
                "Actual coverage: {} · One month served reviewed: {}",
                f.w.classification.replace('_', " "),
                f.w.served_one_month
            ));
            for (label, n) in [
                ("Basic correction", f.w.basic_adjustment_cents),
                ("Cumulative policy top-up / advance", f.w.policy_topup_cents),
                (
                    "Same-employer external prior13th",
                    f.w.external_thirteenth_cents,
                ),
                ("Other benefits sharing cap", f.w.other_benefits_cents),
            ] {
                ui.label(format!("{label}: {}", payroll::money(n)));
            }
            for (label, t) in [
                ("Coverage evidence", &f.w.eligibility_note),
                ("Basic correction evidence", &f.w.adjustment_note),
                ("Policy / advance evidence", &f.w.policy_note),
                ("Opening payment / benefit evidence", &f.w.opening_note),
                ("Tax review", &f.w.tax_note),
                ("Contribution review", &f.w.contribution_note),
                ("HR review", &f.w.review_note),
            ] {
                if !t.is_empty() {
                    ui.strong(label);
                    ui.label(t);
                }
            }
            self.payroll_history(ui, f.item.id);
            return !ui.button("Back to benefit run").clicked();
        }
        ui.label("Actual current-employer earned basic determines this benefit. Salary rates, rounded accrual memos and prior-employer basic do not increase it. Withholding and contribution adjustments require explicit HR reconciliation; no standalone bonus tax or second full monthly contribution is assumed.");
        if ui
            .button("Refresh sources and require new reviews")
            .clicked()
        {
            self.send_payroll(Change::new("thirteenth_item_refresh",json!({"id":f.item.id,"expected_revision":f.item.revision,"reason":"Refresh annual benefit source and require new reviews"})));
        }
        let src = benefit::source(&f.item);
        if let Ok(s) = &src {
            ui.label(format!(
                "Current-employer coverage {} to {}",
                s.starts_on, s.as_of
            ));
            ui.collapsing("Issued basic included",|ui|{for p in &s.periods{ui.label(format!("{} to {} · {}",p.starts_on,p.ends_on,payroll::money(p.basic_cents)));}if s.periods.is_empty(){ui.label("No issued payroll in this coverage. Review the missing intervals explicitly.");}});
        }
        card(ui, |ui| {
            ui.heading("Opening basic for missing intervals");
            ui.label("Use earned current-employer basic from verified records. Dates cannot overlap issued payroll or other openings. A zero amount still needs its evidence reference.");
            let mut remove = None;
            for (n, o) in f.openings.iter_mut().enumerate() {
                ui.push_id(n, |ui| {
                    ui.separator();
                    ui.horizontal_wrapped(|ui| {
                        ui.strong(format!("Opening {}", n + 1));
                        if ui.button("Remove opening").clicked() {
                            remove = Some(n);
                        }
                    });
                    field(ui, "From (YYYY-MM-DD)", &mut o.start);
                    field(ui, "Through (YYYY-MM-DD)", &mut o.end);
                    amount(ui, "Earned basic (PHP)", &mut o.amount);
                    field(ui, "Evidence / reviewed-zero reference", &mut o.reference);
                });
            }
            if let Some(n) = remove {
                f.openings.remove(n);
            }
            if f.openings.len() < 32 && ui.button("Add opening interval").clicked() {
                let preview = benefit::source(&f.item)
                    .and_then(|s| benefit::calculate(&s, &f.w))
                    .ok();
                let gap = preview.as_ref().and_then(|c| c.coverage_gaps.first());
                f.openings.push(OpeningForm {
                    start: gap.map(|g| g.starts_on.to_string()).unwrap_or_default(),
                    end: gap.map(|g| g.ends_on.to_string()).unwrap_or_default(),
                    amount: 0,
                    reference: String::new(),
                });
            }
        });
        card(ui, |ui| {
            ui.heading("Coverage and employer policy");
            choice(
                ui,
                "thirteenth_classification",
                "Actual coverage",
                &mut f.w.classification,
                &["", "rank_and_file", "company_policy"],
            );
            ui.checkbox(
                &mut f.w.served_one_month,
                "I verified at least one month served during the calendar year",
            );
            note(
                ui,
                "Coverage / actual duties evidence",
                &mut f.w.eligibility_note,
            );
            let mut subtract = f.w.basic_adjustment_cents < 0;
            let mut absolute = f.w.basic_adjustment_cents.abs();
            amount(ui, "Basic correction (PHP)", &mut absolute);
            ui.checkbox(&mut subtract, "Subtract this documented basic correction");
            f.w.basic_adjustment_cents = if subtract { -absolute } else { absolute };
            note(ui, "Basic correction reason", &mut f.w.adjustment_note);
            amount(
                ui,
                "Cumulative employer-funded top-up / advance (PHP)",
                &mut f.w.policy_topup_cents,
            );
            note(
                ui,
                "More favorable policy / employer advance evidence",
                &mut f.w.policy_note,
            );
            ui.label("This is an employer-funded benefit amount. Never collect an employee salary deduction to fund 13th-month pay. Reconcile earlier advances against later earned entitlement.");
        });
        card(ui, |ui| {
            ui.heading("Earlier payments and shared benefit cap");
            amount(
                ui,
                "Same employer's off-system 13th-month already paid (PHP)",
                &mut f.w.external_thirteenth_cents,
            );
            amount(
                ui,
                "Other annual benefits sharing the exemption (PHP)",
                &mut f.w.other_benefits_cents,
            );
            note(
                ui,
                "Opening payment / other-benefit evidence",
                &mut f.w.opening_note,
            );
            ui.label("Issued in-app installments are included automatically. Do not enter them again. Prior-employer benefit facts can use the shared cap; their basic salary does not increase this employer's entitlement.");
            let mut partial = f.w.requested_gross_cents.is_some();
            ui.checkbox(&mut partial, "Issue a partial installment");
            if partial {
                amount(
                    ui,
                    "This gross installment (PHP)",
                    f.w.requested_gross_cents
                        .get_or_insert(f.item.totals.remaining_cents),
                );
            } else {
                f.w.requested_gross_cents = None;
            }
        });
        card(ui, |ui| {
            ui.heading("Withholding and contribution review");
            amount(
                ui,
                "Reviewed supplementary / cumulative withholding (PHP)",
                &mut f.w.withholding_cents,
            );
            note(
                ui,
                "Tax method, opening facts and evidence (including zero)",
                &mut f.w.tax_note,
            );
            ui.checkbox(&mut f.w.tax_reviewed,"I reconciled the shared annual cap and applicable withholding, including previous employment");
            amount(
                ui,
                "SSS month-level adjustment (PHP)",
                &mut f.w.sss_adjustment_cents,
            );
            amount(
                ui,
                "PhilHealth month-level adjustment (PHP)",
                &mut f.w.philhealth_adjustment_cents,
            );
            amount(
                ui,
                "Pag-IBIG month-level adjustment (PHP)",
                &mut f.w.pagibig_adjustment_cents,
            );
            note(
                ui,
                "Contribution reconciliation evidence (including zero)",
                &mut f.w.contribution_note,
            );
            ui.checkbox(&mut f.w.contributions_reviewed,"I reconciled existing monthly contributions and any additional amount; no duplicate full monthly charge");
        });
        let parsed =
            f.openings
                .iter()
                .map(|o| {
                    Ok(benefit::Opening {
                        starts_on: o.start.parse::<NaiveDate>().map_err(|_| {
                            anyhow::anyhow!("Enter each opening start as YYYY-MM-DD.")
                        })?,
                        ends_on: o.end.parse::<NaiveDate>().map_err(|_| {
                            anyhow::anyhow!("Enter each opening end as YYYY-MM-DD.")
                        })?,
                        basic_cents: o.amount,
                        reference: o.reference.clone(),
                    })
                })
                .collect::<anyhow::Result<Vec<_>>>();
        let mut value = None;
        match parsed {
            Ok(o) => {
                f.w.openings = o;
                if let Ok(s) = &src {
                    match benefit::calculate(s, &f.w) {
                        Ok(c) => {
                            ui.heading("Payout preview");
                            totals(ui, &c, "thirteenth_preview");
                            value = Some(json!(f.w));
                        }
                        Err(e) => {
                            ui.colored_label(Color32::DARK_RED, e.to_string());
                        }
                    }
                }
            }
            Err(e) => {
                ui.colored_label(Color32::DARK_RED, e.to_string());
            }
        }
        ui.checkbox(
            &mut f.w.reviewed,
            "I reviewed earned coverage, eligibility, earlier installments and all adjustments",
        );
        note(ui, "HR review", &mut f.w.review_note);
        note(ui, "Reason for saving", &mut f.reason);
        if value.is_some() {
            value = Some(json!(f.w));
            let a = Change::new(
                "thirteenth_item_save",
                json!({"id":f.item.id,"expected_revision":f.item.revision,"worksheet":value,"reason":f.reason}),
            );
            let (save, cancel) = footer(ui, &a, &self.data, "Save benefit worksheet");
            if save {
                self.send_payroll(a);
            }
            if cancel {
                return false;
            }
        } else if ui.button("Discard worksheet changes").clicked() {
            return false;
        }
        self.payroll_history(ui, f.item.id);
        true
    }
    fn thirteenth_slips(&mut self, ui: &mut Ui) {
        let hr = self.data.access.allows(crate::staff_roles::Capability::PayrollPreparation);
        let slips: Vec<_> = self
            .data
            .payroll
            .thirteenth
            .slips
            .iter()
            .filter(|s| hr || Some(s.employee_id) == self.data.access.employee_id)
            .cloned()
            .collect();
        if let Some(s) = slips
            .iter()
            .find(|s| Some(s.id) == self.payroll_state.thirteenth.slip)
        {
            if ui.button("Back to benefit slips").clicked() {
                self.payroll_state.thirteenth.slip = None;
                return;
            }
            card(ui, |ui| {
                ui.heading(&s.company_name);
                ui.strong(format!(
                    "13TH-MONTH PAY · {} · Earned through {}",
                    s.year, s.as_of
                ));
                ui.label(format!(
                    "Planned pay {} · {} · {}",
                    s.pay_date, s.item.employee_code, s.item.employee_name
                ));
                totals(ui, &s.item.totals, "thirteenth_own_totals");
                ui.label("Approved payout calculation; actual payment is recorded separately.");
            });
            ui.horizontal_wrapped(|ui| {
                if ui.button("Save printable PDF").clicked() {
                    self.payroll_export("thirteenth_payslip_pdf", Some(s.id));
                }
                if ui.button("Export benefit payslip CSV").clicked() {
                    self.payroll_export("thirteenth_payslip", Some(s.id));
                }
            });
            return;
        }
        ui.heading("Issued 13th-month slips");
        ui.label("HR/Admin read issued employee slips. Other staff receive only their own. HR reviews and opening evidence stay private.");
        for s in slips {
            card(ui, |ui| {
                if ui
                    .link(format!(
                        "{} · {} · {} · Through {}",
                        s.year, s.item.employee_code, s.item.employee_name, s.as_of
                    ))
                    .clicked()
                {
                    self.payroll_state.thirteenth.slip = Some(s.id);
                }
                ui.label(format!(
                    "Planned pay {} · Net {}",
                    s.pay_date,
                    payroll::money(s.item.totals.net_cents)
                ));
            });
        }
    }
}
