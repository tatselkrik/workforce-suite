use super::*;
use crate::payroll_government::bir_file::{self as bir, Severity};
use std::path::PathBuf;

#[derive(Default)]
pub(super) struct State {
    source: Option<PathBuf>,
    pub(super) report: Option<bir::Report>,
    page: usize,
    excel: bool,
    pending: bool,
    expand_totals: bool,
}
impl State {
    pub(super) fn accept(&mut self, report: bir::Report, data: &Snapshot) {
        if data.access.role.manages_hr()
            && report.company_id == data.company.id
            && report.user_id == data.access.user_id
        {
            self.report = Some(report);
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
    pub(super) fn preview(errors: bool, expand_totals: bool) -> Self {
        Self {
            source: Some(PathBuf::from("Fictional source.dat")),
            report: Some(bir::preview_report(errors)),
            expand_totals,
            ..Default::default()
        }
    }
}
enum Intent {
    Back,
    Choose,
    Check(PathBuf),
    Save(PathBuf, String, bool),
}
impl HrisApp {
    pub(super) fn payroll_bir_file(&mut self, ui: &mut Ui) {
        let state = &mut self.payroll_state.government.bir;
        let mut intent = None;
        ui.heading("BIR Schedule 1 file check");
        ui.add(egui::Label::new("Read-only checks for the published 1604C Schedule 1 plaintext layout: columns, dates, codes, employer header and exact control totals. The original file and payroll are unchanged.").wrap());
        ui.add(egui::Label::new(RichText::new("This is not official BIR validation, a tax calculation, a certificate check or an HRIS payroll comparison. Schedule 2, non-ASCII encodings and sealed/validated files need separate review.").weak()).wrap());
        ui.add_space(8.0);
        let connected = self.data.mode == Mode::Supabase;
        ui.horizontal_wrapped(|ui| {
            if ui
                .add_enabled(!self.busy, egui::Button::new("Back to reconciliation"))
                .clicked()
            {
                intent = Some(Intent::Back);
            }
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
                    egui::Button::new("Recheck source"),
                )
                .clicked()
            {
                intent = state.source.clone().map(Intent::Check);
            }
            ui.add_enabled_ui(!self.busy, |ui| {
                egui::ComboBox::from_id_salt("bir_findings_format")
                    .selected_text(if state.excel {
                        "Excel findings"
                    } else {
                        "CSV findings"
                    })
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut state.excel, false, "CSV findings");
                        ui.selectable_value(&mut state.excel, true, "Excel findings");
                    });
            });
            if ui
                .add_enabled(
                    !self.busy && connected && state.source.is_some() && state.report.is_some(),
                    egui::Button::new("Save findings"),
                )
                .clicked()
            {
                if let (Some(source), Some(report)) = (&state.source, &state.report) {
                    intent = Some(Intent::Save(
                        source.clone(),
                        report.sha256.clone(),
                        state.excel,
                    ));
                }
            }
        });
        ui.add_space(8.0);
        if let Some(report) = &state.report {
            let label = if report.errors > 0 {
                "Issues found"
            } else if report.reviews > 0 {
                "Structure consistent — review still needed"
            } else {
                "Structure and exact control totals consistent"
            };
            ui.label(RichText::new(label).strong().color(if report.errors > 0 {
                Color32::from_rgb(196, 71, 63)
            } else {
                TEAL
            }));
            ui.label(format!(
                "{} · Tax year {} · {} details · {} errors · {} review items",
                report.file_name,
                report
                    .year
                    .map(|y| y.to_string())
                    .unwrap_or_else(|| "unresolved".into()),
                report.details,
                report.errors,
                report.reviews
            ));
            ui.label(match report.employer_match{Some(true)=>"Header matches this company's registered BIR TIN/branch; agency assignment is not certified.",
                Some(false)=>"Header belongs to a different registered employer. Review the chosen file; it has not been imported.",None=>"Employer matching is unverified because company BIR registration is incomplete."});
            ui.add(
                egui::Label::new(
                    RichText::new(format!("Source SHA256: {}", report.sha256)).monospace(),
                )
                .wrap(),
            );
            ui.add_space(8.0);
            egui::CollapsingHeader::new("Exact Schedule 1 control totals")
                .default_open(state.expand_totals)
                .show(ui, |ui| {
                    let field_width = (ui.available_width() * 0.37).clamp(140.0, 330.0);
                    egui::Grid::new("bir_control_totals")
                        .striped(true)
                        .max_col_width(field_width)
                        .spacing(Vec2::new(18.0, 10.0))
                        .show(ui, |ui| {
                            ui.strong("Field");
                            ui.strong("Detail sum (PHP)");
                            ui.strong("Control (PHP)");
                            ui.strong("Check");
                            ui.end_row();
                            for t in &report.totals {
                                ui.add(egui::Label::new(&t.field).wrap());
                                ui.label(
                                    t.detail_cents
                                        .map(crate::payroll_government::decimal)
                                        .unwrap_or_else(|| "Unavailable".into()),
                                );
                                ui.label(
                                    t.control_cents
                                        .map(crate::payroll_government::decimal)
                                        .unwrap_or_else(|| "Unavailable".into()),
                                );
                                ui.label(
                                    if t.detail_cents.is_none() || t.control_cents.is_none() {
                                        "Unresolved"
                                    } else if t.detail_cents == t.control_cents {
                                        "Matches"
                                    } else {
                                        "Differs"
                                    },
                                );
                                ui.end_row();
                            }
                        });
                });
            let pages = report.issues.len().div_ceil(10).max(1);
            state.page = state.page.min(pages - 1);
            if !report.issues.is_empty() {
                ui.horizontal_wrapped(|ui| {
                    if ui
                        .add_enabled(state.page > 0, egui::Button::new("Previous"))
                        .clicked()
                    {
                        state.page -= 1;
                    }
                    ui.label(format!("Findings page {} of {}", state.page + 1, pages));
                    if ui
                        .add_enabled(state.page + 1 < pages, egui::Button::new("Next"))
                        .clicked()
                    {
                        state.page += 1;
                    }
                });
                for issue in report.issues.iter().skip(state.page * 10).take(10) {
                    ui.group(|ui| {
                        ui.horizontal_wrapped(|ui| {
                            ui.strong(if issue.line == 0 {
                                "File/control totals".into()
                            } else {
                                format!("Line {}", issue.line)
                            });
                            ui.strong(&issue.field);
                            ui.label(if issue.severity == Severity::Error {
                                "Error"
                            } else {
                                "Review"
                            });
                        });
                        ui.add(egui::Label::new(&issue.message).wrap());
                    });
                }
            }
            if report.omitted_issues() > 0 {
                ui.label(format!("{} further diagnostics are omitted. Fix the displayed issues and recheck; the file has not passed.",report.omitted_issues()));
            }
        } else {
            ui.label(if self.busy{"Checking the local file and current company access…"}else{"Choose an unvalidated .dat file. Findings stay in memory and omit employee names and identifiers from the saved diagnostic list."});
        }
        ui.add_space(12.0);
        ui.add(egui::Label::new(RichText::new("Official module compatibility remains unverified. This check follows the published 49-column D1 / 36-column C1 layout, including PERA. Always run the official BIR module before any filing.").weak()).wrap());
        match intent {
            Some(Intent::Back) => {
                self.payroll_state.government.bir_open = false;
                self.payroll_state.government.bir = State::default();
            }
            Some(Intent::Choose) => match choose_source() {
                Ok(Some(source)) => self.check_bir_source(source),
                Ok(None) => {}
                Err(e) => self.notice = Some((e.to_string(), true)),
            },
            Some(Intent::Check(source)) => self.check_bir_source(source),
            Some(Intent::Save(source, hash, excel)) => {
                match crate::reports::choose_export(if excel {
                    "BIR-1604C-findings.xlsx"
                } else {
                    "BIR-1604C-findings.csv"
                }) {
                    Ok(Some(destination)) => {
                        self.payroll_state.government.bir.pending = true;
                        self.send(Command::GovernmentBirCheck {
                            source,
                            expected_hash: Some(hash),
                            destination: Some(destination),
                        });
                    }
                    Ok(None) => {}
                    Err(e) => self.notice = Some((e.to_string(), true)),
                }
            }
            None => {}
        }
    }
    fn check_bir_source(&mut self, source: PathBuf) {
        let state = &mut self.payroll_state.government.bir;
        state.source = Some(source.clone());
        state.report = None;
        state.page = 0;
        state.pending = true;
        self.send(Command::GovernmentBirCheck {
            source,
            expected_hash: None,
            destination: None,
        });
    }
}

