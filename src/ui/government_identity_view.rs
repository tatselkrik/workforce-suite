use super::*;
use crate::payroll_government::bir_identity::{Report, Status};
use chrono::Datelike;
use std::path::PathBuf;

#[derive(Default)]
pub(super) struct State {
    year: i32,
    source: Option<PathBuf>,
    report: Option<Report>,
    page: usize,
    excel: bool,
    pending: bool,
}
impl State {
    pub(super) fn accept(&mut self, r: Report, data: &Snapshot) {
        if data.access.role.manages_hr()
            && r.company_id == data.company.id
            && r.user_id == data.access.user_id
            && r.year == self.year
        {
            self.report = Some(r);
            self.page = 0;
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
    pub(super) fn preview() -> Self {
        use crate::payroll_government::{EmployeeLabel, bir_identity::Row};
        let person = EmployeeLabel {
            employee_id: Uuid::from_u128(2),
            employee_code: "000008".into(),
            employee_name: "Fictional Ángela Cruz".into(),
        };
        Self {
            year: 2026,
            source: Some("Fictional source.dat".into()),
            report: Some(Report {
                company_id: Uuid::from_u128(1),
                user_id: Uuid::from_u128(1),
                year: 2026,
                as_of: "2026-10-26".parse().unwrap(),
                generated_at: "2026-10-26T15:00:00Z".parse().unwrap(),
                source_sha256: "b".repeat(64),
                file_name: "Fictional source.dat".into(),
                schedule: crate::payroll_government::bir_file::Schedule::One,
                preflight_reviews: 0,
                roster_people: 12,
                usable_registrations: 9,
                received_people_not_uniquely_matched: 2,
                rows: vec![
                    Row {
                        line: 2,
                        status: Status::CurrentPairMatched,
                        employee: Some(person),
                        candidates: 1,
                        observed_sources: Some(7),
                        received_sources: Some(3),
                    },
                    Row {
                        line: 3,
                        status: Status::Unmatched,
                        employee: None,
                        candidates: 0,
                        observed_sources: None,
                        received_sources: None,
                    },
                    Row {
                        line: 4,
                        status: Status::Ambiguous,
                        employee: None,
                        candidates: 2,
                        observed_sources: None,
                        received_sources: None,
                    },
                ],
            }),
            ..Default::default()
        }
    }
}
enum Intent {
    Choose,
    Check(PathBuf),
    Export(PathBuf, String, bool),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::payroll_government::bir_identity::{self as bi, tests as fixture};
    #[test]
    fn private_identity_response_requires_same_year_user_company_and_hr_role() {
        let mut data = DemoStore::new().data;
        data.access.user_id = Uuid::from_u128(4);
        let basis = bi::Basis::from_wire(fixture::wire(), data.company.id, 2026).unwrap();
        let report = bi::review(
            basis,
            &fixture::source(&["234567890"]),
            fixture::FILE_NAME,
            "2026-10-26".parse().unwrap(),
            data.access.user_id,
        )
        .unwrap();
        let mut state = State {
            year: 2026,
            ..Default::default()
        };
        for variant in 0..4 {
            let mut d = data.clone();
            match variant {
                0 => d.access.role = Role::Manager,
                1 => d.access.user_id = Uuid::new_v4(),
                2 => d.company.id = Uuid::new_v4(),
                _ => state.year = 2025,
            };
            state.accept(report.clone(), &d);
            assert!(state.report.is_none());
        }
        state.year = 2026;
        state.accept(report, &data);
        assert!(state.report.is_some());
        state.pending = true;
        state.failed();
        assert!(state.report.is_none());
    }
}
impl HrisApp {
    pub(super) fn payroll_government_identities(&mut self, ui: &mut Ui) {
        let state = &mut self.payroll_state.government.identities;
        if state.year == 0 {
            state.year = self.data.company.today().year();
        }
        let mut intent = None;
        let mut back = false;
        let connected = self.data.mode == Mode::Supabase;
        ui.heading("BIR file identity review");
        ui.add(egui::Label::new("Match a local, structurally consistent standalone Schedule 1 or 2 file to current employee BIR registration pairs. Exact TIN and branch pairs are used; employee names never choose a match. The file stays on this computer.").wrap());
        ui.add(egui::Label::new(RichText::new("Current registration matches do not certify historical identity, agency assignment, file coverage, expected withholding, 2316 certificates, MWE eligibility or official validation. Payroll, registrations and the original file stay unchanged.").weak()).wrap());
        ui.add_space(8.0);
        ui.horizontal_wrapped(|ui| {
            back = ui
                .add_enabled(!self.busy, egui::Button::new("Back to reconciliation"))
                .clicked();
            ui.add_enabled_ui(!self.busy, |ui| {
                let old = state.year;
                egui::ComboBox::from_id_salt("bir_identity_year")
                    .selected_text(state.year.to_string())
                    .show_ui(ui, |ui| {
                        for y in (2025..=self.data.company.today().year().min(2200)).rev() {
                            ui.selectable_value(&mut state.year, y, y.to_string());
                        }
                    });
                if old != state.year {
                    state.report = None;
                    state.page = 0;
                }
                egui::ComboBox::from_id_salt("bir_identity_format")
                    .selected_text(if state.excel {
                        "Excel review"
                    } else {
                        "CSV review"
                    })
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut state.excel, false, "CSV review");
                        ui.selectable_value(&mut state.excel, true, "Excel review");
                    });
            });
            if ui
                .add_enabled(
                    !self.busy && connected,
                    egui::Button::new("Choose .dat source"),
                )
                .clicked()
            {
                intent = Some(Intent::Choose);
            }
            if ui
                .add_enabled(
                    !self.busy && connected && state.source.is_some(),
                    egui::Button::new("Recheck identities"),
                )
                .clicked()
            {
                intent = state.source.clone().map(Intent::Check);
            }
            if ui
                .add_enabled(
                    !self.busy && connected && state.report.is_some(),
                    egui::Button::new("Export fresh identity review"),
                )
                .clicked()
            {
                if let (Some(source), Some(r)) = (&state.source, &state.report) {
                    intent = Some(Intent::Export(
                        source.clone(),
                        r.source_sha256.clone(),
                        state.excel,
                    ));
                }
            }
        });
        if let Some(r) = &state.report {
            ui.add_space(8.0);
            ui.add(
                egui::Label::new(format!(
                    "{} · {} · Year {} · Current records as of {}",
                    r.file_name,
                    r.schedule.label(),
                    r.year,
                    r.as_of
                ))
                .wrap(),
            );
            ui.add(
                egui::Label::new(
                    RichText::new(format!("Source SHA256: {}", r.source_sha256)).monospace(),
                )
                .wrap(),
            );
            ui.small(format!("Current snapshot: {} UTC. Exports recheck the source hash and fetch current registrations again.",r.generated_at.format("%Y-%m-%d %H:%M:%S")));
            let matched = r
                .rows
                .iter()
                .filter(|r| r.status == Status::CurrentPairMatched)
                .count();
            let ambiguous = r
                .rows
                .iter()
                .filter(|r| r.status == Status::Ambiguous)
                .count();
            ui.add(egui::Label::new(RichText::new(format!("{} file identities · {} current pairs matched · {} unmatched · {} ambiguous",r.rows.len(),matched,r.rows.len()-matched-ambiguous,ambiguous)).strong()).wrap());
            ui.add(egui::Label::new(format!("{} usable current registrations among {} employees. {} employees with recorded receipts are not uniquely matched in this file; review coverage and registration history. This is not proof of missing required declarations.",r.usable_registrations,r.roster_people,r.received_people_not_uniquely_matched)).wrap());
            if r.preflight_reviews > 0 {
                ui.add(egui::Label::new(format!("{} preflight review item(s) remain. Use Check BIR 1604C file for their details; identity matching does not resolve them.",r.preflight_reviews)).wrap());
            }
            let pages = r.rows.len().div_ceil(8).max(1);
            state.page = state.page.min(pages - 1);
            ui.horizontal_wrapped(|ui| {
                if ui
                    .add_enabled(state.page > 0, egui::Button::new("Previous"))
                    .clicked()
                {
                    state.page -= 1;
                }
                ui.label(format!("Page {} of {}", state.page + 1, pages));
                if ui
                    .add_enabled(state.page + 1 < pages, egui::Button::new("Next"))
                    .clicked()
                {
                    state.page += 1;
                }
            });
            for row in r.rows.iter().skip(state.page * 8).take(8) {
                egui::Frame::group(ui.style()).show(ui,|ui|{
                ui.set_min_width(ui.available_width());ui.add(egui::Label::new(RichText::new(format!("File line {} · {}",row.line,row.status.label())).strong()).wrap());
                if let Some(p)=&row.employee{
                    ui.add(egui::Label::new(format!("{} · {}",p.employee_code,p.employee_name)).wrap());
                    ui.add(egui::Label::new(format!("{} observed annual sources · {} actual receipt sources in year",row.observed_sources.unwrap_or(0),row.received_sources.unwrap_or(0))).wrap());
                    ui.small("Source counts are not a comparison of tax amounts or proof of filing eligibility.");
                }else if row.candidates>1{ui.add(egui::Label::new(format!("{} current registrations share this pair. No employee or payroll source was selected.",row.candidates)).wrap());}
                else{ui.label("No employee or payroll source was selected. Review the original line and current registration details.");}
            });
            }
        } else {
            ui.add_space(10.0);
            ui.add(egui::Label::new("Choose the file's year and .dat source. A complete matching company BIR registration is required. Resolve structure/control errors using Check BIR 1604C file first. Mixed schedules and unsupported years remain outside this review.").wrap());
        }
        if back {
            self.payroll_state.government.identities_open = false;
            return;
        }
        let year = state.year;
        match intent {
            Some(Intent::Choose) => match super::bir_file_view::choose_source() {
                Ok(Some(path)) => {
                    state.source = Some(path.clone());
                    state.report = None;
                    state.pending = true;
                    self.send(Command::GovernmentBirIdentities {
                        year,
                        source: path,
                        expected_hash: None,
                        destination: None,
                    });
                }
                Ok(None) => {}
                Err(e) => self.notice = Some((e.to_string(), true)),
            },
            Some(Intent::Check(source)) => {
                state.pending = true;
                self.send(Command::GovernmentBirIdentities {
                    year,
                    source,
                    expected_hash: None,
                    destination: None,
                });
            }
            Some(Intent::Export(source, hash, excel)) => {
                match crate::reports::choose_export(&format!(
                    "HRIS-BIR-current-identities-{year}.{}",
                    if excel { "xlsx" } else { "csv" }
                )) {
                    Ok(Some(path)) => {
                        state.pending = true;
                        self.send(Command::GovernmentBirIdentities {
                            year,
                            source,
                            expected_hash: Some(hash),
                            destination: Some(path),
                        });
                    }
                    Ok(None) => {}
                    Err(e) => self.notice = Some((e.to_string(), true)),
                }
            }
            None => {}
        }
    }
}
