use crate::{
    access::{AccountDraft, AdminAction, Role},
    account::{self, AccountAction, AccountProfile, DisplayPreferences, PersonalDetails},
    backend::{self, Command, ConnectionConfig, Update},
    branding,
    contacts::EmployeeContacts,
    corrections::CorrectionDraft,
    documents::{self, DocumentAction, DocumentCategory, DocumentDraft, DocumentState},
    domain::*,
    employment::{EmploymentFilter, EmploymentKind, EmploymentOptionDraft},
    updater::{self, UpdateState},
};
use chrono::{Datelike, Duration, Months, NaiveDate};
use eframe::egui::{self, Align, Color32, FontId, Frame, Layout, RichText, Stroke, Ui, Vec2};
use std::sync::mpsc;
use uuid::Uuid;

const NAVY: Color32 = Color32::from_rgb(22, 36, 51);
const INK: Color32 = Color32::from_rgb(29, 45, 60);
const MUTED: Color32 = Color32::from_rgb(103, 119, 134);
const TEAL: Color32 = Color32::from_rgb(0, 123, 112);
const BG: Color32 = Color32::from_rgb(245, 247, 250);
const LINE: Color32 = Color32::from_rgb(226, 232, 238);

mod staff_roles_view;
mod staff_provision_view;
mod breaks_view;
mod calculations_view;
mod hiring_view;
mod imports_view;
mod attendance_import_view;
mod leave_request_import_view;
mod leave_view;
mod leave_credits_view;
mod leave_openings_view;
mod leave_versions_view;
mod approval_delegation_view;
mod notifications_view;
mod onboarding_view;
mod exits_view;
mod performance_view;
mod learning_view;
mod career_view;
mod payroll_view;
mod cutoff_view;
mod thirteenth_view;
mod tax_view;
mod payment_view;
mod final_view;
mod amendment_view;
mod wage_view;
mod payroll_report_view;
mod payroll_exchange_view;
mod government_view;
mod government_registration_view;
mod government_collection_view;
mod bir_file_view;
mod bir_builder_view;
mod government_annual_view;
mod government_identity_view;
mod government_declarations_view;
mod organization_view;
mod overtime_view;
mod recruiting_view;
mod report_columns;
mod reports_view;
mod requests_view;
mod shifts_view;
mod workforce_view;

