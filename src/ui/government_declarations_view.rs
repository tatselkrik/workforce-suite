use super::*;
use crate::payroll_government::{bir_declarations::Report, decimal};
use chrono::Datelike;
use std::path::PathBuf;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::payroll_government::{
        bir_declarations,
        bir_identity::{self, tests as fixture},
    };
    #[test]
    fn declaration_view_rejects_wrong_scope_and_clears_failed_private_results() {
        let mut data = DemoStore::new().data;
        data.access.user_id = Uuid::from_u128(4);
        let report = bir_declarations::review(
            bir_identity::Basis::from_wire(fixture::wire(), data.company.id, 2026).unwrap(),
            &fixture::source(&["234567890"]),
            fixture::FILE_NAME,
            "2026-10-26".parse().unwrap(),
            data.access.user_id,
        )
        .unwrap();
        for mode in 0..5 {
            let mut snapshot = data.clone();
            let mut state = State {
                year: 2026,
                ..Default::default()
            };
            match mode {
                0 => snapshot.access.role = Role::Manager,
                1 => snapshot.access.role = Role::Employee,
                2 => snapshot.company.id = Uuid::new_v4(),
                3 => snapshot.access.user_id = Uuid::new_v4(),
                _ => state.year = 2025,
            }
            state.accept(report.clone(), &snapshot);
            assert!(state.report.is_none());
        }
        let mut state = State {
            year: 2026,
            ..Default::default()
        };
        state.accept(report, &data);
        assert!(state.report.is_some());
        state.pending = true;
        state.failed();
        assert!(state.report.is_none());
    }
}

#[derive(Default)]
pub(super) struct State {
    year: i32,
    source: Option<PathBuf>,
    report: Option<Report>,
    selected: usize,
    source_page: usize,
    excel: bool,
    pending: bool,
}
impl State {
    #[cfg(feature = "qa-screenshot")]
    pub(super) fn preview(kind: &str) -> Self {
        use crate::payroll_government::{annual, bir_file as bir, bir_identity as bi};
        let mut ledger = annual::fixture();
        let mwe = kind.contains("mwe");
        let unmatched = kind.contains("unmatched");
        if kind.contains("unknown") {
            ledger.wire.entries[0].fact.parts.employee_cents = None;
        }
        let row = bi::Row {
            line: 2,
            status: if unmatched {
                bi::Status::Ambiguous
            } else {
                bi::Status::CurrentPairMatched
            },
            employee: (!unmatched).then(|| ledger.wire.employees[0].clone()),
            candidates: if unmatched { 2 } else { 1 },
            observed_sources: (!unmatched).then_some(7),
            received_sources: (!unmatched).then_some(3),
        };
        let checked = bir::preview_report(false, mwe);
        let columns = if mwe {
            &bir::MWE_MONEY_COLUMNS[..]
        } else {
            &bir::MONEY_COLUMNS[..]
        };
        let declared = checked
            .totals
            .into_iter()
            .zip(columns)
            .map(|(t, c)| bir::DeclaredAmount {
                column: *c + 1,
                field: t.field,
                cents: t.detail_cents.unwrap(),
            })
            .collect();
        let header = bi::Report {
            company_id: Uuid::from_u128(1),
            user_id: Uuid::from_u128(1),
            year: 2026,
            as_of: ledger.wire.as_of,
            generated_at: ledger.wire.generated_at,
            source_sha256: "b".repeat(64),
            file_name: "Fictional source.dat".into(),
            schedule: checked.schedule,
            preflight_reviews: usize::from(mwe),
            roster_people: 12,
            usable_registrations: 9,
            received_people_not_uniquely_matched: usize::from(unmatched),
            rows: vec![row.clone()],
        };
        Self {
            year: 2026,
            source: Some("Fictional source.dat".into()),
            report: Some(Report {
                identity: header,
                rows: vec![crate::payroll_government::bir_declarations::Row {
                    identity: row,
                    declared,
                    sources: if unmatched {
                        Vec::new()
                    } else {
                        ledger.wire.entries
                    },
                }],
                sources_without_unique_file_match: if unmatched { 7 } else { 0 },
            }),
            ..Default::default()
        }
    }
    pub(super) fn accept(&mut self, r: Report, data: &Snapshot) {
        let identity = &r.identity;
        if data
            .access
            .allows(crate::staff_roles::Capability::GovernmentPreparation)
            && identity.company_id == data.company.id
            && identity.user_id == data.access.user_id
            && identity.year == self.year
        {
            self.report = Some(r);
            self.selected = 0;
            self.source_page = 0;
            self.pending = false;
        }
    }
    pub(super) fn failed(&mut self) {
        if self.pending {
            self.report = None;
            self.pending = false;
        }
    }
}
enum Intent {
    Choose,
    Check(PathBuf),
    Export(PathBuf, String, bool),
}

