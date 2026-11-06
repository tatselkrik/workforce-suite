use super::*;
fn decode_save(response: Response) -> Result<Value> {
    if matches!(
        response.status(),
        StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN
    ) {
        return Err(AccessRevoked.into());
    }
    if matches!(
        response.status(),
        StatusCode::BAD_REQUEST | StatusCode::CONFLICT | StatusCode::UNPROCESSABLE_ENTITY
    ) {
        let error = decode::<Value>(response).unwrap_err();
        return Err(crate::work_activity::Rejected(error.to_string()).into());
    }
    decode(response)
}
impl CloudClient {
    pub fn work_report_follow_ups(
        &mut self,
        current: &Snapshot,
        work_date: NaiveDate,
    ) -> Result<crate::work_activity::ReportFollowUps> {
        let access = self.current_access()?;
        if access != current.access {
            return Err(AccessRevoked.into());
        }
        let mut follow_ups: crate::work_activity::ReportFollowUps = decode(
            self.request(Method::POST, "rpc/work_report_follow_ups")?
                .json(&json!({"p_work_date": work_date}))
                .send()?,
        )?;
        if self.current_access()? != access {
            return Err(AccessRevoked.into());
        }
        follow_ups.validate(current, work_date)?;
        follow_ups
            .rows
            .sort_by_cached_key(|row| current.employee_name(row.employee_id).to_lowercase());
        Ok(follow_ups)
    }