#[derive(Clone, Copy, Default, PartialEq, Eq)]
enum EmployeeView {
    #[default]
    Directory,
    Organization,
    Development,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Page {
    Payroll,
    Overview,
    Employees,
    Attendance,
    Leave,
    Reports,
    Hiring,
    Exits,
    Accounts,
    Settings,
    Profile,
    Notifications,
}

impl Page {
    const ALL: [Self; 10] = [
        Self::Overview,
        Self::Employees,
        Self::Attendance,
        Self::Leave,
        Self::Payroll,
        Self::Reports,
        Self::Hiring,
        Self::Exits,
        Self::Accounts,
        Self::Settings,
    ];
    fn title(self) -> &'static str {
        match self {
            Self::Overview => "Overview",
            Self::Employees => "Employees",
            Self::Attendance => "Attendance",
            Self::Leave => "Leave",
            Self::Payroll => "Payroll",
            Self::Reports => "Reports",
            Self::Hiring => "Hiring",
            Self::Exits => "Exit & clearance",
            Self::Accounts => "Account Manager",
            Self::Settings => "Settings",
            Self::Profile => "Profile",
            Self::Notifications => "Notifications",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ProfileSection {
    Employment,
    Contacts,
    Emergency,
    Security,
    Sessions,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ContactSection {
    Employment,
    Contact,
    Emergency,
    Access,
    History,
    Documents,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum SessionChoice {
    Others,
    Everywhere,
}

pub struct HrisApp {
    data: Snapshot,
    page: Page,
    commands: mpsc::Sender<Command>,
    updates: mpsc::Receiver<Update>,
    busy: bool,
    notice: Option<(String, bool)>,
    search: String,
    employee_status: Option<EmployeeStatus>,
    employee_view: EmployeeView,
    organization: organization_view::State,
    leave_state: leave_view::State,
    shift_state: shifts_view::State,
    break_state: breaks_view::State,
    calculation_state: calculations_view::State,
    request_state: requests_view::State,
    overtime_state: overtime_view::State,
    report_state: reports_view::State,
    import_state: imports_view::State,
    attendance_import_state:attendance_import_view::State,
    leave_request_import_state:leave_request_import_view::State,
    notification_state: notifications_view::State,
    hiring_state: hiring_view::State,
    recruiting_state: recruiting_view::State,
    onboarding_state: onboarding_view::State,
    exit_state: exits_view::State,
    performance_state: performance_view::State,
    learning_state: learning_view::State,
    career_state: career_view::State,
    payroll_state: payroll_view::State,
    department: String,
    employment_type_filter: EmploymentFilter,
    work_location_filter: EmploymentFilter,
    employment_option_form: Option<EmploymentOptionDraft>,
    document_form: Option<DocumentDraft>,
    document_category: Option<DocumentCategory>,
    document_search: String,
    show_archived_documents: bool,
    show_discarded_documents: bool,
    close_document_on_success: bool,
    attendance_date: NaiveDate,
    calendar_month: NaiveDate,
    check_in_employee: Option<Uuid>,
    leave_filter: Option<LeaveStatus>,
    employee_form: Option<EmployeeDraft>,
    contacts_form: Option<EmployeeContacts>,
    contacts_section: ContactSection,
    leave_form: Option<LeaveDraft>,
    correction_form: Option<CorrectionDraft>,
    correction_history: Option<Uuid>,
    close_on_success: bool,
    company_draft: Company,
    connection: Option<ConnectionConfig>,
    deployment_error: Option<String>,
    login_email: String,
    login_password: String,
    company_logo: Option<egui::TextureHandle>,
    login_branding: branding::LoginBranding,
    login_logo: Option<egui::TextureHandle>,
    login_branding_warning: Option<String>,
    account: Option<AccountProfile>,
    avatar: Option<egui::TextureHandle>,
    personal_draft: PersonalDetails,
    display_draft: DisplayPreferences,
    profile_section: ProfileSection,
    current_password: String,
    new_password: String,
    confirm_password: String,
    new_email: String,
    email_password: String,
    show_passwords: bool,
    session_confirmation: Option<SessionChoice>,
    account_form: Option<AccountDraft>,
    show_disabled_staff: bool,
    staff_roles_state: crate::staff_roles::editor::State,
    staff_provision_state: crate::staff_roles::provision_editor::State,
    profile_contacts_draft: Option<EmployeeContacts>,
    update_state: UpdateState,
    update_confirmation: bool,
    #[cfg(feature = "qa-screenshot")]
    qa_frames: u32,
    #[cfg(feature = "qa-screenshot")]
    qa_started: std::time::Instant,
}

pub fn configure_theme(ctx: &egui::Context) {
    ctx.set_theme(egui::Theme::Light);
    for theme in [egui::Theme::Light, egui::Theme::Dark] {
        ctx.style_mut_of(theme, |style| {
            let dark = theme == egui::Theme::Dark;
            style.visuals = if dark {
                egui::Visuals::dark()
            } else {
                egui::Visuals::light()
            };
            let border = if dark {
                Color32::from_rgb(65, 76, 89)
            } else {
                LINE
            };
            let input = if dark {
                Color32::from_rgb(30, 38, 48)
            } else {
                Color32::from_rgb(242, 245, 248)
            };
            style.visuals.override_text_color = Some(if dark {
                Color32::from_rgb(230, 236, 243)
            } else {
                INK
            });
            style.visuals.weak_text_color = Some(if dark {
                Color32::from_rgb(167, 181, 198)
            } else {
                MUTED
            });
            style.visuals.panel_fill = if dark {
                Color32::from_rgb(22, 28, 36)
            } else {
                BG
            };
            style.visuals.window_fill = if dark {
                Color32::from_rgb(32, 40, 51)
            } else {
                Color32::WHITE
            };
            style.visuals.window_stroke = Stroke::new(1.0, border);
            style.visuals.text_edit_bg_color = Some(input);
            style.visuals.selection.bg_fill = if dark {
                Color32::from_rgb(18, 77, 72)
            } else {
                Color32::from_rgb(208, 238, 233)
            };
            style.visuals.selection.stroke = Stroke::new(
                1.0,
                if dark {
                    Color32::from_rgb(99, 221, 199)
                } else {
                    TEAL
                },
            );
            style.visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0, border);
            style.visuals.widgets.inactive.bg_fill = input;
            style.visuals.widgets.inactive.weak_bg_fill = input;
            style.visuals.widgets.inactive.bg_stroke = Stroke::new(1.0, border);
            style.visuals.widgets.hovered.bg_fill = if dark {
                Color32::from_rgb(40, 71, 74)
            } else {
                Color32::from_rgb(226, 240, 237)
            };
            style.visuals.widgets.hovered.weak_bg_fill = style.visuals.widgets.hovered.bg_fill;
            style.visuals.widgets.hovered.bg_stroke = Stroke::new(1.0, TEAL);
            style.spacing.item_spacing = Vec2::new(12.0, 10.0);
            style.spacing.button_padding = Vec2::new(14.0, 9.0);
            style.spacing.interact_size.y = 34.0;
            style
                .text_styles
                .insert(egui::TextStyle::Body, FontId::proportional(15.0));
            style
                .text_styles
                .insert(egui::TextStyle::Button, FontId::proportional(14.0));
            style
                .text_styles
                .insert(egui::TextStyle::Small, FontId::proportional(12.0));
            style
                .text_styles
                .insert(egui::TextStyle::Heading, FontId::proportional(28.0));
        });
    }
}

fn muted(ui: &Ui) -> Color32 {
    ui.visuals().weak_text_color()
}
fn accent(ui: &Ui) -> Color32 {
    if ui.visuals().dark_mode {
        Color32::from_rgb(102, 224, 201)
    } else {
        TEAL
    }
}
fn adaptive(ui: &Ui, light: Color32, dark: Color32) -> Color32 {
    if ui.visuals().dark_mode { dark } else { light }
}

impl HrisApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        configure_theme(&cc.egui_ctx);
        let (connection, deployment_error) = match ConnectionConfig::load() {
            Ok(config) if config.url.is_empty() => (None, None),
            Ok(config) => (Some(config), None),
            Err(error) => (None, Some(error.to_string())),
        };
        let demo = (connection.is_none() && deployment_error.is_none()).then(DemoStore::new);
        let data = demo
            .as_ref()
            .map(|store| store.data.clone())
            .unwrap_or_else(Snapshot::signed_out);
        let (commands, updates) = backend::worker(demo, cc.egui_ctx.clone());
        let company_logo = std::env::current_exe()
            .ok()
            .and_then(|exe| exe.parent().map(|folder| folder.join("company-logo.png")))
            .and_then(|path| branding::load_logo(&path).ok())
            .map(|image| {
                cc.egui_ctx
                    .load_texture("company-logo", image, egui::TextureOptions::LINEAR)
            });
        let login_branding = connection
            .as_ref()
            .and_then(branding::load_login_branding)
            .unwrap_or_default();
        let login_logo = login_branding
            .logo_png
            .as_ref()
            .and_then(|png| branding::circular_logo(png).ok())
            .map(|image| {
                cc.egui_ctx
                    .load_texture("login-company-logo", image, egui::TextureOptions::LINEAR)
            })
            .or_else(|| {
                login_branding
                    .company_name
                    .is_empty()
                    .then(|| company_logo.clone())
                    .flatten()
            });
        let account = (data.mode == Mode::Demo).then(AccountProfile::demo);
        let personal_draft = account
            .as_ref()
            .map(|a| a.details.clone())
            .unwrap_or_default();
        let report_state = reports_view::State::new(data.company.today());
        let mut app = Self {
            attendance_date: data.company.today(),
            calendar_month: month_start(data.company.today()),
            company_draft: data.company.clone(),
            data,
            page: Page::Overview,
            commands,
            updates,
            busy: false,
            notice: updater::previous_result(),
            search: String::new(),
            employee_status: Some(EmployeeStatus::Active),
            employee_view: EmployeeView::Directory,
            organization: organization_view::State::default(),
            leave_state: leave_view::State::default(),
            shift_state: shifts_view::State::default(),
            break_state: breaks_view::State::default(),
            calculation_state: calculations_view::State::default(),
            request_state: requests_view::State::default(),
            overtime_state: overtime_view::State::default(),
            report_state,
            import_state: imports_view::State::default(),
            attendance_import_state:attendance_import_view::State::default(),
            leave_request_import_state:leave_request_import_view::State::default(),
            notification_state: notifications_view::State::default(),
            hiring_state: hiring_view::State::default(),
            recruiting_state: Default::default(),
            onboarding_state: Default::default(),
            exit_state: Default::default(),
            performance_state: Default::default(),
            learning_state: Default::default(),
            career_state: Default::default(),
            payroll_state: Default::default(),
            department: String::new(),
            employment_type_filter: EmploymentFilter::All,
            work_location_filter: EmploymentFilter::All,
            employment_option_form: None,
            document_form: None,
            document_category: None,
            document_search: String::new(),
            show_archived_documents: false,
            show_discarded_documents: false,
            close_document_on_success: false,
            check_in_employee: None,
            leave_filter: None,
            employee_form: None,
            contacts_form: None,
            contacts_section: ContactSection::Contact,
            leave_form: None,
            correction_form: None,
            correction_history: None,
            close_on_success: false,
            connection,
            deployment_error,
            login_email: String::new(),
            login_password: String::new(),
            company_logo,
            login_branding,
            login_logo,
            login_branding_warning: None,
            account,
            avatar: None,
            personal_draft,
            display_draft: DisplayPreferences::default(),
            profile_section: ProfileSection::Employment,
            current_password: String::new(),
            new_password: String::new(),
            confirm_password: String::new(),
            new_email: String::new(),
            email_password: String::new(),
            show_passwords: false,
            session_confirmation: None,
            account_form: None,
            show_disabled_staff: false,
            staff_roles_state: Default::default(),
            staff_provision_state: Default::default(),
            profile_contacts_draft: None,
            update_state: UpdateState::Idle,
            update_confirmation: false,
            #[cfg(feature = "qa-screenshot")]
            qa_frames: 0,
            #[cfg(feature = "qa-screenshot")]
            qa_started: std::time::Instant::now(),
        };
        #[cfg(feature = "qa-screenshot")]
        {
            if app.data.mode == Mode::Demo {
                if let Ok(name) = std::env::var("HRIS_PREVIEW_COMPANY_NAME") {
                    app.data.company.name = name;
                }
            }
            match std::env::var("HRIS_PREVIEW_PAGE").as_deref() {
                Ok("employees") => app.page = Page::Employees,
                Ok("attendance") => app.page = Page::Attendance,
                Ok("leave") => app.page = Page::Leave,
                Ok("payroll") => app.page = Page::Payroll,
                Ok("reports") => app.page = Page::Reports,
                Ok("hiring") => app.page = Page::Hiring,
                Ok("exits") => app.page = Page::Exits,
                Ok("notifications") => app.page = Page::Notifications,
                Ok("settings") => app.page = Page::Settings,
                Ok("profile") => app.page = Page::Profile,
                Ok("accounts") => app.page = Page::Accounts,
                _ => {}
            }
            if app.data.mode == Mode::Demo {
                if let Ok(role) = std::env::var("HRIS_PREVIEW_ROLE") {
                    app.data.access.role = match role.as_str() {
                        "hr_generalist" => Role::HrGeneralist,
                        "manager" => Role::Manager,
                        "employee" => Role::Employee,
                        _ => Role::Administrator,
                    };
                    if !app.data.access.allows(crate::staff_roles::Capability::Personnel) {
                        let own = Uuid::from_u128(if app.data.access.role == Role::Manager {
                            103
                        } else {
                            101
                        });
                        app.data.access.employee_id = Some(own);
                        app.data.access.direct_reports = if app.data.access.role == Role::Manager {
                            vec![Uuid::from_u128(101)]
                        } else {
                            Vec::new()
                        };
                        app.data.employees.retain(|e| {
                            Some(e.id) == app.data.access.employee_id
                                || app.data.access.direct_reports.contains(&e.id)
                        });
                        app.data
                            .attendance
                            .retain(|a| app.data.employees.iter().any(|e| e.id == a.employee_id));
                        app.data
                            .leave
                            .retain(|r| app.data.employees.iter().any(|e| e.id == r.employee_id));
                        app.data
                            .contacts
                            .retain(|c| app.data.access.can_read_contacts(c.employee_id));
                    }
                    if !app.data.access.role.manages_accounts() {
                        app.data.accounts.clear();
                    }
                    if !app.data.access.uses_desktop() {
                        app.login_branding = branding::LoginBranding {
                            company_name: app.data.company.name.clone(),
                            logo_png: app.data.logo_png.clone(),
                        };
                        app.login_logo = app.company_logo.clone();
                        app.data = Snapshot::signed_out();
                        app.account = None;
                        app.notice = Some((crate::access::EMPLOYEE_PORTAL_NOTICE.into(), true));
                    }
                }
                if matches!(
                    std::env::var("HRIS_PREVIEW_FORM").as_deref(),
                    Ok("account" | "staff-access")
                ) {
                    app.new_employee();
                    let mut draft = AccountDraft::new();
                    draft.role = Role::HrGeneralist;
                    app.account_form = Some(draft);
                    if let Some(employee) = app.employee_form.as_mut() {
                        employee.full_name = "Synthetic Staff Preview".into();
                    }
                }
                match std::env::var("HRIS_PREVIEW_PROFILE").as_deref() {
                    Ok("contacts") => app.profile_section = ProfileSection::Contacts,
                    Ok("emergency") => app.profile_section = ProfileSection::Emergency,
                    Ok("security") => app.profile_section = ProfileSection::Security,
                    Ok("sessions") => app.profile_section = ProfileSection::Sessions,
                    _ => {}
                }
                if std::env::var("HRIS_PREVIEW_UPDATE").as_deref() == Ok("available") {
                    app.update_state = UpdateState::Available(updater::VerifiedRelease {
                        release: updater::Release {
                            application_id: crate::APP_ID.into(),
                            version: "0.1.9".into(),
                            platform: "windows-x64".into(),
                            requires_schema: "20261021054356".into(),
                            artifact: "0.1.8/HRIS-Desktop-Update-0.1.8.exe".into(),
                            sha256: "0".repeat(64),
                            executable_sha256: "0".repeat(64),
                            size: 1,
                            notes: "Fictional update preview for layout testing.".into(),
                        },
                        signed: updater::SignedRelease {
                            payload: String::new(),
                            signature: String::new(),
                        },
                    });
                }
                if let Ok(theme) = std::env::var("HRIS_PREVIEW_THEME") {
                    app.display_draft.theme = if theme == "dark" {
                        account::ThemeChoice::Dark
                    } else {
                        account::ThemeChoice::Light
                    };
                    if let Some(profile) = app.account.as_mut() {
                        profile.display.theme = app.display_draft.theme;
                    }
                }
                if let Ok(path) = std::env::var("HRIS_PREVIEW_COMPANY_LOGO") {
                    if let Ok(png) = branding::logo_from_file(std::path::Path::new(&path)) {
                        app.company_logo = branding::circular_logo(&png).ok().map(|image| {
                            cc.egui_ctx.load_texture(
                                "company-logo",
                                image,
                                egui::TextureOptions::LINEAR,
                            )
                        });
                        app.data.logo_png = Some(png);
                        app.data.company.logo_version = Some(Uuid::from_u128(900));
                    }
                }
                if let Ok(scale) = std::env::var("HRIS_PREVIEW_SCALE") {
                    if let Ok(scale) = scale.parse::<u16>() {
                        if (100..=125).contains(&scale) {
                            if let Some(profile) = app.account.as_mut() {
                                profile.display.scale_percent = scale;
                            }
                            app.display_draft.scale_percent = scale;
                            cc.egui_ctx.set_zoom_factor(scale as f32 / 100.0);
                        }
                    }
                }
                if let Ok(path) = std::env::var("HRIS_PREVIEW_AVATAR") {
                    if let Ok(photo) = account::photo_from_file(std::path::Path::new(&path)) {
                        let image = account::circular_photo(&photo).ok();
                        if let Some(profile) = app.account.as_mut() {
                            profile.photo = Some(photo);
                        }
                        app.avatar = image.map(|image| {
                            cc.egui_ctx.load_texture(
                                "profile-avatar",
                                image,
                                egui::TextureOptions::LINEAR,
                            )
                        });
                    }
                }
            }
            match std::env::var("HRIS_PREVIEW_FORM").as_deref() {
                Ok("employee") => app.new_employee(),
                Ok("employee-existing") => app.open_employee(Uuid::from_u128(103)),
                Ok("contacts") | Ok("emergency") => {
                    app.open_contacts(Uuid::from_u128(103));
                    if std::env::var("HRIS_PREVIEW_FORM").as_deref() == Ok("emergency") {
                        app.contacts_section = ContactSection::Emergency;
                    }
                }
                Ok("leave") => app.new_leave(),
                Ok("staff-existing") => {
                    if let Some(access) = app
                        .data
                        .accounts
                        .iter()
                        .find(|a| a.role == Role::HrGeneralist)
                        .cloned()
                    {
                        if let Some(id) = access.employee_id {
                            app.open_employee(id);
                            app.account_form = Some(AccountDraft::from_account(&access));
                            app.contacts_section = ContactSection::Access;
                        }
                    }
                }
                Ok("staff-access") => {
                    app.contacts_section = ContactSection::Access;
                }
                _ => {}
            }
            if app.data.mode == Mode::Demo
                && std::env::var("HRIS_PREVIEW_ATTENDANCE_DATE").as_deref() == Ok("previous")
            {
                let date = app.data.company.today().pred_opt().unwrap();
                app.attendance_date = date;
                app.calendar_month = month_start(date);
                if let Some(mut record) = app.data.attendance.first().cloned() {
                    record.id = Uuid::new_v4();
                    record.work_date = date;
                    record.checked_in_at -= Duration::days(1);
                    record.checked_out_at =
                        record.checked_out_at.map(|value| value - Duration::days(1));
                    app.data.attendance.push(record);
                }
                if let Some(employee) = app.data.employees.get(1) {
                    app.data.leave.push(LeaveRequest {
                        policy: Default::default(),
                        id: Uuid::new_v4(),
                        company_id: app.data.company.id,
                        employee_id: employee.id,
                        kind: LeaveKind::Annual,
                        starts_on: date,
                        ends_on: date,
                        reason: "Fictional calendar preview".into(),
                        status: LeaveStatus::Approved,
                    });
                }
            }
            if matches!(
                std::env::var("HRIS_PREVIEW_FORM").as_deref(),
                Ok("correction-new"
                    | "correction-edit"
                    | "correction-history"
                    | "correction-error")
            ) {
                let day = app.data.company.today() - Duration::days(2);
                let employee = app.data.employees[0].id;
                app.data.attendance.clear();
                app.attendance_date = day;
                app.calendar_month = month_start(day);
                let mut draft = CorrectionDraft::new(employee, day, None, &app.data);
                draft.checked_in = "08:00".into();
                draft.checked_out = "17:00".into();
                draft.reason =
                    "Fictional QA: missed check-in verified against the shift log.".into();
                if std::env::var("HRIS_PREVIEW_FORM").as_deref() != Ok("correction-new") {
                    let mut store = DemoStore {
                        data: app.data.clone(),
                    };
                    if let Ok(change) = draft.change(&store.data, chrono::Utc::now()) {
                        let _ = store.execute(Action::CorrectAttendance(change));
                        app.data = store.data;
                    }
                    app.open_correction(employee, day);
                    if let Some(edit) = &mut app.correction_form {
                        edit.checked_in = "08:15".into();
                        edit.reason = "Fictional QA: supervisor confirmed the arrival time.".into();
                    }
                    if std::env::var("HRIS_PREVIEW_FORM").as_deref() == Ok("correction-history") {
                        if let Some(edit) = app.correction_form.take() {
                            let mut store = DemoStore {
                                data: app.data.clone(),
                            };
                            if let Ok(change) = edit.change(&store.data, chrono::Utc::now()) {
                                let _ = store.execute(Action::CorrectAttendance(change));
                                app.data = store.data;
                            }
                            app.correction_history = app.data.attendance.first().map(|a| a.id);
                        }
                    } else if std::env::var("HRIS_PREVIEW_FORM").as_deref()
                        == Ok("correction-error")
                    {
                        app.notice=Some(("Someone changed this attendance record. Refresh, reopen it and review their changes.".into(),true));
                    }
                } else {
                    app.correction_form = Some(draft);
                }
            }
            match std::env::var("HRIS_PREVIEW_FORM").as_deref() {
                Ok("employment-inactive" | "employment-filter") => {
                    app.data.employment_types[0].active = false;
                    app.data.work_locations[0].active = false;
                    if std::env::var("HRIS_PREVIEW_FORM").as_deref() == Ok("employment-filter") {
                        app.employment_type_filter =
                            EmploymentFilter::Choice(app.data.employment_types[0].id);
                        app.work_location_filter =
                            EmploymentFilter::Choice(app.data.work_locations[0].id);
                    } else {
                        app.open_employee(Uuid::from_u128(100));
                    }
                }
                Ok("employment-long") => {
                    app.data.employment_types[0].name="Company-defined specialist employment classification for regional service staff".into();
                    app.data.work_locations[0].name="Regional service and administration office for the northern operations district".into();
                    if app.page == Page::Employees {
                        app.open_employee(Uuid::from_u128(100));
                    }
                }
                Ok("choice-new") => {
                    app.employment_option_form =
                        Some(EmploymentOptionDraft::new(EmploymentKind::EmploymentType))
                }
                Ok("employment-new") => app.new_employee(),
                Ok("choice-inactive") => {
                    let mut draft = EmploymentOptionDraft::existing(
                        EmploymentKind::WorkLocation,
                        &app.data.work_locations[0],
                    );
                    draft.active = false;
                    app.employment_option_form = Some(draft);
                }
                Ok(
                    "chart" | "chart-filtered" | "chart-collapsed" | "chart-archived"
                    | "chart-long" | "chart-deep" | "chart-empty" | "chart-manager"
                    | "chart-record" | "chart-no-match" | "chart-cycle",
                ) => {
                    let preview = std::env::var("HRIS_PREVIEW_FORM").unwrap_or_default();
                    app.employee_view = EmployeeView::Organization;
                    for employee in &mut app.data.employees {
                        employee.reports_to = match employee.id.as_u128() {
                            101 => Some(Uuid::from_u128(103)),
                            102 | 103 | 106 | 107 => Some(Uuid::from_u128(100)),
                            104 | 105 => Some(Uuid::from_u128(102)),
                            _ => None,
                        };
                    }
                    match preview.as_str() {
                        "chart-filtered" => {
                            app.organization.filters.department = Some("Operations".into())
                        }
                        "chart-collapsed" => {
                            app.organization.collapsed.insert(Uuid::from_u128(100));
                        }
                        "chart-archived" => app.organization.filters.include_archived = true,
                        "chart-empty" => app.data.employees.clear(),
                        "chart-no-match" => {
                            app.organization.filters.search = "No matching person".into()
                        }
                        "chart-cycle" => {
                            if let Some(e) = app
                                .data
                                .employees
                                .iter_mut()
                                .find(|e| e.id == Uuid::from_u128(106))
                            {
                                e.reports_to = Some(e.id);
                            }
                        }
                        "chart-long" => {
                            if let Some(e) = app
                                .data
                                .employees
                                .iter_mut()
                                .find(|e| e.id == Uuid::from_u128(100))
                            {
                                e.full_name="Alexandra Rivera Santos — regional employee services and operations".into();
                                e.department="Regional employee services and administrative operations for the northern district".into();
                                e.job_title="Senior manager for regional people operations and organizational development".into();
                            }
                            app.data.work_locations[0].name="Regional headquarters and employee services centre for the northern district".into();
                        }
                        "chart-deep" => {
                            let template = app.data.employees[0].clone();
                            for id in 3000..3012 {
                                let mut employee = template.clone();
                                employee.id = Uuid::from_u128(id);
                                employee.employee_code = format!("QA-{id}");
                                employee.full_name =
                                    format!("Synthetic reporting level {}", id - 2998);
                                employee.reports_to =
                                    Some(Uuid::from_u128(if id == 3000 { 100 } else { id - 1 }));
                                app.data.employees.push(employee);
                            }
                        }
                        "chart-record" => app.open_employee(Uuid::from_u128(101)),
                        _ => {}
                    }
                }
                Ok(
                    "documents"
                    | "documents-archived"
                    | "documents-empty"
                    | "documents-upload"
                    | "documents-contract"
                    | "documents-edit"
                    | "documents-error"
                    | "documents-manager"
                    | "documents-docx"
                    | "documents-docx-file",
                ) => {
                    let preview = std::env::var("HRIS_PREVIEW_FORM").unwrap_or_default();
                    let person = if preview == "documents-manager" {
                        app.data.access.employee_id.unwrap_or(Uuid::from_u128(101))
                    } else {
                        Uuid::from_u128(101)
                    };
                    let today = app.data.company.today();
                    if preview != "documents-empty" {
                        for i in 0..4 {
                            let id = Uuid::from_u128(6000 + i);
                            let archived = i == 2;
                            let details=documents::DocumentDetails {
                                title:match i{0=>"Sample employment contract and terms for regional client services",1=>"Sample training certificate",2=>"Previous sample contract",_=>"Interrupted sample upload"}.into(),
                                category:if i==1 {DocumentCategory::Certificate}else{DocumentCategory::Contract},
                                notes:if i==0 {"Fictional preview only. Original signed scan retained; metadata changes do not replace the file."}else{"Fictional preview only."}.into(),
                                issued_on:Some(today-chrono::Duration::days(60)),expires_on:Some(today+chrono::Duration::days(if i==1 {-1}else{14})),
                                contract_start:if i==1{None}else{Some(today-chrono::Duration::days(30))},contract_end:if i==1{None}else{Some(today+chrono::Duration::days(14))},
                            };
                            let doc = documents::EmployeeDocument {
                                id,
                                company_id: app.data.company.id,
                                employee_id: person,
                                details,
                                original_filename: format!(
                                    "Fictional sample {}.{}",
                                    i + 1,
                                    if preview == "documents-docx-file" && i == 0 {
                                        "docx"
                                    } else {
                                        "pdf"
                                    }
                                ),
                                mime_type: if preview == "documents-docx-file" && i == 0 {
                                    documents::DOCX_MIME
                                } else {
                                    "application/pdf"
                                }
                                .into(),
                                byte_size: 120000,
                                sha256: "a".repeat(64),
                                object_name: documents::object_name(
                                    app.data.company.id,
                                    person,
                                    id,
                                    if preview == "documents-docx-file" && i == 0 {
                                        documents::DOCX_MIME
                                    } else {
                                        "application/pdf"
                                    },
                                )
                                .unwrap(),
                                state: if i == 3 {
                                    DocumentState::Pending
                                } else {
                                    DocumentState::Ready
                                },
                                archived,
                                revision: if archived { 2 } else { 1 },
                                created_by: app.data.access.user_id,
                                created_by_name: "Alex Rivera".into(),
                                created_at: chrono::Utc::now() - chrono::Duration::days(i as i64),
                                updated_at: chrono::Utc::now(),
                            };
                            if i < 3 {
                                app.data.document_events.push(documents::DocumentEvent {
                                    id: Uuid::new_v4(),
                                    sequence: i as i64 + 1,
                                    company_id: doc.company_id,
                                    employee_id: person,
                                    document_id: id,
                                    kind: "uploaded".into(),
                                    actor_id: doc.created_by,
                                    actor_name: doc.created_by_name.clone(),
                                    actor_role: Role::Administrator,
                                    before_record: None,
                                    after_record: doc.clone(),
                                    recorded_at: doc.created_at,
                                });
                            }
                            app.data.documents.push(doc);
                        }
                    }
                    app.open_employee(person);
                    if app.data.access.allows(crate::staff_roles::Capability::Personnel) {
                        app.contacts_section = ContactSection::Documents;
                    }
                    if preview == "documents-archived" {
                        app.show_archived_documents = true;
                    }
                    if matches!(
                        preview.as_str(),
                        "documents-upload"
                            | "documents-contract"
                            | "documents-error"
                            | "documents-docx"
                    ) {
                        let mut draft = DocumentDraft::new(person);
                        if preview != "documents-upload" {
                            draft.title = "Fictional employment contract".into();
                            draft.category = DocumentCategory::Contract;
                            draft.contract_start = today.to_string();
                            draft.contract_end = (today + chrono::Duration::days(365)).to_string();
                            draft.file = Some(std::path::PathBuf::from("Fictional contract.pdf"));
                            if preview == "documents-docx" {
                                draft.file =
                                    Some(std::path::PathBuf::from("Fictional contract.docx"));
                            }
                        }
                        if preview == "documents-error" {
                            app.notice = Some((
                                "This document changed. Refresh and reopen it before saving."
                                    .into(),
                                true,
                            ));
                        }
                        app.document_form = Some(draft);
                    } else if preview == "documents-edit" {
                        app.document_form = Some(DocumentDraft::existing(&app.data.documents[0]));
                    }
                }
                Ok(
                    "history-baseline" | "history-updated" | "history-created" | "history-archived"
                    | "history-long" | "history-empty" | "history-manager",
                ) => {
                    let preview = std::env::var("HRIS_PREVIEW_FORM").unwrap_or_default();
                    let mut store = DemoStore {
                        data: app.data.clone(),
                    };
                    let mut target = Uuid::from_u128(101);
                    if preview == "history-created" {
                        let draft = EmployeeDraft {
                            employee_code: "QA-HISTORY-PREVIEW".into(),
                            full_name: "Synthetic New Employee".into(),
                            department: "Operations".into(),
                            job_title: "Associate".into(),
                            annual_leave_days: 15,
                            ..Default::default()
                        };
                        let _ = store.execute(Action::SaveEmployee(draft));
                        target = store.data.employees.last().unwrap().id;
                    } else if matches!(preview.as_str(), "history-updated" | "history-long") {
                        let mut draft = EmployeeDraft::from(
                            store
                                .data
                                .employees
                                .iter()
                                .find(|e| e.id == target)
                                .unwrap(),
                        );
                        draft.department = if preview == "history-long" {
                            "Regional client services and administrative operations for the northern district".into()
                        } else {
                            "Client Support".into()
                        };
                        draft.job_title = if preview == "history-long" {
                            "Senior specialist for regional client service coordination and workforce planning".into()
                        } else {
                            "Senior associate".into()
                        };
                        draft.work_location_id = Some(Uuid::from_u128(2000));
                        let _ = store.execute(Action::SaveEmployee(draft));
                    } else if preview == "history-archived" {
                        target = Uuid::from_u128(102);
                        let mut draft = EmployeeDraft::from(
                            store
                                .data
                                .employees
                                .iter()
                                .find(|e| e.id == target)
                                .unwrap(),
                        );
                        draft.status = EmployeeStatus::Archived;
                        let _ = store.execute(Action::SaveEmployee(draft));
                    } else if preview == "history-manager" {
                        target = store.data.access.employee_id.unwrap_or(target);
                    }
                    if preview == "history-empty" {
                        store.data.employment_history.clear();
                    }
                    app.data = store.data;
                    app.open_employee(target);
                    if app.data.access.allows(crate::staff_roles::Capability::Personnel) {
                        app.contacts_section = ContactSection::History;
                    }
                }
                _ => {}
            }
            app.prepare_leave_preview();
            app.prepare_leave_credits_preview();
            app.prepare_dated_leave_preview();
            app.prepare_approval_delegation_preview();
            app.prepare_shift_preview();
            app.prepare_break_preview();
            app.prepare_calculation_preview();
            app.prepare_request_preview();
            app.prepare_overtime_preview();
            app.prepare_reports_preview();
            app.prepare_import_preview();
            app.prepare_attendance_import_preview();app.prepare_leave_request_import_preview();
            app.prepare_notifications_preview();
            app.prepare_hiring_preview();
            app.prepare_recruiting_preview();
            app.prepare_onboarding_preview();
            app.prepare_exit_preview();
            app.prepare_performance_preview();
            app.prepare_learning_preview();
            app.prepare_career_preview();
            app.prepare_payroll_preview();
            if std::env::var("HRIS_PREVIEW_PAGE").as_deref() == Ok("sign-in") {
                app.login_branding = branding::LoginBranding {
                    company_name: app.data.company.name.clone(),
                    logo_png: app.data.logo_png.clone(),
                };
                app.login_logo = app.company_logo.clone();
                app.data = Snapshot::signed_out();
                app.account = None;
                app.connection = Some(ConnectionConfig {
                    url: "https://preview.example.invalid".into(),
                    publishable_key: "sb_publishable_preview_fixture_only".into(),
                });
                app.deployment_error = None;
            }
        }
        // Keep the same constructor in production and screenshot builds.
        app.check_in_employee = app
            .data
            .active()
            .find(|e| app.data.access.can_record(e.id))
            .map(|e| e.id);
        app
    }

    fn send(&mut self, command: Command) {
        if self.busy {
            return;
        }
        match self.commands.send(command) {
            Ok(()) => {
                self.busy = true;
                self.notice = None;
            }
            Err(_) => {
                self.notice = Some((
                    "The background worker stopped. Restart the app.".into(),
                    true,
                ))
            }
        }
    }

    fn receive(&mut self, ctx: &egui::Context) {
        while let Ok(update) = self.updates.try_recv() {
            if let Some(state) = update.update_state {
                if let UpdateState::Failed(message) = &state {
                    if !update.background {
                        self.notice = Some((message.clone(), true));
                    }
                }
                if matches!(state, UpdateState::Restarting) {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
                self.update_state.accept(state);
            }
            if update.background {
                continue;
            }
            self.busy = false;
            match update.result {
                Ok(data) => {
                    if self
                        .leave_state
                        .saving_revision
                        .is_some_and(|r| data.leave_setup.policy.revision == r + 1)
                    {
                        self.leave_state.draft = None;
                        self.leave_state.saving_revision = None;
                    }
                    if self.data.access != data.access {
                        self.staff_roles_state=Default::default();
                        self.staff_provision_state=Default::default();
                        self.organization = organization_view::State::default();
                        self.leave_state = leave_view::State::default();
                        self.shift_state = shifts_view::State::default();
                        self.break_state = breaks_view::State::default();
                        self.calculation_state = calculations_view::State::default();
                        self.request_state = requests_view::State::default();
                        self.overtime_state = overtime_view::State::default();
                        self.report_state = reports_view::State::new(data.company.today());
                        self.notification_state = notifications_view::State::default();
                        self.hiring_state = hiring_view::State::default();
                        self.recruiting_state = Default::default();
                        self.onboarding_state = Default::default();
                        self.exit_state = Default::default();
                        self.performance_state = Default::default();
                        self.learning_state = Default::default();
                        self.career_state = Default::default();
                        self.payroll_state = Default::default();
                        self.import_state = imports_view::State::default();
                        self.attendance_import_state=attendance_import_view::State::default();self.leave_request_import_state=leave_request_import_view::State::default();
                        self.employee_view = EmployeeView::Directory;
                        self.document_form = None;
                        self.account_form = None;
                        self.employee_form = None;
                        self.contacts_form = None;
                        self.leave_form = None;
                        self.correction_form = None;
                        self.employment_option_form = None;
                        self.correction_history = None;
                        clear_edit_history(
                            ctx,
                            &["account-initial-password", "account-initial-confirm"],
                        );
                    }
                    if self.data.mode != data.mode {
                        self.organization = organization_view::State::default();
                        self.leave_state = leave_view::State::default();
                        self.shift_state = shifts_view::State::default();
                        self.break_state = breaks_view::State::default();
                        self.calculation_state = calculations_view::State::default();
                        self.request_state = requests_view::State::default();
                        self.overtime_state = overtime_view::State::default();
                        self.report_state = reports_view::State::new(data.company.today());
                        self.notification_state = notifications_view::State::default();
                        self.hiring_state = hiring_view::State::default();
                        self.recruiting_state = Default::default();
                        self.onboarding_state = Default::default();
                        self.exit_state = Default::default();
                        self.performance_state = Default::default();
                        self.learning_state = Default::default();
                        self.career_state = Default::default();
                        self.payroll_state = Default::default();
                        self.import_state = imports_view::State::default();
                        self.attendance_import_state=attendance_import_view::State::default();self.leave_request_import_state=leave_request_import_view::State::default();
                        self.employee_view = EmployeeView::Directory;
                        self.document_form = None;
                        self.clear_profile_secrets(ctx);
                        self.session_confirmation = None;
                        self.profile_section = ProfileSection::Employment;
                        self.page = Page::Overview;
                        self.search.clear();
                        self.department.clear();
                        self.employment_type_filter = EmploymentFilter::All;
                        self.work_location_filter = EmploymentFilter::All;
                        self.employee_status = Some(EmployeeStatus::Active);
                        self.leave_filter = None;
                        self.attendance_date = data.company.today();
                        self.calendar_month = month_start(self.attendance_date);
                        self.check_in_employee = data.active().next().map(|employee| employee.id);
                        self.employee_form = None;
                        self.contacts_form = None;
                        self.leave_form = None;
                        self.correction_form = None;
                        self.employment_option_form = None;
                        self.correction_history = None;
                    }
                    if let Some(id) = self.import_state.preview.as_ref().map(|p| p.id) {
                        if data.imports.batches.iter().any(|b| b.id == id) {
                            self.import_state.preview = None;
                            self.import_state.reason.clear();
                            self.import_state.batch = Some(id);
                        }
                    }
                    self.import_state.applying = false;
                    if let Some(report)=update.government_bir_declarations{self.payroll_state.government.accept_declarations(report,&data);}
                    if let Some(report)=update.government_bir_identities{self.payroll_state.government.accept_identities(report,&data);}
                    if let Some(ledger)=update.government_annual{self.payroll_state.government.accept_annual(ledger,&data);}
                    if let Some(report)=update.government_annual_pay{self.payroll_state.government.accept_annual_pay(report,&data);}
                    if let Some(basis)=update.government_sss{self.payroll_state.government.accept_collection(basis,&data);}
                    if let Some(report)=update.government_bir{self.payroll_state.government.accept_bir(report,&data);}
                    if let Some(report)=update.government_bir_build{self.payroll_state.government.accept_bir_build(report,&data);}
                    if let Some(record)=update.government_registration{self.payroll_state.government.accept_registration(record,&data);}
                    if let Some(report)=update.government{self.payroll_state.government.accept(report,&data);}
                    if let Some(report)=update.payroll_report{self.payroll_state.reports.accept(report,&data);}
                    if let Some(exchange)=update.payroll_exchange{self.payroll_state.exchange.accept(exchange,&data);}
                    if let Some(report) = update.report {
                        if report.filter == self.report_state.filter {
                            self.report_state.result = Some(report);
                        }
                    }
                    if data.mode == Mode::Supabase && data.logo_warning.is_none() {
                        let next = branding::LoginBranding {
                            company_name: data.company.name.clone(),
                            logo_png: data.logo_png.clone(),
                        };
                        if self.login_branding != next || self.login_branding_warning.is_some() {
                            self.login_logo = next
                                .logo_png
                                .as_ref()
                                .and_then(|png| branding::circular_logo(png).ok())
                                .map(|image| {
                                    ctx.load_texture(
                                        "login-company-logo",
                                        image,
                                        egui::TextureOptions::LINEAR,
                                    )
                                });
                            self.login_branding_warning = self.connection.as_ref().and_then(|config|
                                branding::save_login_branding(config,&next).err().map(|_|"The company logo is saved online, but sign-in branding on this device could not be refreshed. Refresh to retry.".into()));
                            self.login_branding = next;
                        }
                    }
                    sync_profile_contacts(&self.data, &data, &mut self.profile_contacts_draft);
                    if self.data.logo_png != data.logo_png || self.data.mode != data.mode {
                        self.company_logo = data
                            .logo_png
                            .as_ref()
                            .and_then(|png| branding::circular_logo(png).ok())
                            .map(|image| {
                                ctx.load_texture(
                                    "company-logo",
                                    image,
                                    egui::TextureOptions::LINEAR,
                                )
                            });
                    }
                    if self.data.company != data.company {
                        if company_fields_equal(&self.company_draft, &self.data.company) {
                            self.company_draft = data.company.clone();
                        } else {
                            self.company_draft.logo_version = data.company.logo_version;
                        }
                    }
                    let hiring_record_saved = self.hiring_state.accept(&data);
                    let recruiting_saved = self.recruiting_state.accept(&data);
                    let onboarding_saved = self.onboarding_state.accept(&data);
                    let exit_saved = self.exit_state.accept(&data);
                    let performance_saved = self.performance_state.accept(&data);
                    let learning_saved = self.learning_state.accept(&data);
                    let payroll_saved = self.payroll_state.accept(&data);
                    let career_saved = self.career_state.accept(&data);
                    let hiring_saved = hiring_record_saved && recruiting_saved && onboarding_saved && exit_saved && performance_saved && learning_saved && career_saved && payroll_saved;
                    self.data = data;
                    let mut staff_update_error=None;
                    if let Some(roles)=update.staff_roles {
                        if let Err(error)=self.staff_roles_state.accept(&self.data.access,roles) {staff_update_error=Some(error.to_string());}
                    }
                    if let Some(provision)=update.staff_provision {
                        if let Err(error)=self.accept_staff_provision(provision) {staff_update_error=Some(error.to_string());}
                    }
                    self.leave_request_import_state.sync(&self.data);
                    if let Some(p)=update.leave_import_preview{self.leave_request_import_state.accept_preview(p,&self.data);}
                    if let Some(r)=update.leave_import_receipt{self.leave_request_import_state.accept_receipt(r,&self.data);}
                    if let Some(h)=update.leave_import_history{self.leave_request_import_state.accept_history(h,&self.data);}
                    self.attendance_import_state.sync(&self.data);
                    if let Some(p)=update.attendance_import_preview{self.attendance_import_state.accept_preview(p,&self.data);}
                    if let Some(r)=update.attendance_import_receipt{self.attendance_import_state.accept_receipt(r,&self.data);}
                    if let Some(h)=update.attendance_import_history{self.attendance_import_state.accept_history(h,&self.data);}
                    if let Some(preview)=update.leave_opening_preview {self.leave_state.openings.accept(preview);}
                    if let Some(preview)=update.leave_opening_import_preview {self.leave_state.openings.accept_import(preview,&self.data);}
                    self.notification_state.invalidate();
                    if !self.data.active().any(|e| {
                        Some(e.id) == self.check_in_employee && self.data.access.can_record(e.id)
                    }) {
                        self.check_in_employee = self
                            .data
                            .active()
                            .find(|e| self.data.access.can_record(e.id))
                            .map(|e| e.id);
                    }
                    if self.break_state.saving {
                        self.break_state.form = None;
                        self.break_state.void_target = None;
                        self.break_state.reason.clear();
                        self.break_state.saving = false;
                    }
                    if self.overtime_state.saving {
                        self.overtime_state.form = None;
                        self.overtime_state.decision = None;
                        self.overtime_state.reason.clear();
                        self.overtime_state.saving = false;
                    }
                    if self.request_state.saving {
                        self.request_state.form = None;
                        self.request_state.resolution = None;
                        self.request_state.reason.clear();
                        self.request_state.saving = false;
                    }
                    if self.calculation_state.saving {
                        self.calculation_state.draft = None;
                        self.calculation_state.apply = None;
                        self.calculation_state.saving = false;
                    }
                    if self.shift_state.saving {
                        self.shift_state.template = None;
                        self.shift_state.assignment = None;
                        self.shift_state.saving = false;
                    }
                    if self.close_on_success {
                        self.document_form = None;
                        self.employee_form = None;
                        self.contacts_form = None;
                        self.leave_form = None;
                        self.correction_form = None;
                        self.employment_option_form = None;
                        self.correction_history = None;
                        self.account_form = None;
                    }
                    if self.close_document_on_success {
                        self.document_form = None;
                    }
                    if let Some(error)=staff_update_error {self.notice=Some((error,true));}
                    else if !hiring_saved {
                        self.notice = Some(("The saved record has changed. Your draft is preserved; reload the saved record before continuing.".into(), true));
                    } else if !update.message.is_empty() {
                        self.notice = Some((update.message, false));
                    }
                }
                Err(error) => {
                    if update.staff_role_rejected {self.staff_roles_state.reject();}
                    if let Some(rejection)=update.staff_provision_rejection {let _=self.staff_provision_state.reject(&rejection);}
                    if !update.background{self.attendance_import_state.failed(&error);self.leave_request_import_state.failed(&error);}
                    self.hiring_state.pending = None;
                    self.onboarding_state.pending = None;
                    self.exit_state.pending = None;
                    self.performance_state.pending = None;
                    self.learning_state.pending = None;
                    self.career_state.pending = None;
                    self.payroll_state.pending = None;
                    self.payroll_state.government.pending = None;
                    self.payroll_state.government.registration.pending = false;
                    self.payroll_state.government.collection_export_pending=false;
                    self.payroll_state.government.bir.failed();
                    self.payroll_state.government.builder.failed();
                    self.payroll_state.government.annual.failed();
                    self.payroll_state.government.identities.failed();
                    self.payroll_state.government.declarations.failed();
                    self.recruiting_state.pending = None;
                    self.recruiting_state.resume_pending = None;
                    self.report_state.pending_views = None;
                    self.overtime_state.saving = false;
                    self.leave_state.saving_revision = None;
                    self.shift_state.saving = false;
                    self.break_state.saving = false;
                    self.calculation_state.saving = false;
                    self.request_state.saving = false;
                    self.notice = Some((error, true));
                }
            }
            if update.account != self.account {
                let photo_changed = update.account.as_ref().and_then(|a| a.photo.as_ref())
                    != self.account.as_ref().and_then(|a| a.photo.as_ref());
                if photo_changed {
                    self.avatar = update
                        .account
                        .as_ref()
                        .and_then(|a| a.photo.as_ref())
                        .and_then(|photo| account::circular_photo(photo).ok())
                        .map(|image| {
                            ctx.load_texture("profile-avatar", image, egui::TextureOptions::LINEAR)
                        });
                }
                if sync_account_drafts(
                    self.account.as_ref(),
                    update.account.as_ref(),
                    &mut self.personal_draft,
                    &mut self.display_draft,
                ) {
                    ctx.set_zoom_factor(self.display_draft.scale_percent as f32 / 100.0);
                }
                self.account = update.account;
            }
            if let Some(account) = &self.account {
                self.report_state.accept_account(&account.report_views);
            }
            self.close_on_success = false;
            self.close_document_on_success = false;
            if self.page == Page::Accounts && !self.data.access.role.manages_accounts() {
                self.page = Page::Overview;
            }
        }
    }

    fn clear_profile_secrets(&mut self, ctx: &egui::Context) {
        self.current_password.clear();
        self.new_password.clear();
        self.confirm_password.clear();
        self.email_password.clear();
        self.show_passwords = false;
        clear_edit_history(
            ctx,
            &[
                "profile-current-password",
                "profile-new-password",
                "profile-confirm-password",
                "profile-email-password",
                "account-initial-password",
                "account-initial-confirm",
            ],
        );
    }

    fn navigate(&mut self, page: Page, ctx: &egui::Context) {
        if self.page != page {
            self.clear_profile_secrets(ctx);
            self.session_confirmation = None;
            self.account_form = None;
            self.page = page;
        }
    }

    fn time_format(&self) -> &'static str {
        self.account
            .as_ref()
            .map(|a| a.display)
            .unwrap_or_default()
            .time_format()
    }

    fn new_employee(&mut self) {
        if !self.data.access.allows(crate::staff_roles::Capability::Personnel) {
            return;
        }
        self.contacts_form = Some(EmployeeContacts::empty(self.data.company.id, Uuid::nil()));
        self.contacts_section = ContactSection::Employment;
        self.employee_form = Some(EmployeeDraft {
            employee_code: format!("EMP-{:06}", self.data.employees.len() + 1),
            annual_leave_days: self.data.company.annual_leave_days,
            ..Default::default()
        });
    }

    fn open_employee(&mut self, id: Uuid) {
        self.document_form = None;
        self.document_search.clear();
        self.document_category = None;
        self.show_archived_documents = false;
        self.show_discarded_documents = false;
        self.account_form = None;
        let Some(employee) = self.data.employees.iter().find(|e| e.id == id) else {
            return;
        };
        self.employee_form = Some(EmployeeDraft::from(employee));
        self.contacts_form = Some(
            self.data
                .contacts
                .iter()
                .find(|c| c.employee_id == id)
                .cloned()
                .unwrap_or_else(|| EmployeeContacts::empty(self.data.company.id, id)),
        );
        self.contacts_section = ContactSection::Employment;
        self.notice = None;
    }

    #[cfg(feature = "qa-screenshot")]
    fn open_contacts(&mut self, id: Uuid) {
        self.open_employee(id);
        self.contacts_section = ContactSection::Contact;
    }

    fn employee_record(&mut self, ui: &mut Ui) {
        let Some(mut draft) = self.employee_form.take() else {
            return;
        };
        let mut contacts = self.contacts_form.take().unwrap_or_else(|| {
            EmployeeContacts::empty(self.data.company.id, draft.id.unwrap_or_default())
        });
        let mut access = self.account_form.take();
        let versioned_staff=self.data.access.staff_context.as_ref().is_some_and(|c|c.runtime_ready);
        if versioned_staff{access=None;}
        let mut transferred=false;
        let editable = self.data.access.allows(crate::staff_roles::Capability::Personnel);
        let private_visible = editable
            || draft
                .id
                .is_some_and(|id| self.data.access.can_read_contacts(id));
        let mut close = false;
        let mut save = false;
        card(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                close = ui
                    .button(
                        if self.page == Page::Employees
                            && self.employee_view == EmployeeView::Organization
                        {
                            "< Back to chart"
                        } else {
                            "< Back"
                        },
                    )
                    .clicked();
                ui.label(
                    RichText::new(if draft.full_name.is_empty() {
                        "New employee"
                    } else {
                        &draft.full_name
                    })
                    .size(20.0)
                    .strong(),
                );
            });
            ui.horizontal_wrapped(|ui| {
                ui.selectable_value(
                    &mut self.contacts_section,
                    ContactSection::Employment,
                    "Employment",
                );
                if private_visible {
                    ui.selectable_value(
                        &mut self.contacts_section,
                        ContactSection::Contact,
                        "Contact details",
                    );
                    ui.selectable_value(
                        &mut self.contacts_section,
                        ContactSection::Emergency,
                        "Emergency contact",
                    );
                }
                if self.data.access.role.manages_accounts() {
                    ui.selectable_value(
                        &mut self.contacts_section,
                        ContactSection::Access,
                        "Staff role & sign-in",
                    );
                }
                if editable && draft.id.is_some() {
                    ui.selectable_value(
                        &mut self.contacts_section,
                        ContactSection::Documents,
                        "Documents / 201 files",
                    );
                    ui.selectable_value(
                        &mut self.contacts_section,
                        ContactSection::History,
                        "History",
                    );
                }
            });
            ui.separator();
            ui.set_max_width(ui.available_width().min(
                if matches!(
                    self.contacts_section,
                    ContactSection::History | ContactSection::Documents
                ) {
                    920.0
                } else {
                    680.0
                },
            ));
            let body_height =
                (ui.available_height() - if editable { 90.0 } else { 130.0 }).max(80.0);
            let scroll = egui::ScrollArea::vertical()
                .id_salt("employee_record_body")
                .max_height(body_height);
            #[cfg(feature = "qa-screenshot")]
            let scroll = scroll
                .animated(false)
                .stick_to_bottom(std::env::var("HRIS_PREVIEW_SCROLL").as_deref() == Ok("bottom"));
            scroll.show(ui,|ui| {
                ui.add_enabled_ui(editable && !self.busy,|ui|match self.contacts_section {
                    ContactSection::Employment=>employment_fields(ui,&mut draft,&self.data),
                    ContactSection::Contact if private_visible=>contact_fields(ui,&mut contacts,false),
                    ContactSection::Emergency if private_visible=>contact_fields(ui,&mut contacts,true),
                    ContactSection::History if editable=>{
                        if let Some(id)=draft.id { employment_history(ui,&self.data,id,self.time_format()); }
                    }
                    ContactSection::Documents if editable=>{
                        if let Some(id)=draft.id { self.employee_documents(ui,id); }
                    }
                    ContactSection::Access if versioned_staff&&self.data.access.role.manages_accounts()=>{
                        transferred=self.staff_employee_access(ui,&draft,&contacts);
                    }
                    ContactSection::Access if self.data.access.role.manages_accounts()=>{
                        if access.is_none() {
                            if let Some(existing)=self.data.accounts.iter().find(|a|a.employee_id==draft.id && draft.id.is_some()) {
                                access=Some(AccountDraft::from_account(existing));
                            }
                        }
                        if let Some(access)=access.as_mut() {staff_access_fields(ui,access,&self.data);}
                        else {
                            ui.label("No staff role is assigned. This employee does not use the desktop app.");
                            ui.label("Assign a staff role here when this person needs desktop access.");
                            if ui.button("Assign staff role").clicked() {
                                let mut added=AccountDraft::new();added.role=Role::HrGeneralist;added.employee_id=draft.id;access=Some(added);
                            }
                        }
                    }
                    _=>{}
                });
            });
            ui.add_space(12.0);
            ui.horizontal_wrapped(|ui| {
                if editable && self.document_form.is_none() {
                    save = primary(
                        ui,
                        if access.is_some() {
                            "Save employee & role"
                        } else if self.contacts_section == ContactSection::Documents {
                            "Save employee changes"
                        } else {
                            "Save changes"
                        },
                    );
                }
                close |= ui
                    .button(
                        if editable && self.contacts_section != ContactSection::Documents {
                            "Cancel"
                        } else {
                            "Close record"
                        },
                    )
                    .clicked();
            });
            if !editable {
                ui.label("Employment is managed by HR. Update your own contact and emergency details in Profile.");
            }
        });
        if save {
            match draft
                .employee(self.data.company.id)
                .and_then(|_| contacts.normalized())
            {
                Ok(normalized) => {
                    contacts = normalized.clone();
                    if let Some(mut role) = access.take() {
                        role.employee_name = draft.full_name.trim().into();
                        role.employee_id = draft.id;
                        match role.validate(&self.data) {
                            Ok(()) => {
                                access = Some(public_access_draft(&role));
                                clear_edit_history(
                                    ui.ctx(),
                                    &["account-initial-password", "account-initial-confirm"],
                                );
                                self.close_on_success = true;
                                self.send(Command::Admin(AdminAction::SaveStaff(
                                    role,
                                    draft.clone(),
                                    normalized,
                                )));
                            }
                            Err(error) => {
                                self.notice = Some((error.to_string(), true));
                                access = Some(role);
                            }
                        }
                    } else {
                        self.close_on_success = true;
                        self.send(Command::Change(Action::SaveEmployeeRecord(
                            draft.clone(),
                            normalized,
                        )));
                    }
                }
                Err(error) => self.notice = Some((error.to_string(), true)),
            }
        }
        if (!close || self.busy) && !transferred {
            self.employee_form = Some(draft);
            self.contacts_form = Some(contacts);
            self.account_form = access;
        } else {
            self.document_form = None;
            clear_edit_history(
                ui.ctx(),
                &["account-initial-password", "account-initial-confirm"],
            );
        }
    }

    fn new_leave(&mut self) {
        let today = self.data.company.today();
        let start = (0..7)
            .map(|i| today + Duration::days(i))
            .find(|d| d.weekday().number_from_monday() <= 5)
            .unwrap_or(today);
        self.leave_form = Some(LeaveDraft {
            options: Default::default(),
            employee_id: self
                .data
                .active()
                .find(|e| self.data.access.can_record(e.id))
                .map(|e| e.id),
            starts_on: start.to_string(),
            ends_on: start.to_string(),
            ..Default::default()
        });
    }

    fn sidebar(&mut self, ui: &mut Ui) {
        let compact = ui.available_height() < 600.0;
        ui.add_space(if compact { 4.0 } else { 12.0 });
        ui.vertical_centered(|ui| {
            company_logo(
                ui,
                self.company_logo.as_ref(),
                &self.data.company.name,
                if compact { 54.0 } else { 72.0 },
            );
            ui.add_space(if compact { 6.0 } else { 10.0 });
            ui.add(
                egui::Label::new(
                    RichText::new(&self.data.company.name)
                        .color(Color32::WHITE)
                        .size(18.0)
                        .strong(),
                )
                .wrap(),
            );
        });
        ui.add_space(if compact { 14.0 } else { 28.0 });
        let nav_height = (ui.available_height() - 112.0).max(70.0);
        let page_count = Page::ALL.iter().filter(|p| **p != Page::Accounts || self.data.access.role.manages_accounts()).count() as f32;
        let nav_gap = if compact { 4.0 } else { ui.spacing().item_spacing.y };
        let button_height = ((nav_height - nav_gap * (page_count - 1.0)) / page_count).clamp(28.0, 42.0);
        egui::ScrollArea::vertical()
            .id_salt("sidebar_navigation")
            .max_height(nav_height)
            .show(ui, |ui| {
                if compact {
                    ui.spacing_mut().item_spacing.y = 4.0;
                    ui.spacing_mut().button_padding.y = 3.0;
                    ui.spacing_mut().interact_size.y = 28.0;
                }
                for page in Page::ALL {
                    if page == Page::Accounts && !self.data.access.role.manages_accounts() {
                        continue;
                    }
                    if page == Page::Hiring && !self.data.access.uses_desktop() {
                        continue;
                    }
                    let selected = self.page == page;
                    let label = RichText::new(page.title()).size(15.0).color(if selected {
                        Color32::WHITE
                    } else {
                        Color32::from_rgb(175, 193, 207)
                    });
                    let button = egui::Button::new(label)
                        .fill(if selected { TEAL } else { Color32::TRANSPARENT })
                        .stroke(Stroke::NONE)
                        .corner_radius(8);
                    if ui
                        .add_sized(
                            [ui.available_width(), button_height],
                            button,
                        )
                        .clicked()
                    {
                        self.navigate(page, ui.ctx());
                    }
                }
            });
        ui.with_layout(Layout::bottom_up(Align::Min), |ui| {
            ui.add_space(4.0);
            ui.label(
                RichText::new(if self.data.mode == Mode::Demo {
                    "DEMO · NOT SAVED"
                } else {
                    "CONNECTED"
                })
                .color(Color32::from_rgb(115, 214, 197))
                .size(11.0)
                .strong(),
            );
            let name = self
                .data
                .own_employee()
                .map(|e| e.full_name.as_str())
                .or_else(|| self.account.as_ref().map(|a| a.details.display_name()))
                .unwrap_or("Your account");
            let role = if self.data.mode == Mode::Demo {
                "Demo account".to_owned()
            } else {
                self.data.access.role_label()
            };
            if profile_link(
                ui,
                self.avatar.as_ref(),
                name,
                &role,
                self.page == Page::Profile,
            )
            .clicked()
            {
                self.navigate(Page::Profile, ui.ctx());
            }
            ui.separator();
        });
    }

    fn header(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            ui.label(
                RichText::new(self.page.title().to_uppercase())
                    .color(muted(ui))
                    .size(11.0)
                    .strong(),
            );
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.label(
                    RichText::new(format!("v{}", updater::VERSION))
                        .small()
                        .color(muted(ui)),
                );
                if self.busy {
                    ui.spinner();
                    ui.label(RichText::new("Working…").color(muted(ui)));
                } else if ui.button("Refresh").clicked() {
                    self.send(Command::Refresh);
                }
                self.notification_state.current(&self.data);
                let unread = self.notification_state.unread;
                if ui
                    .add_enabled(
                        !self.busy && !self.has_unsaved_edits(),
                        egui::Button::new(format!("Notifications ({unread})")),
                    )
                    .on_hover_text("Save or close open editors before opening Notifications.")
                    .clicked()
                {
                    self.navigate(Page::Notifications, ui.ctx());
                }
                ui.label(
                    RichText::new(self.data.company.today().format("%b %d, %Y").to_string())
                        .color(muted(ui)),
                );
            });
        });
        ui.add_space(14.0);
        self.update_banner(ui);
        let (title, subtitle) = match self.page {
            Page::Overview => (
                "Your people, at a glance",
                "A clear view of your team and today's HR activity.",
            ),
            Page::Employees => (
                if self.employee_form.is_some() {
                    "Employee record"
                } else if self.employee_view == EmployeeView::Organization {
                    "Organization chart"
                } else if self.employee_view == EmployeeView::Development {
                    "Development"
                } else {
                    "Employee directory"
                },
                if self.employee_form.is_some() {
                    "Employment, reporting and private contacts in one record."
                } else if self.employee_view == EmployeeView::Organization {
                    "Explore reporting relationships and open employee records."
                } else if self.employee_view == EmployeeView::Development {
                    "Goals, appraisals, learning and career development."
                } else {
                    "Click an employee to open their record."
                },
            ),
            Page::Attendance => (
                if self.break_state.detail.is_some() {
                    "Attendance breaks"
                } else {
                    match self.shift_state.tab {
                        shifts_view::Tab::Daily => "Daily attendance",
                        shifts_view::Tab::Shifts => "Shift schedules",
                        shifts_view::Tab::Assignments => "Employee schedules",
                        shifts_view::Tab::History => "Schedule history",
                        shifts_view::Tab::Calculations => "Time calculations",
                        shifts_view::Tab::Requests => "Correction requests",
                        shifts_view::Tab::Overtime => "Overtime approvals",
                    }
                },
                "Review attendance and manage employee work schedules.",
            ),
            Page::Leave => (
                "Time away, organized",
                "Manage requests, leave policies and ordered approvals.",
            ),
            Page::Hiring if !self.data.access.allows(crate::staff_roles::Capability::Recruiting) => (
                "Your hiring tasks",
                "Review assigned interviews, record assessments and complete assigned onboarding tasks.",
            ),
            Page::Hiring => (
                "Hiring",
                "Manage applications, interviews and hiring workflows.",
            ),
            Page::Exits => ("Exit & clearance", "Manage employee handover, clearance and protected exit finalization."),
            Page::Payroll => ("Payroll & benefits", "Reviewed Philippine pay, private salary records and approved payslips."),
            Page::Reports => (
                "Reports",
                "Attendance, filed leave charges and employee totals within your access.",
            ),
            Page::Accounts => (
                "Account Manager",
                "Manage employee sign-in, roles and account access.",
            ),
            Page::Settings if !self.data.access.role.manages_accounts() => {
                ("Settings", "Manage your display preferences.")
            }
            Page::Settings => (
                "Settings",
                "Manage company defaults and your display preferences.",
            ),
            Page::Profile => (
                "Your profile",
                "Manage your personal details, account security and sessions.",
            ),
            Page::Notifications => (
                "Notifications",
                "Approval tasks, personal updates and reminders within your current access.",
            ),
        };
        let employee_open =
            matches!(self.page, Page::Employees | Page::Accounts) && self.employee_form.is_some();
        if !employee_open {
            ui.heading(RichText::new(title).strong());
            ui.label(RichText::new(subtitle).color(muted(ui)));
            ui.add_space(12.0);
        }
        if self.data.mode == Mode::Demo {
            if employee_open
                || (self.page == Page::Employees
                    && self.employee_view == EmployeeView::Organization)
            {
                ui.label(
                    RichText::new("Demo · Fictional records. Changes reset when the app closes.")
                        .color(accent(ui))
                        .small(),
                );
            } else {
                Frame::new().fill(adaptive(ui, Color32::from_rgb(232,243,240), Color32::from_rgb(24,58,54))).corner_radius(8).inner_margin(12).show(ui,|ui| {
                ui.set_width(ui.available_width());
                ui.label(RichText::new("Demo workspace · Fictional sample data. Changes reset when the app closes.").color(accent(ui)).size(12.0));
            });
            }
        }
        if let Some((message, error)) = self.notice.clone() {
            Frame::new()
                .fill(if error {
                    Color32::from_rgb(255, 235, 231)
                } else {
                    Color32::from_rgb(229, 244, 238)
                })
                .corner_radius(8)
                .inner_margin(10)
                .show(ui, |ui| {
                    ui.horizontal_wrapped(|ui| {
                        ui.label(RichText::new(message).color(if error {
                            Color32::from_rgb(160, 52, 38)
                        } else {
                            TEAL
                        }));
                        if ui.small_button("Dismiss").clicked() {
                            self.notice = None;
                        }
                    });
                });
        }
        ui.add_space(12.0);
    }

    fn has_unsaved_edits(&self) -> bool {
        self.employee_form.is_some()
            || self.document_form.is_some()
            || self.employment_option_form.is_some()
            || self.correction_form.is_some()
            || self.leave_form.is_some()
            || self.leave_state.draft.is_some()
            || self.leave_state.policy_pending.is_some()
            || self.leave_state.withdraw_target.is_some()
            || self.leave_state.openings.has_draft()
            || self.leave_state.delegation.has_draft()
            || self.shift_state.template.is_some()
            || self.shift_state.assignment.is_some()
            || self.break_state.form.is_some()
            || self.break_state.void_target.is_some()
            || self.calculation_state.editing()
            || self.request_state.editing()
            || self.overtime_state.editing()
            || self.import_state.editing()
            || self.attendance_import_state.has_draft() || self.leave_request_import_state.has_draft()
            || self.hiring_state.editing()
            || self.onboarding_state.editing()
            || self.exit_state.editing()
            || (self.performance_state.editing() || self.learning_state.editing() || self.career_state.editing())
            || self.payroll_state.editing()
            || self.payroll_state.exchange.has_draft()
            || self.recruiting_state.editing()
            || self.report_state.view_draft.is_some()
            || self.account_form.is_some()
            || self.staff_roles_state.has_draft()
            || self.staff_provision_state.has_draft()
            || !company_fields_equal(&self.company_draft, &self.data.company)
            || self.profile_contacts_draft.as_ref().is_some_and(|draft| {
                self.data.access.employee_id.is_some_and(|id| {
                    !same_contacts(
                        draft,
                        &self
                            .data
                            .own_contacts()
                            .cloned()
                            .unwrap_or_else(|| EmployeeContacts::empty(self.data.company.id, id)),
                    )
                })
            })
            || self.account.as_ref().is_some_and(|a| {
                a.details != self.personal_draft || a.display != self.display_draft
            })
            || !self.current_password.is_empty()
            || !self.new_password.is_empty()
            || !self.confirm_password.is_empty()
            || !self.new_email.is_empty()
            || !self.email_password.is_empty()
    }

    fn update_banner(&mut self, ui: &mut Ui) {
        if let UpdateState::Available(update) = self.update_state.clone() {
            card(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.label(
                        RichText::new(format!("HRIS {} is available", update.release.version))
                            .strong(),
                    );
                    let ready = !self.busy && !self.has_unsaved_edits();
                    if ui
                        .add_enabled(ready, egui::Button::new("Update now"))
                        .clicked()
                    {
                        self.update_confirmation = true;
                    }
                });
                if !update.release.notes.is_empty() {
                    let mut summary: String = update.release.notes.chars().take(160).collect();
                    if update.release.notes.chars().count() > 160 {
                        summary.push('…');
                    }
                    ui.label(summary);
                }
                if self.has_unsaved_edits() {
                    ui.label("Save or cancel your open edits before updating.");
                }
            });
            ui.add_space(10.0);
        }
    }

    fn confirm_update(&mut self, ctx: &egui::Context) {
        if !self.update_confirmation {
            return;
        }
        let UpdateState::Available(update) = self.update_state.clone() else {
            self.update_confirmation = false;
            return;
        };
        let mut apply = false;
        let mut cancel = false;
        let mut open = true;
        egui::Window::new("Update HRIS Desktop").collapsible(false).resizable(false).open(&mut open)
            .anchor(egui::Align2::CENTER_CENTER,[0.0,0.0]).default_width(430.0).show(ctx,|ui| {
                ui.label(format!("Install version {} and restart HRIS?",update.release.version));
                ui.label("The download will be verified before installation. Your company connection and logo will be preserved. You will sign in again after restarting.");
                ui.horizontal(|ui| {
                    apply=ui.add_enabled(!self.busy && !self.has_unsaved_edits(),egui::Button::new("Update and restart")).clicked();
                    cancel=ui.button("Later").clicked();
                });
            });
        if apply {
            self.update_confirmation = false;
            self.send(Command::InstallUpdate(update));
        }
        if cancel || !open {
            self.update_confirmation = false;
        }
    }

    fn overview(&mut self, ui: &mut Ui) {
        let today = self.data.company.today();
        let active = self.data.active().filter(|e|self.data.access.can_read_operational_employee(e)).count();
        let attendance = self
            .data
            .attendance
            .iter()
            .filter(|a| a.work_date == today)
            .count();
        let pending = self
            .data
            .leave
            .iter()
            .filter(|r| r.status == LeaveStatus::Pending)
            .count();
        let away = self
            .data
            .leave
            .iter()
            .filter(|r| {
                r.status == LeaveStatus::Approved && r.charges().iter().any(|d| d.date == today)
            })
            .count();
        ui.columns(4, |columns| {
            for (ui, (label, value, caption)) in columns.iter_mut().zip([
                ("ACTIVE EMPLOYEES", active, "Your attendance and leave scope"),
                ("CHECKED IN TODAY", attendance, "Daily attendance records"),
                ("AWAITING REVIEW", pending, "Pending leave requests"),
                ("ON LEAVE TODAY", away, "Approved time away"),
            ]) {
                card(ui, |ui| {
                    ui.label(RichText::new(label).color(muted(ui)).size(10.0).strong());
                    ui.label(RichText::new(value.to_string()).size(34.0).strong());
                    ui.label(RichText::new(caption).color(muted(ui)).size(11.0));
                });
            }
        });
        ui.add_space(14.0);
        card(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new("Today's attendance").size(18.0).strong());
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if ui.button("View attendance").clicked() {
                        self.page = Page::Attendance;
                    }
                });
            });
            ui.label(
                RichText::new(format!(
                    "{} · {}",
                    today.format("%A, %B %d"),
                    self.data.company.timezone
                ))
                .color(muted(ui))
                .size(12.0),
            );
            ui.add_space(8.0);
            egui::Grid::new("overview_attendance")
                .striped(true)
                .min_col_width(160.0)
                .spacing([24.0, 12.0])
                .show(ui, |ui| {
                    headers(ui, &["EMPLOYEE", "DEPARTMENT", "CHECK-IN", "STATUS"]);
                    for record in self
                        .data
                        .attendance
                        .iter()
                        .filter(|a| a.work_date == today)
                        .take(5)
                    {
                        ui.label(self.data.employee_name(record.employee_id));
                        ui.label(
                            self.data
                                .employees
                                .iter()
                                .find(|e| e.id == record.employee_id)
                                .map(|e| e.department.as_str())
                                .unwrap_or("—"),
                        );
                        ui.label(
                            record
                                .checked_in_at
                                .with_timezone(&self.data.company.tz())
                                .format(self.time_format())
                                .to_string(),
                        );
                        badge(
                            ui,
                            if record.checked_out_at.is_none() {
                                "Checked in"
                            } else {
                                "Completed"
                            },
                            true,
                        );
                        ui.end_row();
                    }
                });
            if attendance == 0 {
                empty(ui, "No check-ins yet today.");
            }
        });
        ui.add_space(14.0);
        card(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new("Pending leave requests").size(18.0).strong());
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if ui.button("Open leave").clicked() {
                        self.page = Page::Leave;
                        self.leave_filter = Some(LeaveStatus::Pending);
                    }
                });
            });
            for row in self
                .data
                .leave
                .iter()
                .filter(|r| r.status == LeaveStatus::Pending)
                .take(3)
            {
                ui.separator();
                ui.horizontal(|ui| {
                    ui.label(RichText::new(self.data.employee_name(row.employee_id)).strong());
                    ui.label(
                        RichText::new(format!(
                            "{} · {} · {} workday(s)",
                            row.kind.label(),
                            row.starts_on,
                            weekdays(row.starts_on, row.ends_on)
                        ))
                        .color(muted(ui)),
                    );
                    badge(ui, "Pending", false);
                });
            }
            if pending == 0 {
                empty(ui, "You're all caught up. No pending requests.");
            }
        });
        ui.add_space(10.0);
        ui.horizontal(|ui| {
            if self.data.access.allows(crate::staff_roles::Capability::Personnel) && primary(ui, "+ Add employee") {
                self.new_employee();
            }
            if ui.button("+ Request leave").clicked() {
                self.new_leave();
            }
        });
    }

    fn employees(&mut self, ui: &mut Ui) {
        if self.employee_form.is_some() {
            self.employee_record(ui);
            return;
        }
        if self.performance_state.editing() || self.learning_state.editing() || self.career_state.editing(){self.performance(ui);return;}
        ui.horizontal_wrapped(|ui| {
            ui.selectable_value(
                &mut self.employee_view,
                EmployeeView::Directory,
                "Directory",
            );
            ui.selectable_value(
                &mut self.employee_view,
                EmployeeView::Organization,
                "Organization chart",
            );
            ui.selectable_value(&mut self.employee_view,EmployeeView::Development,"Development");
        });
        ui.add_space(10.0);
        if self.employee_view == EmployeeView::Development {self.performance(ui);return;}
        if self.employee_view == EmployeeView::Organization {
            self.organization_chart(ui);
            return;
        }
        ui.horizontal_wrapped(|ui| {
            ui.add_sized(
                [250.0, 36.0],
                egui::TextEdit::singleline(&mut self.search)
                    .hint_text("Search name, code, or job title…"),
            );
            egui::ComboBox::from_id_salt("employee_status")
                .selected_text(
                    self.employee_status
                        .map(|s| s.label())
                        .unwrap_or("All statuses"),
                )
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.employee_status, None, "All statuses");
                    ui.selectable_value(
                        &mut self.employee_status,
                        Some(EmployeeStatus::Active),
                        "Active",
                    );
                    ui.selectable_value(
                        &mut self.employee_status,
                        Some(EmployeeStatus::Archived),
                        "Archived",
                    );
                });
            let mut departments: Vec<_> = self
                .data
                .employees
                .iter()
                .map(|e| e.department.clone())
                .collect();
            departments.sort();
            departments.dedup();
            egui::ComboBox::from_id_salt("department")
                .selected_text(if self.department.is_empty() {
                    "All departments"
                } else {
                    &self.department
                })
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.department, String::new(), "All departments");
                    for department in departments {
                        ui.selectable_value(&mut self.department, department.clone(), department);
                    }
                });
        });
        ui.horizontal_wrapped(|ui| {
            employment_filter(
                ui,
                EmploymentKind::EmploymentType,
                &self.data,
                &mut self.employment_type_filter,
            );
            employment_filter(
                ui,
                EmploymentKind::WorkLocation,
                &self.data,
                &mut self.work_location_filter,
            );
            if ui.button("Clear filters").clicked() {
                self.search.clear();
                self.department.clear();
                self.employee_status = Some(EmployeeStatus::Active);
                self.employment_type_filter = EmploymentFilter::All;
                self.work_location_filter = EmploymentFilter::All;
            }
        });
        ui.horizontal(|ui| {
            ui.label(
                RichText::new(format!("{} employees total", self.data.employees.len()))
                    .color(muted(ui)),
            );
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if self.data.access.allows(crate::staff_roles::Capability::Personnel) && primary(ui, "+ Add employee") {
                    self.new_employee();
                }
            });
        });
        let query = self.search.trim().to_lowercase();
        let rows: Vec<_> = self
            .data
            .employees
            .iter()
            .filter(|e| {
                self.employee_status.is_none_or(|s| s == e.status)
                    && (self.department.is_empty() || self.department == e.department)
                    && self.employment_type_filter.matches(e.employment_type_id)
                    && self.work_location_filter.matches(e.work_location_id)
                    && (query.is_empty()
                        || format!(
                            "{} {} {} {} {} {}",
                            e.full_name,
                            e.employee_code,
                            e.department,
                            e.job_title,
                            EmploymentKind::EmploymentType
                                .display(&self.data, e.employment_type_id),
                            EmploymentKind::WorkLocation.display(&self.data, e.work_location_id)
                        )
                        .to_lowercase()
                        .contains(&query))
            })
            .cloned()
            .collect();
        card(ui, |ui| {
            egui::ScrollArea::horizontal()
                .id_salt("employee_table_scroll")
                .show(ui, |ui| {
                    egui::Grid::new("employees_grid")
                        .striped(true)
                        .spacing([18.0, 16.0])
                        .show(ui, |ui| {
                            headers(
                                ui,
                                &[
                                    "CODE",
                                    "EMPLOYEE",
                                    "DEPARTMENT",
                                    "EMPLOYMENT TYPE",
                                    "WORK LOCATION",
                                    "JOINED",
                                    "ANNUAL BALANCE",
                                    "STATUS",
                                    "",
                                ],
                            );
                            for employee in &rows {
                                ui.label(
                                    RichText::new(&employee.employee_code)
                                        .color(muted(ui))
                                        .monospace(),
                                );
                                ui.vertical(|ui| {
                                    ui.set_width(180.0);
                                    if ui
                                        .link(RichText::new(&employee.full_name).strong())
                                        .clicked()
                                    {
                                        self.open_employee(employee.id);
                                    }
                                    ui.label(
                                        RichText::new(&employee.job_title)
                                            .color(muted(ui))
                                            .size(12.0),
                                    );
                                });
                                ui.label(&employee.department);
                                ui.label(
                                    EmploymentKind::EmploymentType
                                        .display(&self.data, employee.employment_type_id),
                                );
                                ui.label(
                                    EmploymentKind::WorkLocation
                                        .display(&self.data, employee.work_location_id),
                                );
                                ui.label(
                                    employee
                                        .joined_on
                                        .map(|d| d.to_string())
                                        .unwrap_or_else(|| "Not set".into()),
                                );
                                if !self.data.access.can_read_operational_employee(employee) { ui.label("Restricted"); } else if let Some(kind) =
                                    self.data.leave_setup.policy.leave_type("annual")
                                {
                                    let today = self.data.company.today();
                                    let balance = crate::leave::balance(
                                        &self.data,
                                        employee,
                                        kind,
                                        today.year(),
                                        today,
                                        None,
                                    );
                                    ui.label(format!(
                                        "{} days available",
                                        crate::leave::days(balance.available_units)
                                    ));
                                }
                                badge(
                                    ui,
                                    employee.status.label(),
                                    employee.status == EmployeeStatus::Active,
                                );
                                ui.horizontal(|ui| {
                                    if self.data.access.allows(crate::staff_roles::Capability::Personnel)
                                        && ui.button("Edit").clicked()
                                    {
                                        self.open_employee(employee.id);
                                    }
                                });
                                ui.end_row();
                            }
                        });
                });
            if rows.is_empty() {
                empty(ui, "No employees match these filters.");
            }
        });
        ui.label(RichText::new("Balances show the current calendar year. Archive employees from Edit to preserve their history.").color(muted(ui)).size(12.0));
        ui.label(RichText::new("Contacts are managed by HR and administrators. Staff can update their own contacts in Profile.").color(muted(ui)).size(12.0));
    }

    fn attendance(&mut self, ui: &mut Ui) {
        if !self.attendance_navigation(ui) {
            return;
        }
        let today = self.data.company.today();
        ui.horizontal_wrapped(|ui| {
            ui.label("Attendance date");
            let response = ui.button(format!(
                "{}  |  Choose date",
                self.attendance_date.format("%b %d, %Y")
            ));
            let popup = egui::Popup::menu(&response)
                .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside);
            #[cfg(feature = "qa-screenshot")]
            let popup = if std::env::var("HRIS_PREVIEW_CALENDAR").as_deref() == Ok("open") {
                popup.open(true)
            } else {
                popup
            };
            popup.show(|ui| {
                if attendance_calendar(
                    ui,
                    &mut self.attendance_date,
                    &mut self.calendar_month,
                    today,
                    &self.data,
                ) {
                    ui.close();
                }
            });
            if ui.button("Today").clicked() {
                self.attendance_date = today;
                self.calendar_month = month_start(today);
            }
            ui.label(RichText::new(&self.data.company.timezone).color(muted(ui)));
        });
        let date = self.attendance_date;
        card(ui, |ui| {
            ui.label(RichText::new("Record a check-in").strong());
            ui.horizontal_wrapped(|ui| {
                if date < today && self.data.access.allows(crate::staff_roles::Capability::Personnel) {
                    egui::ComboBox::from_id_salt("past_attendance_employee")
                        .width(250.0)
                        .selected_text(
                            self.check_in_employee
                                .map(|id| self.data.employee_name(id))
                                .unwrap_or("Choose employee"),
                        )
                        .show_ui(ui, |ui| {
                            for e in &self.data.employees {
                                if e.joined_on.is_none_or(|d| d <= date) {
                                    let label = format!(
                                        "{} · {}{}",
                                        e.employee_code,
                                        e.full_name,
                                        if e.status == EmployeeStatus::Archived {
                                            " (archived)"
                                        } else {
                                            ""
                                        }
                                    );
                                    ui.selectable_value(
                                        &mut self.check_in_employee,
                                        Some(e.id),
                                        label,
                                    );
                                }
                            }
                        });
                } else {
                    employee_picker(
                        ui,
                        "checkin_employee",
                        &self.data,
                        &mut self.check_in_employee,
                    );
                }
                ui.add_enabled_ui(date == today && self.check_in_employee.is_some(), |ui| {
                    if primary(ui, "Check in") {
                        if let Some(id) = self.check_in_employee {
                            self.send(Command::Change(Action::CheckIn(id)));
                        }
                    }
                });
                if self.data.access.allows(crate::staff_roles::Capability::Personnel) {
                    if ui
                        .add_enabled(
                            date <= today && self.check_in_employee.is_some(),
                            egui::Button::new(if date < today {
                                "Record past attendance"
                            } else {
                                "Record with reason"
                            }),
                        )
                        .clicked()
                    {
                        if let Some(id) = self.check_in_employee {
                            self.open_correction(id, date);
                        }
                    }
                }
            });
            ui.label(RichText::new(if self.data.access.allows(crate::staff_roles::Capability::Personnel) { "Check in records the current server time. Past entries and changed times require a reason and are kept in correction history." } else { "Check in records the current server time. Ask HR to resolve missing or incorrect attendance." }).color(muted(ui)).size(12.0));
        });
        ui.add_space(8.0);
        let rows = crate::attendance::daily_roster(&self.data, date);
        let recorded = rows.iter().filter(|row| row.attendance.is_some()).count();
        let on_leave = rows
            .iter()
            .filter(|row| row.attendance.is_none() && row.approved_leave.is_some())
            .count();
        let missing = rows
            .iter()
            .filter(|row| {
                row.attendance.is_none() && row.scheduled_workday && row.approved_leave_units < 2
            })
            .count();
        card(ui, |ui| {
            ui.label(
                RichText::new(date.format("%A, %B %d, %Y").to_string())
                    .size(17.0)
                    .strong(),
            );
            ui.horizontal_wrapped(|ui| {
                badge(ui, &format!("{recorded} recorded"), true);
                badge(ui, &format!("{missing} no check-in"), false);
                badge(ui, &format!("{on_leave} approved leave"), false);
            });
            ui.add_space(8.0);
            egui::ScrollArea::horizontal().show(ui, |ui| {
                egui::Grid::new("attendance_table")
                    .striped(true)
                    .spacing([28.0, 16.0])
                    .show(ui, |ui| {
                        headers(
                            ui,
                            &["EMPLOYEE / PLANNED SHIFT", "CHECK-IN", "CHECK-OUT", "ELAPSED", "STATUS", ""],
                        );
                        for row in &rows {
                            ui.vertical(|ui| {
                                ui.set_width(230.0);
                                ui.label(RichText::new(&row.employee.full_name).strong()).on_hover_text(&row.employee.employee_code);
                                if let Some(p) = &row.planned_shift { ui.label(RichText::new(format!("{} · {} · {}",p.name,p.hours(),p.timezone)).small().color(muted(ui))); } else { ui.label(RichText::new(if row.attendance.is_some() {"No saved shift hours"} else {"Company calendar"}).small().color(muted(ui))); }
                                if let Some(record) = &row.attendance {
                                    if ui.button(format!("Breaks ({})",self.data.breaks.for_attendance(record.id).count())).clicked() {self.break_state.detail=Some(record.id);}
                                }
                            });
                            if let Some(record) = &row.attendance {
                                ui.label(
                                    record
                                        .checked_in_at
                                        .with_timezone(&self.data.company.tz())
                                        .format(self.time_format())
                                        .to_string(),
                                );
                                ui.label(
                                    record
                                        .checked_out_at
                                        .map(|t| {
                                            t.with_timezone(&self.data.company.tz())
                                                .format(self.time_format())
                                                .to_string()
                                        })
                                        .unwrap_or_else(|| "—".into()),
                                );
                                ui.label(
                                    record
                                        .checked_out_at
                                        .map(|t| {
                                            format!(
                                                "{:.1} h",
                                                (t - record.checked_in_at).num_minutes() as f64
                                                    / 60.0
                                            )
                                        })
                                        .unwrap_or_else(|| "In progress".into()),
                                );
                                badge(
                                    ui,
                                    if record.checked_out_at.is_some() {
                                        "Completed"
                                    } else {
                                        "Checked in"
                                    },
                                    true,
                                );
                                ui.horizontal(|ui| {
                                    if record.checked_out_at.is_none()
                                        && self.data.access.can_record(record.employee_id)
                                    {
                                        if ui.add_enabled(self.data.breaks.open(record.id).is_none(),egui::Button::new("Check out now")).on_hover_text("End an open break first. Records the current server time. Use Correct for a missed past check-out.").clicked() {
                                            self.send(Command::Change(Action::CheckOut(record.id)));
                                        }
                                    }
                                    if self.data.access.allows(crate::staff_roles::Capability::Personnel) {
                                        if ui.button("Correct").clicked() {
                                            self.open_correction(record.employee_id, date);
                                        }
                                        let count = self
                                            .data
                                            .corrections
                                            .iter()
                                            .filter(|c| c.attendance_id == record.id)
                                            .count();
                                        if ui.button(format!("History ({count})")).clicked() {
                                            self.correction_history = Some(record.id);
                                        }
                                    }
                                });
                            } else {
                                for _ in 0..3 {
                                    ui.label("—");
                                }
                                badge(
                                    ui,
                                    if row.approved_leave_units>=2 {
                                        "Approved leave (full day)"
                                    } else if row.approved_leave_units==1 {
                                        "Approved leave (half-day)"
                                    } else if !row.scheduled_workday {
                                        "Holiday / non-workday"
                                    } else {
                                        "No check-in"
                                    },
                                    false,
                                );
                                if self.data.access.allows(crate::staff_roles::Capability::Personnel)
                                    && ui.button("Record").clicked()
                                {
                                    self.open_correction(row.employee.id, date);
                                }
                            }
                            ui.end_row();
                        }
                    });
            });
            if rows.is_empty() {
                empty(ui, "No employee records available for this date.");
            }
            ui.label(RichText::new("Roster uses dated shift assignments, hire dates, saved leave charges and company holidays. Unassigned employees use the company calendar. No check-in does not confirm absence.").color(muted(ui)).small());
        });
        ui.label(RichText::new("Elapsed time includes breaks. Open Breaks to review recorded paid/unpaid time. Overnight attendance belongs to its starting date. Open records can be closed from that date.").color(muted(ui)).size(12.0));
        ui.add_space(12.0);
        card(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.label(RichText::new("Biometric integration").strong());
                badge(ui, "Planned", false);
            });
            ui.label("Attendance-device integration is not available in this prototype.");
            ui.add_enabled(false, egui::Button::new("Connect device"));
        });
    }

    fn leave(&mut self, ui: &mut Ui) {
        self.leave_page(ui);
    }

    fn accounts(&mut self, ui: &mut Ui) {
        if !self.data.access.role.manages_accounts() {
            return;
        }
        if self.data.access.staff_context.as_ref().is_some_and(|c|c.runtime_ready) {
            self.staff_roles_panel(ui);return;
        }
        let staff: Vec<_> = self
            .data
            .accounts
            .iter()
            .filter(|a| a.role.uses_desktop())
            .cloned()
            .collect();
        let enabled = staff.iter().filter(|a| a.active).count();
        ui.horizontal_wrapped(|ui| {
            ui.label(format!("{enabled} active staff role(s)"));
            if primary(ui, "+ Add staff employee") {
                self.new_employee();
                let mut access = AccountDraft::new();
                access.role = Role::HrGeneralist;
                self.account_form = Some(access);
                self.contacts_section = ContactSection::Employment;
            }
            ui.checkbox(
                &mut self.show_disabled_staff,
                format!("Show disabled ({})", staff.len() - enabled),
            );
        });
        ui.label(RichText::new("Staff are employees with desktop roles. Open a person to manage their employment, contacts and access together.").color(muted(ui)));
        for access in staff
            .into_iter()
            .filter(|a| a.active || self.show_disabled_staff)
            .collect::<Vec<_>>()
        {
            let Some(employee) = access
                .employee_id
                .and_then(|id| self.data.employees.iter().find(|e| e.id == id))
                .cloned()
            else {
                continue;
            };
            card(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    if ui
                        .link(RichText::new(&employee.full_name).strong())
                        .clicked()
                    {
                        self.open_employee(employee.id);
                        self.account_form = Some(AccountDraft::from_account(&access));
                    }
                    badge(ui, &employee.employee_code, true);
                    badge(ui, access.role.label(), access.active);
                    badge(
                        ui,
                        if access.active { "Enabled" } else { "Disabled" },
                        access.active,
                    );
                });
                ui.label(format!("Sign-in: {}", access.email));
                if !access.email_confirmed {
                    ui.label(RichText::new("Email confirmation pending").color(muted(ui)));
                }
                ui.horizontal_wrapped(|ui| {
                    if ui.button("Edit employee").clicked() {
                        self.open_employee(employee.id);
                        self.account_form = Some(AccountDraft::from_account(&access));
                    }
                    if !access.email_confirmed
                        && access.active
                        && self.data.mode == Mode::Supabase
                        && ui.button("Send confirmation").clicked()
                    {
                        self.send(Command::Admin(AdminAction::ResendConfirmation(
                            access.user_id,
                        )));
                    }
                });
            });
            ui.add_space(8.0);
        }
        ui.collapsing("Role permissions", |ui| {
            for role in Role::ALL.into_iter().filter(|r| r.uses_desktop()) {
                ui.label(RichText::new(role.label()).strong());
                ui.label(role.description());
                ui.add_space(4.0);
            }
        });
        ui.collapsing("Recent access changes", |ui| {
            for event in &self.data.access_events {
                let person = self
                    .data
                    .accounts
                    .iter()
                    .find(|a| a.user_id == event.user_id)
                    .and_then(|a| a.employee_id)
                    .map(|id| self.data.employee_name(id))
                    .unwrap_or("Employee");
                ui.label(format!(
                    "{} · {} · {}",
                    account_timestamp(
                        &event.created_at,
                        self.data.company.tz(),
                        self.time_format()
                    ),
                    if event.action == "account_added" {
                        "Role assigned"
                    } else {
                        "Access changed"
                    },
                    person
                ));
            }
            ui.label(
                RichText::new(
                    "Shows the 50 most recent access changes. Passwords are never included.",
                )
                .color(muted(ui))
                .small(),
            );
        });
    }

    fn company_defaults_fields(&mut self, ui: &mut Ui) {
        field(
            ui,
            "Company name",
            &mut self.company_draft.name,
            "Your company",
        );
        field(
            ui,
            "Time zone",
            &mut self.company_draft.timezone,
            "Asia/Manila",
        );
        ui.label("Annual leave for new employees");
        ui.add(
            egui::DragValue::new(&mut self.company_draft.annual_leave_days)
                .range(0..=366)
                .suffix(" days"),
        );
        ui.label(
            RichText::new("Existing allowances stay unchanged.")
                .color(muted(ui))
                .small(),
        );
        if primary(ui, "Save company settings") {
            match self.company_draft.validate() {
                Ok(()) => self.send(Command::Change(Action::SaveCompany(
                    self.company_draft.clone(),
                ))),
                Err(error) => self.notice = Some((error.to_string(), true)),
            }
        }
    }

    fn company_logo_controls(&mut self, ui: &mut Ui) {
        ui.label(RichText::new("Company logo").strong());
        company_logo(
            ui,
            self.company_logo.as_ref(),
            &self.data.company.name,
            80.0,
        );
        ui.horizontal_wrapped(|ui| {
            if ui.button("Upload logo").clicked() {
                match account::choose_image("Choose a company logo") {
                    Ok(Some(path)) => self.send(Command::Change(Action::UploadCompanyLogo(path))),
                    Ok(None) => {}
                    Err(error) => self.notice = Some((error.to_string(), true)),
                }
            }
            if ui
                .add_enabled(
                    self.data.company.logo_version.is_some(),
                    egui::Button::new("Remove logo"),
                )
                .clicked()
            {
                self.send(Command::Change(Action::RemoveCompanyLogo));
            }
        });
        ui.label(RichText::new("PNG or JPEG, up to 5 MiB and 2048 × 2048. Transparent circular display on the sidebar and sign-in page.").color(muted(ui)).small());
        if let Some(warning) = self
            .data
            .logo_warning
            .as_ref()
            .or(self.login_branding_warning.as_ref())
        {
            ui.colored_label(ui.visuals().error_fg_color, warning);
        }
    }

    fn settings(&mut self, ui: &mut Ui) {
        if self.data.access.role.manages_accounts() {
            card(ui, |ui| {
                ui.set_max_width(ui.available_width().min(840.0));
                ui.label(RichText::new("Company defaults").size(18.0).strong());
                ui.add_space(6.0);
                ui.columns(2, |columns| {
                    for column in columns.iter_mut() {
                        column.spacing_mut().item_spacing.y = 6.0;
                    }
                    self.company_defaults_fields(&mut columns[0]);
                    self.company_logo_controls(&mut columns[1]);
                });
            });
            ui.add_space(12.0);
            let _section = ui.scope(|ui| self.employment_options_settings(ui));
            #[cfg(feature = "qa-screenshot")]
            if matches!(
                std::env::var("HRIS_PREVIEW_FORM").as_deref(),
                Ok("employment-setup" | "employment-setup-bottom")
            ) {
                let align = if std::env::var("HRIS_PREVIEW_FORM").as_deref()
                    == Ok("employment-setup-bottom")
                {
                    Align::BOTTOM
                } else {
                    Align::TOP
                };
                ui.scroll_to_rect(_section.response.rect, Some(align));
            }
            ui.add_space(12.0);
        }
        if self.data.access.role.manages_accounts() {
            card(ui, |ui| {
                ui.label(
                    RichText::new("Leave policies and approvals")
                        .size(18.0)
                        .strong(),
                );
                ui.label("Configure workweeks, holidays, leave types, accrual, carryover and approval chains.");
                if ui.button("Manage leave policies").clicked() {
                    self.page = Page::Leave;
                    self.leave_state.view = leave_view::View::Policies;
                }
                ui.add_space(8.0);
                ui.label(RichText::new("Attendance calculation rules").strong());
                ui.label("Configure grace allowances, overtime minimum and rounding.");
                if ui.button("Manage calculation rules").clicked() {
                    self.page = Page::Attendance;
                    self.shift_state.tab = shifts_view::Tab::Calculations;
                    self.calculation_state.policy = true;
                }
            });
            ui.add_space(12.0);
        }
        card(ui, |ui| {
            ui.label(RichText::new("Display preferences").size(18.0).strong());
            ui.label(
                RichText::new("These preferences apply to your account.")
                    .color(muted(ui))
                    .size(12.0),
            );
            ui.horizontal_wrapped(|ui| {
                ui.label("Theme");
                for theme in [
                    account::ThemeChoice::Light,
                    account::ThemeChoice::Dark,
                    account::ThemeChoice::System,
                ] {
                    ui.selectable_value(&mut self.display_draft.theme, theme, theme.label());
                }
            });
            ui.horizontal(|ui| {
                ui.label("Display scale");
                ui.add(
                    egui::Slider::new(&mut self.display_draft.scale_percent, 100..=125)
                        .step_by(5.0)
                        .suffix("%"),
                );
            });
            ui.horizontal(|ui| {
                ui.label("Time format");
                ui.selectable_value(&mut self.display_draft.clock_24h, true, "24-hour");
                ui.selectable_value(&mut self.display_draft.clock_24h, false, "12-hour");
            });
            ui.label(
                RichText::new(if self.display_draft.clock_24h {
                    "Example: 14:30"
                } else {
                    "Example: 02:30 PM"
                })
                .color(muted(ui))
                .size(12.0),
            );
            if primary(ui, "Save display preferences") {
                self.send(Command::Account(AccountAction::SaveDisplay(
                    self.display_draft,
                )));
            }
        });
        ui.add_space(12.0);
        card(ui, |ui| {
            ui.label(RichText::new("App updates").size(18.0).strong());
            ui.label(format!("HRIS Desktop {}", updater::VERSION));
            ui.label("Updates are checked automatically after a successful sign-in. An available update stays visible until you update or sign out.");
            match &self.update_state {
                UpdateState::UpToDate => {
                    ui.label("You have the latest compatible version.");
                }
                UpdateState::Failed(message) => {
                    ui.colored_label(ui.visuals().error_fg_color, message);
                }
                UpdateState::Available(update) => {
                    ui.label(format!("Version {} is available.", update.release.version));
                }
                UpdateState::Restarting => {
                    ui.label("Restarting to finish the update…");
                }
                UpdateState::Idle => {
                    ui.label("Sign in to check this company's update channel.");
                }
            }
            if ui
                .add_enabled(
                    self.data.mode == Mode::Supabase,
                    egui::Button::new("Check for updates"),
                )
                .clicked()
            {
                self.send(Command::CheckUpdates);
            }
        });
    }

    fn profile(&mut self, ui: &mut Ui) {
        let Some(profile) = self.account.clone() else {
            empty(ui, "Refresh to load your account.");
            return;
        };
        let signed_in = self.data.mode == Mode::Supabase;
        let employee_name = self
            .data
            .own_employee()
            .map(|e| e.full_name.clone())
            .unwrap_or_else(|| profile.details.display_name().into());
        card(ui, |ui| {
            ui.horizontal(|ui| {
                avatar(ui,self.avatar.as_ref(),&employee_name,86.0);
                ui.vertical(|ui| {
                    ui.label(RichText::new(&employee_name).size(22.0).strong());
                    ui.label(RichText::new(if signed_in { self.data.access.role_label() } else { "Demo account".to_owned() }).color(muted(ui)));
                    ui.horizontal_wrapped(|ui| {
                        if ui.button("Change photo").clicked() {
                            match account::choose_photo() {
                                Ok(Some(path)) => self.send(Command::Account(AccountAction::UploadPhoto(path))),
                                Ok(None) => {},
                                Err(error) => self.notice = Some((error.to_string(),true)),
                            }
                        }
                        if ui.add_enabled(profile.photo.is_some(),egui::Button::new("Remove photo")).clicked() {
                            self.send(Command::Account(AccountAction::RemovePhoto));
                        }
                    });
                    ui.label(RichText::new("PNG or JPEG, up to 5 MiB. Photos are centered and cropped to a circle.").color(muted(ui)).size(11.0));
                });
            });
            if let Some(warning) = &profile.photo_warning {
                ui.colored_label(Color32::from_rgb(160, 92, 26), warning);
            }
        });
        ui.add_space(8.0);
        let previous = self.profile_section;
        ui.horizontal_wrapped(|ui| {
            ui.selectable_value(
                &mut self.profile_section,
                ProfileSection::Employment,
                "Employment",
            );
            ui.selectable_value(
                &mut self.profile_section,
                ProfileSection::Contacts,
                "Contact details",
            );
            ui.selectable_value(
                &mut self.profile_section,
                ProfileSection::Emergency,
                "Emergency contact",
            );
            ui.selectable_value(
                &mut self.profile_section,
                ProfileSection::Security,
                "Account & security",
            );
            ui.selectable_value(
                &mut self.profile_section,
                ProfileSection::Sessions,
                "Sessions",
            );
        });
        if previous != self.profile_section {
            self.clear_profile_secrets(ui.ctx());
        }
        ui.add_space(8.0);
        match self.profile_section {
            ProfileSection::Employment => {
                card(ui, |ui| {
                    ui.label(RichText::new("Employment").size(18.0).strong());
                    ui.label(RichText::new("Managed by HR. These are the same details shown in your employee record.").color(muted(ui)));
                    if let Some(employee) = self.data.own_employee() {
                        employment_summary(ui, employee, &self.data);
                    } else {
                        empty(ui, "Refresh to load your employment details.");
                    }
                });
                ui.add_space(12.0);
                card(ui, |ui| {
                    ui.label(RichText::new("Sign-in account").strong());
                    ui.label(&profile.email);
                    ui.label(format!("Staff role: {}", self.data.access.role_label()));
                    if ui.button("Manage account security").clicked() {
                        self.profile_section = ProfileSection::Security;
                    }
                });
            }
            ProfileSection::Contacts | ProfileSection::Emergency => {
                let Some(person) = self.data.access.employee_id else {
                    empty(ui, "Refresh to load your employee record.");
                    return;
                };
                let mut contacts = self.profile_contacts_draft.take().unwrap_or_else(|| {
                    self.data
                        .own_contacts()
                        .cloned()
                        .unwrap_or_else(|| EmployeeContacts::empty(self.data.company.id, person))
                });
                card(ui, |ui| {
                    let emergency = self.profile_section == ProfileSection::Emergency;
                    ui.label(
                        RichText::new(if emergency {
                            "Emergency contact"
                        } else {
                            "Contact details"
                        })
                        .size(18.0)
                        .strong(),
                    );
                    ui.label(RichText::new("Changes update your employee record. Both contact tabs are saved together.").color(muted(ui)));
                    contact_fields(ui, &mut contacts, emergency);
                    ui.horizontal_wrapped(|ui| {
                        if primary(ui, "Save my contact details") {
                            match contacts.normalized() {
                                Ok(saved) => {
                                    contacts = saved.clone();
                                    self.send(Command::Change(Action::SaveOwnContacts(saved)));
                                }
                                Err(error) => self.notice = Some((error.to_string(), true)),
                            }
                        }
                        if ui.button("Revert changes").clicked() {
                            contacts = self.data.own_contacts().cloned().unwrap_or_else(|| {
                                EmployeeContacts::empty(self.data.company.id, person)
                            });
                        }
                    });
                });
                self.profile_contacts_draft = Some(contacts);
            }
            ProfileSection::Security => {
                if !signed_in {
                    ui.label(RichText::new("Sign in to an HRIS account to change account security. This demo never changes a real account.").color(muted(ui)));
                }
                ui.add_enabled_ui(signed_in, |ui| {
                    card(ui, |ui| {
                        ui.label(RichText::new("Change password").size(18.0).strong());
                        ui.label(RichText::new("Use 12–128 characters. Your current password is required.").color(muted(ui)).size(12.0));
                        password_field(ui,"Current password","profile-current-password",&mut self.current_password,self.show_passwords);
                        password_field(ui,"New password","profile-new-password",&mut self.new_password,self.show_passwords);
                        password_field(ui,"Confirm new password","profile-confirm-password",&mut self.confirm_password,self.show_passwords);
                        ui.checkbox(&mut self.show_passwords,"Show passwords");
                        if primary(ui,"Update password") {
                            match account::validate_password_change(&self.current_password,&self.new_password,&self.confirm_password) {
                                Ok(()) => {
                                    let action = AccountAction::ChangePassword {
                                        current: std::mem::take(&mut self.current_password),new:std::mem::take(&mut self.new_password),confirmation:std::mem::take(&mut self.confirm_password)
                                    };
                                    self.clear_profile_secrets(ui.ctx());
                                    self.send(Command::Account(action));
                                }
                                Err(error) => self.notice = Some((error.to_string(),true)),
                            }
                        }
                    });
                    ui.add_space(12.0);
                    card(ui, |ui| {
                        ui.label(RichText::new("Account email").size(18.0).strong());
                        ui.label(format!("Current email: {}",profile.email));
                        if !profile.pending_email.is_empty() {
                            ui.label(RichText::new(format!("Awaiting confirmation: {}",profile.pending_email)).color(accent(ui)));
                        }
                        profile_field(ui,"New email address","profile-new-email",&mut self.new_email,"Your new account email");
                        password_field(ui,"Current password","profile-email-password",&mut self.email_password,self.show_passwords);
                        ui.label(RichText::new("Confirm the links sent to your email addresses, then Refresh. Your current email remains active until confirmation.").color(muted(ui)).size(12.0));
                        if primary(ui,"Request email change") {
                            if let Err(error) = account::validate_email(&self.new_email) { self.notice = Some((error.to_string(),true)); }
                            else if self.email_password.is_empty() { self.notice = Some(("Enter your current password.".into(),true)); }
                            else {
                                let action = AccountAction::ChangeEmail { current:std::mem::take(&mut self.email_password),new_email:std::mem::take(&mut self.new_email) };
                                self.clear_profile_secrets(ui.ctx());
                                self.send(Command::Account(action));
                            }
                        }
                    });
                });
            }
            ProfileSection::Sessions => {
                card(ui, |ui| {
                    ui.label(RichText::new("Your sessions").size(18.0).strong());
                    ui.label(if signed_in {
                        "You are signed in on this device."
                    } else {
                        "This is a local demo. There is no signed-in account."
                    });
                    if let Some(last) = &profile.last_sign_in_at {
                        ui.label(
                            RichText::new(format!(
                                "Last sign-in: {}",
                                account_timestamp(last, self.data.company.tz(), self.time_format())
                            ))
                            .color(muted(ui)),
                        );
                    }
                    ui.label(
                        RichText::new(
                            "This app does not save your password or session when it closes.",
                        )
                        .color(muted(ui))
                        .size(12.0),
                    );
                    ui.add_space(10.0);
                    ui.add_enabled_ui(signed_in, |ui| {
                        if ui.button("Sign out other devices").clicked() { self.session_confirmation = Some(SessionChoice::Others); }
                        ui.label(RichText::new("Keep this device signed in while revoking the other sessions.").color(muted(ui)).size(12.0));
                        ui.add_space(8.0);
                        if ui.button("Sign out everywhere").clicked() { self.session_confirmation = Some(SessionChoice::Everywhere); }
                        ui.label(RichText::new("Includes this device. Other devices may retain access until their current access token expires.").color(muted(ui)).size(12.0));
                        ui.separator();
                        if primary(ui,"Sign out of this device") { self.sign_out(ui.ctx(),false); }
                    });
                });
            }
        }
        if !self.busy {
            if let Some(path) = ui.ctx().input(|input| {
                input
                    .raw
                    .dropped_files
                    .first()
                    .map(|f| f.path().to_path_buf())
            }) {
                self.send(Command::Account(AccountAction::UploadPhoto(path)));
            }
        }
    }

    fn sign_out(&mut self, ctx: &egui::Context, everywhere: bool) {
        self.clear_profile_secrets(ctx);
        self.login_email.clear();
        self.login_password.clear();
        self.new_email.clear();
        clear_login_history(ctx);
        clear_edit_history(
            ctx,
            &[
                "profile-full-name",
                "profile-preferred-name",
                "profile-contact-phone",
                "profile-new-email",
            ],
        );
        self.send(if everywhere {
            Command::SignOutEverywhere
        } else {
            Command::SignOut
        });
    }

    fn confirm_session_action(&mut self, ctx: &egui::Context) {
        if let Some(choice) = self.session_confirmation {
            let mut confirmed = false;
            let mut cancel = false;
            egui::Window::new("Confirm sign out").collapsible(false).resizable(false)
                .anchor(egui::Align2::CENTER_CENTER,Vec2::ZERO).show(ctx,|ui| {
                    ui.set_max_width(390.0);
                    ui.label(if choice == SessionChoice::Others { "Sign out other devices and keep this device signed in?" } else { "Sign out all devices, including this one?" });
                    ui.label(RichText::new("Other devices may keep access until their current access token expires.").color(muted(ui)).size(12.0));
                    ui.horizontal(|ui| { confirmed = primary(ui,"Confirm sign out"); cancel = ui.button("Cancel").clicked(); });
                });
            if cancel || confirmed {
                self.session_confirmation = None;
            }
            if confirmed {
                if choice == SessionChoice::Others {
                    self.send(Command::Account(AccountAction::SignOutOthers));
                } else {
                    self.sign_out(ctx, true);
                }
            }
        }
    }

    fn sign_in(&mut self, ui: &mut Ui) {
        let logo = self.login_logo.clone();
        let company_name = self.login_branding.company_name.clone();
        sign_in_layout(ui, logo.as_ref(), &company_name, |ui| {
            if let Some(error) = &self.deployment_error {
                ui.heading("Company connection unavailable");
                ui.label(error);
                ui.label(RichText::new("Your HRIS administrator configures this installation. Reopen the app after they have corrected its connection.").color(muted(ui)));
                return;
            }
            ui.heading("Sign in");
            ui.label(
                RichText::new("Use your HRIS account to access company records.").color(muted(ui)),
            );
            if let Some((message, error)) = &self.notice {
                ui.colored_label(
                    if *error {
                        Color32::from_rgb(160, 52, 38)
                    } else {
                        TEAL
                    },
                    message,
                );
            }
            ui.add_space(8.0);
            if login_controls(
                ui,
                &mut self.login_email,
                &mut self.login_password,
                !self.busy,
            ) {
                if self.login_email.trim().is_empty() || self.login_password.is_empty() {
                    self.notice = Some(("Enter your email and password.".into(), true));
                } else if let Some(config) = &self.connection {
                    let command = Command::Connect {
                        config: config.clone(),
                        email: self.login_email.trim().into(),
                        password: std::mem::take(&mut self.login_password),
                    };
                    clear_login_history(ui.ctx());
                    self.send(command);
                }
            }
            if self.busy {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label("Signing in…");
                });
            }
        });
    }

    fn employment_options_settings(&mut self, ui: &mut Ui) {
        card(ui, |ui| {
            ui.label(RichText::new("Employment setup").size(18.0).strong());
            ui.label("Manage the choices HR can assign in employee records. Existing assignments remain when a choice is disabled.");
            for kind in EmploymentKind::ALL {
                ui.add_space(8.0);
                ui.horizontal_wrapped(|ui| {
                    ui.label(RichText::new(kind.plural()).strong());
                    if ui
                        .button(format!("+ Add {}", kind.label().to_lowercase()))
                        .clicked()
                    {
                        self.employment_option_form = Some(EmploymentOptionDraft::new(kind));
                        self.notice = None;
                    }
                });
                egui::Grid::new(("employment_options", kind.table()))
                    .striped(true)
                    .spacing([18.0, 10.0])
                    .show(ui, |ui| {
                        for option in kind.choices(&self.data) {
                            ui.vertical(|ui| {
                                ui.set_width(220.0);
                                ui.label(&option.name);
                            });
                            badge(
                                ui,
                                if option.active {
                                    "Available"
                                } else {
                                    "Inactive"
                                },
                                option.active,
                            );
                            if ui.button("Edit").clicked() {
                                self.employment_option_form =
                                    Some(EmploymentOptionDraft::existing(kind, option));
                                self.notice = None;
                            }
                            ui.end_row();
                        }
                    });
                if kind.choices(&self.data).is_empty() {
                    ui.label("No choices yet. Add one to make it available in employee records.");
                }
            }
        });
    }

    fn employment_option_dialog(&mut self, ctx: &egui::Context) {
        if let Some(mut draft) = self.employment_option_form.take() {
            let mut open = true;
            let mut save = false;
            let mut cancel = false;
            egui::Window::new(format!("{} {}",if draft.id.is_some(){"Edit"}else{"Add"},draft.kind.label().to_lowercase()))
                .open(&mut open).collapsible(false).resizable(false).default_width(440.0)
                .anchor(egui::Align2::CENTER_CENTER,[0.0,0.0]).show(ctx,|ui| {
                    ui.add_enabled_ui(!self.busy,|ui| {
                        field(ui,"Name",&mut draft.name,"Company-specific name");
                        ui.checkbox(&mut draft.active,"Available for new assignments");
                        ui.label("Disabling keeps this choice on existing employee records and in filters. It can be enabled again here.");
                        if draft.id.is_some() { ui.label(RichText::new("Renaming updates the displayed label on existing assignments.").color(muted(ui)).small()); }
                        if let Some((text,true))=&self.notice {ui.colored_label(ui.visuals().error_fg_color,text);}
                        ui.add_space(8.0);
                        ui.horizontal(|ui|{save=primary(ui,"Save choice");cancel=ui.button("Cancel").clicked();});
                    });
                });
            if save {
                match draft.option(&self.data) {
                    Ok(_) => {
                        self.close_on_success = true;
                        self.send(Command::Change(Action::SaveEmploymentOption(draft.clone())));
                    }
                    Err(error) => self.notice = Some((error.to_string(), true)),
                }
            }
            if (open && !cancel) || self.busy {
                self.employment_option_form = Some(draft);
            }
        }
    }

    fn employee_documents(&mut self, ui: &mut Ui, employee: Uuid) {
        if documents::authorize(&self.data, employee).is_err() {
            return;
        }
        ui.label(
            RichText::new("Employee documents / 201 files")
                .size(18.0)
                .strong(),
        );
        ui.label("Private HR/Admin files. Documents save separately from employee changes.");
        ui.horizontal_wrapped(|ui| {
            if primary(ui, "+ Upload document") {
                self.document_form = Some(DocumentDraft::new(employee));
                self.notice = None;
            }
            ui.checkbox(&mut self.show_archived_documents, "Show archived");
            ui.checkbox(&mut self.show_discarded_documents, "Show discarded uploads");
        });
        ui.horizontal_wrapped(|ui| {
            ui.add(
                egui::TextEdit::singleline(&mut self.document_search)
                    .hint_text("Search document title or filename")
                    .desired_width(250.0),
            );
            egui::ComboBox::from_id_salt("document_category_filter")
                .selected_text(
                    self.document_category
                        .map_or("All categories", DocumentCategory::label),
                )
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.document_category, None, "All categories");
                    for category in DocumentCategory::ALL {
                        ui.selectable_value(
                            &mut self.document_category,
                            Some(category),
                            category.label(),
                        );
                    }
                });
        });
        let query = self.document_search.trim().to_lowercase();
        let rows: Vec<_> = documents::records(&self.data, employee)
            .into_iter()
            .filter(|d| {
                (self.show_archived_documents || !d.archived)
                    && (self.show_discarded_documents || d.state != DocumentState::Discarded)
                    && self
                        .document_category
                        .is_none_or(|kind| kind == d.details.category)
                    && (query.is_empty()
                        || format!("{} {}", d.details.title, d.original_filename)
                            .to_lowercase()
                            .contains(&query))
            })
            .cloned()
            .collect();
        if rows.is_empty() {
            empty(
                ui,
                "No documents match this view. Upload a contract, scan, certificate or other attachment.",
            );
        }
        for doc in rows {
            ui.separator();
            let _title = ui.add(
                egui::Label::new(RichText::new(&doc.details.title).strong().size(16.0)).wrap(),
            );
            #[cfg(feature = "qa-screenshot")]
            if matches!(
                (
                    std::env::var("HRIS_PREVIEW_SCROLL").as_deref(),
                    doc.id.as_u128()
                ),
                (Ok("document-file"), 6000) | (Ok("document-archived"), 6002)
            ) {
                _title.scroll_to_me(Some(Align::Min));
            }
            ui.horizontal_wrapped(|ui| {
                ui.label(doc.details.category.label());
                badge(
                    ui,
                    doc.status(self.data.company.today()),
                    doc.state == DocumentState::Ready
                        && !doc.archived
                        && doc
                            .details
                            .expires_on
                            .is_none_or(|d| d >= self.data.company.today()),
                );
            });
            ui.add(
                egui::Label::new(format!(
                    "{} · {} · {:.1} KiB",
                    doc.original_filename,
                    match doc.mime_type.as_str() {
                        "application/pdf" => "PDF",
                        documents::DOCX_MIME => "DOCX",
                        "image/png" => "PNG",
                        "image/jpeg" => "JPEG",
                        _ => "Document",
                    },
                    doc.byte_size as f64 / 1024.0
                ))
                .wrap(),
            );
            ui.label(format!(
                "Uploaded by {} · {}",
                doc.created_by_name,
                account_timestamp(
                    &doc.created_at.to_rfc3339(),
                    self.data.company.tz(),
                    self.time_format()
                )
            ));
            let date = |value: Option<NaiveDate>| {
                value
                    .map(|d| d.to_string())
                    .unwrap_or_else(|| "Not set".into())
            };
            if doc.details.issued_on.is_some() || doc.details.expires_on.is_some() {
                ui.label(format!(
                    "Issued: {} · Expires: {}",
                    date(doc.details.issued_on),
                    date(doc.details.expires_on)
                ));
            }
            if doc.details.category == DocumentCategory::Contract {
                ui.label(format!(
                    "Contract start: {} · End: {}",
                    date(doc.details.contract_start),
                    date(doc.details.contract_end)
                ));
            }
            if !doc.details.notes.is_empty() {
                ui.add(egui::Label::new(&doc.details.notes).wrap());
            }
            let _actions = ui.horizontal_wrapped(|ui| {
                if doc.state == DocumentState::Ready {
                    if ui.button("Edit details").clicked() {
                        self.document_form = Some(DocumentDraft::existing(&doc));
                        self.notice = None;
                    }
                    if ui.button("Download").clicked() {
                        match documents::choose_file(Some(&doc.original_filename)) {
                            Ok(Some(destination)) => {
                                self.send(Command::Document(DocumentAction::Download {
                                    id: doc.id,
                                    destination,
                                }))
                            }
                            Ok(None) => {}
                            Err(error) => self.notice = Some((error.to_string(), true)),
                        }
                    }
                    if ui
                        .button(if doc.archived { "Restore" } else { "Archive" })
                        .clicked()
                    {
                        self.send(Command::Document(DocumentAction::Archive {
                            id: doc.id,
                            revision: doc.revision,
                            archived: !doc.archived,
                        }));
                    }
                } else {
                    if doc.state == DocumentState::Pending
                        && doc.created_by == self.data.access.user_id
                        && ui.button("Retry upload").clicked()
                    {
                        self.document_form = Some(DocumentDraft::existing(&doc));
                        self.notice = None;
                    }
                    if ui
                        .button(if doc.state == DocumentState::Pending {
                            "Discard incomplete upload"
                        } else {
                            "Clean up upload"
                        })
                        .clicked()
                    {
                        self.send(Command::Document(DocumentAction::Discard { id: doc.id }));
                    }
                }
            });
            #[cfg(feature = "qa-screenshot")]
            if std::env::var("HRIS_PREVIEW_SCROLL").as_deref() == Ok("document-actions")
                && doc.id.as_u128() == 6000
            {
                _actions.response.scroll_to_me(Some(Align::Min));
            }
            if doc.state == DocumentState::Pending {
                ui.label("This file is not yet filed. Retry with the exact same file, or discard this incomplete upload.");
            }
            if doc.state == DocumentState::Discarded {
                ui.label("This upload is permanently discarded. Clean up upload retries removal of any unused file.");
            }
            let mut events: Vec<_> = self
                .data
                .document_events
                .iter()
                .filter(|e| e.company_id == self.data.company.id && e.document_id == doc.id)
                .collect();
            events.sort_by_key(|e| std::cmp::Reverse(e.sequence));
            if !events.is_empty() {
                let history =
                    egui::CollapsingHeader::new(format!("Document history ({})", events.len()))
                        .id_salt(("document_history", doc.id));
                #[cfg(feature = "qa-screenshot")]
                let history = history.default_open(
                    std::env::var("HRIS_PREVIEW_SCROLL").as_deref() == Ok("document-history")
                        && doc.id.as_u128() == 6000,
                );
                history.show(ui, |ui| {
                    for event in events {
                        ui.separator();
                        let kind = match event.kind.as_str() {
                            "uploaded" => "Filed",
                            "details_updated" => "Details updated",
                            "archived" => "Archived",
                            "restored" => "Restored",
                            _ => "Saved",
                        };
                        ui.add(
                            egui::Label::new(format!(
                                "{} · {} · {} ({})",
                                kind,
                                account_timestamp(
                                    &event.recorded_at.to_rfc3339(),
                                    self.data.company.tz(),
                                    self.time_format()
                                ),
                                event.actor_name,
                                event.actor_role.label()
                            ))
                            .wrap(),
                        );
                        let fields = |record: &documents::EmployeeDocument| {
                            vec![
                                ("Title", record.details.title.clone()),
                                ("Category", record.details.category.label().to_owned()),
                                ("Notes", record.details.notes.clone()),
                                ("Issued", date(record.details.issued_on)),
                                ("Expires", date(record.details.expires_on)),
                                ("Contract start", date(record.details.contract_start)),
                                ("Contract end", date(record.details.contract_end)),
                                (
                                    "Archived",
                                    if record.archived { "Yes" } else { "No" }.into(),
                                ),
                            ]
                        };
                        let before = event.before_record.as_ref().map(&fields);
                        for (i, (name, after)) in
                            fields(&event.after_record).into_iter().enumerate()
                        {
                            if let Some(old) = before.as_ref() {
                                if old[i].1 != after {
                                    ui.add(
                                        egui::Label::new(format!(
                                            "{name}: {} → {after}",
                                            if old[i].1.is_empty() {
                                                "Not set"
                                            } else {
                                                &old[i].1
                                            }
                                        ))
                                        .wrap(),
                                    );
                                }
                            } else if name == "Title" || name == "Category" {
                                ui.add(egui::Label::new(format!("{name}: {after}")).wrap());
                            }
                        }
                    }
                    #[cfg(feature = "qa-screenshot")]
                    if std::env::var("HRIS_PREVIEW_SCROLL").as_deref() == Ok("document-history")
                        && doc.id.as_u128() == 6000
                    {
                        ui.scroll_to_cursor(Some(Align::BOTTOM));
                    }
                });
            }
        }
        ui.add_space(10.0);
        ui.label("PDF, DOCX, PNG and JPEG up to 10 MiB. Archive retains the original file and its history. To replace a file, upload another document and archive the old one.");
    }

    fn document_dialog(&mut self, ctx: &egui::Context) {
        let Some(mut draft) = self.document_form.take() else {
            return;
        };
        if documents::authorize(&self.data, draft.employee_id).is_err() {
            return;
        }
        let editing = draft
            .original
            .as_ref()
            .is_some_and(|d| d.state == DocumentState::Ready);
        let retrying = draft
            .original
            .as_ref()
            .is_some_and(|d| d.state == DocumentState::Pending);
        let mut open = true;
        let mut save = false;
        let mut cancel = false;
        egui::Window::new(if editing{"Edit document details"}else if retrying{"Retry document upload"}else{"Upload employee document"})
            .id(egui::Id::unique("employee_document_dialog")).open(&mut open).collapsible(false).resizable(false)
            .default_width(520.0).anchor(egui::Align2::CENTER_CENTER,[0.0,0.0]).show(ctx,|ui| {
                ui.add_enabled_ui(!self.busy,|ui| {
                    egui::ScrollArea::vertical().id_salt("document_form_body").max_height((ctx.content_rect().height()-260.0).max(130.0)).show(ui,|ui| {
                        ui.label(RichText::new(self.data.employee_name(draft.employee_id)).size(18.0).strong());
                        if editing {
                            ui.add(egui::Label::new(format!("Original file: {}",draft.original.as_ref().unwrap().original_filename)).wrap());
                            ui.label("The file is retained unchanged. These fields describe the document.");
                        } else {
                            if ui.button("Choose PDF, DOCX, PNG or JPEG…").clicked() {
                                match documents::choose_file(None) {
                                    Ok(Some(path))=>{
                                        if draft.title.trim().is_empty() {draft.title=path.file_stem().and_then(|s|s.to_str()).unwrap_or("").chars().take(120).collect();}
                                        draft.file=Some(path);
                                    },
                                    Ok(None)=>{},Err(error)=>self.notice=Some((error.to_string(),true)),
                                }
                            }
                            ui.add(egui::Label::new(draft.file.as_ref().and_then(|p|p.file_name()).map(|s|s.to_string_lossy().into_owned()).unwrap_or_else(||"No file selected · up to 10 MiB".into())).wrap());
                            if retrying {ui.label("Choose the exact original file and retain its reserved details. Discard the incomplete upload to start over with different details.");}
                        }
                        field(ui,"Document title *",&mut draft.title,"Contract, certificate, HR form…");
                        ui.label("Category");
                        let previous=draft.category;
                        egui::ComboBox::from_id_salt("document_category").selected_text(draft.category.label()).show_ui(ui,|ui| {
                            for category in DocumentCategory::ALL {ui.selectable_value(&mut draft.category,category,category.label());}
                        });
                        if previous!=draft.category && draft.category!=DocumentCategory::Contract {draft.contract_start.clear();draft.contract_end.clear();}
                        field(ui,"Issue date (optional)",&mut draft.issued_on,"YYYY-MM-DD");
                        field(ui,"Expiry date (optional)",&mut draft.expires_on,"YYYY-MM-DD");
                        if draft.category==DocumentCategory::Contract {
                            field(ui,"Contract start (optional)",&mut draft.contract_start,"YYYY-MM-DD");
                            field(ui,"Contract end (optional)",&mut draft.contract_end,"YYYY-MM-DD");
                        }
                        ui.label("Notes (optional)");
                        ui.add_sized([ui.available_width(),70.0],egui::TextEdit::multiline(&mut draft.notes).hint_text("Describe this document. Up to 1,000 characters."));
                        ui.label("Saves independently from Employment and Contact details. Dates are for reference; automated expiry alerts come later.");
                        #[cfg(feature="qa-screenshot")]
                        if std::env::var("HRIS_PREVIEW_SCROLL").as_deref()==Ok("bottom"){ui.scroll_to_cursor(Some(Align::BOTTOM));}
                    });
                    if let Some((message,true))=&self.notice {ui.add(egui::Label::new(RichText::new(message).color(ui.visuals().error_fg_color)).wrap());}
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {save=primary(ui,if editing{"Save document details"}else if retrying{"Retry upload"}else{"Upload document"});cancel=ui.button("Cancel").clicked();});
                });
            });
        if save {
            match draft.details().and_then(|_| {
                anyhow::ensure!(
                    editing || draft.file.is_some(),
                    "Choose a document to upload."
                );
                Ok(())
            }) {
                Ok(()) => {
                    self.close_document_on_success = true;
                    self.send(Command::Document(if editing {
                        DocumentAction::Edit(draft.clone())
                    } else {
                        DocumentAction::Upload(draft.clone())
                    }));
                }
                Err(error) => self.notice = Some((error.to_string(), true)),
            }
        }
        if (open && !cancel) || self.busy {
            self.document_form = Some(draft);
        }
    }

    fn open_correction(&mut self, employee: Uuid, day: NaiveDate) {
        let original = self
            .data
            .attendance
            .iter()
            .find(|a| a.employee_id == employee && a.work_date == day)
            .cloned();
        self.correction_form = Some(CorrectionDraft::new(employee, day, original, &self.data));
        self.notice = None;
    }

    fn attendance_forms(&mut self, ctx: &egui::Context) {
        if let Some(mut draft) = self.correction_form.take() {
            let mut open = true;
            let mut save = false;
            let mut cancel = false;
            let title = if draft.original.is_some() {
                "Correct attendance"
            } else {
                "Record attendance with reason"
            };
            let height = (ctx.content_rect().height() - 110.0).max(260.0);
            let error_height = if matches!(self.notice, Some((_, true))) {
                72.0
            } else {
                0.0
            };
            egui::Window::new(title).open(&mut open).collapsible(false).resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0]).default_width(530.0)
                .show(ctx, |ui| {
                    ui.add_enabled_ui(!self.busy, |ui| {
                        egui::ScrollArea::vertical().id_salt("correction_fields").max_height(height - 60.0 - error_height).show(ui, |ui| {
                            ui.label(RichText::new(self.data.employee_name(draft.employee_id)).strong().size(18.0));
                            ui.label(format!("{} · {}", draft.work_date.format("%A, %b %d, %Y"), self.data.company.timezone));
                            if let Some(before) = &draft.original {
                                ui.add_space(6.0);
                                ui.label(RichText::new("Currently recorded").strong());
                                ui.label(attendance_times(before, &self.data));
                            }
                            ui.add_space(8.0);
                            field(ui, "Check-in date *", &mut draft.checked_in_date, "YYYY-MM-DD; overnight shifts may check in the next day");
                            field(ui, "Check-in time *", &mut draft.checked_in, "24-hour time, e.g. 08:30");
                            field(ui, "Check-out date", &mut draft.checked_out_date, "YYYY-MM-DD (supports overnight shifts)");
                            field(ui, "Check-out time", &mut draft.checked_out, "24-hour time, or leave empty if still open");
                            if draft.checked_out.trim().is_empty() {
                                ui.label(RichText::new("An open shift prevents another check-in until it is closed. Use Correct to enter a missed check-out.").color(muted(ui)).small());
                            }
                            ui.add_space(6.0);
                            ui.label("Reason for this entry or correction *");
                            ui.add_sized([ui.available_width(), 72.0], egui::TextEdit::multiline(&mut draft.reason).hint_text("Explain the missed check-in or disputed time. Avoid sensitive personal details."));
                            ui.label(RichText::new(format!("{} / 500 characters", draft.reason.trim().chars().count())).color(muted(ui)).small());
                            ui.label("HR corrections apply immediately. History keeps the reason, previous times and who saved the change.");
                            #[cfg(feature = "qa-screenshot")]
                            if std::env::var("HRIS_PREVIEW_SCROLL").as_deref() == Ok("bottom") { ui.scroll_to_cursor(Some(Align::BOTTOM)); }
                        });
                        if let Some((text, true)) = &self.notice { ui.colored_label(ui.visuals().error_fg_color, text); }
                        ui.add_space(8.0);
                        ui.horizontal(|ui| {
                            save = primary(ui, "Save correction");
                            cancel = ui.button("Cancel").clicked();
                        });
                    });
                });
            if save {
                match draft.change(&self.data, chrono::Utc::now()) {
                    Ok(change) => {
                        self.close_on_success = true;
                        self.send(Command::Change(Action::CorrectAttendance(change)));
                    }
                    Err(error) => self.notice = Some((error.to_string(), true)),
                }
            }
            if (open && !cancel) || self.busy {
                self.correction_form = Some(draft);
            }
        }
        if let Some(id) = self.correction_history {
            let mut open = true;
            let mut close = false;
            egui::Window::new("Attendance correction history")
                .open(&mut open)
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .default_width(550.0)
                .show(ctx, |ui| {
                    if let Some(record) = self.data.attendance.iter().find(|a| a.id == id) {
                        ui.label(
                            RichText::new(self.data.employee_name(record.employee_id))
                                .size(18.0)
                                .strong(),
                        );
                        ui.label(format!(
                            "{} · {}",
                            record.work_date, self.data.company.timezone
                        ));
                        ui.label(format!("Current: {}", attendance_times(record, &self.data)));
                    }
                    egui::ScrollArea::vertical()
                        .id_salt("correction_history")
                        .max_height((ctx.content_rect().height() - 240.0).max(180.0))
                        .show(ui, |ui| {
                            let mut events: Vec<_> = self
                                .data
                                .corrections
                                .iter()
                                .filter(|c| c.attendance_id == id)
                                .collect();
                            events.sort_by(|a, b| {
                                b.created_at.cmp(&a.created_at).then(b.id.cmp(&a.id))
                            });
                            if events.is_empty() {
                                ui.label("No HR corrections have been made to this record.");
                            }
                            for event in events {
                                ui.separator();
                                ui.label(
                                    RichText::new(format!(
                                        "{} · {}",
                                        event
                                            .created_at
                                            .with_timezone(&self.data.company.tz())
                                            .format("%b %d, %Y %H:%M:%S"),
                                        event.actor_name
                                    ))
                                    .strong(),
                                );
                                ui.label(
                                    RichText::new(event.actor_role.label())
                                        .color(muted(ui))
                                        .small(),
                                );
                                ui.label(format!("Reason: {}", event.reason));
                                ui.label(format!(
                                    "Before: {}",
                                    event
                                        .before_record
                                        .as_ref()
                                        .map(|a| attendance_times(a, &self.data))
                                        .unwrap_or_else(|| "No attendance record".into())
                                ));
                                ui.label(format!(
                                    "After: {}",
                                    attendance_times(&event.after_record, &self.data)
                                ));
                            }
                        });
                    close = ui.button("Close").clicked();
                });
            if !open || close {
                self.correction_history = None;
            }
        }
    }

    fn forms(&mut self, ctx: &egui::Context) {
        self.document_dialog(ctx);
        self.employment_option_dialog(ctx);
        self.attendance_forms(ctx);
        if let Some(mut draft) = self.leave_form.take() {
            let mut open = true;
            let mut save = false;
            let mut cancel = false;
            egui::Window::new("Request leave").open(&mut open).collapsible(false).resizable(false).vscroll(false).anchor(egui::Align2::CENTER_CENTER,[0.0,0.0]).default_width(440.0)
                .show(ctx,|ui| {
                    ui.add_enabled_ui(!self.busy,|ui| {
                        let body=egui::ScrollArea::vertical().id_salt("leave_request_fields").max_height((ctx.content_rect().height()-240.0).max(160.0));
                        #[cfg(feature="qa-screenshot")]
                        let body=body.stick_to_bottom(std::env::var("HRIS_PREVIEW_SCROLL").as_deref()==Ok("bottom"));
                        body.show(ui,|ui|{
                            self.leave_request_fields(ui,&mut draft);
                            ui.label("Note (optional)");ui.add_sized([ui.available_width(),70.0],egui::TextEdit::multiline(&mut draft.reason).hint_text("Add only information needed to review this request."));
                            ui.label(RichText::new("Exclude sensitive medical details. Requests cannot cross calendar years.").color(muted(ui)).size(11.0));
                            if let Some((text,true))=&self.notice {ui.colored_label(ui.visuals().error_fg_color,text);}
                        });
                        ui.separator();
                        ui.horizontal(|ui|{save=primary(ui,"Submit request");cancel=ui.button("Cancel").clicked();});
                    });
                });
            if save {
                match crate::leave::prepare(&self.data, &draft) {
                    Ok(_) => {
                        self.close_on_success = true;
                        self.send(Command::Change(Action::RequestLeave(draft.clone())));
                    }
                    Err(error) => self.notice = Some((error.to_string(), true)),
                }
            }
            if (open && !cancel) || self.busy {
                self.leave_form = Some(draft);
            }
        }
    }
}

