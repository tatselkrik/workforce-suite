//! Source-bound Account Manager drafts; uncertain saves retain their exact identity.
use super::{
    RoleChange, StaffRole, StaffRoles,
    admin::{Account, AdminContext, Update},
};
use crate::access::Access;
use anyhow::{Result, ensure};
use uuid::Uuid;

#[derive(Clone)]
pub struct Draft {
    pub source: Account,
    pub roles: Vec<StaffRole>,
    pub active: bool,
    pub reason: String,
}
#[derive(Default)]
pub struct State {
    pub load_requested: bool,
    pub scroll_to_editor: bool,
    pub rejected: bool,
    pub context: Option<AdminContext>,
    pub draft: Option<Draft>,
    pub pending: Option<RoleChange>,
}
impl State {
    pub fn has_draft(&self) -> bool {
        self.draft.is_some() || self.pending.is_some()
    }
    pub fn open(&mut self, actor: &Access, user: Uuid) -> Result<()> {
        ensure!(
            self.pending.is_none(),
            "Resolve or retry the existing staff change first."
        );
        ensure!(
            self.draft.is_none(),
            "Save or cancel the open staff draft first."
        );
        let context = self
            .context
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Refresh staff assignments first."))?;
        context.validate(actor)?;
        ensure!(context.runtime_ready, "Staff editing is not available yet.");
        ensure!(
            user != actor.user_id,
            "Another Administrator must change your access."
        );
        let source = context
            .accounts
            .iter()
            .find(|a| a.user_id == user)
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("Choose an account from the current company."))?;
        self.draft = Some(Draft {
            roles: source
                .roles
                .as_slice()
                .iter()
                .copied()
                .filter(|r| *r != StaffRole::Employee)
                .collect(),
            active: source.active,
            source,
            reason: String::new(),
        });
        self.rejected = false;
        self.scroll_to_editor = true;
        Ok(())
    }
    pub fn cancel(&mut self) -> Result<()> {
        ensure!(
            self.pending.is_none(),
            "This save may already be recorded. Retain the exact retry until its receipt is confirmed."
        );
        self.draft = None;
        self.rejected = false;
        Ok(())
    }
    pub fn reject(&mut self) {
        self.pending = None;
        self.rejected = true;
    }
    pub fn submit(&mut self, actor: &Access) -> Result<RoleChange> {
        ensure!(
            !self.rejected,
            "Refresh assignments, then cancel and reopen this rejected draft before saving."
        );
        if let Some(request) = &self.pending {
            let source = actor
                .staff_context
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("Refresh protected access first."))?;
            source.validate(actor)?;
            request.validate(source)?;
            return Ok(request.clone());
        }
        let context = self
            .context
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Refresh staff assignments first."))?;
        context.validate(actor)?;
        let draft = self
            .draft
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Open a staff assignment first."))?;
        ensure!(
            context.accounts.iter().any(|a| a == &draft.source),
            "The saved assignment changed. Cancel this draft and reopen the current account before saving."
        );
        let roles = if draft.roles.is_empty() {
            StaffRoles::default()
        } else {
            StaffRoles::new(draft.roles.clone())?
        };
        let request = context.prepare(
            actor,
            draft.source.user_id,
            roles,
            draft.active,
            draft.reason.trim().into(),
            Uuid::new_v4(),
        )?;
        self.pending = Some(request.clone());
        Ok(request)
    }
    pub fn accept(&mut self, actor: &Access, update: Update) -> Result<()> {
        update.context.validate(actor)?;
        if let Some(receipt) = update.receipt {
            let pending = self.pending.as_ref().ok_or_else(|| {
                anyhow::anyhow!("A staff receipt arrived without its reviewed request.")
            })?;
            let draft = self
                .draft
                .as_ref()
                .ok_or_else(|| anyhow::anyhow!("The original staff draft is missing."))?;
            receipt.validate(actor, pending, &draft.source)?;
            self.pending = None;
            self.draft = None;
        }
        self.context = Some(update.context);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        access::Role,
        staff_roles::{Context, admin::Receipt},
    };
    use chrono::Utc;
    pub(super) fn fixture() -> (Access, State) {
        let mut a = Access {
            company_id: Uuid::from_u128(1),
            user_id: Uuid::from_u128(2),
            employee_id: Some(Uuid::from_u128(3)),
            role: Role::Administrator,
            staff_contract_version: 1,
            ..Default::default()
        };
        a.bind_staff_context(Context {
            schema_version: 1,
            company_id: a.company_id,
            user_id: a.user_id,
            employee_id: a.employee_id.unwrap(),
            base_role: a.role,
            revision: 1,
            runtime_ready: true,
            roles: StaffRoles::legacy(Role::Administrator),
        })
        .unwrap();
        let account = |u, e, r| Account {
            user_id: Uuid::from_u128(u),
            employee_id: Uuid::from_u128(e),
            employee_code: format!("EMP-{e}"),
            full_name: format!("Fictional {e}"),
            active: true,
            base_role: r,
            member_updated_at: Utc::now(),
            revision: 1,
            roles: StaffRoles::legacy(r),
        };
        let c = AdminContext {
            schema_version: 1,
            company_id: a.company_id,
            user_id: a.user_id,
            runtime_ready: true,
            account_count: 2,
            accounts: vec![
                account(2, 3, Role::Administrator),
                account(4, 5, Role::Employee),
            ],
        };
        (
            a,
            State {
                context: Some(c),
                ..Default::default()
            },
        )
    }
    #[test]
    fn uncertain_retry_keeps_identity_roles_active_and_reason() {
        let (a, mut s) = fixture();
        s.open(&a, Uuid::from_u128(4)).unwrap();
        let d = s.draft.as_mut().unwrap();
        d.roles = vec![StaffRole::LearningPerformanceOfficer];
        d.active = false;
        d.reason = "Reviewed access change".into();
        let first = s.submit(&a).unwrap();
        s.draft.as_mut().unwrap().reason = "Accidental later edit".into();
        assert_eq!(s.submit(&a).unwrap(), first);
        assert!(s.cancel().is_err());
        assert!(s.open(&a, Uuid::from_u128(4)).is_err());
    }
    #[test]
    fn refresh_preserves_draft_and_rejects_stale_first_submission() {
        let (a, mut s) = fixture();
        s.open(&a, Uuid::from_u128(4)).unwrap();
        s.draft.as_mut().unwrap().reason = "Reviewed change".into();
        let mut c = s.context.clone().unwrap();
        c.accounts[1].revision += 1;
        s.accept(
            &a,
            Update {
                context: c,
                receipt: None,
            },
        )
        .unwrap();
        assert!(s.has_draft());
        assert!(s.submit(&a).is_err());
        assert!(s.pending.is_none());
        s.cancel().unwrap();
        s.open(&a, Uuid::from_u128(4)).unwrap();
        assert_eq!(s.draft.unwrap().source.revision, 2);
    }
    #[test]
    fn only_matching_receipt_clears_draft_and_pending() {
        let (a, mut s) = fixture();
        s.open(&a, Uuid::from_u128(4)).unwrap();
        s.draft.as_mut().unwrap().reason = "Reviewed change".into();
        let p = s.submit(&a).unwrap();
        let source = s.draft.as_ref().unwrap().source.clone();
        let receipt = Receipt {
            schema_version: 1,
            id: p.id,
            company_id: a.company_id,
            actor_id: a.user_id,
            user_id: p.user_id,
            employee_id: source.employee_id,
            active: p.active,
            roles: p.roles.clone(),
            revision: 2,
            base_role: p.roles.compatible_base_role(),
            member_updated_at: source.member_updated_at,
        };
        let mut wrong = receipt.clone();
        wrong.active = !p.active;
        assert!(
            s.accept(
                &a,
                Update {
                    context: s.context.clone().unwrap(),
                    receipt: Some(wrong)
                }
            )
            .is_err()
        );
        assert!(s.has_draft());
        let mut current = s.context.clone().unwrap();
        current.accounts[1].revision = 2;
        s.accept(
            &a,
            Update {
                context: current,
                receipt: Some(receipt),
            },
        )
        .unwrap();
        assert!(!s.has_draft());
        assert_eq!(s.context.unwrap().accounts[1].revision, 2);
    }
    #[test]
    fn self_foreign_incomplete_and_unready_context_never_open() {
        let (a, mut s) = fixture();
        assert!(s.open(&a, a.user_id).is_err());
        assert!(s.open(&a, Uuid::from_u128(99)).is_err());
        s.context.as_mut().unwrap().runtime_ready = false;
        assert!(s.open(&a, Uuid::from_u128(4)).is_err());
        s.context.as_mut().unwrap().runtime_ready = true;
        s.context.as_mut().unwrap().company_id = Uuid::from_u128(99);
        assert!(s.open(&a, Uuid::from_u128(4)).is_err());
    }
}

#[cfg(test)]
mod rejection_tests {
    use super::*;
    #[test]
    fn confirmed_rejection_preserves_review_without_locking_a_definitely_unsaved_retry() {
        let (a, mut state) = super::tests::fixture();
        state.open(&a, Uuid::from_u128(4)).unwrap();
        state.draft.as_mut().unwrap().reason = "Reviewed reason".into();
        let original = state.submit(&a).unwrap();
        state.reject();
        assert!(state.pending.is_none());
        assert_eq!(state.draft.as_ref().unwrap().reason, original.reason);
        assert!(state.submit(&a).is_err());
        state.cancel().unwrap();
        state.open(&a, original.user_id).unwrap();
        assert!(!state.rejected);
    }
}
