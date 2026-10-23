use super::payroll_view::{ItemForm, employee_choice, field, footer, note, totals};
use super::*;
use crate::payroll::{self, PayrollAction as Change};
use crate::payroll_cutoffs::{self as cutoff, Run};
use serde_json::json;

#[derive(Default)]
pub(super) struct State {
    pub(super) selected: Option<Uuid>,
    slip: Option<Uuid>,
    employee: Option<Uuid>,
    show_slips: bool,
    run: Option<RunForm>,
    decision: Option<Decision>,
}
impl State {
    pub(super) fn editing(&self) -> bool {
        self.run.is_some() || self.decision.is_some()
    }
    pub(super) fn close_forms(&mut self) {
        self.run = None;
        self.decision = None;
    }
}
struct RunForm {
    id: Uuid,
    revision: Option<i64>,
    month: String,
    part: u8,
    pay_date: String,
    title: String,
    reason: String,
}
struct Decision {
    operation: String,
    id: Uuid,
    revision: i64,
    reason: String,
    confirmed: bool,
    paid_on: String,
    payment_reference: String,
}
impl HrisApp {
    #[cfg(feature = "qa-screenshot")]
    pub(super) fn prepare_cutoff_preview(&mut self, form: &str) {
        use crate::payroll::{Component, Day, Pay, Worksheet};
        let employee = self.data.employees[1].id;
        let profile = Uuid::from_u128(99001);
        let r1 = Uuid::from_u128(99010);
        let r2 = Uuid::from_u128(99020);
        let i1 = Uuid::from_u128(99011);
        let i2 = Uuid::from_u128(99021);
        let actor = self.data.access.user_id;
        let apply = |data: &mut Snapshot, op: &str, v: serde_json::Value| {
            payroll::apply_demo(data, &Change::new(op, v)).expect("Fictional cutoff preview");
        };
        apply(
            &mut self.data,
            "profile_save",
            json!({"id":profile,"employee_id":employee,"effective_on":"2026-02-01","pay":Pay{rate_cents:3000000,components:vec![Component{label:"Rice allowance".into(),kind:"rice".into(),amount_cents:250000,reference:String::new()}],..Default::default()},"reason":"Fictional salary profile"}),
        );
        for (part, rid, iid, end) in [(1, r1, i1, "2026-09-15"), (2, r2, i2, "2026-09-30")] {
            apply(
                &mut self.data,
                "cutoff_run_create",
                json!({"id":rid,"month":"2026-09-01","part":part,"pay_date":end,"title":format!("September 2026 · Cutoff {part}"),"reason":"Fictional cutoff"}),
            );
            apply(
                &mut self.data,
                "cutoff_item_add",
                json!({"id":iid,"run_id":rid,"employee_id":employee,"reason":"Fictional employee worksheet"}),
            );
            let w = Worksheet {
                reviewed: true,
                review_note: "Fictional reviewed pay and deductions".into(),
                exception_note: "Fictional reviewed overtime".into(),
                days: vec![Day {
                    date: NaiveDate::from_ymd_opt(2026, 9, if part == 1 { 3 } else { 18 }).unwrap(),
                    kind: "ordinary".into(),
                    regular_minutes: 480,
                    overtime_minutes: 60,
                    night_overtime: 60,
                    ..Default::default()
                }],
                ..Default::default()
            };
            apply(
                &mut self.data,
                "cutoff_item_save",
                json!({"id":iid,"expected_revision":0,"worksheet":w,"reason":"Fictional payroll review"}),
            );
            if part == 1 || form.ends_with("slip") || form.ends_with("manager") {
                let rev = self
                    .data
                    .payroll
                    .cutoffs
                    .runs
                    .iter()
                    .find(|r| r.base.id == rid)
                    .unwrap()
                    .base
                    .revision;
                apply(
                    &mut self.data,
                    "cutoff_run_submit",
                    json!({"id":rid,"expected_revision":rev,"scope_reviewed":true,"reason":"Fictional submission"}),
                );
                self.data.access.user_id = Uuid::from_u128(99004);
                apply(
                    &mut self.data,
                    "cutoff_run_approve",
                    json!({"id":rid,"expected_revision":rev+1,"reason":"Fictional independent approval"}),
                );
                self.data.access.user_id = actor;
            }
        }
        self.payroll_state.cutoffs.selected = Some(r2);
        if form.ends_with("list") {
            self.payroll_state.cutoffs.selected = None;
        }
        if form.ends_with("worksheet") {
            let i = self
                .data
                .payroll
                .cutoffs
                .items
                .iter()
                .find(|i| i.base.id == i2)
                .unwrap()
                .base
                .clone();
            self.payroll_state.item = Some(ItemForm {
                cutoff: Some((NaiveDate::from_ymd_opt(2026, 9, 1).unwrap(), 2)),
                worksheet: i.worksheet.clone(),
                item: i,
                reason: String::new(),
                new_date: "2026-09-16".into(),
            });
        }
        if form.ends_with("create") {
            self.payroll_state.cutoffs.run = Some(RunForm {
                id: Uuid::from_u128(99030),
                revision: None,
                month: "2026-10-01".into(),
                part: 1,
                pay_date: "2026-10-15".into(),
                title: "October 2026 · Cutoff 1".into(),
                reason: String::new(),
            });
        }
        if form.ends_with("approval") {
            let r = self
                .data
                .payroll
                .cutoffs
                .runs
                .iter()
                .find(|r| r.base.id == r2)
                .unwrap()
                .clone();
            self.cutoff_decide(&r, "run_submit");
        }
        if form.ends_with("slip") || form.ends_with("manager") {
            self.payroll_state.cutoffs.show_slips = true;
            self.payroll_state.cutoffs.slip = Some(i2);
            if let Ok(path) = std::env::var("HRIS_CUTOFF_PDF_TO") {
                let s = self
                    .data
                    .payroll
                    .cutoffs
                    .slips
                    .iter()
                    .find(|s| s.base.id == i2)
                    .unwrap();
                crate::payroll_pdf::save(
                    std::path::Path::new(&path),
                    &crate::payroll_pdf::build_cutoff(s).unwrap(),
                )
                .unwrap();
            }
            if form.ends_with("manager") {
                self.data.access.role = Role::Manager;
                self.data.access.employee_id = Some(employee);
                self.data.payroll.cutoffs.runs.clear();
                self.data.payroll.cutoffs.items.clear();
                self.data.payroll.profiles.clear();
                self.data.payroll.events.clear();
            }
        }
    }
    pub(super) fn payroll_cutoffs(&mut self, ui: &mut Ui) {
        let hr = self.data.access.role.manages_hr();
        if !hr {
            self.payroll_state.cutoffs.show_slips = true;
        }
        if hr {
            ui.horizontal_wrapped(|ui| {
                ui.selectable_value(
                    &mut self.payroll_state.cutoffs.show_slips,
                    false,
                    "Cutoff runs",
                );
                ui.selectable_value(
                    &mut self.payroll_state.cutoffs.show_slips,
                    true,
                    "Cutoff payslips",
                );
            });
        }
        ui.add_space(10.0);
        if self.payroll_state.cutoffs.show_slips {
            self.cutoff_slips(ui);
            return;
        }
        if let Some(id) = self.payroll_state.cutoffs.selected {
            if let Some(r) = self
                .data
                .payroll
                .cutoffs
                .runs
                .iter()
                .find(|r| r.base.id == id)
                .cloned()
            {
                self.cutoff_detail(ui, &r);
                return;
            }
            self.payroll_state.cutoffs.selected = None;
        }
        ui.heading("Semi-monthly payroll cutoffs");
        ui.label("1st–15th and16th–month-end, with separate pay dates and approved payslips. Monthly salary and recurring amounts are allocated between halves. Government contributions settle on the second cutoff; withholding uses the semi-monthly table.");
        if ui.button("Create cutoff payroll").clicked() {
            let month = payroll::month_start(self.data.company.today());
            self.payroll_state.cutoffs.run = Some(RunForm {
                id: Uuid::new_v4(),
                revision: None,
                month: month.to_string(),
                part: 1,
                pay_date: month.with_day(15).unwrap().to_string(),
                title: format!("{} cutoff 1", month.format("%B %Y")),
                reason: String::new(),
            });
        }
        let mut runs = self.data.payroll.cutoffs.runs.clone();
        runs.sort_by_key(|r| std::cmp::Reverse((r.base.month, r.part)));
        for r in runs {
            ui.add_space(8.0);
            card(ui, |ui| {
                if ui.link(RichText::new(&r.base.title).strong()).clicked() {
                    self.payroll_state.cutoffs.selected = Some(r.base.id);
                }
                ui.label(format!(
                    "{} to {} · Pay date {}",
                    r.starts_on, r.ends_on, r.pay_date
                ));
                let items: Vec<_> = self
                    .data
                    .payroll
                    .cutoffs
                    .items
                    .iter()
                    .filter(|i| i.base.run_id == r.base.id)
                    .collect();
                ui.horizontal_wrapped(|ui| {
                    ui.label(format!("{} · {} employees", r.base.status, items.len()));
                    ui.strong(payroll::money(
                        items.iter().map(|i| i.base.totals.net).sum(),
                    ));
                });
            });
        }
    }
    fn cutoff_decide(&mut self, r: &Run, operation: &str) {
        self.payroll_state.cutoffs.decision = Some(Decision {
            operation: operation.into(),
            id: r.base.id,
            revision: r.base.revision,
            reason: String::new(),
            confirmed: false,
            paid_on: self.data.company.today().to_string(),
            payment_reference: String::new(),
        });
    }
    fn cutoff_detail(&mut self, ui: &mut Ui, r: &Run) {
        if ui.button("Back to cutoff runs").clicked() {
            self.payroll_state.cutoffs.selected = None;
            return;
        }
        ui.heading(&r.base.title);
        ui.label(format!(
            "{} to {} · Cutoff {} · Planned pay date {} · {}",
            r.starts_on, r.ends_on, r.part, r.pay_date, r.base.status
        ));
        ui.label("First-cutoff contributions are zero. The second cutoff uses the immutable first payslip to settle monthly contributions and exemption caps. New joiners and proration require HR review; missing attendance never creates automatic absence deductions.");
        ui.horizontal_wrapped(|ui| {
            if r.base.status == "draft" {
                if ui.button("Edit title / pay date").clicked() {
                    self.payroll_state.cutoffs.run = Some(RunForm {
                        id: r.base.id,
                        revision: Some(r.base.revision),
                        month: r.base.month.to_string(),
                        part: r.part,
                        pay_date: r.pay_date.to_string(),
                        title: r.base.title.clone(),
                        reason: String::new(),
                    });
                }
                if ui.button("Submit for approval").clicked() {
                    self.cutoff_decide(r, "run_submit");
                }
            }
            if r.base.status == "submitted" {
                if ui.button("Return to draft").clicked() {
                    self.cutoff_decide(r, "run_return");
                }
                if self.data.access.role == Role::Administrator
                    && r.base.submitted_by != Some(self.data.access.user_id)
                    && ui.button("Approve & issue payslips").clicked()
                {
                    self.cutoff_decide(r, "run_approve");
                }
                if r.base.submitted_by == Some(self.data.access.user_id) {
                    ui.label("Another Administrator must approve this submission.");
                }
            }
            if ["draft", "submitted"].contains(&r.base.status.as_str())
                && ui.button("Cancel unissued cutoff").clicked()
            {
                self.cutoff_decide(r, "run_cancel");
            }
            if r.base.status == "approved"
                && self.data.access.role == Role::Administrator
                && ui.button("Record payment").clicked()
            {
                self.cutoff_decide(r, "run_paid");
            }
            if ui.button("Export cutoff register CSV").clicked() {
                self.payroll_export("cutoff_register", Some(r.base.id));
            }
            if ui
                .button("Export issued month reconciliation CSV")
                .clicked()
            {
                self.payroll_export("cutoff_monthly", Some(r.base.id));
            }
        });
        if r.base.status == "paid" {
            ui.label(format!(
                "Recorded payment {} · {}",
                r.base.paid_on.map(|d| d.to_string()).unwrap_or_default(),
                r.base.payment_reference
            ));
        }
        if r.base.status == "draft" {
            card(ui, |ui| {
                employee_choice(
                    ui,
                    &self.data,
                    "cutoff_add_employee",
                    &mut self.payroll_state.cutoffs.employee,
                );
                if let Some(eid) = self.payroll_state.cutoffs.employee {
                    let has = self
                        .data
                        .payroll
                        .cutoffs
                        .items
                        .iter()
                        .any(|i| i.base.run_id == r.base.id && i.base.employee_id == eid);
                    if ui
                        .add_enabled(!has, egui::Button::new("Add employee worksheet"))
                        .clicked()
                    {
                        self.send_payroll(Change::new("cutoff_item_add",json!({"id":Uuid::new_v4(),"run_id":r.base.id,"employee_id":eid,"reason":"Add employee to reviewed cutoff scope"})));
                    }
                }
            });
        }
        for i in self
            .data
            .payroll
            .cutoffs
            .items
            .iter()
            .filter(|i| i.base.run_id == r.base.id)
            .cloned()
            .collect::<Vec<_>>()
        {
            ui.add_space(8.0);
            card(ui, |ui| {
                ui.heading(format!(
                    "{} · {}",
                    i.base.employee_code, i.base.employee_name
                ));
                ui.horizontal_wrapped(|ui| {
                    ui.label(if i.base.worksheet.reviewed {
                        "Reviewed"
                    } else {
                        "Needs review"
                    });
                    ui.strong(format!("Net {}", payroll::money(i.base.totals.net)));
                });
                if !i.base.worksheet.review_note.is_empty() {
                    ui.label(format!("HR review: {}", i.base.worksheet.review_note));
                }
                if !i.base.worksheet.exception_note.is_empty() {
                    ui.label(format!("HR exception: {}", i.base.worksheet.exception_note));
                }
                ui.horizontal_wrapped(|ui| {
                    if r.base.status == "draft" && ui.button("Open worksheet").clicked() {
                        self.payroll_state.item = Some(ItemForm {
                            cutoff: Some((r.base.month, r.part)),
                            worksheet: i.base.worksheet.clone(),
                            item: i.base.clone(),
                            reason: String::new(),
                            new_date: r.starts_on.to_string(),
                        });
                        self.payroll_state.warnings.clear();
                    }
                    if r.base.status == "draft" && ui.button("Remove from draft").clicked() {
                        self.payroll_state.cutoffs.decision = Some(Decision {
                            operation: "item_remove".into(),
                            id: i.base.id,
                            revision: i.base.revision,
                            reason: String::new(),
                            confirmed: false,
                            paid_on: String::new(),
                            payment_reference: String::new(),
                        });
                    }
                    if ["approved", "paid"].contains(&r.base.status.as_str())
                        && ui.button("View cutoff payslip").clicked()
                    {
                        self.payroll_state.cutoffs.show_slips = true;
                        self.payroll_state.cutoffs.slip = Some(i.base.id);
                    }
                });
                ui.collapsing("Calculation totals", |ui| {
                    totals(ui, &i.base.totals, format!("cutoff_total_{}", i.base.id))
                });
            });
        }
        self.payroll_history(ui, r.base.id);
    }
    pub(super) fn payroll_cutoff_forms(&mut self, ui: &mut Ui) {
        if let Some(mut f) = self.payroll_state.cutoffs.run.take() {
            ui.heading(if f.revision.is_some() {
                "Edit draft cutoff"
            } else {
                "Create cutoff payroll"
            });
            ui.add_enabled_ui(f.revision.is_none(), |ui| {
                let old = (f.month.clone(), f.part);
                field(ui, "Month first (YYYY-MM-01)", &mut f.month);
                ui.horizontal_wrapped(|ui| {
                    ui.radio_value(&mut f.part, 1, "1st–15th");
                    ui.radio_value(&mut f.part, 2, "16th–month-end");
                });
                if old != (f.month.clone(), f.part) {
                    if let Ok(m) = f.month.parse() {
                        if let Ok((_, end)) = cutoff::bounds(m, f.part) {
                            f.pay_date = end.to_string();
                        }
                    }
                }
            });
            field(ui, "Planned pay date (YYYY-MM-DD)", &mut f.pay_date);
            field(ui, "Cutoff title", &mut f.title);
            ui.label("The planned date is retained on issued payslips. Actual payments are recorded separately. Review the company's legal and contractual pay schedule.");
            note(ui, "Reason", &mut f.reason);
            let a = Change::new(
                if f.revision.is_none() {
                    "cutoff_run_create"
                } else {
                    "cutoff_run_update"
                },
                if f.revision.is_none() {
                    json!({"id":f.id,"month":f.month,"part":f.part,"pay_date":f.pay_date,"title":f.title,"reason":f.reason})
                } else {
                    json!({"id":f.id,"expected_revision":f.revision,"pay_date":f.pay_date,"title":f.title,"reason":f.reason})
                },
            );
            let (save, cancel) = footer(ui, &a, &self.data, "Save cutoff");
            if save {
                self.send_payroll(a);
            }
            if !cancel {
                self.payroll_state.cutoffs.run = Some(f);
            }
            return;
        }
        if let Some(mut f) = self.payroll_state.cutoffs.decision.take() {
            ui.heading(match f.operation.as_str() {
                "run_submit" => "Submit cutoff for approval",
                "run_approve" => "Approve cutoff and issue payslips",
                "run_return" => "Return cutoff for correction",
                "run_cancel" => "Cancel draft cutoff",
                "run_paid" => "Record cutoff payment",
                "item_remove" => "Remove employee from draft",
                _ => "Review cutoff change",
            });
            ui.label("Changes retain their reason and audit history. Issued payslips are immutable; recording payment does not move money.");
            if f.operation == "run_submit" {
                ui.checkbox(&mut f.confirmed,"I reviewed included and omitted employees, both cutoff amounts, sources and planned pay date");
            }
            if f.operation == "run_paid" {
                field(ui, "Actual paid date (YYYY-MM-DD)", &mut f.paid_on);
                field(ui, "Payment reference", &mut f.payment_reference);
            }
            note(ui, "Reason", &mut f.reason);
            let mut record = json!({"id":f.id,"expected_revision":f.revision,"reason":f.reason});
            if f.operation == "run_submit" {
                record["scope_reviewed"] = json!(f.confirmed);
            }
            if f.operation == "run_paid" {
                record["paid_on"] = json!(f.paid_on);
                record["payment_reference"] = json!(f.payment_reference);
            }
            let a = Change::new(&format!("cutoff_{}", f.operation), record);
            let (save, cancel) = footer(ui, &a, &self.data, "Confirm change");
            if save {
                self.send_payroll(a);
            }
            if !cancel {
                self.payroll_state.cutoffs.decision = Some(f);
            }
        }
    }
    fn cutoff_slips(&mut self, ui: &mut Ui) {
        if let Some(id) = self.payroll_state.cutoffs.slip {
            if let Some(s) = self
                .data
                .payroll
                .cutoffs
                .slips
                .iter()
                .find(|s| s.base.id == id)
                .cloned()
            {
                if ui.button("Back to cutoff payslips").clicked() {
                    self.payroll_state.cutoffs.slip = None;
                    return;
                }
                card(ui, |ui| {
                    ui.heading(&s.base.company_name);
                    ui.strong(format!("PAYSLIP · {} to {}", s.starts_on, s.ends_on));
                    ui.label(format!(
                        "Cutoff {} · Planned pay date {}",
                        s.part, s.pay_date
                    ));
                    ui.label(format!(
                        "{} · {} · {}",
                        s.base.item.employee_code,
                        s.base.item.employee_name,
                        s.base.item.department
                    ));
                    totals(ui, &s.base.item.totals, "cutoff_slip_totals");
                    ui.label("Approved calculation. Actual payment is recorded separately.");
                });
                ui.horizontal_wrapped(|ui| {
                    if ui.button("Save printable PDF").clicked() {
                        self.payroll_export("cutoff_payslip_pdf", Some(id));
                    }
                    if ui.button("Export cutoff payslip CSV").clicked() {
                        self.payroll_export("cutoff_payslip", Some(id));
                    }
                });
                return;
            }
            self.payroll_state.cutoffs.slip = None;
        }
        ui.heading("Cutoff payslips");
        ui.label("HR/Admin can read issued employee slips. Other staff see only their own. Draft salaries and HR notes stay private.");
        let mut slips = self.data.payroll.cutoffs.slips.clone();
        slips.sort_by_key(|s| std::cmp::Reverse((s.base.month, s.part)));
        for s in slips {
            card(ui, |ui| {
                if ui
                    .link(format!(
                        "{} to {} · {} · {}",
                        s.starts_on,
                        s.ends_on,
                        s.base.item.employee_code,
                        s.base.item.employee_name
                    ))
                    .clicked()
                {
                    self.payroll_state.cutoffs.slip = Some(s.base.id);
                }
                ui.label(format!(
                    "Planned pay {} · Net {}",
                    s.pay_date,
                    payroll::money(s.base.item.totals.net)
                ));
            });
        }
    }
}