fn attendance_times(record: &Attendance, data: &Snapshot) -> String {
    format!(
        "{} to {}",
        record
            .checked_in_at
            .with_timezone(&data.company.tz())
            .format("%b %d, %Y %H:%M:%S"),
        record
            .checked_out_at
            .map(|t| t
                .with_timezone(&data.company.tz())
                .format("%b %d, %Y %H:%M:%S")
                .to_string())
            .unwrap_or_else(|| "Open shift".into())
    )
}

impl eframe::App for HrisApp {
    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.receive(&ctx);
        let theme = if self.page == Page::Settings {
            self.display_draft.theme
        } else {
            self.account
                .as_ref()
                .map(|a| a.display.theme)
                .unwrap_or_default()
        };
        ctx.set_theme(theme.preference());
        if (self.page == Page::Accounts && !self.data.access.role.manages_accounts())
            || (matches!(self.page, Page::Hiring | Page::Exits) && !self.data.access.uses_desktop())
        {
            self.navigate(Page::Overview, &ctx);
        }
        if self.data.mode == Mode::SignedOut {
            egui::CentralPanel::default()
                .frame(Frame::new().fill(ui.visuals().panel_fill).inner_margin(28))
                .show(ui, |ui| {
                    egui::ScrollArea::vertical().show(ui, |ui| self.sign_in(ui));
                });
        } else {
            let form_open = self.leave_form.is_some()
                || self.document_form.is_some()
                || self.employment_option_form.is_some()
                || self.correction_form.is_some()
                || self.correction_history.is_some()
                || self.session_confirmation.is_some()
                || self.update_confirmation;
            egui::Panel::left("sidebar")
                .resizable(false)
                .exact_size(220.0)
                .frame(Frame::new().fill(NAVY).inner_margin(20))
                .show(ui, |ui| {
                    ui.add_enabled_ui(
                        !self.busy
                            && !form_open
                            && self.employee_form.is_none()
                            && !self.staff_provision_state.has_draft()
                            && !self.hiring_state.editing()
                            && !self.recruiting_state.editing()
                            && !self.onboarding_state.editing()
                            && !self.exit_state.editing()
                            && !self.payroll_state.editing()
                            && !(self.performance_state.editing() || self.learning_state.editing() || self.career_state.editing()),
                        |ui| self.sidebar(ui),
                    );
                });
            egui::CentralPanel::default()
                .frame(Frame::new().fill(ui.visuals().panel_fill).inner_margin(28))
                .show(ui, |ui| {
                    if self.staff_provision_state.form.is_some(){
                        self.header(ui);
                        egui::ScrollArea::vertical().id_salt("staff_provision_form").show(ui,|ui|self.staff_provision_form(ui));
                        return;
                    }
                    if matches!(self.page, Page::Employees | Page::Accounts)
                        && self.employee_form.is_some()
                    {
                        ui.add_enabled_ui(!form_open, |ui| {
                            self.header(ui);
                            ui.add_enabled_ui(!self.busy, |ui| self.employee_record(ui));
                        });
                        return;
                    }
                    let scroll_id=("main_scroll",self.page as u8);
                    let scroll=if self.page==Page::Payroll&&self.payroll_state.tab==13{
                        egui::ScrollArea::vertical().id_salt((scroll_id,self.payroll_state.government.scroll_key()))
                    }else{egui::ScrollArea::vertical().id_salt(scroll_id)};
                    #[cfg(feature = "qa-screenshot")]
                    let scroll = if std::env::var("HRIS_PREVIEW_SCROLL").as_deref() == Ok("bottom")
                    {
                        scroll.stick_to_bottom(true)
                    } else {
                        scroll
                    }
                    .animated(false);
                    scroll.show(ui, |ui| {
                        ui.add_enabled_ui(!form_open, |ui| {
                            self.header(ui);
                            ui.add_enabled_ui(!self.busy, |ui| match self.page {
                                Page::Overview => self.overview(ui),
                                Page::Employees => self.employees(ui),
                                Page::Attendance => self.attendance(ui),
                                Page::Leave => self.leave(ui),
                                Page::Payroll => self.payroll(ui),
                                Page::Reports => self.reports(ui),
                                Page::Hiring => self.hiring(ui),
                                Page::Exits => self.exits(ui),
                                Page::Accounts if self.data.access.role.manages_accounts() => {
                                    self.accounts(ui)
                                }
                                Page::Accounts => {
                                    empty(ui, "Only administrators can manage accounts.")
                                }
                                Page::Settings => self.settings(ui),
                                Page::Profile => self.profile(ui),
                                Page::Notifications => self.notifications_page(ui),
                            });
                        });
                    });
                });
            self.forms(&ctx);
            self.confirm_session_action(&ctx);
            self.confirm_update(&ctx);
        }
        #[cfg(feature = "qa-screenshot")]
        {
            self.qa_frames += 1;
            ctx.send_viewport_cmd(egui::ViewportCommand::RequestPaintWhileHidden);
            if self.qa_frames >= 4
                && self.qa_started.elapsed() >= std::time::Duration::from_millis(400)
            {
                if let Ok(path) = std::env::var("HRIS_SCREENSHOT_TO") {
                    ctx.request_screenshot(move |pixels| {
                        let result = image::save_buffer(
                            &path,
                            pixels.as_raw(),
                            pixels.width() as u32,
                            pixels.height() as u32,
                            image::ColorType::Rgba8,
                        );
                        if let Err(error) = result {
                            eprintln!("Screenshot failed: {error}");
                            std::process::exit(1);
                        }
                        std::process::exit(0);
                    });
                }
            }
            ctx.request_repaint_after(std::time::Duration::from_millis(30));
        }
        ctx.request_repaint_after(std::time::Duration::from_secs(20));
    }
}

