//! Separate native application shells over the same protected payroll implementation.
use crate::{
    access::Access,
    backend::Command,
    domain::{Action, Snapshot},
    staff_roles::Capability,
    workspaces::{Entry, Group, Page},
};
use anyhow::{Context, Result, ensure};
use std::path::{Path, PathBuf};
pub type AppResult = eframe::Result;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Product {
    Hris,
    Payroll,
}
impl Product {
    pub const fn id(self) -> &'static str {
        match self {
            Self::Hris => crate::APP_ID,
            Self::Payroll => "io.github.tatselkrik.payrolldesktop",
        }
    }
    pub const fn name(self) -> &'static str {
        match self {
            Self::Hris => "HRIS Desktop",
            Self::Payroll => "Payroll Desktop",
        }
    }
    pub const fn executable(self) -> &'static str {
        match self {
            Self::Hris => "HRIS-Desktop.exe",
            Self::Payroll => "Payroll-Desktop.exe",
        }
    }
    pub const fn marker(self) -> &'static str {
        match self {
            Self::Hris => ".hris-installation",
            Self::Payroll => ".payroll-installation",
        }
    }
    pub const fn sign_in_message(self) -> &'static str {
        match self {
            Self::Hris => "Use your HRIS account to access company records.",
            Self::Payroll => {
                "Use your HRIS account. Payroll Officer or Administrator access is required."
            }
        }
    }
    pub fn admits(self, access: &Access) -> bool {
        access.uses_desktop()
            && (self == Self::Hris || access.allows(Capability::PayrollPreparation))
    }
    pub fn landing(self, data: &Snapshot) -> Page {
        if self == Self::Payroll {
            Page::Payroll
        } else {
            crate::workspaces::landing(data)
        }
    }
    pub fn visible(self, page: Page, data: &Snapshot) -> bool {
        if self == Self::Payroll {
            self.admits(&data.access)
                && matches!(
                    page,
                    Page::Payroll | Page::Settings | Page::Profile | Page::Notifications
                )
        } else {
            crate::workspaces::visible(page, data)
        }
    }
    pub fn label(self, page: Page, data: &Snapshot) -> &'static str {
        if page == Page::Payroll {
            if self == Self::Payroll {
                "Payroll"
            } else {
                "My payslips"
            }
        } else {
            crate::workspaces::label(page, data)
        }
    }
    pub fn entries(self, data: &Snapshot) -> Vec<Entry> {
        if self == Self::Payroll {
            if !self.admits(&data.access) {
                return Vec::new();
            }
            vec![
                Entry {
                    page: Page::Payroll,
                    label: "Payroll",
                    group: Some(Group::Work),
                },
                Entry {
                    page: Page::Settings,
                    label: "Settings",
                    group: Some(Group::Administration),
                },
            ]
        } else {
            crate::workspaces::entries(data)
                .into_iter()
                .map(|mut e| {
                    if e.page == Page::Payroll {
                        e.label = "My payslips";
                    }
                    e
                })
                .collect()
        }
    }
    // The worker applies this before any dispatch. Server capabilities remain the authority.
    pub fn command_allowed(self, command: &Command) -> bool {
        match command {
            Command::Connect { .. }
            | Command::Refresh
            | Command::SignOut
            | Command::SignOutEverywhere
            | Command::Account(_) => true,
            Command::Change(Action::SaveOwnContacts(_)) => true,
            Command::Change(Action::NotificationReads(action)) if self == Self::Payroll => {
                match action {
                    crate::notifications::ReadAction::Mark { keys, .. } => {
                        !keys.is_empty()
                            && keys.iter().all(|k| {
                                k.starts_with("payroll-")
                                    && crate::notifications::lifecycle::valid_key(k)
                            })
                    }
                    crate::notifications::ReadAction::Reset => false,
                }
            }
            Command::Change(Action::Payroll(_)) => self == Self::Payroll,
            Command::GovernmentBirDeclarations { .. }
            | Command::GovernmentBirIdentities { .. }
            | Command::GovernmentAnnual { .. }
            | Command::GovernmentAnnualPay { .. }
            | Command::GovernmentSssPrepare { .. }
            | Command::GovernmentBirCheck { .. }
            | Command::GovernmentBirBuild(_)
            | Command::GovernmentSssExport { .. }
            | Command::GovernmentPreparation { .. }
            | Command::GovernmentRegistration { .. }
            | Command::Government { .. }
            | Command::PayrollReport { .. }
            | Command::PayrollExchange(_) => self == Self::Payroll,
            Command::PayrollExport {
                kind, record_id, ..
            } => {
                self == Self::Payroll
                    || (record_id.is_some() && OWN_EXPORTS.contains(&kind.as_str()))
            }
            _ => self == Self::Hris,
        }
    }
    pub fn installation(self) -> Result<PathBuf> {
        Ok(PathBuf::from(
            std::env::var_os("LOCALAPPDATA")
                .context("Application installation folder unavailable.")?,
        )
        .join("Programs")
        .join(self.name()))
    }
    pub fn project(self, data: &mut Snapshot) {
        if self == Self::Payroll {
            data.contacts
                .retain(|c| Some(c.employee_id) == data.access.employee_id);
            data.attendance.clear();
            data.leave.clear();
            data.accounts.clear();
            data.access_events.clear();
            data.corrections.clear();
            data.employment_history.clear();
            data.documents.clear();
            data.document_events.clear();
            data.approvals = None;
            data.hiring = Default::default();
            data.recruiting = Default::default();
            data.onboarding = Default::default();
            data.exits = Default::default();
            data.performance = Default::default();
            data.learning = Default::default();
            data.career = Default::default();
            data.imports = Default::default();
            data.overtime = Default::default();
            data.requests = Default::default();
            data.shifts = Default::default();
            data.breaks = Default::default();
            data.calculations = Default::default();
            data.leave_setup = Default::default();
        } else {
            own_payroll(&mut data.payroll, data.access.employee_id);
        }
    }
}
pub const OWN_EXPORTS: &[&str] = &[
    "payslip",
    "payslip_pdf",
    "cutoff_payslip",
    "cutoff_payslip_pdf",
    "thirteenth_payslip",
    "thirteenth_payslip_pdf",
    "tax_payslip",
    "tax_payslip_pdf",
    "final_statement",
    "final_statement_pdf",
    "amendment_statement",
    "amendment_statement_pdf",
];
fn own_payroll(p: &mut crate::payroll::Setup, own: Option<uuid::Uuid>) {
    let mut next = crate::payroll::Setup::default();
    next.slips = std::mem::take(&mut p.slips)
        .into_iter()
        .filter(|s| Some(s.employee_id) == own)
        .collect();
    next.cutoffs.slips = std::mem::take(&mut p.cutoffs.slips)
        .into_iter()
        .filter(|s| Some(s.base.employee_id) == own)
        .collect();
    next.thirteenth.slips = std::mem::take(&mut p.thirteenth.slips)
        .into_iter()
        .filter(|s| Some(s.employee_id) == own)
        .collect();
    next.tax.slips = std::mem::take(&mut p.tax.slips)
        .into_iter()
        .filter(|s| Some(s.employee_id) == own)
        .collect();
    next.finals.slips = std::mem::take(&mut p.finals.slips)
        .into_iter()
        .filter(|s| Some(s.employee_id) == own)
        .collect();
    next.finals.statuses = std::mem::take(&mut p.finals.statuses)
        .into_iter()
        .filter(|s| Some(s.employee_id) == own)
        .collect();
    next.amendments.slips = std::mem::take(&mut p.amendments.slips)
        .into_iter()
        .filter(|s| Some(s.employee_id) == own)
        .collect();
    next.amendments.statuses = std::mem::take(&mut p.amendments.statuses)
        .into_iter()
        .filter(|s| Some(s.employee_id) == own)
        .collect();
    *p = next;
}
#[derive(Clone, Copy, Debug)]
pub struct Application {
    pub product: Product,
    pub version: &'static str,
}
impl Application {
    pub const fn hris() -> Self {
        Self {
            product: Product::Hris,
            version: env!("CARGO_PKG_VERSION"),
        }
    }
    pub const fn payroll(version: &'static str) -> Self {
        Self {
            product: Product::Payroll,
            version,
        }
    }
}
pub fn run(app: Application) -> AppResult {
    if std::env::args().nth(1).as_deref() == Some("--apply-update") {
        if app.product != Product::Hris {
            std::process::exit(1);
        }
        if let Some(path) = std::env::args_os().nth(2) {
            if let Err(error) = crate::updater::apply_plan(Path::new(&path)) {
                eprintln!("HRIS update failed: {error}");
                std::process::exit(1);
            }
        } else {
            std::process::exit(1);
        }
        return Ok(());
    }
    #[cfg(target_os = "windows")]
    {
        let id: Vec<u16> = app.product.id().encode_utf16().chain(Some(0)).collect();
        // SAFETY: the application ID is a valid null-terminated UTF-16 buffer.
        unsafe {
            windows_sys::Win32::UI::Shell::SetCurrentProcessExplicitAppUserModelID(id.as_ptr());
        }
    }
    let size = [1320.0, 880.0];
    #[cfg(feature = "qa-screenshot")]
    let size = if std::env::var("HRIS_PREVIEW_SIZE").as_deref() == Ok("compact") {
        [1000.0, 720.0]
    } else {
        size
    };
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_app_id(app.product.id())
            .with_inner_size(size)
            .with_min_inner_size([1000.0, 720.0]),
        renderer: eframe::Renderer::Glow,
        ..Default::default()
    };
    eframe::run_native(
        app.product.name(),
        options,
        Box::new(move |cc| Ok(Box::new(crate::ui::HrisApp::new_for(cc, app)))),
    )
}
/// Launch only the separately verified local product. Never execute a path from company data.
pub fn open_payroll() -> Result<()> {
    let folder = Product::Payroll.installation()?.canonicalize().context("Payroll Desktop is not installed yet. Install your company's Payroll Desktop package first.")?;
    verify_payroll_installation(&folder)?;
    std::process::Command::new(folder.join(Product::Payroll.executable()))
        .current_dir(folder)
        .spawn()
        .context("Could not open Payroll Desktop.")?;
    Ok(())
}
fn verify_payroll_installation(folder: &Path) -> Result<()> {
    ensure!(
        folder.join(Product::Payroll.marker()).metadata()?.len() <= 128,
        "Payroll installation marker is invalid."
    );
    ensure!(
        std::fs::read(folder.join(Product::Payroll.marker()))? == Product::Payroll.id().as_bytes(),
        "Payroll Desktop is not installed in this folder."
    );
    let path = folder.join("BUILD.json");
    ensure!(
        path.metadata()?.len() <= 256 * 1024,
        "Payroll installation metadata is invalid."
    );
    let build: serde_json::Value = serde_json::from_slice(&std::fs::read(path)?)?;
    let exe = folder.join(Product::Payroll.executable());
    ensure!(
        exe.metadata()?.is_file()
            && exe.metadata()?.len() <= crate::updater::MAX_BYTES
            && exe.canonicalize()?.parent() == Some(folder),
        "Payroll executable is outside its installation."
    );
    ensure!(
        build["application_id"] == Product::Payroll.id()
            && build["executable_sha256"].as_str()
                == Some(crate::updater::digest(&std::fs::read(exe)?).as_str()),
        "Payroll installation checksum does not match. Reinstall the verified package."
    );
    Ok(())
}

