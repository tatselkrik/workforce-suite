use super::*;
use crate::recruiting::{self, Attachment, ResumeAction, Setup};
impl CloudClient {
    pub(super) fn exits_snapshot(&mut self, access: &Access) -> Result<crate::exits::Setup> {
        let mut setup = crate::exits::Setup::default();
        let company = Some(access.company_id);
        if access.role.manages_hr() {
            setup.cases = self.rows("exit_cases", company)?;
            setup.readiness = decode(
                self.request(Method::POST, "rpc/exit_readiness_list")?
                    .json(&json!({}))
                    .send()?,
            )?;
            ensure!(
                setup
                    .readiness
                    .iter()
                    .all(|r| r.company_id == access.company_id
                        && setup.cases.iter().any(|c| c.id == r.case_id)),
                "Exit readiness response is outside the current scope."
            );
        }
        setup.tasks = self.rows("clearance_tasks", company)?;
        setup.events = self.rows("exit_events", company)?;
        Ok(setup)
    }
    pub(super) fn onboarding_snapshot(
        &mut self,
        access: &Access,
    ) -> Result<crate::onboarding::Setup> {
        let mut setup = crate::onboarding::Setup::default();
        let company = Some(access.company_id);
        if access.role.manages_hr() {
            setup.offers = self.rows("hiring_offers", company)?;
            setup.offer_events = self.rows("hiring_offer_events", company)?;
            setup.conversions = self.rows("hire_conversions", company)?;
            setup.cases = self.rows("onboarding_cases", company)?;
        }
        setup.tasks = self.rows("onboarding_tasks", company)?;
        setup.events = self.rows("onboarding_events", company)?;
        Ok(setup)
    }

    pub(super) fn recruiting_snapshot(&mut self, access: &Access) -> Result<Setup> {
        let company = access.company_id;
        let hr = access.role.manages_hr();
        let interviewers = if hr {
            decode(
                self.request(Method::POST, "rpc/recruiting_interviewers")?
                    .json(&json!({}))
                    .send()?,
            )?
        } else {
            Vec::new()
        };
        Ok(Setup {
            documents: if hr {
                self.rows("applicant_documents", Some(company))?
            } else {
                Vec::new()
            },
            document_events: if hr {
                self.rows("applicant_document_events", Some(company))?
            } else {
                Vec::new()
            },
            interviews: self.rows("recruiting_interviews", Some(company))?,
            feedback: self.rows("interview_feedback", Some(company))?,
            events: self.rows("interview_events", Some(company))?,
            interviewers,
        })
    }
    fn resume_write(&mut self, operation: &str, payload: Value) -> Result<Attachment> {
        decode(
            self.request(Method::POST, "rpc/save_applicant_document")?
                .json(&json!({"p_operation":operation,"p_document":payload}))
                .send()?,
        )
    }
    pub fn execute_resume(
        &mut self,
        action: ResumeAction,
        current: &Snapshot,
    ) -> Result<(Snapshot, String)> {
        ensure!(
            current.access.role.manages_hr(),
            "Only HR and Administrators can manage application attachments."
        );
        let message = match action {
            ResumeAction::Upload(d) => {
                let prepared = d.prepared(current)?;
                let saved = self.resume_write("reserve", prepared.payload.clone())?;
                saved.validate()?;
                ensure!(
                    saved.id == d.document.id
                        && saved.company_id == current.company.id
                        && saved.applicant_id == d.document.employee_id
                        && saved.details == d.document.details()?
                        && saved.byte_size == prepared.bytes.len()
                        && saved.mime_type == prepared.mime
                        && saved.sha256 == prepared.payload["sha256"].as_str().unwrap_or(""),
                    "The upload reservation did not match the selected attachment. No bytes were sent."
                );
                if saved.state == DocumentState::Pending {
                    let uploaded = self
                        .account_request(
                            Method::POST,
                            &format!(
                                "/storage/v1/object/applicant-documents/{}",
                                saved.object_name
                            ),
                        )?
                        .header("Content-Type", prepared.mime)
                        .header("Cache-Control", "max-age=0")
                        .body(prepared.bytes)
                        .send();
                    drop(uploaded);
                    let filed=self.resume_write("finalize",json!({"id":saved.id})).context("Upload not confirmed. Refresh, retry with the exact same file or discard the incomplete upload.")?;
                    ensure!(
                        filed.state == DocumentState::Ready
                            && filed.id == saved.id
                            && filed.company_id == saved.company_id
                            && filed.applicant_id == saved.applicant_id,
                        "The filed attachment response is invalid."
                    );
                } else {
                    ensure!(
                        saved.state == DocumentState::Ready,
                        "This upload was discarded. Start a new upload."
                    );
                }
                "Application attachment filed. Original bytes and history are retained."
            }
            ResumeAction::Edit(d) => {
                let old = recruiting::attachment(current, d.document.id)?;
                ensure!(
                    old.applicant_id == d.document.employee_id && d.document.original.is_some(),
                    "Reopen the saved attachment first."
                );
                self.resume_write("edit", d.payload()?)?;
                "Attachment details saved."
            }
            ResumeAction::Archive {
                id,
                revision,
                archived,
            } => {
                recruiting::attachment(current, id)?;
                self.resume_write(
                    if archived { "archive" } else { "restore" },
                    json!({"id":id,"revision":revision}),
                )?;
                if archived {
                    "Attachment archived; original file retained."
                } else {
                    "Attachment restored."
                }
            }
            ResumeAction::Discard { id } => {
                recruiting::attachment(current, id)?;
                let saved = self.resume_write("discard", json!({"id":id}))?;
                saved.validate()?;
                ensure!(
                    saved.id == id
                        && saved.company_id == current.company.id
                        && saved.state == DocumentState::Discarded,
                    "Invalid discarded attachment response."
                );
                let _: Value = decode(
                    self.account_request(Method::DELETE, "/storage/v1/object/applicant-documents")?
                        .json(&json!({"prefixes":[saved.object_name]}))
                        .send()?,
                )
                .context("Upload discarded. Retry cleanup if unused file removal failed.")?;
                "Incomplete upload discarded and unused bytes cleaned up."
            }
            ResumeAction::Download { id, destination } => {
                let r = recruiting::attachment(current, id)?;
                r.validate()?;
                ensure!(
                    r.state == DocumentState::Ready,
                    "This attachment is not yet filed."
                );
                let receipt = r.file_record()?;
                let mut response = self
                    .account_request(
                        Method::GET,
                        &format!(
                            "/storage/v1/object/authenticated/applicant-documents/{}",
                            r.object_name
                        ),
                    )?
                    .query(&[("v", Uuid::new_v4().to_string())])
                    .header("Cache-Control", "no-cache")
                    .send()?;
                if !response.status().is_success() {
                    let _: Value = decode(response)?;
                    unreachable!()
                };
                let mut bytes = Vec::new();
                response
                    .by_ref()
                    .take(documents::MAX_FILE_BYTES as u64 + 1)
                    .read_to_end(&mut bytes)?;
                documents::save_download(&receipt, &bytes, &destination)?;
                "Application attachment downloaded to your selected file."
            }
        };
        Ok((self.snapshot().context("Attachment operation completed, but refresh failed. Keep the draft and Refresh before retrying.")?,message.into()))
    }
}
