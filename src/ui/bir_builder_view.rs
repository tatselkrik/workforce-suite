use super::*;
use crate::payroll_government::bir_file::{
    Schedule,
    builder::{Action, Preview, Selection},
};
use std::path::PathBuf;

pub(super) struct State {
    selection: Selection,
    source: Option<PathBuf>,
    report: Option<Preview>,
    excel: bool,
    reviewed: bool,
    pending: bool,
    totals_open: bool,
}
impl Default for State {
    fn default() -> Self {
        Self::new(0)
    }
}
impl State {
    pub(super) fn new(year: i32) -> Self {
        Self {
            selection: Selection {
                schedule: Schedule::One,
                year,
            },
            source: None,
            report: None,
            excel: true,
            reviewed: false,
            pending: false,
            totals_open: false,
        }
    }
    pub(super) fn accept(&mut self, report: Preview, data: &Snapshot) {
        if data
            .access
            .allows(crate::staff_roles::Capability::GovernmentPreparation)
            && report.company_id == data.company.id
            && report.user_id == data.access.user_id
            && report.selection == self.selection
            && self.source.is_some()
        {
            self.report = Some(report);
            self.pending = false;
            self.reviewed = false;
        }
    }
    pub(super) fn failed(&mut self) {
        if self.pending {
            self.report = None;
            self.reviewed = false;
            self.pending = false;
        }
    }
    fn invalidate(&mut self) {
        self.report = None;
        self.reviewed = false;
    }
    #[cfg(feature = "qa-screenshot")]
    pub(super) fn preview(data: &Snapshot, scene: &str) -> Self {
        use crate::payroll_government::bir_file as bir;
        let mut state = Self::new(2026);
        if scene.contains("mwe") {
            state.selection.schedule = Schedule::Two;
        }
        if !scene.contains("empty") {
            state.source = Some(PathBuf::from("Fictional reviewed declarations.xlsx"));
            let (fields, columns) = if state.selection.schedule == Schedule::One {
                (&bir::FIELDS[..], &bir::MONEY_COLUMNS[..])
            } else {
                (&bir::MWE_FIELDS[..], &bir::MWE_MONEY_COLUMNS[..])
            };
            state.report = Some(Preview {
                selection: state.selection,
                company_id: data.company.id,
                user_id: data.access.user_id,
                source_sha256: "a".repeat(64),
                output_sha256: "b".repeat(64),
                basis_sha256: "c".repeat(64),
                file_name: "0123456780000123120261604C.DAT".into(),
                details: 3,
                controls: columns.len(),
                review_items: usize::from(state.selection.schedule == Schedule::Two),
                totals: columns
                    .iter()
                    .map(|&i| bir::Total {
                        field: fields[i].into(),
                        detail_cents: Some(1234567),
                        control_cents: Some(1234567),
                    })
                    .collect(),
            });
        }
        state.totals_open = scene.contains("totals");
        state
    }
}
enum Intent {
    Back,
    Template,
    Choose,
    Preview,
    Save,
}

