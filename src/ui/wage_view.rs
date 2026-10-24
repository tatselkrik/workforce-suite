use super::payroll_view::{employee_choice, field, footer, note};
use super::*;
use crate::payroll::{PayrollAction as Change, money};
use crate::payroll_wages as w;
use serde_json::json;
#[derive(Default)]
pub(super) struct State {
    scope: Option<(Uuid, Uuid)>,
    tab: usize,
    pub(super) selected: Option<Uuid>,
    lo: String,
    hi: String,
    search: String,
    form: Option<Form>,
}
enum Form {
    Classification(ClassForm),
    Decision(Decision),
    Settings {
        enabled: bool,
        reason: String,
        revision: i64,
    },
}
struct ClassForm {
    id: Uuid,
    employee: Option<Uuid>,
    lo: String,
    hi: String,
    coverage: String,
    series: String,
    workplace: String,
    evidence: String,
    note: String,
    reason: String,
    revision: Option<i64>,
}
struct Decision {
    id: Uuid,
    revision: i64,
    operation: String,
    reason: String,
    confirmed: bool,
}
impl State {
    pub(super) fn bind(&mut self, data: &Snapshot) {
        let scope = Some((data.company.id, data.access.user_id));
        if !data.access.role.manages_hr() || self.scope != scope {
            *self = Self {
                scope,
                ..Default::default()
            };
        }
    }
    pub(super) fn editing(&self) -> bool {
        self.form.is_some()
    }
    pub(super) fn accept(&mut self, a: &Change) {
        self.form = None;
        self.selected = a.record["id"].as_str().and_then(|s| s.parse().ok());
    }
}
fn status(code: &str) -> String {
    match code {
        "meets_recorded_floor" => "Meets recorded basic floor".into(),
        "documented_exemption" => "Documented exemption reviewed".into(),
        "below_recorded_floor" => "Below recorded basic floor".into(),
        _ => code.replace('_', " "),
    }
}
fn result(ui: &mut Ui, a: &w::Assessment) {
    ui.label(if a.blocking_days > 0 {
        format!(
            "{} unresolved day(s) · reference-rate gap up to {}",
            a.blocking_days,
            money(a.max_gap_salary_cents)
        )
    } else if a.exemption_days > 0 {
        format!(
            "Recorded checks resolved · {} exemption day(s)",
            a.exemption_days
        )
    } else {
        "Meets the recorded basic floor for this period".into()
    });
    if a.max_required_salary_cents > 0 {
        ui.label(format!(
            "Highest required salary reference: {}",
            money(a.max_required_salary_cents)
        ));
    }
    let mut seen = std::collections::BTreeSet::new();
    for d in &a.days {
        if seen.insert((&d.status, &d.rate_id)) {
            ui.label(format!("{} · {}", d.date, status(&d.status)));
            if let Some(id) = &d.rate_id {
                if let Some(r) = w::catalog().rates.iter().find(|r| &r.id == id) {
                    ui.label(format!(
                        "{} · {} · daily basic {}",
                        r.region,
                        r.order,
                        money(r.daily_basic_cents)
                    ));
                }
            }
        }
    }
}
impl HrisApp {
    pub(super) fn payroll_wages(&mut self, ui: &mut Ui) {
        self.payroll_state.wages.bind(&self.data);
        if !self.data.access.role.manages_hr() {
            ui.label("HR/Admin wage reviews only.");
            return;
        }
        let today = self.data.company.today();
        if self.payroll_state.wages.lo.is_empty() {
            self.payroll_state.wages.lo = NaiveDate::from_ymd_opt(today.year(), today.month(), 1)
                .unwrap()
                .to_string();
            self.payroll_state.wages.hi = today.to_string();
        }
        ui.heading("Regional wage compliance");
        ui.label("Compare recorded basic rates with an evidenced regional wage floor. This does not change salary or certify all wage obligations.");
        ui.horizontal_wrapped(|ui| {
            for (i, label) in [
                "Employee checks",
                "Classifications",
                "Rate catalog",
                "Payroll check history",
                "Enforcement",
            ]
            .iter()
            .enumerate()
            {
                if ui
                    .selectable_label(self.payroll_state.wages.tab == i, *label)
                    .clicked()
                {
                    self.payroll_state.wages.tab = i;
                    self.payroll_state.wages.selected = None;
                }
            }
        });
        ui.add_space(10.0);
        match self.payroll_state.wages.tab {
            0 => {
                let s = &mut self.payroll_state.wages;
                ui.columns(2, |columns| {
                    field(&mut columns[0], "From (YYYY-MM-DD)", &mut s.lo);
                    field(&mut columns[1], "Through (YYYY-MM-DD)", &mut s.hi);
                });
                ui.horizontal_wrapped(|ui| {
                    ui.label("Find employee");
                    ui.text_edit_singleline(&mut s.search);
                });
                let dates =
                    s.lo.parse::<NaiveDate>()
                        .and_then(|lo| s.hi.parse::<NaiveDate>().map(|hi| (lo, hi)));
                let Ok((lo, hi)) = dates else {
                    ui.label("Enter valid dates.");
                    return;
                };
                if lo > hi || lo.year() != 2026 || hi.year() != 2026 {
                    ui.label("Use a 2026 interval in this reviewed pack.");
                    return;
                }
                let query = s.search.to_lowercase();
                for e in self.data.employees.clone().iter().filter(|e| {
                    format!("{} {}", e.employee_code, e.full_name)
                        .to_lowercase()
                        .contains(&query)
                }) {
                    let assessment = w::current(&self.data, e.id, lo, hi);
                    card(ui, |ui| {
                        ui.label(
                            RichText::new(format!("{} · {}", e.employee_code, e.full_name))
                                .strong(),
                        );
                        match &assessment {
                            Ok(a) => result(ui, a),
                            Err(error) => {
                                ui.label(error.to_string());
                            }
                        }
                    });
                }
            }
            1 => {
                if ui.button("New wage classification").clicked() {
                    self.payroll_state.wages.form = Some(Form::Classification(ClassForm {
                        id: Uuid::new_v4(),
                        employee: None,
                        lo: NaiveDate::from_ymd_opt(2026, today.month(), 1)
                            .unwrap()
                            .to_string(),
                        hi: "2026-12-31".into(),
                        coverage: "covered".into(),
                        series: String::new(),
                        workplace: String::new(),
                        evidence: String::new(),
                        note: String::new(),
                        reason: String::new(),
                        revision: None,
                    }));
                }
                ui.label("A different Administrator approves each effective interval. Approved facts remain immutable; retire and replace superseded classifications.");
                for c in self
                    .data
                    .payroll
                    .wages
                    .classifications
                    .clone()
                    .iter()
                    .filter(|c| c.company_id == self.data.company.id)
                {
                    card(ui, |ui| {
                        if ui
                            .button(format!(
                                "{} · {} · {}",
                                self.data.employee_name(c.employee_id),
                                c.starts_on,
                                c.status
                            ))
                            .clicked()
                        {
                            self.payroll_state.wages.selected = Some(c.id);
                        }
                        ui.label(format!(
                            "Through {} · {} · {}",
                            c.ends_on, c.workplace, c.coverage
                        ));
                        if self.payroll_state.wages.selected == Some(c.id) {
                            if let Some(rate) = w::series()
                                .into_iter()
                                .find(|r| r.series_key == c.series_key)
                            {
                                ui.label(format!(
                                    "{} · {} · {}",
                                    rate.region, rate.label, rate.area
                                ));
                            }
                            ui.label(format!("Classification evidence: {}", c.evidence));
                            ui.label(format!("Coverage review: {}", c.note));
                            ui.horizontal_wrapped(|ui| {
                                if c.status == "draft" && ui.button("Edit review").clicked() {
                                    self.payroll_state.wages.form =
                                        Some(Form::Classification(ClassForm {
                                            id: c.id,
                                            employee: Some(c.employee_id),
                                            lo: c.starts_on.to_string(),
                                            hi: c.ends_on.to_string(),
                                            coverage: c.coverage.clone(),
                                            series: c.series_key.clone(),
                                            workplace: c.workplace.clone(),
                                            evidence: c.evidence.clone(),
                                            note: c.note.clone(),
                                            reason: String::new(),
                                            revision: Some(c.revision),
                                        }));
                                }
                                let approve = self.data.access.role == Role::Administrator
                                    && ![c.created_by, c.prepared_by]
                                        .contains(&self.data.access.user_id);
                                if c.status == "draft"
                                    && ui
                                        .add_enabled(
                                            approve,
                                            egui::Button::new("Approve classification"),
                                        )
                                        .clicked()
                                {
                                    self.payroll_state.wages.form =
                                        Some(Form::Decision(Decision {
                                            id: c.id,
                                            revision: c.revision,
                                            operation: "approve".into(),
                                            reason: String::new(),
                                            confirmed: false,
                                        }));
                                }
                                if c.status == "draft" && ui.button("Cancel draft").clicked() {
                                    self.payroll_state.wages.form =
                                        Some(Form::Decision(Decision {
                                            id: c.id,
                                            revision: c.revision,
                                            operation: "cancel".into(),
                                            reason: String::new(),
                                            confirmed: false,
                                        }));
                                }
                                if c.status == "approved"
                                    && self.data.access.role == Role::Administrator
                                    && ui.button("Retire classification").clicked()
                                {
                                    self.payroll_state.wages.form =
                                        Some(Form::Decision(Decision {
                                            id: c.id,
                                            revision: c.revision,
                                            operation: "retire".into(),
                                            reason: String::new(),
                                            confirmed: false,
                                        }));
                                }
                            });
                        }
                    });
                }
            }
            2 => {
                ui.label(format!(
                    "{} · verified {} · supported through {}",
                    w::VERSION,
                    w::catalog().verified_on,
                    w::catalog().supported_until
                ));
                for note in &w::catalog().regional_notes {
                    ui.label(note);
                }
                let mut regions: Vec<_> = w::catalog()
                    .rates
                    .iter()
                    .map(|r| r.region.as_str())
                    .collect();
                regions.sort_unstable();
                regions.dedup();
                for region in regions {
                    ui.collapsing(format!("Region {region}"), |ui| {
                        for r in w::catalog().rates.iter().filter(|r| r.region == region) {
                            card(ui, |ui| {
                                ui.label(RichText::new(&r.label).strong());
                                ui.label(&r.area);
                                ui.label(format!(
                                    "{} to {} · {} · daily basic {}",
                                    r.starts_on,
                                    r.ends_on,
                                    r.order,
                                    money(r.daily_basic_cents)
                                ));
                                if !r.note.is_empty() {
                                    ui.label(&r.note);
                                }
                                ui.horizontal_wrapped(|ui| {
                                    ui.hyperlink_to("Official regional page", &r.source_url);
                                    ui.hyperlink_to("Wage order", &r.order_url);
                                });
                            });
                        }
                    });
                }
            }
            3 => {
                ui.label("Submission and approval snapshots retain the recorded result. Later changes never rewrite an earlier check.");
                for c in self
                    .data
                    .payroll
                    .wages
                    .checks
                    .clone()
                    .iter()
                    .filter(|c| c.company_id == self.data.company.id)
                {
                    card(ui, |ui| {
                        ui.label(
                            RichText::new(format!(
                                "{} · {} · revision{}",
                                c.kind, c.stage, c.run_revision
                            ))
                            .strong(),
                        );
                        ui.label(format!(
                            "{} · enforcement {}",
                            c.recorded_at.format("%Y-%m-%d %H:%M UTC"),
                            if c.enforced { "on" } else { "off" }
                        ));
                        for row in c.assessment["employees"].as_array().into_iter().flatten() {
                            ui.label(format!(
                                "{} · {} · {} unresolved day(s)",
                                row["employee_code"].as_str().unwrap_or(""),
                                row["employee_name"].as_str().unwrap_or(""),
                                row["result"]["blocking_days"].as_u64().unwrap_or(0)
                            ));
                        }
                    });
                }
            }
            _ => {
                let settings = self
                    .data
                    .payroll
                    .wages
                    .settings
                    .iter()
                    .find(|s| s.id == self.data.company.id);
                let enforced = settings.is_some_and(|s| s.enforced);
                let revision = settings.map_or(0, |s| s.revision);
                ui.heading(if enforced {
                    "Recorded wage checks are enforced"
                } else {
                    "Recorded wage checks are advisory"
                });
                ui.label("When enabled, all included monthly/cutoff recipients need resolved wage checks before submission and approval. Older desktop clients follow the same server guard. Changed facts require return and resubmission.");
                ui.label("Complete classification reviews first. A basic-rate flag is not an arrears calculation; it does not account for every paid-day, proration or contractual obligation.");
                if self.data.access.role == Role::Administrator
                    && ui
                        .button(if enforced {
                            "Change enforcement"
                        } else {
                            "Enable enforcement"
                        })
                        .clicked()
                {
                    self.payroll_state.wages.form = Some(Form::Settings {
                        enabled: enforced,
                        reason: String::new(),
                        revision,
                    });
                }
            }
        }
    }
    pub(super) fn payroll_wage_forms(&mut self, ui: &mut Ui) {
        self.payroll_state.wages.bind(&self.data);
        let Some(mut form) = self.payroll_state.wages.form.take() else {
            return;
        };
        let mut keep = true;
        match &mut form {
            Form::Classification(f) => {
                ui.heading(if f.revision.is_some() {
                    "Edit wage classification"
                } else {
                    "New wage classification"
                });
                if f.revision.is_none() {
                    employee_choice(ui, &self.data, "wage_employee", &mut f.employee);
                } else {
                    ui.label(self.data.employee_name(f.employee.unwrap()));
                }
                field(ui, "Effective from (YYYY-MM-DD)", &mut f.lo);
                field(ui, "Effective through (YYYY-MM-DD)", &mut f.hi);
                super::payroll_view::choice(
                    ui,
                    "wage_coverage",
                    "Coverage",
                    &mut f.coverage,
                    &["covered", "exempt"],
                );
                if f.coverage == "covered" {
                    egui::ComboBox::from_id_salt("wage_series")
                        .width(ui.available_width().min(660.0))
                        .selected_text(
                            w::series()
                                .into_iter()
                                .find(|r| r.series_key == f.series)
                                .map(|r| format!("{} · {}", r.region, r.label))
                                .unwrap_or_else(|| {
                                    "Choose the actual area / sector / establishment category"
                                        .into()
                                }),
                        )
                        .show_ui(ui, |ui| {
                            for r in w::series() {
                                ui.selectable_value(
                                    &mut f.series,
                                    r.series_key.clone(),
                                    format!("{} · {} · {}", r.region, r.label, r.area),
                                );
                            }
                        });
                    if let Some(r) = w::series().into_iter().find(|r| r.series_key == f.series) {
                        ui.label(&r.area);
                        ui.hyperlink_to("Verify official regional schedule", &r.source_url);
                    }
                } else {
                    f.series.clear();
                    ui.label("An exemption requires specific legal evidence and independent approval. Unknown or nonstandard coverage is not an exemption.");
                }
                field(ui, "Actual work area / establishment", &mut f.workplace);
                note(
                    ui,
                    "Sector, establishment size and applicability evidence (HR private)",
                    &mut f.evidence,
                );
                note(
                    ui,
                    "Coverage rationale and current-source review (HR private)",
                    &mut f.note,
                );
                ui.label("Do not infer establishment headcount from accounts in this app. Archived employees retain their owed-pay records. This review does not change salary or minimum-wage tax status.");
                note(ui, "Reason for this change (HR private)", &mut f.reason);
                let mut record = json!({"id":f.id,"employee_id":f.employee,"starts_on":f.lo,"ends_on":f.hi,"coverage":f.coverage,"series_key":f.series,"workplace":f.workplace,"evidence":f.evidence,"note":f.note,"reason":f.reason,"catalog_version":w::VERSION});
                let op = if let Some(revision) = f.revision {
                    record.as_object_mut().unwrap().remove("employee_id");
                    record["expected_revision"] = json!(revision);
                    "wage_save"
                } else {
                    "wage_create"
                };
                let a = Change::new(op, record);
                let (save, cancel) = footer(ui, &a, &self.data, "Save classification review");
                if save {
                    self.send_payroll(a);
                }
                keep = !cancel;
            }
            Form::Decision(f) => {
                let record = self
                    .data
                    .payroll
                    .wages
                    .classifications
                    .iter()
                    .find(|c| c.id == f.id && c.company_id == self.data.company.id)
                    .cloned();
                let Some(c) = record else {
                    ui.label("Wage review unavailable. Refresh.");
                    return;
                };
                ui.heading(format!("{} wage classification", match f.operation.as_str() { "approve" => "Approve", "retire" => "Retire", _ => "Cancel" }));
                ui.label(format!(
                    "{} · {} through {}",
                    self.data.employee_name(c.employee_id),
                    c.starts_on,
                    c.ends_on
                ));
                ui.label(&c.workplace);
                ui.label(&c.evidence);
                ui.label(&c.note);
                ui.checkbox(&mut f.confirmed,if f.operation=="approve"{"I reviewed the actual workplace, category, evidence and current official sources."}else{"I confirm this reasoned history-preserving action."});
                note(ui, "Reason (HR private)", &mut f.reason);
                let a = Change::new(
                    &format!("wage_{}", f.operation),
                    json!({"id":f.id,"expected_revision":f.revision,"reason":f.reason,"catalog_version":w::VERSION}),
                );
                ui.horizontal_wrapped(|ui| {
                    if ui
                        .add_enabled(
                            f.confirmed && a.payload(&self.data).is_ok() && !self.busy,
                            egui::Button::new("Confirm"),
                        )
                        .clicked()
                    {
                        self.send_payroll(a);
                    }
                    if ui.button("Back without recording").clicked() {
                        keep = false;
                    }
                });
            }
            Form::Settings {
                enabled,
                reason,
                revision,
            } => {
                ui.heading("Wage-check enforcement");
                ui.checkbox(
                    enabled,
                    "Require resolved wage checks on every monthly/cutoff issue",
                );
                ui.label("Existing published payslips remain unchanged. This decision is recorded with your reason. It does not replace review of current official rules or all actual-pay obligations.");
                note(ui, "Reason (HR private)", reason);
                let a = Change::new(
                    "wage_settings",
                    json!({"id":self.data.company.id,"expected_revision":revision,"enforced":enabled,"reason":reason,"catalog_version":w::VERSION}),
                );
                let (save, cancel) = footer(ui, &a, &self.data, "Save enforcement");
                if save {
                    self.send_payroll(a);
                }
                keep = !cancel;
            }
        }
        if keep {
            self.payroll_state.wages.form = Some(form);
        }
    }
}

