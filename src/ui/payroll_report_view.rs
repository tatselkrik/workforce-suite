use super::payroll_view::field;
use super::*;
use crate::payroll_reporting as p;
#[derive(Default)]
pub(super) struct State {
    scope: Option<(Uuid, Uuid, Role)>,
    lo: String,
    hi: String,
    axis: String,
    kind: String,
    employee: Option<Uuid>,
    search: String,
    page: usize,
    result: Option<p::Report>,
}
impl State {
    pub(super) fn bind(&mut self, data: &Snapshot) {
        let scope = Some((data.company.id, data.access.user_id, data.access.role));
        if self.scope != scope || !data.access.role.manages_hr() {
            let year = data.company.today().year();
            *self = Self {
                scope,
                lo: format!("{year}-01-01"),
                hi: format!("{year}-12-31"),
                axis: "issued".into(),
                kind: "all".into(),
                ..Default::default()
            };
        }
    }
    fn filter(&self) -> anyhow::Result<p::Filter> {
        let f = p::Filter {
            starts_on: self
                .lo
                .parse()
                .map_err(|_| anyhow::anyhow!("Enter a valid starting date."))?,
            ends_on: self
                .hi
                .parse()
                .map_err(|_| anyhow::anyhow!("Enter a valid ending date."))?,
            axis: self.axis.clone(),
            kind: self.kind.clone(),
            employee_id: self.employee,
            search: self.search.clone(),
        };
        f.validate()?;
        Ok(f)
    }
    pub(super) fn accept(&mut self, report: p::Report, data: &Snapshot) {
        self.bind(data);
        if data.access.role.manages_hr()
            && report.wire.company_id == data.company.id
            && self.filter().is_ok_and(|f| f == report.wire.filter)
        {
            self.result = Some(report);
            self.page = 0;
        }
    }
}
pub(super) fn kind(k: &str) -> &str {
    match k {
        "monthly" => "Monthly",
        "cutoff" => "Cutoff",
        "thirteenth" => "13th-month",
        "annual_tax" => "Annual tax advice",
        "final" => "Final statement",
        "amendment" => "Linked amendments",
        _ => "All sources",
    }
}
fn entry(ui:&mut Ui,label:&str,value:&str){
    let width=(ui.available_width()*0.62).clamp(160.0,440.0);
    ui.horizontal_top(|ui|{
        ui.allocate_ui_with_layout(egui::vec2(width,0.0),Layout::top_down(Align::Min),|ui|{
            ui.set_min_width(width);ui.set_max_width(width);ui.add(egui::Label::new(label).wrap());
        });
        ui.label(value);
    });
}
fn amount(ui:&mut Ui,label:&str,n:i64){entry(ui,label,&crate::payroll::money(n));}
impl HrisApp {
    pub(super) fn payroll_reports(&mut self, ui: &mut Ui) {
        self.payroll_state.reports.bind(&self.data);
        if !self.data.access.role.manages_hr() {
            return;
        }
        ui.heading("Consolidated payroll reports");
        ui.label("Review issued records or actual recorded payment/settlement dates across salary, cutoffs, 13th-month pay, annual tax and final pay.");
        let s = &mut self.payroll_state.reports;
        ui.columns(2, |c| {
            field(&mut c[0], "From (YYYY-MM-DD)", &mut s.lo);
            field(&mut c[1], "Through (YYYY-MM-DD)", &mut s.hi);
        });
        ui.horizontal_wrapped(|ui| {
            egui::ComboBox::from_id_salt("pay_report_axis")
                .selected_text(if s.axis == "received" {
                    "Payment / settlement date"
                } else {
                    "Approval date"
                })
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut s.axis, "issued".into(), "Approval date");
                    ui.selectable_value(
                        &mut s.axis,
                        "received".into(),
                        "Payment / settlement date",
                    );
                });
            egui::ComboBox::from_id_salt("pay_report_kind")
                .selected_text(kind(&s.kind))
                .show_ui(ui, |ui| {
                    for k in [
                        "all",
                        "monthly",
                        "cutoff",
                        "thirteenth",
                        "annual_tax",
                        "final",
                        "amendment",
                    ] {
                        ui.selectable_value(&mut s.kind, k.into(), kind(k));
                    }
                });
            egui::ComboBox::from_id_salt("pay_report_employee")
                .selected_text(
                    s.employee
                        .map(|id| self.data.employee_name(id))
                        .unwrap_or("All employees"),
                )
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut s.employee, None, "All employees");
                    for e in self
                        .data
                        .employees
                        .iter()
                        .filter(|e| e.company_id == self.data.company.id)
                    {
                        ui.selectable_value(
                            &mut s.employee,
                            Some(e.id),
                            format!("{} · {}", e.employee_code, e.full_name),
                        );
                    }
                });
        });
        ui.horizontal_wrapped(|ui| {
            ui.label("Search saved name, code or department");
            ui.text_edit_singleline(&mut s.search);
        });
        let filter = s.filter();
        let mut generate = false;
        let mut export = false;
        ui.horizontal_wrapped(|ui| {
            generate = ui
                .add_enabled(
                    filter.is_ok() && !self.busy,
                    egui::Button::new("Generate / refresh report"),
                )
                .clicked();
            export = ui
                .add_enabled(
                    filter
                        .as_ref()
                        .is_ok_and(|f| s.result.as_ref().is_some_and(|r| &r.wire.filter == f))
                        && !self.busy,
                    egui::Button::new("Export fresh CSV"),
                )
                .clicked();
        });
        if let Err(e) = &filter {
            ui.label(RichText::new(e.to_string()).color(muted(ui)));
        }
        if let Ok(f) = filter {
            if generate {
                self.send(Command::PayrollReport {
                    filter: f.clone(),
                    destination: None,
                });
            }
            if export {
                let name = format!("HRIS-payroll-{}-{}-{}.csv", f.axis, f.starts_on, f.ends_on);
                match crate::reports::choose_export(&name) {
                    Ok(Some(path)) => self.send(Command::PayrollReport {
                        filter: f.clone(),
                        destination: Some(path),
                    }),
                    Ok(None) => {}
                    Err(e) => self.notice = Some((e.to_string(), true)),
                }
            }
            self.payroll_report_result(ui, &f);
        }
        ui.add_space(12.0);
        ui.collapsing("Existing monthly-register exports", |ui| {
            self.payroll_legacy_reports(ui)
        });
    }
    fn payroll_report_result(&mut self, ui: &mut Ui, filter: &p::Filter) {
        let s = &mut self.payroll_state.reports;
        let Some(report) = s.result.as_ref().filter(|r| r.wire.filter == *filter) else {
            ui.label("Generate this filter to view its current figures.");
            return;
        };
        let t = &report.totals;
        ui.label(format!(
            "Generated {} · {} record(s)",
            report.wire.generated_at.format("%Y-%m-%d %H:%M UTC"),
            report.wire.rows.len()
        ));
        ui.label("Source offsets are included in a combined final settlement and are not another cash payment. Tax advice and settled tax adjustments are shown separately from payroll cash.");
        card(ui, |ui| {
            ui.vertical(|ui|{
                    amount(ui, "Recorded payroll cash", t.recorded_cash_cents);
                    amount(ui,"Outstanding supplemental payouts",t.amendment_outstanding_cents);
                    if t.overpayment_review_cents!=0{amount(ui,"Overpayment reviews (no automatic recovery)",t.overpayment_review_cents);}
                    amount(
                        ui,
                        "Source offsets included in final settlement",
                        t.source_offset_cents,
                    );
                    amount(
                        ui,
                        "Outstanding selected salary / 13th sources",
                        t.current_source_outstanding_cents,
                    );
                    amount(
                        ui,
                        "Reserved subset of those sources",
                        t.current_source_reserved_cents,
                    );
                });
        });
        ui.label("Balances reflect current records at generation for the selected sources, rather than a reconstructed historical close. A final statement contains its reserved source balances.");
        if t.missing_receipts > 0 {
            ui.label(RichText::new(format!("{} source(s) marked paid have no individual receipt. Their payment date, cash and outstanding balance remain unconfirmed.",t.missing_receipts)).color(muted(ui)));
        }
        #[cfg(feature="qa-screenshot")]
        let figures_open=std::env::var("HRIS_PREVIEW_FORM").is_ok_and(|s|s.ends_with("figures"));
        #[cfg(not(feature="qa-screenshot"))]
        let figures_open=false;
        egui::CollapsingHeader::new("Issued and reviewed figures by source").default_open(figures_open).show(ui, |ui| {
   ui.vertical(|ui|{
    amount(ui,"Salary / cutoff / 13th issued gross",t.source_gross_cents);amount(ui,"Salary / cutoff / 13th issued net",t.source_net_cents);
    amount(ui,"Employee contributions in those sources",t.source_employee_contributions_cents);amount(ui,"Employer contributions in those sources",t.source_employer_contributions_cents);amount(ui,"Source withholding",t.source_withholding_cents);
    amount(ui,"Final statements issued, including withdrawn",t.final_statement_net_cents);amount(ui,"Source net contained in final statements",t.final_contained_source_net_cents);amount(ui,"Final net beyond those sources (may be negative)",t.final_additional_net_cents);
    amount(ui,"Annual tax advice (+ due / - refund)",t.annual_advised_adjustment_cents);amount(ui,"Settled subset of those annual tax adjustments",t.annual_settled_adjustment_cents);
   });ui.label("Figures across source types overlap. Do not add every statement net or tax advice to payroll cash. No remittance, filing or money transfer occurs here.");
   #[cfg(feature="qa-screenshot")]
   if std::env::var("HRIS_PREVIEW_SCROLL").as_deref()==Ok("report-figures"){ui.scroll_to_cursor(Some(Align::Center));}

  });
        if report.wire.rows.is_empty() {
            empty(ui, "No issued or received records match these filters.");
            return;
        }
        let count = report.wire.rows.len();
        let pages = count.div_ceil(50);
        s.page = s.page.min(pages - 1);
        ui.horizontal_wrapped(|ui| {
            if ui
                .add_enabled(s.page > 0, egui::Button::new("Previous"))
                .clicked()
            {
                s.page -= 1;
            }
            ui.label(format!(
                "Page {} of {} · CSV includes all {} records",
                s.page + 1,
                pages,
                count
            ));
            if ui
                .add_enabled(s.page + 1 < pages, egui::Button::new("Next"))
                .clicked()
            {
                s.page += 1;
            }
        });
        for r in report.wire.rows.iter().skip(s.page * 50).take(50) {
            card(ui, |ui| {
                ui.label(
                    RichText::new(format!(
                        "{} · {} · {}",
                        r.employee_code,
                        r.employee_name,
                        kind(&r.kind)
                    ))
                    .strong(),
                );
                ui.label(format!(
                    "{} through {} · approved {} · {}",
                    r.period_start, r.period_end, r.issued_on, r.department
                ));
                ui.label(if r.status==r.payment_state{r.status.replace('_'," ")}else{format!("{} · {}",r.status.replace('_'," "),r.payment_state.replace('_'," "))});
                ui.vertical(|ui|{
                        if let Some(net) = r.net_cents {
                            amount(ui, "Statement net", net);
                        }
                        if let Some(id)=r.linked_id{entry(ui,"Linked original",&format!("{} · {id}",r.linked_kind.as_deref().unwrap_or("")));}
                        if let Some(v)=r.review_amount_cents{amount(ui,"Discrepancy review only",v);}
                        amount(ui, "Recorded payroll cash", r.cash_cents);
                        if r.offset_cents > 0 {
                            amount(
                                ui,
                                "Source offset (not another cash payment)",
                                r.offset_cents,
                            );
                        }
                        if let Some(v) = r.contained_source_net_cents {
                            amount(ui, "Contained source net", v);
                        }
                        if let Some(v) = r.tax_adjustment_cents {
                            amount(ui, "Tax adjustment: withholding (+) / refund (-)", v);
                        }
                        if let Some(v) = r.outstanding_cents {
                            amount(ui, if r.kind=="amendment"{"Current supplemental outstanding"}else{"Current source outstanding"}, v);
                        }
                        if r.payment_state=="reserved" {
                            amount(ui, "Current reserved subset", r.reserved_cents);
                        }
                        if let Some(d) = r.received_on {
                            entry(ui,"Received / settled on",&d.to_string());
                        }
                    });
            });
        }
    }
    #[cfg(feature = "qa-screenshot")]
    pub(super) fn prepare_consolidated_report_preview(&mut self, form: &str) {
        if form.contains("amendment"){self.prepare_amendment_preview("payroll-amendment-paid");}else{self.prepare_final_preview("payroll-final-settled");}
        self.payroll_state.tab = 4;
        self.payroll_state.reports = State::default();
        self.payroll_state.reports.bind(&self.data);
        if form.ends_with("received") {
            self.payroll_state.reports.axis = "received".into();
        }
        if form.ends_with("empty") {
            self.payroll_state.reports.lo = "2027-01-01".into();
            self.payroll_state.reports.hi = "2027-12-31".into();
        }
        let f = self.payroll_state.reports.filter().unwrap();
        let report =
            p::demo(&self.data, &f, chrono::Utc::now()).expect("Fictional mixed payroll report");
        if let Ok(path) = std::env::var("HRIS_PREVIEW_PAYROLL_REPORT_CSV") {
            crate::reports::save_csv(std::path::Path::new(&path), &report.csv().unwrap()).unwrap();
        }
        self.payroll_state.reports.accept(report, &self.data);
        if form.ends_with("manager") {
            self.data.access.role = Role::Manager;
            self.payroll_state.tab = 3;
        }
    }
}