impl HrisApp {
    pub(super) fn payroll_bir_builder(&mut self, ui: &mut Ui) {
        if !self
            .data
            .access
            .allows(crate::staff_roles::Capability::GovernmentPreparation)
        {
            ui.label("BIR drafts require current HR/Administrator access.");
            return;
        }
        let connected = self.data.mode == Mode::Supabase;
        let state = &mut self.payroll_state.government.builder;
        let mut intent = None;
        ui.heading("Build BIR 1604C draft");
        ui.add(egui::Label::new("Download a declaration template, enter explicitly reviewed values, then preview and save a new plaintext DAT draft. HRIS assembles the registered employer header, row sequences and exact control totals.").wrap());
        ui.add(egui::Label::new(RichText::new("Tax, exemptions, prior-employer amounts, MWE eligibility and filing declarations are not inferred or reconciled with payroll. Identities and agency assignments are not certified. Official BIR validation and agency acceptance remain required. No file is uploaded or submitted.").weak()).wrap());
        ui.add_space(8.0);
        ui.horizontal_wrapped(|ui| {
            if ui
                .add_enabled(!self.busy, egui::Button::new("Back to reconciliation"))
                .clicked()
            {
                intent = Some(Intent::Back);
            }
            let before = state.selection;
            ui.add_enabled_ui(!self.busy, |ui| {
                egui::ComboBox::from_id_salt("bir_builder_schedule")
                    .selected_text(state.selection.schedule.label())
                    .show_ui(ui, |ui| {
                        ui.selectable_value(
                            &mut state.selection.schedule,
                            Schedule::One,
                            "Schedule 1",
                        );
                        ui.selectable_value(
                            &mut state.selection.schedule,
                            Schedule::Two,
                            "Schedule 2 — MWE",
                        );
                    });
                egui::ComboBox::from_id_salt("bir_builder_year")
                    .selected_text(format!("Tax year {}", state.selection.year))
                    .show_ui(ui, |ui| {
                        for year in (2023..=self.data.company.today().year()).rev() {
                            ui.selectable_value(&mut state.selection.year, year, year.to_string());
                        }
                    });
                egui::ComboBox::from_id_salt("bir_builder_template_format")
                    .selected_text(if state.excel {
                        "Excel template"
                    } else {
                        "CSV template"
                    })
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut state.excel, true, "Excel template");
                        ui.selectable_value(&mut state.excel, false, "CSV template");
                    });
            });
            if before != state.selection {
                state.invalidate();
            }
        });
        ui.horizontal_wrapped(|ui| {
            if ui
                .add_enabled(
                    !self.busy && connected,
                    egui::Button::new("Save blank template"),
                )
                .clicked()
            {
                intent = Some(Intent::Template);
            }
            if ui
                .add_enabled(
                    !self.busy && connected,
                    egui::Button::new("Choose declarations CSV/Excel"),
                )
                .clicked()
            {
                intent = Some(Intent::Choose);
            }
            if ui
                .add_enabled(
                    !self.busy && connected && state.source.is_some(),
                    egui::Button::new("Preview / refresh draft"),
                )
                .clicked()
            {
                intent = Some(Intent::Preview);
            }
        });
        ui.add_space(8.0);
        if let Some(source) = &state.source {
            ui.add(
                egui::Label::new(format!(
                    "Declaration source: {}",
                    source.file_name().unwrap_or_default().to_string_lossy()
                ))
                .wrap(),
            );
        }
        if let Some(report) = &state.report {
            ui.group(|ui| {
                ui.strong("Published-layout draft — official validation pending");
                ui.add(egui::Label::new(format!("{} · {} reviewed detail rows · {} exact controls",report.selection.schedule.label(),report.details,report.controls)).wrap());
                ui.add(egui::Label::new(format!("Output: {}",report.file_name)).wrap());
                ui.add(egui::Label::new("The source file is unchanged. The DAT draft contains declared tax identifiers and pay amounts; minimal diagnostic findings remain separate.").wrap());
                if report.selection.schedule==Schedule::Two {
                    ui.add(egui::Label::new("Minimum-wage eligibility, wage/factor bases and encoding compatibility remain unverified.").wrap());
                }
                egui::CollapsingHeader::new("Declared control totals").default_open(state.totals_open).show(ui,|ui| {
                    let field_width=(ui.available_width()*0.6).clamp(140.0,400.0);
                    egui::Grid::new("bir_builder_controls").num_columns(2).max_col_width(field_width).striped(true).show(ui,|ui| {
                        ui.strong("Declared field");ui.strong("PHP");ui.end_row();
                        for total in &report.totals {
                            ui.add(egui::Label::new(&total.field).wrap().halign(Align::Min));
                            ui.label(total.control_cents.map(crate::payroll_government::decimal).unwrap_or_else(||"Unknown".into()));
                            ui.end_row();
                        }
                    });
                });
                egui::CollapsingHeader::new("Source and draft fingerprints").show(ui,|ui| {
                    ui.add(egui::Label::new(format!("Source SHA256: {}",report.source_sha256)).wrap());
                    ui.add(egui::Label::new(format!("DAT SHA256: {}",report.output_sha256)).wrap());
                });
            });
            ui.add_enabled_ui(!self.busy, |ui| {
                ui.checkbox(
                    &mut state.reviewed,
                    "I reviewed these declarations; official BIR validation is still required.",
                );
            });
            if ui
                .add_enabled(
                    !self.busy && connected && state.reviewed,
                    egui::Button::new("Save DAT draft"),
                )
                .clicked()
            {
                intent = Some(Intent::Save);
            }
        } else {
            ui.add(egui::Label::new(if self.busy{"Preparing the local draft and checking current company access…"}else{"Templates contain no sample people or prefilled monetary declarations. Enter explicit zero where appropriate; blank amounts are not zero."}).wrap());
        }
        match intent {
            Some(Intent::Back) => {
                self.payroll_state.government.builder_open = false;
                self.payroll_state.government.builder = State::default();
            }
            Some(Intent::Template) => {
                let state = &self.payroll_state.government.builder;
                let name = format!(
                    "BIR-Schedule-{}-declarations.{}",
                    if state.selection.schedule == Schedule::One {
                        1
                    } else {
                        2
                    },
                    if state.excel { "xlsx" } else { "csv" }
                );
                match crate::reports::choose_export(&name) {
                    Ok(Some(destination)) => {
                        self.send(Command::GovernmentBirBuild(Action::Template {
                            schedule: state.selection.schedule,
                            destination,
                        }))
                    }
                    Ok(None) => {}
                    Err(e) => self.notice = Some((e.to_string(), true)),
                }
            }
            Some(Intent::Choose) => {
                match crate::imports::choose_tabular_source("Choose reviewed BIR declarations") {
                    Ok(Some(source)) => {
                        let state = &mut self.payroll_state.government.builder;
                        state.source = Some(source);
                        state.invalidate();
                        self.preview_bir_draft();
                    }
                    Ok(None) => {}
                    Err(e) => self.notice = Some((e.to_string(), true)),
                }
            }
            Some(Intent::Preview) => self.preview_bir_draft(),
            Some(Intent::Save) => {
                let state = &mut self.payroll_state.government.builder;
                if let (Some(source), Some(report)) = (&state.source, &state.report) {
                    match choose_dat_destination(&report.file_name) {
                        Ok(Some(destination)) => {
                            let action = Action::Prepare {
                                selection: state.selection,
                                source: source.clone(),
                                expected_basis: Some(report.basis_sha256.clone()),
                                destination: Some(destination),
                                reviewed: state.reviewed,
                            };
                            state.pending = true;
                            self.send(Command::GovernmentBirBuild(action));
                        }
                        Ok(None) => {}
                        Err(e) => self.notice = Some((e.to_string(), true)),
                    }
                }
            }
            None => {}
        }
    }
    fn preview_bir_draft(&mut self) {
        let state = &mut self.payroll_state.government.builder;
        if let Some(source) = state.source.clone() {
            state.invalidate();
            state.pending = true;
            let action = Action::Prepare {
                selection: state.selection,
                source,
                expected_basis: None,
                destination: None,
                reviewed: false,
            };
            self.send(Command::GovernmentBirBuild(action));
        }
    }
}

