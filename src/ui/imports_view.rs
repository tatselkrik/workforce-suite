use super::*;
use crate::imports::{self, Preview};
use crate::reports;
#[derive(Default)]
pub(super) struct State {
    pub open: bool,
    pub preview: Option<Preview>,
    pub reason: String,
    pub page: usize,
    pub applying: bool,
    pub batch: Option<Uuid>,
}
impl State {
    pub fn editing(&self) -> bool {
        self.preview.is_some()
    }
}
impl HrisApp {
    pub(super) fn employee_import(&mut self, ui: &mut Ui) {
        if !self.data.access.role.manages_hr() {
            empty(
                ui,
                "Only HR and Administrators can import employee records.",
            );
            return;
        }
        card(ui, |ui| {
            ui.heading("Employee CSV import");
            ui.label("Create new employee and contact records from UTF-8 CSV. Previewing makes no changes. Existing employee codes are preserved; use the employee editor for updates.");
            ui.add_space(10.0);
            ui.horizontal_wrapped(|ui| {
                if ui.button("Save CSV template").clicked() {
                    match reports::choose_export("hris-employee-import-template.csv") {
                        Ok(Some(path)) => match imports::template()
                            .and_then(|bytes| reports::save_csv(&path, &bytes))
                        {
                            Ok(()) => self.notice = Some((
                                "Employee import template saved. Existing files were preserved."
                                    .into(),
                                false,
                            )),
                            Err(e) => self.notice = Some((e.to_string(), true)),
                        },
                        Err(e) => self.notice = Some((e.to_string(), true)),
                        _ => {}
                    }
                }
                if primary(
                    ui,
                    if self.import_state.preview.is_some() {
                        "Reopen / choose CSV"
                    } else {
                        "Choose CSV to preview"
                    },
                ) {
                    match imports::choose_source() {
                        Ok(Some(path)) => match Preview::load(&self.data, &path) {
                            Ok(p) => {
                                self.import_state.preview = Some(p);
                                self.import_state.page = 0;
                                self.import_state.batch = None;
                                self.import_state.reason.clear();
                            }
                            Err(e) => self.notice = Some((e.to_string(), true)),
                        },
                        Err(e) => self.notice = Some((e.to_string(), true)),
                        _ => {}
                    }
                }
            });
            ui.label(RichText::new("Maximum 500 rows / 2 MiB. Required headers: employee_code, full_name. Optional columns use the template names; blank annual_leave_days uses the company default. Managers and employment options must already exist and be active.").small().color(muted(ui)));
            ui.label(RichText::new("Imports create active employee records without sign-in accounts. Raw CSV files are not uploaded or retained in the import audit.").small().color(muted(ui)));
        });
        ui.add_space(12.0);
        if let Some(p) = self.import_state.preview.clone() {
            card(ui, |ui| {
                ui.heading(&p.filename);
                ui.horizontal_wrapped(|ui| {
                    badge(ui, &format!("{} ready", p.valid_count()), true);
                    badge(
                        ui,
                        &format!("{} need fixing", p.rows.len() - p.valid_count()),
                        p.valid(),
                    );
                    ui.label(format!("{} source rows", p.rows.len()));
                });
                ui.label("All rows must be valid. An import saves the complete batch in one transaction; a failed row leaves no partial employee records.");
                field(
                    ui,
                    "Import reason *",
                    &mut self.import_state.reason,
                    "e.g. Initial employee onboarding",
                );
                ui.horizontal_wrapped(|ui| {
                    let ready = p.valid()
                        && !self.import_state.reason.trim().is_empty()
                        && self.import_state.reason.chars().count() <= 500
                        && !self.import_state.reason.chars().any(char::is_control);
                    if ui
                        .add_enabled(
                            ready,
                            egui::Button::new(
                                RichText::new(format!("Import all {} employees", p.rows.len()))
                                    .color(Color32::WHITE),
                            )
                            .fill(TEAL),
                        )
                        .clicked()
                    {
                        self.import_state.applying = true;
                        self.send(Command::Change(Action::ImportEmployees(
                            p.clone(),
                            self.import_state.reason.clone(),
                        )));
                    }
                    if ui
                        .add_enabled(!p.valid(), egui::Button::new("Export errors CSV"))
                        .clicked()
                    {
                        match reports::choose_export("hris-employee-import-errors.csv") {
                            Ok(Some(destination)) => self.send(Command::ImportErrors {
                                preview: p.clone(),
                                destination,
                            }),
                            Err(e) => self.notice = Some((e.to_string(), true)),
                            _ => {}
                        }
                    }
                    if ui.button("Discard preview").clicked() {
                        self.import_state.preview = None;
                        self.import_state.reason.clear();
                    }
                });
                if !p.valid() {
                    ui.colored_label(ui.visuals().error_fg_color,"Fix the source file, then reopen it to validate again. No employees have been imported.");
                }
            });
            ui.add_space(10.0);
            const PAGE: usize = 10;
            let pages = p.rows.len().div_ceil(PAGE).max(1);
            self.import_state.page = self.import_state.page.min(pages - 1);
            ui.horizontal_wrapped(|ui| {
                if ui
                    .add_enabled(self.import_state.page > 0, egui::Button::new("Previous"))
                    .clicked()
                {
                    self.import_state.page -= 1;
                }
                ui.label(format!(
                    "Preview page {} of {}",
                    self.import_state.page + 1,
                    pages
                ));
                if ui
                    .add_enabled(
                        self.import_state.page + 1 < pages,
                        egui::Button::new("Next"),
                    )
                    .clicked()
                {
                    self.import_state.page += 1;
                }
            });
            for row in p.rows.iter().skip(self.import_state.page * PAGE).take(PAGE) {
                ui.push_id(("import-row", row.source_row), |ui| {
                    card(ui, |ui| {
                        ui.horizontal_wrapped(|ui| {
                            badge(ui, &format!("Line {}", row.source_row), true);
                            badge(
                                ui,
                                if row.errors.is_empty() {
                                    "Ready"
                                } else {
                                    "Needs fixing"
                                },
                                row.errors.is_empty(),
                            );
                            ui.label(
                                RichText::new(format!("{} · {}", row.employee_code, row.full_name))
                                    .strong(),
                            );
                        });
                        for error in &row.errors {
                            ui.colored_label(ui.visuals().error_fg_color, error);
                        }
                        let details = egui::CollapsingHeader::new(
                            "Employment, contact and emergency details",
                        );
                        #[cfg(feature = "qa-screenshot")]
                        let details = details.default_open(
                            std::env::var("HRIS_PREVIEW_FORM").as_deref() == Ok("import-details"),
                        );
                        details.show(ui, |ui| {
                            let c = &row.contacts;
                            let mut fields = Vec::new();
                            if let Some(e) = &row.employee {
                                fields.extend([
                                    ("Department", e.department.clone()),
                                    ("Job title", e.job_title.clone()),
                                    (
                                        "Date joined",
                                        e.joined_on.map(|d| d.to_string()).unwrap_or_default(),
                                    ),
                                    ("Annual leave", format!("{} days", e.annual_leave_days)),
                                    (
                                        "Employment type",
                                        EmploymentKind::EmploymentType
                                            .display(&self.data, e.employment_type_id),
                                    ),
                                    (
                                        "Work location",
                                        EmploymentKind::WorkLocation
                                            .display(&self.data, e.work_location_id),
                                    ),
                                    (
                                        "Reports to",
                                        e.reports_to
                                            .map(|id| self.data.employee_name(id).to_owned())
                                            .unwrap_or_default(),
                                    ),
                                ]);
                            }
                            fields.extend([
                                ("Phone", c.phone.clone()),
                                ("Personal email", c.personal_email.clone()),
                                ("Address", c.address.clone()),
                                ("Emergency name", c.emergency_name.clone()),
                                ("Relationship", c.emergency_relationship.clone()),
                                ("Emergency phone", c.emergency_phone.clone()),
                                ("Alternate phone", c.emergency_alternate_phone.clone()),
                            ]);
                            egui::Grid::new("import-details")
                                .num_columns(2)
                                .spacing(Vec2::new(18.0, 9.0))
                                .show(ui, |ui| {
                                    for (label, value) in fields {
                                        ui.label(RichText::new(label).color(muted(ui)));
                                        ui.label(if value.is_empty() { "Not set" } else { &value });
                                        ui.end_row();
                                    }
                                });
                        });
                    });
                });
                ui.add_space(8.0);
            }
            return;
        }
        ui.heading("Import history");
        let mut batches = self.data.imports.batches.clone();
        batches.sort_by_key(|b| std::cmp::Reverse((b.created_at, b.id)));
        if batches.is_empty() {
            empty(ui, "No employee CSV batches have been imported.");
        }
        for batch in batches.iter().take(30) {
            card(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    badge(ui, &format!("{} employees", batch.row_count), true);
                    ui.label(&batch.actor_name);
                    ui.label(batch.actor_role.label());
                    ui.label(
                        batch
                            .created_at
                            .with_timezone(&self.data.company.tz())
                            .format("%b %d, %Y %H:%M:%S")
                            .to_string(),
                    );
                });
                ui.label(&batch.reason);
                if ui
                    .selectable_label(
                        self.import_state.batch == Some(batch.id),
                        "Imported employee records",
                    )
                    .clicked()
                {
                    self.import_state.batch = if self.import_state.batch == Some(batch.id) {
                        None
                    } else {
                        Some(batch.id)
                    };
                }
                if self.import_state.batch == Some(batch.id) {
                    let mut items: Vec<_> = self
                        .data
                        .imports
                        .items
                        .iter()
                        .filter(|i| i.batch_id == batch.id)
                        .cloned()
                        .collect();
                    items.sort_by_key(|i| i.source_row);
                    for item in items {
                        ui.horizontal_wrapped(|ui| {
                            ui.label(format!(
                                "Line {} · {}",
                                item.source_row,
                                self.data.employee_name(item.employee_id)
                            ));
                            if ui.button("Open employee").clicked() {
                                self.navigate(Page::Employees, ui.ctx());
                                self.open_employee(item.employee_id);
                            }
                        });
                    }
                }
            });
            ui.add_space(8.0);
        }
    }
    #[cfg(feature = "qa-screenshot")]
    pub(super) fn prepare_import_preview(&mut self) {
        let form = std::env::var("HRIS_PREVIEW_FORM").unwrap_or_default();
        if !form.starts_with("import-") || self.data.mode != Mode::Demo {
            return;
        }
        self.page = Page::Reports;
        self.import_state.open = true;
        if form == "import-manager" {
            self.data.access.role = Role::Manager;
            self.import_state.open = false;
            return;
        }
        if form == "import-hr" {
            self.data.access.role = Role::HrGeneralist;
        }
        if form == "import-empty" {
            return;
        }
        let good = "employee_code,full_name,department,job_title,joined_on,annual_leave_days,phone,personal_email,address,emergency_name,emergency_relationship,emergency_phone\nCSV-001,Synthetic Import One,Operations,Associate,2026-01-01,15,000 000 0901,synthetic.import@example.invalid,Fictional address,Synthetic Contact,Sibling,000 000 0902\nCSV-002,Synthetic Import Two,Engineering,Engineer,2026-02-01,18,,,,,,\n";
        let bad = "employee_code,full_name,phone,employment_type,emergency_name\nNS-001,Synthetic Existing,12,Unknown option,Partial\nCSV-002,,bad,,\nCSV-002,Synthetic Duplicate,,,\n";
        let p = Preview::from_csv(
            &self.data,
            "Fictional employees.csv".into(),
            if form == "import-errors" {
                bad.as_bytes()
            } else {
                good.as_bytes()
            },
        )
        .unwrap();
        if form == "import-history" {
            let id = p.id;
            let mut store = DemoStore {
                data: self.data.clone(),
            };
            store
                .execute(Action::ImportEmployees(
                    p,
                    "Fictional onboarding batch with retained audit".into(),
                ))
                .unwrap();
            self.data = store.data;
            self.import_state.batch = Some(id);
        } else {
            self.import_state.preview = Some(p);
            self.import_state.reason =
                "Fictional onboarding from a checked employee register".into();
        }
    }
}
