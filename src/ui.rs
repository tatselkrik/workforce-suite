use crate::{
    account::{self, AccountAction, AccountProfile, DisplayPreferences, PersonalDetails},
    backend::{self, Command, ConnectionConfig, Update},
    branding,
    domain::*,
};
use chrono::{Datelike, Duration};
use eframe::egui::{self, Align, Color32, FontId, Frame, Layout, RichText, Stroke, Ui, Vec2};
use std::sync::mpsc;
use uuid::Uuid;

const NAVY: Color32 = Color32::from_rgb(22, 36, 51);
const INK: Color32 = Color32::from_rgb(29, 45, 60);
const MUTED: Color32 = Color32::from_rgb(103, 119, 134);
const TEAL: Color32 = Color32::from_rgb(0, 123, 112);
const BG: Color32 = Color32::from_rgb(245, 247, 250);
const LINE: Color32 = Color32::from_rgb(226, 232, 238);

#[derive(Clone, Copy, PartialEq, Eq)]
enum Page {
    Overview,
    Employees,
    Attendance,
    Leave,
    Settings,
    Profile,
}

impl Page {
    const ALL: [Self; 5] = [
        Self::Overview,
        Self::Employees,
        Self::Attendance,
        Self::Leave,
        Self::Settings,
    ];
    fn title(self) -> &'static str {
        match self {
            Self::Overview => "Overview",
            Self::Employees => "Employees",
            Self::Attendance => "Attendance",
            Self::Leave => "Leave",
            Self::Settings => "Settings",
            Self::Profile => "Profile",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ProfileSection {
    Personal,
    Security,
    Sessions,
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
    department: String,
    attendance_date: String,
    check_in_employee: Option<Uuid>,
    leave_filter: Option<LeaveStatus>,
    employee_form: Option<EmployeeDraft>,
    leave_form: Option<LeaveDraft>,
    close_on_success: bool,
    company_draft: Company,
    connection: Option<ConnectionConfig>,
    deployment_error: Option<String>,
    login_email: String,
    login_password: String,
    company_logo: Option<egui::TextureHandle>,
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
    #[cfg(feature = "qa-screenshot")]
    qa_frames: u32,
}

pub fn configure_theme(ctx: &egui::Context) {
    ctx.set_theme(egui::Theme::Light);
    ctx.set_visuals(egui::Visuals::light());
    ctx.global_style_mut(|style| {
        style.visuals.override_text_color = Some(INK);
        style.visuals.panel_fill = BG;
        style.visuals.window_fill = Color32::WHITE;
        style.visuals.window_stroke = Stroke::new(1.0, LINE);
        style.visuals.text_edit_bg_color = Some(Color32::from_rgb(242, 245, 248));
        style.visuals.selection.bg_fill = Color32::from_rgb(208, 238, 233);
        style.visuals.selection.stroke = Stroke::new(1.0, TEAL);
        style.visuals.widgets.inactive.bg_fill = Color32::from_rgb(242, 245, 248);
        style.visuals.widgets.inactive.weak_bg_fill = Color32::from_rgb(242, 245, 248);
        style.visuals.widgets.inactive.bg_stroke = Stroke::new(1.0, LINE);
        style.visuals.widgets.hovered.bg_fill = Color32::from_rgb(226, 240, 237);
        style.visuals.widgets.hovered.weak_bg_fill = Color32::from_rgb(226, 240, 237);
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
        let account = (data.mode == Mode::Demo).then(AccountProfile::demo);
        let personal_draft = account
            .as_ref()
            .map(|a| a.details.clone())
            .unwrap_or_default();
        let mut app = Self {
            attendance_date: data.company.today().to_string(),
            company_draft: data.company.clone(),
            data,
            page: Page::Overview,
            commands,
            updates,
            busy: false,
            notice: None,
            search: String::new(),
            employee_status: Some(EmployeeStatus::Active),
            department: String::new(),
            check_in_employee: None,
            leave_filter: None,
            employee_form: None,
            leave_form: None,
            close_on_success: false,
            connection,
            deployment_error,
            login_email: String::new(),
            login_password: String::new(),
            company_logo,
            account,
            avatar: None,
            personal_draft,
            display_draft: DisplayPreferences::default(),
            profile_section: ProfileSection::Personal,
            current_password: String::new(),
            new_password: String::new(),
            confirm_password: String::new(),
            new_email: String::new(),
            email_password: String::new(),
            show_passwords: false,
            session_confirmation: None,
            #[cfg(feature = "qa-screenshot")]
            qa_frames: 0,
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
                Ok("settings") => app.page = Page::Settings,
                Ok("profile") => app.page = Page::Profile,
                _ => {}
            }
            if app.data.mode == Mode::Demo {
                match std::env::var("HRIS_PREVIEW_PROFILE").as_deref() {
                    Ok("security") => app.profile_section = ProfileSection::Security,
                    Ok("sessions") => app.profile_section = ProfileSection::Sessions,
                    _ => {}
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
                Ok("leave") => app.new_leave(),
                _ => {}
            }
        }
        // Keep the same constructor in production and screenshot builds.
        app.check_in_employee = app.data.active().next().map(|e| e.id);
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
            self.busy = false;
            match update.result {
                Ok(data) => {
                    if self.data.mode != data.mode {
                        self.clear_profile_secrets(ctx);
                        self.session_confirmation = None;
                        self.profile_section = ProfileSection::Personal;
                        self.page = Page::Overview;
                        self.search.clear();
                        self.department.clear();
                        self.employee_status = Some(EmployeeStatus::Active);
                        self.leave_filter = None;
                        self.attendance_date = data.company.today().to_string();
                        self.check_in_employee = data.active().next().map(|employee| employee.id);
                        self.employee_form = None;
                        self.leave_form = None;
                    }
                    self.company_draft = data.company.clone();
                    self.data = data;
                    if self.close_on_success {
                        self.employee_form = None;
                        self.leave_form = None;
                    }
                    if !update.message.is_empty() {
                        self.notice = Some((update.message, false));
                    }
                }
                Err(error) => self.notice = Some((error, true)),
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
                self.personal_draft = update
                    .account
                    .as_ref()
                    .map(|a| a.details.clone())
                    .unwrap_or_default();
                self.display_draft = update
                    .account
                    .as_ref()
                    .map(|a| a.display)
                    .unwrap_or_default();
                ctx.set_zoom_factor(self.display_draft.scale_percent as f32 / 100.0);
                self.account = update.account;
            }
            self.close_on_success = false;
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
            ],
        );
    }

    fn navigate(&mut self, page: Page, ctx: &egui::Context) {
        if self.page != page {
            self.clear_profile_secrets(ctx);
            self.session_confirmation = None;
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
        self.employee_form = Some(EmployeeDraft {
            employee_code: format!("EMP-{:03}", self.data.employees.len() + 1),
            joined_on: self.data.company.today().to_string(),
            annual_leave_days: self.data.company.annual_leave_days,
            ..Default::default()
        });
    }

    fn new_leave(&mut self) {
        let today = self.data.company.today();
        let start = (0..7)
            .map(|i| today + Duration::days(i))
            .find(|d| d.weekday().number_from_monday() <= 5)
            .unwrap_or(today);
        self.leave_form = Some(LeaveDraft {
            employee_id: self.data.active().next().map(|e| e.id),
            starts_on: start.to_string(),
            ends_on: start.to_string(),
            ..Default::default()
        });
    }

    fn sidebar(&mut self, ui: &mut Ui) {
        ui.add_space(12.0);
        ui.vertical_centered(|ui| {
            if let Some(logo) = &self.company_logo {
                ui.add(
                    egui::Image::new(logo)
                        .fit_to_exact_size(Vec2::new(112.0, 72.0))
                        .maintain_aspect_ratio(true),
                );
            } else {
                let (rect, _) = ui.allocate_exact_size(Vec2::splat(72.0), egui::Sense::hover());
                ui.painter().circle_stroke(
                    rect.center(),
                    32.0,
                    Stroke::new(1.5, Color32::from_rgb(115, 214, 197)),
                );
                ui.painter().text(
                    rect.center(),
                    egui::Align2::CENTER_CENTER,
                    branding::initials(&self.data.company.name),
                    FontId::proportional(25.0),
                    Color32::from_rgb(222, 242, 237),
                );
            }
            ui.add_space(10.0);
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
        ui.add_space(28.0);
        for page in Page::ALL {
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
            if ui.add_sized([ui.available_width(), 42.0], button).clicked() {
                self.navigate(page, ui.ctx());
            }
        }
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
                .account
                .as_ref()
                .map(|a| a.details.display_name())
                .unwrap_or("Your account");
            let role = if self.data.mode == Mode::Demo {
                "Demo account"
            } else {
                "Administrator"
            };
            if profile_link(
                ui,
                self.avatar.as_ref(),
                name,
                role,
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
                    .color(MUTED)
                    .size(11.0)
                    .strong(),
            );
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if self.busy {
                    ui.spinner();
                    ui.label(RichText::new("Working…").color(MUTED));
                } else if ui.button("Refresh").clicked() {
                    self.send(Command::Refresh);
                }
                ui.label(
                    RichText::new(self.data.company.today().format("%b %d, %Y").to_string())
                        .color(MUTED),
                );
            });
        });
        ui.add_space(14.0);
        let (title, subtitle) = match self.page {
            Page::Overview => (
                "Your people, at a glance",
                "A clear view of your team and today's HR activity.",
            ),
            Page::Employees => (
                "Employee directory",
                "Keep your people and their work details in one place.",
            ),
            Page::Attendance => (
                "Daily attendance",
                "Record check-ins, close shifts, and review daily records.",
            ),
            Page::Leave => (
                "Time away, organized",
                "Review requests and keep annual leave balances in view.",
            ),
            Page::Settings => (
                "Settings",
                "Manage company defaults and your display preferences.",
            ),
            Page::Profile => (
                "Your profile",
                "Manage your personal details, account security and sessions.",
            ),
        };
        ui.heading(RichText::new(title).strong());
        ui.label(RichText::new(subtitle).color(MUTED));
        ui.add_space(12.0);
        if self.data.mode == Mode::Demo {
            Frame::new().fill(Color32::from_rgb(232,243,240)).corner_radius(8).inner_margin(12).show(ui,|ui| {
                ui.set_width(ui.available_width());
                ui.label(RichText::new("Demo workspace · Fictional sample data. Changes reset when the app closes.").color(TEAL).size(12.0));
            });
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

    fn overview(&mut self, ui: &mut Ui) {
        let today = self.data.company.today();
        let active = self.data.active().count();
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
                r.status == LeaveStatus::Approved
                    && r.starts_on <= today
                    && r.ends_on >= today
                    && today.weekday().number_from_monday() <= 5
            })
            .count();
        ui.columns(4, |columns| {
            for (ui, (label, value, caption)) in columns.iter_mut().zip([
                ("ACTIVE EMPLOYEES", active, "People in your workspace"),
                ("CHECKED IN TODAY", attendance, "Daily attendance records"),
                ("AWAITING REVIEW", pending, "Pending leave requests"),
                ("ON LEAVE TODAY", away, "Approved time away"),
            ]) {
                card(ui, |ui| {
                    ui.label(RichText::new(label).color(MUTED).size(10.0).strong());
                    ui.label(RichText::new(value.to_string()).size(34.0).strong());
                    ui.label(RichText::new(caption).color(MUTED).size(11.0));
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
                .color(MUTED)
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
                ui.label(RichText::new("Requests to review").size(18.0).strong());
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
                        .color(MUTED),
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
            if primary(ui, "+ Add employee") {
                self.new_employee();
            }
            if ui.button("+ Request leave").clicked() {
                self.new_leave();
            }
        });
    }

    fn employees(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
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
        ui.horizontal(|ui| {
            ui.label(
                RichText::new(format!("{} employees total", self.data.employees.len()))
                    .color(MUTED),
            );
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if primary(ui, "+ Add employee") {
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
                    && (query.is_empty()
                        || format!(
                            "{} {} {} {}",
                            e.full_name, e.employee_code, e.department, e.job_title
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
                                    "JOINED",
                                    "ANNUAL BALANCE",
                                    "STATUS",
                                    "",
                                ],
                            );
                            for employee in &rows {
                                ui.label(
                                    RichText::new(&employee.employee_code)
                                        .color(MUTED)
                                        .monospace(),
                                );
                                ui.vertical(|ui| {
                                    ui.set_width(180.0);
                                    ui.label(RichText::new(&employee.full_name).strong());
                                    ui.label(
                                        RichText::new(&employee.job_title).color(MUTED).size(12.0),
                                    );
                                });
                                ui.label(&employee.department);
                                ui.label(employee.joined_on.to_string());
                                let used = self
                                    .data
                                    .annual_used(employee.id, self.data.company.today().year());
                                ui.label(format!(
                                    "{} / {} days",
                                    employee.annual_leave_days.saturating_sub(used),
                                    employee.annual_leave_days
                                ));
                                badge(
                                    ui,
                                    employee.status.label(),
                                    employee.status == EmployeeStatus::Active,
                                );
                                if ui.button("Edit").clicked() {
                                    self.employee_form = Some(EmployeeDraft::from(employee));
                                }
                                ui.end_row();
                            }
                        });
                });
            if rows.is_empty() {
                empty(ui, "No employees match these filters.");
            }
        });
        ui.label(RichText::new("Balances show the current calendar year. Archive employees from Edit to preserve their history.").color(MUTED).size(12.0));
    }