#[cfg(windows)]
fn choose_dat_destination(filename: &str) -> anyhow::Result<Option<PathBuf>> {
    use anyhow::ensure;
    use std::{ffi::OsString, os::windows::ffi::OsStringExt};
    use windows_sys::Win32::UI::{Controls::Dialogs::*, Input::KeyboardAndMouse::GetActiveWindow};
    let mut path = vec![0u16; 32768];
    let name: Vec<u16> = filename.encode_utf16().collect();
    ensure!(name.len() < path.len(), "BIR draft filename is too long.");
    path[..name.len()].copy_from_slice(&name);
    let filter: Vec<u16> = "BIR plaintext draft (*.dat)\0*.dat\0\0"
        .encode_utf16()
        .collect();
    let title: Vec<u16> = "Save new BIR draft — existing files are preserved\0"
        .encode_utf16()
        .collect();
    let extension: Vec<u16> = "dat\0".encode_utf16().collect();
    // SAFETY: the buffers remain live for this synchronous native dialog.
    let mut dialog: OPENFILENAMEW = unsafe { std::mem::zeroed() };
    dialog.lStructSize = std::mem::size_of::<OPENFILENAMEW>() as u32;
    dialog.hwndOwner = unsafe { GetActiveWindow() };
    dialog.lpstrFilter = filter.as_ptr();
    dialog.lpstrTitle = title.as_ptr();
    dialog.lpstrDefExt = extension.as_ptr();
    dialog.lpstrFile = path.as_mut_ptr();
    dialog.nMaxFile = path.len() as u32;
    dialog.Flags = OFN_EXPLORER
        | OFN_PATHMUSTEXIST
        | OFN_NOCHANGEDIR
        | OFN_DONTADDTORECENT
        | OFN_NOREADONLYRETURN;
    if unsafe { GetSaveFileNameW(&mut dialog) } != 0 {
        let n = path.iter().position(|&c| c == 0).unwrap_or(path.len());
        Ok(Some(PathBuf::from(OsString::from_wide(&path[..n]))))
    } else {
        ensure!(
            unsafe { CommDlgExtendedError() } == 0,
            "Could not open the BIR draft destination chooser."
        );
        Ok(None)
    }
}
#[cfg(not(windows))]
fn choose_dat_destination(_filename: &str) -> anyhow::Result<Option<PathBuf>> {
    anyhow::bail!("Native BIR draft saving is available on Windows.")
}

