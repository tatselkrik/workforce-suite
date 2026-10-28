use super::payroll_view::{amount, field, footer, note};
use super::tax_view::signed_amount;
use super::*;
use crate::{
    payroll::{self, PayrollAction as Change},
    payroll_amendments as p,
};
use serde_json::json;
#[derive(Default)]
pub(super) struct State {
    selected: Option<Uuid>,
    slip: Option<Uuid>,
    issued: bool,
    search: String,
    form: Option<Form>,
}
enum Form {
    New {
        id: Uuid,
        target: Option<(String, Uuid)>,
        cash: String,
        title: String,
        purpose: String,
        reason: String,
    },
    Worksheet(Box<Edit>),
    Decision {
        id: Uuid,
        revision: i64,
        op: String,
        reason: String,
        reference: String,
        confirmed: bool,
    },
}
struct Edit {
    r: p::Record,
    w: p::Worksheet,
    reason: String,
}
impl State {
    pub(super) fn open_notice(&mut self,id:Uuid){self.selected=None;self.issued=true;self.search.clear();self.slip=Some(id);}
    pub(super) fn editing(&self) -> bool {
        self.form.is_some()
    }
    pub(super) fn accept(&mut self, a: &Change, data: &Snapshot) {
        let id = a.record["id"].as_str().and_then(|s| s.parse().ok());
        if a.operation == "amendment_refresh" {
            if let Some(Form::Worksheet(f)) = self.form.as_mut() {
                if let Some(r) = data
                    .payroll
                    .amendments
                    .records
                    .iter()
                    .find(|r| Some(r.id) == id)
                {
                    f.r = r.clone();
                    f.w.reviewed = false;
                    f.w.tax_reviewed = false;
                    f.w.contributions_reviewed = false;
                    f.w.review_note.clear();
                    if f.w.purpose == "payout" {
                        if let Ok(s) = p::source(r) {
                            f.w.other_benefits_cents =
                                f.w.other_benefits_cents.max(s.shared_floor_cents);
                        }
                    }
                    return;
                }
            }
        }
        if a.operation == "amendment_create" {
            self.selected = id;
        }
        self.form = None;
    }
}
#[cfg(feature = "qa-screenshot")]
impl HrisApp {
    pub(super) fn prepare_amendment_preview(&mut self, form: &str) {
        self.prepare_final_preview("payroll-final-settled");
        let mut store = DemoStore::new();
        store.data = self.data.clone();
        let admin = store.data.access.user_id;
        let eid = store.data.employees[1].id;
        let hr = Uuid::from_u128(99400);
        let today = store.data.company.today();
        let target = p::targets(&store.data)
            .unwrap()
            .into_iter()
            .find(|p| p.employee_id == eid && p.kind == "monthly")
            .or_else(|| {
                p::targets(&store.data)
                    .unwrap()
                    .into_iter()
                    .find(|p| p.employee_id == eid)
            })
            .unwrap();
        let id = Uuid::from_u128(99401);
        store.data.access.user_id = hr;
        store.data.access.role = Role::HrGeneralist;
        let apply = |s: &mut DemoStore, op: &str, v: serde_json::Value| {
            s.execute(Action::Payroll(Change::new(op, v)))
                .expect("Fictional amendment preview");
        };
        let review = form.ends_with("review");
        apply(
            &mut store,
            "amendment_create",
            json!({"id":id,"target_kind":target.kind,"target_id":target.id,"cash_date":today,"title":if review{"Reviewed payroll discrepancy"}else{"Supplemental salary correction"},"purpose":if review{"review_only"}else{"payout"},"reason":"Fictional linked amendment"}),
        );
        let mut r = store.data.payroll.amendments.records[0].clone();
        let mut w = r.worksheet.clone();
        if review {
            w.review_amount_cents = -(target.net_cents.min(150000));
        } else {
            w.taxable_cents = 250000;
            if ["monthly", "cutoff"].contains(&target.kind.as_str()) {
                w.basic_cents = 150000;
            }
            w.shared_cents = 50000;
            w.withholding_cents = 25000;
            w.tax_note =
                "Fictional supplementary withholding and annual benefit classification reviewed"
                    .into();
            w.contribution_note =
                "Fictional original month reconciled; no additional statutory charge or refund"
                    .into();
            w.tax_reviewed = true;
            w.contributions_reviewed = true;
        }
        w.evidence = "Fictional original worksheet and approved corrected difference".into();
        w.review_note = "Fictional original, earlier adjustments and independent review".into();
        w.reviewed = true;
        apply(
            &mut store,
            "amendment_save",
            json!({"id":id,"expected_revision":r.revision,"worksheet":w,"reason":"Fictional reviewed difference"}),
        );
        r = store.data.payroll.amendments.records[0].clone();
        if !form.ends_with("worksheet") && !form.ends_with("new") {
            apply(
                &mut store,
                "amendment_submit",
                json!({"id":id,"expected_revision":r.revision,"reason":"Fictional submission"}),
            );
            r = store.data.payroll.amendments.records[0].clone();
            if !form.ends_with("approve") {
                store.data.access.user_id = admin;
                store.data.access.role = Role::Administrator;
                apply(
                    &mut store,
                    "amendment_approve",
                    json!({"id":id,"expected_revision":r.revision,"reason":"Fictional independent approval"}),
                );
                r = store.data.payroll.amendments.records[0].clone();
                if form.ends_with("paid") || form.ends_with("manager") || form.ends_with("list") {
                    apply(
                        &mut store,
                        "amendment_pay",
                        json!({"id":id,"expected_revision":r.revision,"paid_on":today,"payment_reference":"Fictional supplemental cash receipt","received_confirmed":true,"reason":"Fictional actual supplemental cash only"}),
                    );
                }
                if form.ends_with("withdrawn") {
                    apply(
                        &mut store,
                        "amendment_withdraw",
                        json!({"id":id,"expected_revision":r.revision,"unpaid_confirmed":true,"reason":"Fictional never-paid supplement"}),
                    );
                }
            }
        }
        store.data.access.user_id = admin;
        store.data.access.role = Role::Administrator;
        self.data = store.data;
        self.payroll_state.tab = 12;
        self.payroll_state.amendments = State::default();
        self.payroll_state.amendments.selected = Some(id);
        let r = self.data.payroll.amendments.records[0].clone();
        if form.ends_with("list") {
            self.payroll_state.amendments.selected = None;
        }
        if form.ends_with("new") {
            self.payroll_state.amendments.form = Some(Form::New {
                id: Uuid::from_u128(99402),
                target: Some((target.kind.clone(), target.id)),
                cash: today.to_string(),
                title: "Additional payroll review".into(),
                purpose: "payout".into(),
                reason: "Fictional original selected".into(),
            });
        }
        if form.ends_with("worksheet") {
            self.payroll_state.amendments.form = Some(Form::Worksheet(Box::new(Edit {
                w: r.worksheet.clone(),
                r: r.clone(),
                reason: "Fictional worksheet save".into(),
            })));
        }
        for op in ["approve", "pay"] {
            if form.ends_with(op) {
                self.payroll_state.amendments.form = Some(Form::Decision {
                    id,
                    revision: r.revision,
                    op: op.into(),
                    reason: "Fictional reviewed action".into(),
                    reference: if op == "pay" {
                        "Fictional actual payment reference".into()
                    } else {
                        String::new()
                    },
                    confirmed: false,
                });
            }
        }
        if form.ends_with("paid")
            || form.ends_with("review")
            || form.ends_with("withdrawn")
            || form.ends_with("manager")
        {
            self.payroll_state.amendments.issued = true;
            self.payroll_state.amendments.slip = Some(id);
        }
        if form.ends_with("manager") {
            self.data.access.role = Role::Manager;
            self.data.access.employee_id = Some(eid);
        }
        if let Some(s) = self
            .data
            .payroll
            .amendments
            .slips
            .iter()
            .find(|s| s.id == id)
        {
            if let Ok(path) = std::env::var("HRIS_PREVIEW_AMENDMENT_PDF") {
                crate::payroll_pdf::save(
                    std::path::Path::new(&path),
                    &crate::payroll_pdf::build_amendment(s, p::status(&self.data, id)).unwrap(),
                )
                .unwrap();
            }
            if let Ok(path) = std::env::var("HRIS_PREVIEW_AMENDMENT_CSV") {
                crate::reports::save_csv(
                    std::path::Path::new(&path),
                    &p::export_csv(&self.data, "amendment_statement", Some(id)).unwrap(),
                )
                .unwrap();
            }
        }
    }
}
fn entry(ui: &mut Ui, label: &str, value: &str) {
    ui.horizontal_top(|ui| {
        let width = (ui.available_width() * 0.38).min(235.0);
        ui.allocate_ui_with_layout(egui::vec2(width, 0.0), Layout::top_down(Align::Min), |ui| {
            ui.set_min_width(width);
            ui.set_max_width(width);
            ui.add(egui::Label::new(RichText::new(label).color(muted(ui))).wrap());
        });
        ui.add(egui::Label::new(value).wrap());
    });
}
fn totals(ui: &mut Ui, t: &p::Calculation, review: bool) {
    if review {
        entry(
            ui,
            "Reviewed discrepancy",
            &payroll::money(t.review_amount_cents),
        );
        ui.label("A discrepancy review authorizes no deduction, employee debt or tax reduction.");
        return;
    }
    for (label, n) in p::totals_rows(t) {
        entry(ui, label, &payroll::money(n));
    }
}
fn original_export(app: &mut HrisApp, ui: &mut Ui, kind: &str, id: Uuid) {
    if ui.button("Export original payslip PDF").clicked() {
        app.payroll_export(
            match kind {
                "monthly" => "payslip_pdf",
                "cutoff" => "cutoff_payslip_pdf",
                "thirteenth" => "thirteenth_payslip_pdf",
                _ => "final_statement_pdf",
            },
            Some(id),
        );
    }
}
impl HrisApp {
    pub(super) fn payroll_amendments(&mut self, ui: &mut Ui) {
        let hr = self.data.access.allows(crate::staff_roles::Capability::PayrollPreparation);
        if !hr {
            self.payroll_state.amendments.issued = true;
            self.payroll_state.amendments.form = None;
        }
        ui.heading("Payroll amendments");
        ui.label("Link a paid original to a supplemental correction. Original payslips and receipts stay unchanged. Overpayment discrepancies have a separate review without automatic recovery.");
        ui.horizontal_wrapped(|ui| {
            if hr {
                ui.selectable_value(
                    &mut self.payroll_state.amendments.issued,
                    false,
                    "HR reviews",
                );
            }
            ui.selectable_value(
                &mut self.payroll_state.amendments.issued,
                true,
                if hr {
                    "Issued statements"
                } else {
                    "My amendment statements"
                },
            );
        });
        if self.payroll_state.amendments.issued {
            self.amendment_slips(ui);
            return;
        }
        if let Some(r) = self
            .data
            .payroll
            .amendments
            .records
            .iter()
            .find(|r| {
                Some(r.id) == self.payroll_state.amendments.selected
                    && r.company_id == self.data.company.id
            })
            .cloned()
        {
            if ui.button("Back to amendment reviews").clicked() {
                self.payroll_state.amendments.selected = None;
                return;
            }
            card(ui, |ui| {
                ui.heading(&r.title);
                entry(
                    ui,
                    "Employee",
                    &format!("{} · {}", r.employee_code, r.employee_name),
                );
                entry(
                    ui,
                    "Original",
                    &format!("{} · {}", r.target_kind, r.target_id),
                );
                entry(
                    ui,
                    "Earned period",
                    &format!("{} to {}", r.starts_on, r.ends_on),
                );
                entry(
                    ui,
                    "Status / payment date",
                    &format!("{} · {}", r.status, r.cash_date),
                );
                totals(ui, &r.totals, r.worksheet.purpose == "review_only");
                original_export(self, ui, &r.target_kind, r.target_id);
            });
            ui.horizontal_wrapped(|ui| {
                if r.status == "draft" && ui.button("Edit worksheet").clicked() {
                    self.payroll_state.amendments.form = Some(Form::Worksheet(Box::new(Edit {
                        w: r.worksheet.clone(),
                        r: r.clone(),
                        reason: String::new(),
                    })));
                }
                let mut ops = vec![];
                if r.status == "draft" {
                    ops.push(("submit", "Submit for independent approval"));
                }
                if r.status == "submitted" {
                    ops.push(("return", "Return to draft"));
                    if self.data.access.role == Role::Administrator {
                        ops.push(("approve", "Approve amendment"));
                    }
                }
                if ["draft", "submitted"].contains(&r.status.as_str()) {
                    ops.push(("cancel", "Cancel review"));
                }
                if r.status == "approved" && self.data.access.role == Role::Administrator {
                    ops.push(("pay", "Record supplemental payment"));
                    ops.push(("withdraw", "Withdraw unpaid supplement"));
                }
                for (op, label) in ops {
                    if ui.button(label).clicked() {
                        self.payroll_state.amendments.form = Some(Form::Decision {
                            id: r.id,
                            revision: r.revision,
                            op: op.into(),
                            reason: String::new(),
                            reference: String::new(),
                            confirmed: false,
                        });
                    }
                }
            });
            ui.collapsing("Private review and evidence", |ui| {
                entry(ui, "Evidence", &r.worksheet.evidence);
                entry(ui, "Tax review", &r.worksheet.tax_note);
                entry(ui, "Contribution review", &r.worksheet.contribution_note);
                entry(ui, "Review note", &r.worksheet.review_note);
            });
            self.payroll_history(ui, r.id);
            return;
        }
        if ui.button("New linked amendment").clicked() {
            self.payroll_state.amendments.form = Some(Form::New {
                id: Uuid::new_v4(),
                target: None,
                cash: self.data.company.today().to_string(),
                title: "Payroll correction".into(),
                purpose: "payout".into(),
                reason: String::new(),
            });
        }
        field(
            ui,
            "Search employee, code or title",
            &mut self.payroll_state.amendments.search,
        );
        let query = self.payroll_state.amendments.search.to_lowercase();
        let mut rows = self
            .data
            .payroll
            .amendments
            .records
            .iter()
            .filter(|r| {
                r.company_id == self.data.company.id
                    && format!("{} {} {}", r.employee_name, r.employee_code, r.title)
                        .to_lowercase()
                        .contains(&query)
            })
            .cloned()
            .collect::<Vec<_>>();
        rows.sort_by_key(|r| {
            (
                std::cmp::Reverse(r.cash_date),
                r.employee_code.clone(),
                r.id,
            )
        });
        if rows.is_empty() {
            ui.label("No amendment reviews match this search.");
        }
        for r in rows {
            card(ui, |ui| {
                if ui
                    .selectable_label(
                        false,
                        RichText::new(format!("{} · {}", r.employee_code, r.employee_name))
                            .strong(),
                    )
                    .clicked()
                {
                    self.payroll_state.amendments.selected = Some(r.id);
                }
                ui.label(format!("{} · {} · {}", r.title, r.status, r.cash_date));
                entry(
                    ui,
                    "Amount",
                    &payroll::money(if r.worksheet.purpose == "review_only" {
                        r.totals.review_amount_cents
                    } else {
                        r.totals.net_cents
                    }),
                );
            });
        }
    }
    fn amendment_slips(&mut self, ui: &mut Ui) {
        let hr = self.data.access.allows(crate::staff_roles::Capability::PayrollPreparation);
        let rows = self
            .data
            .payroll
            .amendments
            .slips
            .iter()
            .filter(|s| {
                s.company_id == self.data.company.id
                    && (hr || self.data.access.employee_id == Some(s.employee_id))
            })
            .cloned()
            .collect::<Vec<_>>();
        if let Some(s) = rows
            .iter()
            .find(|s| Some(s.id) == self.payroll_state.amendments.slip)
        {
            let s = s.clone();
            if ui.button("Back to issued statements").clicked() {
                self.payroll_state.amendments.slip = None;
                return;
            }
            card(ui, |ui| {
                ui.heading(&s.title);
                entry(
                    ui,
                    "Employee",
                    &format!("{} · {}", s.employee_code, s.employee_name),
                );
                entry(
                    ui,
                    "Original",
                    &format!("{} · {}", s.target_kind, s.target_id),
                );
                entry(
                    ui,
                    "Earned period",
                    &format!("{} to {}", s.starts_on, s.ends_on),
                );
                entry(ui, "Planned supplemental payment", &s.cash_date.to_string());
                if let Some(st) = self
                    .data
                    .payroll
                    .amendments
                    .statuses
                    .iter()
                    .find(|p| p.id == s.id)
                {
                    entry(
                        ui,
                        "Recorded status",
                        &format!(
                            "{} · {} · {}",
                            st.kind,
                            st.recorded_on,
                            payroll::money(st.net_cents)
                        ),
                    );
                } else {
                    ui.label(if s.purpose == "review_only" {
                        "Reviewed discrepancy. No payment or automatic recovery."
                    } else {
                        "Approved supplement; actual payment has not been recorded."
                    });
                }
                totals(ui, &s.totals, s.purpose == "review_only");
            });
            ui.horizontal_wrapped(|ui| {
                if ui.button("Export amendment PDF").clicked() {
                    self.payroll_export("amendment_statement_pdf", Some(s.id));
                }
                if ui.button("Export UTF-8 CSV").clicked() {
                    self.payroll_export("amendment_statement", Some(s.id));
                }
                original_export(self, ui, &s.target_kind, s.target_id);
            });
            return;
        }
        if rows.is_empty() {
            ui.label("No issued amendment statements are available.");
        }
        for s in rows {
            card(ui, |ui| {
                if ui
                    .selectable_label(
                        false,
                        RichText::new(format!("{} · {}", s.employee_code, s.title)).strong(),
                    )
                    .clicked()
                {
                    self.payroll_state.amendments.slip = Some(s.id);
                }
                ui.label(format!(
                    "{} · {} · {}",
                    s.employee_name,
                    s.cash_date,
                    payroll::money(if s.purpose == "review_only" {
                        s.totals.review_amount_cents
                    } else {
                        s.totals.net_cents
                    })
                ));
            });
        }
    }
    pub(super) fn payroll_amendment_forms(&mut self, ui: &mut Ui) {
        if !self.data.access.allows(crate::staff_roles::Capability::PayrollPreparation) {
            self.payroll_state.amendments.form = None;
            return;
        }
        let Some(mut form) = self.payroll_state.amendments.form.take() else {
            return;
        };
        let keep;
        match &mut form {
            Form::New {
                id,
                target,
                cash,
                title,
                purpose,
                reason,
            } => {
                ui.heading("New linked payroll amendment");
                ui.label("Select a payslip with an actual payment record. A never-paid final statement can be withdrawn through Final pay.");
                let candidates = p::targets(&self.data).unwrap_or_default();
                let label = target
                    .as_ref()
                    .and_then(|(k, id)| candidates.iter().find(|p| p.kind == *k && p.id == *id))
                    .map(|p| {
                        format!(
                            "{} · {} · {} through {}",
                            p.employee_code, p.employee_name, p.kind, p.ends_on
                        )
                    })
                    .unwrap_or("Choose a paid original".into());
                egui::ComboBox::from_id_salt("amendment_original")
                    .selected_text(label)
                    .width(ui.available_width().min(650.0))
                    .show_ui(ui, |ui| {
                        for p in candidates {
                            ui.selectable_value(
                                target,
                                Some((p.kind.clone(), p.id)),
                                format!(
                                    "{} · {} · {} through {}",
                                    p.employee_code, p.employee_name, p.kind, p.ends_on
                                ),
                            );
                        }
                    });
                ui.horizontal_wrapped(|ui| {
                    ui.selectable_value(purpose, "payout".into(), "Supplemental payout");
                    ui.selectable_value(purpose, "review_only".into(), "Overpayment review only");
                });
                field(
                    ui,
                    "Planned supplemental payment / review date (YYYY-MM-DD)",
                    cash,
                );
                field(ui, "Statement title (visible to employee)", title);
                note(ui, "Reason (HR private)", reason);
                ui.label("Supported payment year:2026 from 6 January. Prior-year contribution/tax refunds and actual wage recovery need a separate year-specific process.");
                let (kind, tid) = target.clone().unwrap_or_default();
                let a = Change::new(
                    "amendment_create",
                    json!({"id":id,"target_kind":kind,"target_id":tid,"cash_date":cash,"title":title,"purpose":purpose,"reason":reason}),
                );
                let (save, cancel) = footer(ui, &a, &self.data, "Create review");
                if save {
                    self.send_payroll(a);
                }
                keep = !cancel;
            }
            Form::Worksheet(f) => {
                ui.heading(format!("Amendment · {}", f.r.employee_name));
                entry(
                    ui,
                    "Linked original",
                    &format!("{} · {}", f.r.target_kind, f.r.target_id),
                );
                entry(ui, "Payment year", &f.r.cash_date.year().to_string());
                let s = match p::source(&f.r) {
                    Ok(s) => s,
                    Err(e) => {
                        ui.label(e.to_string());
                        return;
                    }
                };
                if ui
                    .button("Refresh original and prior adjustments; review again")
                    .clicked()
                {
                    self.send_payroll(Change::new("amendment_refresh",json!({"id":f.r.id,"expected_revision":f.r.revision,"reason":"Refresh linked original, refund credits and shared benefit facts"})));
                }
                if f.w.purpose == "review_only" {
                    signed_amount(
                        ui,
                        "Overpayment discrepancy (negative PHP)",
                        &mut f.w.review_amount_cents,
                    );
                    ui.label("Approval records the discrepancy. It will not deduct wages, reduce taxable income, reverse a receipt or create an automatic receivable.");
                } else {
                    card(ui, |ui| {
                        ui.strong("Additional amounts owed");
                        amount(
                            ui,
                            "Regular taxable earnings (including basic)",
                            &mut f.w.taxable_cents,
                        );
                        if ["monthly", "cutoff"].contains(&f.r.target_kind.as_str()) {
                            amount(ui, "Basic portion of those earnings", &mut f.w.basic_cents);
                        }
                        amount(
                            ui,
                            "Qualifying 13th / other shared benefits",
                            &mut f.w.shared_cents,
                        );
                        amount(
                            ui,
                            "Other benefits already using the annual cap",
                            &mut f.w.other_benefits_cents,
                        );
                        entry(
                            ui,
                            "Protected known benefit floor",
                            &payroll::money(s.shared_floor_cents),
                        );
                        amount(
                            ui,
                            "Evidenced exempt / reimbursement / deposit return",
                            &mut f.w.exempt_cents,
                        );
                        ui.label("Basic is included in regular earnings above. Only evidenced statutory exclusions, reimbursements or deposit returns belong in the exempt amount.");
                    });
                    card(ui, |ui| {
                        ui.strong("Reviewed original-month adjustments");
                        signed_amount(
                            ui,
                            "SSS employee: charge (+) / refund (-)",
                            &mut f.w.sss_cents,
                        );
                        signed_amount(
                            ui,
                            "PhilHealth employee: charge (+) / refund (-)",
                            &mut f.w.philhealth_cents,
                        );
                        signed_amount(
                            ui,
                            "Pag-IBIG employee: charge (+) / refund (-)",
                            &mut f.w.pagibig_cents,
                        );
                        signed_amount(
                            ui,
                            "Withholding (+) / same-year refund (-)",
                            &mut f.w.withholding_cents,
                        );
                        amount(
                            ui,
                            "Evidenced lawful deduction",
                            &mut f.w.lawful_deduction_cents,
                        );
                        entry(
                            ui,
                            "Remaining paid-year tax credit",
                            &payroll::money(s.withholding_refundable_cents),
                        );
                        ui.label("Reconcile the original contribution month and supplementary withholding. These are reviewed differences; do not enter a second full statutory charge. Employer-side remittance corrections remain a separate reconciliation.");
                        note(
                            ui,
                            "Withholding, exemption and other-employer evidence (HR private)",
                            &mut f.w.tax_note,
                        );
                        note(
                            ui,
                            "Contribution month, refunds and employer-side handling (HR private)",
                            &mut f.w.contribution_note,
                        );
                        ui.checkbox(
                            &mut f.w.tax_reviewed,
                            "Supplementary/cumulative withholding and benefit treatment reviewed",
                        );
                        ui.checkbox(
                            &mut f.w.contributions_reviewed,
                            "Original-month contributions and refunds reviewed, including zero",
                        );
                    });
                }
                note(
                    ui,
                    "Original discrepancy, calculation and legal evidence (HR private)",
                    &mut f.w.evidence,
                );
                note(
                    ui,
                    "Independent review summary (HR private)",
                    &mut f.w.review_note,
                );
                ui.checkbox(
                    &mut f.w.reviewed,
                    "Original, previous adjustments and corrected difference reviewed",
                );
                match p::calculate(&s, &f.w) {
                    Ok(t) => {
                        card(ui, |ui| totals(ui, &t, f.w.purpose == "review_only"));
                        if let Err(e) = p::ready(&s, &f.w) {
                            ui.label(RichText::new(e.to_string()).color(muted(ui)));
                        }
                    }
                    Err(e) => {
                        ui.colored_label(Color32::DARK_RED, e.to_string());
                    }
                }
                note(ui, "Reason for saving (HR private)", &mut f.reason);
                let a = Change::new(
                    "amendment_save",
                    json!({"id":f.r.id,"expected_revision":f.r.revision,"worksheet":f.w,"reason":f.reason}),
                );
                let (save, cancel) = footer(ui, &a, &self.data, "Save worksheet");
                if save {
                    self.send_payroll(a);
                }
                keep = !cancel;
            }
            Form::Decision {
                id,
                revision,
                op,
                reason,
                reference,
                confirmed,
            } => {
                let Some(r) = self
                    .data
                    .payroll
                    .amendments
                    .records
                    .iter()
                    .find(|r| r.id == *id)
                    .cloned()
                else {
                    return;
                };
                ui.heading(format!("{} amendment", op.replace('_', " ")));
                entry(ui, "Employee", &r.employee_name);
                totals(ui, &r.totals, r.worksheet.purpose == "review_only");
                if op == "pay" {
                    entry(ui, "Approved actual payment date", &r.cash_date.to_string());
                    field(ui, "Actual payment reference (HR private)", reference);
                    ui.label("Record the supplemental cash only. The original receipt remains unchanged. An unpaid statement must be withdrawn and reviewed again to change its date.");
                }
                ui.checkbox(
                    confirmed,
                    match op.as_str() {
                        "pay" => "Actual supplemental cash was received, including zero",
                        "withdraw" => "No supplemental cash was ever paid",
                        _ => "I reviewed the saved facts and this action",
                    },
                );
                note(ui, "Reason (HR private)", reason);
                let mut v = json!({"id":id,"expected_revision":revision,"reason":reason});
                if op == "pay" {
                    v["paid_on"] = json!(r.cash_date);
                    v["payment_reference"] = json!(reference);
                    v["received_confirmed"] = json!(*confirmed);
                }
                if op == "withdraw" {
                    v["unpaid_confirmed"] = json!(*confirmed);
                }
                let a = Change::new(&format!("amendment_{op}"), v);
                let valid = a.payload(&self.data);
                if let Err(e) = &valid {
                    ui.label(RichText::new(e.to_string()).color(muted(ui)));
                }
                let mut save = false;
                let mut cancel = false;
                ui.horizontal_wrapped(|ui| {
                    save = ui
                        .add_enabled(
                            *confirmed && valid.is_ok(),
                            egui::Button::new("Confirm action"),
                        )
                        .clicked();
                    cancel = ui.button("Back without recording").clicked();
                });
                if save {
                    self.send_payroll(a);
                }
                keep = !cancel;
            }
        }
        if keep {
            self.payroll_state.amendments.form = Some(form);
        }
    }
}