/// Same shared page implementations and ordering as HRIS0.1.64.
pub const PAYROLL_TABS: &[&str] = &[
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
    "Wage compliance",
    "Amendments",
    "Government",
    "Data exchange",
];

#[cfg(test)]
mod installation_tests {
    use super::*;
    #[test]
    fn launcher_verifies_separate_product_marker_and_exact_binary_without_running_unknown_files() {
        let folder = std::env::temp_dir().join(format!("payroll-launch-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&folder).unwrap();
        let folder = folder.canonicalize().unwrap();
        let marker = folder.join(Product::Payroll.marker());
        let exe = folder.join(Product::Payroll.executable());
        let build = folder.join("BUILD.json");
        let unknown = folder.join("company-note.txt");
        std::fs::write(&marker, Product::Payroll.id()).unwrap();
        std::fs::write(&exe, b"MZfictional-not-executed").unwrap();
        std::fs::write(&unknown, b"Preserve company note").unwrap();
        let metadata = serde_json::json!({"application_id":Product::Payroll.id(),"executable_sha256":crate::updater::digest(&std::fs::read(&exe).unwrap())});
        std::fs::write(&build, serde_json::to_vec(&metadata).unwrap()).unwrap();
        assert!(verify_payroll_installation(&folder).is_ok());
        std::fs::write(&marker, Product::Hris.id()).unwrap();
        assert!(verify_payroll_installation(&folder).is_err());
        std::fs::write(&marker, Product::Payroll.id()).unwrap();
        let mut wrong = metadata.clone();
        wrong["application_id"] = serde_json::json!(Product::Hris.id());
        std::fs::write(&build, serde_json::to_vec(&wrong).unwrap()).unwrap();
        assert!(verify_payroll_installation(&folder).is_err());
        std::fs::write(&build, serde_json::to_vec(&metadata).unwrap()).unwrap();
        std::fs::write(&exe, b"MZchanged-not-executed").unwrap();
        assert!(verify_payroll_installation(&folder).is_err());
        assert_eq!(std::fs::read(&unknown).unwrap(), b"Preserve company note");
        for path in [marker, exe, build, unknown] {
            std::fs::remove_file(path).unwrap();
        }
        std::fs::remove_dir(folder).unwrap();
    }
}