fn same_contacts(a: &EmployeeContacts, b: &EmployeeContacts) -> bool {
    let mut a = a.clone();
    let mut b = b.clone();
    a.updated_at = None;
    b.updated_at = None;
    a == b
}

fn sync_profile_contacts(
    previous: &Snapshot,
    next: &Snapshot,
    draft: &mut Option<EmployeeContacts>,
) {
    if next.mode == Mode::SignedOut || previous.access.employee_id != next.access.employee_id {
        *draft = None;
        return;
    }
    let Some(id) = next.access.employee_id else {
        *draft = None;
        return;
    };
    let old = previous
        .own_contacts()
        .cloned()
        .unwrap_or_else(|| EmployeeContacts::empty(previous.company.id, id));
    let new = next
        .own_contacts()
        .cloned()
        .unwrap_or_else(|| EmployeeContacts::empty(next.company.id, id));
    if draft
        .as_ref()
        .is_some_and(|draft| same_contacts(draft, &old) || same_contacts(draft, &new))
    {
        *draft = Some(new);
    }
}

fn company_fields_equal(a: &Company, b: &Company) -> bool {
    a.name == b.name && a.timezone == b.timezone && a.annual_leave_days == b.annual_leave_days
}

fn company_logo(ui: &mut Ui, texture: Option<&egui::TextureHandle>, name: &str, size: f32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(size), egui::Sense::hover());
    ui.painter()
        .circle_stroke(rect.center(), size / 2.0 - 2.0, Stroke::new(1.5, TEAL));
    if let Some(texture) = texture {
        ui.painter().image(
            texture.id(),
            rect,
            egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
            Color32::WHITE,
        );
    } else {
        ui.painter().text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            branding::initials(name),
            FontId::proportional(size * 0.34),
            if ui.visuals().dark_mode {
                Color32::from_rgb(222, 242, 237)
            } else {
                TEAL
            },
        );
    }
}

