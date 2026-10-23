use super::payroll_view::{amount, choice, employee_choice, field, footer, note};
use super::*;
use crate::{
    payroll::{self, PayrollAction as Change},
    payroll_tax as tax,
};
use serde_json::json;
#[derive(Default)]
pub(super) struct State {
    selected: Option<Uuid>,
    slips: bool,
    slip: Option<Uuid>,
    form: Option<Form>,
}
enum Form {
    New(NewForm),
    Worksheet(WorksheetForm),
    Decision(Decision),
}
struct NewForm {
    id: Uuid,
    employee: Option<Uuid>,
    year: String,
    kind: String,
    as_of: String,
    end: String,
    title: String,
    reason: String,
}
struct WorksheetForm {
    record: tax::Record,
    w: tax::Worksheet,
    openings: Vec<OpeningForm>,
    previous: Vec<PreviousForm>,
    reason: String,
}
struct OpeningForm {
    start: String,
    end: String,
    row: tax::Opening,
}
struct PreviousForm {
    start: String,
    end: String,
    row: tax::PreviousEmployer,
}
struct Decision {
    id: Uuid,
    revision: i64,
    op: String,
    reason: String,
    confirmed: bool,
    date: String,
    reference: String,
}
impl State {
    pub(super) fn editing(&self) -> bool {
        self.form.is_some()
    }
    pub(super) fn accept(&mut self, a: &Change, data: &Snapshot) {
        let id = a.record["id"].as_str().and_then(|s| s.parse().ok());
        if a.operation == "tax_refresh" {
            if let Some(Form::Worksheet(f)) = self.form.as_mut() {
                if let Some(r) = data.payroll.tax.records.iter().find(|r| Some(r.id) == id) {
                    f.record = r.clone();
                    f.w.final_compensation_reviewed = false;
                    f.w.previous_employers_reviewed = false;
                    f.w.benefits_reviewed = false;
                    f.w.review_note.clear();
                    f.reason.clear();
                    return;
                }
            }
        }
        if a.operation == "tax_create" {
            self.selected = id;
        }
        self.form = None;
    }
}
fn worksheet(r: tax::Record) -> WorksheetForm {
    WorksheetForm {
        w: r.worksheet.clone(),
        openings: r
            .worksheet
            .openings
            .iter()
            .map(|o| OpeningForm {
                start: o.starts_on.to_string(),
                end: o.ends_on.to_string(),
                row: o.clone(),
            })
            .collect(),
        previous: r
            .worksheet
            .previous_employers
            .iter()
            .map(|o| PreviousForm {
                start: o.starts_on.to_string(),
                end: o.ends_on.to_string(),
                row: o.clone(),
            })
            .collect(),
        record: r,
        reason: String::new(),
    }
}
fn numbers(ui: &mut Ui, c: &tax::Calculation) {
    for (label, n) in tax::totals_rows(c) {
        ui.horizontal_top(|ui| {
            let width = (ui.available_width() * 0.62).min(370.0);
            ui.with_layout(Layout::top_down(Align::Min), |ui| {
                ui.set_width(width);
                ui.set_min_height(20.0);
                ui.label(label);
            });
            ui.label(RichText::new(payroll::money(n)).strong());
        });
    }
    for g in &c.coverage_gaps {
        ui.colored_label(
            Color32::DARK_RED,
            format!(
                "Missing paid/opening coverage: {} to {}",
                g.starts_on, g.ends_on
            ),
        );
    }
    if c.unreconciled_benefits_cents > 0 {
        ui.colored_label(
            Color32::DARK_RED,
            format!(
                "Earlier benefit facts to reconcile: {}",
                payroll::money(c.unreconciled_benefits_cents)
            ),
        );
    }
}
fn signed_amount(ui: &mut Ui, label: &str, n: &mut i64) {
    ui.horizontal_wrapped(|ui| {
        ui.label(label);
        ui.add(
            egui::DragValue::new(n)
                .speed(100.0)
                .range(-tax::LIMIT..=tax::LIMIT)
                .custom_formatter(|v, _| format!("{:.2}", v / 100.0))
                .custom_parser(|s| {
                    let s = s.trim();
                    let negative = s.starts_with('-');
                    let s = s.strip_prefix('-').unwrap_or(s);
                    let mut p = s.split('.');
                    let major = p.next()?.parse::<i64>().ok()?;
                    let minor = p.next().unwrap_or("");
                    if p.next().is_some()
                        || minor.len() > 2
                        || !minor.bytes().all(|b| b.is_ascii_digit())
                    {
                        return None;
                    }
                    let v = major.checked_mul(100)?.checked_add(if minor.is_empty() {
                        0
                    } else {
                        minor.parse::<i64>().ok()? * if minor.len() == 1 { 10 } else { 1 }
                    })?;
                    Some((if negative { -v } else { v }) as f64)
                }),
        );
    });
}
impl HrisApp {
    pub(super) fn payroll_tax(&mut self, ui: &mut Ui) {
        let hr = self.data.access.role.manages_hr();
        if !hr {
            self.payroll_state.tax.slips = true;
            self.payroll_state.tax.form = None;
        }
        ui.horizontal_wrapped(|ui| {
            if hr {
                ui.selectable_value(&mut self.payroll_state.tax.slips, false, "Reconciliations");
            }
            ui.selectable_value(
                &mut self.payroll_state.tax.slips,
                true,
                if hr {
                    "Issued tax advice"
                } else {
                    "My tax advice"
                },
            );
        });
        ui.label("Annual compensation tax advice. Settlement is recorded separately; no automatic deduction, refund transfer or government filing.");
        ui.add_space(8.0);
        if self.payroll_state.tax.slips {
            self.tax_slips(ui);
            return;
        }
        if let Some(r) = self
            .data
            .payroll
            .tax
            .records
            .iter()
            .find(|r| Some(r.id) == self.payroll_state.tax.selected)
            .cloned()
        {
            if ui.button("Back to reconciliations").clicked() {
                self.payroll_state.tax.selected = None;
                return;
            }
            card(ui, |ui| {
                ui.heading(&r.title);
                ui.label(format!(
                    "{} · {} · {} · {} through {}",
                    r.employee_code,
                    r.employee_name,
                    r.status,
                    r.kind.replace('_', " "),
                    r.as_of
                ));
                numbers(ui, &r.totals);
                ui.horizontal_wrapped(|ui| {
                    if ui
                        .button(if r.status == "draft" {
                            "Edit worksheet"
                        } else {
                            "View saved worksheet & evidence"
                        })
                        .clicked()
                    {
                        self.payroll_state.tax.form = Some(Form::Worksheet(worksheet(r.clone())));
                    }
                    if r.status == "draft" && ui.button("Submit for approval").clicked() {
                        self.tax_decide(&r, "submit");
                    }
                    if r.status == "submitted" {
                        if ui
                            .add_enabled(
                                self.data.access.role == Role::Administrator
                                    && r.submitted_by != Some(self.data.access.user_id),
                                egui::Button::new("Approve tax advice"),
                            )
                            .clicked()
                        {
                            self.tax_decide(&r, "approve");
                        }
                        if ui.button("Return to draft").clicked() {
                            self.tax_decide(&r, "return");
                        }
                    }
                    if ["draft", "submitted"].contains(&r.status.as_str())
                        && ui.button("Cancel close").clicked()
                    {
                        self.tax_decide(&r, "cancel");
                    }
                    if r.status == "approved"
                        && self.data.access.role == Role::Administrator
                        && ui.button("Record settlement").clicked()
                    {
                        self.tax_decide(&r, "settle");
                    }
                    if ui.button("Export reconciliation CSV").clicked() {
                        self.payroll_export("tax_register", Some(r.id));
                    }
                });
                if r.status == "settled" {
                    ui.label(format!(
                        "Settlement recorded {} · {}",
                        r.settled_on.unwrap(),
                        r.settlement_reference
                    ));
                }
            });
            self.payroll_history(ui, r.id);
            return;
        }
        if ui.button("New annual tax reconciliation").clicked() {
            let year = self.data.company.today().year();
            self.payroll_state.tax.form = Some(Form::New(NewForm {
                id: Uuid::new_v4(),
                employee: None,
                year: year.to_string(),
                kind: "year_end".into(),
                as_of: format!("{year}-12-31"),
                end: format!("{year}-12-31"),
                title: format!("{year} annual compensation tax"),
                reason: String::new(),
            }));
        }
        if self.data.payroll.tax.records.is_empty() {
            ui.label("No annual tax closes yet. Prepare a year-end draft, or an evidenced termination close after final paid compensation is recorded.");
        }
        for r in self.data.payroll.tax.records.clone().iter().rev() {
            card(ui, |ui| {
                if ui
                    .link(format!("{} · {}", r.employee_code, r.employee_name))
                    .clicked()
                {
                    self.payroll_state.tax.selected = Some(r.id);
                }
                ui.label(format!(
                    "{} · {} · as of {} · {}",
                    r.title,
                    r.kind.replace('_', " "),
                    r.as_of,
                    r.status
                ));
                ui.label(format!(
                    "Additional withholding {} · Refund {}",
                    payroll::money(r.totals.additional_withholding_cents),
                    payroll::money(r.totals.refund_cents)
                ));
            });
        }
        if !self.data.payroll.tax.records.is_empty()
            && ui.button("Export all reconciliation rows").clicked()
        {
            self.payroll_export("tax_register", None);
        }
    }
    fn tax_decide(&mut self, r: &tax::Record, op: &str) {
        self.payroll_state.tax.form = Some(Form::Decision(Decision {
            id: r.id,
            revision: r.revision,
            op: op.into(),
            reason: String::new(),
            confirmed: false,
            date: self.data.company.today().to_string(),
            reference: String::new(),
        }));
    }
    pub(super) fn payroll_tax_forms(&mut self, ui: &mut Ui) {
        if !self.data.access.role.manages_hr() {
            self.payroll_state.tax.form = None;
            return;
        }
        let Some(mut form) = self.payroll_state.tax.form.take() else {
            return;
        };
        let keep;
        match &mut form {
            Form::New(f) => {
                ui.heading("New annual tax reconciliation");
                employee_choice(ui, &self.data, "tax_employee", &mut f.employee);
                field(ui, "Tax year", &mut f.year);
                choice(
                    ui,
                    "tax_kind",
                    "Close type",
                    &mut f.kind,
                    &["year_end", "termination"],
                );
                field(ui, "Service end (YYYY-MM-DD)", &mut f.end);
                field(ui, "Payments received as of (YYYY-MM-DD)", &mut f.as_of);
                ui.label("Year-end uses 31 December. Termination dates require HR evidence; this close does not archive the employee or disable their account.");
                field(ui, "Title", &mut f.title);
                note(ui, "Reason / close context", &mut f.reason);
                let a = Change::new(
                    "tax_create",
                    json!({"id":f.id,"employee_id":f.employee,"year":f.year.parse::<i32>().ok(),"kind":f.kind,"as_of":f.as_of,"service_end":f.end,"title":f.title,"reason":f.reason}),
                );
                let (save, cancel) = footer(ui, &a, &self.data, "Create reconciliation");
                if save {
                    self.send_payroll(a);
                }
                keep = !cancel;
            }
            Form::Worksheet(f) => {
                keep = self.tax_worksheet(ui, f);
            }
            Form::Decision(f) => {
                ui.heading(format!(
                    "{} annual tax reconciliation",
                    f.op.replace('_', " ")
                ));
                if let Some(r) = self.data.payroll.tax.records.iter().find(|r| r.id == f.id) {
                    numbers(ui, &r.totals);
                    if f.op == "submit" || f.op == "approve" {
                        if let Err(e) = tax::source(r).and_then(|s| tax::ready(&s, &r.worksheet)) {
                            ui.colored_label(Color32::DARK_RED, e.to_string());
                        }
                    }
                }
                if f.op == "settle" {
                    ui.label("Record only an actual withholding/refund or employer-settled adjustment. This button moves no money and files no return.");
                    field(ui, "Actual settlement date", &mut f.date);
                    note(
                        ui,
                        "Payment / payroll settlement reference",
                        &mut f.reference,
                    );
                }
                note(ui, "Reason", &mut f.reason);
                ui.checkbox(
                    &mut f.confirmed,
                    "I reviewed this action and its saved evidence.",
                );
                let mut v = json!({"id":f.id,"expected_revision":f.revision,"reason":f.reason});
                if f.op == "settle" {
                    v["settled_on"] = json!(f.date);
                    v["settlement_reference"] = json!(f.reference);
                }
                let a = Change::new(&format!("tax_{}", f.op), v);
                let (mut save, mut cancel) = (false, false);
                ui.add_enabled_ui(f.confirmed, |ui| {
                    (save, cancel) = footer(ui, &a, &self.data, "Confirm");
                });
                if !f.confirmed {
                    cancel = ui.button("Back").clicked();
                }
                if save {
                    self.send_payroll(a);
                }
                keep = !cancel;
            }
        }
        if keep {
            self.payroll_state.tax.form = Some(form);
        }
    }
    fn tax_worksheet(&mut self, ui: &mut Ui, f: &mut WorksheetForm) -> bool {
        ui.heading(format!("Annual tax · {}", f.record.employee_name));
        let editable = f.record.status == "draft";
        ui.label(format!(
            "{} · Service end {} · Payment as of {} · {}",
            f.record.year, f.record.service_end, f.record.as_of, f.record.status
        ));
        let src = match tax::source(&f.record) {
            Ok(s) => s,
            Err(e) => {
                ui.label(e.to_string());
                return !ui.button("Back").clicked();
            }
        };
        card(ui, |ui| {
            ui.strong("Protected paid-payroll source");
            ui.label(format!("{} included payments · {} unrecorded payments · {} earlier unsettled tax adjustments",src.income.len(),src.pending_payments,src.pending_adjustments));
            ui.collapsing("Included paid taxable / withholding facts", |ui| {
                for p in &src.income {
                    ui.label(format!(
                        "{} · taxable {} · withheld {}",
                        p.starts_on
                            .zip(p.ends_on)
                            .map(|(a, b)| format!("{a} to {b}"))
                            .unwrap_or_else(|| "Supplementary / cash-only payment".into()),
                        payroll::money(p.taxable_cents),
                        payroll::money(p.withholding_cents)
                    ));
                }
            });
            if editable
                && ui
                    .button("Refresh paid sources and require new review")
                    .clicked()
            {
                self.send_payroll(Change::new("tax_refresh",json!({"id":f.record.id,"expected_revision":f.record.revision,"reason":"Refresh paid income and annual tax source facts"})));
            }
        });
        if !editable {
            ui.label("Saved worksheet and private evidence for independent HR review.");
            card(ui, |ui| {
                ui.heading("Saved current-employer openings");
                if f.w.openings.is_empty() {
                    ui.label("No off-system current-employer opening intervals.");
                }
                for o in &f.w.openings {
                    ui.separator();
                    ui.strong(format!("{} to {}", o.starts_on, o.ends_on));
                    ui.label(format!(
                        "Taxable {} · Net withholding {}",
                        payroll::money(o.taxable_cents),
                        payroll::money(o.withholding_cents)
                    ));
                    ui.label(format!(
                        "Included shared benefits: gross {} · Exempt {}",
                        payroll::money(o.benefit_gross_cents),
                        payroll::money(o.benefit_exempt_cents)
                    ));
                    ui.label(format!("Evidence: {}", o.reference));
                }
            });
            card(ui, |ui| {
                ui.heading("Saved prior-employer certificates");
                if f.w.previous_employers.is_empty() {
                    ui.label("No prior-employer certificate rows. Confirm the saved no-previous-employer review.");
                }
                for o in &f.w.previous_employers {
                    ui.separator();
                    ui.strong(&o.employer);
                    ui.label(format!("{} to {}", o.starts_on, o.ends_on));
                    ui.label(format!(
                        "Certified taxable {} · Net withholding {}",
                        payroll::money(o.taxable_cents),
                        payroll::money(o.withholding_cents)
                    ));
                    ui.label(format!(
                        "Included shared benefits: gross {} · Exempt {}",
                        payroll::money(o.benefit_gross_cents),
                        payroll::money(o.benefit_exempt_cents)
                    ));
                    ui.label(format!("Evidence: {}", o.reference));
                }
            });
            card(ui, |ui| {
                ui.heading("Saved classification and settlement review");
                ui.label(format!(
                    "Benefits reclassified from regular taxable figures: {}",
                    payroll::money(f.w.reclassified_benefit_cents)
                ));
                if !f.w.classification_note.is_empty() {
                    ui.label(&f.w.classification_note);
                }
                ui.label(format!(
                    "Taxable correction: {}",
                    payroll::money(f.w.taxable_adjustment_cents)
                ));
                if !f.w.adjustment_note.is_empty() {
                    ui.label(&f.w.adjustment_note);
                }
                ui.label(format!(
                    "Reviewed available last-pay cash: {}",
                    payroll::money(f.w.last_pay_available_cents)
                ));
                ui.label(&f.w.settlement_note);
                ui.label(&f.w.review_note);
                ui.label(format!("Final compensation reviewed: {} · Prior employers reviewed: {} · Benefits reviewed: {}",f.w.final_compensation_reviewed,f.w.previous_employers_reviewed,f.w.benefits_reviewed));
            });
            card(ui, |ui| {
                ui.heading("Saved calculation");
                numbers(ui, &f.record.totals);
            });
            return !ui.button("Back to reconciliation").clicked();
        }
        ui.add_enabled_ui(editable,|ui|{
            card(ui,|ui|{ui.heading("Current-employer opening ledger");ui.label("Only paid off-system intervals missing from the protected source. Taxable already includes any taxable benefit portion and is net of mandatory employee contributions. Every interval, including zero, needs evidence.");
                let mut remove=None;for(n,o)in f.openings.iter_mut().enumerate(){ui.push_id(("tax_opening",n),|ui|{ui.separator();field(ui,"Start (YYYY-MM-DD)",&mut o.start);field(ui,"End (YYYY-MM-DD)",&mut o.end);
                    amount(ui,"Taxable compensation",&mut o.row.taxable_cents);amount(ui,"Net withholding after refunds",&mut o.row.withholding_cents);amount(ui,"Included shared benefits gross",&mut o.row.benefit_gross_cents);amount(ui,"Included shared benefits exempt",&mut o.row.benefit_exempt_cents);note(ui,"Private evidence reference",&mut o.row.reference);if ui.button("Remove opening interval").clicked(){remove=Some(n);}
                });}if let Some(n)=remove{f.openings.remove(n);}
                if f.openings.len()<32&&ui.button("Add opening interval").clicked(){f.openings.push(OpeningForm{start:src.starts_on.to_string(),end:src.service_end.to_string(),row:tax::Opening{starts_on:src.starts_on,ends_on:src.service_end,taxable_cents:0,withholding_cents:0,benefit_gross_cents:0,benefit_exempt_cents:0,reference:String::new()}});}
            });
            card(ui,|ui|{ui.heading("Successive prior employers");ui.label("Use same-year certified 2316 totals, including taxable benefits; withholding must be net of the former employer's refund. Concurrent employers need separate tax review.");
                let mut remove=None;for(n,o)in f.previous.iter_mut().enumerate(){ui.push_id(("tax_previous",n),|ui|{ui.separator();field(ui,"Employer / certificate label",&mut o.row.employer);field(ui,"Employment start",&mut o.start);field(ui,"Employment end",&mut o.end);
                    amount(ui,"Certified taxable compensation",&mut o.row.taxable_cents);amount(ui,"Certified net withholding",&mut o.row.withholding_cents);amount(ui,"Included shared benefits gross",&mut o.row.benefit_gross_cents);amount(ui,"Included shared benefits exempt",&mut o.row.benefit_exempt_cents);note(ui,"Private certificate evidence reference",&mut o.row.reference);if ui.button("Remove prior employer").clicked(){remove=Some(n);}
                });}if let Some(n)=remove{f.previous.remove(n);}
                if f.previous.len()<8&&ui.button("Add previous employer").clicked(){let start=NaiveDate::from_ymd_opt(src.as_of.year(),1,1).unwrap();let end=src.starts_on.pred_opt().unwrap().max(start);f.previous.push(PreviousForm{start:start.to_string(),end:end.to_string(),row:tax::PreviousEmployer{starts_on:start,ends_on:end,employer:String::new(),taxable_cents:0,withholding_cents:0,benefit_gross_cents:0,benefit_exempt_cents:0,reference:String::new()}});}
            });
            card(ui,|ui|{ui.heading("Benefit classification and settlement review");
                amount(ui,"Shared benefits already included in regular taxable figures",&mut f.w.reclassified_benefit_cents);note(ui,"Classification evidence; no extra income is added",&mut f.w.classification_note);
                signed_amount(ui,"Annual taxable correction (+ / -)",&mut f.w.taxable_adjustment_cents);note(ui,"Correction evidence",&mut f.w.adjustment_note);
                amount(ui,"Last-pay cash available for employer-applied withholding",&mut f.w.last_pay_available_cents);note(ui,"Final compensation / termination evidence and settlement arrangements",&mut f.w.settlement_note);
                ui.checkbox(&mut f.w.final_compensation_reviewed,"Final actual compensation, payment dates and missing intervals reviewed.");
                ui.checkbox(&mut f.w.previous_employers_reviewed,"Prior-employer certificates reviewed, or confirmed no prior employer.");
                ui.checkbox(&mut f.w.benefits_reviewed,"Single annual 90,000 benefit exclusion and classifications reviewed.");
                note(ui,"Annual close review",&mut f.w.review_note);
            });
        });
        let prepare = (|| -> anyhow::Result<tax::Worksheet> {
            let mut w = f.w.clone();
            w.openings = f
                .openings
                .iter()
                .map(|o| {
                    let mut row = o.row.clone();
                    row.starts_on = o.start.parse()?;
                    row.ends_on = o.end.parse()?;
                    Ok(row)
                })
                .collect::<anyhow::Result<_>>()?;
            w.previous_employers = f
                .previous
                .iter()
                .map(|o| {
                    let mut row = o.row.clone();
                    row.starts_on = o.start.parse()?;
                    row.ends_on = o.end.parse()?;
                    Ok(row)
                })
                .collect::<anyhow::Result<_>>()?;
            Ok(w)
        })();
        match prepare {
            Ok(w) => {
                match tax::calculate(&src, &w) {
                    Ok(c) => {
                        card(ui, |ui| {
                            ui.heading(if editable {
                                "Calculated preview"
                            } else {
                                "Saved calculation"
                            });
                            numbers(ui, &c);
                        });
                        if editable {
                            if let Err(e) = tax::ready(&src, &w) {
                                ui.label(RichText::new(e.to_string()).color(muted(ui)));
                            }
                        }
                    }
                    Err(e) => {
                        ui.colored_label(Color32::DARK_RED, e.to_string());
                    }
                }
                if editable {
                    note(ui, "Reason for worksheet edit", &mut f.reason);
                    let a = Change::new(
                        "tax_save",
                        json!({"id":f.record.id,"expected_revision":f.record.revision,"worksheet":w,"reason":f.reason}),
                    );
                    let (save, cancel) = footer(ui, &a, &self.data, "Save annual tax worksheet");
                    if save {
                        self.send_payroll(a);
                    }
                    return !cancel;
                }
            }
            Err(e) => {
                ui.colored_label(Color32::DARK_RED, format!("Use YYYY-MM-DD dates: {e}"));
            }
        }
        !ui.button("Back to reconciliation").clicked()
    }
    fn tax_slips(&mut self, ui: &mut Ui) {
        let hr = self.data.access.role.manages_hr();
        let slips: Vec<_> = self
            .data
            .payroll
            .tax
            .slips
            .iter()
            .filter(|s| {
                s.company_id == self.data.company.id
                    && (hr || Some(s.employee_id) == self.data.access.employee_id)
            })
            .cloned()
            .collect();
        if let Some(s) = slips
            .iter()
            .find(|s| Some(s.id) == self.payroll_state.tax.slip)
        {
            if ui.button("Back to issued tax advice").clicked() {
                self.payroll_state.tax.slip = None;
                return;
            }
            card(ui, |ui| {
                ui.heading(format!("{} annual compensation tax", s.year));
                ui.label(format!(
                    "{} · {} · {}",
                    s.employee_code,
                    s.employee_name,
                    s.kind.replace('_', " ")
                ));
                ui.label(format!(
                    "Payment as of {} · Service end {}",
                    s.as_of, s.service_end
                ));
                numbers(ui, &s.totals);
                ui.label("Issued tax advice; settlement recorded separately. No automatic deduction, refund transfer or government filing.");
                ui.horizontal_wrapped(|ui| {
                    if ui.button("Download printable PDF").clicked() {
                        self.payroll_export("tax_payslip_pdf", Some(s.id));
                    }
                    if ui.button("Download CSV").clicked() {
                        self.payroll_export("tax_payslip", Some(s.id));
                    }
                });
            });
            return;
        }
        if slips.is_empty() {
            ui.label("No issued annual tax advice available.");
        }
        for s in slips.iter().rev() {
            card(ui, |ui| {
                if ui
                    .link(format!(
                        "{} · {} · {}",
                        s.year, s.employee_code, s.employee_name
                    ))
                    .clicked()
                {
                    self.payroll_state.tax.slip = Some(s.id);
                }
                ui.label(format!(
                    "As of {} · Additional withholding {} · Refund {}",
                    s.as_of,
                    payroll::money(s.totals.additional_withholding_cents),
                    payroll::money(s.totals.refund_cents)
                ));
            });
        }
    }
}

