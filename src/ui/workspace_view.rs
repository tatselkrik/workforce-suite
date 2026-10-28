use super::*;
impl HrisApp {
    pub(super) fn current_workspace_hidden(&self) -> bool {
        self.employee_form.is_none()
            && workspaces::hidden(self.page, self.employee_view, &self.data)
    }
    pub(super) fn reconcile_workspaces(&mut self, ctx: &egui::Context) {
        if self.data.mode == Mode::SignedOut {
            return;
        }
        let (page, section) = workspaces::reconcile(
            self.page,
            self.employee_view,
            &self.data,
            self.has_unsaved_edits(),
        );
        self.employee_view = section;
        if page != self.page {
            self.navigate(page, ctx);
        }
    }
    pub(super) fn workspace_settings(&mut self, ui: &mut Ui) {
        if !self
            .data
            .access
            .allows(crate::staff_roles::Capability::CompanyConfiguration)
        {
            return;
        }
        card(ui, |ui| {
            ui.set_max_width(ui.available_width().min(840.0));
            ui.label(RichText::new("Workspaces").size(18.0).strong());
            ui.label("Choose optional sections shown in this company's desktop app. Records, role permissions and the employee portal stay unchanged.");
            ui.label(RichText::new("Overview, attendance, leave, your profile and account administration remain available within each person's permissions.").color(muted(ui)).small());
            if self.data.workspaces.schema_version != 1 {
                ui.label("Company workspace settings need a database update. Existing sections remain available.");
                return;
            }
            let current = self.data.workspaces.clone();
            let draft = self
                .workspace_draft
                .get_or_insert_with(|| crate::workspaces::Draft::from(&current));
            let pending = draft.pending.is_some();
            let stale = draft.stale(&current);
            ui.add_enabled_ui(!pending && !self.busy, |ui| {
                for module in Module::ALL {
                    let mut enabled = draft.enabled.contains(&module);
                    if ui.checkbox(&mut enabled, module.label()).changed() {
                        if enabled {
                            draft.enabled.push(module)
                        } else {
                            draft.enabled.retain(|m| *m != module)
                        }
                        draft.enabled.sort();
                    }
                    ui.indent(("module_detail", module), |ui| {
                        ui.label(RichText::new(module.description()).color(muted(ui)).small());
                    });
                }
            });
            if stale {
                ui.label(RichText::new("These settings changed elsewhere. Your choices are retained; cancel and review the current values before starting another save.").color(ui.visuals().warn_fg_color));
            }
            if pending {
                ui.label(
                    "The save outcome is unresolved. Retry the same change to recover its receipt.",
                );
            }
            let changed = draft.changed();
            let mut save = false;
            let mut cancel = false;
            ui.horizontal_wrapped(|ui| {
                save = ui
                    .add_enabled(
                        !self.busy && changed && (!stale || pending),
                        egui::Button::new(if pending {
                            "Retry same visibility change"
                        } else {
                            "Save workspace settings"
                        }),
                    )
                    .clicked();
                cancel = ui
                    .add_enabled(
                        !self.busy && !pending && changed,
                        egui::Button::new("Cancel workspace changes"),
                    )
                    .clicked();
            });
            if save {
                match self
                    .workspace_draft
                    .as_mut()
                    .unwrap()
                    .request(&current, &self.data.access)
                {
                    Ok(request) => self.send(Command::SaveWorkspaces(request)),
                    Err(error) => self.notice = Some((error.to_string(), true)),
                }
            }
            if cancel {
                self.workspace_draft = None
            }
        });
        ui.add_space(12.0);
    }
}

#[cfg(feature = "qa-screenshot")]
impl HrisApp {
    pub(super) fn prepare_workspace_preview(&mut self) {
        if self.data.mode != Mode::Demo {
            return;
        }
        if let Ok(raw) = std::env::var("HRIS_PREVIEW_STAFF_ROLES") {
            assert!(raw.len() <= 512);
            let roles: crate::staff_roles::StaffRoles =
                serde_json::from_str(&raw).expect("fictional preview roles");
            self.data.access.user_id = Uuid::from_u128(640);
            self.data.access.staff_contract_version = 1;
            self.data.access.role = roles.compatible_base_role();
            self.data
                .access
                .bind_staff_context(crate::staff_roles::Context {
                    schema_version: 1,
                    company_id: self.data.company.id,
                    user_id: self.data.access.user_id,
                    employee_id: self.data.access.employee_id.unwrap(),
                    base_role: self.data.access.role,
                    revision: 1,
                    runtime_ready: true,
                    roles,
                })
                .unwrap();
            let own = self.data.access.employee_id;
            self.data.access.direct_reports = if self
                .data
                .access
                .allows(crate::staff_roles::Capability::TeamReview)
            {
                self.data
                    .employees
                    .iter()
                    .filter(|e| e.reports_to == own)
                    .map(|e| e.id)
                    .collect()
            } else {
                Vec::new()
            };
            if !self
                .data
                .access
                .allows(crate::staff_roles::Capability::Personnel)
            {
                let access = self.data.access.clone();
                let visible: Vec<_> = self
                    .data
                    .employees
                    .iter()
                    .filter(|e| access.can_read_operational_employee(e))
                    .map(|e| e.id)
                    .collect();
                self.data
                    .attendance
                    .retain(|r| visible.contains(&r.employee_id));
                self.data.leave.retain(|r| visible.contains(&r.employee_id));
                self.data.contacts.retain(|r| Some(r.employee_id) == own);
            }
            self.data.workspaces = crate::workspaces::Context {
                schema_version: 1,
                company_id: self.data.company.id,
                user_id: self.data.access.user_id,
                ..Default::default()
            };
        }
        if let Ok(raw) = std::env::var("HRIS_PREVIEW_MODULES") {
            assert!(raw.len() <= 512);
            let mut modules: Vec<Module> =
                serde_json::from_str(&raw).expect("fictional optional sections");
            modules.sort();
            modules.dedup();
            self.data.workspaces.enabled = modules;
            self.data.workspaces.revision = 1;
            self.data.workspaces.updated_at = Some(chrono::Utc::now());
        }
        if std::env::var("HRIS_PREVIEW_PAGE").as_deref() == Ok("landing") {
            self.page = workspaces::landing(&self.data);
        }
        if self.page == Page::Employees && workspaces::prefers_development(&self.data) {
            self.employee_view = EmployeeView::Development;
        }
    }
}