fn employment_history(ui: &mut Ui, data: &Snapshot, employee: Uuid, time_format: &str) {
    ui.label("Saved changes, newest first. Unsaved edits are not shown.")
        .on_hover_text("History is read-only. Correct the Employment fields to add a new event; the original entry remains.");
    let events = crate::history::timeline(data, employee);
    if events.is_empty() {
        empty(
            ui,
            "No saved employment history is available. Refresh to load the latest records.",
        );
    }
    for (index, event) in events.into_iter().enumerate() {
        ui.add_space(10.0);
        let changes = event.changes();
        let title = if event.kind == crate::history::HistoryKind::Updated {
            format!(
                "{} ({} {})",
                event.kind.label(),
                changes.len(),
                if changes.len() == 1 {
                    "field"
                } else {
                    "fields"
                }
            )
        } else {
            event.kind.label().into()
        };
        ui.horizontal_wrapped(|ui| {
            ui.label(
                RichText::new(account_timestamp(
                    &event.recorded_at.to_rfc3339(),
                    data.company.tz(),
                    time_format,
                ))
                .strong(),
            );
            ui.label(match event.actor_role {
                Some(role) => format!("Saved by {} ({})", event.actor_name, role.label()),
                None => "Saved by System".into(),
            });
        });
        egui::CollapsingHeader::new(title).id_salt(("employment-history",event.id)).default_open(index==0).show(ui,|ui| {
            if event.kind==crate::history::HistoryKind::StartingSnapshot {
                ui.label("Starting snapshot captured when history was enabled. Earlier changes are not recorded.");
            }
            let updating=event.before_record.is_some();
            let value_width=((ui.available_width()-155.0)/if updating {2.0}else{1.0}).clamp(85.0,320.0);
            egui::Grid::new(("employment-history-fields",event.id)).striped(true).spacing([12.0,10.0]).max_col_width(value_width).show(ui,|ui| {
                ui.strong("Field");
                if updating {ui.strong("Before");}
                ui.strong(if updating {"After"}else{"Saved value"});ui.end_row();
                for change in changes {
                    // Match the value cells' top-down layout: a bare Grid label is vertically centered.
                    let _field=ui.vertical(|ui|ui.label(change.label)).inner;
                    #[cfg(feature="qa-screenshot")]
                    if index==0 && matches!((std::env::var("HRIS_PREVIEW_SCROLL").as_deref(),change.label),
                        (Ok("department"),"Department") | (Ok("job-title"),"Job title")) {
                        _field.scroll_to_me(Some(Align::Min));
                    }
                    if let Some(before)=change.before.as_ref(){ui.vertical(|ui|{ui.set_width(value_width);ui.add(egui::Label::new(before).wrap());});}
                    let after=if change.before.as_ref()==Some(&change.after){format!("{} (different record)",change.after)}else{change.after};
                    ui.vertical(|ui|{ui.set_width(value_width);ui.add(egui::Label::new(after).wrap());});ui.end_row();
                }
            });
        });
        ui.separator();
    }
}