#[cfg(windows)]
fn choose_source() -> anyhow::Result<Option<PathBuf>> {
    use anyhow::ensure;
    use std::{ffi::OsString, os::windows::ffi::OsStringExt};
    use windows_sys::Win32::UI::{Controls::Dialogs::*, Input::KeyboardAndMouse::GetActiveWindow};
    let mut path = vec![0u16; 32768];
    let filter: Vec<u16> = "BIR plaintext source (*.dat)\0*.dat\0\0"
        .encode_utf16()
        .collect();
    let title: Vec<u16> = "Check unvalidated BIR1604C Schedule 1 source\0"
        .encode_utf16()
        .collect();
    // SAFETY: these buffers outlive the synchronous native file chooser.
    let mut d: OPENFILENAMEW = unsafe { std::mem::zeroed() };
    d.lStructSize = std::mem::size_of::<OPENFILENAMEW>() as u32;
    d.hwndOwner = unsafe { GetActiveWindow() };
    d.lpstrFilter = filter.as_ptr();
    d.lpstrTitle = title.as_ptr();
    d.lpstrFile = path.as_mut_ptr();
    d.nMaxFile = path.len() as u32;
    d.Flags = OFN_EXPLORER
        | OFN_FILEMUSTEXIST
        | OFN_PATHMUSTEXIST
        | OFN_NOCHANGEDIR
        | OFN_DONTADDTORECENT;
    if unsafe { GetOpenFileNameW(&mut d) } != 0 {
        let n = path.iter().position(|&x| x == 0).unwrap_or(path.len());
        Ok(Some(PathBuf::from(OsString::from_wide(&path[..n]))))
    } else {
        ensure!(
            unsafe { CommDlgExtendedError() } == 0,
            "Could not open the BIR source chooser."
        );
        Ok(None)
    }
}
#[cfg(not(windows))]
fn choose_source() -> anyhow::Result<Option<PathBuf>> {
    anyhow::bail!("Native source selection is available on Windows.")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn private_results_require_the_same_company_and_user_and_hr_role() {
        let mut data = DemoStore::new().data;
        let make = || bir::Report {
            company_id: data.company.id,
            user_id: data.access.user_id,
            sha256: "a".repeat(64),
            file_name: "fictional.dat".into(),
            year: Some(2026),
            details: 1,
            employer_match: None,
            errors: 0,
            reviews: 0,
            issues: Vec::new(),
            totals: Vec::new(),
        };
        let mut s = State::default();
        let r = make();
        s.accept(r.clone(), &data);
        assert!(s.report.is_some());
        s.report = None;
        let mut foreign = r.clone();
        foreign.company_id = Uuid::new_v4();
        s.accept(foreign, &data);
        assert!(s.report.is_none());
        let mut foreign = r.clone();
        foreign.user_id = Uuid::new_v4();
        s.accept(foreign, &data);
        assert!(s.report.is_none());
        data.access.role = Role::Manager;
        s.accept(r, &data);
        assert!(s.report.is_none());
    }
    #[test]
    fn a_failed_own_operation_clears_stale_findings_but_unrelated_errors_preserve_them() {
        let data = DemoStore::new().data;
        let r = bir::Report {
            company_id: data.company.id,
            user_id: data.access.user_id,
            sha256: "a".repeat(64),
            file_name: "fictional.dat".into(),
            year: Some(2026),
            details: 1,
            employer_match: None,
            errors: 0,
            reviews: 0,
            issues: Vec::new(),
            totals: Vec::new(),
        };
        let mut s = State {
            source: Some(PathBuf::from("retained-source.dat")),
            report: Some(r),
            ..Default::default()
        };
        s.failed();
        assert!(s.report.is_some());
        s.pending = true;
        s.failed();
        assert!(s.report.is_none() && s.source.is_some() && !s.pending);
    }
}
