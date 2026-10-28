use super::payroll_view::{field, footer, note};
use super::*;
use crate::{
    payroll::{self, PayrollAction as Change},
    payroll_payments::{self as payments, Candidate},
};
use serde_json::json;
pub(super) struct State {
    search: String,
    outstanding: bool,
    form: Option<Form>,
}
impl Default for State {
    fn default() -> Self {
        Self {
            search: String::new(),
            outstanding: true,
            form: None,
        }
    }
}
struct Form {
    id: Uuid,
    row: Candidate,
    date: String,
    reference: String,
    reason: String,
    confirmed: bool,
}
impl State {
    pub(super) fn editing(&self) -> bool {
        self.form.is_some()
    }
    pub(super) fn accept(&mut self) {
        self.form = None;
    }
}
impl HrisApp {
    pub(super) fn payroll_payments(&mut self, ui: &mut Ui) {
        if !self
            .data
            .access
            .allows(crate::staff_roles::Capability::PayrollPreparation)
        {
            return;
        }
        ui.heading("Employee payment records");
        ui.label("Record an actual full-payslip payment for one person. Other employees remain outstanding. Recording a payment moves no money.");
        ui.horizontal_wrapped(|ui| {
            ui.selectable_value(
                &mut self.payroll_state.payments.outstanding,
                true,
                "Outstanding",
            );
            ui.selectable_value(
                &mut self.payroll_state.payments.outstanding,
                false,
                "All issued payslips",
            );
            ui.label("Find employee");
            ui.add(
                egui::TextEdit::singleline(&mut self.payroll_state.payments.search)
                    .desired_width(220.0),
            );
        });
        let rows = match payments::candidates(&self.data) {
            Ok(r) => r,
            Err(e) => {
                ui.label(e.to_string());
                return;
            }
        };
        let query = self.payroll_state.payments.search.trim().to_lowercase();
        let rows: Vec<_> = rows
            .into_iter()
            .filter(|r| {
                (!self.payroll_state.payments.outstanding || r.paid_on.is_none())
                    && (query.is_empty()
                        || format!("{} {}", r.employee_code, r.employee_name)
                            .to_lowercase()
                            .contains(&query))
            })
            .collect();
        ui.label(format!("{} payslips", rows.len()));
        for r in rows {
            card(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.strong(format!("{} · {}", r.employee_code, r.employee_name));
                    ui.label(payroll::money(r.amount_cents));
                });
                ui.label(format!(
                    "{} · {} · Period ends {}",
                    r.kind.replace('_', " "),
                    r.title,
                    r.ends_on
                ));
                if let Some(date) = r.paid_on {
                    let final_settlement = self.data.payroll.receipts.iter().any(|p| {
                        p.slip_id == r.slip_id && p.kind == r.kind && p.origin == "final_settlement"
                    });
                    ui.label(if final_settlement{format!("Source balance settled in final pay on {date}; actual combined cash is on its final statement.")}else{format!("Payment recorded {date}")});
                    ui.label(&r.payment_reference);
                } else {
                    ui.horizontal_wrapped(|ui| {
                        ui.label("Outstanding");
                        if self.data.access.role == Role::Administrator
                            && ui.button("Record this employee's payment").clicked()
                        {
                            self.payroll_state.payments.form = Some(Form {
                                id: Uuid::new_v4(),
                                row: r.clone(),
                                date: self.data.company.today().to_string(),
                                reference: String::new(),
                                reason: String::new(),
                                confirmed: false,
                            });
                        }
                    });
                }
            });
        }
    }
    pub(super) fn payroll_payment_forms(&mut self, ui: &mut Ui) {
        let Some(mut f) = self.payroll_state.payments.form.take() else {
            return;
        };
        if self.data.access.role != Role::Administrator {
            return;
        }
        if let Ok(rows) = payments::candidates(&self.data) {
            if let Some(current) = rows
                .into_iter()
                .find(|r| r.kind == f.row.kind && r.slip_id == f.row.slip_id)
            {
                if current.revision != f.row.revision {
                    ui.label("Payment status changed. Review the latest status before recording.");
                    if ui.button("Use current payment status").clicked() {
                        f.row = current;
                        f.confirmed = false;
                    }
                }
            }
        }
        ui.heading("Record employee payment");
        ui.label(format!("{} · {}", f.row.employee_code, f.row.employee_name));
        ui.label(format!("{} · {}", f.row.kind, f.row.title));
        ui.strong(format!(
            "Full issued net: {}",
            payroll::money(f.row.amount_cents)
        ));
        ui.label("This records a payment already made. The issued payslip and other employees' payment records remain unchanged.");
        field(ui, "Actual paid date (YYYY-MM-DD)", &mut f.date);
        note(ui, "Payment reference (HR private)", &mut f.reference);
        note(ui, "Reason / evidence (HR private)", &mut f.reason);
        ui.checkbox(
            &mut f.confirmed,
            "I verified that this full payslip amount was actually paid to this employee.",
        );
        let a = Change::new(
            "receipt_record",
            json!({"id":f.id,"kind":f.row.kind,"slip_id":f.row.slip_id,"expected_revision":f.row.revision,"paid_on":f.date.trim(),"payment_reference":f.reference.trim(),"reason":f.reason.trim()}),
        );
        let (mut save, mut discard) = (false, false);
        ui.add_enabled_ui(f.confirmed, |ui| {
            let x = footer(ui, &a, &self.data, "Save payment record");
            save = x.0;
            discard = x.1;
        });
        // Discard remains usable while the payment is unconfirmed or invalid.
        let discard = discard || (!f.confirmed && ui.button("Back without recording").clicked());
        if save {
            self.send_payroll(a);
        }
        if !discard {
            self.payroll_state.payments.form = Some(f);
        }
    }
}

