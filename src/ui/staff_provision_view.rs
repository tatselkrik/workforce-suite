use super::*;
use crate::staff_roles::{
    StaffRole,
    provision::{self, Operation, Rejection, RejectionKind, Stage},
    provision_editor::{Form, Section},
};
use anyhow::{Result, ensure};
impl HrisApp {
    pub(super) fn accept_staff_provision(&mut self, update: provision::Update) -> Result<()> {
        let actor = &self.data.access;
        match update {
            provision::Update::Source(source) => {
                self.staff_provision_state.accept_source(actor, source)?
            }
            provision::Update::Pending(pending) => {
                self.staff_provision_state.accept_pending(actor, pending)?
            }
            provision::Update::Completed { receipt, accounts } => {
                accounts.validate(actor)?;
                self.staff_provision_state.accept_receipt(actor, &receipt)?;
                self.staff_roles_state.context = Some(accounts);
            }
            provision::Update::Cancelled(id) => {
                self.staff_provision_state.accept_cancelled(actor, id)?
            }
            provision::Update::Status { request, intent } => {
                ensure!(
                    self.staff_provision_state
                        .pending
                        .as_ref()
                        .is_some_and(|p| p.data().id == request.data().id),
                    "The account status belongs to another pending form."
                );
                let kind = if let Some(intent) = intent {
                    intent.validate(actor, &request)?;
                    match intent.state {
                        Stage::Reserved => RejectionKind::NeedsReview,
                        Stage::Superseded => RejectionKind::Closed,
                        _ => anyhow::bail!(
                            "The resolved account status requires its completion or cancellation receipt."
                        ),
                    }
                } else {
                    RejectionKind::NotStarted
                };
                self.staff_provision_state.reject(&Rejection {
                    id: request.data().id,
                    kind,
                })?;
            }
        }
        Ok(())
    }
    pub(super) fn staff_provision_history(&mut self, ui: &mut Ui) {
        ui.collapsing("Pending account setup",|ui|{
   if ui.add_enabled(!self.busy,egui::Button::new("Refresh pending accounts")).clicked(){self.staff_provision_state.history_requested=true;self.send(Command::StaffProvision(Operation::Pending));}
   let Some(history)=self.staff_provision_state.history.clone()else{ui.label("Refresh to recover your saved account attempts after an interrupted setup.");return;};
   ui.label(format!("{} pending account attempt(s)",history.entry_count));
   for item in &history.entries{card(ui,|ui|{
    ui.label(RichText::new(&item.request.data().employee.full_name).strong());ui.label(format!("{} · {}",item.request.data().employee.employee_code,item.intent.email));
    ui.label("Sign-in identity reserved; employee access is not granted until setup is committed.");
    if ui.add_enabled(!self.busy&&!self.staff_provision_state.has_draft()&&!self.staff_roles_state.has_draft(),egui::Button::new("Resume this attempt")).clicked(){
     if let Err(error)=self.staff_provision_state.resume(&self.data.access,item){self.notice=Some((error.to_string(),true));}
    }
   });}
  });
    }
    pub(super) fn staff_employee_access(
        &mut self,
        ui: &mut Ui,
        employee: &EmployeeDraft,
        contacts: &EmployeeContacts,
    ) -> bool {
        let Some(id) = employee.id else {
            ui.label("Save this employee before adding sign-in access, or use Add staff employee in Account Manager.");
            return false;
        };
        let account = self
            .data
            .accounts
            .iter()
            .find(|a| a.employee_id == Some(id))
            .cloned();
        if let Some(account) = account {
            ui.label(format!("Sign-in email: {}", account.email));
            if self.staff_roles_state.context.is_none()
                && !self.staff_roles_state.load_requested
                && !self.busy
            {
                self.staff_roles_state.load_requested = true;
                self.send(Command::StaffRoles(
                    crate::staff_roles::admin::Operation::Refresh,
                ));
            }
            if let Some(roles) = self
                .staff_roles_state
                .context
                .as_ref()
                .and_then(|c| c.accounts.iter().find(|a| a.user_id == account.user_id))
            {
                ui.horizontal_wrapped(|ui| {
                    for role in roles.roles.as_slice() {
                        badge(ui, role.label(), roles.active);
                    }
                });
            }
            if account.user_id == self.data.access.user_id {
                ui.label("Another Administrator manages your roles and sign-in access.");
            } else if self
                .staff_roles_state
                .draft
                .as_ref()
                .is_some_and(|d| d.source.employee_id == id)
            {
                self.staff_role_draft(ui);
            } else if ui
                .add_enabled(
                    !self.busy
                        && !self.staff_roles_state.has_draft()
                        && !self.staff_provision_state.has_draft(),
                    egui::Button::new("Manage responsibilities and access"),
                )
                .clicked()
            {
                if let Err(error) = self
                    .staff_roles_state
                    .open(&self.data.access, account.user_id)
                {
                    self.notice = Some((error.to_string(), true));
                }
            }
            if !account.email_confirmed
                && account.active
                && ui.button("Send confirmation").clicked()
            {
                self.send(Command::Admin(AdminAction::ResendConfirmation(
                    account.user_id,
                )));
            }
            ui.label("Employment and contact changes are saved separately using Save changes.");
            return false;
        }
        ui.label("This employee has no sign-in account. Their employment and contact edits can be saved together with account setup.");
        if ui
            .add_enabled(
                !self.busy
                    && !self.staff_provision_state.has_draft()
                    && !self.staff_roles_state.has_draft(),
                egui::Button::new("Add sign-in access"),
            )
            .clicked()
        {
            match self.staff_provision_state.start_existing(
                &self.data,
                employee.clone(),
                contacts.clone(),
            ) {
                Ok(id) => {
                    self.page = Page::Accounts;
                    self.send(Command::StaffProvision(Operation::Source(id)));
                    return true;
                }
                Err(error) => self.notice = Some((error.to_string(), true)),
            }
        }
        false
    }
    pub(super) fn staff_provision_form(&mut self, ui: &mut Ui) {
        let pending = self.staff_provision_state.pending.is_some();
        let cancelling = self.staff_provision_state.pending_cancel.is_some();
        let review = self.staff_provision_state.review;
        let fixed_email = pending || self.staff_provision_state.supersedes.is_some();
        let Some(form) = self.staff_provision_state.form.as_mut() else {
            return;
        };
        let (mut save, mut discard) = (false, false);
        let mut check = false;
        let mut load_source = false;
        let mut reviewed = false;
        let mut cancel = false;
        card(ui, |ui| {
            ui.heading(if form.employee.id.is_some() {
                "Employee sign-in setup"
            } else {
                "Add staff employee"
            });
            ui.label("Save employment, private contacts and assigned responsibilities together.");
            provision_fields(
                ui,
                form,
                &self.data,
                !pending && !self.busy,
                !self.busy && !cancelling && review.is_none(),
                fixed_email,
            );
            if let Some(kind) = review {
                ui.separator();
                ui.label(RichText::new(match kind{RejectionKind::NotStarted=>"No reservation was saved for this attempt. Review current details before trying again.",RejectionKind::NeedsReview=>"The saved account attempt needs review. A replacement retains its reserved sign-in identity and original password.",RejectionKind::Closed=>"This attempt was replaced. Return to pending accounts to find the current one."}).strong());
                if kind != RejectionKind::Closed {
                    if form.employee.id.is_some() {
                        load_source = ui
                            .add_enabled(
                                !self.busy,
                                egui::Button::new("Load current saved employment and contacts"),
                            )
                            .clicked();
                        if let Some(source) = &self.staff_provision_state.offered_source {
                            ui.label(format!(
                                "Current saved record: {} · {} · {}",
                                source.employee.employee_code,
                                source.employee.full_name,
                                source.employee.department
                            ));
                            ui.label("Using this source replaces the employment/contact edits in this form. The previous submitted attempt remains in history.");
                            reviewed = ui
                                .add_enabled(
                                    !self.busy,
                                    egui::Button::new("Use saved details and review a replacement"),
                                )
                                .clicked();
                        }
                    } else {
                        reviewed = ui
                            .add_enabled(
                                !self.busy,
                                egui::Button::new("Review a replacement attempt"),
                            )
                            .clicked();
                    }
                }
            }
            ui.horizontal_wrapped(|ui| {
                save |= ui
                    .add_enabled(
                        !self.busy && !cancelling && review.is_none(),
                        egui::Button::new(if pending {
                            "Retry this exact setup"
                        } else {
                            "Save employee and access"
                        }),
                    )
                    .clicked();
                discard |= ui
                    .add_enabled(
                        !self.busy && (!pending || review == Some(RejectionKind::NotStarted)),
                        egui::Button::new("Discard this form"),
                    )
                    .clicked();
                if pending {
                    check = ui
                        .add_enabled(!self.busy, egui::Button::new("Check saved outcome"))
                        .clicked();
                }
            });
            if pending {
                ui.label("Your original submitted details stay fixed until the saved outcome is checked. Retrying does not create a second sign-in identity.");
                ui.collapsing("Close this pending attempt",|ui|{
     let reason=self.staff_provision_state.cancel_reason.get_or_insert_with(String::new);ui.add_enabled(!self.busy&&!cancelling,egui::TextEdit::multiline(reason).char_limit(500).desired_rows(2).hint_text("Reason for closing this attempt"));
     cancel=ui.add_enabled(!self.busy,egui::Button::new(if cancelling{"Retry cancellation"}else{"Close pending attempt"})).clicked();
     ui.label("Its history and any unconfirmed sign-in identity are retained. This does not delete an employee.");
    });
            } else if form.employee.id.is_some()
                && self.staff_provision_state.source.is_none()
                && review.is_none()
            {
                ui.label("Load saved employee details before adding sign-in access.");
                load_source = ui
                    .add_enabled(!self.busy, egui::Button::new("Refresh saved details"))
                    .clicked();
            }
        });
        if load_source {
            if let Some(id) = self
                .staff_provision_state
                .form
                .as_ref()
                .and_then(|f| f.employee.id)
            {
                self.send(Command::StaffProvision(Operation::Source(id)));
            }
        }
        if reviewed {
            if let Err(error) = self.staff_provision_state.review_current(&self.data.access) {
                self.notice = Some((error.to_string(), true));
            }
        }
        if check {
            if let Some(request) = self.staff_provision_state.pending.clone() {
                self.send(Command::StaffProvision(Operation::Status(request)));
            }
        }
        if cancel {
            match self.staff_provision_state.cancel_request() {
                Ok((request, reason)) => self.send(Command::StaffProvision(Operation::Cancel {
                    request,
                    reason,
                })),
                Err(error) => self.notice = Some((error.to_string(), true)),
            }
        }
        if discard {
            if let Err(error) = self.staff_provision_state.discard() {
                self.notice = Some((error.to_string(), true));
            }
        }
        if save {
            match self.staff_provision_state.submit(&self.data) {
                Ok((request, password)) => {
                    clear_edit_history(
                        ui.ctx(),
                        &["staff-provision-password", "staff-provision-confirm"],
                    );
                    self.send(Command::StaffProvision(Operation::Create {
                        request,
                        password,
                    }));
                }
                Err(error) => self.notice = Some((error.to_string(), true)),
            }
        }
    }
}
fn provision_fields(
    ui: &mut Ui,
    form: &mut Form,
    data: &Snapshot,
    editable: bool,
    credentials: bool,
    fixed_email: bool,
) {
    ui.horizontal_wrapped(|ui| {
        for (section, label) in [
            (Section::Employment, "Employment"),
            (Section::Contact, "Contact details"),
            (Section::Emergency, "Emergency contact"),
            (Section::Access, "Roles and sign-in"),
        ] {
            ui.selectable_value(&mut form.section, section, label);
        }
    });
    match form.section {
        Section::Employment => {
            ui.add_enabled_ui(editable, |ui| {
                employment_fields(ui, &mut form.employee, data)
            });
        }
        Section::Contact => {
            ui.add_enabled_ui(editable, |ui| contact_fields(ui, &mut form.contacts, false));
        }
        Section::Emergency => {
            ui.add_enabled_ui(editable, |ui| contact_fields(ui, &mut form.contacts, true));
        }
        Section::Access => {
            ui.add_enabled_ui(editable, |ui| {
                if fixed_email {
                    ui.label(format!("Sign-in email: {}", form.email));
                } else {
                    field(ui, "Sign-in email", &mut form.email, "Work email");
                }
                ui.checkbox(&mut form.active, "Sign-in access enabled");
                ui.label("Staff responsibilities");
                for role in StaffRole::ALL
                    .into_iter()
                    .filter(|r| *r != StaffRole::Employee)
                {
                    let mut selected = form.roles.contains(&role);
                    if ui.checkbox(&mut selected, role.label()).changed() {
                        form.roles.retain(|r| *r != role);
                        if selected {
                            form.roles.push(role);
                        }
                    }
                }
                if form.roles.is_empty() {
                    ui.label("Employee portal only. No desktop role is assigned.");
                }
                ui.label("Reason for these responsibilities");
                ui.add(
                    egui::TextEdit::multiline(&mut form.reason)
                        .char_limit(500)
                        .desired_width(f32::INFINITY)
                        .desired_rows(2),
                );
            });
            ui.add_enabled_ui(credentials, |ui| {
                password_field(
                    ui,
                    "Initial password",
                    "staff-provision-password",
                    &mut form.password,
                    false,
                );
                password_field(
                    ui,
                    "Confirm initial password",
                    "staff-provision-confirm",
                    &mut form.confirmation,
                    false,
                );
            });
            ui.label("Use 12–128 characters. On a retry, a password is needed only if sign-in creation did not finish. An existing sign-in identity keeps its original password.");
            ui.label("After saving, use Send confirmation. No setup email is sent automatically by this form.");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn provisioning_sections_fit_themes_and_widths_and_keep_locked_values() {
        for width in [480.0, 760.0, 1100.0] {
            for dark in [false, true] {
                for editable in [false, true] {
                    for section in [
                        Section::Employment,
                        Section::Contact,
                        Section::Emergency,
                        Section::Access,
                    ] {
                        let ctx = egui::Context::default();
                        configure_theme(&ctx);
                        ctx.set_visuals(if dark {
                            egui::Visuals::dark()
                        } else {
                            egui::Visuals::light()
                        });
                        let data = DemoStore::new().data;
                        let mut form = Form {
                            employee: EmployeeDraft {
                                employee_code: "000004".into(),
                                full_name: "Fictional Staff With A Longer Name".into(),
                                annual_leave_days: 15,
                                ..Default::default()
                            },
                            contacts: EmployeeContacts::empty(data.company.id, Uuid::nil()),
                            email: "person@example.invalid".into(),
                            roles: vec![
                                StaffRole::LearningPerformanceOfficer,
                                StaffRole::PayrollOfficer,
                            ],
                            active: true,
                            reason: "Reviewed combined responsibilities".into(),
                            password: "fictional initial password".into(),
                            confirmation: "fictional initial password".into(),
                            section,
                        };
                        let before = (
                            form.employee.employee_code.clone(),
                            form.email.clone(),
                            form.roles.clone(),
                            form.password.clone(),
                        );
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
                                        let available = ui.available_width();
                                        let scroll = egui::ScrollArea::vertical().show(ui, |ui| {
                                            provision_fields(
                                                ui, &mut form, &data, editable, false, !editable,
                                            )
                                        });
                                        assert!(
                                            scroll.content_size.x <= available + 1.0,
                                            "Provisioning overflow at{width}: {:?}",
                                            scroll.content_size
                                        );
                                    });
                                },
                            );
                            output.textures_delta.clear();
                        }
                        assert_eq!(
                            (
                                form.employee.employee_code,
                                form.email,
                                form.roles,
                                form.password
                            ),
                            before
                        );
                    }
                }
            }
        }
    }
}
