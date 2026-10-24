use super::*;
#[cfg(feature = "qa-screenshot")]
use crate::payroll::Profile;
use crate::payroll::{self, Component, Day, Item, Pay, PayrollAction as Change, Run, Worksheet};
#[cfg(feature = "qa-screenshot")]
use serde_json::Value;
use serde_json::json;
#[derive(Default)]
pub(super) struct State {
    pub(super) finals: super::final_view::State,
    pub(super) payments: super::payment_view::State,
    pub(super) tax: super::tax_view::State,
    pub(super) thirteenth: super::thirteenth_view::State,
    pub(super) cutoffs: super::cutoff_view::State,
    pub(super) tab: usize,
    selected: Option<Uuid>,
    slip: Option<Uuid>,
    employee: Option<Uuid>,
    year: i32,
    profile: Option<ProfileForm>,
    holiday: Option<HolidayForm>,
    run: Option<RunForm>,
    pub(super) item: Option<ItemForm>,
    decision: Option<Decision>,
    pub pending: Option<Change>,
    pub(super) warnings: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> (DemoStore, Item) {
        let mut s = DemoStore::new();
        let eid = s.data.employees[1].id;
        let p = Uuid::new_v4();
        let r = Uuid::new_v4();
        let i = Uuid::new_v4();
        for a in [
            Change::new(
                "profile_save",
                json!({"id":p,"employee_id":eid,"effective_on":"2026-02-01","pay":Pay{rate_cents:3_000_000,..Default::default()},"reason":"Synthetic profile"}),
            ),
            Change::new(
                "run_create",
                json!({"id":r,"month":"2026-09-01","title":"Synthetic UI fixture","reason":"Synthetic run"}),
            ),
            Change::new(
                "item_add",
                json!({"id":i,"run_id":r,"employee_id":eid,"reason":"Synthetic worksheet"}),
            ),
        ] {
            s.execute(Action::Payroll(a)).unwrap();
        }
        let i = s.data.payroll.items[0].clone();
        (s, i)
    }
    #[test]
    fn refreshing_source_keeps_unsaved_adjustments_but_requires_new_review() {
        let (mut store, item) = fixture();
        let mut worksheet = item.worksheet.clone();
        worksheet.adjustments.push(Component {
            label: "Unsaved allowance".into(),
            kind: "taxable_allowance".into(),
            amount_cents: 50000,
            reference: String::new(),
        });
        worksheet.reviewed = true;
        worksheet.review_note = "Old review".into();
        let a = Change::new(
            "item_refresh",
            json!({"id":item.id,"expected_revision":item.revision,"reason":"Synthetic source refresh"}),
        );
        let mut state = State {
            item: Some(ItemForm {
                cutoff: None,
                item,
                worksheet,
                reason: "Unsaved reason".into(),
                new_date: "2026-09-01".into(),
            }),
            pending: Some(a.clone()),
            ..Default::default()
        };
        store.execute(Action::Payroll(a)).unwrap();
        assert!(state.accept(&store.data));
        let f = state.item.unwrap();
        assert_eq!(f.worksheet.adjustments[0].label, "Unsaved allowance");
        assert!(!f.worksheet.reviewed);
        assert!(f.worksheet.review_note.is_empty());
        assert_eq!(f.item.revision, 1);
    }
    #[test]
    fn unrelated_refresh_keeps_open_salary_form() {
        let (store, _) = fixture();
        let mut state = State {
            profile: Some(ProfileForm {
                id: Uuid::new_v4(),
                employee: None,
                date: "2026-09-01".into(),
                pay: Pay::default(),
                revision: None,
                reason: "Unsaved".into(),
            }),
            ..Default::default()
        };
        assert!(state.accept(&store.data));
        assert_eq!(state.profile.unwrap().reason, "Unsaved");
    }
    #[test]
    fn cutoff_source_refresh_preserves_unsaved_adjustments_and_period() {
        let (mut store, monthly) = fixture();
        let run = Uuid::new_v4();
        let item = Uuid::new_v4();
        let mon = NaiveDate::from_ymd_opt(2026, 9, 1).unwrap();
        for a in [
            Change::new(
                "cutoff_run_create",
                json!({"id":run,"month":mon,"part":1,"pay_date":"2026-09-15","title":"Synthetic cutoff","reason":"Synthetic period"}),
            ),
            Change::new(
                "cutoff_item_add",
                json!({"id":item,"run_id":run,"employee_id":monthly.employee_id,"reason":"Synthetic scope"}),
            ),
        ] {
            store.execute(Action::Payroll(a)).unwrap();
        }
        let saved = store.data.payroll.cutoffs.items[0].base.clone();
        let mut w = saved.worksheet.clone();
        w.adjustments.push(Component {
            label: "Unsaved cutoff allowance".into(),
            kind: "taxable_allowance".into(),
            amount_cents: 1234,
            reference: String::new(),
        });
        w.reviewed = true;
        w.review_note = "Previous review".into();
        let a = Change::new(
            "cutoff_item_refresh",
            json!({"id":item,"expected_revision":saved.revision,"reason":"Synthetic source refresh"}),
        );
        let mut state = State {
            item: Some(ItemForm {
                cutoff: Some((mon, 1)),
                item: saved,
                worksheet: w,
                reason: "Unsaved reason".into(),
                new_date: mon.to_string(),
            }),
            pending: Some(a.clone()),
            ..Default::default()
        };
        store.execute(Action::Payroll(a)).unwrap();
        assert!(state.accept(&store.data));
        let f = state.item.unwrap();
        assert_eq!(f.cutoff, Some((mon, 1)));
        assert_eq!(f.worksheet.adjustments[0].amount_cents, 1234);
        assert!(!f.worksheet.reviewed);
        assert!(f.worksheet.review_note.is_empty());
        assert_eq!(f.item.revision, 1);
    }
}
struct ProfileForm {
    id: Uuid,
    employee: Option<Uuid>,
    date: String,
    pay: Pay,
    revision: Option<i64>,
    reason: String,
}
struct HolidayForm {
    id: Uuid,
    date: String,
    name: String,
    kind: String,
    source: String,
    revision: Option<i64>,
    reason: String,
}
struct RunForm {
    id: Uuid,
    month: String,
    title: String,
    reason: String,
}
pub(super) struct ItemForm {
    pub(super) cutoff: Option<(NaiveDate, u8)>,
    pub(super) item: Item,
    pub(super) worksheet: Worksheet,
    pub(super) reason: String,
    pub(super) new_date: String,
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
impl State {
    pub fn editing(&self) -> bool {
        self.finals.editing() || self.payments.editing() || self.tax.editing() || self.thirteenth.editing() || self.profile.is_some()
            || self.cutoffs.editing()
            || self.holiday.is_some()
            || self.run.is_some()
            || self.item.is_some()
            || self.decision.is_some()
    }
    pub fn accept(&mut self, data: &Snapshot) -> bool {
        let Some(a) = self.pending.take() else {
            return true;
        };
        if !a.saved(data) {
            return false;
        }
        if a.operation.starts_with("final_"){self.finals.accept(&a,data);return true;}
        if a.operation.starts_with("receipt_") {self.payments.accept();return true;}
        if a.operation.starts_with("tax_") {self.tax.accept(&a,data);return true;}
        if a.operation.starts_with("thirteenth_") {self.thirteenth.accept(&a,data);return true;}
        if a.operation == "item_refresh" || a.operation == "cutoff_item_refresh" {
            if let (Some(f), Some(item)) = (
                self.item.as_mut(),
                if a.operation == "cutoff_item_refresh" {
                    data.payroll
                        .cutoffs
                        .items
                        .iter()
                        .map(|i| &i.base)
                        .find(|i| {
                            Some(i.id) == a.record["id"].as_str().and_then(|s| s.parse().ok())
                        })
                } else {
                    data.payroll.items.iter().find(|i| {
                        Some(i.id) == a.record["id"].as_str().and_then(|s| s.parse().ok())
                    })
                },
            ) {
                f.item = item.clone();
                f.worksheet.reviewed = false;
                f.worksheet.review_note.clear();
                f.reason.clear();
                return true;
            }
        }
        if a.operation == "run_create" {
            self.selected = a.record["id"].as_str().and_then(|s| s.parse().ok());
        }
        if a.operation == "cutoff_run_create" {
            self.cutoffs.selected = a.record["id"].as_str().and_then(|s| s.parse().ok());
        }
        self.cutoffs.close_forms();
        self.profile = None;
        self.holiday = None;
        self.run = None;
        self.item = None;
        self.decision = None;
        self.warnings.clear();
        true
    }
}
pub(super) fn amount(ui: &mut Ui, label: &str, cents: &mut i64) {
    ui.horizontal_wrapped(|ui| {
        ui.label(label);
        ui.add(
            egui::DragValue::new(cents)
                .speed(100.0)
                .range(0..=10_000_000_000i64)
                .custom_formatter(|v, _| format!("{:.2}", v / 100.0))
                .custom_parser(|s| {
                    let s = s.trim();
                    let mut parts = s.split('.');
                    let whole = parts.next()?.parse::<i64>().ok()?;
                    let dec = parts.next().unwrap_or("");
                    if parts.next().is_some()
                        || dec.len() > 2
                        || !dec.bytes().all(|x| x.is_ascii_digit())
                        || whole < 0
                    {
                        return None;
                    }
                    let minor = if dec.is_empty() {
                        0
                    } else {
                        dec.parse::<i64>().ok()? * if dec.len() == 1 { 10 } else { 1 }
                    };
                    Some((whole.checked_mul(100)? + minor) as f64)
                }),
        );
    });
}
pub(super) fn field(ui: &mut Ui, label: &str, s: &mut String) {
    ui.label(label);
    ui.add(egui::TextEdit::singleline(s).desired_width(420.0));
}
pub(super) fn note(ui: &mut Ui, label: &str, s: &mut String) {
    ui.label(label);
    ui.add(
        egui::TextEdit::multiline(s)
            .desired_rows(2)
            .desired_width(f32::INFINITY),
    );
}
pub(super) fn choice(
    ui: &mut Ui,
    id: impl std::hash::Hash + std::fmt::Debug,
    label: &str,
    s: &mut String,
    values: &[&str],
) {
    egui::ComboBox::from_id_salt(id)
        .selected_text(format!("{label}: {}", s.replace('_', " ")))
        .show_ui(ui, |ui| {
            for v in values {
                ui.selectable_value(s, (*v).into(), v.replace('_', " "));
            }
        });
}
pub(super) fn footer(ui: &mut Ui, a: &Change, data: &Snapshot, label: &str) -> (bool, bool) {
    let valid = a.payload(data);
    if let Err(e) = &valid {
        ui.label(RichText::new(e.to_string()).color(muted(ui)));
    }
    let mut x = (false, false);
    ui.horizontal_wrapped(|ui| {
        x.0 = ui
            .add_enabled(valid.is_ok(), egui::Button::new(label))
            .clicked();
        x.1 = ui.button("Discard changes").clicked();
    });
    x
}
pub(super) fn employee_choice(ui: &mut Ui, data: &Snapshot, id: &str, selected: &mut Option<Uuid>) {
    egui::ComboBox::from_id_salt(id)
        .width(340.0)
        .selected_text(
            selected
                .map(|id| format!("{}", data.employee_name(id)))
                .unwrap_or_else(|| "Choose employee".into()),
        )
        .show_ui(ui, |ui| {
            for e in &data.employees {
                ui.selectable_value(
                    selected,
                    Some(e.id),
                    format!(
                        "{} · {}{}",
                        e.employee_code,
                        e.full_name,
                        if e.status == EmployeeStatus::Archived {
                            " (archived)"
                        } else {
                            ""
                        }
                    ),
                );
            }
        });
}
fn components(ui: &mut Ui, id: &str, cs: &mut Vec<Component>) {
    let mut remove = None;
    for (i, c) in cs.iter_mut().enumerate() {
        ui.push_id((id, i), |ui| {
            ui.separator();
            ui.horizontal_wrapped(|ui| {
                ui.label(format!("{}.", i + 1));
                ui.text_edit_singleline(&mut c.label);
                choice(ui, "kind", "Kind", &mut c.kind, payroll::COMPONENT_KINDS);
                if ui.button("Remove").clicked() {
                    remove = Some(i);
                }
            });
            amount(ui, "Amount (PHP)", &mut c.amount_cents);
            field(
                ui,
                "Evidence / consent / payment reference",
                &mut c.reference,
            );
        });
    }
    if let Some(i) = remove {
        cs.remove(i);
    }
    if cs.len() < 40 && ui.button("Add component").clicked() {
        cs.push(Component {
            label: String::new(),
            kind: "taxable_allowance".into(),
            ..Default::default()
        });
    }
}
pub(super) fn totals(ui: &mut Ui, t: &payroll::Totals, id: impl std::hash::Hash + std::fmt::Debug) {
    egui::Grid::new(id)
        .num_columns(2)
        .spacing([32.0, 7.0])
        .striped(true)
        .show(ui, |ui| {
            for (label, n) in payroll::totals_rows(t) {
                ui.vertical(|ui| {
                    ui.set_width(240.0);
                    ui.label(label);
                });
                ui.vertical(|ui| {
                    ui.set_width(190.0);
                    ui.label(RichText::new(payroll::money(n)).strong());
                });
                ui.end_row();
            }
        });
}
impl HrisApp {
    pub(super) fn send_payroll(&mut self, a: Change) {
        self.payroll_state.pending = Some(a.clone());
        self.send(Command::Change(Action::Payroll(a)));
    }
    pub(super) fn payroll(&mut self, ui: &mut Ui) {
        if self.payroll_state.finals.editing(){self.payroll_final_forms(ui);return;}
        if self.payroll_state.payments.editing() {self.payroll_payment_forms(ui);return;}
        if self.payroll_state.tax.editing() {self.payroll_tax_forms(ui);return;}
        if self.payroll_state.thirteenth.editing() {self.payroll_thirteenth_forms(ui);return;}
        if self.payroll_state.cutoffs.editing() {
            self.payroll_cutoff_forms(ui);
            return;
        }
        if self.payroll_state.year == 0 {
            self.payroll_state.year = self.data.company.today().year();
        }
        if self.payroll_state.editing() {
            self.payroll_forms(ui);
            return;
        }
        let hr = self.data.access.role.manages_hr();
        if !hr && ![3, 6, 7, 8, 10].contains(&self.payroll_state.tab) {
            self.payroll_state.tab = 3;
        }
        ui.horizontal_wrapped(|ui| {
            for (i, label) in [
                "Monthly registers",
                "Salaries & benefits",
                "Payroll holidays",
                "My payslips",
                "Reports",
                "Statutory rules",
                "Cutoffs",
                "13th-month pay",
                "Annual tax",
                "Payments",
                "Final pay",
            ]
            .iter()
            .enumerate()
            {
                if (hr || i == 3 || i == 6 || i == 7 || i == 8 || i == 10)
                    && ui
                        .selectable_label(
                            self.payroll_state.tab == i,
                            if i == 3 && hr { "Payslips" } else { *label },
                        )
                        .clicked()
                {
                    self.payroll_state.tab = i;
                    self.payroll_state.selected = None;
                    self.payroll_state.slip = None;
                }
            }
        });
        ui.add_space(12.0);
        match self.payroll_state.tab {
            0 => self.payroll_runs(ui),
            1 => self.payroll_profiles(ui),
            2 => self.payroll_holidays(ui),
            3 => self.payroll_slips(ui),
            4 => self.payroll_reports(ui),
            6 => self.payroll_cutoffs(ui),
            7 => self.payroll_thirteenth(ui),
            8 => self.payroll_tax(ui),
            9 => self.payroll_payments(ui),
            10 => self.payroll_final(ui),
            _ => {
                card(ui, |ui| {
                    ui.heading("Philippine private-sector payroll");
                    ui.label(format!(
                        "Rule pack: {} · verified 23 October 2026",
                        payroll::RULE_VERSION
                    ));
                    ui.label("Monthly reconciliation register. Record interim payments as advances; this is not a monthly-only payment schedule. No bank transfer or government filing occurs here.");
                    ui.separator();
                    ui.label("Ordinary OT:125%. Rest/special day:130%; special + rest:150%; regular holiday:200%/260% on rest; double regular:300%/390%. Nonordinary OT multiplies by130%. Night differential adds10% of the applicable normal/OT rate for22:00–06:00.");
                    ui.label("SSS:5% employee /10% employer on MSC5,000–35,000 plus employer-only EC. PhilHealth:5% fixed monthly basic,10,000–100,000, equally shared. Pag-IBIG:1%/2% employee,2% employer, fund salary capped10,000. Monthly BIR table:2023 onwards.");
                    ui.label("Rice and laundry use monthly caps2,500 and400 respectively. Other allowances default taxable. Regional minimum wages, unusual coverage and fringe-benefit tax need separate review. Use13th-month pay for actual benefit payouts and Annual tax for recorded-pay closing reconciliation; monthly basic pay /12 remains an accrual memo.");
                    for (title, url) in payroll::SOURCES {
                        ui.hyperlink_to(*title, *url);
                    }
                });
            }
        }
    }
    fn payroll_profiles(&mut self, ui: &mut Ui) {
        if ui.button("Add salary version").clicked() {
            self.payroll_state.profile = Some(ProfileForm {
                id: Uuid::new_v4(),
                employee: None,
                date: payroll::month_start(self.data.company.today()).to_string(),
                pay: Pay::default(),
                revision: None,
                reason: String::new(),
            });
        }
        ui.label("HR/Admin only. Salary versions do not change Employment or earlier payslips. Add a new effective month for a raise; review mid-month proration in the worksheet.");
        let mut rows = self.data.payroll.profiles.clone();
        rows.sort_by_key(|p| {
            (
                self.data.employee_name(p.employee_id).to_owned(),
                std::cmp::Reverse(p.effective_on),
            )
        });
        for p in rows {
            ui.add_space(8.0);
            card(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.label(RichText::new(self.data.employee_name(p.employee_id)).strong());
                    ui.label(format!(
                        "From{} · {} {}",
                        p.effective_on,
                        payroll::money(p.pay.rate_cents),
                        p.pay.basis
                    ));
                    if ui.button("Edit version").clicked() {
                        self.payroll_state.profile = Some(ProfileForm {
                            id: p.id,
                            employee: Some(p.employee_id),
                            date: p.effective_on.to_string(),
                            pay: p.pay.clone(),
                            revision: Some(p.revision),
                            reason: String::new(),
                        });
                    }
                });
                ui.label(format!(
                    "{} recurring allowance / benefit / deduction components",
                    p.pay.components.len()
                ));
            });
        }
        if self.data.payroll.profiles.is_empty() {
            empty(
                ui,
                "No salary profiles yet. Add the first salary version to prepare payroll.",
            );
        }
    }
    fn payroll_holidays(&mut self, ui: &mut Ui) {
        if ui.button("Add payroll holiday").clicked() {
            self.payroll_state.holiday = Some(HolidayForm {
                id: Uuid::new_v4(),
                date: self.data.company.today().to_string(),
                name: String::new(),
                kind: "regular".into(),
                source: String::new(),
                revision: None,
                reason: String::new(),
            });
        }
        ui.label("Classify national and local proclamations here. The leave calendar does not distinguish regular/special days; payroll classification does not change saved leave charges. To withdraw a classification, retain its date and change it to Ordinary with a reason.");
        let mut hs = self.data.payroll.holidays.clone();
        hs.sort_by_key(|h| h.date);
        for h in hs {
            card(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.label(format!(
                        "{} · {} · {}",
                        h.date,
                        h.name,
                        h.kind.replace('_', " ")
                    ));
                    if ui.button("Edit classification").clicked() {
                        self.payroll_state.holiday = Some(HolidayForm {
                            id: h.id,
                            date: h.date.to_string(),
                            name: h.name.clone(),
                            kind: h.kind.clone(),
                            source: h.source.clone(),
                            revision: Some(h.revision),
                            reason: String::new(),
                        });
                    }
                });
                ui.label(&h.source);
            });
        }
    }
    fn payroll_runs(&mut self, ui: &mut Ui) {
        if let Some(id) = self.payroll_state.selected {
            if let Some(r) = self.data.payroll.runs.iter().find(|r| r.id == id).cloned() {
                self.payroll_run_detail(ui, &r);
                return;
            }
            self.payroll_state.selected = None;
        }
        if ui.button("Create monthly payroll").clicked() {
            let mon = payroll::month_start(self.data.company.today());
            self.payroll_state.run = Some(RunForm {
                id: Uuid::new_v4(),
                month: mon.to_string(),
                title: format!("{} payroll", mon.format("%B %Y")),
                reason: String::new(),
            });
        }
        ui.label("Prepare worksheets, submit for another Administrator's approval, then record payment. Approved payslips stay fixed when salary or attendance changes.");
        let mut rs = self.data.payroll.runs.clone();
        rs.sort_by_key(|r| std::cmp::Reverse(r.month));
        for r in rs {
            ui.add_space(8.0);
            card(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    if ui.link(RichText::new(&r.title).strong()).clicked() {
                        self.payroll_state.selected = Some(r.id);
                    }
                    ui.label(format!(
                        "{} · {} · {} employees",
                        r.month.format("%B %Y"),
                        r.status,
                        self.data
                            .payroll
                            .items
                            .iter()
                            .filter(|i| i.run_id == r.id)
                            .count()
                    ));
                });
            });
        }
        if self.data.payroll.runs.is_empty() {
            empty(ui, "Create the first payroll after adding salary profiles.");
        }
    }
    fn payroll_decide(&mut self, r: &Run, op: &str) {
        self.payroll_state.decision = Some(Decision {
            operation: op.into(),
            id: r.id,
            revision: r.revision,
            reason: String::new(),
            confirmed: false,
            paid_on: self.data.company.today().to_string(),
            payment_reference: String::new(),
        });
    }
    fn payroll_run_detail(&mut self, ui: &mut Ui, r: &Run) {
        if ui.button("Back to payroll runs").clicked() {
            self.payroll_state.selected = None;
            return;
        }
        ui.heading(&r.title);
        ui.label(format!(
            "{} · {} · revision{}",
            r.month.format("%B %Y"),
            r.status,
            r.revision
        ));
        ui.horizontal_wrapped(|ui| {
            if r.status == "draft" && ui.button("Submit for approval").clicked() {
                self.payroll_decide(r, "run_submit");
            }
            if r.status == "submitted" {
                if ui.button("Return to draft").clicked() {
                    self.payroll_decide(r, "run_return");
                }
                if self.data.access.role == Role::Administrator
                    && r.submitted_by != Some(self.data.access.user_id)
                    && ui.button("Approve & issue payslips").clicked()
                {
                    self.payroll_decide(r, "run_approve");
                }
            }
            if ["draft", "submitted"].contains(&r.status.as_str())
                && ui.button("Cancel run").clicked()
            {
                self.payroll_decide(r, "run_cancel");
            }
            if r.status == "approved"
                && self.data.access.role == Role::Administrator
                && ui.button("Record payment").clicked()
            {
                self.payroll_decide(r, "run_paid");
            }
            if ui.button("Export register CSV").clicked() {
                self.payroll_export("register", Some(r.id));
            }
            if ui.button("Statutory worksheet CSV").clicked() {
                self.payroll_export("statutory", Some(r.id));
            }
        });
        if r.status == "submitted" && r.submitted_by == Some(self.data.access.user_id) {
            ui.label(
                "Another Administrator must approve this run. The submitter cannot approve it.",
            );
        }
        if let Some(d) = r.paid_on {
            ui.label(format!("Payment recorded{} · {}", d, r.payment_reference));
        }
        if r.status == "draft" {
            card(ui, |ui| {
                employee_choice(
                    ui,
                    &self.data,
                    "payroll_add_employee",
                    &mut self.payroll_state.employee,
                );
                if let Some(eid) = self.payroll_state.employee {
                    let exists = self
                        .data
                        .payroll
                        .items
                        .iter()
                        .any(|i| i.run_id == r.id && i.employee_id == eid);
                    let profile = self.data.payroll.profile(eid, r.month).is_some();
                    if ui
                        .add_enabled(
                            !exists && profile,
                            egui::Button::new("Add employee worksheet"),
                        )
                        .clicked()
                    {
                        self.send_payroll(Change::new("item_add",json!({"id":Uuid::new_v4(),"run_id":r.id,"employee_id":eid,"reason":"Add employee to reviewed payroll scope"})));
                    }
                    if !profile {
                        ui.label("Add a salary version effective on or before this month first.");
                    }
                }
            });
        }
        let rows: Vec<_> = self
            .data
            .payroll
            .items
            .iter()
            .filter(|i| i.run_id == r.id)
            .cloned()
            .collect();
        let (gross, net) = rows
            .iter()
            .fold((0, 0), |(g, n), i| (g + i.totals.gross, n + i.totals.net));
        ui.label(format!(
            "{} employee(s) · gross{} · net due{}",
            rows.len(),
            payroll::money(gross),
            payroll::money(net)
        ));
        for i in rows {
            ui.add_space(8.0);
            card(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.label(
                        RichText::new(format!("{} · {}", i.employee_code, i.employee_name))
                            .strong(),
                    );
                    ui.label(if i.worksheet.reviewed {
                        "Reviewed"
                    } else {
                        "Needs review"
                    });
                    ui.label(format!("Net{}", payroll::money(i.totals.net)));
                    if r.status == "draft" && ui.button("Remove from draft").clicked() {
                        self.payroll_state.decision = Some(Decision {
                            operation: "item_remove".into(),
                            id: i.id,
                            revision: i.revision,
                            reason: String::new(),
                            confirmed: false,
                            paid_on: String::new(),
                            payment_reference: String::new(),
                        });
                    }
                    if r.status == "draft" && ui.button("Open worksheet").clicked() {
                        self.payroll_state.item = Some(ItemForm {
                            cutoff: None,
                            worksheet: i.worksheet.clone(),
                            item: i.clone(),
                            reason: String::new(),
                            new_date: r.month.to_string(),
                        });
                    }
                    if ["approved", "paid"].contains(&r.status.as_str())
                        && ui.button("View payslip").clicked()
                    {
                        self.payroll_state.slip = Some(i.id);
                        self.payroll_state.tab = 3;
                    }
                });
                ui.collapsing("Calculation breakdown", |ui| {
                    totals(ui, &i.totals, ("run_totals", i.id));
                });
            });
        }
        self.payroll_history(ui, r.id);
    }
    fn payroll_forms(&mut self, ui: &mut Ui) {
        if let Some(mut f) = self.payroll_state.profile.take() {
            ui.heading("Salary & recurring benefits");
            ui.add_enabled_ui(f.revision.is_none(), |ui| {
                employee_choice(ui, &self.data, "salary_employee", &mut f.employee);
                field(ui, "Effective month first (YYYY-MM-01)", &mut f.date);
            });
            choice(
                ui,
                "salary_basis",
                "Salary basis",
                &mut f.pay.basis,
                &["monthly", "daily"],
            );
            ui.label("Monthly means paid on all calendar days, including unworked rest days and holidays. The divisor converts it to a daily/hourly rate; it is not a fixed26-day assumption.");
            amount(ui, "Salary rate (PHP)", &mut f.pay.rate_cents);
            ui.horizontal(|ui| {
                ui.label("Annual divisor");
                ui.add(egui::DragValue::new(&mut f.pay.annual_divisor).range(365..=366));
            });
            if f.pay.basis == "daily" {
                amount(
                    ui,
                    "Fixed monthly basic reference (PhilHealth)",
                    &mut f.pay.monthly_reference_cents,
                );
            }
            ui.checkbox(
                &mut f.pay.premium_eligible,
                "Covered by overtime, holiday/rest premium and night differential",
            );
            ui.checkbox(
                &mut f.pay.minimum_wage_earner,
                "HR verified minimum-wage-earner income-tax status",
            );
            ui.horizontal_wrapped(|ui| {
                ui.checkbox(&mut f.pay.sss, "SSS");
                ui.checkbox(&mut f.pay.philhealth, "PhilHealth");
                ui.checkbox(&mut f.pay.pagibig, "Pag-IBIG");
            });
            note(
                ui,
                "Coverage / wage-order basis (required for exceptions)",
                &mut f.pay.coverage_note,
            );
            ui.collapsing("Government membership identifiers (private)", |ui| {
                field(ui, "TIN", &mut f.pay.tin);
                field(ui, "SSS number", &mut f.pay.sss_number);
                field(ui, "PhilHealth number", &mut f.pay.philhealth_number);
                field(ui, "Pag-IBIG number", &mut f.pay.pagibig_number);
            });
            ui.heading("Recurring monthly components");
            ui.label("Employer benefits are noncash cost memos. Deductions need lawful consent/evidence; advances need a prior-payment reference. Rice/laundry exemptions are capped across all components.");
            components(ui, "recurring", &mut f.pay.components);
            note(ui, "Reason for salary version / change", &mut f.reason);
            let a = Change::new(
                "profile_save",
                json!({"id":f.id,"employee_id":f.employee,"effective_on":f.date,"pay":f.pay,"expected_revision":f.revision,"reason":f.reason}),
            );
            let (save, cancel) = footer(ui, &a, &self.data, "Save salary version");
            if save {
                self.send_payroll(a);
            }
            if !cancel {
                self.payroll_state.profile = Some(f);
            }
            return;
        }
        if let Some(mut f) = self.payroll_state.holiday.take() {
            ui.heading("Payroll holiday classification");
            field(ui, "Date (YYYY-MM-DD)", &mut f.date);
            field(ui, "Holiday name", &mut f.name);
            choice(
                ui,
                "payroll_holiday_kind",
                "Day kind",
                &mut f.kind,
                payroll::DAY_KINDS,
            );
            field(
                ui,
                "Proclamation / authority / correction source",
                &mut f.source,
            );
            note(ui, "Reason", &mut f.reason);
            let a = Change::new(
                "holiday_save",
                json!({"id":f.id,"date":f.date,"name":f.name,"kind":f.kind,"source":f.source,"expected_revision":f.revision,"reason":f.reason}),
            );
            let (save, cancel) = footer(ui, &a, &self.data, "Save classification");
            if save {
                self.send_payroll(a);
            }
            if !cancel {
                self.payroll_state.holiday = Some(f);
            }
            return;
        }
        if let Some(mut f) = self.payroll_state.run.take() {
            ui.heading("Create monthly payroll register");
            field(ui, "Month first (YYYY-MM-01)", &mut f.month);
            field(ui, "Run title", &mut f.title);
            note(ui, "Reason", &mut f.reason);
            ui.label("A current month may be prepared as a draft. Submit/approve completed 2026 months only. Interim payments are recorded as advances; no funds move here.");
            let a = Change::new(
                "run_create",
                json!({"id":f.id,"month":f.month,"title":f.title,"reason":f.reason}),
            );
            let (save, cancel) = footer(ui, &a, &self.data, "Create payroll");
            if save {
                self.send_payroll(a);
            }
            if !cancel {
                self.payroll_state.run = Some(f);
            }
            return;
        }
        if let Some(mut f) = self.payroll_state.item.take() {
            self.payroll_item_form(ui, &mut f);
            return;
        }
        if let Some(mut f) = self.payroll_state.decision.take() {
            ui.heading(match f.operation.as_str() {
                "run_submit" => "Submit payroll for approval",
                "run_approve" => "Approve payroll and issue payslips",
                "run_paid" => "Record payroll payment",
                "run_return" => "Return payroll to draft",
                "run_cancel" => "Cancel unissued payroll",
                "item_remove" => "Remove employee from draft",
                _ => "Review payroll action",
            });
            ui.label("Approval issues immutable payslips; payment recording does not transfer funds. An approved run cannot be overwritten or cancelled. Correct differences through a later documented adjustment.");
            if f.operation == "run_submit" {
                let included: Vec<_> = self
                    .data
                    .payroll
                    .items
                    .iter()
                    .filter(|i| i.run_id == f.id)
                    .map(|i| i.employee_name.clone())
                    .collect();
                ui.label(format!(
                    "Included{}: {}",
                    included.len(),
                    included.join(", ")
                ));
                let omitted = self
                    .data
                    .active()
                    .filter(|e| {
                        !self
                            .data
                            .payroll
                            .items
                            .iter()
                            .any(|i| i.run_id == f.id && i.employee_id == e.id)
                    })
                    .map(|e| e.full_name.clone())
                    .collect::<Vec<_>>();
                ui.label(format!(
                    "Not included{}: {}",
                    omitted.len(),
                    omitted.join(", ")
                ));
                ui.checkbox(
                    &mut f.confirmed,
                    "I reviewed which employees are included and omitted",
                );
            }
            if f.operation == "run_paid" {
                field(ui, "Actual payment date (YYYY-MM-DD)", &mut f.paid_on);
                field(
                    ui,
                    "Bank / payroll payment reference",
                    &mut f.payment_reference,
                );
            }
            note(ui, "Reason / review outcome", &mut f.reason);
            let mut v = json!({"id":f.id,"expected_revision":f.revision,"reason":f.reason});
            if f.operation == "run_submit" {
                v["scope_reviewed"] = f.confirmed.into();
            }
            if f.operation == "run_paid" {
                v["paid_on"] = f.paid_on.clone().into();
                v["payment_reference"] = f.payment_reference.clone().into();
            }
            let a = Change::new(&f.operation, v);
            let (save, cancel) = footer(ui, &a, &self.data, "Confirm payroll action");
            if save {
                self.send_payroll(a);
            }
            if !cancel {
                self.payroll_state.decision = Some(f);
            }
        }
    }
    fn payroll_item_form(&mut self, ui: &mut Ui, f: &mut ItemForm) {
        ui.heading(format!(
            "{} · {}",
            f.item.employee_code, f.item.employee_name
        ));
        ui.label(format!(
            "Saved salary {} {} · version {}",
            payroll::money(f.item.profile.pay.rate_cents),
            f.item.profile.pay.basis,
            f.item.profile.effective_on
        ));
        let mon = f.cutoff.map(|c| c.0).unwrap_or_else(|| {
            self.data
                .payroll
                .runs
                .iter()
                .find(|r| r.id == f.item.run_id)
                .map(|r| r.month)
                .unwrap_or_else(|| payroll::month_start(self.data.company.today()))
        });
        let (lo, hi) = f
            .cutoff
            .and_then(|(m, p)| crate::payroll_cutoffs::bounds(m, p).ok())
            .unwrap_or((mon, payroll::month_end(mon)));
        if let Some((_, part)) = f.cutoff {
            ui.label(format!("Cutoff {part}: {lo} to {hi}. Monthly government contributions are settled on cutoff 2. Recurring amounts are allocated once across the two halves."));
        }
        ui.horizontal_wrapped(|ui|{if ui.button("Replace days with attendance proposal").clicked(){let(ds,warnings)=payroll::attendance_proposal(&self.data,f.item.employee_id,mon);f.worksheet.days=ds.into_iter().filter(|d|d.date>=lo&&d.date<=hi).collect();f.worksheet.reviewed=false;self.payroll_state.warnings=warnings;}if ui.button("Refresh source & require new review").clicked(){let a=Change::new(if f.cutoff.is_some(){"cutoff_item_refresh"}else{"item_refresh"},json!({"id":f.item.id,"expected_revision":f.item.revision,"reason":"Refresh payroll source facts and require a fresh review"}));self.send_payroll(a);}});
        ui.label("Proposal only. Review statutory8-hour workdays, midnight holiday boundaries, break times and approved OT. Missing days are not automatically unpaid. The server checks source changes before submission and approval.");
        for w in &self.payroll_state.warnings {
            ui.label(RichText::new(w).color(muted(ui)));
        }
        let mut remove = None;
        for (i, d) in f.worksheet.days.iter_mut().enumerate() {
            ui.push_id(i, |ui| {
                card(ui, |ui| {
                    ui.horizontal_wrapped(|ui| {
                        ui.strong(d.date.to_string());
                        choice(ui, "kind", "Day kind", &mut d.kind, payroll::DAY_KINDS);
                        ui.checkbox(&mut d.rest, "Rest day");
                        if ui.button("Remove day").clicked() {
                            remove = Some(i);
                        }
                    });
                    egui::Grid::new(("worksheet_minutes", i))
                        .num_columns(2)
                        .spacing([16.0, 7.0])
                        .show(ui, |ui| {
                            for (label, n) in [
                                ("Regular worked minutes", &mut d.regular_minutes),
                                ("OT minutes", &mut d.overtime_minutes),
                                ("Night regular subset", &mut d.night_regular),
                                ("Night OT subset", &mut d.night_overtime),
                                ("Paid leave minutes (daily pay)", &mut d.paid_leave_minutes),
                                (
                                    "Unworked regular-holiday minutes (eligible daily pay)",
                                    &mut d.unworked_holiday_minutes,
                                ),
                                ("Unpaid minutes (monthly deduction)", &mut d.unpaid_minutes),
                            ] {
                                ui.vertical(|ui| {
                                    ui.set_width(260.0);
                                    ui.label(label);
                                });
                                ui.add(egui::DragValue::new(n).range(0..=960));
                                ui.end_row();
                            }
                        });
                });
            });
        }
        if let Some(i) = remove {
            f.worksheet.days.remove(i);
            f.worksheet.reviewed = false;
        }
        ui.horizontal_wrapped(|ui| {
            ui.label("Add date");
            ui.text_edit_singleline(&mut f.new_date);
            if ui.button("Add reviewed day row").clicked() {
                if let Ok(date) = f.new_date.parse::<NaiveDate>() {
                    if date >= lo && date <= hi && !f.worksheet.days.iter().any(|d| d.date == date)
                    {
                        let kind = self
                            .data
                            .payroll
                            .holidays
                            .iter()
                            .find(|h| h.date == date)
                            .map(|h| h.kind.clone())
                            .unwrap_or_else(|| "ordinary".into());
                        f.worksheet.days.push(Day {
                            date,
                            kind,
                            ..Default::default()
                        });
                        f.worksheet.days.sort_by_key(|d| d.date);
                        f.worksheet.reviewed = false;
                    }
                }
            }
        });
        ui.collapsing("Proration and contribution-basis overrides", |ui| {
            for (index, (label, n)) in [
                (
                    "Prorated monthly basic",
                    &mut f.worksheet.basic_override_cents,
                ),
                (
                    "SSS monthly compensation basis",
                    &mut f.worksheet.sss_basis_override,
                ),
                (
                    "PhilHealth fixed monthly basic",
                    &mut f.worksheet.philhealth_basis_override,
                ),
                (
                    "Pag-IBIG monthly fund salary",
                    &mut f.worksheet.pagibig_basis_override,
                ),
            ]
            .into_iter()
            .enumerate()
            {
                if index > 0 && f.cutoff.is_some_and(|c| c.1 == 1) {
                    continue;
                }
                let mut enabled = n.is_some();
                if ui.checkbox(&mut enabled, label).changed() {
                    *n = if enabled { Some(0) } else { None };
                }
                if let Some(v) = n {
                    amount(ui, "Override (PHP)", v);
                }
            }
        });
        ui.heading(if f.cutoff.is_some() {
            "This cutoff's adjustments / advances"
        } else {
            "This month's adjustments / advances"
        });
        components(ui, "adjustments", &mut f.worksheet.adjustments);
        note(
            ui,
            "Exception review (OT without current authorization / proration / statutory basis)",
            &mut f.worksheet.exception_note,
        );
        note(
            ui,
            "HR review notes (not included in employee payslips)",
            &mut f.worksheet.review_note,
        );
        ui.checkbox(
            &mut f.worksheet.reviewed,
            "I reviewed dates, coverage, pay, holiday eligibility, deductions and employee scope",
        );
        note(ui, "Save reason", &mut f.reason);
        let preview = if let Some((_, part)) = f.cutoff {
            crate::payroll_cutoffs::previous(&f.item)
                .and_then(|p| {
                    crate::payroll_cutoffs::calculate(
                        &f.item.profile.pay,
                        &f.worksheet,
                        mon,
                        part,
                        p.as_ref(),
                    )
                })
                .map(|c| c.totals)
        } else {
            payroll::calculate(&f.item.profile.pay, &f.worksheet, mon)
        };
        match preview {
            Ok(t) => {
                card(ui, |ui| {
                    ui.heading("Calculation preview");
                    totals(ui, &t, "worksheet_totals");
                });
                if t.net < 0 {
                    ui.label(
                        RichText::new(
                            "Negative net pay: correct deductions/advances before approval.",
                        )
                        .color(Color32::RED),
                    );
                }
            }
            Err(e) => {
                ui.label(RichText::new(e.to_string()).color(muted(ui)));
            }
        }
        let a = Change::new(
            if f.cutoff.is_some() {
                "cutoff_item_save"
            } else {
                "item_save"
            },
            json!({"id":f.item.id,"expected_revision":f.item.revision,"worksheet":f.worksheet,"reason":f.reason}),
        );
        let (save, cancel) = footer(ui, &a, &self.data, "Save employee worksheet");
        if save {
            self.send_payroll(a);
        }
        if !cancel {
            self.payroll_state.item = Some(ItemForm {
                cutoff: f.cutoff,
                item: f.item.clone(),
                worksheet: f.worksheet.clone(),
                reason: f.reason.clone(),
                new_date: f.new_date.clone(),
            });
        }
    }
    fn payroll_slips(&mut self, ui: &mut Ui) {
        let visible:Vec<_>=self.data.payroll.slips.iter().filter(|s|s.company_id==self.data.company.id && (self.data.access.role.manages_hr() || self.data.access.employee_id==Some(s.employee_id))).cloned().collect();
        if let Some(id) = self.payroll_state.slip {
            if let Some(s) = visible.iter().find(|s| s.id == id).cloned() {
                if ui.button("Back to payslips").clicked() {
                    self.payroll_state.slip = None;
                    return;
                }
                card(ui, |ui| {
                    ui.heading(&s.company_name);
                    ui.label(
                        RichText::new(format!("PAYSLIP · {}", s.month.format("%B %Y"))).strong(),
                    );
                    ui.label(format!(
                        "{} · {} · {}",
                        s.item.employee_code, s.item.employee_name, s.item.department
                    ));
                    ui.label(format!(
                        "Approved{} · {}",
                        s.approved_at.format("%Y-%m-%d %H:%M UTC"),
                        s.rule_version
                    ));
                    totals(ui, &s.item.totals, "payslip_totals");
                    ui.label("Approved calculation. Payment is recorded separately; this is not a bank receipt.");
                });
                ui.horizontal_wrapped(|ui| {
                    if ui.button("Save printable PDF").clicked() {
                        self.payroll_export("payslip_pdf", Some(s.id));
                    }
                    if ui.button("Export payslip CSV").clicked() {
                        self.payroll_export("payslip", Some(s.id));
                    }
                });
                ui.collapsing("Saved earning / deduction details", |ui| {
                    for c in s
                        .item
                        .profile
                        .pay
                        .components
                        .iter()
                        .chain(&s.item.worksheet.adjustments)
                    {
                        ui.label(format!(
                            "{} · {} · {}",
                            c.label,
                            c.kind.replace('_', " "),
                            payroll::money(c.amount_cents)
                        ));
                    }
                    for d in &s.item.worksheet.days {
                        ui.label(format!(
                            "{} · {} · regular{}m / OT{}m / night{}+{}m",
                            d.date,
                            d.kind,
                            d.regular_minutes,
                            d.overtime_minutes,
                            d.night_regular,
                            d.night_overtime
                        ));
                    }
                });
                return;
            }
            self.payroll_state.slip = None;
        }
        ui.label(if self.data.access.role.manages_hr(){"Approved payslips. HR/Admin can open any approved employee payslip; Managers see only their own."}else{"Your approved payslips. Salary profiles, draft payroll and team pay remain private to HR/Admin."});
        let mut rows = visible.clone();
        rows.sort_by_key(|s| std::cmp::Reverse(s.month));
        for s in rows {
            ui.horizontal_wrapped(|ui| {
                if ui
                    .link(format!(
                        "{} · {} · {}",
                        s.month.format("%B %Y"),
                        s.item.employee_code,
                        s.item.employee_name
                    ))
                    .clicked()
                {
                    self.payroll_state.slip = Some(s.id);
                }
                ui.strong(payroll::money(s.item.totals.net));
            });
        }
        if visible.is_empty() {
            empty(ui, "No approved payslips yet.");
        }
    }
    fn payroll_reports(&mut self, ui: &mut Ui) {
        ui.label("Monthly-register reconciliation only. Cutoff totals are exported from Cutoffs. No government upload/filing or remittance occurs. Annual summaries here cover approved monthly registers; they do not annualize tax or fill prior-employer/opening balances.");
        ui.horizontal(|ui| {
            ui.label("Annual summary year");
            ui.add(egui::DragValue::new(&mut self.payroll_state.year).range(2026..=2200));
            if ui
                .button("Export annual monthly-register totals CSV")
                .clicked()
            {
                self.payroll_export("annual", None);
            }
        });
        for r in self.data.payroll.runs.clone() {
            ui.horizontal_wrapped(|ui| {
                ui.label(format!("{} · {}", r.month.format("%B %Y"), r.status));
                if ui.button("Register").clicked() {
                    self.payroll_export("register", Some(r.id));
                }
                if ui
                    .button("SSS / PhilHealth / Pag-IBIG / BIR worksheet")
                    .clicked()
                {
                    self.payroll_export("statutory", Some(r.id));
                }
            });
        }
    }
    pub(super) fn payroll_export(&mut self, kind: &str, id: Option<Uuid>) {
        let pdf = kind.ends_with("payslip_pdf")||kind=="final_statement_pdf";
        let extension = if pdf { "pdf" } else { "csv" };
        let name = format!(
            "HRIS-payroll-{kind}-{}.{extension}",
            id.map(|id| id.to_string())
                .unwrap_or_else(|| self.payroll_state.year.to_string())
        );
        match crate::reports::choose_export_as(&name, pdf) {
            Ok(Some(destination)) => self.send(Command::PayrollExport {
                kind: kind.into(),
                record_id: id,
                year: self.payroll_state.year,
                destination,
            }),
            Ok(None) => {}
            Err(e) => self.notice = Some((e.to_string(), true)),
        }
    }
    pub(super) fn payroll_history(&self, ui: &mut Ui, id: Uuid) {
        ui.collapsing("Payroll audit history", |ui| {
            for e in self
                .data
                .payroll
                .events
                .iter()
                .filter(|e| e.record_id == id)
                .rev()
            {
                ui.label(format!(
                    "{} · {} · {}",
                    e.recorded_at.format("%Y-%m-%d %H:%M UTC"),
                    e.actor_name,
                    e.kind.replace('_', " ")
                ));
                ui.label(&e.reason);
            }
        });
    }
    #[cfg(feature = "qa-screenshot")]
    pub(super) fn prepare_payroll_preview(&mut self) {
        let form = std::env::var("HRIS_PREVIEW_FORM").unwrap_or_default();
        if !form.starts_with("payroll") {
            return;
        }
        self.page = Page::Payroll;
        if form.starts_with("payroll-final"){self.prepare_final_preview(&form);return;}
        if form.starts_with("payroll-payment") {self.payroll_state.tab=9;self.prepare_payment_preview(&form);return;}
        if form.starts_with("payroll-tax") {self.payroll_state.tab=8;self.prepare_tax_preview(&form);return;}
        if form.starts_with("payroll-thirteenth") {self.payroll_state.tab=7;self.prepare_thirteenth_preview(&form);return;}
        if form.starts_with("payroll-cutoff") {
            self.payroll_state.tab = 6;
            self.prepare_cutoff_preview(&form);
            return;
        }
        let month = NaiveDate::from_ymd_opt(2026, 9, 1).unwrap();
        let employee = self.data.employees[1].id;
        let profile = Profile {
            id: Uuid::from_u128(88001),
            company_id: self.data.company.id,
            employee_id: employee,
            effective_on: month,
            pay: Pay {
                rate_cents: 3_000_000,
                components: vec![Component {
                    label: "Rice allowance".into(),
                    kind: "rice".into(),
                    amount_cents: 250_000,
                    reference: String::new(),
                }],
                ..Default::default()
            },
            revision: 0,
            created_by: self.data.access.user_id,
        };
        self.data.payroll.profiles.push(profile.clone());
        let r = Run {
            id: Uuid::from_u128(88002),
            company_id: self.data.company.id,
            month,
            title: "September 2026 payroll".into(),
            status: "draft".into(),
            revision: 0,
            created_by: self.data.access.user_id,
            submitted_by: None,
            approved_by: None,
            paid_on: None,
            payment_reference: String::new(),
            rule_version: payroll::RULE_VERSION.into(),
        };
        self.data.payroll.runs.push(r.clone());
        let w = Worksheet {
            days: vec![Day {
                date: month,
                kind: "regular".into(),
                regular_minutes: 480,
                overtime_minutes: 120,
                night_regular: 60,
                night_overtime: 60,
                ..Default::default()
            }],
            review_note: "Fictional attendance and holiday review".into(),
            exception_note: "Synthetic documented OT review".into(),
            reviewed: true,
            ..Default::default()
        };
        let item = Item {
            id: Uuid::from_u128(88003),
            company_id: self.data.company.id,
            run_id: r.id,
            employee_id: employee,
            employee_name: self.data.employee_name(employee).into(),
            employee_code: "DEMO-002".into(),
            department: "Operations".into(),
            profile: profile.clone(),
            source_basis: Value::Null,
            totals: payroll::calculate(&profile.pay, &w, month).unwrap(),
            worksheet: w.clone(),
            revision: 0,
        };
        self.data.payroll.items.push(item.clone());
        match form.as_str() {
            "payroll-salary" => {
                self.payroll_state.profile = Some(ProfileForm {
                    id: profile.id,
                    employee: Some(employee),
                    date: month.to_string(),
                    pay: profile.pay.clone(),
                    revision: Some(0),
                    reason: String::new(),
                });
            }
            "payroll-worksheet" => {
                self.payroll_state.item = Some(ItemForm {
                    cutoff: None,
                    item: item.clone(),
                    worksheet: w,
                    reason: String::new(),
                    new_date: month.to_string(),
                });
            }
            "payroll-holidays" => self.payroll_state.tab = 2,
            "payroll-rules" => self.payroll_state.tab = 5,
            "payroll-reports" => self.payroll_state.tab = 4,
            "payroll-approval" => self.payroll_decide(&r, "run_submit"),
            "payroll-payslip" | "payroll-manager" => {
                self.data.payroll.slips.push(payroll::Slip {
                    id: item.id,
                    company_id: self.data.company.id,
                    run_id: r.id,
                    employee_id: employee,
                    month,
                    company_name: self.data.company.name.clone(),
                    item,
                    approved_by: Uuid::from_u128(88004),
                    approved_at: chrono::Utc::now(),
                    rule_version: payroll::RULE_VERSION.into(),
                });
                self.payroll_state.tab = 3;
                self.payroll_state.slip = Some(Uuid::from_u128(88003));
                if form == "payroll-manager" {
                    self.data.access.role = Role::Manager;
                    self.data.access.employee_id = Some(employee);
                    self.data.payroll.profiles.clear();
                    self.data.payroll.items.clear();
                    self.data.payroll.runs.clear();
                }
            }
            _ => self.payroll_state.selected = Some(r.id),
        }
    }
}
