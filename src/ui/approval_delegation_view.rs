use super::*;
use crate::approvals::{self, Change, Context, Scope};

#[derive(Default)]
pub(super) struct State {
    access: Option<crate::access::Access>,
    principal: Option<Uuid>,
    delegate: Option<Uuid>,
    workflow: Scope,
    starts: String,
    ends: String,
    reason: String,
    reference: String,
    revoke: Option<Uuid>,
    pending: Option<Change>,
    error: Option<String>,
    saved: bool,
    page: usize,
}
impl State {
    pub(super) fn has_draft(&self) -> bool {
        self.pending.is_some()
            || self.revoke.is_some()
            || !self.reason.is_empty()
            || !self.reference.is_empty()
    }
    fn reconcile(&mut self, data: &Snapshot) {
        if self.access.as_ref() != Some(&data.access) {
            *self = Self {
                access: Some(data.access.clone()),
                ..Default::default()
            };
        }
        if let (Some(p), Some(c)) = (&self.pending, &data.approvals) {
            let exists = c.grants.iter().any(|g| g.id == p.id())
                || c.revocations.iter().any(|r| r.id == p.id());
            if exists {
                if p.matches_history(c) {
                    self.pending = None;
                    self.reason.clear();
                    self.reference.clear();
                    self.revoke = None;
                    self.error = None;
                    self.saved = true;
                } else {
                    self.error=Some("This request identity has different recorded details. Keep the draft and review history.".into());
                }
            }
        }
    }
}
impl HrisApp {
    #[cfg(feature = "qa-screenshot")]
    pub(super) fn prepare_approval_delegation_preview(&mut self) {
        let form = std::env::var("HRIS_PREVIEW_FORM").unwrap_or_default();
        if !form.starts_with("delegation") || self.data.mode != Mode::Demo {
            return;
        }
        self.data.approvals = Some(Context::empty(&self.data));
        self.data.approvals.as_mut().unwrap().staff = self
            .data
            .employees
            .iter()
            .take(3)
            .enumerate()
            .map(|(n, e)| approvals::Staff {
                user_id: Uuid::from_u128(104810 + n as u128),
                employee_id: e.id,
                name: e.full_name.clone(),
                role: Role::Manager,
            })
            .collect();
        let staff = self.data.approvals.as_ref().unwrap().staff.clone();
        if staff.len() < 2 {
            return;
        }
        let today = self.data.company.today();
        for (n, scope) in Scope::ALL.into_iter().enumerate() {
            let change=Change::Grant{id:Uuid::from_u128(948001+n as u128),expected_revision:n as i64,scope,principal_id:staff[0].user_id,delegate_id:staff[1].user_id,starts_on:if n==1{today+Duration::days(2)}else{today},ends_on:today+Duration::days(10),reason:"Fictional approved absence coverage; original plans and decisions remain in history.".into(),reference:"Fictional authorization record".into()};
            approvals::apply_demo(&mut self.data, &change).unwrap();
        }
        let revoke = Change::Revoke {
            id: Uuid::from_u128(948004),
            expected_revision: 3,
            delegation_id: Uuid::from_u128(948003),
            reason: "Fictional early return; original assignment retained.".into(),
        };
        approvals::apply_demo(&mut self.data, &revoke).unwrap();
        let c = self.data.approvals.as_mut().unwrap();
        let grant = &c.grants[0];
        c.receipts.push(approvals::Receipt {
            id: Uuid::from_u128(948005),
            delegation_id: grant.id,
            scope: Scope::Leave,
            request_id: Uuid::from_u128(948099),
            request_revision: 1,
            decision: approvals::Decision::Approve,
            principal_id: grant.principal_id,
            delegate_id: grant.delegate_id,
            principal_name: grant.principal_name.clone(),
            delegate_name: grant.delegate_name.clone(),
            created_at: chrono::Utc::now(),
        });
        if form.contains("manager") {
            self.data.access.role = Role::Manager;
            self.data.access.user_id = staff[1].user_id;
            self.data.access.employee_id = Some(staff[1].employee_id);
            c.user_id = self.data.access.user_id;
            c.employee_id = self.data.access.employee_id;
            c.role = Role::Manager;
            c.audit.clear();
            c.staff.clear();
        }
        let mut state = State {
            access: Some(self.data.access.clone()),
            ..Default::default()
        };
        if form.contains("draft") {
            state.principal = Some(staff[0].user_id);
            state.delegate = Some(staff[1].user_id);
            state.workflow = Scope::Overtime;
            state.reason="Fictional coverage during approved absence. Current authority and source facts are checked for each request.".into();
            state.reference = "Fictional authorization only".into();
        }
        self.leave_state.delegation = state;
        self.leave_state.view = super::leave_view::View::Delegation;
        self.page = Page::Leave;
    }
    pub(super) fn approval_delegations(&mut self, ui: &mut Ui) {
        if self.data.mode == Mode::Demo && self.data.approvals.is_none() {
            self.data.approvals = Some(Context::empty(&self.data));
        }
        let mut state = std::mem::take(&mut self.leave_state.delegation);
        state.reconcile(&self.data);
        let Some(c) = self.data.approvals.as_ref() else {
            empty(ui, "Refresh to load approval delegation.");
            self.leave_state.delegation = state;
            return;
        };
        let admin = self.data.access.role.manages_accounts();
        let today = self.data.company.today();
        let mut command = None;
        card(ui, |ui| {
            ui.label(
                RichText::new("Temporary approval delegation")
                    .size(20.0)
                    .strong(),
            );
            ui.label("Assign a substitute for leave, attendance corrections or overtime while an approver is away. Original approval plans and every decision remain in history.");
            ui.label(RichText::new("Only Administrators set this up. It does not grant access to personnel files, account settings or payroll approvals. Self-approval and forwarding delegated authority are blocked.").small().color(muted(ui)));
            ui.label(format!("Dates include both endpoints and use {}. Staff roles and access are checked again for every review.",self.data.company.timezone));
        });
        if state.saved {
            ui.label(
                RichText::new("Delegation change saved. Original history retained.").color(TEAL),
            );
        }
        ui.add_space(10.0);
        if admin {
            card(ui, |ui| {
                if let Some(g) = state
                    .revoke
                    .and_then(|id| c.grants.iter().find(|g| g.id == id))
                {
                    ui.strong("Revoke delegation");
                    ui.label(format!(
                        "{} covering {} · {}",
                        g.delegate_name,
                        g.principal_name,
                        g.scope.label()
                    ));
                    ui.label("Revocation stops further delegated decisions immediately. Earlier decisions remain valid and visible in history.");
                } else {
                    ui.strong("Add delegation");
                }
                if state.starts.is_empty() {
                    state.starts = today.to_string();
                }
                if state.ends.is_empty() {
                    state.ends = (today + Duration::days(7)).to_string();
                }
                ui.add_enabled_ui(!self.busy&&state.pending.is_none(),|ui|{
                if state.revoke.is_none(){
                    ui.horizontal_wrapped(|ui|{
                        ui.label("Workflow");egui::ComboBox::from_id_salt("delegation-workflow").selected_text(state.workflow.label()).show_ui(ui,|ui|{for s in Scope::ALL{ui.selectable_value(&mut state.workflow,s,s.label());}});
                    });
                    egui::Grid::new("delegation-staff").num_columns(2).spacing([16.0,10.0]).show(ui,|ui|{
                        ui.label("Original approver");egui::ComboBox::from_id_salt("delegation-principal").width(340.0).selected_text(c.staff.iter().find(|s|Some(s.user_id)==state.principal).map(|s|format!("{} · {}",s.name,s.role.label())).unwrap_or_else(||"Choose staff member".into())).show_ui(ui,|ui|{for s in &c.staff{ui.selectable_value(&mut state.principal,Some(s.user_id),format!("{} · {}",s.name,s.role.label()));}});ui.end_row();
                        ui.label("Substitute approver");let original=c.staff.iter().find(|s|Some(s.user_id)==state.principal);
                        egui::ComboBox::from_id_salt("delegation-substitute").width(340.0).selected_text(c.staff.iter().find(|s|Some(s.user_id)==state.delegate).map(|s|format!("{} · {}",s.name,s.role.label())).unwrap_or_else(||"Choose substitute".into())).show_ui(ui,|ui|{for s in c.staff.iter().filter(|s|original.is_some_and(|p|p.user_id!=s.user_id&&approvals::substitute_role(p.role,s.role))){ui.selectable_value(&mut state.delegate,Some(s.user_id),format!("{} · {}",s.name,s.role.label()));}});ui.end_row();
                        ui.label("Starts on");ui.add(egui::TextEdit::singleline(&mut state.starts).desired_width(180.0).hint_text("YYYY-MM-DD"));ui.end_row();
                        ui.label("Ends on (inclusive)");ui.add(egui::TextEdit::singleline(&mut state.ends).desired_width(180.0).hint_text("YYYY-MM-DD"));ui.end_row();
                    });
                    ui.label(RichText::new("HR requires an HR/Admin substitute. Administrator steps require another Administrator. Use one delegation per workflow; overlapping assignments for the same original approver are blocked.").small().color(muted(ui)));
                }
                ui.label("Reason");ui.add(egui::TextEdit::multiline(&mut state.reason).desired_rows(2).desired_width(f32::INFINITY).char_limit(500));
                if state.revoke.is_none(){ui.label("Reference (optional, HR/Admin only)");ui.add(egui::TextEdit::singleline(&mut state.reference).desired_width(400.0).char_limit(200));}
                ui.horizontal_wrapped(|ui|{
                    if ui.button(if state.revoke.is_some(){"Revoke delegation"}else{"Save delegation"}).clicked(){
                        let prepared=(||->anyhow::Result<Change>{
                            let id=Uuid::new_v4();let expected_revision=c.revision;
                            let change=if let Some(delegation_id)=state.revoke{Change::Revoke{id,expected_revision,delegation_id,reason:state.reason.trim().into()}}else{Change::Grant{id,expected_revision,scope:state.workflow,principal_id:state.principal.ok_or_else(||anyhow::anyhow!("Choose the original approver."))?,delegate_id:state.delegate.ok_or_else(||anyhow::anyhow!("Choose a substitute."))?,starts_on:parse_date(&state.starts)?,ends_on:parse_date(&state.ends)?,reason:state.reason.trim().into(),reference:state.reference.trim().into()}};
                            change.payload(&self.data)?;Ok(change)
                        })();
                        match prepared{Ok(change)=>{state.pending=Some(change.clone());state.error=None;state.saved=false;command=Some(change);},Err(e)=>state.error=Some(e.to_string())}
                    }
                    if state.revoke.is_some()&&ui.button("Cancel revocation draft").clicked(){state.revoke=None;state.reason.clear();state.error=None;}
                });
            });
                if let Some(p) = &state.pending {
                    ui.label("This submitted draft is retained until its outcome is known. Refresh before retrying; the same request identity prevents duplicate saves.");
                    ui.horizontal_wrapped(|ui| {
                        if ui
                            .add_enabled(!self.busy, egui::Button::new("Retry same request"))
                            .clicked()
                        {
                            command = Some(p.clone());
                        }
                    });
                    if p.expected_revision() != c.revision
                        && !c.grants.iter().any(|g| g.id == p.id())
                        && !c.revocations.iter().any(|r| r.id == p.id())
                        && ui
                            .add_enabled(
                                !self.busy,
                                egui::Button::new("Review updated settings and return to draft"),
                            )
                            .clicked()
                    {
                        state.pending = None;
                        state.error = None;
                    }
                }
                if let Some(error) = &state.error {
                    ui.colored_label(ui.visuals().error_fg_color, error);
                }
            });
        }
        ui.add_space(12.0);
        ui.heading(if self.data.access.role.manages_hr() {
            "Delegation history"
        } else {
            "Your delegation assignments"
        });
        let total = c.grants.len();
        let pages = total.div_ceil(15).max(1);
        state.page = state.page.min(pages - 1);
        ui.horizontal_wrapped(|ui| {
            if ui
                .add_enabled(state.page > 0, egui::Button::new("Previous"))
                .clicked()
            {
                state.page -= 1;
            }
            ui.label(format!(
                "Page {} of {pages} · {total} assignment(s)",
                state.page + 1
            ));
            if ui
                .add_enabled(state.page + 1 < pages, egui::Button::new("Next"))
                .clicked()
            {
                state.page += 1;
            }
        });
        if c.grants.is_empty() {
            card(ui, |ui| {
                empty(
                    ui,
                    "No temporary approval delegations in your current scope.",
                )
            });
        }
        for g in c.grants.iter().rev().skip(state.page * 15).take(15) {
            ui.add_space(8.0);
            card(ui, |ui| {
                let revoked = c.revocations.iter().find(|r| r.delegation_id == g.id);
                let status = if revoked.is_some() {
                    "Revoked"
                } else if g.current {
                    "Active"
                } else if g.starts_on > today {
                    "Scheduled"
                } else if g.ends_on < today {
                    "Expired"
                } else {
                    "Staff access changed"
                };
                ui.horizontal_wrapped(|ui| {
                    ui.strong(format!("{} covering {}", g.delegate_name, g.principal_name));
                    badge(ui, status, g.current);
                    ui.label(g.scope.label());
                });
                ui.label(format!(
                    "{} through {} · Original: {} · Substitute: {}",
                    g.starts_on,
                    g.ends_on,
                    g.principal_role.label(),
                    g.delegate_role.label()
                ));
                if let Some(a) = c.audit.iter().find(|a| a.id == g.id) {
                    egui::CollapsingHeader::new("HR/Admin record")
                        .id_salt(("delegation-audit", g.id))
                        .show(ui, |ui| {
                            ui.label(&a.reason);
                            if !a.reference.is_empty() {
                                ui.label(format!("Reference: {}", a.reference));
                            }
                            ui.label(format!(
                                "Recorded by {} · {}",
                                a.actor_name,
                                a.created_at
                                    .with_timezone(&self.data.company.tz())
                                    .format("%Y-%m-%d %H:%M")
                            ));
                        });
                }
                if let Some(r) = revoked {
                    ui.label(format!(
                        "Revoked {}",
                        r.created_at
                            .with_timezone(&self.data.company.tz())
                            .format("%Y-%m-%d %H:%M")
                    ));
                    if let Some(a) = c.audit.iter().find(|a| a.id == r.id) {
                        ui.label(format!("{} · {}", a.actor_name, a.reason));
                    }
                }
                if admin
                    && revoked.is_none()
                    && ui
                        .add_enabled(
                            !self.busy && state.pending.is_none(),
                            egui::Button::new("Revoke…"),
                        )
                        .clicked()
                {
                    state.revoke = Some(g.id);
                    state.reason.clear();
                    state.saved = false;
                    state.error = None;
                }
            });
        }
        ui.add_space(12.0);
        ui.heading("Delegated decision receipts");
        if c.receipts.is_empty() {
            ui.label("No decisions have used these assignments yet.");
        }
        for r in c.receipts.iter().rev().take(30) {
            card(ui, |ui| {
                ui.strong(format!("{} · {}", r.scope.label(), r.decision.label()));
                ui.label(format!(
                    "{} acted for {}",
                    r.delegate_name, r.principal_name
                ));
                ui.label(
                    r.created_at
                        .with_timezone(&self.data.company.tz())
                        .format("%Y-%m-%d %H:%M")
                        .to_string(),
                );
            });
        }
        if c.receipts.len() > 30 {
            ui.label(format!("Showing the latest30 of{} retained receipts. Open an individual request for its complete delegated decision history.",c.receipts.len()));
        }
        self.leave_state.delegation = state;
        if let Some(change) = command {
            self.send(Command::Change(Action::ApprovalDelegation(change)));
        }
    }
    pub(super) fn delegated_review_history(&self, ui: &mut Ui, scope: Scope, id: Uuid) {
        let Some(c) = &self.data.approvals else {
            return;
        };
        if let Some(g) = c
            .permits
            .iter()
            .find(|p| p.scope == scope && p.request_id == id)
            .and_then(|p| c.grants.iter().find(|g| g.id == p.delegation_id))
        {
            ui.label(RichText::new(format!("You are reviewing for {} under a temporary delegation through {}. Current authority and source facts are checked again when you decide.",g.principal_name,g.ends_on)).strong());
        }
        for r in c
            .receipts
            .iter()
            .filter(|r| r.scope == scope && r.request_id == id)
        {
            ui.label(format!(
                "{}: {} acted for {} · {}",
                r.decision.label(),
                r.delegate_name,
                r.principal_name,
                r.created_at
                    .with_timezone(&self.data.company.tz())
                    .format("%Y-%m-%d %H:%M")
            ));
        }
    }
}
