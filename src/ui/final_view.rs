use super::payroll_view::{amount, choice, field, footer, note};
use super::*;
use crate::{
    payroll::{self, PayrollAction as Change},
    payroll_final as final_pay, payroll_tax as tax,
};
use serde_json::json;
#[derive(Default)]
pub(super) struct State {
    selected: Option<Uuid>,
    slips: bool,
    slip: Option<Uuid>,
    search: String,
    form: Option<Form>,
}
enum Form {
    New(NewForm),
    Worksheet(Box<WorksheetForm>),
    Decision(Decision),
}
struct NewForm {
    id: Uuid,
    employee: Option<Uuid>,
    end: String,
    cash: String,
    title: String,
    reason: String,
}
struct WorksheetForm {
    record: final_pay::Record,
    w: final_pay::Worksheet,
    birth: String,
    openings: Vec<Opening>,
    previous: Vec<Previous>,
    reason: String,
}
struct Opening {
    start: String,
    end: String,
    row: tax::Opening,
}
struct Previous {
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
    handling_confirmed: bool,
    reference: String,
    handling: String,
}
impl State {
    pub(super) fn editing(&self) -> bool {
        self.form.is_some()
    }
    pub(super) fn accept(&mut self, a: &Change, data: &Snapshot) {
        let id = a.record["id"].as_str().and_then(|s| s.parse().ok());
        if a.operation == "final_refresh" {
            if let Some(Form::Worksheet(f)) = self.form.as_mut() {
                if let Some(r) = data
                    .payroll
                    .finals
                    .records
                    .iter()
                    .find(|r| Some(r.id) == id)
                {
                    f.record = r.clone();
                    final_pay::reset_reviews(&mut f.w);
                    if let Ok(s) = final_pay::source(r) {
                        f.w.vacation.previous_days_hundredths =
                            f.w.vacation
                                .previous_days_hundredths
                                .max(s.previous_vacation_days_hundredths);
                    }
                    f.reason.clear();
                    return;
                }
            }
        }
        if a.operation == "final_create" {
            self.selected = id;
        }
        self.form = None;
    }
}
fn worksheet(r: final_pay::Record) -> WorksheetForm {
    WorksheetForm {
        w: r.worksheet.clone(),
        birth: r
            .worksheet
            .retirement
            .birth_date
            .map(|d| d.to_string())
            .unwrap_or_default(),
        openings: r
            .worksheet
            .tax
            .openings
            .iter()
            .map(|r| Opening {
                start: r.starts_on.to_string(),
                end: r.ends_on.to_string(),
                row: r.clone(),
            })
            .collect(),
        previous: r
            .worksheet
            .tax
            .previous_employers
            .iter()
            .map(|r| Previous {
                start: r.starts_on.to_string(),
                end: r.ends_on.to_string(),
                row: r.clone(),
            })
            .collect(),
        record: r,
        reason: String::new(),
    }
}
fn numbers(ui: &mut Ui, c: &final_pay::Calculation) {
    for (label, n) in final_pay::totals_rows(c) {
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
    if let Some(d) = c.due_on {
        ui.label(format!(
            "Release due by {d} · {} credited service years",
            c.credited_service_years
        ));
    }
    if c.planned_late {
        ui.colored_label(
            Color32::DARK_RED,
            "The planned release exceeds the reviewed separation-based deadline.",
        );
    }
    if c.tax.employer_advance_cents > 0 {
        ui.label("The tax deficiency beyond final cash needs separate employer handling. It is not an automatic employee debt.");
    }
    for gap in &c.tax.coverage_gaps {
        ui.colored_label(
            Color32::DARK_RED,
            format!(
                "Annual income coverage needs evidence: {} to {}",
                gap.starts_on, gap.ends_on
            ),
        );
    }
}
fn units(ui: &mut Ui, label: &str, n: &mut u32) {
    ui.horizontal_wrapped(|ui| {
        ui.label(label);
        ui.add(
            egui::DragValue::new(n)
                .range(0..=36600)
                .speed(25.0)
                .custom_formatter(|v, _| format!("{:.2}", v / 100.0))
                .custom_parser(|s| {
                    let mut p = s.trim().split('.');
                    let whole = p.next()?.parse::<u32>().ok()?;
                    let frac = p.next().unwrap_or("");
                    if p.next().is_some()
                        || frac.len() > 2
                        || !frac.bytes().all(|b| b.is_ascii_digit())
                    {
                        return None;
                    }
                    let minor = if frac.is_empty() {
                        0
                    } else {
                        frac.parse::<u32>().ok()? * if frac.len() == 1 { 10 } else { 1 }
                    };
                    Some(whole.checked_mul(100)?.checked_add(minor)? as f64)
                }),
        );
    });
}
impl HrisApp {
    pub(super) fn payroll_final(&mut self, ui: &mut Ui) {
        let hr = self.data.access.role.manages_hr();
        if !hr {
            self.payroll_state.finals.slips = true;
            self.payroll_state.finals.form = None;
        }
        ui.horizontal_wrapped(|ui| {
            if hr {
                ui.selectable_value(
                    &mut self.payroll_state.finals.slips,
                    false,
                    "Final-pay reviews",
                );
            }
            ui.selectable_value(
                &mut self.payroll_state.finals.slips,
                true,
                if hr {
                    "Issued statements"
                } else {
                    "My final statements"
                },
            );
        });
        ui.label("Final pay combines outstanding issued payslips, reviewed entitlements and a prospective annual tax adjustment. Actual settlement is recorded separately.");
        ui.add_space(8.0);
        if self.payroll_state.finals.slips {
            self.final_slips(ui);
            return;
        }
        if let Some(r) = self
            .data
            .payroll
            .finals
            .records
            .iter()
            .find(|r| {
                Some(r.id) == self.payroll_state.finals.selected
                    && r.company_id == self.data.company.id
            })
            .cloned()
        {
            if ui.button("Back to final-pay reviews").clicked() {
                self.payroll_state.finals.selected = None;
                return;
            }
            card(ui, |ui| {
                ui.heading(&r.title);
                ui.label(format!(
                    "{} · {} · {}",
                    r.employee_code, r.employee_name, r.status
                ));
                ui.label(format!(
                    "Last working day {} · Planned cash {}",
                    r.service_end, r.cash_date
                ));
                numbers(ui, &r.totals);
                ui.horizontal_wrapped(|ui| {
                    if ui
                        .button(if r.status == "draft" {
                            "Edit worksheet"
                        } else {
                            "Review saved worksheet & evidence"
                        })
                        .clicked()
                    {
                        self.payroll_state.finals.form =
                            Some(Form::Worksheet(Box::new(worksheet(r.clone()))));
                    }
                    match r.status.as_str() {
                        "draft" => {
                            if ui.button("Submit for independent approval").clicked() {
                                self.final_decide(&r, "submit");
                            }
                            if ui.button("Cancel draft").clicked() {
                                self.final_decide(&r, "cancel");
                            }
                        }
                        "submitted" => {
                            if self.data.access.role == Role::Administrator
                                && ui.button("Approve final statement").clicked()
                            {
                                self.final_decide(&r, "approve");
                            }
                            if ui.button("Return for review").clicked() {
                                self.final_decide(&r, "return");
                            }
                        }
                        "approved" => {
                            if self.data.access.role == Role::Administrator {
                                if ui.button("Record actual final settlement").clicked() {
                                    self.final_decide(&r, "settle");
                                }
                                if ui.button("Withdraw unpaid statement").clicked() {
                                    self.final_decide(&r, "withdraw");
                                }
                            }
                        }
                        _ => {}
                    }
                    if ui.button("Export review row").clicked() {
                        self.payroll_export("final_register", Some(r.id));
                    }
                });
                if r.status == "settled" {
                    ui.label(format!("Recorded actual cash {}", r.settled_on.unwrap()));
                    ui.label(format!("HR payment reference: {}", r.payment_reference));
                    if !r.handling_reference.is_empty() {
                        ui.label(format!("HR tax handling: {}", r.handling_reference));
                    }
                }
                if r.status == "withdrawn" {
                    ui.label("This unpaid statement was withdrawn. Its issued copy is retained; prepare a new case with fresh facts.");
                }
            });
            self.payroll_history(ui, r.id);
            return;
        }
        ui.horizontal_wrapped(|ui| {
            if ui.button("New final-pay review").clicked() {
                let date = self.data.company.today().to_string();
                self.payroll_state.finals.form = Some(Form::New(NewForm {
                    id: Uuid::new_v4(),
                    employee: None,
                    end: date.clone(),
                    cash: date,
                    title: "Final pay".into(),
                    reason: String::new(),
                }));
            }
            ui.label("Find employee");
            ui.text_edit_singleline(&mut self.payroll_state.finals.search);
        });
        let q = self.payroll_state.finals.search.to_lowercase();
        let records = self
            .data
            .payroll
            .finals
            .records
            .iter()
            .filter(|r| {
                r.company_id == self.data.company.id
                    && format!("{} {} {}", r.employee_code, r.employee_name, r.title)
                        .to_lowercase()
                        .contains(&q)
            })
            .cloned()
            .collect::<Vec<_>>();
        if records.is_empty() {
            ui.label("No matching final-pay reviews.");
        }
        for r in records.iter().rev() {
            card(ui, |ui| {
                if ui
                    .link(format!(
                        "{} · {} · {}",
                        r.employee_code, r.employee_name, r.title
                    ))
                    .clicked()
                {
                    self.payroll_state.finals.selected = Some(r.id);
                }
                ui.label(format!(
                    "{} · Last working day {} · Planned cash {}",
                    r.status, r.service_end, r.cash_date
                ));
                ui.strong(format!(
                    "Combined final cash {}",
                    payroll::money(r.totals.net_cents)
                ));
            });
        }
        if !records.is_empty() && ui.button("Export final-pay register").clicked() {
            self.payroll_export("final_register", None);
        }
    }
    fn final_decide(&mut self, r: &final_pay::Record, op: &str) {
        self.payroll_state.finals.form = Some(Form::Decision(Decision {
            id: r.id,
            revision: r.revision,
            op: op.into(),
            reason: String::new(),
            confirmed: false,
            handling_confirmed: false,
            reference: String::new(),
            handling: String::new(),
        }));
    }
    pub(super) fn payroll_final_forms(&mut self, ui: &mut Ui) {
        if !self.data.access.role.manages_hr() {
            self.payroll_state.finals.form = None;
            return;
        }
        let Some(mut form) = self.payroll_state.finals.form.take() else {
            return;
        };
        let keep;
        match &mut form {
            Form::New(f) => {
                ui.heading("New final-pay review");
                egui::ComboBox::from_id_salt("final_employee")
                    .selected_text(
                        f.employee
                            .map(|e| self.data.employee_name(e))
                            .unwrap_or("Choose employee"),
                    )
                    .show_ui(ui, |ui| {
                        for e in &self.data.employees {
                            ui.selectable_value(
                                &mut f.employee,
                                Some(e.id),
                                format!(
                                    "{} · {}{}",
                                    e.employee_code,
                                    e.full_name,
                                    if e.status == EmployeeStatus::Active {
                                        ""
                                    } else {
                                        " · Archived"
                                    }
                                ),
                            );
                        }
                    });
                field(ui, "Last working day (YYYY-MM-DD)", &mut f.end);
                field(ui, "Planned actual cash date (YYYY-MM-DD)", &mut f.cash);
                field(ui, "Title", &mut f.title);
                note(
                    ui,
                    "Reason / separation context (HR private)",
                    &mut f.reason,
                );
                ui.label("Prepare the last salary/cutoff and earned 13th-month slips first. All outstanding issued slips through this cash date are included. Archiving, account access and clearance do not gate owed pay.");
                ui.label("This reviewed pack supports same-year 2026 separation and cash dates. Cross-year settlements need a separate tax-year review.");
                let a = Change::new(
                    "final_create",
                    json!({"id":f.id,"employee_id":f.employee,"service_end":f.end,"cash_date":f.cash,"title":f.title,"reason":f.reason}),
                );
                let (save, cancel) = footer(ui, &a, &self.data, "Create final-pay review");
                if save {
                    self.send_payroll(a);
                }
                keep = !cancel;
            }
            Form::Worksheet(f) => {
                keep = self.final_worksheet(ui, f);
            }
            Form::Decision(f) => {
                let record = self
                    .data
                    .payroll
                    .finals
                    .records
                    .iter()
                    .find(|r| r.id == f.id && r.company_id == self.data.company.id)
                    .cloned();
                let Some(r) = record else {
                    ui.label("Refresh this final-pay case.");
                    return;
                };
                ui.heading(match f.op.as_str() {
                    "settle" => "Record actual final settlement",
                    "withdraw" => "Withdraw unpaid final statement",
                    "approve" => "Approve final statement",
                    "submit" => "Submit final-pay review",
                    _ => "Final-pay decision",
                });
                ui.label(format!("{} · {}", r.employee_code, r.employee_name));
                numbers(ui, &r.totals);
                let mut valid = true;
                if ["submit", "approve"].contains(&f.op.as_str()) {
                    if let Err(e) =
                        final_pay::source(&r).and_then(|s| final_pay::ready(&s, &r.worksheet))
                    {
                        ui.colored_label(Color32::DARK_RED, e.to_string());
                        valid = false;
                    }
                }
                if f.op == "approve"
                    && [Some(r.created_by), Some(r.prepared_by), r.submitted_by]
                        .contains(&Some(self.data.access.user_id))
                {
                    ui.colored_label(Color32::DARK_RED,"A different Administrator must approve. The creator, worksheet preparer and submitter cannot approve this case.");
                    valid = false;
                }
                if f.op == "settle" {
                    ui.label(format!("Approved cash date: {}", r.cash_date));
                    ui.label("Record only actual combined final cash and completed tax/refund/employer handling. Source net balances are settled within this amount; they are not another cash payment.");
                    note(
                        ui,
                        "Actual payment reference (HR private)",
                        &mut f.reference,
                    );
                    note(
                        ui,
                        "Employer tax handling reference (required for unfunded tax)",
                        &mut f.handling,
                    );
                    ui.checkbox(
                        &mut f.confirmed,
                        "I verified the actual combined final cash was received, including zero.",
                    );
                    ui.checkbox(&mut f.handling_confirmed,"I verified all final tax/refund/employer handling was completed, including zero.");
                    if r.cash_date != self.data.company.today() {
                        ui.label("The recorded date must match the approved cash date and follow approval. Withdraw an unpaid statement and prepare a fresh date if payment was rescheduled.");
                    }
                } else {
                    ui.checkbox(
                        &mut f.confirmed,
                        if f.op == "withdraw" {
                            "I confirm no final cash or employer tax settlement has occurred."
                        } else {
                            "I reviewed this action and the saved evidence."
                        },
                    );
                }
                note(ui, "Reason (HR private)", &mut f.reason);
                let mut v = json!({"id":f.id,"expected_revision":f.revision,"reason":f.reason});
                if f.op == "settle" {
                    v["settled_on"] = json!(r.cash_date);
                    v["payment_reference"] = json!(f.reference);
                    v["handling_reference"] = json!(f.handling);
                    v["cash_received_confirmed"] = json!(f.confirmed);
                    v["employer_handling_confirmed"] = json!(f.handling_confirmed);
                }
                if f.op == "withdraw" {
                    v["unpaid_confirmed"] = json!(f.confirmed);
                }
                let a = Change::new(&format!("final_{}", f.op), v);
                let (mut save, mut cancel) = (false, false);
                let enabled = valid && f.confirmed && (f.op != "settle" || f.handling_confirmed);
                let payload = a.payload(&self.data);
                if let Err(e) = &payload {
                    ui.label(RichText::new(e.to_string()).color(muted(ui)));
                }
                ui.horizontal_wrapped(|ui| {
                    save = ui
                        .add_enabled(enabled && payload.is_ok(), egui::Button::new("Confirm"))
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
            self.payroll_state.finals.form = Some(form);
        }
    }
    fn final_worksheet(&mut self, ui: &mut Ui, f: &mut WorksheetForm) -> bool {
        ui.heading(format!("Final pay · {}", f.record.employee_name));
        let editable = f.record.status == "draft";
        ui.label(format!(
            "{} · Last working day {} · Cash date {}",
            f.record.status, f.record.service_end, f.record.cash_date
        ));
        let src = match final_pay::source(&f.record) {
            Ok(s) => s,
            Err(e) => {
                ui.label(e.to_string());
                return !ui.button("Back").clicked();
            }
        };
        if f.record.service_end.year() != f.record.cash_date.year() {
            card(ui, |ui| {
                ui.strong(format!("Delayed final pay · {} payment year", f.record.cash_date.year()));
                ui.label("Employment ended in an earlier year. Only income actually or constructively received in this payment year belongs in this tax review. Earlier-year income and withholding stay in their original year. No current-year service days are invented.");
                ui.add_enabled_ui(editable, |ui| {
                    ui.checkbox(&mut f.w.cash_year_reviewed, "Receipt year, earlier certificates, and other employment reviewed");
                    note(ui, "Actual/constructive receipt and cash-year evidence (HR private)", &mut f.w.cash_year_note);
                });
            });
        }
        card(ui, |ui| {
            ui.strong("Outstanding issued pay");
            ui.label(format!(
                "{} owed payslips · {} previously received income sources",
                src.outstanding.len(),
                src.annual.income.len()
            ));
            for p in &src.outstanding {
                ui.label(format!(
                    "{} · net {}",
                    p.income
                        .starts_on
                        .zip(p.income.ends_on)
                        .map(|(a, b)| format!("{a} to {b}"))
                        .unwrap_or("13th-month / supplementary benefit".into()),
                    payroll::money(p.net_cents)
                ));
            }
            ui.label("Source nets are included once. The combined cash below also accounts for reviewed benefits and final tax.");
            if editable
                && ui
                    .button("Refresh sources and require new reviews")
                    .clicked()
            {
                self.send_payroll(Change::new("final_refresh",json!({"id":f.record.id,"expected_revision":f.record.revision,"reason":"Refresh final income, payment and employment facts"})));
            }
        });
        egui::CollapsingHeader::new("Leave conversion").default_open(true).show(ui,|ui|{ui.add_enabled_ui(editable,|ui|{units(ui,"Unused vacation days payable",&mut f.w.vacation.days_hundredths);amount(ui,"Reviewed vacation daily rate",&mut f.w.vacation.daily_cents);units(ui,"Vacation days already monetized this calendar year",&mut f.w.vacation.previous_days_hundredths);note(ui,"Leave balance, rate and prior conversion evidence (HR private)",&mut f.w.vacation.reference);ui.label("Private vacation de minimis: up to 12 days in the cash year. Excess shares the annual PHP 90,000 benefit cap. SIL, sick leave and other balances are not automatically vacation credits; review other owed conversion below.");});});
        egui::CollapsingHeader::new("Separation pay").show(ui,|ui|{ui.add_enabled_ui(editable,|ui|{let p=&mut f.w.separation;choice(ui,"final_cause","Reviewed cause",&mut p.cause,&["none","resignation","just_cause","retrenchment","closure_without_serious_losses","closure_serious_losses","redundancy","labor_saving_devices","disease","death_or_disability","company_policy"]);amount(ui,"Latest monthly basic",&mut p.monthly_basic_cents);amount(ui,"Regular monthly allowances in separation base",&mut p.regular_allowances_cents);amount(ui,"Total cumulative separation award",&mut p.total_award_cents);amount(ui,"Separation award already paid",&mut p.previously_paid_cents);ui.checkbox(&mut p.tax_exempt,"Reviewed qualifying involuntary/death/disability tax exemption");note(ui,"Cause, pay base, award and tax evidence (HR private)",&mut p.reference);ui.label("Half-month causes have a one-month minimum. Redundancy/labor-saving use one month per credited year. Six-month service rounding determines the amount; an account role does not determine entitlement.");});});
        egui::CollapsingHeader::new("Retirement pay").show(ui,|ui|{ui.add_enabled_ui(editable,|ui|{let r=&mut f.w.retirement;choice(ui,"final_retirement","Method",&mut r.method,&["none","statutory","plan"]);field(ui,"Evidenced birth date (YYYY-MM-DD)",&mut f.birth);ui.checkbox(&mut r.ordinary_statutory_coverage,"Reviewed coverage under ordinary private-sector statutory retirement");amount(ui,"Latest daily retirement basic",&mut r.daily_basic_cents);amount(ui,"Total cumulative plan / policy award",&mut r.plan_award_cents);amount(ui,"Retirement award already paid",&mut r.previously_paid_cents);ui.checkbox(&mut r.employee_plan_covered,"This employee is covered by an employer retirement plan");choice(ui,"final_retirement_tax","Reviewed tax treatment",&mut r.tax_treatment,&["taxable","statutory_no_plan","qualified_plan"]);ui.checkbox(&mut r.qualified_plan_reviewed,"BIR plan qualification and employee eligibility verified");ui.checkbox(&mut r.first_qualified_availment,"First qualifying retirement-tax availment verified");note(ui,"Service, coverage, plan and tax evidence (HR private)",&mut r.reference);ui.label("Ordinary statutory minimum: actual 5 years of service, age 60+,22.5 daily units per credited year. An employee covered but ineligible under a plan cannot automatically use the no-plan tax exemption.");});});
        egui::CollapsingHeader::new("Other owed amounts & lawful deductions").show(ui,|ui|{ui.add_enabled_ui(editable,|ui|{ui.label("Labels appear on the employee statement. Evidence remains HR private. Do not add salary/13th-month amounts already in the included source payslips.");let mut remove=None;for(i,l)in f.w.lines.iter_mut().enumerate(){ui.push_id(i,|ui|{field(ui,"Statement label",&mut l.label);choice(ui,"kind","Treatment",&mut l.kind,&["taxable","shared_benefit","deposit_return","lawful_deduction"]);amount(ui,"Amount",&mut l.amount_cents);note(ui,"Evidence / lawful authorization (HR private)",&mut l.reference);if ui.button("Remove amount").clicked(){remove=Some(i);}ui.separator();});}if let Some(i)=remove{f.w.lines.remove(i);}if f.w.lines.len()<32&&ui.button("Add reviewed amount").clicked(){f.w.lines.push(final_pay::Line{label:String::new(),kind:"taxable".into(),amount_cents:0,reference:String::new()});}});});
        egui::CollapsingHeader::new("Additional contributions & release deadline").show(ui,|ui|{ui.add_enabled_ui(editable,|ui|{amount(ui,"Extra employee SSS (not already in source slips)",&mut f.w.sss_adjustment_cents);amount(ui,"Extra employee PhilHealth",&mut f.w.philhealth_adjustment_cents);amount(ui,"Extra employee Pag-IBIG",&mut f.w.pagibig_adjustment_cents);note(ui,"Contribution reconciliation, including zero (HR private)",&mut f.w.contribution_note);ui.horizontal_wrapped(|ui|{ui.label("Release within days from separation");ui.add(egui::DragValue::new(&mut f.w.release_within_days).range(1..=30));});ui.label("Clearance does not restart the deadline. A more favorable company deadline may be shorter than 30 days.");});});
        egui::CollapsingHeader::new("Annual income openings & prior-employer certificates").show(ui,|ui|{ui.add_enabled_ui(editable,|ui|{
   ui.label("Evidence must cover every missing current-employer interval, including explicitly reviewed zero income. Received and outstanding source periods cannot overlap openings.");let mut remove=None;
   for(i,o)in f.openings.iter_mut().enumerate(){ui.push_id(("opening",i),|ui|{field(ui,"Opening start",&mut o.start);field(ui,"Opening end",&mut o.end);amount(ui,"Taxable compensation (net mandatory contributions)",&mut o.row.taxable_cents);amount(ui,"Original withholding",&mut o.row.withholding_cents);amount(ui,"Shared benefits gross",&mut o.row.benefit_gross_cents);amount(ui,"Benefits already excluded",&mut o.row.benefit_exempt_cents);note(ui,"Opening evidence (HR private)",&mut o.row.reference);if ui.button("Remove opening").clicked(){remove=Some(i);}ui.separator();});}if let Some(i)=remove{f.openings.remove(i);}
   if f.openings.len()<32&&ui.button("Add evidenced current-employer interval").clicked(){let gap=final_pay::calculate(&src,&f.w).ok().and_then(|c|c.tax.coverage_gaps.first().cloned());let(a,b)=gap.map(|g|(g.starts_on,g.ends_on)).unwrap_or((src.annual.starts_on,src.service_end));f.openings.push(Opening{start:a.to_string(),end:b.to_string(),row:tax::Opening{starts_on:a,ends_on:b,taxable_cents:0,withholding_cents:0,benefit_gross_cents:0,benefit_exempt_cents:0,reference:String::new()}});}
   let mut remove=None;for(i,o)in f.previous.iter_mut().enumerate(){ui.push_id(("previous",i),|ui|{field(ui,"Previous employer",&mut o.row.employer);field(ui,"Employment start",&mut o.start);field(ui,"Employment end",&mut o.end);amount(ui,"Certified taxable compensation",&mut o.row.taxable_cents);amount(ui,"Withholding after former-employer refund",&mut o.row.withholding_cents);amount(ui,"Certified benefits gross",&mut o.row.benefit_gross_cents);amount(ui,"Certified benefits excluded",&mut o.row.benefit_exempt_cents);note(ui,"Certificate / evidence (HR private)",&mut o.row.reference);if ui.button("Remove previous employer").clicked(){remove=Some(i);}ui.separator();});}if let Some(i)=remove{f.previous.remove(i);}
   let jan=NaiveDate::from_ymd_opt(2026,1,1).unwrap();if src.annual.starts_on>jan&&f.previous.len()<8&&ui.button("Add previous-employer certificate").clicked(){let end=src.annual.starts_on.pred_opt().unwrap();f.previous.push(Previous{start:jan.to_string(),end:end.to_string(),row:tax::PreviousEmployer{starts_on:jan,ends_on:end,employer:String::new(),taxable_cents:0,withholding_cents:0,benefit_gross_cents:0,benefit_exempt_cents:0,reference:String::new()}});}
   amount(ui,"Benefits already counted as ordinary taxable income",&mut f.w.tax.reclassified_benefit_cents);note(ui,"Reclassification evidence (HR private)",&mut f.w.tax.classification_note);super::tax_view::signed_amount(ui,"Signed taxable compensation correction",&mut f.w.tax.taxable_adjustment_cents);note(ui,"Taxable correction evidence (HR private)",&mut f.w.tax.adjustment_note);
  });});
        egui::CollapsingHeader::new("Required reviews & tax handling").default_open(true).show(ui,|ui|{ui.add_enabled_ui(editable,|ui|{ui.checkbox(&mut f.w.source_scope_reviewed,"Last salary/cutoff and earned 13th-month scope is complete; no owed source omitted");ui.checkbox(&mut f.w.entitlements_reviewed,"Leave, separation/retirement, deposits and lawful deductions reviewed, including zero");ui.checkbox(&mut f.w.contributions_reviewed,"Additional contributions reconciled once, including zero");ui.checkbox(&mut f.w.tax.final_compensation_reviewed,"Prospective final and received compensation is complete");ui.checkbox(&mut f.w.tax.previous_employers_reviewed,"Net prior-employer certificates / no prior employer reviewed");ui.checkbox(&mut f.w.tax.benefits_reviewed,"Shared PHP 90,000 cap and prior vacation conversions reviewed");note(ui,"Final entitlement review (HR private)",&mut f.w.review_note);note(ui,"Annual tax review (HR private)",&mut f.w.tax.review_note);note(ui,"Final cash, refund and employer-handling plan (HR private)",&mut f.w.tax.settlement_note);});});
        let parsed = (|| -> anyhow::Result<()> {
            f.w.retirement.birth_date = if f.birth.trim().is_empty() {
                None
            } else {
                Some(f.birth.trim().parse()?)
            };
            for o in &mut f.openings {
                o.row.starts_on = o.start.parse()?;
                o.row.ends_on = o.end.parse()?;
            }
            for o in &mut f.previous {
                o.row.starts_on = o.start.parse()?;
                o.row.ends_on = o.end.parse()?;
            }
            f.w.tax.openings = f.openings.iter().map(|o| o.row.clone()).collect();
            f.w.tax.previous_employers = f.previous.iter().map(|o| o.row.clone()).collect();
            Ok(())
        })();
        if let Err(e) = parsed {
            ui.colored_label(
                Color32::DARK_RED,
                format!("Use valid YYYY-MM-DD dates: {e}"),
            );
            return !ui.button("Back").clicked();
        }
        card(ui, |ui| {
            ui.heading("Prospective final statement");
            match final_pay::calculate(&src, &f.w) {
                Ok(c) => {
                    numbers(ui, &c);
                    if editable {
                        if let Err(e) = final_pay::ready(&src, &f.w) {
                            ui.label(RichText::new(e.to_string()).color(muted(ui)));
                        }
                    }
                }
                Err(e) => {
                    ui.colored_label(Color32::DARK_RED, e.to_string());
                }
            }
        });
        if editable {
            note(ui, "Reason for worksheet edit (HR private)", &mut f.reason);
            let a = Change::new(
                "final_save",
                json!({"id":f.record.id,"expected_revision":f.record.revision,"worksheet":f.w,"reason":f.reason}),
            );
            let (save, cancel) = footer(ui, &a, &self.data, "Save final-pay worksheet");
            if save {
                self.send_payroll(a);
            }
            !cancel
        } else {
            !ui.button("Back to final-pay review").clicked()
        }
    }
    fn final_slips(&mut self, ui: &mut Ui) {
        let hr = self.data.access.role.manages_hr();
        let slips = self
            .data
            .payroll
            .finals
            .slips
            .iter()
            .filter(|s| {
                s.company_id == self.data.company.id
                    && (hr || self.data.access.employee_id == Some(s.employee_id))
            })
            .cloned()
            .collect::<Vec<_>>();
        if let Some(s) = slips
            .iter()
            .find(|s| Some(s.id) == self.payroll_state.finals.slip)
        {
            if ui.button("Back to issued final statements").clicked() {
                self.payroll_state.finals.slip = None;
                return;
            }
            card(ui, |ui| {
                ui.heading("Final pay statement");
                ui.label(format!(
                    "{} · {} · {}",
                    s.employee_code, s.employee_name, s.department
                ));
                ui.label(format!(
                    "Last working day {} · Planned cash {}",
                    s.service_end, s.cash_date
                ));
                if let Some(st) = final_pay::status(&self.data, s.id) {
                    ui.strong(format!(
                        "{} recorded on {}",
                        if st.kind == "settled" {
                            "Settlement"
                        } else {
                            "Withdrawal"
                        },
                        st.recorded_on
                    ));
                    if st.kind == "withdrawn" {
                        ui.label("Withdrawn before settlement. The retained amounts describe the old statement; prepare a fresh review for payment.");
                    }
                } else {
                    ui.label("Approved statement; actual combined cash not yet recorded.");
                }
                numbers(ui, &s.totals);
                ui.collapsing("Included source payslips", |ui| {
                    for p in s.sources.as_array().into_iter().flatten() {
                        ui.label(format!(
                            "{} · through {} · net {}",
                            p["kind"].as_str().unwrap_or(""),
                            p["ends_on"].as_str().unwrap_or(""),
                            payroll::money(p["amount_cents"].as_i64().unwrap_or(0))
                        ));
                    }
                });
                for l in s.components.as_array().into_iter().flatten() {
                    ui.label(format!(
                        "{} · {}",
                        l["label"].as_str().unwrap_or(""),
                        payroll::money(l["amount_cents"].as_i64().unwrap_or(0))
                    ));
                }
                ui.label("Source balances are included in the combined final cash, not paid again. Recording a settlement moves no money and files no government return.");
                ui.horizontal_wrapped(|ui| {
                    if ui.button("Download printable PDF").clicked() {
                        self.payroll_export("final_statement_pdf", Some(s.id));
                    }
                    if ui.button("Download CSV").clicked() {
                        self.payroll_export("final_statement", Some(s.id));
                    }
                });
            });
            return;
        }
        if slips.is_empty() {
            ui.label("No issued final statements available.");
        }
        for s in slips.iter().rev() {
            card(ui, |ui| {
                if ui
                    .link(format!(
                        "{} · {} · {}",
                        s.employee_code, s.employee_name, s.service_end
                    ))
                    .clicked()
                {
                    self.payroll_state.finals.slip = Some(s.id);
                }
                ui.label(format!(
                    "Combined final cash {} · {}",
                    payroll::money(s.totals.net_cents),
                    final_pay::status(&self.data, s.id)
                        .map(|s| s.kind.as_str())
                        .unwrap_or("approved; cash not recorded")
                ));
            });
        }
    }
}

#[cfg(feature = "qa-screenshot")]
impl HrisApp {
    pub(super) fn prepare_final_preview(&mut self, form: &str) {
        if form.contains("cross-year"){self.prepare_cross_year_final_preview(form);return;}
        self.prepare_payment_preview("payroll-payment");
        let mut store = DemoStore::new();
        store.data = self.data.clone();
        let employee = store.data.employees[1].id;
        let other = store.data.employees[2].id;
        for e in &mut store.data.employees {
            if [employee, other].contains(&e.id) {
                e.joined_on = Some(NaiveDate::from_ymd_opt(2026, 1, 1).unwrap());
            }
        }
        let admin = store.data.access.user_id;
        let hr = Uuid::from_u128(99300);
        let today = store.data.company.today();
        let first = Uuid::from_u128(99301);
        let second = Uuid::from_u128(99302);
        let apply = |s: &mut DemoStore, op: &str, v: serde_json::Value| {
            s.execute(Action::Payroll(Change::new(op, v)))
                .expect("Fictional final-pay preview");
        };
        for (eid, id) in [(employee, first), (other, second)] {
            store.data.access.user_id = hr;
            store.data.access.role = Role::HrGeneralist;
            apply(
                &mut store,
                "final_create",
                json!({"id":id,"employee_id":eid,"service_end":"2026-09-30","cash_date":today,"title":"Reviewed final pay","reason":"Fictional final-pay review"}),
            );
            let r = store.data.payroll.finals.records.last().unwrap().clone();
            let w=final_pay::Worksheet{vacation:final_pay::Vacation{days_hundredths:1350,daily_cents:120000,previous_days_hundredths:200,reference:"Fictional reviewed vacation balance, rate and earlier conversion".into()},separation:final_pay::Separation{cause:"resignation".into(),reference:"Fictional voluntary separation; no statutory award".into(),..Default::default()},lines:vec![final_pay::Line{label:"Returned employee deposit".into(),kind:"deposit_return".into(),amount_cents:150000,reference:"Fictional retained deposit evidence".into()}],entitlements_reviewed:true,source_scope_reviewed:true,contributions_reviewed:true,contribution_note:"Fictional contribution reconciliation; source payslip already contains all mandatory contributions".into(),review_note:"Fictional salary,13th-month, leave and final obligation scope reviewed".into(),tax:tax::Worksheet{openings:vec![tax::Opening{starts_on:NaiveDate::from_ymd_opt(2026,1,1).unwrap(),ends_on:NaiveDate::from_ymd_opt(2026,8,31).unwrap(),taxable_cents:22000000,withholding_cents:600000,benefit_gross_cents:0,benefit_exempt_cents:0,reference:"Fictional certified current-employer opening".into()}],final_compensation_reviewed:true,previous_employers_reviewed:true,benefits_reviewed:true,review_note:"Fictional annual source, shared benefits and no former employer confirmed".into(),settlement_note:"Fictional planned cash includes reviewed tax refund; no money transferred".into(),..Default::default()},..Default::default()};
            apply(
                &mut store,
                "final_save",
                json!({"id":id,"expected_revision":r.revision,"worksheet":w,"reason":"Fictional worksheet review"}),
            );
            let rev = store.data.payroll.finals.records.last().unwrap().revision;
            if id == first && (form.ends_with("worksheet") || form.ends_with("new")) {
                continue;
            }
            apply(
                &mut store,
                "final_submit",
                json!({"id":id,"expected_revision":rev,"reason":"Fictional independent submission"}),
            );
            if id == first && form.ends_with("approve") {
                continue;
            }
            store.data.access.user_id = admin;
            store.data.access.role = Role::Administrator;
            let rev = store.data.payroll.finals.records.last().unwrap().revision;
            apply(
                &mut store,
                "final_approve",
                json!({"id":id,"expected_revision":rev,"reason":"Fictional independent Administrator approval"}),
            );
            if id == first
                && ["settled", "manager", "withdrawn"]
                    .iter()
                    .any(|s| form.ends_with(s))
            {
                let rev = store.data.payroll.finals.records.last().unwrap().revision;
                if form.ends_with("withdrawn") {
                    apply(
                        &mut store,
                        "final_withdraw",
                        json!({"id":id,"expected_revision":rev,"unpaid_confirmed":true,"reason":"Fictional unpaid date change"}),
                    );
                } else {
                    apply(
                        &mut store,
                        "final_settle",
                        json!({"id":id,"expected_revision":rev,"settled_on":today,"payment_reference":"Fictional recorded combined settlement","handling_reference":"","cash_received_confirmed":true,"employer_handling_confirmed":true,"reason":"Fictional final cash and tax records only; no transfer"}),
                    );
                }
            }
        }
        store.data.access.user_id = admin;
        store.data.access.role = Role::Administrator;
        self.data = store.data;
        self.payroll_state.tab = 10;
        self.payroll_state.finals = State::default();
        self.payroll_state.finals.selected = Some(first);
        let r = self
            .data
            .payroll
            .finals
            .records
            .iter()
            .find(|r| r.id == first)
            .unwrap()
            .clone();
        if form.ends_with("list") {
            self.payroll_state.finals.selected = None;
        }
        if form.ends_with("worksheet") {
            self.payroll_state.finals.form = Some(Form::Worksheet(Box::new(worksheet(r.clone()))));
        }
        if form.ends_with("new") {
            self.payroll_state.finals.form = Some(Form::New(NewForm {
                id: Uuid::from_u128(99303),
                employee: Some(employee),
                end: "2026-09-30".into(),
                cash: today.to_string(),
                title: "Final pay".into(),
                reason: "Fictional separation review".into(),
            }));
        }
        if form.ends_with("approve") {
            self.final_decide(&r, "approve");
        }
        if form.ends_with("settle") {
            self.final_decide(&r, "settle");
        }
        if form.ends_with("manager") {
            self.data.access.role = Role::Manager;
            self.data.access.employee_id = Some(employee);
            self.payroll_state.finals.selected = None;
            self.payroll_state.finals.slips = true;
        }
        if form.ends_with("settled") || form.ends_with("withdrawn") {
            self.payroll_state.finals.slips = true;
            self.payroll_state.finals.slip = Some(first);
        }
        if let Ok(path) = std::env::var("HRIS_PREVIEW_FINAL_PDF") {
            if let Some(s) = self
                .data
                .payroll
                .finals
                .slips
                .iter()
                .find(|s| s.id == first)
            {
                let bytes = crate::payroll_pdf::build_final(s, final_pay::status(&self.data, s.id))
                    .expect("Fictional final PDF");
                crate::payroll_pdf::save(std::path::Path::new(&path), &bytes)
                    .expect("New fictional final PDF");
            }
        }
    }
}
#[cfg(feature="qa-screenshot")]
impl HrisApp{
 fn prepare_cross_year_final_preview(&mut self,form:&str){
  self.prepare_payment_preview("payroll-payment");let mut store=DemoStore::new();store.data=self.data.clone();store.data.payroll=Default::default();let eid=store.data.employees[1].id;store.data.employees[1].joined_on=Some("2020-01-01".parse().unwrap());let admin=store.data.access.user_id;let hr=Uuid::from_u128(99500);let id=Uuid::from_u128(99501);let today=store.data.company.today();store.data.access.role=Role::HrGeneralist;store.data.access.user_id=hr;
  let apply=|s:&mut DemoStore,op:&str,v:serde_json::Value|{s.execute(Action::Payroll(Change::new(op,v))).expect("Fictional delayed final-pay preview");};
  apply(&mut store,"final_create",json!({"id":id,"employee_id":eid,"service_end":"2025-12-31","cash_date":today,"title":"Delayed final pay · prior-year separation","reason":"Fictional actual receipt year"}));let mut r=store.data.payroll.finals.records[0].clone();let mut w=r.worksheet.clone();w.cash_year_reviewed=true;w.cash_year_note="Fictional evidence: amount was not made available or received in 2025. The 2026 receipt and other employment reviewed.".into();w.entitlements_reviewed=true;w.source_scope_reviewed=true;w.contributions_reviewed=true;w.review_note="Fictional owed prior service, no automatic clearance or access blocker".into();w.contribution_note="Fictional original-month contributions reconciled, no extra charge".into();w.lines=vec![final_pay::Line{label:"Delayed earned salary".into(),kind:"taxable".into(),amount_cents:30000000,reference:"Fictional reviewed unpaid obligation".into()}];w.tax.final_compensation_reviewed=true;w.tax.previous_employers_reviewed=true;w.tax.benefits_reviewed=true;w.tax.review_note="Fictional payment-year income and no other employer reviewed".into();w.tax.settlement_note="Fictional combined cash and statutory withholding reviewed".into();apply(&mut store,"final_save",json!({"id":id,"expected_revision":r.revision,"worksheet":w,"reason":"Fictional cross-year worksheet"}));r=store.data.payroll.finals.records[0].clone();
  if form.ends_with("paid"){apply(&mut store,"final_submit",json!({"id":id,"expected_revision":r.revision,"reason":"Fictional delayed final submit"}));r=store.data.payroll.finals.records[0].clone();store.data.access.role=Role::Administrator;store.data.access.user_id=admin;apply(&mut store,"final_approve",json!({"id":id,"expected_revision":r.revision,"reason":"Fictional independent approval"}));r=store.data.payroll.finals.records[0].clone();apply(&mut store,"final_settle",json!({"id":id,"expected_revision":r.revision,"settled_on":today,"payment_reference":"Fictional delayed cash","handling_reference":"","cash_received_confirmed":true,"employer_handling_confirmed":true,"reason":"Fictional delayed final cash only"}));}
  store.data.access.role=Role::Administrator;store.data.access.user_id=admin;self.data=store.data;self.payroll_state.tab=10;self.payroll_state.finals=State::default();self.payroll_state.finals.selected=Some(id);
  if form.ends_with("paid"){self.payroll_state.finals.slips=true;self.payroll_state.finals.slip=Some(id);}else{self.payroll_state.finals.form=Some(Form::Worksheet(Box::new(worksheet(self.data.payroll.finals.records[0].clone()))));}
  if let Ok(path)=std::env::var("HRIS_PREVIEW_FINAL_PDF"){if let Some(s)=self.data.payroll.finals.slips.first(){crate::payroll_pdf::save(std::path::Path::new(&path),&crate::payroll_pdf::build_final(s,final_pay::status(&self.data,id)).unwrap()).unwrap();}}
 }
}
