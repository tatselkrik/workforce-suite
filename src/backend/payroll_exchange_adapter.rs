use super::*;
use crate::payroll_exchange as x;
impl CloudClient {
    pub fn payroll_exchange(
        &mut self,
        current: &Snapshot,
        operation: &x::Operation,
    ) -> Result<x::Update> {
        x::authorize(current)?;
        let before = self.current_access()?;
        if before != current.access {
            return Err(AccessRevoked.into());
        }
        let mut update = x::Update::default();
        let mut output = None;
        match operation {
            x::Operation::State => {}
            x::Operation::SaveProfile(change) => {
                change.validate()?;
                ensure!(
                    before.role == crate::access::Role::Administrator,
                    "Only Administrators configure profiles."
                );
                let state: x::State = decode(
                    self.request(Method::POST, "rpc/save_payroll_exchange_profile")?
                        .json(&json!({"p_change":change}))
                        .send()?,
                )?;
                state.validate(&before)?;
                update.state = Some(state);
                update.profile_saved = Some(change.request_id);
            }
            x::Operation::Preview(request) => {
                request.validate()?;
                let preview: x::Preview = decode(
                    self.request(Method::POST, "rpc/preview_payroll_exchange")?
                        .json(&json!({"p_request":request}))
                        .send()?,
                )?;
                preview.validate(&before, request)?;
                update.preview = Some(preview);
            }
            x::Operation::Prepare {
                request,
                fingerprint,
            } => {
                request.validate()?;
                let detail: x::Detail = decode(
                    self.request(Method::POST, "rpc/prepare_payroll_exchange")?
                        .json(&json!({"p_request":request,"p_fingerprint":fingerprint}))
                        .send()?,
                )?;
                detail.validate(&before, request.id)?;
                detail.basis.matches(request)?;
                ensure!(
                    detail.basis.fingerprint == *fingerprint,
                    "Prepared fingerprint differs from the preview."
                );
                update.detail = Some(detail);
            }
            x::Operation::Detail { id, .. } | x::Operation::Template { id, .. } => {
                let detail: x::Detail = decode(
                    self.request(Method::POST, "rpc/payroll_exchange_detail")?
                        .json(&json!({"p_id":id}))
                        .send()?,
                )?;
                detail.validate(&before, *id)?;
                match operation {
                    x::Operation::Detail {
                        destination: Some(path),
                        ..
                    } => output = Some((path, detail.basis.csv.as_bytes().to_vec())),
                    x::Operation::Template { destination, .. } => {
                        output = Some((destination, detail.template()?))
                    }
                    _ => {}
                }
                update.detail = Some(detail);
            }
            x::Operation::Resolve { kind, id, payload } => {
                let receipt: x::Attempt = decode(
                    self.request(Method::POST, "rpc/payroll_exchange_attempt")?
                        .json(&json!({"p_kind":kind,"p_id":id,"p_payload":payload}))
                        .send()?,
                )?;
                receipt.validate(&before, kind, *id)?;
                if receipt.committed && kind != "profile" {
                    let exchange_id = if kind == "prepare" {
                        *id
                    } else {
                        serde_json::from_value(payload["exchange_id"].clone())?
                    };
                    let detail: x::Detail = decode(
                        self.request(Method::POST, "rpc/payroll_exchange_detail")?
                            .json(&json!({"p_id":exchange_id}))
                            .send()?,
                    )?;
                    detail.validate(&before, exchange_id)?;
                    update.detail = Some(detail);
                }
                update.resolved = Some(receipt);
            }
            x::Operation::Acknowledge(batch) => {
                // A retry may follow an already-committed acknowledgement. Validate its
                // shape against captured facts locally; the server owns current revisions.
                ensure!(
                    !batch.id.is_nil()
                        && !batch.exchange_id.is_nil()
                        && !batch.rows.is_empty()
                        && batch.rows.len() <= 1000,
                    "Invalid acknowledgement identity/size."
                );
                let detail: x::Detail = decode(
                    self.request(Method::POST, "rpc/ack_payroll_exchange")?
                        .json(&json!({"p_batch":batch}))
                        .send()?,
                )?;
                detail.validate(&before, batch.exchange_id)?;
                for r in &batch.rows {
                    ensure!(
                        detail.events.iter().any(|e| e.batch_id == batch.id
                            && e.record_key == r.record_key
                            && e.revision == r.expected_revision + 1
                            && e.status == r.status
                            && e.reference == r.reference
                            && e.note == r.note),
                        "Acknowledgement receipt differs from the submitted rows."
                    );
                }
                update.detail = Some(detail);
            }
        }
        if update.state.is_none() {
            let state: x::State = decode(
                self.request(Method::POST, "rpc/payroll_exchange_state")?
                    .json(&json!({}))
                    .send()?,
            )?;
            state.validate(&before)?;
            update.state = Some(state);
        }
        if self.current_access()? != before {
            return Err(AccessRevoked.into());
        }
        if let Some((path, bytes)) = output {
            crate::reports::save_csv(path, &bytes)?;
        }
        Ok(update)
    }
}
