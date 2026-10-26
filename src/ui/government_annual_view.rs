use super::*;
#[cfg(any(test, feature = "qa-screenshot"))]
use crate::payroll_government::annual;
use crate::payroll_government::{State as SourceState, annual::Ledger};
use chrono::Datelike;

#[derive(Default)]
pub(super) struct State {
    year: i32,
    report: Option<Ledger>,
    search: String,
    received_only: bool,
    page: usize,
    excel: bool,
    pending: bool,
}
impl State {
    pub(super) fn accept(&mut self, report: Ledger, data: &Snapshot) {
        if data.access.role.manages_hr()
            && report.wire.company_id == data.company.id
            && report.wire.year == self.year
        {
            self.report = Some(report);
            self.pending = false;
        }
    }
    pub(super) fn failed(&mut self) {
        if self.pending {
            self.report = None;
            self.pending = false;
        }
    }
    #[cfg(feature = "qa-screenshot")]
    pub(super) fn preview(name: &str) -> Self {
        let mut wire = annual::fixture().wire;
        if name.contains("empty") {
            wire.entries.clear();
            wire.employees.clear();
        }
        if name.contains("unknown") {
            wire.entries[1].fact.parts.employee_cents = None;
        }
        let report = Ledger::from_wire(wire, Uuid::from_u128(1), 2026).unwrap();
        Self {
            year: 2026,
            report: Some(report),
            page: usize::from(name.contains("page2")),
            received_only: name.contains("received"),
            ..Default::default()
        }
    }
}
impl HrisApp {
    pub(super) fn payroll_government_annual(&mut self, ui: &mut Ui) {
        let state = &mut self.payroll_state.government.annual;
        if state.year == 0 {
            state.year = self.data.company.today().year();
        }
        let mut back = false;
        let mut read = false;
        let mut export = false;
        ui.heading("Annual recorded sources · BIR");
        ui.add(egui::Label::new("Review withholding recorded on individual receipts in the selected year. Original service dates and unpaid, missing or postdated receipts stay separate. Current records are shown as of today; this does not reconstruct a historical year-end snapshot.").wrap());
        ui.add(egui::Label::new(RichText::new("Internal source worksheet only. This is not expected BIR withholding, a tax calculation, a 1604C/2316 reconciliation or an officially validated return. Prior-employer amounts, PERA and certificate classifications are not inferred.").weak()).wrap());
        ui.add_space(8.0);
        let connected = self.data.mode == Mode::Supabase;
        ui.horizontal_wrapped(|ui| {
            back = ui
                .add_enabled(!self.busy, egui::Button::new("Back to reconciliation"))
                .clicked();
            ui.add_enabled_ui(!self.busy, |ui| {
                let old = state.year;
                egui::ComboBox::from_id_salt("government_annual_year")
                    .selected_text(state.year.to_string())
                    .show_ui(ui, |ui| {
                        for year in (2025..=self.data.company.today().year().min(2200)).rev() {
                            ui.selectable_value(&mut state.year, year, year.to_string());
                        }
                    });
                if old != state.year {
                    state.report = None;
                    state.page = 0;
                }
                egui::ComboBox::from_id_salt("government_annual_format")
                    .selected_text(if state.excel {
                        "Excel (.xlsx)"
                    } else {
                        "CSV (.csv)"
                    })
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut state.excel, false, "CSV (.csv)");
                        ui.selectable_value(&mut state.excel, true, "Excel (.xlsx)");
                    });
            });
            read = ui
                .add_enabled(
                    !self.busy && connected,
                    egui::Button::new("Generate / refresh"),
                )
                .clicked();
            export = ui
                .add_enabled(
                    !self.busy && connected && state.report.is_some(),
                    egui::Button::new("Export fresh annual worksheet"),
                )
                .clicked();
        });
        if let Some(report) = &state.report {
            let w = &report.wire;
            let s = &report.summary;
            ui.add_space(8.0);
            ui.label(format!(
                "{} · Receipt year {} · As of {} · Observed January–{}",
                w.company_name,
                w.year,
                w.as_of,
                w.through_month.format("%B")
            ));
            ui.small(format!("Generated {} UTC. Exports fetch current protected records again; screen filters do not limit exported totals.",w.generated_at.format("%Y-%m-%d %H:%M:%S")));
            let total = match s.complete_received_cents {
                Some(n) => format!(
                    "Recorded received withholding: {}",
                    crate::payroll::money(n)
                ),
                None if s.received == 0 => {
                    "No individual receipts recorded in this year; no zero-tax conclusion.".into()
                }
                None => format!(
                    "Known received subtotal: {} · Incomplete: {} amount(s) unknown",
                    crate::payroll::money(s.known_received_cents),
                    s.unknown_received
                ),
            };
            ui.add(egui::Label::new(RichText::new(total).strong()).wrap());
            ui.horizontal_wrapped(|ui| {
                for text in [
                    format!("{} received", s.received),
                    format!("{} approved/unpaid", s.unpaid),
                    format!("{} missing receipt/date", s.missing_receipt),
                    format!("{} postdated", s.postdated),
                    format!("{} withdrawn/review only", s.excluded),
                ] {
                    ui.label(text);
                    ui.separator();
                }
            });
            ui.add_space(8.0);
            ui.horizontal_wrapped(|ui| {
                let old = (state.search.clone(), state.received_only);
                ui.add(
                    egui::TextEdit::singleline(&mut state.search)
                        .hint_text("Find employee or source")
                        .desired_width(230.0),
                );
                ui.checkbox(&mut state.received_only, "Received only");
                if old != (state.search.clone(), state.received_only) {
                    state.page = 0;
                }
            });
            let query = state.search.trim().to_lowercase();
            let entries: Vec<_> = w
                .entries
                .iter()
                .filter(|e| {
                    let p = report.employee(e.fact.employee_id);
                    (!state.received_only || e.fact.state == SourceState::Received)
                        && (query.is_empty()
                            || format!(
                                "{} {} {} {}",
                                p.employee_code,
                                p.employee_name,
                                e.fact.kind.label(),
                                e.fact.id
                            )
                            .to_lowercase()
                            .contains(&query))
                })
                .collect();
            let pages = entries.len().div_ceil(6).max(1);
            state.page = state.page.min(pages - 1);
            ui.horizontal_wrapped(|ui| {
                if ui
                    .add_enabled(state.page > 0, egui::Button::new("Previous"))
                    .clicked()
                {
                    state.page -= 1;
                }
                ui.label(format!(
                    "Page {} of {} · {} of {} sources",
                    state.page + 1,
                    pages,
                    entries.len(),
                    w.entries.len()
                ));
                if ui
                    .add_enabled(state.page + 1 < pages, egui::Button::new("Next"))
                    .clicked()
                {
                    state.page += 1;
                }
            });
            if entries.is_empty() {
                ui.label(if w.entries.is_empty(){"No recorded sources were observed. This does not certify no liability or a zero return."}else{"No sources match the screen filters."});
            }
            for e in entries.into_iter().skip(state.page * 6).take(6) {
                let f = &e.fact;
                let p = report.employee(f.employee_id);
                egui::Frame::group(ui.style()).show(ui,|ui|{
                    ui.set_min_width(ui.available_width());
                    ui.add(egui::Label::new(RichText::new(format!("{} · {}",p.employee_code,p.employee_name)).strong()).wrap());
                    ui.horizontal_wrapped(|ui|{ui.label(f.kind.label());ui.separator();ui.label(f.state.label());});
                    ui.add(egui::Label::new(format!("Service: {} to {} · Actual receipt: {}",f.starts_on,f.ends_on,f.received_on.map(|d|d.to_string()).unwrap_or_else(||"Not recorded".into()))).wrap());
                    ui.label(format!("Recorded withholding: {}",f.parts.employee_cents.map(crate::payroll::money).unwrap_or_else(||"Unknown".into())));
                    ui.add(egui::Label::new(e.treatment()).wrap());
                    egui::CollapsingHeader::new("Source reference and observations").id_salt((f.kind,f.id)).show(ui,|ui|{
                        ui.add(egui::Label::new(format!("Source: {}",f.id)).wrap());
                        ui.add(egui::Label::new(format!("Observed in: {}",e.observed_months.iter().map(|d|d.format("%b %Y").to_string()).collect::<Vec<_>>().join(", "))).wrap());
                        ui.small("Observation months are query context, not evidence of payment or contribution allocation.");
                    });
                });
            }
        } else {
            ui.add_space(10.0);
            ui.label("Choose a year and generate the protected annual source ledger. No payroll or receipts will be changed.");
        }
        if back {
            self.payroll_state.government.annual_open = false;
            return;
        }
        let year = state.year;
        if export {
            let extension = if state.excel { "xlsx" } else { "csv" };
            match crate::reports::choose_export(&format!(
                "HRIS-BIR-recorded-sources-{year}.{extension}"
            )) {
                Ok(Some(path)) => {
                    state.pending = true;
                    self.send(Command::GovernmentAnnual {
                        year,
                        destination: Some(path),
                    });
                }
                Ok(None) => {}
                Err(e) => self.notice = Some((e.to_string(), true)),
            }
        } else if read {
            state.pending = true;
            self.send(Command::GovernmentAnnual {
                year,
                destination: None,
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn responses_cannot_cross_year_or_staff_scope() {
        let mut data = DemoStore::new().data;
        let r = annual::fixture();
        assert_eq!(data.company.id, r.wire.company_id);
        let mut state = State {
            year: 2025,
            ..Default::default()
        };
        state.accept(r.clone(), &data);
        assert!(state.report.is_none());
        state.year = 2026;
        data.access.role = Role::Manager;
        state.accept(r.clone(), &data);
        assert!(state.report.is_none());
        data.access.role = Role::Administrator;
        state.accept(r, &data);
        assert!(state.report.is_some());
        state.pending = true;
        state.failed();
        assert!(state.report.is_none());
    }
}
