use super::*;
use crate::payroll_government::{self as g, sss_collection as collection, sss_r3::Remark};
use anyhow::{Context, Result};

pub(super) struct State {
    pub(super) basis: collection::Basis,
    pub(super) fresh: Option<collection::Basis>,
    employer_name: String,
    rows: Vec<Row>,
    reviewed: bool,
    page: usize,
    error: Option<String>,
}
struct Row {
    id: Uuid,
    last_name: String,
    first_name: String,
    initial: String,
    remark: Option<Remark>,
    date: String,
}
enum Intent {
    Close,
    Refresh,
    Replace,
    Export(Box<collection::Request>),
}
impl State {
    #[cfg(feature = "qa-screenshot")]
    pub(super) fn preview(basis: collection::Basis, review: bool) -> Self {
        let mut s = Self::new(basis);
        if review {
            s.rows[0].initial = "M".into();
            s.rows[0].remark = Some(Remark::NewHire);
            s.rows[0].date = "2026-09-01".into();
            s.reviewed = true;
        }
        s
    }
    pub(super) fn new(basis: collection::Basis) -> Self {
        let rows = basis
            .people
            .iter()
            .map(|p| Row {
                id: p.employee_id,
                last_name: if p.legal["name_suffix"].is_empty() {
                    p.legal["last_name"].clone()
                } else {
                    format!("{} {}", p.legal["last_name"], p.legal["name_suffix"])
                },
                first_name: p.legal["first_name"].clone(),
                initial: String::new(),
                remark: None,
                date: String::new(),
            })
            .collect();
        Self {
            employer_name: basis.employer_name.clone(),
            basis,
            fresh: None,
            rows,
            reviewed: false,
            page: 0,
            error: None,
        }
    }
    fn request(&self) -> Result<collection::Request> {
        let declarations = self
            .rows
            .iter()
            .map(|r| {
                let remark = r
                    .remark
                    .context("Choose a declaration for every employee, including other pages.")?;
                let event_on = if matches!(remark, Remark::NewHire | Remark::Separated) {
                    Some(r.date.parse().map_err(|_| {
                        anyhow::anyhow!("Use YYYY-MM-DD for each declared hire/separation date.")
                    })?)
                } else {
                    None
                };
                Ok(collection::Declaration {
                    employee_id: r.id,
                    last_name: r.last_name.clone(),
                    first_name: r.first_name.clone(),
                    middle_initial: r.initial.clone(),
                    remark,
                    event_on,
                })
            })
            .collect::<Result<Vec<_>>>()?;
        Ok(collection::Request {
            expected_fingerprint: self.basis.fingerprint.clone(),
            receipt_id: self.basis.receipt.id,
            employer_name: self.employer_name.clone(),
            declarations,
            reviewed: self.reviewed,
        })
    }
}
impl HrisApp {
    pub(super) fn payroll_sss_collection(&mut self, ui: &mut Ui) {
        let Some(mut state) = self.payroll_state.government.collection.take() else {
            return;
        };
        let filter = g::Filter {
            month: state.basis.month,
            agency: g::Agency::Sss,
        };
        let mut intent = None;
        ui.add_enabled_ui(!self.busy, |ui| {
            intent = render(ui, &mut state, self.busy, self.data.mode == Mode::Supabase);
        });
        self.payroll_state.government.collection = Some(state);
        match intent {
            Some(Intent::Close) => {
                self.payroll_state.government.collection = None;
                self.payroll_state.government.collection_export_pending = false;
            }
            Some(Intent::Refresh) => self.send(Command::GovernmentSssPrepare { filter }),
            Some(Intent::Replace) => {
                if let Some(fresh) = self
                    .payroll_state
                    .government
                    .collection
                    .as_mut()
                    .and_then(|s| s.fresh.take())
                {
                    self.payroll_state.government.collection = Some(State::new(fresh));
                }
            }
            Some(Intent::Export(request)) => match choose_reference_destination(&format!(
                "HRIS-SSS-R3-reference-{}.txt",
                filter.month
            )) {
                Ok(Some(destination)) => {
                    self.payroll_state.government.collection_export_pending = true;
                    self.send(Command::GovernmentSssExport {
                        filter,
                        request,
                        destination,
                    });
                }
                Ok(None) => {}
                Err(e) => self.notice = Some((e.to_string(), true)),
            },
            None => {}
        }
    }
}
#[cfg(windows)]
fn choose_reference_destination(filename: &str) -> Result<Option<std::path::PathBuf>> {
    use std::{ffi::OsString, os::windows::ffi::OsStringExt};
    use windows_sys::Win32::UI::{Controls::Dialogs::*, Input::KeyboardAndMouse::GetActiveWindow};
    let mut path = vec![0u16; 32768];
    for (slot, c) in path.iter_mut().zip(filename.encode_utf16()) {
        *slot = c;
    }
    let filter: Vec<u16> = "Text reference draft (*.txt)\0*.txt\0\0"
        .encode_utf16()
        .collect();
    let title: Vec<u16> = "Save SSS reference draft to a new file\0"
        .encode_utf16()
        .collect();
    let ext: Vec<u16> = "txt\0".encode_utf16().collect();
    // SAFETY: owned UTF-16 buffers live through the synchronous Windows dialog.
    let mut dialog: OPENFILENAMEW = unsafe { std::mem::zeroed() };
    dialog.lStructSize = std::mem::size_of::<OPENFILENAMEW>() as u32;
    dialog.hwndOwner = unsafe { GetActiveWindow() };
    dialog.lpstrFilter = filter.as_ptr();
    dialog.lpstrTitle = title.as_ptr();
    dialog.lpstrDefExt = ext.as_ptr();
    dialog.lpstrFile = path.as_mut_ptr();
    dialog.nMaxFile = path.len() as u32;
    dialog.Flags = OFN_EXPLORER | OFN_PATHMUSTEXIST | OFN_NOCHANGEDIR | OFN_DONTADDTORECENT;
    if unsafe { GetSaveFileNameW(&mut dialog) } != 0 {
        let len = path.iter().position(|c| *c == 0).unwrap_or(path.len());
        Ok(Some(std::path::PathBuf::from(OsString::from_wide(
            &path[..len],
        ))))
    } else {
        anyhow::ensure!(
            unsafe { CommDlgExtendedError() } == 0,
            "Could not open the reference file chooser."
        );
        Ok(None)
    }
}
#[cfg(not(windows))]
fn choose_reference_destination(_: &str) -> Result<Option<std::path::PathBuf>> {
    anyhow::bail!("Reference draft file selection currently supports Windows only.")
}
fn input(ui: &mut Ui, label: &str, value: &mut String) -> bool {
    ui.add(egui::Label::new(label).wrap());
    ui.add(egui::TextEdit::singleline(value).desired_width(ui.available_width()))
        .changed()
}
fn label(remark: Option<Remark>) -> &'static str {
    match remark {
        None => "Choose declaration",
        Some(Remark::Normal) => "N · Normal",
        Some(Remark::NewHire) => "1 · New hire",
        Some(Remark::Separated) => "2 · Separated",
        Some(Remark::NoEarnings) => "3 · No earnings",
    }
}
fn render(ui: &mut Ui, s: &mut State, busy: bool, connected: bool) -> Option<Intent> {
    let mut out = None;
    let mut changed = false;
    ui.heading("SSS R3 reference draft");
    ui.add(egui::Label::new("Prepare a local reference-format draft for HR review. My.SSS acceptance has not been verified; this does not submit a collection list or send money.").wrap());
    ui.horizontal_wrapped(|ui| {
        if ui
            .add_enabled(!busy, egui::Button::new("Discard form"))
            .clicked()
        {
            out = Some(Intent::Close);
        }
        if ui
            .add_enabled(
                !busy && connected,
                egui::Button::new("Check current sources"),
            )
            .clicked()
        {
            out = Some(Intent::Refresh);
        }
    });
    if let Some(fresh) = &s.fresh {
        ui.add(
            egui::Label::new(if fresh.fingerprint == s.basis.fingerprint {
                "Current source facts match this form. Your edits are kept."
            } else {
                "Current source facts changed. Your edits are kept; export will require a new form."
            })
            .wrap(),
        );
        if ui
            .add_enabled(
                !busy,
                egui::Button::new("Discard edits and load checked sources"),
            )
            .clicked()
        {
            out = Some(Intent::Replace);
        }
    }
    card(ui, |ui| {
        ui.label(format!(
            "Contribution month: {}",
            s.basis.month.format("%B %Y")
        ));
        ui.label(format!("Employer SSS: {}", s.basis.employer_sss));
        ui.label(format!(
            "Recorded TR/SBR: {} · Paid {}",
            s.basis.receipt.reference, s.basis.receipt.paid_on
        ));
        ui.label(format!(
            "Recorded amount: {} · {} contributing employees",
            crate::payroll::money(s.basis.receipt.amount_cents),
            s.basis.people.len()
        ));
        ui.add(
            egui::Label::new(format!(
                "Recorded legal employer name: {}",
                s.basis.employer_name
            ))
            .wrap(),
        );
        changed |= input(
            ui,
            "Employer name representation in this draft · maximum 30 ASCII characters",
            &mut s.employer_name,
        );
    });
    ui.add(egui::Label::new("Names below are file representations; registration/employment records stay unchanged. Use only representations checked against SSS documents. Unsupported widths/characters are blocked rather than truncated or transliterated. Middle initials and employment declarations are explicit.").wrap());
    if ui
        .add_enabled(
            !busy,
            egui::Button::new("Declare all contributing employees Normal"),
        )
        .clicked()
    {
        for r in &mut s.rows {
            r.remark = Some(Remark::Normal);
        }
        changed = true;
    }
    ui.label("Review new-hire/separation exceptions individually. This increment does not infer no-earnings employees from zero deductions.");
    let pages = s.rows.len().div_ceil(8).max(1);
    s.page = s.page.min(pages - 1);
    ui.horizontal(|ui| {
        if ui
            .add_enabled(s.page > 0, egui::Button::new("Previous employees"))
            .clicked()
        {
            s.page -= 1;
        }
        ui.label(format!("Page {} of {}", s.page + 1, pages));
        if ui
            .add_enabled(s.page + 1 < pages, egui::Button::new("Next employees"))
            .clicked()
        {
            s.page += 1;
        }
    });
    for r in s.rows.iter_mut().skip(s.page * 8).take(8) {
        let p = s
            .basis
            .people
            .iter()
            .find(|p| p.employee_id == r.id)
            .unwrap();
        card(ui, |ui| {
            ui.add(egui::Label::new(format!("{} · {}", p.employee_code, p.employee_name)).wrap());
            ui.add(
                egui::Label::new(format!(
                    "SSS {} · SS {} · EC {}",
                    p.legal["sss_number"],
                    crate::payroll::money(p.ss_cents),
                    crate::payroll::money(p.ec_cents)
                ))
                .wrap(),
            );
            ui.add(
                egui::Label::new(format!(
                    "Recorded name: {}, {} {} {}",
                    p.legal["last_name"],
                    p.legal["first_name"],
                    p.legal["middle_name"],
                    p.legal["name_suffix"]
                ))
                .wrap(),
            );
            ui.columns(2, |cols| {
                changed |= input(
                    &mut cols[0],
                    "Surname representation · max 15 ASCII",
                    &mut r.last_name,
                );
                changed |= input(
                    &mut cols[1],
                    "Given name representation · max 15 ASCII",
                    &mut r.first_name,
                );
            });
            changed |= input(
                ui,
                "Explicit middle initial · one ASCII character or blank",
                &mut r.initial,
            );
            let before = r.remark;
            egui::ComboBox::from_id_salt(("sss_declaration", r.id))
                .selected_text(label(r.remark))
                .show_ui(ui, |ui| {
                    for v in [Remark::Normal, Remark::NewHire, Remark::Separated] {
                        ui.selectable_value(&mut r.remark, Some(v), label(Some(v)));
                    }
                });
            changed |= before != r.remark;
            if matches!(r.remark, Some(Remark::NewHire | Remark::Separated)) {
                changed |= input(
                    ui,
                    "Declared hire/separation date · YYYY-MM-DD · covered month only",
                    &mut r.date,
                );
            }
        });
    }
    if changed {
        s.reviewed = false;
        s.error = None;
    }
    ui.checkbox(&mut s.reviewed,"I reviewed the recorded receipt, unchanged identifiers/amounts, file name representations and all employment declarations.");
    if let Some(e) = &s.error {
        ui.colored_label(Color32::from_rgb(180, 60, 60), e);
    }
    if ui
        .add_enabled(
            !busy
                && connected
                && s.reviewed
                && s.fresh
                    .as_ref()
                    .is_none_or(|f| f.fingerprint == s.basis.fingerprint),
            egui::Button::new("Save reference draft for review"),
        )
        .clicked()
    {
        match s.request().and_then(|r| s.basis.render(&r).map(|_| r)) {
            Ok(r) => out = Some(Intent::Export(Box::new(r))),
            Err(e) => s.error = Some(e.to_string()),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn form_fits_compact_and_normal_content_widths() {
        for width in [510.0, 900.0] {
            let ctx = egui::Context::default();
            let b = collection::Basis::from_preparation(&collection::fixture()).unwrap();
            let mut s = State::new(b);
            let raw = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(width, 1600.0),
                )),
                ..Default::default()
            };
            for _ in 0..2 {
                let mut output = ctx.run_ui(raw.clone(), |ui| {
                    let right = ui.max_rect().right();
                    render(ui, &mut s, false, true);
                    assert!(
                        ui.min_rect().right() <= right + 1.0,
                        "Form overflow at width {width}"
                    );
                });
                output.textures_delta.clear();
            }
        }
    }
    #[test]
    fn declarations_are_explicit_and_source_refresh_never_overwrites_unsaved_names() {
        let b = collection::Basis::from_preparation(&collection::fixture()).unwrap();
        let mut s = State::new(b.clone());
        assert!(s.request().is_err());
        s.rows[0].last_name = "Unsaved name".into();
        s.fresh = Some(b);
        assert_eq!(s.rows[0].last_name, "Unsaved name");
        assert!(!s.reviewed);
        s.rows[0].remark = Some(Remark::Normal);
        s.reviewed = true;
        assert!(s.request().is_ok());
    }
}