#[cfg(feature = "qa-screenshot")]
impl HrisApp {
    pub(super) fn prepare_wage_preview(&mut self, form: &str) {
        use crate::domain::{Action, DemoStore};
        use crate::payroll::{Pay, Profile};
        let mut d = DemoStore::new();
        let admin = d.data.access.user_id;
        let hr = Uuid::from_u128(390001);
        let cid = d.data.company.id;
        let date = NaiveDate::from_ymd_opt(2026, 10, 1).unwrap();
        let employees: Vec<_> = d.data.employees.iter().take(2).map(|e| e.id).collect();
        for (index, eid) in employees.iter().enumerate() {
            d.data.payroll.profiles.push(Profile {
                id: Uuid::from_u128(390010 + index as u128),
                company_id: cid,
                employee_id: *eid,
                effective_on: NaiveDate::from_ymd_opt(2026, 2, 1).unwrap(),
                pay: Pay {
                    rate_cents: if index == 0 { 1_500_000 } else { 3_000_000 },
                    ..Default::default()
                },
                revision: 0,
                created_by: hr,
            });
            d.data.access.user_id = hr;
            d.data.access.role = Role::HrGeneralist;
            let id = Uuid::from_u128(390020 + index as u128);
            d.execute(Action::Payroll(Change::new("wage_create",json!({"id":id,"employee_id":eid,"starts_on":"2026-01-01","ends_on":"2026-12-31","coverage":"covered","series_key":"vi-nonagri-large","workplace":"Fictional Bacolod establishment","evidence":"Fictional nonagricultural workplace with more than 10 workers; source reviewed 24 October 2026.","note":"Fictional classification for preview only.","reason":"Fictional independent wage review","catalog_version":w::VERSION})))).unwrap();
            if !form.ends_with("approve") || index == 1 {
                d.data.access.user_id = admin;
                d.data.access.role = Role::Administrator;
                d.execute(Action::Payroll(Change::new("wage_approve",json!({"id":id,"expected_revision":0,"reason":"Fictional independent source review","catalog_version":w::VERSION})))).unwrap();
            }
        }
        d.data.access.user_id = admin;
        d.data.access.role = Role::Administrator;
        self.data = d.data;
        self.payroll_state.tab = 11;
        self.payroll_state.wages = State::default();
        self.payroll_state.wages.bind(&self.data);
        let s = &mut self.payroll_state.wages;
        s.lo = date.to_string();
        s.hi = "2026-10-31".into();
        if form.ends_with("classifications") {
            s.tab = 1;
            s.selected = Some(Uuid::from_u128(390020));
        }
        if form.ends_with("catalog") {
            s.tab = 2;
        }
        if form.ends_with("history") {
            s.tab = 3;
        }
        if form.ends_with("history") {
            let rows:Vec<_>=employees.iter().map(|eid|json!({"employee_id":eid,"employee_code":self.data.employees.iter().find(|e|e.id==*eid).unwrap().employee_code,"employee_name":self.data.employee_name(*eid),"result":w::current(&self.data,*eid,date,"2026-10-31".parse().unwrap()).unwrap()})).collect();
            self.data.payroll.wages.checks.push(w::Check{id:Uuid::new_v4(),company_id:cid,run_id:Uuid::new_v4(),kind:"cutoff".into(),stage:"submitted".into(),run_revision:3,enforced:false,assessment:json!({"version":w::VERSION,"enforced":false,"settings_revision":0,"employees":rows}),recorded_at:chrono::Utc::now()});
        }

        if form.ends_with("enforcement") {
            s.tab = 4;
        }
        if form.ends_with("settings") {
            s.tab = 4;
            s.form = Some(Form::Settings {
                enabled: false,
                reason: "Fictional explicit enforcement decision".into(),
                revision: 0,
            });
        }
        if form.ends_with("new") {
            s.form = Some(Form::Classification(ClassForm {
                id: Uuid::new_v4(),
                employee: Some(employees[0]),
                lo: "2026-10-01".into(),
                hi: "2026-12-31".into(),
                coverage: "covered".into(),
                series: "vi-nonagri-large".into(),
                workplace: "Fictional Bacolod establishment".into(),
                evidence: "Fictional actual industry and establishment headcount evidence".into(),
                note: "Fictional current-source review".into(),
                reason: "Fictional draft wage review".into(),
                revision: None,
            }));
        }
        if form.ends_with("approve") {
            s.tab = 1;
            s.form = Some(Form::Decision(Decision {
                id: Uuid::from_u128(390020),
                revision: 0,
                operation: "approve".into(),
                reason: "Fictional independent review".into(),
                confirmed: false,
            }));
        }
        if form.ends_with("manager") {
            self.data.access.role = Role::Manager;
            self.payroll_state.tab = 3;
        }
    }
}