    fn attendance(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            if ui.button("<").clicked() {
                if let Ok(date) = parse_date(&self.attendance_date) {
                    self.attendance_date = (date - Duration::days(1)).to_string();
                }
            }
            ui.add_sized(
                [125.0, 34.0],
                egui::TextEdit::singleline(&mut self.attendance_date),
            );
            if ui.button(">").clicked() {
                if let Ok(date) = parse_date(&self.attendance_date) {
                    self.attendance_date = (date + Duration::days(1)).to_string();
                }
            }
            if ui.button("Today").clicked() {
                self.attendance_date = self.data.company.today().to_string();
            }
            ui.label(RichText::new(&self.data.company.timezone).color(MUTED));
        });
        let date = match parse_date(&self.attendance_date) {
            Ok(date) => date,
            Err(error) => {
                ui.colored_label(Color32::DARK_RED, error.to_string());
                return;
            }
        };
        card(ui, |ui| {
            ui.label(RichText::new("Record a check-in").strong());
            ui.horizontal(|ui| {
                employee_picker(
                    ui,
                    "checkin_employee",
                    &self.data,
                    &mut self.check_in_employee,
                );
                ui.add_enabled_ui(
                    date == self.data.company.today() && self.check_in_employee.is_some(),
                    |ui| {
                        if primary(ui, "Check in") {
                            if let Some(id) = self.check_in_employee {
                                self.send(Command::Change(Action::CheckIn(id)));
                            }
                        }
                    },
                );
            });
            ui.label(RichText::new("Check-ins use today's company date. Live timestamps come from the database server.").color(MUTED).size(12.0));
        });
        ui.add_space(8.0);
        let rows: Vec<_> = self
            .data
            .attendance
            .iter()
            .filter(|a| a.work_date == date)
            .cloned()
            .collect();
        card(ui, |ui| {
            ui.label(
                RichText::new(format!(
                    "{} attendance record(s) · {}",
                    rows.len(),
                    date.format("%A, %B %d")
                ))
                .size(17.0)
                .strong(),
            );
            ui.add_space(8.0);
            egui::ScrollArea::horizontal().show(ui, |ui| {
                egui::Grid::new("attendance_table")
                    .striped(true)
                    .spacing([32.0, 16.0])
                    .show(ui, |ui| {
                        headers(
                            ui,
                            &["EMPLOYEE", "CHECK-IN", "CHECK-OUT", "ELAPSED", "STATUS", ""],
                        );
                        for row in &rows {
                            ui.label(
                                RichText::new(self.data.employee_name(row.employee_id)).strong(),
                            );
                            ui.label(
                                row.checked_in_at
                                    .with_timezone(&self.data.company.tz())
                                    .format(self.time_format())
                                    .to_string(),
                            );
                            ui.label(
                                row.checked_out_at
                                    .map(|t| {
                                        t.with_timezone(&self.data.company.tz())
                                            .format(self.time_format())
                                            .to_string()
                                    })
                                    .unwrap_or_else(|| "—".into()),
                            );
                            ui.label(
                                row.checked_out_at
                                    .map(|t| {
                                        format!(
                                            "{:.1} h",
                                            (t - row.checked_in_at).num_minutes() as f64 / 60.0
                                        )
                                    })
                                    .unwrap_or_else(|| "In progress".into()),
                            );
                            badge(
                                ui,
                                if row.checked_out_at.is_some() {
                                    "Completed"
                                } else {
                                    "Checked in"
                                },
                                true,
                            );
                            if row.checked_out_at.is_none() {
                                if ui.button("Check out").clicked() {
                                    self.send(Command::Change(Action::CheckOut(row.id)));
                                }
                            } else {
                                ui.label("—");
                            }
                            ui.end_row();
                        }
                    });
            });
            if rows.is_empty() {
                empty(ui, "No attendance records for this date.");
            }
        });
        ui.label(RichText::new("Elapsed time includes breaks; it is not a payroll calculation. Open overnight records can be closed from their original date.").color(MUTED).size(12.0));
    }

    fn leave(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            ui.selectable_value(&mut self.leave_filter, None, "All requests");
            for status in [
                LeaveStatus::Pending,
                LeaveStatus::Approved,
                LeaveStatus::Rejected,
            ] {
                ui.selectable_value(&mut self.leave_filter, Some(status), status.label());
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if primary(ui, "+ Request leave") {
                    self.new_leave();
                }
            });
        });
        let rows: Vec<_> = self
            .data
            .leave
            .iter()
            .filter(|r| self.leave_filter.is_none_or(|s| s == r.status))
            .cloned()
            .collect();
        card(ui, |ui| {
            egui::ScrollArea::horizontal().show(ui, |ui| {
                egui::Grid::new("leave_grid")
                    .striped(true)
                    .spacing([18.0, 16.0])
                    .show(ui, |ui| {
                        headers(
                            ui,
                            &["EMPLOYEE", "TYPE", "DATES", "DAYS", "STATUS", "REVIEW"],
                        );
                        for row in &rows {
                            ui.label(
                                RichText::new(self.data.employee_name(row.employee_id)).strong(),
                            )
                            .on_hover_text(&row.reason);
                            ui.label(row.kind.label());
                            ui.vertical(|ui| {
                                ui.set_width(125.0);
                                ui.label(row.starts_on.to_string());
                                ui.label(
                                    RichText::new(format!("to {}", row.ends_on))
                                        .color(MUTED)
                                        .size(12.0),
                                );
                            });
                            ui.label(weekdays(row.starts_on, row.ends_on).to_string());
                            badge(ui, row.status.label(), row.status == LeaveStatus::Approved);
                            ui.horizontal(|ui| {
                                if row.status == LeaveStatus::Pending {
                                    if ui.button("Approve").clicked() {
                                        self.send(Command::Change(Action::DecideLeave(
                                            row.id,
                                            LeaveStatus::Approved,
                                        )));
                                    }
                                    if ui.button("Reject").clicked() {
                                        self.send(Command::Change(Action::DecideLeave(
                                            row.id,
                                            LeaveStatus::Rejected,
                                        )));
                                    }
                                } else {
                                    ui.label(RichText::new("Reviewed").color(MUTED));
                                }
                            });
                            ui.end_row();
                        }
                    });
            });
            if rows.is_empty() {
                empty(ui, "No leave requests in this view.");
            }
        });
        ui.label(RichText::new("Prototype policy: Monday–Friday workdays, no holiday calendar or carryover. Annual leave is deducted only after approval.").color(MUTED).size(12.0));
    }

    fn settings(&mut self, ui: &mut Ui) {
        card(ui, |ui| {
            ui.label(RichText::new("Company defaults").size(18.0).strong());
            ui.set_max_width(630.0);
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
            ui.horizontal(|ui| {
                ui.label("Annual leave for new employees");
                ui.add(
                    egui::DragValue::new(&mut self.company_draft.annual_leave_days)
                        .range(0..=366)
                        .suffix(" days"),
                );
            });
            ui.label(RichText::new("The default allowance applies to new employees. Existing employees retain their individual allowance.").color(MUTED).size(12.0));
            if primary(ui, "Save company settings") {
                match self.company_draft.validate() {
                    Ok(()) => self.send(Command::Change(Action::SaveCompany(
                        self.company_draft.clone(),
                    ))),
                    Err(error) => self.notice = Some((error.to_string(), true)),
                }
            }
        });
        ui.add_space(12.0);
        card(ui, |ui| {
            ui.label(RichText::new("Display preferences").size(18.0).strong());
            ui.label(
                RichText::new("These preferences apply to your account.")
                    .color(MUTED)
                    .size(12.0),
            );
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
                .color(MUTED)
                .size(12.0),
            );
            if primary(ui, "Save display preferences") {
                self.send(Command::Account(AccountAction::SaveDisplay(
                    self.display_draft,
                )));
            }
        });
    }

    fn profile(&mut self, ui: &mut Ui) {
        let Some(profile) = self.account.clone() else {
            empty(ui, "Refresh to load your account.");
            return;
        };
        let signed_in = self.data.mode == Mode::Supabase;
        card(ui, |ui| {
            ui.horizontal(|ui| {
                avatar(ui,self.avatar.as_ref(),profile.details.display_name(),86.0);
                ui.vertical(|ui| {
                    ui.label(RichText::new(profile.details.display_name()).size(22.0).strong());
                    ui.label(RichText::new(if signed_in { "Administrator" } else { "Demo account" }).color(MUTED));
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
                    ui.label(RichText::new("PNG or JPEG, up to 5 MiB. Photos are centered and cropped to a circle.").color(MUTED).size(11.0));
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
                ProfileSection::Personal,
                "Personal details",
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
            ProfileSection::Personal => {
                card(ui, |ui| {
                    ui.label(RichText::new("Personal details").size(18.0).strong());
                    profile_field(
                        ui,
                        "Full name",
                        "profile-full-name",
                        &mut self.personal_draft.full_name,
                        "Your full name",
                    );
                    profile_field(
                        ui,
                        "Preferred name (optional)",
                        "profile-preferred-name",
                        &mut self.personal_draft.preferred_name,
                        "Name shown in the app",
                    );
                    profile_field(
                        ui,
                        "Contact phone (optional)",
                        "profile-contact-phone",
                        &mut self.personal_draft.contact_phone,
                        "Your contact number",
                    );
                    ui.horizontal(|ui| {
                        if primary(ui, "Save personal details") {
                            match self.personal_draft.validate() {
                                Ok(()) => self.send(Command::Account(AccountAction::SaveDetails(
                                    self.personal_draft.trimmed(),
                                ))),
                                Err(error) => self.notice = Some((error.to_string(), true)),
                            }
                        }
                        if ui.button("Revert changes").clicked() {
                            self.personal_draft = profile.details.clone();
                        }
                    });
                });
                ui.add_space(12.0);
                card(ui, |ui| {
                    ui.label(RichText::new("Account information").size(18.0).strong());
                    ui.label(&profile.email);
                    if signed_in {
                        badge(
                            ui,
                            if profile.email_confirmed_at.is_some() {
                                "Email verified"
                            } else {
                                "Email not verified"
                            },
                            profile.email_confirmed_at.is_some(),
                        );
                    }
                    ui.label(
                        RichText::new("Access: Administrator · Managed by your company")
                            .color(MUTED)
                            .size(12.0),
                    );
                    if let Some(created) = &profile.created_at {
                        ui.label(
                            RichText::new(format!("Account created {}", account_date(created)))
                                .color(MUTED)
                                .size(12.0),
                        );
                    }
                    if ui.button("Manage account security").clicked() {
                        self.profile_section = ProfileSection::Security;
                    }
                });
            }
            ProfileSection::Security => {
                if !signed_in {
                    ui.label(RichText::new("Sign in to an HRIS account to change account security. This demo never changes a real account.").color(MUTED));
                }
                ui.add_enabled_ui(signed_in, |ui| {
                    card(ui, |ui| {
                        ui.label(RichText::new("Change password").size(18.0).strong());
                        ui.label(RichText::new("Use 12–128 characters. Your current password is required.").color(MUTED).size(12.0));
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
                            ui.label(RichText::new(format!("Awaiting confirmation: {}",profile.pending_email)).color(TEAL));
                        }
                        profile_field(ui,"New email address","profile-new-email",&mut self.new_email,"Your new account email");
                        password_field(ui,"Current password","profile-email-password",&mut self.email_password,self.show_passwords);
                        ui.label(RichText::new("Confirm the links sent to your email addresses, then Refresh. Your current email remains active until confirmation.").color(MUTED).size(12.0));
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
                            .color(MUTED),
                        );
                    }
                    ui.label(
                        RichText::new(
                            "This app does not save your password or session when it closes.",
                        )
                        .color(MUTED)
                        .size(12.0),
                    );
                    ui.add_space(10.0);
                    ui.add_enabled_ui(signed_in, |ui| {
                        if ui.button("Sign out other devices").clicked() { self.session_confirmation = Some(SessionChoice::Others); }
                        ui.label(RichText::new("Keep this device signed in while revoking the other sessions.").color(MUTED).size(12.0));
                        ui.add_space(8.0);
                        if ui.button("Sign out everywhere").clicked() { self.session_confirmation = Some(SessionChoice::Everywhere); }
                        ui.label(RichText::new("Includes this device. Other devices may retain access until their current access token expires.").color(MUTED).size(12.0));
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
                    ui.label(RichText::new("Other devices may keep access until their current access token expires.").color(MUTED).size(12.0));
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
        ui.vertical_centered(|ui| {
            ui.add_space(70.0);
            ui.heading(RichText::new("HRIS Desktop").strong());
            ui.label(RichText::new("Your company workspace").color(MUTED));
            ui.add_space(24.0);
            Frame::new().fill(Color32::WHITE).stroke(Stroke::new(1.0, LINE))
                .corner_radius(12).inner_margin(28).show(ui, |ui| {
                    ui.set_width(420.0);
                    ui.with_layout(Layout::top_down(Align::Min), |ui| {
                        if let Some(error) = &self.deployment_error {
                            ui.heading("Company connection unavailable");
                            ui.label(error);
                            ui.label(RichText::new("Your HRIS administrator configures this installation. Reopen the app after they have corrected its connection.").color(MUTED));
                            return;
                        }
                        ui.heading("Sign in");
                        ui.label(RichText::new("Use your HRIS account to access company records.").color(MUTED));
                        if let Some((message, error)) = &self.notice {
                            ui.colored_label(if *error { Color32::from_rgb(160, 52, 38) } else { TEAL }, message);
                        }
                        ui.add_space(8.0);
                        if login_controls(ui, &mut self.login_email, &mut self.login_password, !self.busy) {
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
                            ui.horizontal(|ui| { ui.spinner(); ui.label("Signing in…"); });
                        }
                    });
                });
        });
    }

    fn forms(&mut self, ctx: &egui::Context) {
        if let Some(mut draft) = self.employee_form.take() {
            let mut open = true;
            let mut save = false;
            let mut cancel = false;
            egui::Window::new(if draft.id.is_some() {
                "Edit employee"
            } else {
                "Add employee"
            })
            .id(egui::Id::unique("employee_form"))
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .default_width(440.0)
            .show(ctx, |ui| {
                ui.add_enabled_ui(!self.busy, |ui| {
                    field(ui, "Employee code", &mut draft.employee_code, "EMP-001");
                    field(ui, "Full name", &mut draft.full_name, "Employee name");
                    field(ui, "Department", &mut draft.department, "Operations");
                    field(ui, "Job title", &mut draft.job_title, "Team member");
                    field(ui, "Date joined", &mut draft.joined_on, "YYYY-MM-DD");
                    ui.horizontal(|ui| {
                        ui.label("Annual leave allowance");
                        ui.add(
                            egui::DragValue::new(&mut draft.annual_leave_days)
                                .range(0..=366)
                                .suffix(" days"),
                        );
                    });
                    egui::ComboBox::from_id_salt("edit_employee_status")
                        .selected_text(draft.status.label())
                        .show_ui(ui, |ui| {
                            ui.selectable_value(
                                &mut draft.status,
                                EmployeeStatus::Active,
                                "Active",
                            );
                            ui.selectable_value(
                                &mut draft.status,
                                EmployeeStatus::Archived,
                                "Archived",
                            );
                        });
                    ui.add_space(6.0);
                    ui.horizontal(|ui| {
                        save = primary(ui, "Save employee");
                        cancel = ui.button("Cancel").clicked();
                    });
                    if let Some((text, true)) = &self.notice {
                        ui.colored_label(Color32::DARK_RED, text);
                    }
                });
            });
            if save {
                match draft.employee(self.data.company.id) {
                    Ok(_) => {
                        self.close_on_success = true;
                        self.send(Command::Change(Action::SaveEmployee(draft.clone())));
                    }
                    Err(error) => self.notice = Some((error.to_string(), true)),
                }
            }
            if (open && !cancel) || self.busy {
                self.employee_form = Some(draft);
            }
        }
        if let Some(mut draft) = self.leave_form.take() {
            let mut open = true;
            let mut save = false;
            let mut cancel = false;
            egui::Window::new("Request leave").open(&mut open).collapsible(false).resizable(false).anchor(egui::Align2::CENTER_CENTER,[0.0,0.0]).default_width(440.0)
                .show(ctx,|ui| {
                    ui.add_enabled_ui(!self.busy,|ui| {
                        ui.label("Employee");employee_picker(ui,"leave_employee",&self.data,&mut draft.employee_id);
                        ui.label("Leave type");egui::ComboBox::from_id_salt("leave_kind").selected_text(draft.kind.label()).show_ui(ui,|ui|{for kind in LeaveKind::ALL{ui.selectable_value(&mut draft.kind,kind,kind.label());}});
                        field(ui,"Start date",&mut draft.starts_on,"YYYY-MM-DD");field(ui,"End date",&mut draft.ends_on,"YYYY-MM-DD");
                        if let (Ok(start),Ok(end))=(parse_date(&draft.starts_on),parse_date(&draft.ends_on)) {
                            ui.label(RichText::new(format!("{} Monday–Friday workday(s)",weekdays(start,end))).color(TEAL));
                            if let Some(employee)=self.data.employees.iter().find(|e|Some(e.id)==draft.employee_id){let used=self.data.annual_used(employee.id,start.year());ui.label(RichText::new(format!("Annual balance for {}: {} / {} days",start.year(),employee.annual_leave_days.saturating_sub(used),employee.annual_leave_days)).color(MUTED).size(12.0));}
                        }
                        ui.label("Note (optional)");ui.add_sized([ui.available_width(),70.0],egui::TextEdit::multiline(&mut draft.reason).hint_text("Add only information needed to review this request."));
                        ui.label(RichText::new("Exclude sensitive medical details. Requests cannot cross calendar years in this prototype.").color(MUTED).size(11.0));
                        ui.horizontal(|ui|{save=primary(ui,"Submit request");cancel=ui.button("Cancel").clicked();});
                        if let Some((text,true))=&self.notice {ui.colored_label(Color32::DARK_RED,text);}
                    });
                });
            if save {
                match draft.request(self.data.company.id) {
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

impl eframe::App for HrisApp {
    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.receive(&ctx);
        if self.data.mode == Mode::SignedOut {
            egui::CentralPanel::default()
                .frame(Frame::new().fill(BG).inner_margin(28))
                .show(ui, |ui| {
                    egui::ScrollArea::vertical().show(ui, |ui| self.sign_in(ui));
                });
        } else {
            let form_open = self.employee_form.is_some()
                || self.leave_form.is_some()
                || self.session_confirmation.is_some();
            egui::Panel::left("sidebar")
                .resizable(false)
                .exact_size(220.0)
                .frame(Frame::new().fill(NAVY).inner_margin(20))
                .show(ui, |ui| {
                    ui.add_enabled_ui(!self.busy && !form_open, |ui| self.sidebar(ui));
                });
            egui::CentralPanel::default()
                .frame(Frame::new().fill(BG).inner_margin(28))
                .show(ui, |ui| {
                    let scroll =
                        egui::ScrollArea::vertical().id_salt(("main_scroll", self.page as u8));
                    #[cfg(feature = "qa-screenshot")]
                    let scroll = if std::env::var("HRIS_PREVIEW_SCROLL").as_deref() == Ok("bottom")
                    {
                        scroll.stick_to_bottom(true)
                    } else {
                        scroll
                    };
                    scroll.show(ui, |ui| {
                        ui.add_enabled_ui(!form_open, |ui| {
                            self.header(ui);
                            ui.add_enabled_ui(!self.busy, |ui| match self.page {
                                Page::Overview => self.overview(ui),
                                Page::Employees => self.employees(ui),
                                Page::Attendance => self.attendance(ui),
                                Page::Leave => self.leave(ui),
                                Page::Settings => self.settings(ui),
                                Page::Profile => self.profile(ui),
                            });
                        });
                    });
                });
            self.forms(&ctx);
            self.confirm_session_action(&ctx);
        }
        #[cfg(feature = "qa-screenshot")]
        {
            self.qa_frames += 1;
            ctx.send_viewport_cmd(egui::ViewportCommand::RequestPaintWhileHidden);
            if self.qa_frames == 4 {
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
            if self.qa_frames <= 5 {
                ctx.request_repaint();
            }
        }
        ctx.request_repaint_after(std::time::Duration::from_secs(20));
    }
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
            ui.label(
                RichText::new(role)
                    .color(Color32::from_rgb(175, 206, 208))
                    .size(11.0),
            );
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

fn account_date(value: &str) -> String {
    chrono::DateTime::parse_from_rfc3339(value)
        .map(|date| date.format("%b %d, %Y").to_string())
        .unwrap_or_else(|_| "Unavailable".into())
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

fn card<R>(ui: &mut Ui, contents: impl FnOnce(&mut Ui) -> R) -> R {
    Frame::new()
        .fill(Color32::WHITE)
        .stroke(Stroke::new(1.0, LINE))
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
        ui.label(RichText::new(*label).color(MUTED).size(10.0).strong());
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
    ui.label(RichText::new(message).color(MUTED));
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
            for employee in data.active() {
                ui.selectable_value(
                    selected,
                    Some(employee.id),
                    format!("{} · {}", employee.full_name, employee.employee_code),
                );
            }
        });
}
