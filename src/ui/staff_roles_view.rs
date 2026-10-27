use super::*;
use crate::staff_roles::{
    StaffRole,
    admin::{Account, Operation},
};
impl HrisApp {
    pub(super) fn staff_roles_panel(&mut self, ui: &mut Ui) {
        ui.horizontal_wrapped(|ui| {
            ui.label("Staff responsibilities and sign-in access");
            if ui
                .add_enabled(!self.busy, egui::Button::new("Refresh assignments"))
                .clicked()
            {
                self.staff_roles_state.load_requested = true;
                self.send(Command::StaffRoles(Operation::Refresh));
            }
        });
        let Some(context) = self.staff_roles_state.context.clone() else {
            if !self.busy && !self.staff_roles_state.load_requested {
                self.staff_roles_state.load_requested = true;
                self.send(Command::StaffRoles(Operation::Refresh));
            }
            ui.label(if self.busy {
                "Loading staff assignments…"
            } else {
                "Use Refresh assignments to retry loading."
            });
            return;
        };
        if let Err(error) = context.validate(&self.data.access) {
            ui.label(error.to_string());
            return;
        }
        if !context.runtime_ready {
            return;
        }
        ui.label(RichText::new("Each account belongs to an employee. Assign several staff roles when one person covers more than one responsibility.").color(muted(ui)));
        ui.checkbox(
            &mut self.show_disabled_staff,
            "Show disabled sign-in access",
        );
        let show_disabled = self.show_disabled_staff;
        let staff: Vec<_> = context
            .accounts
            .iter()
            .filter(|a| a.roles.uses_desktop())
            .cloned()
            .collect();
        ui.label(format!("{} staff accounts", staff.len()));
        for account in staff.iter().filter(|a| a.active || show_disabled) {
            self.staff_role_account(ui, account);
        }
        ui.collapsing("Employees with portal-only access", |ui| {
            for account in context
                .accounts
                .iter()
                .filter(|a| !a.roles.uses_desktop())
                .filter(|a| a.active || show_disabled)
            {
                self.staff_role_account(ui, account);
            }
        });
        self.staff_role_draft(ui);
    }
    fn staff_role_account(&mut self, ui: &mut Ui, account: &Account) {
        card(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.label(RichText::new(&account.full_name).strong());
                badge(ui, &account.employee_code, true);
                badge(
                    ui,
                    if account.active {
                        "Enabled"
                    } else {
                        "Disabled"
                    },
                    account.active,
                );
            });
            ui.horizontal_wrapped(|ui| {
                for role in account.roles.as_slice() {
                    badge(ui, role.label(), true);
                }
            });
            ui.horizontal_wrapped(|ui| {
                if ui
                    .add_enabled(!self.busy, egui::Button::new("Edit employee"))
                    .clicked()
                {
                    self.open_employee(account.employee_id);
                }
                let allowed = !self.busy
                    && !self.staff_roles_state.has_draft()
                    && account.user_id != self.data.access.user_id;
                if ui
                    .add_enabled(allowed, egui::Button::new("Manage access"))
                    .clicked()
                {
                    if let Err(error) = self
                        .staff_roles_state
                        .open(&self.data.access, account.user_id)
                    {
                        self.notice = Some((error.to_string(), true));
                    }
                }
                if account.user_id == self.data.access.user_id {
                    ui.label(
                        RichText::new("Another Administrator manages your access.")
                            .small()
                            .color(muted(ui)),
                    );
                }
            });
        });
        ui.add_space(8.0);
    }
    fn staff_role_draft(&mut self, ui: &mut Ui) {
        let focus = std::mem::take(&mut self.staff_roles_state.scroll_to_editor);
        let Some(draft) = self.staff_roles_state.draft.as_mut() else {
            return;
        };
        if focus {
            ui.scroll_to_cursor(Some(Align::Min));
        }
        let pending = self.staff_roles_state.pending.is_some();
        let owner = self
            .data
            .accounts
            .iter()
            .any(|a| a.user_id == draft.source.user_id && a.is_owner);
        let (save, cancel) = editor_fields(
            ui,
            draft,
            owner,
            pending,
            self.busy,
            self.staff_roles_state.rejected,
        );
        if cancel {
            if let Err(error) = self.staff_roles_state.cancel() {
                self.notice = Some((error.to_string(), true));
            }
        }
        if save {
            match self.staff_roles_state.submit(&self.data.access) {
                Ok(request) => self.send(Command::StaffRoles(Operation::Save(request))),
                Err(error) => self.notice = Some((error.to_string(), true)),
            }
        }
    }
}