impl HrisApp {
    pub(super) fn payroll_bir_declarations(&mut self, ui: &mut egui::Ui) {
        let state = &mut self.payroll_state.government.declarations;
        if state.year == 0 {
            state.year = self.data.company.today().year();
        }
        let connected = self.data.mode == Mode::Supabase;
        let mut intent = None;
        let mut back = false;
        ui.heading("BIR declarations and recorded sources");
        ui.add(egui::Label::new("Review literal amounts from a local file beside payroll sources for its unique current employee match. These are separate records: no expected return amount or tax difference is calculated.").wrap());
        ui.add(egui::Label::new(RichText::new("Current registration matches do not certify historical identity, income classification, MWE/PERA eligibility, 2316 certificates or official validation. The file and payroll stay unchanged.").weak()).wrap());
        ui.horizontal_wrapped(|ui| {
            back = ui
                .add_enabled(!self.busy, egui::Button::new("Back to reconciliation"))
                .clicked();
            ui.add_enabled_ui(!self.busy, |ui| {
                let old = state.year;
                egui::ComboBox::from_id_salt("bir_decl_year")
                    .selected_text(state.year.to_string())
                    .show_ui(ui, |ui| {
                        for y in (2025..=self.data.company.today().year().min(2200)).rev() {
                            ui.selectable_value(&mut state.year, y, y.to_string());
                        }
                    });
                if state.year != old {
                    state.report = None;
                    state.selected = 0;
                    state.source_page = 0;
                }
                egui::ComboBox::from_id_salt("bir_decl_format")
                    .selected_text(if state.excel {
                        "Excel worksheet"
                    } else {
                        "CSV worksheet"
                    })
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut state.excel, false, "CSV worksheet");
                        ui.selectable_value(&mut state.excel, true, "Excel worksheet");
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
                    egui::Button::new("Refresh declarations and sources"),
                )
                .clicked()
            {
                intent = state.source.clone().map(Intent::Check);
            }
            if ui
                .add_enabled(
                    !self.busy && connected && state.report.is_some(),
                    egui::Button::new("Export fresh internal worksheet"),
                )
                .clicked()
            {
                if let (Some(path), Some(r)) = (&state.source, &state.report) {
                    intent = Some(Intent::Export(
                        path.clone(),
                        r.identity.source_sha256.clone(),
                        state.excel,
                    ));
                }
            }
        });
        if let Some(report) = &state.report {
            let identity = &report.identity;
            ui.add(egui::Label::new(format!("{} · year {} · current snapshot {} · {} declarations · {} observed sources not uniquely linked in this file",identity.schedule.label(),identity.year,identity.generated_at.format("%Y-%m-%d %H:%M UTC"),report.rows.len(),report.sources_without_unique_file_match)).wrap());
            ui.add(egui::Label::new(format!("Source SHA256: {}", identity.source_sha256)).wrap());
            if identity.preflight_reviews > 0 {
                ui.add(egui::Label::new(format!("{} preflight review items remain unresolved, including any encoding/MWE basis checks.",identity.preflight_reviews)).wrap());
            }
            if !report.rows.is_empty() {
                state.selected = state.selected.min(report.rows.len() - 1);
                let previous = state.selected;
                egui::ComboBox::from_id_salt("bir_decl_line")
                    .selected_text(row_label(&report.rows[state.selected]))
                    .show_ui(ui, |ui| {
                        for (n, row) in report.rows.iter().enumerate() {
                            ui.selectable_value(&mut state.selected, n, row_label(row));
                        }
                    });
                if state.selected != previous {
                    state.source_page = 0;
                }
                let row = &report.rows[state.selected];
                ui.add(egui::Label::new(row.identity.status.label()).wrap());
                if ui.available_width() >= 850.0 {
                    ui.columns(2, |columns| {
                        declared(&mut columns[0], row);
                        sources(&mut columns[1], row, &mut state.source_page);
                    });
                } else {
                    declared(ui, row);
                    ui.add_space(12.0);
                    sources(ui, row, &mut state.source_page);
                }
            }
        } else {
            ui.add(egui::Label::new("Choose the file's year and a standalone Schedule 1 or 2 source. Complete current company BIR registration and resolve whole-file structure/control errors first. This private worksheet contains amounts; the separate minimal diagnostics and identity exports remain available.").wrap());
        }
        if back {
            self.payroll_state.government.declarations_open = false;
            return;
        }
        let year = state.year;
        match intent {
            Some(Intent::Choose) => match super::bir_file_view::choose_source() {
                Ok(Some(path)) => {
                    state.source = Some(path.clone());
                    state.report = None;
                    state.pending = true;
                    self.send(Command::GovernmentBirDeclarations {
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
                self.send(Command::GovernmentBirDeclarations {
                    year,
                    source,
                    expected_hash: None,
                    destination: None,
                });
            }
            Some(Intent::Export(source, hash, excel)) => {
                match crate::reports::choose_export(&format!(
                    "HRIS-BIR-declarations-recorded-sources-{year}.{}",
                    if excel { "xlsx" } else { "csv" }
                )) {
                    Ok(Some(path)) => {
                        state.pending = true;
                        self.send(Command::GovernmentBirDeclarations {
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
fn row_label(row: &crate::payroll_government::bir_declarations::Row) -> String {
    format!(
        "Line {} · {}",
        row.identity.line,
        row.identity
            .employee
            .as_ref()
            .map(|e| format!("{} · {}", e.employee_code, e.employee_name))
            .unwrap_or_else(|| row.identity.status.label().into())
    )
}
fn declared(ui: &mut egui::Ui, row: &crate::payroll_government::bir_declarations::Row) {
    ui.heading("File-declared amounts");
    ui.small("Literal PHP values; not certified or recomputed tax.");
    let field_width = (ui.available_width() - 110.0).max(160.0);
    egui::Grid::new(("bir_declared_fields", row.identity.line))
        .num_columns(2)
        .striped(true)
        .spacing(egui::vec2(14.0, 8.0))
        .show(ui, |ui| {
            for d in &row.declared {
                let text = format!("{} · {}", d.column, d.field);
                let font = egui::TextStyle::Body.resolve(ui.style());
                let height = ui
                    .fonts_mut(|fonts| {
                        fonts
                            .layout(text.clone(), font, ui.visuals().text_color(), field_width)
                            .size()
                            .y
                    })
                    .max(ui.text_style_height(&egui::TextStyle::Monospace));
                ui.allocate_ui_with_layout(
                    egui::vec2(field_width, height),
                    egui::Layout::left_to_right(egui::Align::Center),
                    |ui| {
                        ui.add(egui::Label::new(text).wrap());
                    },
                );
                ui.allocate_ui_with_layout(
                    egui::vec2(90.0, height),
                    egui::Layout::right_to_left(egui::Align::Center),
                    |ui| {
                        ui.monospace(decimal(d.cents));
                    },
                );
                ui.end_row();
            }
        });
}
fn sources(
    ui: &mut egui::Ui,
    row: &crate::payroll_government::bir_declarations::Row,
    page: &mut usize,
) {
    use crate::payroll_government::State;
    ui.heading("Recorded payroll sources");
    ui.small("These records are not assigned to any file-declaration column.");
    if row.sources.is_empty() {
        ui.add(
            egui::Label::new(if row.identity.employee.is_some() {
                "No sources observed for this current match. This does not establish zero tax."
            } else {
                "No unique current employee was selected. No payroll sources were linked."
            })
            .wrap(),
        );
        return;
    }
    let pages = row.sources.len().div_ceil(10);
    *page = (*page).min(pages - 1);
    ui.horizontal_wrapped(|ui| {
        if ui
            .add_enabled(*page > 0, egui::Button::new("Previous sources"))
            .clicked()
        {
            *page -= 1;
        }
        ui.label(format!(
            "{}–{} of {}",
            *page * 10 + 1,
            ((*page + 1) * 10).min(row.sources.len()),
            row.sources.len()
        ));
        if ui
            .add_enabled(*page + 1 < pages, egui::Button::new("Next sources"))
            .clicked()
        {
            *page += 1;
        }
    });
    for e in row.sources.iter().skip(*page * 10).take(10) {
        let f = &e.fact;
        egui::Frame::group(ui.style()).show(ui, |ui| {
            ui.strong(format!("{} · {}", f.kind.label(), f.state.label()));
            ui.add(
                egui::Label::new(format!(
                    "Service {} to {} · receipt {}",
                    f.starts_on,
                    f.ends_on,
                    f.received_on
                        .map(|d| d.to_string())
                        .unwrap_or_else(|| "Missing".into())
                ))
                .wrap(),
            );
            ui.label(format!(
                "Recorded withholding: {}",
                f.parts
                    .employee_cents
                    .map(|n| format!("PHP {}", decimal(n)))
                    .unwrap_or_else(|| "Unknown".into())
            ));
            ui.add(egui::Label::new(e.treatment()).wrap());
            if f.state != State::Received {
                ui.small("Excluded from actual receipts in the selected year.");
            }
            ui.add(egui::Label::new(format!("Source {}", f.id)).wrap());
        });
    }
}