#[cfg(feature = "qa-screenshot")]
impl HrisApp {
    pub(super) fn prepare_tax_preview(&mut self, form: &str) {
        let mut s = DemoStore::new();
        let eid = s.data.employees[1].id;
        s.data.employees[1].joined_on = Some("2026-07-01".parse().unwrap());
        let id = Uuid::from_u128(99700);
        let apply = |s: &mut DemoStore, op: &str, v: serde_json::Value| {
            s.execute(Action::Payroll(Change::new(op, v)))
                .expect("Fictional annual tax preview");
        };
        apply(
            &mut s,
            "tax_create",
            json!({"id":id,"employee_id":eid,"year":2026,"kind":"termination","as_of":"2026-09-30","service_end":"2026-09-30","title":"2026 annual compensation reconciliation","reason":"Fictional tax close; no actual termination"}),
        );
        if !form.ends_with("missing") {
            let w=tax::Worksheet{openings:vec![tax::Opening{starts_on:"2026-07-01".parse().unwrap(),ends_on:"2026-09-30".parse().unwrap(),taxable_cents:30000000,withholding_cents:1500000,benefit_gross_cents:0,benefit_exempt_cents:0,reference:"Fictional reviewed current-employer ledger".into()}],previous_employers:if form.ends_with("refund"){vec![]}else{vec![tax::PreviousEmployer{starts_on:"2026-01-01".parse().unwrap(),ends_on:"2026-06-30".parse().unwrap(),employer:"Fictional Previous Employer Inc.".into(),taxable_cents:30000000,withholding_cents:1500000,benefit_gross_cents:6000000,benefit_exempt_cents:6000000,reference:"Fictional certified2316 totals, after prior refund".into()}]},last_pay_available_cents:500000,settlement_note:"Fictional service-end evidence and reviewed settlement arrangements. No money is transferred.".into(),review_note:"Fictional actual compensation and shared annual exclusion reviewed.".into(),final_compensation_reviewed:true,previous_employers_reviewed:true,benefits_reviewed:true,..Default::default()};
            apply(
                &mut s,
                "tax_save",
                json!({"id":id,"expected_revision":0,"worksheet":w,"reason":"Fictional annual source review"}),
            );
        }
        if form.ends_with("saved")
            || form.ends_with("slip")
            || form.ends_with("manager")
            || form.ends_with("refund")
        {
            let rev = s.data.payroll.tax.records[0].revision;
            apply(
                &mut s,
                "tax_submit",
                json!({"id":id,"expected_revision":rev,"reason":"Fictional independent submission"}),
            );
        }
        if form.ends_with("slip") || form.ends_with("manager") || form.ends_with("refund") {
            s.data.access.user_id = Uuid::from_u128(99701);
            let rev = s.data.payroll.tax.records[0].revision;
            apply(
                &mut s,
                "tax_approve",
                json!({"id":id,"expected_revision":rev,"reason":"Fictional independent approval"}),
            );
            if let Ok(path) = std::env::var("HRIS_TAX_PDF_TO") {
                std::fs::write(
                    path,
                    crate::payroll_pdf::build_tax(&s.data.payroll.tax.slips[0]).unwrap(),
                )
                .unwrap();
            }
        }
        if form.ends_with("manager") {
            s.data.access.role = Role::Manager;
            s.data.access.employee_id = Some(eid);
        }
        self.data = s.data;
        self.payroll_state.tax = State::default();
        let r = self.data.payroll.tax.records[0].clone();
        if form.ends_with("detail") {
            self.payroll_state.tax.selected = Some(id);
        }
        if form.ends_with("worksheet") || form.ends_with("missing") || form.ends_with("saved") {
            self.payroll_state.tax.selected = Some(id);
            self.payroll_state.tax.form = Some(Form::Worksheet(worksheet(r.clone())));
        }
        if form.ends_with("approval") {
            self.tax_decide(&r, "submit");
        }
        if form.ends_with("create") {
            self.payroll_state.tax.form = Some(Form::New(NewForm {
                id: Uuid::from_u128(99702),
                employee: Some(eid),
                year: "2026".into(),
                kind: "year_end".into(),
                as_of: "2026-12-31".into(),
                end: "2026-12-31".into(),
                title: "2026 year-end compensation tax".into(),
                reason: String::new(),
            }));
        }
        if form.ends_with("slip") || form.ends_with("manager") || form.ends_with("refund") {
            self.payroll_state.tax.slips = true;
            self.payroll_state.tax.slip = Some(id);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn tax_source_refresh_keeps_open_edits_and_requires_new_reviews() {
        let mut store = DemoStore::new();
        let id = Uuid::new_v4();
        let eid = store.data.employees[1].id;
        store.execute(Action::Payroll(Change::new("tax_create",json!({"id":id,"employee_id":eid,"year":2026,"kind":"termination","service_end":"2026-09-30","as_of":"2026-09-30","title":"Synthetic tax UI","reason":"Synthetic UI fixture"})))).unwrap();
        let mut f = worksheet(store.data.payroll.tax.records[0].clone());
        f.w.taxable_adjustment_cents = 12345;
        f.w.adjustment_note = "Unsaved corrected source".into();
        f.w.final_compensation_reviewed = true;
        f.w.previous_employers_reviewed = true;
        f.w.benefits_reviewed = true;
        f.reason = "Unsaved edit".into();
        let mut state = State {
            form: Some(Form::Worksheet(f)),
            ..Default::default()
        };
        let a = Change::new(
            "tax_refresh",
            json!({"id":id,"expected_revision":0,"reason":"Refresh synthetic annual tax"}),
        );
        store.execute(Action::Payroll(a.clone())).unwrap();
        state.accept(&a, &store.data);
        let Some(Form::Worksheet(f)) = state.form else {
            panic!("Open tax draft lost")
        };
        assert_eq!(f.w.taxable_adjustment_cents, 12345);
        assert_eq!(f.w.adjustment_note, "Unsaved corrected source");
        assert!(
            !f.w.final_compensation_reviewed
                && !f.w.previous_employers_reviewed
                && !f.w.benefits_reviewed
        );
        assert_eq!(f.record.revision, 1);
    }
}