#[cfg(test)]
mod tests {
    use super::*;
    fn report(data: &Snapshot) -> Preview {
        Preview {
            selection: Selection {
                schedule: Schedule::One,
                year: 2026,
            },
            company_id: data.company.id,
            user_id: data.access.user_id,
            source_sha256: "a".repeat(64),
            output_sha256: "b".repeat(64),
            basis_sha256: "c".repeat(64),
            file_name: "fictional.DAT".into(),
            details: 1,
            controls: 31,
            review_items: 0,
            totals: vec![],
        }
    }
    #[test]
    fn builder_screen_accepts_only_current_actor_company_hr_selection_and_selected_source() {
        let mut data = DemoStore::new().data;
        data.company.id = Uuid::from_u128(1);
        data.access.user_id = Uuid::from_u128(2);
        let good = report(&data);
        let mut state = State::new(2026);
        state.accept(good.clone(), &data);
        assert!(state.report.is_none());
        state.source = Some(PathBuf::from("fictional.csv"));
        state.accept(good.clone(), &data);
        assert!(state.report.is_some());
        for role in [Role::Manager, Role::Employee] {
            state.invalidate();
            data.access.role = role;
            state.accept(good.clone(), &data);
            assert!(state.report.is_none());
        }
        data.access.role = Role::Administrator;
        for kind in 0..4 {
            let mut changed = good.clone();
            match kind {
                0 => changed.user_id = Uuid::from_u128(8),
                1 => changed.company_id = Uuid::from_u128(9),
                2 => changed.selection.year = 2025,
                _ => changed.selection.schedule = Schedule::Two,
            }
            state.accept(changed, &data);
            assert!(state.report.is_none());
        }
    }
    #[test]
    fn builder_failed_request_or_changed_selection_clears_review_and_preview() {
        let data = DemoStore::new().data;
        let mut state = State::new(2026);
        state.source = Some(PathBuf::from("fictional.csv"));
        state.accept(report(&data), &data);
        state.reviewed = true;
        state.pending = true;
        state.failed();
        assert!(state.report.is_none() && !state.reviewed && !state.pending);
        state.accept(report(&data), &data);
        state.reviewed = true;
        state.invalidate();
        assert!(state.report.is_none() && !state.reviewed);
    }
}