    pub fn station_operation(&mut self, request: &Value) -> Result<Value> {
        ensure!(
            self.product != Some(crate::desktop::Product::Payroll),
            "Attendance stations belong in Workforce HR."
        );
        let access = self.current_access()?;
        ensure!(
            access.allows(crate::staff_roles::Capability::AccountAdministration)
                || (request["operation"] == "pin_reset"
                    && access.allows(crate::staff_roles::Capability::AttendanceAdministration)),
            "Station administration is not permitted."
        );
        let value = decode_save(
            self.request(Method::POST, "rpc/attendance_station")?
                .json(&json!({"p_request":request}))
                .send()?,
        )?;
        if self.current_access()? != access {
            return Err(AccessRevoked.into());
        }
        Ok(value)
    }
    pub fn work_activity(
        &mut self,
        current: &Snapshot,
        request: Option<Value>,
    ) -> Result<crate::work_activity::Context> {
        ensure!(
            self.product != Some(crate::desktop::Product::Payroll),
            "Work reporting belongs in Workforce HR."
        );
        let access = self.current_access()?;
        if access != current.access {
            return Err(AccessRevoked.into());
        }
        let mut receipt = None;
        let mut evidence_id = None;
        if let Some(request) = request {
            ensure!(
                serde_json::to_vec(&request)?.len() <= 60000,
                "Work request is too large."
            );
            if request["operation"] == "evidence" {
                evidence_id = Some(serde_json::from_value::<Uuid>(request["id"].clone())?);
            } else if request["operation"] == "evidence_cleanup" {
                self.clean_attendance_evidence()?;
            } else if let Some(operation) = request["operation"]
                .as_str()
                .and_then(|v| v.strip_prefix("station_"))
            {
                let mut station = request.clone();
                station["operation"] = json!(operation);
                self.station_operation(&station)?;
            } else {
                let result = decode_save(
                    self.request(Method::POST, "rpc/work_activity_change")?
                        .json(&json!({"p_request":request}))
                        .send()
                        .context("Save delivery is uncertain. Retry the same work request.")?,
                )?;
                ensure!(
                    result["id"] == request["id"],
                    "The saved work receipt does not match this request."
                );
            }
            receipt = Some(json!({"request_id":request["request_id"],"id":request["id"]}));
        }
        let mut context: crate::work_activity::Context = decode(
            self.request(Method::POST, "rpc/work_activity_context")?
                .json(&json!({"p_own":false}))
                .send()?,
        )?;
        if access.allows(crate::staff_roles::Capability::AccountAdministration) {
            context.stations =
                serde_json::from_value(self.station_operation(&json!({"operation":"list"}))?)?;
        }
        if let Some(id) = evidence_id {
            context.images = self.attendance_evidence(current, id)?;
        }
        context.receipt = receipt;
        context.validate(current)?;
        if self.current_access()? != access {
            return Err(AccessRevoked.into());
        }
        Ok(context)
    }
    fn clean_attendance_evidence(&mut self) -> Result<()> {
        let access = self.current_access()?;
        ensure!(
            access.allows(crate::staff_roles::Capability::AttendanceAdministration),
            "Photo cleanup requires HR attendance access."
        );
        let candidates: Vec<Value> = decode(
            self.request(Method::POST, "rpc/attendance_evidence_retention")?
                .json(&json!({"p_ids":null}))
                .send()?,
        )?;
        ensure!(candidates.len() <= 100, "Too many cleanup candidates.");
        let mut ids = vec![];
        let mut paths = vec![];
        for v in candidates {
            let id: Uuid = serde_json::from_value(v["id"].clone())?;
            let path = v["object_path"].as_str().unwrap_or("");
            ensure!(
                path == format!("{}/{}.jpg", access.company_id, id),
                "Invalid retained photo path."
            );
            ids.push(id);
            paths.push(path.to_string());
        }
        if !paths.is_empty() {
            if self.current_access()? != access {
                return Err(AccessRevoked.into());
            }
            let response = self
                .http
                .delete(format!(
                    "{}/storage/v1/object/attendance-evidence",
                    self.config.url.trim_end_matches('/')
                ))
                .header("apikey", &self.config.publishable_key)
                .bearer_auth(&self.session.access_token)
                .json(&json!({"prefixes":paths}))
                .send()?;
            ensure!(
                response.status().is_success(),
                "Expired-photo cleanup could not be confirmed."
            );
            let _: Value = decode(
                self.request(Method::POST, "rpc/attendance_evidence_retention")?
                    .json(&json!({"p_ids":ids}))
                    .send()?,
            )?;
        }
        Ok(())
    }
    fn attendance_evidence(
        &mut self,
        current: &Snapshot,
        record: Uuid,
    ) -> Result<Vec<crate::work_activity::EvidenceImage>> {
        ensure!(
            current
                .access
                .allows(crate::staff_roles::Capability::AttendanceAdministration),
            "Attendance photos are HR-private."
        );
        let rows: Vec<Value> = decode(
            self.request(Method::POST, "rpc/attendance_evidence_context")?
                .json(&json!({"p_id":record}))
                .send()?,
        )?;
        ensure!(rows.len() <= 32, "Too many photo entries for one record.");
        let mut images = vec![];
        for row in rows {
            let id: Uuid = serde_json::from_value(row["id"].clone())?;
            let employee: Uuid = serde_json::from_value(row["employee_id"].clone())?;
            ensure!(
                row["company_id"] == current.company.id.to_string()
                    && row["attendance_id"] == record.to_string()
                    && current
                        .attendance
                        .iter()
                        .any(|a| a.id == record && a.employee_id == employee),
                "Photo scope changed."
            );
            if !row["purged_at"].is_null() {
                continue;
            }
            let path = row["object_path"].as_str().unwrap_or("");
            ensure!(
                path == format!("{}/{}.jpg", current.company.id, id),
                "Invalid camera evidence path."
            );
            let response = self
                .http
                .get(format!(
                    "{}/storage/v1/object/authenticated/attendance-evidence/{path}",
                    self.config.url.trim_end_matches('/')
                ))
                .header("apikey", &self.config.publishable_key)
                .bearer_auth(&self.session.access_token)
                .send()?;
            ensure!(
                response.status().is_success(),
                "Attendance photo is unavailable."
            );
            let mut bytes = vec![];
            response.take(65537).read_to_end(&mut bytes)?;
            ensure!(
                bytes.len() <= 65536 && row["photo_hash"] == crate::updater::digest(&bytes),
                "Camera evidence did not match its recorded hash."
            );
            let dimensions = image::ImageReader::with_format(
                std::io::Cursor::new(&bytes),
                image::ImageFormat::Jpeg,
            )
            .into_dimensions()?;
            ensure!(
                (80..=640).contains(&dimensions.0) && (80..=640).contains(&dimensions.1),
                "Invalid camera evidence dimensions."
            );
            images.push(crate::work_activity::EvidenceImage {
                id,
                employee_id: employee,
                attendance_id: record,
                operation: row["operation"].as_str().unwrap_or("").into(),
                recorded_at: row["recorded_at"].as_str().unwrap_or("").into(),
                station: row["station"].as_str().unwrap_or("").into(),
                bytes,
            });
        }
        Ok(images)
    }
}