fn editor_fields(
    ui: &mut Ui,
    draft: &mut crate::staff_roles::editor::Draft,
    owner: bool,
    pending: bool,
    busy: bool,
    rejected: bool,
) -> (bool, bool) {
    let mut save = false;
    let mut cancel = false;
    card(ui, |ui| {
        ui.heading(format!("Access · {}", draft.source.full_name));
        ui.add_enabled_ui(!busy && !pending, |ui| {
            ui.add_enabled(
                !owner,
                egui::Checkbox::new(&mut draft.active, "Sign-in access enabled"),
            );
            ui.label("Staff responsibilities");
            for role in StaffRole::ALL
                .into_iter()
                .filter(|r| *r != StaffRole::Employee)
            {
                let mut selected = draft.roles.contains(&role);
                if ui
                    .add_enabled(
                        !(owner && role == StaffRole::Administrator),
                        egui::Checkbox::new(&mut selected, role.label()),
                    )
                    .changed()
                {
                    draft.roles.retain(|r| *r != role);
                    if selected {
                        draft.roles.push(role);
                    }
                }
            }
            if draft.roles.is_empty() {
                ui.label("Employee portal only. No desktop role is assigned.");
            }
            ui.label("Reason for this change");
            ui.add(
                egui::TextEdit::multiline(&mut draft.reason)
                    .char_limit(500)
                    .desired_rows(2)
                    .desired_width(f32::INFINITY),
            );
        });
        if rejected {
            ui.label("Nothing was saved. Refresh assignments, then cancel and reopen this account to review a new change.");
        }
        if pending {
            ui.label(RichText::new("The save outcome is unresolved. Retry the same change to confirm its receipt; keep this draft until confirmed.").color(muted(ui)));
        }
        if owner {
            ui.label(
                RichText::new(
                    "The installation Administrator retains enabled Administrator access.",
                )
                .color(muted(ui)),
            );
        }
        ui.horizontal_wrapped(|ui| {
            save = ui
                .add_enabled(
                    !busy && !rejected,
                    egui::Button::new(if pending {
                        "Retry this change"
                    } else {
                        "Save access"
                    }),
                )
                .clicked();
            cancel = ui
                .add_enabled(!busy && !pending, egui::Button::new("Cancel"))
                .clicked();
        });
    });

    (save, cancel)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        access::Role,
        staff_roles::{StaffRoles, editor::Draft},
    };
    #[test]
    fn staff_editor_fits_narrow_and_wide_themes_without_mutating_pending_drafts() {
        for width in [480.0, 760.0, 1100.0] {
            for dark in [false, true] {
                for pending in [false, true] {
                    let ctx = egui::Context::default();
                    configure_theme(&ctx);
                    ctx.set_visuals(if dark {
                        egui::Visuals::dark()
                    } else {
                        egui::Visuals::light()
                    });
                    let mut draft = Draft {
                        source: Account {
                            user_id: Uuid::from_u128(1),
                            employee_id: Uuid::from_u128(2),
                            employee_code: "000002".into(),
                            full_name: "Fictional staff member with a longer name".into(),
                            active: true,
                            base_role: Role::Employee,
                            member_updated_at: chrono::Utc::now(),
                            revision: 1,
                            roles: StaffRoles::default(),
                        },
                        roles: vec![
                            StaffRole::LearningPerformanceOfficer,
                            StaffRole::PayrollOfficer,
                        ],
                        active: false,
                        reason: "Reviewed combined responsibilities".into(),
                    };
                    let before = (draft.roles.clone(), draft.active, draft.reason.clone());
                    for _ in 0..2 {
                        let mut output = ctx.run_ui(
                            egui::RawInput {
                                screen_rect: Some(egui::Rect::from_min_size(
                                    egui::Pos2::ZERO,
                                    Vec2::new(width, 1000.0),
                                )),
                                ..Default::default()
                            },
                            |ui| {
                                egui::CentralPanel::default().show(ui, |ui| {
                                    let available = ui.available_rect_before_wrap();
                                    let mut editor_size = Vec2::ZERO;
                                    let scroll = egui::ScrollArea::vertical().show(ui, |ui| {
                                        let start = ui.cursor().min;
                                        assert_eq!(
                                            editor_fields(
                                                ui, &mut draft, false, pending, false, false
                                            ),
                                            (false, false)
                                        );
                                        editor_size = ui.min_rect().max - start;
                                    });
                                    assert!(
                                        scroll.content_size.x <= available.width() + 1.0,
                                        "Horizontal overflow at {width}: {:?}",
                                        scroll.content_size
                                    );
                                    assert!(
                                        editor_size.y < 900.0,
                                        "Editor is excessively tall: {editor_size:?}"
                                    );
                                });
                            },
                        );
                        output.textures_delta.clear();
                    }
                    assert_eq!(
                        (draft.roles, draft.active, draft.reason),
                        before,
                        "Rendering changed a draft"
                    );
                }
            }
        }
    }
}