fn employment_assignment(
    ui: &mut Ui,
    kind: EmploymentKind,
    data: &Snapshot,
    selected: &mut Option<Uuid>,
    employee: Option<Uuid>,
) {
    ui.label(kind.label());
    let existing = employee
        .and_then(|id| data.employees.iter().find(|e| e.id == id))
        .and_then(|e| kind.assignment(e));
    egui::ComboBox::from_id_salt(("employment_assignment", kind.table()))
        .width(250.0)
        .truncate()
        .selected_text(kind.display(data, *selected))
        .show_ui(ui, |ui| {
            ui.selectable_value(selected, None, "Not set");
            for option in kind
                .choices(data)
                .iter()
                .filter(|c| c.active || Some(c.id) == existing)
            {
                ui.selectable_value(
                    selected,
                    Some(option.id),
                    format!(
                        "{}{}",
                        option.name,
                        if option.active {
                            ""
                        } else {
                            " (inactive; existing assignment)"
                        }
                    ),
                );
            }
        });
    if kind.choices(data).iter().all(|c| !c.active) {
        ui.label(
            RichText::new(
                "An Administrator can add or enable choices in Settings > Employment setup.",
            )
            .color(muted(ui))
            .small(),
        );
    }
}

fn employment_filter(
    ui: &mut Ui,
    kind: EmploymentKind,
    data: &Snapshot,
    selected: &mut EmploymentFilter,
) {
    let label = match *selected {
        EmploymentFilter::All => format!("All {}", kind.plural().to_lowercase()),
        EmploymentFilter::NotSet => format!("{}: not set", kind.label()),
        EmploymentFilter::Choice(id) => kind.display(data, Some(id)),
    };
    egui::ComboBox::from_id_salt(("employment_filter", kind.table()))
        .width(210.0)
        .truncate()
        .selected_text(label)
        .show_ui(ui, |ui| {
            ui.selectable_value(
                selected,
                EmploymentFilter::All,
                format!("All {}", kind.plural().to_lowercase()),
            );
            ui.selectable_value(selected, EmploymentFilter::NotSet, "Not set");
            for option in kind.choices(data) {
                ui.selectable_value(
                    selected,
                    EmploymentFilter::Choice(option.id),
                    kind.display(data, Some(option.id)),
                );
            }
        });
}

