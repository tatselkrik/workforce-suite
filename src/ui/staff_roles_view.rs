use super::*;
use crate::staff_roles::{
    StaffRole,
    admin::{Account, Operation},
};
impl HrisApp {
    pub(super) fn staff_roles_panel(&mut self, ui: &mut Ui) {
        if self.staff_provision_state.form.is_some() {
            self.staff_provision_form(ui);
            return;
        }
        ui.horizontal_wrapped(|ui| {
            if ui
                .add_enabled(
                    !self.busy && !self.staff_roles_state.has_draft(),
                    egui::Button::new("+ Add staff employee"),
                )
                .clicked()
            {
                if let Err(error) = self.staff_provision_state.start_new(&self.data) {
                    self.notice = Some((error.to_string(), true));
                }
            }
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
        self.staff_account_history(ui);
        self.staff_provision_history(ui);
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
                if ui
                    .add_enabled(!self.busy, egui::Button::new("Access history"))
                    .clicked()
                {
                    self.send(Command::StaffRoles(Operation::History {
                        user_id: account.user_id,
                    }));
                }
                if self
                    .data
                    .accounts
                    .iter()
                    .any(|a| a.user_id == account.user_id && a.active && !a.email_confirmed)
                    && ui
                        .add_enabled(!self.busy, egui::Button::new("Send confirmation"))
                        .clicked()
                {
                    self.send(Command::Admin(AdminAction::ResendConfirmation(
                        account.user_id,
                    )));
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
    fn staff_account_history(&mut self, ui: &mut Ui) {
        let Some(history) = self.staff_roles_state.history.clone() else {
            return;
        };
        let valid = self
            .staff_roles_state
            .context
            .as_ref()
            .and_then(|c| c.accounts.iter().find(|a| a.user_id == history.user_id))
            .is_some_and(|a| history.validate(&self.data.access, a).is_ok());
        if !valid {
            self.staff_roles_state.history = None;
            return;
        }
        let (refresh, close) = history_fields(ui, &history, self.data.company.tz(), self.busy);
        if refresh {
            self.send(Command::StaffRoles(Operation::History {
                user_id: history.user_id,
            }));
        }
        if close {
            self.staff_roles_state.history = None;
        }
    }

    pub(super) fn staff_role_draft(&mut self, ui: &mut Ui) {
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

fn history_fields(
    ui: &mut Ui,
    history: &crate::staff_roles::history::History,
    zone: chrono_tz::Tz,
    busy: bool,
) -> (bool, bool) {
    use crate::staff_roles::history::{access_label, roles_label};
    let (mut refresh, mut close) = (false, false);
    card(ui, |ui| {
        ui.horizontal_wrapped(|ui| {
            ui.add(
                egui::Label::new(
                    RichText::new(format!(
                        "Access history · {} · {}",
                        history.employee_code, history.full_name
                    ))
                    .strong(),
                )
                .wrap(),
            );
            if ui
                .add_enabled(!busy, egui::Button::new("Refresh history"))
                .clicked()
            {
                refresh = true;
            }
            if ui.button("Close history").clicked() {
                close = true;
            }
        });
        ui.label("Immutable responsibilities and sign-in history. Earlier values that were not recorded stay unknown. Actor names below are current employee names, not saved historical names.");
        ui.collapsing(
            format!("Role history ({})", history.role_event_count),
            |ui| {
                if history.role_events.is_empty() {
                    ui.label("No responsibility history has been captured for this account.");
                }
                for event in history.role_events.iter().rev() {
                    ui.separator();
                    ui.strong(format!(
                        "{} · revision {}",
                        event.kind.label(),
                        event.revision
                    ));
                    ui.label(format!(
                        "{} · {}",
                        event
                            .created_at
                            .with_timezone(&zone)
                            .format("%d %b %Y %H:%M"),
                        event
                            .actor_name
                            .as_deref()
                            .unwrap_or(if event.actor_id.is_none() {
                                "Deployment baseline"
                            } else {
                                "Actor name unavailable"
                            })
                    ));
                    ui.label(format!(
                        "{} → {}",
                        event
                            .previous_roles
                            .as_ref()
                            .map(roles_label)
                            .unwrap_or_else(|| "Not recorded".into()),
                        roles_label(&event.roles)
                    ));
                    ui.label(format!(
                        "Sign-in access: {} → {}",
                        access_label(event.previous_active),
                        access_label(event.active)
                    ));
                    ui.label(&event.reason);
                }
            },
        );
        ui.collapsing(
            format!("Original access events ({})", history.access_event_count),
            |ui| {
                if history.access_events.is_empty() {
                    ui.label("No original account events were recorded.");
                }
                for event in history.access_events.iter().rev() {
                    ui.separator();
                    ui.strong(event.action.label());
                    ui.label(format!(
                        "{} · {}",
                        event
                            .created_at
                            .with_timezone(&zone)
                            .format("%d %b %Y %H:%M"),
                        event
                            .actor_name
                            .as_deref()
                            .unwrap_or("Actor name unavailable")
                    ));
                    ui.label(format!(
                        "Compatibility role: {} → {}",
                        event
                            .previous_role
                            .map(|r| r.label())
                            .unwrap_or("Not recorded"),
                        event.role.map(|r| r.label()).unwrap_or("Not recorded")
                    ));
                    ui.label(format!(
                        "Sign-in access: {} → {}",
                        access_label(event.previous_active),
                        access_label(event.active)
                    ));
                }
            },
        );
    });
    (refresh, close)
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
    #[test]
    fn readonly_history_header_fits_narrow_and_wide_themes() {
        let (_, _, mut history) = crate::staff_roles::history::tests::fixture();
        history.full_name =
            "Fictional staff name that needs to wrap in a narrow Account Manager history view"
                .into();
        for width in [320.0, 480.0, 760.0] {
            for dark in [false, true] {
                let ctx = egui::Context::default();
                configure_theme(&ctx);
                ctx.set_visuals(if dark {
                    egui::Visuals::dark()
                } else {
                    egui::Visuals::light()
                });
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
                            let width = ui.available_width();
                            let scroll = egui::ScrollArea::vertical().show(ui, |ui| {
                                assert_eq!(
                                    history_fields(ui, &history, chrono_tz::Asia::Manila, false),
                                    (false, false)
                                );
                            });
                            assert!(
                                scroll.content_size.x <= width + 1.0,
                                "History overflow: {:?}",
                                scroll.content_size
                            );
                        });
                    },
                );
                output.textures_delta.clear();
            }
        }
    }
}