#[cfg(feature = "qa-screenshot")]
impl HrisApp {
    pub(super) fn prepare_payment_preview(&mut self, form: &str) {
        let mut s = DemoStore::new();
        let run = Uuid::from_u128(99820);
        let a = Uuid::from_u128(99821);
        let b = Uuid::from_u128(99822);
        let employees = [s.data.employees[1].id, s.data.employees[2].id];
        let apply = |s: &mut DemoStore, op: &str, v: serde_json::Value| {
            s.execute(Action::Payroll(Change::new(op, v)))
                .expect("Fictional payment preview");
        };
        for (n, e) in employees.iter().enumerate() {
            apply(
                &mut s,
                "profile_save",
                json!({"id":Uuid::from_u128(99810+n as u128),"employee_id":e,"effective_on":"2026-02-01","pay":payroll::Pay{rate_cents:3_000_000,..Default::default()},"reason":"Fictional salary source"}),
            );
        }
        apply(
            &mut s,
            "run_create",
            json!({"id":run,"month":"2026-09-01","title":"September payroll reconciliation","reason":"Fictional payment preview"}),
        );
        for (e, id) in employees.into_iter().zip([a, b]) {
            apply(
                &mut s,
                "item_add",
                json!({"id":id,"run_id":run,"employee_id":e,"reason":"Fictional employee pay"}),
            );
            let i = s.data.payroll.items.iter().find(|i| i.id == id).unwrap();
            let rev = i.revision;
            let w = payroll::Worksheet {
                reviewed: true,
                review_note: "Fictional reviewed attendance and salary".into(),
                ..i.worksheet.clone()
            };
            apply(
                &mut s,
                "item_save",
                json!({"id":id,"expected_revision":rev,"worksheet":w,"reason":"Fictional pay review"}),
            );
        }
        let rev = s.data.payroll.runs[0].revision;
        apply(
            &mut s,
            "run_submit",
            json!({"id":run,"expected_revision":rev,"scope_reviewed":true,"reason":"Fictional independent submission"}),
        );
        s.data.access.user_id = Uuid::from_u128(99801);
        let rev = s.data.payroll.runs[0].revision;
        apply(
            &mut s,
            "run_approve",
            json!({"id":run,"expected_revision":rev,"reason":"Fictional independent approval"}),
        );
        if form.ends_with("partial") || form.ends_with("history") {
            let c = payments::candidates(&s.data)
                .unwrap()
                .into_iter()
                .find(|c| c.slip_id == a)
                .unwrap();
            apply(
                &mut s,
                "receipt_record",
                json!({"id":Uuid::from_u128(99830),"kind":"monthly","slip_id":a,"expected_revision":c.revision,"paid_on":"2026-09-30","payment_reference":"Fictional recorded employee payment","reason":"Fictional reviewed full-net receipt; no money transferred"}),
            );
        }
        self.data = s.data;
        self.payroll_state.payments = State::default();
        if form.ends_with("history") {
            self.payroll_state.payments.outstanding = false;
        }
        if form.ends_with("record") || form.ends_with("ready") {
            let row = payments::candidates(&self.data)
                .unwrap()
                .into_iter()
                .find(|r| r.slip_id == a)
                .unwrap();
            self.payroll_state.payments.form = Some(Form {
                id: Uuid::from_u128(99831),
                row,
                date: "2026-10-01".into(),
                reference: if form.ends_with("ready") {
                    "Fictional reconciled payment reference".into()
                } else {
                    String::new()
                },
                reason: if form.ends_with("ready") {
                    "Fictional evidence confirms this full payslip was received.".into()
                } else {
                    String::new()
                },
                confirmed: form.ends_with("ready"),
            });
        }
        if form.ends_with("manager") {
            self.data.access.role = Role::Manager;
            self.data.access.employee_id = Some(employees[0]);
        }
    }
}