fn employment_fields(ui: &mut Ui, draft: &mut EmployeeDraft, data: &Snapshot) {
    field(ui, "Employee code", &mut draft.employee_code, "EMP-001");
    field(ui, "Full name", &mut draft.full_name, "Employee name");
    field(ui, "Department", &mut draft.department, "Not set");
    field(ui, "Job title", &mut draft.job_title, "Not set");
    let _choices = ui.scope(|ui| {
        employment_assignment(
            ui,
            EmploymentKind::EmploymentType,
            data,
            &mut draft.employment_type_id,
            draft.id,
        );
        employment_assignment(
            ui,
            EmploymentKind::WorkLocation,
            data,
            &mut draft.work_location_id,
            draft.id,
        );
    });
    #[cfg(feature = "qa-screenshot")]
    if std::env::var("HRIS_PREVIEW_FORM").is_ok_and(|v| v.starts_with("employment-")) {
        ui.scroll_to_rect(_choices.response.rect, Some(Align::Center));
    }
    field(
        ui,
        "Date joined (optional)",
        &mut draft.joined_on,
        "YYYY-MM-DD; leave blank if not set",
    );
    ui.label("Reports to");
    egui::ComboBox::from_id_salt("employee_reports_to")
        .selected_text(
            draft
                .reports_to
                .map(|id| data.employee_name(id))
                .unwrap_or("No assigned manager"),
        )
        .show_ui(ui, |ui| {
            ui.selectable_value(&mut draft.reports_to, None, "No assigned manager");
            for employee in data.active().filter(|e| Some(e.id) != draft.id) {
                ui.selectable_value(
                    &mut draft.reports_to,
                    Some(employee.id),
                    &employee.full_name,
                );
            }
        });
    ui.horizontal_wrapped(|ui| {
        ui.label("Annual leave allowance");
        ui.add(
            egui::DragValue::new(&mut draft.annual_leave_days)
                .range(0..=366)
                .suffix(" days"),
        );
    });
    if data.leave_setup.dated.as_ref().is_some_and(|c|c.enabled) {ui.label(RichText::new("Future-year default. The captured current-year entitlement stays unchanged; use Leave > Credit adjustments for a reviewed current-year correction.").small().color(muted(ui)));}
    egui::ComboBox::from_id_salt("employee_record_status")
        .selected_text(draft.status.label())
        .show_ui(ui, |ui| {
            ui.selectable_value(&mut draft.status, EmployeeStatus::Active, "Active");
            ui.selectable_value(&mut draft.status, EmployeeStatus::Archived, "Archived");
        });
}

fn employment_summary(ui: &mut Ui, employee: &Employee, data: &Snapshot) {
    let value = |v: &str| {
        if v.trim().is_empty() {
            "Not set".to_owned()
        } else {
            v.to_owned()
        }
    };
    let fields = [
        ("Employee code", employee.employee_code.clone()),
        ("Full name", employee.full_name.clone()),
        ("Department", value(&employee.department)),
        ("Job title", value(&employee.job_title)),
        (
            "Employment type",
            EmploymentKind::EmploymentType.display(data, employee.employment_type_id),
        ),
        (
            "Work location",
            EmploymentKind::WorkLocation.display(data, employee.work_location_id),
        ),
        (
            "Date joined",
            employee
                .joined_on
                .map(|d| d.to_string())
                .unwrap_or("Not set".into()),
        ),
        (
            "Reports to",
            employee
                .reports_to
                .map(|id| data.employee_name(id).into())
                .unwrap_or("No assigned manager".into()),
        ),
        (
            "Annual leave allowance",
            format!("{} days", employee.annual_leave_days),
        ),
        ("Status", employee.status.label().into()),
    ];
    egui::Grid::new("profile_employment")
        .num_columns(2)
        .max_col_width((ui.available_width() - 180.0).clamp(160.0, 340.0))
        .spacing([24.0, 14.0])
        .show(ui, |ui| {
            for (name, value) in fields {
                ui.label(RichText::new(name).color(muted(ui)));
                ui.add(egui::Label::new(value).wrap());
                ui.end_row();
            }
        });
}

fn contact_fields(ui: &mut Ui, contacts: &mut EmployeeContacts, emergency: bool) {
    if emergency {
        field(
            ui,
            "Contact name",
            &mut contacts.emergency_name,
            "Full name",
        );
        field(
            ui,
            "Relationship",
            &mut contacts.emergency_relationship,
            "e.g. Parent, spouse, sibling",
        );
        field(ui, "Primary phone", &mut contacts.emergency_phone, "+63 …");
        field(
            ui,
            "Alternate phone",
            &mut contacts.emergency_alternate_phone,
            "Optional second number",
        );
        ui.label("Enter name, relationship and primary phone together, or leave all emergency fields blank.");
    } else {
        field(ui, "Phone", &mut contacts.phone, "+63 …");
        field(
            ui,
            "Personal email",
            &mut contacts.personal_email,
            "Optional email address",
        );
        ui.label("Home address");
        ui.add(
            egui::TextEdit::multiline(&mut contacts.address)
                .desired_width(f32::INFINITY)
                .desired_rows(3)
                .hint_text("Optional address; up to 500 characters"),
        );
        ui.label("Personal email does not change your sign-in email.");
    }
}

fn staff_access_fields(ui: &mut Ui, draft: &mut AccountDraft, data: &Snapshot) {
    let editing = draft.user_id.is_some();
    let protected = draft
        .user_id
        .is_some_and(|id| id == data.company.owner_id || id == data.access.user_id);
    ui.label(RichText::new("Staff role & sign-in").strong());
    if editing {
        ui.label(format!("Sign-in email: {}", draft.email));
    } else {
        field(ui, "Sign-in email", &mut draft.email, "Work email");
    }
    ui.add_enabled_ui(!protected, |ui| {
        ui.checkbox(&mut draft.active, "Sign-in access enabled");
        ui.label("Desktop role");
        let label = |role: Role| {
            if role == Role::Employee {
                "No staff role (employee portal)"
            } else {
                role.label()
            }
        };
        egui::ComboBox::from_id_salt("staff_role")
            .selected_text(label(draft.role))
            .show_ui(ui, |ui| {
                for role in Role::ALL {
                    ui.selectable_value(&mut draft.role, role, label(role));
                }
            });
    });
    if protected {
        ui.label("Your administrator role is protected. Employment details are managed in the other tabs.");
    }
    ui.label(RichText::new(draft.role.description()).color(muted(ui)));
    if !protected {
        ui.label(RichText::new("To suspend access, clear Sign-in access enabled and save. To remove the staff role while retaining the employee, choose No staff role. Show disabled in Account Manager reveals suspended staff.").color(muted(ui)).small());
    }
    if !editing {
        password_field(
            ui,
            "Initial password",
            "account-initial-password",
            &mut draft.password,
            false,
        );
        password_field(
            ui,
            "Confirm initial password",
            "account-initial-confirm",
            &mut draft.confirmation,
            false,
        );
        ui.label("Use 12–128 characters. Share privately. Email confirmation is required before sign-in.");
    }
    ui.label(
        "This role applies to this employee. Employment and contact details are saved together.",
    );
}

fn public_access_draft(draft: &AccountDraft) -> AccountDraft {
    AccountDraft {
        user_id: draft.user_id,
        email: draft.email.clone(),
        role: draft.role,
        active: draft.active,
        employee_id: draft.employee_id,
        manager_user_id: None,
        create_new: draft.create_new,
        employee_name: draft.employee_name.clone(),
        ..Default::default()
    }
}

fn sign_in_layout(
    ui: &mut Ui,
    logo: Option<&egui::TextureHandle>,
    company_name: &str,
    contents: impl FnOnce(&mut Ui),
) -> egui::Rect {
    ui.horizontal(|ui| {
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.label(
                RichText::new(format!("v{}", updater::VERSION))
                    .small()
                    .color(muted(ui)),
            );
        });
    });
    ui.vertical_centered(|ui| {
        ui.add_space(12.0);
        if logo.is_some() || !company_name.is_empty() {
            company_logo(ui, logo, company_name, 64.0);
            ui.add_space(4.0);
        }
        ui.heading(
            RichText::new(if company_name.is_empty() {
                "HRIS Desktop"
            } else {
                company_name
            })
            .size(22.0)
            .strong(),
        );
        ui.add_space(12.0);
        Frame::new()
            .fill(ui.visuals().window_fill)
            .stroke(Stroke::new(
                1.0,
                ui.visuals().widgets.noninteractive.bg_stroke.color,
            ))
            .corner_radius(12)
            .inner_margin(28)
            .show(ui, |ui| {
                ui.set_width(ui.available_width().min(420.0));
                ui.with_layout(Layout::top_down(Align::Min), contents);
            })
            .response
            .rect
    })
    .inner
}

fn sync_account_drafts(
    previous: Option<&AccountProfile>,
    next: Option<&AccountProfile>,
    personal: &mut PersonalDetails,
    display: &mut DisplayPreferences,
) -> bool {
    let changed_identity = previous.map(|a| a.id) != next.map(|a| a.id);
    // A photo/security update must not discard edits in another account section.
    if changed_identity || previous.map(|a| &a.details) != next.map(|a| &a.details) {
        *personal = next.map(|a| a.details.clone()).unwrap_or_default();
    }
    let changed_display =
        changed_identity || previous.map(|a| a.display) != next.map(|a| a.display);
    if changed_display {
        *display = next.map(|a| a.display).unwrap_or_default();
    }
    changed_display
}

fn clear_login_history(ctx: &egui::Context) {
    clear_edit_history(ctx, &["hris-login-email", "hris-login-password"]);
}

fn clear_edit_history(ctx: &egui::Context, names: &[&str]) {
    for name in names {
        let id = egui::Id::unique(*name);
        if let Some(mut state) = egui::text_edit::TextEditState::load(ctx, id) {
            state.clear_undoer();
            state.store(ctx, id);
        }
    }
}

fn login_controls(ui: &mut Ui, email: &mut String, password: &mut String, enabled: bool) -> bool {
    let submitted = ui
        .add_enabled_ui(enabled, |ui| {
            ui.label("Email");
            let email_response = ui.add_sized(
                [ui.available_width(), 34.0],
                egui::TextEdit::singleline(email)
                    .id(egui::Id::unique("hris-login-email"))
                    .hint_text("Your work email"),
            );
            ui.label("Password");
            let password_response = ui.add_sized(
                [ui.available_width(), 34.0],
                egui::TextEdit::singleline(password)
                    .id(egui::Id::unique("hris-login-password"))
                    .password(true)
                    .hint_text("Your HRIS password"),
            );
            let enter = ui.input(|input| input.key_pressed(egui::Key::Enter))
                && (email_response.has_focus()
                    || email_response.lost_focus()
                    || password_response.has_focus()
                    || password_response.lost_focus());
            ui.add_space(8.0);
            primary(ui, "Sign in") || enter
        })
        .inner;
    enabled && submitted
}

fn profile_link(
    ui: &mut Ui,
    texture: Option<&egui::TextureHandle>,
    name: &str,
    role: &str,
    selected: bool,
) -> egui::Response {
    let (rect, response) =
        ui.allocate_exact_size(Vec2::new(ui.available_width(), 64.0), egui::Sense::click());
    response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::Role::Button, ui.is_enabled(), "Open profile")
    });
    ui.painter().rect_filled(
        rect,
        8,
        if selected {
            TEAL
        } else if response.hovered() {
            Color32::from_rgb(45, 65, 81)
        } else {
            Color32::from_rgb(35, 53, 70)
        },
    );
    if response.has_focus() {
        ui.painter().rect_stroke(
            rect,
            8,
            Stroke::new(1.5, Color32::WHITE),
            egui::StrokeKind::Inside,
        );
    }
    let mut contents = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(rect.shrink2(Vec2::new(8.0, 10.0)))
            .layout(Layout::top_down(Align::Min)),
    );
    contents.horizontal(|ui| {
        avatar(ui, texture, name, 40.0);
        ui.vertical(|ui| {
            ui.add(
                egui::Label::new(
                    RichText::new(name)
                        .color(Color32::WHITE)
                        .strong()
                        .size(13.0),
                )
                .truncate(),
            );
            ui.add(egui::Label::new(
                RichText::new(role)
                    .color(Color32::from_rgb(175, 206, 208))
                    .size(11.0),
            ).truncate()).on_hover_text(role);
        });
    });
    response
        .on_hover_text("Open profile")
        .on_hover_cursor(egui::CursorIcon::PointingHand)
}

fn avatar(ui: &mut Ui, texture: Option<&egui::TextureHandle>, name: &str, size: f32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(size), egui::Sense::hover());
    ui.painter()
        .circle_filled(rect.center(), size / 2.0, Color32::from_rgb(222, 242, 237));
    if let Some(texture) = texture {
        ui.painter().image(
            texture.id(),
            rect,
            egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1.0, 1.0)),
            Color32::WHITE,
        );
    } else {
        ui.painter().text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            branding::initials(name),
            FontId::proportional(size * 0.34),
            TEAL,
        );
    }
}

fn profile_field(ui: &mut Ui, label: &str, id: &str, value: &mut String, hint: &str) {
    ui.label(RichText::new(label).size(13.0));
    ui.add_sized(
        [ui.available_width().min(620.0), 34.0],
        egui::TextEdit::singleline(value)
            .id(egui::Id::unique(id))
            .hint_text(hint),
    );
}

fn password_field(ui: &mut Ui, label: &str, id: &str, value: &mut String, show: bool) {
    ui.label(RichText::new(label).size(13.0));
    ui.add_sized(
        [ui.available_width().min(620.0), 34.0],
        egui::TextEdit::singleline(value)
            .id(egui::Id::unique(id))
            .password(!show),
    );
}

fn account_timestamp(value: &str, timezone: chrono_tz::Tz, time_format: &str) -> String {
    chrono::DateTime::parse_from_rfc3339(value)
        .map(|date| {
            date.with_timezone(&timezone)
                .format(&format!("%b %d, %Y · {time_format}"))
                .to_string()
        })
        .unwrap_or_else(|_| "Unavailable".into())
}

#[cfg(test)]
mod login_tests {
    use super::*;

    fn enter_from(field: &str, enabled: bool) -> (usize, String) {
        let ctx = egui::Context::default();
        configure_theme(&ctx);
        let mut email = "qa@example.invalid".to_string();
        let mut password = "synthetic fixture".to_string();
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                Vec2::new(500.0, 600.0),
            )),
            ..Default::default()
        };
        let mut output = ctx.run_ui(input.clone(), |ui| {
            login_controls(ui, &mut email, &mut password, true);
        });
        output.textures_delta.clear(); // This headless harness intentionally has no painter.
        ctx.memory_mut(|memory| memory.request_focus(egui::Id::unique(field)));
        let mut keyboard = input.clone();
        keyboard.events.push(egui::Event::Key {
            key: egui::Key::Enter,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        });
        let mut submissions = 0;
        let mut available = enabled;
        let mut output = ctx.run_ui(keyboard, |ui| {
            if login_controls(ui, &mut email, &mut password, available) {
                submissions += 1;
                available = false; // The real app disables submission while its worker is busy.
            }
        });
        output.textures_delta.clear();
        let mut output = ctx.run_ui(input, |ui| {
            assert!(!login_controls(ui, &mut email, &mut password, available));
        });
        output.textures_delta.clear();
        (submissions, password)
    }

    #[test]
    fn enter_in_email_or_password_submits_once_without_inserting_a_newline() {
        for field in ["hris-login-email", "hris-login-password"] {
            let (submissions, password) = enter_from(field, true);
            assert_eq!(submissions, 1, "Enter must submit from {field}");
            assert_eq!(password, "synthetic fixture");
        }
    }

    #[test]
    fn enter_cannot_resubmit_while_sign_in_is_busy() {
        assert_eq!(enter_from("hris-login-password", false).0, 0);
    }
}

#[cfg(test)]
mod profile_navigation_tests {
    use super::*;

    #[test]
    fn photo_and_security_updates_preserve_unsaved_personal_and_display_edits() {
        let previous = AccountProfile::demo();
        let mut next = previous.clone();
        next.photo = Some(vec![1, 2, 3]);
        next.pending_email = "pending@example.invalid".into();
        let mut personal = PersonalDetails {
            full_name: "Unsaved name".into(),
            ..Default::default()
        };
        let mut display = DisplayPreferences {
            scale_percent: 115,
            clock_24h: false,
            ..Default::default()
        };
        assert!(!sync_account_drafts(
            Some(&previous),
            Some(&next),
            &mut personal,
            &mut display
        ));
        assert_eq!(personal.full_name, "Unsaved name");
        assert_eq!(display.scale_percent, 115);
        next.details.full_name = "Saved name".into();
        assert!(!sync_account_drafts(
            Some(&previous),
            Some(&next),
            &mut personal,
            &mut display
        ));
        assert_eq!(personal.full_name, "Saved name");
        assert_eq!(display.scale_percent, 115);
        assert!(sync_account_drafts(
            Some(&next),
            None,
            &mut personal,
            &mut display
        ));
        assert!(personal.full_name.is_empty());
        assert_eq!(display.scale_percent, 100);
    }

    #[test]
    fn clicking_the_avatar_activates_the_account_control_with_or_without_a_photo() {
        for with_photo in [false, true] {
            let ctx = egui::Context::default();
            let texture = with_photo.then(|| {
                ctx.load_texture(
                    "fixture",
                    egui::ColorImage::filled([2, 2], Color32::WHITE),
                    egui::TextureOptions::LINEAR,
                )
            });
            let input = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    Vec2::new(220.0, 200.0),
                )),
                ..Default::default()
            };
            let mut rect = egui::Rect::NOTHING;
            let mut clicked = false;
            let mut render = |input| {
                let mut output = ctx.run_ui(input, |ui| {
                    ui.with_layout(Layout::bottom_up(Align::Min), |ui| {
                        let response =
                            profile_link(ui, texture.as_ref(), "Alex", "Administrator", false);
                        rect = response.rect;
                        clicked = response.clicked();
                    });
                });
                output.textures_delta.clear();
                (rect, clicked)
            };
            let (rect, _) = render(input.clone());
            let pos = rect.min + Vec2::new(28.0, 32.0);
            for pressed in [true, false] {
                let mut pointer = input.clone();
                pointer.events = vec![
                    egui::Event::PointerMoved(pos),
                    egui::Event::PointerButton {
                        pos,
                        button: egui::PointerButton::Primary,
                        pressed,
                        modifiers: egui::Modifiers::NONE,
                    },
                ];
                let (_, activated) = render(pointer);
                if !pressed {
                    assert!(
                        activated,
                        "The photo must remain a clickable account control"
                    );
                }
            }
        }
    }
}

fn month_start(date: NaiveDate) -> NaiveDate {
    date.with_day(1).unwrap()
}

fn calendar_cells(month: NaiveDate) -> [Option<NaiveDate>; 42] {
    let first = month_start(month);
    let offset = first.weekday().num_days_from_monday() as i64;
    std::array::from_fn(|index| {
        first
            .checked_add_signed(Duration::days(index as i64 - offset))
            .filter(|day| day.month() == first.month() && day.year() == first.year())
    })
}

fn attendance_calendar(
    ui: &mut Ui,
    selected: &mut NaiveDate,
    month: &mut NaiveDate,
    today: NaiveDate,
    data: &Snapshot,
) -> bool {
    calendar_control(ui, selected, month, today, today, data)
}

fn calendar_control(
    ui: &mut Ui,
    selected: &mut NaiveDate,
    month: &mut NaiveDate,
    today: NaiveDate,
    latest: NaiveDate,
    data: &Snapshot,
) -> bool {
    ui.set_min_width(280.0);
    ui.spacing_mut().item_spacing = Vec2::new(6.0, 5.0);
    let mut chosen = false;
    ui.horizontal(|ui| {
        if ui
            .add_enabled(
                *month > NaiveDate::from_ymd_opt(1900, 1, 1).unwrap(),
                egui::Button::new("<"),
            )
            .clicked()
        {
            *month = month.checked_sub_months(Months::new(1)).unwrap();
        }
        let mut year = month.year();
        let mut number = month.month();
        egui::ComboBox::from_id_salt("attendance_calendar_month")
            .width(116.0)
            .selected_text(month.format("%B").to_string())
            .show_ui(ui, |ui| {
                for value in 1..=12 {
                    ui.add_enabled_ui(year < latest.year() || value <= latest.month(), |ui| {
                        ui.selectable_value(
                            &mut number,
                            value,
                            NaiveDate::from_ymd_opt(2024, value, 1)
                                .unwrap()
                                .format("%B")
                                .to_string(),
                        );
                    });
                }
            });
        egui::ComboBox::from_id_salt("attendance_calendar_year")
            .width(64.0)
            .height(180.0)
            .selected_text(year.to_string())
            .show_ui(ui, |ui| {
                for value in (1900..=latest.year()).rev() {
                    ui.selectable_value(&mut year, value, value.to_string());
                }
            });
        if year == latest.year() {
            number = number.min(latest.month());
        }
        *month = NaiveDate::from_ymd_opt(year, number, 1).unwrap();
        if ui
            .add_enabled(*month < month_start(latest), egui::Button::new(">"))
            .clicked()
        {
            *month = month.checked_add_months(Months::new(1)).unwrap();
        }
    });
    ui.separator();
    egui::Grid::new("attendance_calendar_days")
        .spacing([5.0, 4.0])
        .show(ui, |ui| {
            for day in ["Mo", "Tu", "We", "Th", "Fr", "Sa", "Su"] {
                ui.label(RichText::new(day).small().color(muted(ui)));
            }
            ui.end_row();
            let cells = calendar_cells(*month);
            let visible_cells = cells
                .iter()
                .rposition(Option::is_some)
                .map(|last| (last / 7 + 1) * 7)
                .unwrap_or(35);
            for (index, cell) in cells.into_iter().take(visible_cells).enumerate() {
                if let Some(date) = cell {
                    let count = data
                        .attendance
                        .iter()
                        .filter(|record| record.work_date == date)
                        .count();
                    let mut text = RichText::new(date.day().to_string());
                    if count > 0 {
                        text = text.color(accent(ui)).strong();
                    }
                    let mut button = egui::Button::new(text).selected(date == *selected);
                    if date == today {
                        button = button.stroke(Stroke::new(1.0, accent(ui)));
                    }
                    if ui
                        .add_enabled(date <= latest, button.min_size(Vec2::new(34.0, 30.0)))
                        .on_hover_text(format!(
                            "{} · {count} check-in record(s)",
                            date.format("%b %d, %Y")
                        ))
                        .clicked()
                    {
                        *selected = date;
                        chosen = true;
                    }
                } else {
                    ui.allocate_space(Vec2::new(34.0, 30.0));
                }
                if index % 7 == 6 {
                    ui.end_row();
                }
            }
        });
    ui.label(
        RichText::new("Teal dates have recorded check-ins.")
            .small()
            .color(muted(ui)),
    );
    chosen
}

fn card<R>(ui: &mut Ui, contents: impl FnOnce(&mut Ui) -> R) -> R {
    Frame::new()
        .fill(ui.visuals().window_fill)
        .stroke(Stroke::new(
            1.0,
            ui.visuals().widgets.noninteractive.bg_stroke.color,
        ))
        .corner_radius(12)
        .inner_margin(18)
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            contents(ui)
        })
        .inner
}

fn primary(ui: &mut Ui, label: &str) -> bool {
    ui.add(
        egui::Button::new(RichText::new(label).color(Color32::WHITE).strong())
            .fill(TEAL)
            .stroke(Stroke::NONE)
            .corner_radius(7),
    )
    .clicked()
}

fn field(ui: &mut Ui, label: &str, value: &mut String, hint: &str) {
    ui.label(RichText::new(label).size(13.0));
    ui.add_sized(
        [ui.available_width().min(620.0), 34.0],
        egui::TextEdit::singleline(value).hint_text(hint),
    );
}

fn headers(ui: &mut Ui, labels: &[&str]) {
    for label in labels {
        ui.label(RichText::new(*label).color(muted(ui)).size(10.0).strong());
    }
    ui.end_row();
}

fn badge(ui: &mut Ui, label: &str, positive: bool) {
    Frame::new()
        .fill(if positive {
            Color32::from_rgb(229, 244, 238)
        } else {
            Color32::from_rgb(252, 243, 219)
        })
        .corner_radius(5)
        .inner_margin(egui::Margin::symmetric(7, 4))
        .show(ui, |ui| {
            ui.label(
                RichText::new(label)
                    .color(if positive {
                        TEAL
                    } else {
                        Color32::from_rgb(140, 104, 31)
                    })
                    .size(11.0),
            );
        });
}

fn empty(ui: &mut Ui, message: &str) {
    ui.add_space(16.0);
    ui.label(RichText::new(message).color(muted(ui)));
    ui.add_space(16.0);
}

fn employee_picker(ui: &mut Ui, id: &str, data: &Snapshot, selected: &mut Option<Uuid>) {
    egui::ComboBox::from_id_salt(id)
        .width(240.0)
        .selected_text(
            selected
                .map(|id| data.employee_name(id))
                .unwrap_or("Choose an employee"),
        )
        .show_ui(ui, |ui| {
            for employee in data.active().filter(|e| data.access.can_record(e.id)) {
                ui.selectable_value(
                    selected,
                    Some(employee.id),
                    format!("{} · {}", employee.full_name, employee.employee_code),
                );
            }
        });
}

#[cfg(test)]
mod identity_ui_tests {
    use super::*;

    #[test]
    fn sign_in_form_stays_near_the_top_and_fits_the_minimum_and_enlarged_viewports() {
        for size in [Vec2::new(1000.0, 720.0), Vec2::new(800.0, 576.0)] {
            let ctx = egui::Context::default();
            configure_theme(&ctx);
            let mut email = String::new();
            let mut password = String::new();
            for _ in 0..2 {
                let mut output = ctx.run_ui(
                    egui::RawInput {
                        screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, size)),
                        ..Default::default()
                    },
                    |ui| {
                        egui::CentralPanel::default()
                            .frame(Frame::new().inner_margin(28))
                            .show(ui, |ui| {
                                let output = egui::ScrollArea::vertical().show(ui, |ui| {
                                    sign_in_layout(
                                        ui,
                                        None,
                                        "Synthetic Company With A Saved Sign-in Logo",
                                        |ui| {
                                            ui.heading("Sign in");
                                            ui.label(
                                                "Use your HRIS account to access company records.",
                                            );
                                            login_controls(ui, &mut email, &mut password, true);
                                        },
                                    )
                                });
                                assert!(
                                    output.inner.top() < size.y * 0.4,
                                    "Form moved to the bottom: {:?}",
                                    output.inner
                                );
                                assert!(
                                    output.inner.bottom() < size.y - 28.0,
                                    "Form exceeds viewport: {:?}",
                                    output.inner
                                );
                                assert!(
                                    output.content_size.y <= output.inner_rect.height() + 1.0,
                                    "Unexpected sign-in scrollbar: {:?}",
                                    output.content_size
                                );
                            });
                    },
                );
                output.textures_delta.clear();
            }
        }
    }

    #[test]
    fn calendar_month_cells_handle_leap_days_and_monday_alignment() {
        for (year, days) in [(2024, 29), (2100, 28), (2000, 29)] {
            let month = NaiveDate::from_ymd_opt(year, 2, 1).unwrap();
            let cells = calendar_cells(month);
            assert_eq!(cells.iter().flatten().count(), days);
            assert_eq!(
                cells.iter().position(|d| d == &Some(month)).unwrap(),
                month.weekday().num_days_from_monday() as usize
            );
            assert!(
                cells
                    .iter()
                    .flatten()
                    .all(|day| day.month() == 2 && day.year() == year)
            );
        }
    }

    #[test]
    fn calendar_day_click_changes_the_date_and_future_days_remain_disabled() {
        fn text_position(shape: &egui::epaint::Shape, text: &str) -> Option<egui::Pos2> {
            match shape {
                egui::epaint::Shape::Text(value) if value.galley.job.text == text => {
                    Some(value.pos + value.galley.size() * 0.5)
                }
                egui::epaint::Shape::Vec(shapes) => {
                    shapes.iter().find_map(|s| text_position(s, text))
                }
                _ => None,
            }
        }
        for (target, expected) in [("14", 14), ("22", 21)] {
            let ctx = egui::Context::default();
            configure_theme(&ctx);
            let today = NaiveDate::from_ymd_opt(2026, 10, 21).unwrap();
            let mut selected = today;
            let mut month = month_start(today);
            let data = DemoStore::new().data;
            let input = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    Vec2::new(500.0, 600.0),
                )),
                ..Default::default()
            };
            let mut position = None;
            for _ in 0..2 {
                let mut output = ctx.run_ui(input.clone(), |ui| {
                    attendance_calendar(ui, &mut selected, &mut month, today, &data);
                });
                position = output
                    .shapes
                    .iter()
                    .find_map(|shape| text_position(&shape.shape, target));
                output.textures_delta.clear();
            }
            let position = position.expect("Day must be rendered");
            for pressed in [true, false] {
                let mut pointer = input.clone();
                pointer.events = vec![
                    egui::Event::PointerMoved(position),
                    egui::Event::PointerButton {
                        pos: position,
                        button: egui::PointerButton::Primary,
                        pressed,
                        modifiers: egui::Modifiers::NONE,
                    },
                ];
                let mut output = ctx.run_ui(pointer, |ui| {
                    attendance_calendar(ui, &mut selected, &mut month, today, &data);
                });
                output.textures_delta.clear();
            }
            assert_eq!(selected.day(), expected);
        }
    }

    #[test]
    fn own_contact_drafts_survive_refresh_and_accept_the_saved_canonical_record() {
        let mut previous = DemoStore::new().data;
        let own = previous.access.employee_id.unwrap();
        let mut edited = EmployeeContacts::empty(previous.company.id, own);
        edited.phone = "000 000 0000".into();
        let mut draft = Some(edited.clone());
        let mut next = previous.clone();
        next.company.name = "Updated fictional company".into();
        sync_profile_contacts(&previous, &next, &mut draft);
        assert_eq!(draft.as_ref().unwrap().phone, edited.phone);
        edited.updated_at = Some(chrono::Utc::now());
        next.contacts.push(edited.clone());
        sync_profile_contacts(&previous, &next, &mut draft);
        assert_eq!(draft, Some(edited));
        previous = next;
        sync_profile_contacts(&previous, &Snapshot::signed_out(), &mut draft);
        assert!(draft.is_none());
    }
}

#[cfg(test)]
mod employment_history_layout_tests {
    use super::*;
    #[test]
    fn history_fields_and_values_align_in_snapshot_and_change_rows() {
        for scale in [1.0, 1.25] {
            for updating in [false, true] {
                let mut store = DemoStore::new();
                let person = store.data.employees[1].clone();
                let event = store
                    .data
                    .employment_history
                    .iter_mut()
                    .find(|event| event.employee_id == person.id)
                    .unwrap();
                if updating {
                    event.kind = crate::history::HistoryKind::Updated;
                    event.before_record = Some(event.after_record.clone());
                }
                event.after_record.employee.employee_code = "HISTORY-ALIGNMENT".into();
                event.after_record.employee.department = "Regional client services and administrative operations for the northern district".into();
                let changes = event.changes();
                let ctx = egui::Context::default();
                ctx.set_zoom_factor(scale);
                let mut output = None;
                for _ in 0..3 {
                    output = Some(ctx.run_ui(
                        egui::RawInput {
                            screen_rect: Some(egui::Rect::from_min_size(
                                egui::Pos2::ZERO,
                                egui::vec2(520.0, 1800.0),
                            )),
                            ..Default::default()
                        },
                        |ui| employment_history(ui, &store.data, person.id, "%H:%M"),
                    ));
                    output.as_mut().unwrap().textures_delta.clear();
                }
                let output = output.unwrap();
                let tops = |needle: &str| {
                    let positions: Vec<_> = output
                        .shapes
                        .iter()
                        .filter_map(|shape| match &shape.shape {
                            egui::epaint::Shape::Text(text) if text.galley.job.text == needle => {
                                Some(text.pos.y + text.galley.rect.top())
                            }
                            _ => None,
                        })
                        .collect();
                    assert!(!positions.is_empty(), "Missing history text: {needle}");
                    positions
                };
                let aligned =
                    |needle: &str, y: f32| tops(needle).iter().any(|top| (y - top).abs() <= 1.0);
                let header = tops("Field")[0];
                assert!(aligned(
                    if updating { "After" } else { "Saved value" },
                    header
                ));
                if updating {
                    assert!(aligned("Before", header));
                }
                for change in changes {
                    let label_top = tops(change.label)[0];
                    assert!(
                        aligned(&change.after, label_top),
                        "{} is misaligned at scale {scale}, updating={updating}",
                        change.label
                    );
                    if let Some(before) = change.before {
                        assert!(aligned(&before, label_top));
                    }
                }
            }
        }
    }
    #[test]
    fn long_employment_history_values_reserve_height_and_do_not_overlap_the_next_row() {
        let mut store = DemoStore::new();
        let person = store.data.employees[1].clone();
        let department =
            "Regional client services and administrative operations for the northern district";
        let title =
            "Senior specialist for regional client service coordination and workforce planning";
        let mut draft = EmployeeDraft::from(&person);
        draft.department = department.into();
        draft.job_title = title.into();
        draft.work_location_id = Some(Uuid::from_u128(2000));
        store.execute(Action::SaveEmployee(draft)).unwrap();
        let ctx = egui::Context::default();
        let mut output = None;
        for _ in 0..3 {
            output = Some(ctx.run_ui(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(520.0, 1400.0),
                    )),
                    ..Default::default()
                },
                |ui| employment_history(ui, &store.data, person.id, "%H:%M"),
            ));
            output.as_mut().unwrap().textures_delta.clear();
        }
        let output = output.unwrap();
        let bounds = |needle: &str| {
            output
                .shapes
                .iter()
                .find_map(|shape| match &shape.shape {
                    egui::epaint::Shape::Text(text) if text.galley.job.text == needle => {
                        Some(text.galley.rect.translate(text.pos.to_vec2()))
                    }
                    _ => None,
                })
                .unwrap_or_else(|| panic!("Missing rendered history value: {needle}"))
        };
        let department_rect = bounds(department);
        let title_rect = bounds(title);
        let location_rect = bounds("Main office");
        assert!(
            department_rect.height() > 20.0 && title_rect.height() > 20.0,
            "Long values must wrap"
        );
        assert!(
            department_rect.bottom() <= title_rect.top(),
            "Department text overlaps job-title row"
        );
        assert!(
            title_rect.bottom() <= location_rect.top(),
            "Job-title text overlaps work-location row"
        );
        assert!(
            department_rect.right() <= 520.0 && title_rect.right() <= 520.0,
            "History extends outside the viewport"
        );
    }
}
