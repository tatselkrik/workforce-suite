mod recruiting_adapter;
mod performance_adapter;
mod learning_adapter;
mod career_adapter;
mod payroll_adapter;
mod payroll_exchange_adapter;
mod staff_roles_adapter;
mod staff_provision_adapter;
#[cfg(test)]
mod staff_provision_tests;
#[cfg(test)]
mod government_export_tests;
#[cfg(test)]
mod bir_file_tests;
#[cfg(test)]
mod bir_builder_tests;
#[cfg(test)]
mod government_annual_tests;
#[cfg(test)]
mod bir_identity_tests;
#[cfg(test)]
mod staff_roles_tests;
use crate::access::{Access, AccessEvent, AdminAction, ManagedAccount};
use crate::account::{self, AccountAction, AccountProfile, DisplayPreferences, PersonalDetails};
use crate::documents::{self, DocumentAction, DocumentState, EmployeeDocument};
use crate::domain::*;
use crate::updater::{self, SignedRelease, UpdateConfig, UpdateState, VerifiedRelease};
use anyhow::{Context, Result, bail, ensure};
use chrono::Utc;
use reqwest::{
    Method, StatusCode,
    blocking::{Client, RequestBuilder, Response},
};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::{Value, json};
use std::{io::Read, path::Path, sync::mpsc, thread, time::Duration};
use uuid::Uuid;

#[derive(Clone, Default, Serialize, Deserialize)]
pub struct ConnectionConfig {
    pub url: String,
    pub publishable_key: String,
}

impl ConnectionConfig {
    pub fn validate(&self) -> Result<()> {
        let url =
            reqwest::Url::parse(self.url.trim()).context("Enter a valid Supabase project URL.")?;
        let local = matches!(url.host_str(), Some("localhost" | "127.0.0.1" | "[::1]"));
        ensure!(
            url.scheme() == "https" || (url.scheme() == "http" && local),
            "Use HTTPS, except for local Supabase development."
        );
        ensure!(
            url.host_str().is_some()
                && url.username().is_empty()
                && url.password().is_none()
                && url.query().is_none()
                && url.fragment().is_none()
                && url.path() == "/",
            "Use only the project's base URL, without a path or credentials."
        );
        ensure!(
            self.publishable_key.trim().starts_with("sb_publishable_")
                && self.publishable_key.trim().len() >= 20,
            "Use a Supabase publishable key (sb_publishable_...). Secret and service-role keys are not accepted."
        );
        Ok(())
    }

    pub fn load() -> Result<Self> {
        let exe = std::env::current_exe().context("Could not locate the HRIS installation.")?;
        let folder = exe
            .parent()
            .context("Could not locate the HRIS installation folder.")?;
        Self::load_profile(
            &folder.join("hris-connection.json"),
            folder.join(".hris-installation").exists(),
        )
    }

    fn load_profile(path: &Path, installed: bool) -> Result<Self> {
        // A deployment selects its company database. Device settings and environment
        // variables must never silently redirect a company's installed application.
        if !path.exists() {
            ensure!(
                !installed,
                "This installation's company connection is missing. Contact your HRIS administrator."
            );
            return Ok(Self::default());
        }
        let config: Self = serde_json::from_slice(
            &std::fs::read(path).context("Could not read the installed company connection.")?,
        )
        .context("The installed company connection is invalid. Contact your HRIS administrator.")?;
        config.validate().context(
            "The installed company connection is invalid. Contact your HRIS administrator.",
        )?;
        Ok(config)
    }
}

#[derive(Clone, Deserialize)]
struct AuthUser {
    id: Uuid,
    #[serde(default)]
    email: String,
    #[serde(default)]
    new_email: String,
    #[serde(default)]
    email_confirmed_at: Option<String>,
    #[serde(default)]
    created_at: Option<String>,
    #[serde(default)]
    last_sign_in_at: Option<String>,
    #[serde(default)]
    user_metadata: Value,
}

impl AuthUser {
    fn profile(&self) -> AccountProfile {
        let details = self
            .user_metadata
            .get("hris_profile")
            .cloned()
            .and_then(|v| serde_json::from_value::<PersonalDetails>(v).ok())
            .unwrap_or_default();
        let display = self
            .user_metadata
            .get("hris_preferences")
            .cloned()
            .and_then(|v| serde_json::from_value::<DisplayPreferences>(v).ok())
            .filter(|v| v.validate().is_ok())
            .unwrap_or_default();
        AccountProfile {
            id: self.id,
            email: self.email.clone(),
            pending_email: self.new_email.clone(),
            email_confirmed_at: self.email_confirmed_at.clone(),
            created_at: self.created_at.clone(),
            last_sign_in_at: self.last_sign_in_at.clone(),
            details,
            display,
            report_views: crate::report_views::from_metadata(
                self.user_metadata.get("hris_report_views"),
            ),
            ..Default::default()
        }
    }
}

#[derive(Deserialize)]
struct AuthSession {
    access_token: String,
    refresh_token: String,
    #[serde(default)]
    expires_at: i64,
    #[serde(default = "default_expiry")]
    expires_in: i64,
    user: AuthUser,
}

#[derive(Deserialize)]
struct ReportPreferences {
    user_id: Uuid,
    company_id: Uuid,
    views: Value,
    revision: i64,
}

fn default_expiry() -> i64 {
    3600
}

#[cfg(test)]
mod connection_file_tests {
    use super::*;

    #[test]
    fn company_profile_is_required_for_an_installed_app_and_invalid_profiles_fail_closed() {
        let folder = std::env::temp_dir().join(format!("hris-config-test-{}", Uuid::new_v4()));
        std::fs::create_dir(&folder).unwrap();
        let company = folder.join("hris-connection.json");
        let device = folder.join("hris-config.json");
        let configured = ConnectionConfig {
            url: "https://company.example.invalid".into(),
            publishable_key: "sb_publishable_company_test_only".into(),
        };
        std::fs::write(&company, serde_json::to_vec(&configured).unwrap()).unwrap();
        std::fs::write(&device, br#"{"url":"https://another-company.example.invalid","publishable_key":"sb_publishable_previous_test_only"}"#).unwrap();
        assert_eq!(
            ConnectionConfig::load_profile(&company, true).unwrap().url,
            configured.url
        );
        std::fs::write(&company, b"invalid JSON").unwrap();
        assert!(ConnectionConfig::load_profile(&company, true).is_err());
        std::fs::remove_file(&company).unwrap();
        assert!(ConnectionConfig::load_profile(&company, true).is_err());
        assert!(
            ConnectionConfig::load_profile(&company, false)
                .unwrap()
                .url
                .is_empty()
        );
        std::fs::remove_file(&device).unwrap();
        std::fs::remove_dir(&folder).unwrap();
    }
}

pub struct CloudClient {
    config: ConnectionConfig,
    http: Client,
    session: AuthSession,
    account: AccountProfile,
    logo_cache: Option<(Uuid, Vec<u8>)>,
}

#[derive(Debug)]
struct AccessRevoked;
impl std::fmt::Display for AccessRevoked {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(
            "Your HRIS access has changed or expired. Sign in again or contact your administrator.",
        )
    }
}
impl std::error::Error for AccessRevoked {}

#[derive(Debug)]
struct PortalAccount;
impl std::fmt::Display for PortalAccount {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(crate::access::EMPLOYEE_PORTAL_NOTICE)
    }
}
impl std::error::Error for PortalAccount {}

impl CloudClient {
    fn company_logo(&mut self, company: &Company) -> (Option<Vec<u8>>, Option<String>) {
        let Some(version) = company.logo_version else {
            self.logo_cache = None;
            return (None, None);
        };
        if let Some((cached, bytes)) = &self.logo_cache {
            if *cached == version {
                return (Some(bytes.clone()), None);
            }
        }
        let fetched = (|| {
            let mut response = self
                .account_request(
                    Method::GET,
                    &format!(
                        "/storage/v1/object/authenticated/company-branding/{}/logo.png",
                        company.id
                    ),
                )?
                .header("Cache-Control", "no-cache")
                .send()?;
            if !response.status().is_success() {
                let _: Value = decode(response)?;
                unreachable!();
            }
            let mut png = Vec::new();
            response
                .by_ref()
                .take(2 * 1024 * 1024 + 1)
                .read_to_end(&mut png)?;
            ensure!(
                png.len() <= 2 * 1024 * 1024,
                "The company logo is too large."
            );
            let png = crate::branding::normalize_logo(&png)?;
            Ok::<_, anyhow::Error>(png)
        })();
        match fetched {
            Ok(png) => {
                self.logo_cache = Some((version, png.clone()));
                (Some(png), None)
            }
            Err(_) => (
                None,
                Some("The company logo could not be loaded. Refresh to retry.".into()),
            ),
        }
    }

    pub fn check_update(&mut self, config: &UpdateConfig) -> Result<UpdateState> {
        let mut response = self
            .request(Method::POST, "rpc/desktop_update")?
            .json(&json!({}))
            .send()
            .context(
                "Could not check for updates. Check your internet connection and try again.",
            )?;
        if !response.status().is_success() {
            let _: Value = decode(response)?;
            unreachable!();
        }
        let mut bytes = Vec::new();
        response.by_ref().take(16001).read_to_end(&mut bytes)?;
        ensure!(
            bytes.len() <= 16000,
            "The update service returned an oversized response."
        );
        #[derive(Deserialize)]
        struct Feed {
            schema_version: String,
            release: Option<SignedRelease>,
        }
        let feed: Feed =
            serde_json::from_slice(&bytes).context("The update response was invalid.")?;
        let signed = feed.release;
        let Some(signed) = signed else {
            return Ok(UpdateState::UpToDate);
        };
        let verified = signed.verify(config)?;
        ensure!(
            verified.release.requires_schema <= feed.schema_version,
            "This update needs a database update first."
        );
        if updater::Version::parse(&verified.release.version)?
            > updater::Version::parse(updater::VERSION)?
        {
            Ok(UpdateState::Available(verified))
        } else {
            Ok(UpdateState::UpToDate)
        }
    }

    fn install_update(&mut self, update: &VerifiedRelease) -> Result<()> {
        let config = UpdateConfig::load()?
            .context("The installed update verification profile is missing.")?;
        let update = update.signed.verify(&config)?;
        ensure!(
            updater::Version::parse(&update.release.version)?
                > updater::Version::parse(updater::VERSION)?,
            "This update is already installed."
        );
        let mut response = self
            .account_request(
                Method::GET,
                &format!(
                    "/storage/v1/object/authenticated/desktop-updates/{}",
                    update.release.artifact
                ),
            )?
            .timeout(Duration::from_secs(180))
            .header("Cache-Control", "no-cache")
            .send()
            .context("The update could not be downloaded. The current app was kept open.")?;
        if !response.status().is_success() {
            let _: Value = decode(response)?;
            unreachable!();
        }
        if let Some(length) = response.content_length() {
            ensure!(
                length == update.release.size,
                "The update download size did not match."
            );
        }
        let mut bytes = Vec::new();
        response
            .by_ref()
            .take(updater::MAX_BYTES + 1)
            .read_to_end(&mut bytes)?;
        updater::verify_package(&update.release, &bytes)?;
        updater::stage_and_launch(&update, &bytes)?;
        let _ = self.sign_out_scope("local");
        Ok(())
    }

    pub fn connect(
        config: ConnectionConfig,
        email: String,
        password: String,
    ) -> Result<(Self, Snapshot)> {
        config.validate()?;
        ensure!(
            !email.trim().is_empty() && !password.is_empty(),
            "Enter your HRIS account email and password."
        );
        let http = Client::builder()
            .timeout(Duration::from_secs(20))
            .connect_timeout(Duration::from_secs(7))
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .context("Could not start the HTTPS client.")?;
        let response = http
            .post(format!(
                "{}/auth/v1/token",
                config.url.trim().trim_end_matches('/')
            ))
            .query(&[("grant_type", "password")])
            .header("apikey", config.publishable_key.trim())
            .json(&json!({"email": email.trim(), "password": password}))
            .send()
            .context(
                "Cannot reach your company's HRIS. Check your internet connection and try again.",
            )?;
        let mut session: AuthSession = decode(response)?;
        if session.expires_at == 0 {
            session.expires_at = Utc::now().timestamp() + session.expires_in;
        }
        let mut client = Self {
            config,
            http,
            account: session.user.profile(),
            session,
            logo_cache: None,
        };
        let snapshot = client.snapshot()?;
        client.refresh_account()?;
        Ok((client, snapshot))
    }

    fn ensure_session(&mut self) -> Result<()> {
        if self.session.expires_at > Utc::now().timestamp() + 60 {
            return Ok(());
        }
        let response = self
            .http
            .post(format!(
                "{}/auth/v1/token",
                self.config.url.trim().trim_end_matches('/')
            ))
            .query(&[("grant_type", "refresh_token")])
            .header("apikey", self.config.publishable_key.trim())
            .json(&json!({"refresh_token": self.session.refresh_token}))
            .send()
            .context("Session refresh failed. Sign in again.")?;
        if matches!(
            response.status(),
            StatusCode::BAD_REQUEST | StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN
        ) {
            return Err(AccessRevoked.into());
        }
        let mut session: AuthSession =
            decode(response).context("Session expired. Sign in again.")?;
        if session.expires_at == 0 {
            session.expires_at = Utc::now().timestamp() + session.expires_in;
        }
        self.session = session;
        Ok(())
    }

    fn request(&mut self, method: Method, table: &str) -> Result<RequestBuilder> {
        self.ensure_session()?;
        Ok(self
            .http
            .request(
                method,
                format!(
                    "{}/rest/v1/{table}",
                    self.config.url.trim().trim_end_matches('/')
                ),
            )
            .header("apikey", self.config.publishable_key.trim())
            .header("X-HRIS-Leave-Contract", "2")
            .header("X-HRIS-Leave-Credits", "1")
            .header("X-HRIS-Leave-Dated", "1")
            .bearer_auth(&self.session.access_token))
    }

    fn account_request(&mut self, method: Method, path: &str) -> Result<RequestBuilder> {
        self.ensure_session()?;
        Ok(self
            .http
            .request(
                method,
                format!("{}{path}", self.config.url.trim().trim_end_matches('/')),
            )
            .header("apikey", self.config.publishable_key.trim())
            .bearer_auth(&self.session.access_token))
    }

    pub fn account(&self) -> &AccountProfile {
        &self.account
    }

    pub fn refresh_account(&mut self) -> Result<()> {
        let user: AuthUser = decode(
            self.account_request(Method::GET, "/auth/v1/user")?
                .send()
                .context("Could not refresh your account. Check your connection.")?,
        )?;
        self.accept_user(user)?;
        match self.download_photo() {
            Ok(photo) => {
                self.account.photo = photo;
                self.account.photo_warning = None;
            }
            Err(error) => {
                self.account.photo_warning =
                    Some(format!("Your photo could not be loaded. {error}"))
            }
        }
        Ok(())
    }

    fn accept_user(&mut self, user: AuthUser) -> Result<()> {
        ensure!(
            user.id == self.session.user.id,
            "The account response did not match your session. Sign in again."
        );
        let mut account = user.profile();
        if self.account.report_views_company.is_some() {
            account.report_views = self.account.report_views.clone();
            account.report_views_company = self.account.report_views_company;
            account.report_views_revision = self.account.report_views_revision;
        }
        account.photo = self.account.photo.take();
        account.photo_warning = self.account.photo_warning.take();
        self.account = account;
        self.session.user = user;
        Ok(())
    }

    fn photo_path(&self) -> String {
        format!("{}/avatar.png", self.session.user.id)
    }

    fn download_photo(&mut self) -> Result<Option<Vec<u8>>> {
        let path = format!(
            "/storage/v1/object/authenticated/profile-photos/{}",
            self.photo_path()
        );
        let response = self
            .account_request(Method::GET, &path)?
            .header("Cache-Control", "no-cache")
            .send()
            .context("Check your connection and refresh.")?;
        if !response.status().is_success() {
            let status = response.status();
            let error: Value = response.json().unwrap_or(Value::Null);
            let code = error.get("code").and_then(Value::as_str).unwrap_or("");
            let message = error.get("message").and_then(Value::as_str).unwrap_or("");
            if matches!(code, "NoSuchKey" | "not_found")
                || (status == StatusCode::NOT_FOUND && message == "Object not found")
                || (error.get("error").and_then(Value::as_str) == Some("not_found")
                    && message == "Object not found")
            {
                return Ok(None);
            }
            bail!("Your photo storage is unavailable. Contact your HRIS administrator.");
        }
        let mut bytes = Vec::new();
        response
            .take(account::MAX_PHOTO_BYTES as u64 + 1)
            .read_to_end(&mut bytes)?;
        ensure!(
            bytes.len() <= account::MAX_PHOTO_BYTES,
            "The stored photo exceeds the size limit."
        );
        Ok(Some(account::normalize_photo(&bytes)?))
    }

    fn update_user(&mut self, body: Value) -> Result<()> {
        let response = self
            .account_request(Method::PUT, "/auth/v1/user")?
            .json(&body)
            .send()
            .context("Could not save your account change. Check your connection.")?;
        let user: AuthUser = decode(response)?;
        self.accept_user(user)
    }

    fn verify_current_password(&mut self, current: &str) -> Result<()> {
        ensure!(
            !current.is_empty() && current.len() <= 1024,
            "Enter your current password."
        );
        ensure!(
            !self.account.email.is_empty(),
            "Refresh your account before changing security details."
        );
        let response = self
            .http
            .post(format!(
                "{}/auth/v1/token",
                self.config.url.trim().trim_end_matches('/')
            ))
            .query(&[("grant_type", "password")])
            .header("apikey", self.config.publishable_key.trim())
            .json(&json!({"email": self.account.email, "password": current}))
            .send()
            .context("Could not verify your current password. Check your connection.")?;
        let mut verified: AuthSession =
            decode(response).context("Your current password could not be verified. Try again.")?;
        ensure!(
            verified.user.id == self.session.user.id,
            "Password verification returned another account. Sign in again."
        );
        if verified.expires_at == 0 {
            verified.expires_at = Utc::now().timestamp() + verified.expires_in;
        }
        // A fresh signed-in session proves the current password even when the server's
        // optional current_password enforcement is disabled. Tokens stay in this worker.
        self.session = verified;
        Ok(())
    }

    pub fn execute_account(&mut self, action: AccountAction) -> Result<String> {
        match action {
            AccountAction::SaveDetails(details) => {
                details.validate()?;
                self.update_user(json!({"data":{"hris_profile":details.trimmed()}}))?;
                Ok("Personal details saved.".into())
            }
            AccountAction::SaveDisplay(display) => {
                display.validate()?;
                self.update_user(json!({"data":{"hris_preferences":display}}))?;
                Ok("Display preferences saved.".into())
            }
            AccountAction::LoadReportViews { company_id } => {
                let path = format!(
                    "/rest/v1/report_preferences?select=user_id,company_id,views,revision&user_id=eq.{}&company_id=eq.{company_id}",
                    self.session.user.id
                );
                let mut rows: Vec<ReportPreferences> = decode(
                    self.account_request(Method::GET, &path)?
                        .send()
                        .context("Could not load saved views. Retry when connected.")?,
                )?;
                ensure!(
                    rows.len() <= 1,
                    "The report preferences response is invalid."
                );
                if let Some(row) = rows.pop() {
                    self.accept_report_preferences(company_id, row)?;
                } else {
                    // A still-running older installation may have saved a legacy
                    // view after the one-time migration. Offer it until the first
                    // explicit database save; an existing empty row never revives it.
                    self.account.report_views = crate::report_views::from_metadata(
                        self.session.user.user_metadata.get("hris_report_views"),
                    )
                    .into_iter()
                    .filter(|v| v.company_id == company_id)
                    .collect();
                    self.account.report_views_company = Some(company_id);
                    self.account.report_views_revision = 0;
                }
                Ok("Saved report views loaded.".into())
            }
            AccountAction::SaveReportViews {
                company_id,
                expected_revision,
                views,
            } => {
                crate::report_views::validate_collection(&views)?;
                ensure!(
                    self.account.report_views_company == Some(company_id)
                        && self.account.report_views_revision == expected_revision,
                    "Reload saved views before saving changes."
                );
                let row:ReportPreferences=decode(self.account_request(Method::POST,"/rest/v1/rpc/save_report_preferences")?.json(&json!({"p_company_id":company_id,"p_expected_revision":expected_revision,"p_views":views})).send().context("Could not save report views. Reload views to check whether the save completed before retrying.")?)?;
                ensure!(
                    row.revision == expected_revision.checked_add(1).unwrap_or(-1),
                    "The saved views response has an invalid revision. Reload views before retrying."
                );
                self.accept_report_preferences(company_id, row)?;
                Ok("Your saved report views were updated.".into())
            }
            AccountAction::UploadPhoto(path) => {
                let png = account::photo_from_file(&path)?;
                let target = format!("/storage/v1/object/profile-photos/{}", self.photo_path());
                let response = self
                    .account_request(Method::POST, &target)?
                    .header("Content-Type", "image/png")
                    .header("x-upsert", "true")
                    .header("Cache-Control", "max-age=0")
                    .body(png.clone())
                    .send()
                    .context("Could not upload your photo. Check your connection.")?;
                let _: Value = decode(response)?;
                self.account.photo = Some(png);
                self.account.photo_warning = None;
                Ok("Profile photo updated.".into())
            }
            AccountAction::RemovePhoto => {
                let path = self.photo_path();
                let response = self
                    .account_request(Method::DELETE, "/storage/v1/object/profile-photos")?
                    .json(&json!({"prefixes":[path]}))
                    .send()
                    .context("Could not remove your photo. Check your connection.")?;
                let _: Value = decode(response)?;
                self.account.photo = None;
                self.account.photo_warning = None;
                Ok("Profile photo removed.".into())
            }
            AccountAction::ChangePassword {
                current,
                new,
                confirmation,
            } => {
                account::validate_password_change(&current, &new, &confirmation)?;
                self.verify_current_password(&current)?;
                self.update_user(json!({"password":new,"current_password":current}))?;
                Ok("Password changed. Use your new password the next time you sign in.".into())
            }
            AccountAction::ChangeEmail { current, new_email } => {
                account::validate_email(&new_email)?;
                ensure!(
                    !new_email.trim().eq_ignore_ascii_case(&self.account.email),
                    "That is already your account email."
                );
                self.verify_current_password(&current)?;
                self.update_user(json!({"email":new_email.trim()}))?;
                Ok(if self.account.pending_email.is_empty() { "Account email updated." } else {
                    "Email change requested. Confirm the links sent to your email addresses, then Refresh."
                }.into())
            }
            AccountAction::SignOutOthers => {
                self.sign_out_scope("others")?;
                Ok("Other sessions signed out. This device remains signed in. Existing access tokens expire according to the server's session policy.".into())
            }
        }
    }

    fn accept_report_preferences(
        &mut self,
        company_id: Uuid,
        row: ReportPreferences,
    ) -> Result<()> {
        ensure!(
            row.user_id == self.session.user.id
                && row.company_id == company_id
                && row.revision >= 0,
            "The saved views response did not match your account/company."
        );
        self.account.report_views = crate::report_views::from_metadata(Some(&row.views));
        self.account.report_views_company = Some(company_id);
        self.account.report_views_revision = row.revision;
        Ok(())
    }

    fn rows<T: DeserializeOwned>(&mut self, table: &str, company: Option<Uuid>) -> Result<Vec<T>> {
        let mut all = Vec::new();
        for page in 0..200 {
            let mut request = self.request(Method::GET, table)?.query(&[
                ("select", "*"),
                (
                    "order",
                    if matches!(table, "leave_policies" | "attendance_calculation_policies") {
                        "company_id.asc"
                    } else if table == "notification_read_state" {
                        "user_id.asc,company_id.asc"
                    } else {
                        "id.asc"
                    },
                ),
            ]);
            if let Some(company_id) = company {
                request = request.query(&[("company_id", format!("eq.{company_id}"))]);
            }
            let response = request
                .header("Range-Unit", "items")
                .header("Range", format!("{}-{}", page * 500, page * 500 + 499))
                .send()
                .context("Could not load company data. Check your connection.")?;
            let rows: Vec<T> = decode(response)?;
            let size = rows.len();
            all.extend(rows);
            if size < 500 {
                return Ok(all);
            }
        }
        bail!(
            "This prototype supports up to 100,000 rows per module. Narrow the data before loading."
        )
    }

    fn current_access(&mut self) -> Result<Access> {
        let response = self.request(Method::POST, "rpc/my_hris_access")?
            .json(&json!({})).send()
            .context("Could not verify your current HRIS access. Check your connection.")?;
        if matches!(response.status(), StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN) {
            return Err(AccessRevoked.into());
        }
        let mut access: Access = decode(response)?;
        if access.user_id != self.session.user.id {
            return Err(anyhow::Error::new(AccessRevoked).context("The access response did not match your account."));
        }
        if access.staff_contract_version > crate::staff_roles::SCHEMA_VERSION {
            return Err(anyhow::Error::new(AccessRevoked).context("Update HRIS to use the current staff-permissions contract."));
        }
        if access.staff_contract_version == crate::staff_roles::SCHEMA_VERSION {
            let response = self.request(Method::POST, "rpc/staff_role_context")?
                .json(&json!({})).send()
                .context("Could not verify your staff-role permissions. Check your connection.")?;
            if matches!(response.status(), StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN) {
                return Err(AccessRevoked.into());
            }
            let context: crate::staff_roles::Context = decode(response)
                .map_err(|_| anyhow::Error::new(AccessRevoked))?;
            access.bind_staff_context(context).map_err(|_| anyhow::Error::new(AccessRevoked))?;
        }
        if !access.uses_desktop() {
            let _ = self.sign_out_scope("local");
            return Err(PortalAccount.into());
        }
        Ok(access)
    }
    fn verify_snapshot_staff_scope(&mut self, expected: &Access) -> Result<()> {
        // Legacy schema0 has no explicit staff contract. The deployed version1
        // contract must still match AFTER all scoped reads, including revision.
        if expected.staff_contract_version > 0 && self.current_access()? != *expected {
            return Err(AccessRevoked.into());
        }
        Ok(())
    }
    pub fn report(
        &mut self,
        filter: &crate::reports::Filter,
        destination: Option<&Path>,
    ) -> Result<(Snapshot, crate::reports::Report)> {
        filter.validate()?;
        let data = self.snapshot()?;
        let report = crate::reports::build(&data, filter, Utc::now())?;
        let bytes = destination.map(|path| report.export_bytes(path)).transpose()?;
        let access = self.current_access()?;
        if !crate::reports::same_scope(&data.access, &access) {
            return Err(AccessRevoked.into());
        }
        if let (Some(path), Some(bytes)) = (destination, bytes) {
            crate::reports::save_export(path, &bytes)?;
        }
        Ok((data, report))
    }

    fn leave_import_history_for(&mut self,data:&Snapshot)->Result<crate::leave::request_import::History>{
        ensure!(data.access.allows(crate::staff_roles::Capability::Personnel),"Current HR access is required for leave import history.");
        let h:crate::leave::request_import::History=decode(self.request(Method::POST,"rpc/leave_import_context")?.json(&json!({})).send().context("Could not refresh leave import history.")?)?;
        h.validate(data)?;if self.current_access()?!=data.access{return Err(AccessRevoked.into());}Ok(h)
    }
    pub fn leave_import_history(&mut self)->Result<(Snapshot,crate::leave::request_import::History)>{let data=self.snapshot()?;let h=self.leave_import_history_for(&data)?;Ok((data,h))}
    pub fn leave_import_preview(&mut self,batch:&crate::leave::request_import::Batch)->Result<(Snapshot,crate::leave::request_import::Preview)>{
        let data=self.snapshot()?;batch.validate(&data)?;
        let p:crate::leave::request_import::Preview=decode(self.request(Method::POST,"rpc/preview_leave_import")?.json(&json!({"p_batch":batch})).send().context("Could not prepare the protected leave import preview.")?)?;
        p.validate(&data,batch)?;if self.current_access()?!=data.access{return Err(AccessRevoked.into());}Ok((data,p))
    }
    pub fn post_leave_import(&mut self,post:&crate::leave::request_import::Post)->Result<(Snapshot,crate::leave::request_import::Receipt,crate::leave::request_import::History)>{
        // Keep retries independent of later source/request changes. The server
        // binds an existing receipt to the original actor and complete payload.
        post.identity(&self.current_access()?)?;
        let receipt:crate::leave::request_import::Receipt=decode(self.request(Method::POST,"rpc/post_leave_import")?.json(&json!({"p_batch":post.batch,"p_fingerprint":post.preview.fingerprint})).send().context("Leave import delivery is uncertain. Refresh history or retry this exact submission; do not create another batch.")?)?;
        let data=self.snapshot()?;receipt.validate(&data,post)?;let history=self.leave_import_history_for(&data)?;Ok((data,receipt,history))
    }
    fn attendance_import_history_for(&mut self,data:&Snapshot)->Result<crate::attendance_imports::History>{
        ensure!(data.access.allows(crate::staff_roles::Capability::Personnel),"Current HR access is required for attendance import history.");
        let h:crate::attendance_imports::History=decode(self.request(Method::POST,"rpc/attendance_import_context")?.json(&json!({})).send().context("Could not refresh attendance import history.")?)?;
        h.validate(data)?;if self.current_access()?!=data.access{return Err(AccessRevoked.into());}Ok(h)
    }
    pub fn attendance_import_history(&mut self)->Result<(Snapshot,crate::attendance_imports::History)>{let data=self.snapshot()?;let h=self.attendance_import_history_for(&data)?;Ok((data,h))}
    pub fn attendance_import_preview(&mut self,batch:&crate::attendance_imports::Batch)->Result<(Snapshot,crate::attendance_imports::Preview)>{
        let data=self.snapshot()?;batch.validate(&data)?;
        let p:crate::attendance_imports::Preview=decode(self.request(Method::POST,"rpc/preview_attendance_import")?.json(&json!({"p_batch":batch})).send().context("Could not prepare the protected attendance import preview.")?)?;
        p.validate(&data,batch)?;if self.current_access()?!=data.access{return Err(AccessRevoked.into());}Ok((data,p))
    }
    pub fn post_attendance_import(&mut self,post:&crate::attendance_imports::Post)->Result<(Snapshot,crate::attendance_imports::Receipt,crate::attendance_imports::History)>{
        // Keep retries independent of later source/request changes. The server
        // binds an existing receipt to the original actor and complete payload.
        post.identity(&self.current_access()?)?;
        let receipt:crate::attendance_imports::Receipt=decode(self.request(Method::POST,"rpc/post_attendance_import")?.json(&json!({"p_batch":post.batch,"p_fingerprint":post.preview.fingerprint})).send().context("Attendance import delivery is uncertain. Refresh history or retry this exact submission; do not create another batch.")?)?;
        let data=self.snapshot()?;receipt.validate(&data,post)?;let history=self.attendance_import_history_for(&data)?;Ok((data,receipt,history))
    }
    pub fn withdraw_attendance_import(&mut self,change:&crate::attendance_imports::Withdraw)->Result<(Snapshot,crate::attendance_imports::History)>{
        ensure!(self.current_access()?.allows(crate::staff_roles::Capability::AttendanceAdministration),"Current HR access is required to withdraw an imported request.");
        let receipt:Value=decode(self.request(Method::POST,"rpc/withdraw_attendance_import")?.json(&json!({"p_change":change})).send().context("Withdrawal delivery is uncertain. Keep this exact draft and refresh history before retrying.")?)?;
        ensure!(receipt.get("id")==Some(&json!(change.id))&&receipt.get("item_id")==Some(&json!(change.item_id)),"Withdrawal receipt does not match the request.");
        self.attendance_import_history()
    }
    pub fn leave_opening_preview(&mut self,record:&crate::leave::dated::OpeningRecord)->Result<(Snapshot,crate::leave::dated::Preview)> {
        let data=self.snapshot()?;
        record.validate(&data)?;
        let preview:crate::leave::dated::Preview=decode(self.request(Method::POST,"rpc/preview_leave_opening")?.json(&json!({"p_record":record})).send()?)?;
        if self.current_access()?!=data.access {return Err(AccessRevoked.into());}
        preview.validate(&data,record)?;
        Ok((data,preview))
    }
    pub fn leave_opening_import_preview(&mut self,records:&[crate::leave::dated::OpeningRecord])->Result<(Snapshot,crate::leave::opening_import::BatchPreview)> {
        let data=self.snapshot()?;
        ensure!(data.access.allows(crate::staff_roles::Capability::Personnel) && (1..=100).contains(&records.len()),"HR access and 1–100 opening rows are required.");
        let preview:crate::leave::opening_import::BatchPreview=decode(self.request(Method::POST,"rpc/preview_leave_openings")?.json(&json!({"p_records":records})).send()?)?;
        if self.current_access()?!=data.access{return Err(AccessRevoked.into());}
        preview.validate(&data,records)?;Ok((data,preview))
    }
    pub fn snapshot(&mut self) -> Result<Snapshot> {
        let access = self.current_access()?;
        let mut companies: Vec<Company> = self.rows("companies", None)?;
        ensure!(
            !companies.is_empty(),
            "This HR account is not assigned to this installation's company. Ask your HRIS administrator to finish account setup."
        );
        ensure!(
            companies.len() == 1,
            "This installation requires exactly one company. Ask your HRIS administrator to finish setup."
        );
        let company = companies.remove(0);
        ensure!(
            company.id == access.company_id,
            "This HR account is not assigned to this company's records."
        );
        let mut employees: Vec<Employee> = self.rows("employees", Some(company.id))?;
        employees.sort_by(|a, b| a.full_name.cmp(&b.full_name));
        let contacts = self.rows("employee_contacts", Some(company.id))?;
        let mut attendance: Vec<Attendance> = self.rows("attendance", Some(company.id))?;
        attendance.sort_by(|a, b| b.checked_in_at.cmp(&a.checked_in_at));
        let corrections = if access.allows(crate::staff_roles::Capability::AttendanceAdministration) {
            self.rows("attendance_corrections", Some(company.id))?
        } else {
            Vec::new()
        };
        let mut leave: Vec<LeaveRequest> = self.rows("leave_requests", Some(company.id))?;
        leave.sort_by(|a, b| b.starts_on.cmp(&a.starts_on));
        let (accounts, access_events) = if access.role.manages_accounts() {
            let response = self
                .request(Method::POST, "rpc/list_company_accounts")?
                .json(&json!({}))
                .send()?;
            let accounts: Vec<ManagedAccount> = decode(response)?;
            let response = self
                .request(Method::GET, "account_access_events")?
                .query(&[
                    ("select", "*"),
                    ("order", "created_at.desc,id.desc"),
                    ("limit", "50"),
                ])
                .send()?;
            let events: Vec<AccessEvent> = decode(response)?;
            (accounts, events)
        } else {
            (Vec::new(), Vec::new())
        };
        let mut employment_types: Vec<crate::employment::EmploymentOption> =
            self.rows("employment_types", Some(company.id))?;
        let mut work_locations: Vec<crate::employment::EmploymentOption> =
            self.rows("work_locations", Some(company.id))?;
        for choices in [&mut employment_types, &mut work_locations] {
            choices.sort_by(|a, b| {
                a.name
                    .to_lowercase()
                    .cmp(&b.name.to_lowercase())
                    .then(a.id.cmp(&b.id))
            });
        }
        let employment_history = if access.allows(crate::staff_roles::Capability::Personnel) {
            self.rows("employment_history", Some(company.id))?
        } else {
            Vec::new()
        };
        let (documents, document_events) = if access.allows(crate::staff_roles::Capability::PersonnelDocuments) {
            (
                self.rows("employee_documents", Some(company.id))?,
                self.rows("employee_document_events", Some(company.id))?,
            )
        } else {
            (Vec::new(), Vec::new())
        };
        let _legacy_policy = self.rows::<crate::leave::LeavePolicy>("leave_policies",Some(company.id))?.into_iter().next().ok_or_else(||anyhow::anyhow!("Leave policies are not configured. Ask the deployment administrator to apply the required migration."))?;
        let leave_events = self.rows("leave_events", Some(company.id))?;
        let credits:crate::leave::credits::Context=decode(self.request(Method::POST,"rpc/leave_credit_context")?.json(&json!({})).send()?)?;
        if credits.company_id!=access.company_id || credits.user_id!=access.user_id || credits.employee_id!=access.employee_id || credits.role!=access.role {return Err(AccessRevoked.into());}
        credits.validate(&access,company.today())?;
        ensure!(credits.entries.iter().all(|c|employees.iter().any(|e|e.id==c.employee_id && e.company_id==company.id)),"Leave credits include an unknown employee.");
        let dated:crate::leave::dated::Context=decode(self.request(Method::POST,"rpc/leave_dated_context")?.json(&json!({})).send()?)?;
        if dated.company_id!=access.company_id || dated.user_id!=access.user_id || dated.employee_id!=access.employee_id || dated.role!=access.role {return Err(AccessRevoked.into());}
        dated.validate(&access,company.today())?;
        ensure!(dated.anchors.iter().all(|a|employees.iter().any(|e|e.id==a.employee_id)) && dated.openings.iter().all(|o|employees.iter().any(|e|e.id==o.employee_id)),"Dated leave includes an unknown employee.");
        let policy=dated.timeline.policy_on(company.today()).clone();
        let mut shift_templates: Vec<crate::shifts::ShiftTemplate> =
            self.rows("shift_templates", Some(company.id))?;
        shift_templates.sort_by_key(|t| t.name.to_lowercase());
        let shifts = crate::shifts::ShiftSetup {
            templates: shift_templates,
            assignments: self.rows("shift_assignments", Some(company.id))?,
            versions: if access.allows(crate::staff_roles::Capability::AttendanceAdministration) {
                self.rows("shift_template_versions", Some(company.id))?
            } else {
                Vec::new()
            },
        };
        let breaks = crate::breaks::BreakSetup {
            records: self.rows("attendance_breaks", Some(company.id))?,
            events: if access.allows(crate::staff_roles::Capability::AttendanceAdministration) {
                self.rows("attendance_break_events", Some(company.id))?
            } else {
                Vec::new()
            },
        };
        let calculations=crate::calculations::CalculationSetup {
            policy:self.rows::<crate::calculations::CalculationPolicy>("attendance_calculation_policies",Some(company.id))?.into_iter().next().ok_or_else(||anyhow::anyhow!("Attendance calculation rules are not configured. Ask the deployment administrator to apply the required migration."))?,
            policy_events:if access.allows(crate::staff_roles::Capability::AttendanceAdministration){self.rows("attendance_calculation_policy_events",Some(company.id))?}else{Vec::new()},
            attendance_events:if access.allows(crate::staff_roles::Capability::AttendanceAdministration){self.rows("attendance_calculation_events",Some(company.id))?}else{Vec::new()},
        };
        let requests = crate::requests::Setup {
            records: self.rows("attendance_requests", Some(company.id))?,
            events: self.rows("attendance_request_events", Some(company.id))?,
        };
        let overtime = crate::overtime::Setup {
            records: self.rows("overtime_requests", Some(company.id))?,
            events: self.rows("overtime_events", Some(company.id))?,
        };
        let imports = if access.allows(crate::staff_roles::Capability::Personnel) {
            crate::imports::Setup {
                batches: self.rows("employee_import_batches", Some(company.id))?,
                items: self.rows("employee_import_items", Some(company.id))?,
            }
        } else {
            Default::default()
        };
        let hiring = if access.allows(crate::staff_roles::Capability::Recruiting) {
            crate::hiring::Setup {
                records: self.rows("applicants", Some(company.id))?,
                events: self.rows("applicant_events", Some(company.id))?,
            }
        } else {
            Default::default()
        };
        let recruiting = self.recruiting_snapshot(&access)?;
        let onboarding = self.onboarding_snapshot(&access)?;
        let exits = self.exits_snapshot(&access)?;
        let performance = self.performance_snapshot(&access)?;
        let learning = self.learning_snapshot(&access)?;
        let career = self.career_snapshot(&access)?;
        let payroll = self.payroll_snapshot(&access)?;
        let (logo_png, logo_warning) = self.company_logo(&company);
        let mut notification_rows: Vec<crate::notifications::ReadState> =
            self.rows("notification_read_state", Some(company.id))?;
        ensure!(
            notification_rows.len() <= 1,
            "Notification history response is invalid."
        );
        let notification_reads =
            notification_rows
                .pop()
                .unwrap_or(crate::notifications::ReadState {
                    user_id: access.user_id,
                    company_id: company.id,
                    ..Default::default()
                });
        notification_reads.validate(access.user_id, company.id)?;
        let lifecycle_notifications: crate::notifications::lifecycle::Inbox = decode(
            self.request(Method::POST, "rpc/lifecycle_notifications")?
                .json(&json!({"p_surface":"desktop"})).send()?
        )?;
        if lifecycle_notifications.company_id!=access.company_id || lifecycle_notifications.user_id!=access.user_id
            || lifecycle_notifications.employee_id!=access.employee_id || lifecycle_notifications.role!=access.role { return Err(AccessRevoked.into()); }
        lifecycle_notifications.validate(&access, company.today())?;
        ensure!(lifecycle_notifications.items.iter().all(|i| employees.iter().any(|e| e.company_id==company.id && e.id==i.employee_id)), "Lifecycle subject is outside the current employee directory.");
        let approvals:crate::approvals::Context=decode(self.request(Method::POST,"rpc/approval_delegation_context")?.json(&json!({})).send()?)?;
        if approvals.company_id!=access.company_id||approvals.user_id!=access.user_id||approvals.employee_id!=access.employee_id||approvals.role!=access.role{return Err(AccessRevoked.into());}
        approvals.validate_access(&access,company.today())?;
        let snapshot=Snapshot {
            approvals: Some(approvals),
            lifecycle_notifications: Some(lifecycle_notifications),
            payroll,
            learning,
            career,
            performance,
            exits,
            onboarding,
            recruiting,
            hiring,
            notification_reads,
            imports,
            overtime,
            requests,
            calculations,
            breaks,
            shifts,
            leave_setup: crate::leave::LeaveSetup {
                dated: Some(dated),
                credits: Some(credits),
                policy,
                events: leave_events,
            },
            company,
            logo_png,
            logo_warning,
            employees,
            contacts,
            attendance,
            leave,
            mode: Mode::Supabase,
            access,
            accounts,
            access_events,
            corrections,
            employment_types,
            work_locations,
            employment_history,
            documents,
            document_events,
        };
        snapshot.approvals.as_ref().unwrap().validate(&snapshot)?;
        self.verify_snapshot_staff_scope(&snapshot.access)?;
        Ok(snapshot)
    }

    fn document_write(&mut self, operation: &str, payload: Value) -> Result<EmployeeDocument> {
        decode(self.request(Method::POST, "rpc/save_employee_document")?
            .json(&json!({"p_operation":operation,"p_document":payload})).send()
            .context("Could not save the document. Check your connection and Refresh before retrying.")?)
    }

    pub fn execute_document(
        &mut self,
        action: DocumentAction,
        current: &Snapshot,
    ) -> Result<(Snapshot, String)> {
        let message = match action {
            DocumentAction::Upload(draft) => {
                let prepared = documents::prepare_upload(&draft, current)?;
                let reserved = self.document_write("reserve", prepared.payload.clone())?;
                reserved.validate_file_record()?;
                ensure!(
                    reserved.id == draft.id
                        && reserved.company_id == current.company.id
                        && reserved.employee_id == draft.employee_id
                        && reserved.details == draft.details()?
                        && reserved.byte_size == prepared.bytes.len()
                        && reserved.mime_type == prepared.mime
                        && reserved.sha256 == prepared.payload["sha256"].as_str().unwrap_or(""),
                    "The upload reservation did not match the selected document. No file was sent."
                );
                if reserved.state == DocumentState::Pending {
                    // A retry may encounter the already-uploaded immutable object. Finalization
                    // validates Storage's actual size/type/owner before making the record visible.
                    let uploaded = self
                        .account_request(
                            Method::POST,
                            &format!(
                                "/storage/v1/object/employee-documents/{}",
                                reserved.object_name
                            ),
                        )?
                        .header("Content-Type", prepared.mime)
                        .header("Cache-Control", "max-age=0")
                        .body(prepared.bytes)
                        .send();
                    drop(uploaded);
                    self.document_write("finalize",json!({"id":reserved.id}))
                        .context("The upload was not confirmed. Refresh to check it, retry the same file, or discard the incomplete upload.")?;
                } else {
                    ensure!(
                        reserved.state == DocumentState::Ready,
                        "This upload has been discarded. Start a new upload."
                    );
                }
                "Document filed. Its original file is retained; details can be edited separately."
                    .into()
            }
            DocumentAction::Edit(draft) => {
                let doc = documents::record(current, draft.id)?;
                ensure!(
                    doc.employee_id == draft.employee_id && draft.original.is_some(),
                    "Reopen this document before editing it."
                );
                self.document_write("edit", draft.payload()?)?;
                "Document details saved. The original file is unchanged.".into()
            }
            DocumentAction::Archive {
                id,
                revision,
                archived,
            } => {
                documents::record(current, id)?;
                self.document_write(
                    if archived { "archive" } else { "restore" },
                    json!({"id":id,"revision":revision}),
                )?;
                if archived {
                    "Document archived. Its file and history are retained."
                } else {
                    "Document restored."
                }
                .into()
            }
            DocumentAction::Discard { id } => {
                documents::record(current, id)?;
                let discarded = self.document_write("discard", json!({"id":id}))?;
                discarded.validate_file_record()?;
                let response = self.account_request(Method::DELETE,"/storage/v1/object/employee-documents")?
                    .json(&json!({"prefixes":[discarded.object_name]})).send()
                    .context("Upload discarded, but unused-file cleanup failed. Refresh and retry Clean up upload.")?;
                let _: Value = decode(response).context("Upload discarded, but unused-file cleanup failed. Refresh and retry Clean up upload.")?;
                "Incomplete upload discarded and unused file cleaned up.".into()
            }
            DocumentAction::Download { id, destination } => {
                let doc = documents::record(current, id)?;
                doc.validate_file_record()?;
                ensure!(
                    doc.state == DocumentState::Ready,
                    "This upload is not yet filed."
                );
                let mut response = self
                    .account_request(
                        Method::GET,
                        &format!(
                            "/storage/v1/object/authenticated/employee-documents/{}",
                            doc.object_name
                        ),
                    )?
                    .header("Cache-Control", "no-cache")
                    .send()
                    .context("Could not download the private document.")?;
                if !response.status().is_success() {
                    let _: Value = decode(response)?;
                    unreachable!();
                }
                let mut bytes = Vec::new();
                response
                    .by_ref()
                    .take(documents::MAX_FILE_BYTES as u64 + 1)
                    .read_to_end(&mut bytes)?;
                documents::save_download(doc, &bytes, &destination)?;
                "Document downloaded to the file you selected.".into()
            }
        };
        let data = self.snapshot().context(
            "Document operation completed, but refresh failed. Click Refresh before retrying.",
        )?;
        Ok((data, message))
    }

    pub fn execute(&mut self, action: Action, current: &Snapshot) -> Result<Snapshot> {
        current.access.authorize(&action, current)?;
        if let Action::Payroll(action) = &action {
            let payload=action.payload(current)?;
            let endpoint=if action.operation.starts_with("amendment_"){ "rpc/manage_payroll_amendment" }else if action.operation.starts_with("wage_"){ "rpc/manage_payroll_wage" }else if action.operation.starts_with("final_"){ "rpc/manage_payroll_final" }else if action.operation.starts_with("receipt_"){ "rpc/manage_payroll_receipt" }else if action.operation.starts_with("tax_"){ "rpc/manage_payroll_tax" }else if action.operation.starts_with("thirteenth_"){ "rpc/manage_payroll_thirteenth" }else if action.operation.starts_with("cutoff_"){ "rpc/manage_payroll_cutoff" }else{ "rpc/manage_payroll" };
            let saved:Value=decode(self.request(Method::POST,endpoint)?.json(&payload).send()?)?;
            ensure!(saved["company_id"]==json!(current.company.id)&&saved["id"]==payload["p_record"]["id"],"Payroll response did not match your company and record.");
            return self.snapshot().context("Payroll change saved, but refresh failed. Keep your draft and Refresh before retrying.");
        }
        if let Action::Career(action) = &action {
            let payload = action.payload(current)?;
            let saved:Value=decode(self.request(Method::POST,"rpc/manage_career")?.json(&payload).send()?)?;
            ensure!(saved["company_id"]==json!(current.company.id)&&saved["id"]==payload["p_record"]["id"],"Career response did not match your company and record.");
            return self.snapshot().context("Career change saved, but refresh failed. Keep your draft and Refresh before retrying.");
        }
        if let Action::Learning(action) = &action {
            let payload=action.payload(current)?;
            let saved:Value=decode(self.request(Method::POST,"rpc/manage_learning")?.json(&payload).send()?)?;
            ensure!(saved["company_id"]==json!(current.company.id)&&saved["id"]==payload["p_record"]["id"],"Learning response did not match your company and record.");
            return self.snapshot().context("Learning change saved, but refresh failed. Keep your draft and Refresh before retrying.");
        }
        if let Action::Performance(action) = &action {
            let payload=action.payload(current)?;
            let saved:Value=decode(self.request(Method::POST,"rpc/manage_performance")?.json(&payload).send()?)?;
            ensure!(saved["company_id"]==json!(current.company.id)&&saved["id"]==payload["p_record"]["id"],"Performance response did not match your company and record.");
            return self.snapshot().context("Performance change saved, but refresh failed. Keep your draft and Refresh before retrying.");
        }
        if let Action::Exit(action) = &action {
            let payload=action.payload(current)?;
            let saved:Value=decode(self.request(Method::POST,"rpc/manage_exit")?.json(&payload).send()?)?;
            ensure!(saved["company_id"]==json!(current.company.id)&&saved["id"]==payload["p_record"]["id"],"Exit response did not match the current company and record.");
            return self.snapshot().context("Exit or clearance change saved, but refresh failed. Keep your draft and Refresh before retrying.");
        }
        if let Action::Onboarding(action) = &action {
            let payload = action.payload(current)?;
            let saved: Value = decode(
                self.request(Method::POST, "rpc/manage_hiring")?
                    .json(&payload)
                    .send()?,
            )?;
            ensure!(
                saved["company_id"] == json!(current.company.id)
                    && saved["id"] == payload["p_record"]["id"],
                "Hiring response did not match the current record/company."
            );
            return self.snapshot().context("Hiring change saved, but refresh failed. Keep your draft and Refresh before retrying.");
        }
        if let Action::Interview(action) = &action {
            let payload = action.payload(current, Utc::now())?;
            let saved: Value = decode(
                self.request(Method::POST, "rpc/manage_interview")?
                    .json(&payload)
                    .send()?,
            )?;
            ensure!(
                saved["company_id"] == json!(current.company.id)
                    && (if payload["p_operation"] == "feedback" {
                        saved["interview_id"] == payload["p_record"]["id"]
                            && saved["author_id"] == json!(current.access.user_id)
                    } else {
                        saved["id"] == payload["p_record"]["id"]
                    }),
                "Interview response did not match the current record/company."
            );
            return self.snapshot().context(
                "Interview saved, but refresh failed. Keep the draft and Refresh before retrying.",
            );
        }
        if let Action::Hiring(action) = &action {
            let payload = action.payload(current)?;
            let id: Uuid = serde_json::from_value(payload["p_record"]["id"].clone())?;
            let saved:crate::hiring::Applicant=decode(self.request(Method::POST,"rpc/manage_applicant")?.json(&payload).send().context("Could not save the application. Keep this draft and refresh before retrying.")?)?;
            ensure!(
                saved.id == id && saved.company_id == current.company.id,
                "The application response did not match your company/record."
            );
            return self.snapshot().context("Application saved, but refresh failed. Keep this draft and Refresh; retrying the same new application is safe.");
        }
        if let Action::ApprovalDelegation(change)=&action {
            let payload=change.payload(current)?;
            let saved:Value=decode(self.request(Method::POST,"rpc/save_approval_delegation")?.json(&json!({"p_change":payload})).send().context("Delegation outcome is uncertain. Keep this draft and retry the same request.")?)?;
            ensure!(saved.as_object().is_some_and(|o|o.len()==2)&&saved["id"]==json!(change.id())&&saved["company_revision"].as_i64()==Some(change.expected_revision()+1),"Delegation response did not match the submitted request.");
            let fresh=self.snapshot().context("Delegation saved, but refresh failed. Keep this draft and Refresh or retry the same request.")?;
            ensure!(fresh.approvals.as_ref().is_some_and(|c|change.matches_history(c)),"Saved delegation does not match the retained draft. Keep it and Refresh.");
            return Ok(fresh);
        }
        if let Action::LeaveCredit(record)=&action {
            record.validate(current)?;
            let saved:serde_json::Value=decode(self.request(Method::POST,"rpc/post_leave_credit")?.json(&json!({"p_record":record})).send().context("Credit outcome is uncertain. Keep this draft and retry the same request.")?)?;
            ensure!(saved.as_object().is_some_and(|o|o.len()==2) && saved["id"]==json!(record.id)
                && saved["company_revision"].as_i64()==Some(record.expected_revision+1),"Credit response did not match the submitted request.");
            let fresh=self.snapshot().context("Credit saved, but refresh failed. Keep this draft and Refresh or retry the same request.")?;
            ensure!(fresh.leave_setup.credits.as_ref().is_some_and(|c|c.entries.iter().any(|e|record.matches_entry(e))),"Saved credit did not match this draft. Keep it and Refresh.");
            return Ok(fresh);
        }
        if let Action::DatedLeave(change)=&action {
            let (endpoint,payload)=change.payload(current)?;
            let saved:Value=decode(self.request(Method::POST,endpoint)?.json(&payload).send().context("Leave setup outcome is uncertain. Keep this draft and retry the same request.")?)?;
            let batch=matches!(change,crate::leave::dated::Change::OpeningBatch(_));
            ensure!(saved.as_object().is_some_and(|o|o.len()==if batch{3}else{2}) && saved["id"]==json!(change.id()) && saved["company_revision"].as_i64()==Some(change.posted_revision()),"Leave setup response did not match this request.");
            if let crate::leave::dated::Change::OpeningBatch(p)=change {ensure!(saved["opening_ids"]==json!(p.preview.rows.iter().map(|r|r.record.id).collect::<Vec<_>>()),"Opening import response did not match its complete row inventory.");}
            let fresh=self.snapshot().context("Leave setup saved, but refresh failed. Keep this draft and Refresh or retry the same request.")?;
            ensure!(fresh.leave_setup.dated.as_ref().is_some_and(|c|change.matches(c)),"Saved leave setup does not match this draft. Keep it and Refresh.");
            return Ok(fresh);
        }
        if let Action::NotificationReads(action) = &action {
            let payload = action.payload(current.company.id)?;
            let extended = match action {
                crate::notifications::ReadAction::Mark { keys, .. } => keys.iter().any(|k| crate::notifications::lifecycle::valid_key(k)),
                crate::notifications::ReadAction::Reset => current.lifecycle_notifications.as_ref().is_some_and(|s| !s.read_keys.is_empty()),
            };
            let saved: crate::notifications::ReadState = decode(
                self.request(Method::POST, if extended { "rpc/staff_notification_reads" } else { "rpc/set_notification_reads" })?
                    .json(&payload)
                    .send()
                    .context("Could not update notification history. Refresh before retrying.")?,
            )?;
            saved.validate(current.access.user_id, current.company.id)?;
            return self.snapshot().context(
                "Notification history saved, but refresh failed. Refresh before retrying.",
            );
        }
        if let Action::ImportEmployees(preview, reason) = &action {
            let fresh = self.snapshot()?;
            ensure!(
                fresh.access.allows(crate::staff_roles::Capability::Personnel) && fresh.company.id == preview.company_id,
                "Your import access changed. Refresh or sign in again."
            );
            if !fresh.imports.batches.iter().any(|b| b.id == preview.id) {
                preview.validate_current(&fresh)?;
            }
            let response=self.request(Method::POST,"rpc/import_employees")?.json(&preview.payload(reason)?).send().context("Could not submit the import. Refresh and retry the same preview if the outcome is uncertain.")?;
            let _: Uuid = decode(response)?;
            return self.snapshot().context("Import saved, but refresh failed. Refresh or retry the same preview before reopening the file.");
        }
        if let Action::Overtime(action) = &action {
            let payload = crate::overtime::payload(action, current)?;
            let _: Uuid = decode(
                self.request(Method::POST, "rpc/overtime_request")?
                    .json(&json!({"p_request":payload}))
                    .send()
                    .context("Could not save overtime review. Refresh before retrying.")?,
            )?;
            return self
                .snapshot()
                .context("Overtime review saved, but refresh failed. Refresh before retrying.");
        }
        if let Action::AttendanceRequest(action) = &action {
            let payload = crate::requests::payload(action, current, Utc::now())?;
            let _:Uuid=decode(self.request(Method::POST,"rpc/attendance_request")?.json(&json!({"p_request":payload})).send().context("Could not save the attendance request. Check your connection and refresh before retrying.")?)?;
            return self
                .snapshot()
                .context("Attendance request saved, but refresh failed. Refresh before retrying.");
        }
        let calculation_write = match &action {
            Action::SaveCalculationPolicy(draft) => Some((
                "rpc/save_attendance_calculation_policy",
                json!({"p_policy":draft.payload(current)?}),
            )),
            Action::ApplyCalculationPolicy(change) => Some((
                "rpc/apply_attendance_calculation_policy",
                json!({"p_change":change.payload(current)?}),
            )),
            _ => None,
        };
        if let Some((rpc, payload)) = calculation_write {
            let _:Uuid=decode(self.request(Method::POST,rpc)?.json(&payload).send().context("Could not save calculation rules. Check your connection and refresh before retrying.")?)?;
            return self.snapshot().context(
                "Calculation rules saved, but refresh failed. Click Refresh before trying again.",
            );
        }
        if let Action::AttendanceBreak(action) = &action {
            let (operation, payload, _) = action.prepare(current, Utc::now())?;
            let _: Uuid = decode(self.request(Method::POST,"rpc/manage_attendance_break")?.json(&json!({"p_operation":operation,"p_break":payload})).send().context("Could not save the break. Check your connection and refresh before retrying.")?)?;
            return self
                .snapshot()
                .context("Break saved, but refresh failed. Click Refresh before trying again.");
        }
        let shift_write = match &action {
            Action::SaveShiftTemplate(draft) => Some((
                "rpc/save_shift_template",
                json!({"p_shift":draft.payload(current)?}),
            )),
            Action::AssignShift(draft) => {
                draft.assignment(current)?;
                Some(("rpc/assign_shift", json!({"p_assignment":draft.payload()?})))
            }
            _ => None,
        };
        if let Some((rpc, payload)) = shift_write {
            let _: Uuid = decode(self.request(Method::POST, rpc)?.json(&payload).send()
                .context("Could not save the schedule. Check your connection and refresh before retrying.")?)?;
            return self
                .snapshot()
                .context("Schedule saved, but refresh failed. Click Refresh before trying again.");
        }
        if matches!(
            &action,
            Action::UploadCompanyLogo(_) | Action::RemoveCompanyLogo
        ) {
            let path = format!("{}/logo.png", current.company.id);
            let present = if let Action::UploadCompanyLogo(file) = &action {
                let png = crate::branding::logo_from_file(file)?;
                let response = self
                    .account_request(
                        Method::POST,
                        &format!("/storage/v1/object/company-branding/{path}"),
                    )?
                    .header("Content-Type", "image/png")
                    .header("x-upsert", "true")
                    .header("Cache-Control", "max-age=0")
                    .body(png)
                    .send()
                    .context("Could not upload the company logo.")?;
                let _: Value = decode(response)?;
                true
            } else {
                let response = self
                    .account_request(Method::DELETE, "/storage/v1/object/company-branding")?
                    .json(&json!({"prefixes":[path]}))
                    .send()?;
                let _: Value = decode(response)?;
                false
            };
            let response = self
                .request(Method::POST, "rpc/set_company_logo")?
                .json(&json!({"p_present":present}))
                .send()?;
            let _:Option<Uuid>=decode(response).context("The logo file changed, but its company setting could not be saved. Refresh and retry.")?;
            self.logo_cache = None;
            return self.snapshot();
        }
        if let Action::CorrectAttendance(change) = &action {
            change.validate(current, Utc::now())?;
            let response = self.request(Method::POST, "rpc/correct_attendance")?
                .json(&json!({"p_change":change.payload()})).send()
                .context("Could not save the correction. Check your connection and refresh before retrying.")?;
            let _: Uuid = decode(response)?;
            return self.snapshot().context("Attendance correction saved, but refresh failed. Click Refresh before trying again.");
        }
        if let Action::SaveEmploymentOption(draft) = &action {
            draft.option(current)?;
            let response=self.request(Method::POST,"rpc/save_employment_option")?
                .json(&json!({"p_kind":draft.kind,"p_option":draft.payload()})).send()
                .context("Could not save the employment choice. Check your connection and refresh before retrying.")?;
            let _: Uuid = decode(response)?;
            return self.snapshot().context(
                "Employment choice saved, but refresh failed. Click Refresh before trying again.",
            );
        }
        if let Action::SaveEmployeeRecord(draft, contacts) = &action {
            let employee = draft.employee(current.company.id)?;
            crate::employment::validate_assignments(current, &employee)?;
            let contacts = contacts.normalized()?;
            ensure!(
                contacts.company_id == current.company.id,
                "Choose an employee in this company."
            );
            let response = self.request(Method::POST, "rpc/save_employee_record")?
                .json(&json!({"p_employee": {"id": draft.id, "company_id": current.company.id,
                    "employee_code": employee.employee_code, "full_name": employee.full_name,
                    "department": employee.department, "job_title": employee.job_title,
                    "joined_on": employee.joined_on, "annual_leave_days": employee.annual_leave_days,
                    "status": employee.status, "reports_to": employee.reports_to,"employment_type_id":employee.employment_type_id,
                    "work_location_id":employee.work_location_id}, "p_contacts": contacts}))
                .send().context("Could not save the employee record. Check your connection.")?;
            let _: Uuid = decode(response)?;
            return self
                .snapshot()
                .context("Employee saved, but refresh failed. Click Refresh before trying again.");
        }
        if let Action::SaveEmployeeContacts(contacts) | Action::SaveOwnContacts(contacts) = &action
        {
            let contacts = contacts.normalized()?;
            ensure!(
                contacts.company_id == current.company.id
                    && current
                        .employees
                        .iter()
                        .any(|e| e.id == contacts.employee_id),
                "Choose an employee in this company."
            );
            let existing = current
                .contacts
                .iter()
                .any(|c| c.employee_id == contacts.employee_id);
            let mut body = json!({"phone": contacts.phone, "personal_email": contacts.personal_email,
                "address": contacts.address, "emergency_name": contacts.emergency_name,
                "emergency_relationship": contacts.emergency_relationship, "emergency_phone": contacts.emergency_phone,
                "emergency_alternate_phone": contacts.emergency_alternate_phone});
            let method = if existing {
                Method::PATCH
            } else {
                Method::POST
            };
            let mut request = self
                .request(method, "employee_contacts")?
                .header("Prefer", "return=representation");
            if existing {
                request = request.query(&[
                    ("company_id", format!("eq.{}", current.company.id)),
                    ("employee_id", format!("eq.{}", contacts.employee_id)),
                ]);
            } else {
                body["company_id"] = json!(current.company.id);
                body["employee_id"] = json!(contacts.employee_id);
            }
            let rows: Vec<Value> = decode(
                request
                    .json(&body)
                    .send()
                    .context("Could not save employee contacts. Check your connection.")?,
            )?;
            ensure!(
                rows.len() == 1,
                "No contact record changed. Refresh and check your permissions."
            );
            return self
                .snapshot()
                .context("Contacts saved, but refresh failed. Click Refresh before trying again.");
        }
        let (method, table, id, body) = match action {
            Action::Payroll(_) => unreachable!("Payroll changes return above"),
            Action::Career(_) => unreachable!("Career changes return above"),
            Action::Learning(_) => unreachable!("Learning changes return above"),
            Action::Performance(_) => unreachable!("Performance changes return above"),
            Action::Exit(_) => unreachable!("Exit changes return above"),
            Action::Onboarding(_) => unreachable!("Hiring workflow changes return above"),
            Action::Interview(_) => unreachable!("Interview changes return above"),
            Action::Hiring(_) => unreachable!("Applicant changes return above"),
            Action::NotificationReads(_) => unreachable!("Notification reads return above"),
            Action::LeaveCredit(_) => unreachable!("Credit posts return above"),
            Action::DatedLeave(_) => unreachable!("Dated leave posts return above"),
            Action::ApprovalDelegation(_) => unreachable!("Delegation writes return above"),
            Action::ImportEmployees(_, _) => unreachable!("imports use their guarded transaction"),
            Action::Overtime(_) => unreachable!("Overtime actions return above"),
            Action::AttendanceRequest(_) => unreachable!("Attendance requests return above"),
            Action::SaveCalculationPolicy(_) | Action::ApplyCalculationPolicy(_) => {
                unreachable!("Calculation saves return above")
            }
            Action::AttendanceBreak(_) => unreachable!("Break operations return above"),
            Action::SaveShiftTemplate(_) | Action::AssignShift(_) => {
                unreachable!("Schedule saves return above")
            }
            Action::SaveEmploymentOption(_) => unreachable!("Employment choices return above"),
            Action::CorrectAttendance(_) => unreachable!("Corrections return above"),
            Action::SaveEmployeeRecord(_, _) => unreachable!("Employee record saves return above"),
            Action::SaveEmployeeContacts(_) | Action::SaveOwnContacts(_) => {
                unreachable!("Contact saves return above")
            }
            Action::UploadCompanyLogo(_) | Action::RemoveCompanyLogo => {
                unreachable!("Logo saves return above")
            }
            Action::SaveEmployee(draft) => {
                let employee = draft.employee(current.company.id)?;
                crate::employment::validate_assignments(current, &employee)?;
                let mut body = json!({"employee_code": employee.employee_code, "full_name":employee.full_name,
                    "department":employee.department,"job_title":employee.job_title,"joined_on":employee.joined_on,
                    "annual_leave_days":employee.annual_leave_days,"status":employee.status,"reports_to":employee.reports_to,
                    "employment_type_id":employee.employment_type_id,"work_location_id":employee.work_location_id});
                if let Some(id) = draft.id {
                    (Method::PATCH, "employees", Some(id), body)
                } else {
                    body["company_id"] = json!(current.company.id);
                    (Method::POST, "employees", None, body)
                }
            }
            Action::CheckIn(employee_id) => (
                Method::POST,
                "attendance",
                None,
                json!({"company_id":current.company.id,"employee_id":employee_id}),
            ),
            Action::CheckOut(id) => (
                Method::PATCH,
                "attendance",
                Some(id),
                json!({"checked_out_at":Utc::now()}),
            ),
            Action::RequestLeave(draft) => {
                if current.leave_setup.dated.as_ref().is_some_and(|c|c.enabled) {
                    let row=crate::leave::prepare(current,&draft)?;
                    let c=current.leave_setup.dated.as_ref().unwrap();
                    let e=current.employees.iter().find(|e|e.id==row.employee_id).context("Choose an employee.")?;
                    let t=crate::leave::dated::policy_on(current,row.starts_on).leave_type(row.type_code()).context("Choose an active type.")?;
                    let balance=if t.accrual==crate::leave::Accrual::Unlimited{None}else{Some(crate::leave::balance_checked(current,e,t,chrono::Datelike::year(&row.starts_on),row.starts_on,None)?)};
                    (Method::POST,"rpc/submit_leave_checked",None,json!({"p_employee":row.employee_id,"p_type_code":row.type_code(),"p_starts":row.starts_on,"p_ends":row.ends_on,"p_part":row.policy.day_part,"p_reason":row.reason,
                        "p_expected":{"setup_revision":c.revision,"credit_revision":current.leave_setup.credits.as_ref().map_or(0,|c|c.revision),"policy_revision":row.policy.policy_revision,"charged_dates":row.policy.charged_dates,"balance":balance}}))
                } else {
                let row = draft.request(current.company.id)?;
                let code = if draft.options.type_code.is_empty() {
                    row.type_code()
                } else {
                    &draft.options.type_code
                };
                (
                    Method::POST,
                    "rpc/submit_leave",
                    None,
                    json!({"p_employee":row.employee_id,"p_type_code":code,"p_starts":row.starts_on,"p_ends":row.ends_on,"p_part":draft.options.day_part,"p_reason":row.reason}),
                )
                }
            }
            Action::DecideLeave(id, status) => {
                ensure!(
                    matches!(status, LeaveStatus::Approved | LeaveStatus::Rejected),
                    "Choose approve or reject."
                );
                let revision = current
                    .leave
                    .iter()
                    .find(|r| r.id == id)
                    .ok_or_else(|| anyhow::anyhow!("Request not found."))?
                    .policy
                    .revision;
                (
                    Method::POST,
                    "rpc/review_leave",
                    None,
                    json!({"p_id":id,"p_revision":revision,"p_approve":status==LeaveStatus::Approved,"p_note":""}),
                )
            }
            Action::ReviewLeave(id, revision, approve, note) => (
                Method::POST,
                "rpc/review_leave",
                None,
                json!({"p_id":id,"p_revision":revision,"p_approve":approve,"p_note":note}),
            ),
            Action::CancelLeave(id, revision, reason) => (
                Method::POST,
                "rpc/cancel_leave",
                None,
                json!({"p_id":id,"p_revision":revision,"p_reason":reason}),
            ),
            Action::SaveLeavePolicy(policy) => {
                policy.validate(Some(&current.leave_setup.policy))?;
                (
                    Method::POST,
                    "rpc/save_leave_policy",
                    None,
                    json!({"p_policy":policy,"p_expected_revision":policy.revision}),
                )
            }
            Action::SaveCompany(company) => {
                company.validate()?;
                (
                    Method::PATCH,
                    "companies",
                    Some(current.company.id),
                    json!({"name":company.name.trim(),"timezone":company.timezone,"annual_leave_days":company.annual_leave_days}),
                )
            }
        };
        let mut request = self
            .request(method, table)?
            .header("Prefer", "return=representation")
            .json(&body);
        if let Some(id) = id {
            request = request.query(&[("id", format!("eq.{id}"))]);
            if table != "companies" {
                request = request.query(&[("company_id", format!("eq.{}", current.company.id))]);
            }
        }
        if table.starts_with("rpc/") {
            let result: Value = decode(request.send().context("Could not save the leave change. Check your connection and refresh before retrying.")?)?;
            ensure!(
                !result.is_null(),
                "No leave record changed. Refresh before trying again."
            );
            return self.snapshot().context(
                "Leave change saved, but refresh failed. Click Refresh before trying again.",
            );
        }
        let rows: Vec<Value> = decode(
            request
                .send()
                .context("Could not save the change. Check your connection.")?,
        )?;
        ensure!(
            rows.len() == 1,
            "No record changed. Refresh and check your workspace permissions."
        );
        self.snapshot()
            .context("Change saved, but refresh failed. Click Refresh before trying again.")
    }

    pub fn execute_admin(
        &mut self,
        action: AdminAction,
        current: &Snapshot,
    ) -> Result<(Snapshot, String)> {
        ensure!(
            current.access.role.manages_accounts(),
            "Only administrators can manage accounts."
        );
        let message = match action {
            AdminAction::SaveStaff(mut draft, employee, contacts) => {
                draft.employee_name = employee.full_name.trim().into();
                draft.employee_id = employee.id;
                draft.validate(current)?;
                let record = employee.employee(current.company.id)?;
                crate::employment::validate_assignments(current, &record)?;
                let mut person = serde_json::to_value(record)?;
                person["id"] = json!(employee.id);
                let contacts = contacts.normalized()?;
                ensure!(
                    contacts.company_id == current.company.id,
                    "Choose an employee in this company."
                );
                if draft.user_id.is_none() && draft.create_new {
                    let response=self.account_request(Method::POST,"/functions/v1/account-admin")?
                        .json(&json!({"operation":"create","email":draft.email.trim(),"password":draft.password,
                            "active":draft.active,"role":draft.role,"employee_name":employee.full_name.trim(),"employee":person,"contacts":contacts})).send()?;
                    let result: Value = decode(response)?;
                    if result["confirmation_sent"] == true {
                        "Employee and staff role saved. Confirm the email before signing in."
                    } else {
                        "Employee and staff role saved. Confirmation delivery is pending; use Send confirmation to retry."
                    }
                } else {
                    let response=self.request(Method::POST,"rpc/save_staff_account")?
                        .json(&json!({"p_user_id":draft.user_id,"p_email":draft.email.trim(),"p_role":draft.role,
                            "p_active":draft.active,"p_employee":person,"p_contacts":contacts})).send()?;
                    let _: Uuid = decode(response)?;
                    "Employee details and staff access saved."
                }
            }
            AdminAction::Save(draft) => {
                draft.validate(current)?;
                let body = json!({"p_user_id":draft.user_id,"p_email":draft.email.trim(),"p_role":draft.role,
                    "p_active":draft.active,"p_employee_id":draft.employee_id,"p_manager_user_id":draft.manager_user_id});
                if draft.user_id.is_none() && draft.create_new {
                    let response = self.account_request(Method::POST, "/functions/v1/account-admin")?
                        .json(&json!({"operation":"create","email":draft.email.trim(),"password":draft.password,"active":draft.active,
                            "role":draft.role,"employee_id":draft.employee_id,"employee_name":draft.employee_id
                            .and_then(|id| current.employees.iter().find(|e| e.id == id)).map(|e| e.full_name.as_str()).unwrap_or(&draft.employee_name)})).send()?;
                    let result: Value = decode(response)?;
                    if result.get("confirmation_sent").and_then(Value::as_bool) == Some(true) {
                        if draft.role.uses_desktop() {
                            "Account created. The user must confirm their email before signing in."
                        } else {
                            "Employee account created and confirmation email sent. The employee portal is planned; this account has no desktop access."
                        }
                    } else {
                        if draft.role.uses_desktop() {
                            "Account created, but the confirmation email was not sent. Use Send confirmation to retry."
                        } else {
                            "Employee account created for the planned portal. Confirmation email was not sent; use Send confirmation to retry. No desktop access is granted."
                        }
                    }
                } else {
                    let response = self
                        .request(Method::POST, "rpc/save_company_account")?
                        .json(&body)
                        .send()?;
                    let _: Uuid = decode(response)?;
                    "Account access saved."
                }
            }
            AdminAction::ResendConfirmation(user_id) => {
                let response = self
                    .account_request(Method::POST, "/functions/v1/account-admin")?
                    .json(&json!({"operation":"resend","user_id":user_id}))
                    .send()?;
                let _: Value = decode(response)?;
                "Confirmation email sent."
            }
        };
        Ok((
            self.snapshot()
                .context("Account change saved, but refresh failed. Refresh before retrying.")?,
            message.into(),
        ))
    }

    pub fn sign_out(&mut self) -> Result<()> {
        self.sign_out_scope("local")
    }

    fn sign_out_scope(&mut self, scope: &str) -> Result<()> {
        self.ensure_session()?;
        let response = self
            .http
            .post(format!(
                "{}/auth/v1/logout",
                self.config.url.trim().trim_end_matches('/')
            ))
            .query(&[("scope", scope)])
            .header("apikey", self.config.publishable_key.trim())
            .bearer_auth(&self.session.access_token)
            .send()
            .context("Could not reach your company workspace to revoke the requested sessions.")?;
        ensure!(
            response.status().is_success(),
            "The server could not revoke the requested sessions. Try again."
        );
        Ok(())
    }
}

fn decode<T: DeserializeOwned>(response: Response) -> Result<T> {
    let status = response.status();
    if !status.is_success() {
        let value: Value = response.json().unwrap_or(Value::Null);
        let code = value
            .get("code")
            .or_else(|| value.get("error_code"))
            .and_then(Value::as_str)
            .unwrap_or("");
        let message = match (status, code) {
            (StatusCode::UNAUTHORIZED, _) => {
                "Sign-in or session validation failed. Check your account and sign in again.".into()
            }
            (_, "23505") => "That record already exists. Refresh before trying again.".into(),
            (_, "23P01") => {
                if value
                    .get("message")
                    .and_then(Value::as_str)
                    .is_some_and(|m| {
                        m.starts_with("These times overlap another attendance record.")
                    })
                {
                    "These times overlap another attendance record. Review both shifts before saving.".into()
                } else {
                    "These dates overlap an existing pending or approved leave request.".into()
                }
            }
            (_, "42501") => "This operation is not permitted for your company workspace.".into(),
            (_, "PGRST202" | "PGRST205" | "42P01") => {
                "Your company workspace is not ready. Contact your HRIS administrator.".into()
            }
            _ => value
                .get("message")
                .or_else(|| value.get("msg"))
                .and_then(Value::as_str)
                .map(|s| s.chars().take(240).collect::<String>())
                .unwrap_or_else(|| {
                    format!(
                        "HRIS could not complete the request (HTTP {}).",
                        status.as_u16()
                    )
                }),
        };
        bail!("{message}");
    }
    response.json().context(
        "The HRIS server returned an unexpected response. Try again or contact your administrator.",
    )
}

pub enum Command {
    LeaveImportHistory,
    PreviewLeaveImport(crate::leave::request_import::Batch),
    PostLeaveImport(Box<crate::leave::request_import::Post>),
    LeaveImportErrors{source:crate::leave::request_import::Import,preview:Option<crate::leave::request_import::Preview>,destination:std::path::PathBuf},
    AttendanceImportHistory,
    PreviewAttendanceImport(crate::attendance_imports::Batch),
    PostAttendanceImport(Box<crate::attendance_imports::Post>),
    WithdrawAttendanceImport(crate::attendance_imports::Withdraw),
    AttendanceImportErrors{source:crate::attendance_imports::Import,preview:Option<crate::attendance_imports::Preview>,destination:std::path::PathBuf},
    OpeningImportErrors {report:crate::leave::opening_import::Import,destination:std::path::PathBuf},
    PreviewLeaveOpeningImport(Vec<crate::leave::dated::OpeningRecord>),
    PreviewLeaveOpening(crate::leave::dated::OpeningRecord),
    GovernmentBirDeclarations {year:i32,source:std::path::PathBuf,expected_hash:Option<String>,destination:Option<std::path::PathBuf>},
    GovernmentBirIdentities {year:i32,source:std::path::PathBuf,expected_hash:Option<String>,destination:Option<std::path::PathBuf>},
    GovernmentAnnual {year:i32,destination:Option<std::path::PathBuf>},
    GovernmentAnnualPay {year:i32,destination:Option<std::path::PathBuf>},
    GovernmentSssPrepare {filter:crate::payroll_government::Filter},
    GovernmentBirCheck {source:std::path::PathBuf,expected_hash:Option<String>,destination:Option<std::path::PathBuf>},
    GovernmentBirBuild(crate::payroll_government::bir_file::builder::Action),
    GovernmentSssExport {filter:crate::payroll_government::Filter,request:Box<crate::payroll_government::sss_collection::Request>,destination:std::path::PathBuf},
    GovernmentPreparation {filter:crate::payroll_government::Filter,destination:std::path::PathBuf},
    GovernmentRegistration {employee_id:Option<Uuid>,save:Option<crate::payroll_government::registration::Save>},
    Government {filter:crate::payroll_government::Filter,action:Option<crate::payroll_government::Action>,destination:Option<std::path::PathBuf>},
    PayrollReport {filter:crate::payroll_reporting::Filter,destination:Option<std::path::PathBuf>},
    PayrollExchange(crate::payroll_exchange::Operation),
    PayrollExport { kind:String, record_id:Option<Uuid>, year:i32, destination:std::path::PathBuf },
    ImportErrors {
        preview: crate::imports::Preview,
        destination: std::path::PathBuf,
    },
    Report {
        filter: crate::reports::Filter,
        destination: Option<std::path::PathBuf>,
    },
    Document(DocumentAction),
    Resume(crate::recruiting::ResumeAction),
    CheckUpdates,
    InstallUpdate(VerifiedRelease),
    Change(Action),
    Refresh,
    SignOut,
    SignOutEverywhere,
    Account(AccountAction),
    Admin(AdminAction),
    StaffRoles(crate::staff_roles::admin::Operation),
    StaffProvision(crate::staff_roles::provision::Operation),
    Connect {
        config: ConnectionConfig,
        email: String,
        password: String,
    },
}

pub struct Update {
    pub staff_provision: Option<crate::staff_roles::provision::Update>,
    pub staff_provision_rejection: Option<crate::staff_roles::provision::Rejection>,
    pub staff_role_rejected: bool,
    pub staff_roles: Option<crate::staff_roles::admin::Update>,
    pub leave_import_history:Option<crate::leave::request_import::History>,
    pub attendance_import_history:Option<crate::attendance_imports::History>,
    pub leave_import_preview:Option<crate::leave::request_import::Preview>,
    pub attendance_import_preview:Option<crate::attendance_imports::Preview>,
    pub leave_import_receipt:Option<crate::leave::request_import::Receipt>,
    pub attendance_import_receipt:Option<crate::attendance_imports::Receipt>,
    pub leave_opening_import_preview:Option<crate::leave::opening_import::BatchPreview>,
    pub leave_opening_preview:Option<crate::leave::dated::Preview>,
    pub government_bir_declarations:Option<crate::payroll_government::bir_declarations::Report>,
    pub government_bir_identities:Option<crate::payroll_government::bir_identity::Report>,
    pub government_annual:Option<crate::payroll_government::annual::Ledger>,
    pub government_annual_pay:Option<crate::payroll_government::annual_pay::Report>,
    pub government_sss:Option<crate::payroll_government::sss_collection::Basis>,
    pub government_bir:Option<crate::payroll_government::bir_file::Report>,
    pub government_bir_build:Option<crate::payroll_government::bir_file::builder::Preview>,
    pub government_registration:Option<crate::payroll_government::registration::Snapshot>,
    pub government:Option<crate::payroll_government::Reconciliation>,
    pub payroll_report:Option<crate::payroll_reporting::Report>,
    pub payroll_exchange:Option<crate::payroll_exchange::Update>,
    pub report: Option<crate::reports::Report>,
    pub result: std::result::Result<Snapshot, String>,
    pub message: String,
    pub account: Option<AccountProfile>,
    pub update_state: Option<UpdateState>,
    pub background: bool,
}

pub fn worker(
    initial: Option<DemoStore>,
    repaint: eframe::egui::Context,
) -> (mpsc::Sender<Command>, mpsc::Receiver<Update>) {
    let (commands, receiver) = mpsc::channel();
    let (events, updates) = mpsc::channel();
    thread::spawn(move || {
        let mut demo = initial;
        let mut demo_documents = std::collections::HashMap::new();
        let mut demo_resumes = std::collections::HashMap::new();
        let mut demo_leave_imports=crate::leave::request_import::Demo::default();
        let mut demo_attendance_imports=crate::attendance_imports::Demo::default();
        let mut demo_account = demo.as_ref().map(|_| AccountProfile::demo());
        let mut cloud: Option<CloudClient> = None;
        let mut current = demo
            .as_ref()
            .map(|store| store.data.clone())
            .unwrap_or_else(Snapshot::signed_out);
        loop {
            let command = match receiver.recv_timeout(Duration::from_secs(60)) {
                Ok(command) => command,
                Err(mpsc::RecvTimeoutError::Timeout) if cloud.is_some() => Command::Refresh,
                Err(mpsc::RecvTimeoutError::Timeout) => continue,
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
            };
            let mut message = String::new();
            let mut opening_preview_result=None;
            let mut leave_import_history_result=None;
            let mut attendance_import_history_result=None;
            let mut leave_import_preview_result=None;
            let mut attendance_import_preview_result=None;
            let mut leave_import_receipt_result=None;
            let mut attendance_import_receipt_result=None;
            let mut opening_import_preview_result=None;
            let mut staff_roles_result = None;
            let mut staff_provision_result = None;
            let mut report_result = None;
            let mut payroll_report_result = None;
            let mut payroll_exchange_result = None;
            let mut government_result = None;
            let mut annual_result = None;
            let mut annual_pay_result = None;
            let mut bir_identities_result = None;
            let mut bir_declarations_result = None;
            let mut registration_result = None;
            let mut sss_result=None;
            let mut bir_result=None;
            let mut bir_build_result=None;
            let mut update_state = None;
            let mut check_after_sign_in = false;
            let mut result: Result<Snapshot> = match command {
                Command::PayrollExchange(operation)=>{
                    let changed=(||{let client=cloud.as_mut().context("Sign in to the configured company to record payroll exchanges.")?;let data=client.snapshot()?;let update=client.payroll_exchange(&data,&operation)?;Ok((data,update))})();
                    changed.map(|(data,update)|{message=if let Some(attempt)=&update.resolved{if attempt.committed{"Retained attempt was already committed; original history is preserved.".into()}else{"No committed attempt found. Its old identity is now closed against delayed requests; review the draft before submitting again.".into()}}else{"Payroll exchange refreshed. No payroll payments changed.".into()};payroll_exchange_result=Some(update);data})
                }
                Command::LeaveImportHistory=>{
                    let changed=match cloud.as_mut(){Some(client)=>client.leave_import_history(),None=>demo.as_ref().context("Sign in to view leave import history.").and_then(|s|demo_leave_imports.history(&s.data).map(|h|(s.data.clone(),h)))};
                    changed.map(|(data,h)|{leave_import_history_result=Some(h);message="Leave import history refreshed.".into();data})
                }
                Command::PreviewLeaveImport(batch)=>{
                    let changed=match cloud.as_mut(){Some(client)=>client.leave_import_preview(&batch),None=>demo.as_ref().context("Sign in to preview leave imports.").and_then(|s|crate::leave::request_import::Demo::preview(&s.data,&batch).map(|p|(s.data.clone(),p)))};
                    changed.map(|(data,p)|{leave_import_preview_result=Some(p);message="Protected leave preview ready. No requests have been created.".into();data})
                }
                Command::PostLeaveImport(post)=>{
                    let changed=match cloud.as_mut(){Some(client)=>client.post_leave_import(&post),None=>demo.as_mut().context("Sign in to import attendance requests.").and_then(|s|{let r=demo_leave_imports.post(&mut s.data,&post)?;r.validate(&s.data,&post)?;let h=demo_leave_imports.history(&s.data)?;Ok((s.data.clone(),r,h))})};
                    changed.map(|(data,r,h)|{message=format!("{} leave requests imported with normal charge and approval snapshots.",r.row_count);leave_import_receipt_result=Some(r);leave_import_history_result=Some(h);data})
                }
                Command::AttendanceImportHistory=>{
                    let changed=match cloud.as_mut(){Some(client)=>client.attendance_import_history(),None=>demo.as_ref().context("Sign in to view attendance import history.").and_then(|s|demo_attendance_imports.history(&s.data).map(|h|(s.data.clone(),h)))};
                    changed.map(|(data,h)|{attendance_import_history_result=Some(h);message="Attendance import history refreshed.".into();data})
                }
                Command::PreviewAttendanceImport(batch)=>{
                    let changed=match cloud.as_mut(){Some(client)=>client.attendance_import_preview(&batch),None=>demo.as_ref().context("Sign in to preview attendance imports.").and_then(|s|crate::attendance_imports::Demo::preview(&s.data,&batch).map(|p|(s.data.clone(),p)))};
                    changed.map(|(data,p)|{attendance_import_preview_result=Some(p);message="Protected attendance preview ready. No requests have been created.".into();data})
                }
                Command::PostAttendanceImport(post)=>{
                    let changed=match cloud.as_mut(){Some(client)=>client.post_attendance_import(&post),None=>demo.as_mut().context("Sign in to import attendance requests.").and_then(|s|{let r=demo_attendance_imports.post(&mut s.data,&post)?;r.validate(&s.data,&post)?;let h=demo_attendance_imports.history(&s.data)?;Ok((s.data.clone(),r,h))})};
                    changed.map(|(data,r,h)|{message=format!("{} correction requests imported for normal review. Attendance is unchanged until approval.",r.row_count);attendance_import_receipt_result=Some(r);attendance_import_history_result=Some(h);data})
                }
                Command::WithdrawAttendanceImport(change)=>{
                    let changed=match cloud.as_mut(){Some(client)=>client.withdraw_attendance_import(&change),None=>demo.as_mut().context("Sign in to withdraw imported requests.").and_then(|s|{demo_attendance_imports.withdraw(&mut s.data,&change)?;Ok((s.data.clone(),demo_attendance_imports.history(&s.data)?))})};
                    changed.map(|(data,h)|{attendance_import_history_result=Some(h);message="Pending imported request withdrawn; its audit history remains.".into();data})
                }
                Command::LeaveImportErrors{source,preview,destination}=>(||{
                    ensure!(source.scope==current.access&&current.access.allows(crate::staff_roles::Capability::Personnel),"Import access changed. Reopen the source before exporting private review rows.");
                    let bytes=source.error_bytes(&destination,preview.as_ref())?;
                    if let Some(client)=cloud.as_mut(){if client.current_access()?!=current.access{return Err(AccessRevoked.into());}}
                    crate::reports::save_export(&destination,&bytes)?;message="Leave row review saved to a new file.".into();Ok(current.clone())
                })(),
                Command::AttendanceImportErrors{source,preview,destination}=>(||{
                    ensure!(source.scope==current.access&&current.access.allows(crate::staff_roles::Capability::Personnel),"Import access changed. Reopen the source before exporting private review rows.");
                    let bytes=source.error_bytes(&destination,preview.as_ref())?;
                    if let Some(client)=cloud.as_mut(){if client.current_access()?!=current.access{return Err(AccessRevoked.into());}}
                    crate::reports::save_export(&destination,&bytes)?;message="Attendance row review saved to a new file.".into();Ok(current.clone())
                })(),
                Command::OpeningImportErrors{report,destination}=>(||{
                    let bytes=if crate::tabular::is_xlsx(&destination){report.error_xlsx()?}else{report.error_csv()?};
                    match cloud.as_mut(){Some(client)=>{let access=client.current_access()?;if access!=current.access{return Err(AccessRevoked.into());}ensure!(access.allows(crate::staff_roles::Capability::Personnel),"HR access is required to export opening review rows.");},None=>ensure!(demo.as_ref().is_some_and(|s|s.data.mode==Mode::Demo&&s.data.access.allows(crate::staff_roles::Capability::Personnel)),"Sign in to HRIS with HR access before exporting opening reviews.")}
                    crate::reports::save_export(&destination,&bytes)?;message="Opening row review exported.".into();Ok(current.clone())
                })(),
                Command::PreviewLeaveOpeningImport(records)=>{
                    let changed=match cloud.as_mut(){Some(client)=>client.leave_opening_import_preview(&records),None=>demo.as_mut().context("Sign in to preview opening imports.").map(|store|{
                        if store.data.leave_setup.dated.is_none(){store.data.leave_setup.dated=Some(crate::leave::dated::Context::empty(&store.data));}
                        (store.data.clone(),crate::leave::opening_import::BatchPreview::demo(&store.data,&records))
                    })};changed.map(|(data,p)|{opening_import_preview_result=Some(p);message="Opening import preview ready. No rows have been applied.".into();data})
                }
                Command::PreviewLeaveOpening(record)=>{
                    let changed=match cloud.as_mut() {
                        Some(client)=>client.leave_opening_preview(&record),
                        None=>demo.as_mut().context("Sign in to preview leave openings.").and_then(|store|{
                            if store.data.leave_setup.dated.is_none(){store.data.leave_setup.dated=Some(crate::leave::dated::Context::empty(&store.data));}
                            crate::leave::dated::preview_demo(&store.data,&record).map(|p|(store.data.clone(),p))
                        }),
                    };
                    changed.map(|(data,p)|{opening_preview_result=Some(p);message="Opening preview ready. No balance has been posted.".into();data})
                }
                Command::CheckUpdates => {
                    update_state = Some(match (cloud.as_mut(), UpdateConfig::load()) {
                        (Some(client), Ok(Some(config))) => client
                            .check_update(&config)
                            .unwrap_or_else(|e| UpdateState::Failed(e.to_string())),
                        (_, Err(e)) => UpdateState::Failed(e.to_string()),
                        (None, _) => {
                            UpdateState::Failed("Sign in to check your company's updates.".into())
                        }
                        _ => UpdateState::Failed(
                            "Update verification has not been configured for this build.".into(),
                        ),
                    });
                    Ok(current.clone())
                }
                Command::InstallUpdate(update) => {
                    let install = cloud
                        .as_mut()
                        .context("Sign in before updating.")
                        .and_then(|c| c.install_update(&update));
                    update_state = Some(match install {
                        Ok(()) => UpdateState::Restarting,
                        Err(e) => UpdateState::Failed(e.to_string()),
                    });
                    Ok(current.clone())
                }
                Command::Connect {
                    config,
                    email,
                    password,
                } => match CloudClient::connect(config, email, password) {
                    Ok((client, data)) => {
                        cloud = Some(client);
                        check_after_sign_in = true;
                        update_state = Some(UpdateState::Idle);
                        message = "Signed in to your company workspace.".into();
                        Ok(data)
                    }
                    Err(error) => Err(error),
                },
                Command::SignOut | Command::SignOutEverywhere => {
                    demo_documents.clear();
                    demo_resumes.clear();
                    update_state = Some(UpdateState::Idle);
                    let everywhere = matches!(command, Command::SignOutEverywhere);
                    if let Some(mut client) = cloud.take() {
                        if let Err(error) =
                            client.sign_out_scope(if everywhere { "global" } else { "local" })
                        {
                            message = format!("Local session cleared. {error}");
                        }
                    }
                    if message.is_empty() {
                        message = "Signed out.".into();
                    }
                    Ok(match demo.as_mut() {
                        Some(store) => {
                            *store = DemoStore::new();
                            demo_account = Some(AccountProfile::demo());
                            store.data.clone()
                        }
                        None => Snapshot::signed_out(),
                    })
                }
                Command::Refresh => match cloud.as_mut() {
                    Some(client) => client.snapshot().and_then(|data| {
                        client.refresh_account()?;
                        Ok(data)
                    }),
                    None => demo
                        .as_ref()
                        .map(|store| store.data.clone())
                        .context("Sign in to HRIS to access company records."),
                },
                Command::Change(action) => {
                    message = if cloud.is_some() {
                        "Saved to your company workspace."
                    } else {
                        "Updated in the demo workspace."
                    }
                    .into();
                    match cloud.as_mut() {
                        Some(client) => client.execute(action, &current),
                        None => demo
                            .as_mut()
                            .context("Sign in to HRIS to access company records.")
                            .and_then(|store| store.execute(action).map(|_| store.data.clone())),
                    }
                }
                Command::ImportErrors {
                    preview,
                    destination,
                } => {
                    message = "Import error CSV saved. No employee records were changed.".into();
                    (|| -> Result<Snapshot> {
                        if let Some(client) = cloud.as_mut() {
                            let access = client.current_access()?;
                            if !access.allows(crate::staff_roles::Capability::Personnel) || access.company_id != preview.company_id
                            {
                                return Err(AccessRevoked.into());
                            }
                        } else {
                            ensure!(
                                demo.as_ref()
                                    .is_some_and(|s| s.data.access.allows(crate::staff_roles::Capability::Personnel)
                                        && s.data.company.id == preview.company_id),
                                "Only HR or Administrators can export import errors."
                            );
                        }
                        let bytes=if crate::tabular::is_xlsx(&destination){preview.error_xlsx()?}else{preview.error_csv()?};
                        if let Some(client)=cloud.as_mut(){if client.current_access()? != current.access {return Err(AccessRevoked.into());}}
                        crate::reports::save_export(&destination, &bytes)?;
                        Ok(current.clone())
                    })()
                }
                Command::GovernmentBirDeclarations{year,source,expected_hash,destination}=>{
                    (||{let client=cloud.as_mut().context("Sign in to review BIR declarations and sources.")?;let data=client.snapshot()?;
                        bir_declarations_result=Some(client.government_bir_declarations(&data,year,&source,expected_hash.as_deref(),destination.as_deref())?);
                        message=if destination.is_some(){"Fresh internal declaration/source worksheet exported. No expected tax or return was calculated.".into()}
                            else{"Literal BIR declarations and recorded payroll sources loaded separately. Official validation remains unverified.".into()};Ok(data)})()
                }
                Command::GovernmentBirIdentities{year,source,expected_hash,destination}=>{
                    (||{let client=cloud.as_mut().context("Sign in to review current BIR identities.")?;let data=client.snapshot()?;
                        bir_identities_result=Some(client.government_bir_identities(&data,year,&source,expected_hash.as_deref(),destination.as_deref())?);
                        message=if destination.is_some(){"Fresh internal identity review exported. No tax return, payroll or registration was changed.".into()}
                            else{"Current BIR identity pairs reviewed. Historical assignment and expected tax remain unverified.".into()};Ok(data)})()
                }
                Command::GovernmentAnnualPay{year,destination}=>{
                    (||{let client=cloud.as_mut().context("Sign in to review recorded pay details.")?;
                        let data=client.snapshot()?;
                        annual_pay_result=Some(client.government_annual_pay_details(&data,year,destination.as_deref())?);
                        message=if destination.is_some(){"Fresh recorded pay details exported. Review overlapping components and receipt states separately.".into()}
                            else{"Annual sources and issued pay details refreshed together.".into()};Ok(data)})()
                }
                Command::GovernmentAnnual{year,destination}=>{
                    (||{let client=cloud.as_mut().context("Sign in to generate annual recorded sources.")?;
                        let data=client.snapshot()?;
                        annual_result=Some(client.government_annual_ledger(&data,year,destination.as_deref())?);
                        message=if destination.is_some(){"Fresh internal annual source worksheet exported. No tax return or payroll payment was changed.".into()}
                            else{"Annual recorded sources refreshed. Review actual receipt dates and excluded records.".into()};
                        Ok(data)})()
                }
                Command::GovernmentSssPrepare{filter}=>{
                    (||->Result<Snapshot>{
                        let client=cloud.as_mut().context("Company sign-in is required for SSS drafts.")?;
                        sss_result=Some(client.sss_collection_prepare(&current,filter)?);
                        message="SSS draft sources checked against current registration and recorded remittance.".into();Ok(current.clone())
                    })()
                }
                Command::GovernmentBirBuild(action)=>{
                    (||->Result<Snapshot>{
                        let client=cloud.as_mut().context("Company sign-in is required for BIR declaration drafts.")?;
                        bir_build_result=client.government_bir_build(&current,&action)?;
                        message=match &action {
                            crate::payroll_government::bir_file::builder::Action::Template{..}=>"Blank BIR declaration template saved. Enter reviewed values; no payroll or tax classifications were inferred.",
                            crate::payroll_government::bir_file::builder::Action::Prepare{destination:Some(_),..}=>"New BIR plaintext draft saved. Official BIR validation and agency acceptance are still required; nothing was submitted.",
                            _=>"Declaration draft previewed locally. Review the declared values before saving a new DAT file.",
                        }.into();
                        Ok(current.clone())
                    })()
                }
                Command::GovernmentBirCheck{source,expected_hash,destination}=>{
                    (||->Result<Snapshot>{
                        let client=cloud.as_mut().context("Company sign-in is required for BIR file checks.")?;
                        bir_result=Some(client.government_bir_check(&current,&source,expected_hash.as_deref(),destination.as_deref())?);
                        message=if destination.is_some(){"Fresh BIR structure findings saved. No source file, payroll or filing was changed."}else{"BIR file structure checked locally. Official validation and payroll comparison are separate."}.into();
                        Ok(current.clone())
                    })()
                }
                Command::GovernmentSssExport{filter,request,destination}=>{
                    (||->Result<Snapshot>{
                        let client=cloud.as_mut().context("Company sign-in is required for SSS drafts.")?;
                        government_result=Some(client.sss_collection_export(&current,filter,&request,&destination)?);
                        message="SSS reference draft saved for review. No collection list was submitted and no payment was recorded.".into();Ok(current.clone())
                    })()
                }
                Command::GovernmentPreparation{filter,destination}=>{
                    (||->Result<Snapshot>{
                        let client=cloud.as_mut().context("Company sign-in is required for agency preparation exports.")?;
                        government_result=Some(client.government_preparation(&current,filter,&destination)?);
                        message="Agency preparation worksheet exported with current registered identifiers. This is an internal HR file, not an agency upload.".into();
                        Ok(current.clone())
                    })()
                }
                Command::GovernmentRegistration{employee_id,save}=>{
                    (||->Result<Snapshot>{
                        let client=cloud.as_mut().context("Company sign-in is required for registration records.")?;
                        registration_result=Some(client.government_registration(&current,employee_id,save.as_ref())?);
                        message=if save.is_some(){"Registration saved with retained history. Employment and branding are unchanged.".into()}else{"Current registration records loaded.".into()};
                        Ok(current.clone())
                    })()
                }
                Command::Government{filter,action,destination}=>{
                    (||->Result<Snapshot>{
                        let client=cloud.as_mut().context("Company sign-in is required for government reconciliation.")?;
                        let report=client.government_reconciliation(&current,filter,action.as_ref(),destination.as_deref())?;
                        message=if action.is_some(){"Government record saved. Review the current status and audit history.".into()}else if destination.is_some(){"Fresh government review worksheet exported. References and evidence are kept private.".into()}else{"Government reconciliation refreshed from current records.".into()};
                        government_result=Some(report);Ok(current.clone())
                    })()
                }
                Command::PayrollReport{filter,destination}=>{
                    (||->Result<Snapshot>{
                        let report=if let Some(client)=cloud.as_mut(){client.consolidated_payroll_report(&current,&filter,destination.as_deref())?}else{
                            let d=&demo.as_ref().context("Sign in to generate payroll reports.")?.data;
                            let report=crate::payroll_reporting::demo(d,&filter,Utc::now())?;
                            if let Some(path)=destination.as_deref(){crate::reports::save_csv(path,&report.csv()?)?;}
                            report
                        };
                        message=format!("{} {} payroll fact(s). No payment or filing was made.",if destination.is_some(){"Exported"}else{"Generated"},report.wire.rows.len());
                        payroll_report_result=Some(report);Ok(current.clone())
                    })()
                }
                Command::PayrollExport { kind,record_id,year,destination } => {
                    (|| -> Result<Snapshot> {
                        let data=match cloud.as_mut(){Some(client)=>client.snapshot()?,None=>demo.as_ref().context("Sign in to export payroll.")?.data.clone()};
                        ensure!(data.access.company_id==current.access.company_id && data.access.user_id==current.access.user_id,"Your account scope changed. Sign in again.");
                        let bytes=if kind=="amendment_statement_pdf"{let s=data.payroll.amendments.slips.iter().find(|s|Some(s.id)==record_id&&s.company_id==data.company.id&&(data.access.allows(crate::staff_roles::Capability::PayrollReports)||data.access.employee_id==Some(s.employee_id))).context("Only an available own issued amendment may be exported.")?;crate::payroll_pdf::build_amendment(s,crate::payroll_amendments::status(&data,s.id))?}else if kind.starts_with("amendment_"){crate::payroll_amendments::export_csv(&data,&kind,record_id)?}else if kind=="final_statement_pdf"{let s=data.payroll.finals.slips.iter().find(|s|Some(s.id)==record_id&&s.company_id==data.company.id).context("Issued final statement unavailable.")?;ensure!(data.access.allows(crate::staff_roles::Capability::PayrollReports)||data.access.employee_id==Some(s.employee_id),"Only your own final statement is available.");crate::payroll_pdf::build_final(s,crate::payroll_final::status(&data,s.id))?}else if kind.starts_with("final_"){crate::payroll_final::export_csv(&data,&kind,record_id)?}else if kind=="tax_payslip_pdf"{let s=data.payroll.tax.slips.iter().find(|s|Some(s.id)==record_id&&s.company_id==data.company.id).context("Issued tax advice unavailable.")?;ensure!(data.access.allows(crate::staff_roles::Capability::PayrollReports)||data.access.employee_id==Some(s.employee_id),"Only your own tax advice is available.");crate::payroll_pdf::build_tax(s)?}else if kind.starts_with("tax_"){crate::payroll_tax::export_csv(&data,&kind,record_id)?}else if kind=="thirteenth_payslip_pdf"{let s=data.payroll.thirteenth.slips.iter().find(|s|Some(s.id)==record_id&&s.company_id==data.company.id).context("Issued annual benefit slip unavailable.")?;ensure!(data.access.allows(crate::staff_roles::Capability::PayrollReports)||data.access.employee_id==Some(s.employee_id),"Only your own annual benefit slip is available.");crate::payroll_pdf::build_thirteenth(s)?}else if kind.starts_with("thirteenth_"){crate::payroll_thirteenth::export_csv(&data,&kind,record_id)?}else if kind=="cutoff_payslip_pdf"{let s=data.payroll.cutoffs.slips.iter().find(|s|Some(s.base.id)==record_id).context("Issued cutoff payslip unavailable.")?;ensure!(s.base.company_id==data.company.id&&(data.access.allows(crate::staff_roles::Capability::PayrollReports)||data.access.employee_id==Some(s.base.employee_id)),"Only your own cutoff payslip is available.");crate::payroll_pdf::build_cutoff(s)?}else if kind.starts_with("cutoff_"){crate::payroll_cutoffs::export_csv(&data,&kind,record_id)?}else if kind=="payslip_pdf"{let slip=data.payroll.slips.iter().find(|s|Some(s.id)==record_id).context("Approved payslip not available.")?;ensure!(data.access.allows(crate::staff_roles::Capability::PayrollReports)||data.access.employee_id==Some(slip.employee_id),"Only your own payslip is available.");crate::payroll_pdf::build(slip)?}else{crate::payroll::export_csv(&data,&kind,record_id,year)?};
                        if let Some(client)=cloud.as_mut(){let access=client.current_access()?;if access!=data.access{return Err(AccessRevoked.into());}}
                        if kind.ends_with("payslip_pdf")||kind=="final_statement_pdf"||kind=="amendment_statement_pdf"{crate::payroll_pdf::save(&destination,&bytes)?;}else{crate::reports::save_csv(&destination,&bytes)?;}
                        message="Payroll export saved from freshly checked access. No payment or government filing was made.".into();
                        Ok(data)
                    })()
                }
                Command::Report {
                    filter,
                    destination,
                } => {
                    let exporting = destination.is_some();
                    let changed = match cloud.as_mut() {
                        Some(client) => client.report(&filter, destination.as_deref()),
                        None => demo
                            .as_ref()
                            .context("Sign in to run company reports.")
                            .and_then(|store| {
                                let report =
                                    crate::reports::build(&store.data, &filter, Utc::now())?;
                                if let Some(path) = destination.as_deref() {
                                    report.save(path)?;
                                }
                                Ok((store.data.clone(), report))
                            }),
                    };
                    changed.map(|(data, report)| {
                        message = format!(
                            "{} {} row(s).",
                            if exporting { "Exported" } else { "Generated" },
                            report.rows.len()
                        );
                        report_result = Some(report);
                        data
                    })
                }
                Command::Resume(action) => {
                    let changed = match cloud.as_mut() {
                        Some(client) => client.execute_resume(action, &current),
                        None => demo
                            .as_mut()
                            .context("Sign in to manage application attachments.")
                            .and_then(|store| {
                                crate::recruiting::resume_demo(
                                    &mut store.data,
                                    &mut demo_resumes,
                                    action,
                                )
                                .map(|text| (store.data.clone(), text))
                            }),
                    };
                    changed.map(|(data, text)| {
                        message = text;
                        data
                    })
                }
                Command::Document(action) => {
                    let changed = match cloud.as_mut() {
                        Some(client) => client.execute_document(action, &current),
                        None => demo
                            .as_mut()
                            .context("Sign in to HRIS to manage employee documents.")
                            .and_then(|store| {
                                documents::execute_demo(
                                    &mut store.data,
                                    &mut demo_documents,
                                    action,
                                )
                                .map(|text| (store.data.clone(), text))
                            }),
                    };
                    changed.map(|(data, text)| {
                        message = text;
                        data
                    })
                }
                Command::Account(action) => {
                    let changed = match cloud.as_mut() {
                        Some(client) => client.execute_account(action),
                        None => demo_account
                            .as_mut()
                            .context("Sign in to HRIS to manage your profile.")
                            .and_then(|account| execute_demo_account(account, action)),
                    };
                    changed.map(|text| {
                        message = text;
                        current.clone()
                    })
                }
                Command::StaffProvision(operation)=>{
                    let changed=cloud.as_mut().context("Sign in to manage protected staff accounts.")
                        .and_then(|client|client.execute_staff_provision(operation,&current));
                    changed.map(|(data,update)|{message=match &update{
                        crate::staff_roles::provision::Update::Completed{..}=>"Employee and staff access saved. Use Send confirmation to deliver account setup.",
                        crate::staff_roles::provision::Update::Cancelled(_)=>"Pending account attempt closed. Its history and any reserved identity are retained.",
                        crate::staff_roles::provision::Update::Source(_)=>"Saved employee details loaded for review.",
                        crate::staff_roles::provision::Update::Pending(_)=>"Saved account attempts refreshed.",
                        crate::staff_roles::provision::Update::Status{..}=>"Account status checked. Review before replacing an attempt.",
                    }.into();staff_provision_result=Some(update);data})
                }
                Command::StaffRoles(operation) => {
                    let changed = cloud.as_mut().context("Sign in to manage protected staff assignments.")
                        .and_then(|client| client.execute_staff_roles(operation, &current));
                    changed.map(|(data,update)| {
                        message = if update.receipt.is_some() {"Staff responsibilities and sign-in access saved."} else {"Staff assignments refreshed."}.into();
                        staff_roles_result = Some(update); data
                    })
                }
                Command::Admin(action) => {
                    let changed = match cloud.as_mut() {
                        Some(client) => client.execute_admin(action, &current),
                        None => demo
                            .as_mut()
                            .context("Sign in to manage accounts.")
                            .and_then(|store| {
                                execute_demo_admin(store, action)
                                    .map(|message| (store.data.clone(), message))
                            }),
                    };
                    changed.map(|(data, text)| {
                        message = text;
                        data
                    })
                }
            };
            let access_error = result
                .as_ref()
                .err()
                .filter(|error| {
                    error.downcast_ref::<AccessRevoked>().is_some()
                        || error.downcast_ref::<PortalAccount>().is_some()
                })
                .map(|error| error.to_string());
            if let Some(error) = access_error {
                cloud = None;
                message = error;
                result = Ok(Snapshot::signed_out());
            }
            if let Ok(data) = &result {
                current = data.clone();
            }
            if events
                .send(Update {
                    staff_provision: staff_provision_result,
                    staff_provision_rejection: result.as_ref().err().and_then(|e|e.downcast_ref::<crate::staff_roles::provision::Rejection>()).cloned(),
                    staff_role_rejected: result.as_ref().err().is_some_and(|e|e.is::<staff_roles_adapter::Rejected>()),
                    staff_roles: staff_roles_result,
                    leave_import_history:leave_import_history_result,
                    attendance_import_history:attendance_import_history_result,
                    leave_import_preview:leave_import_preview_result,
                    attendance_import_preview:attendance_import_preview_result,
                    leave_import_receipt:leave_import_receipt_result,
                    attendance_import_receipt:attendance_import_receipt_result,
                    leave_opening_import_preview:opening_import_preview_result,
                    leave_opening_preview:opening_preview_result,
                    government_bir_declarations:bir_declarations_result,
                    government_bir_identities:bir_identities_result,
                    government_annual:annual_result,
                    government_annual_pay:annual_pay_result,
                    government_sss:sss_result,
                    government_bir:bir_result,
                    government_bir_build:bir_build_result,
                    government_registration:registration_result,
                    government:government_result,
                    report: report_result,
                    payroll_report:payroll_report_result,
                    payroll_exchange:payroll_exchange_result,
                    result: result.map_err(|e| e.to_string()),
                    message,
                    account: cloud
                        .as_ref()
                        .map(|client| client.account.clone())
                        .or_else(|| demo_account.clone()),
                    update_state,
                    background: false,
                })
                .is_err()
            {
                break;
            }
            repaint.request_repaint();
            // Publish HR results first; an automatic update check never clears busy state or drafts.
            if check_after_sign_in {
                if let (Some(client), Ok(Some(config))) = (cloud.as_mut(), UpdateConfig::load()) {
                    let state = client
                        .check_update(&config)
                        .unwrap_or_else(|e| UpdateState::Failed(e.to_string()));
                    if events
                        .send(Update {
                            staff_provision: None,
                            staff_provision_rejection: None,
                            staff_role_rejected: false,
                            staff_roles: None,
                            leave_opening_import_preview:None,
                            leave_import_history:None,
                            attendance_import_history:None,
                            leave_import_preview:None,
                            attendance_import_preview:None,
                            leave_import_receipt:None,
                            attendance_import_receipt:None,
                            leave_opening_preview:None,
                            government_bir_declarations:None,
                            government_bir_identities:None,
                            government_annual:None,
                            government_annual_pay:None,
                            government_sss:None,
                            government_bir:None,
                            government_bir_build:None,
                            government_registration:None,
                            government:None,
                            report: None,
                            payroll_report:None,
                            payroll_exchange:None,
                            result: Ok(current.clone()),
                            message: String::new(),
                            account: None,
                            update_state: Some(state),
                            background: true,
                        })
                        .is_err()
                    {
                        break;
                    }
                    repaint.request_repaint();
                }
            }
        }
    });
    (commands, updates)
}

fn execute_demo_admin(store: &mut DemoStore, action: AdminAction) -> Result<String> {
    ensure!(
        store.data.access.role.manages_accounts(),
        "Only administrators can manage accounts."
    );
    match action {
        AdminAction::SaveStaff(mut draft, employee, contacts) => {
            let before = store.data.clone();
            let result = (|| {
                draft.employee_name = employee.full_name.trim().into();
                let code = employee.employee_code.trim().to_uppercase();
                store.execute(Action::SaveEmployeeRecord(employee, contacts))?;
                draft.employee_id = store
                    .data
                    .employees
                    .iter()
                    .find(|e| e.employee_code == code)
                    .map(|e| e.id);
                execute_demo_admin(store, AdminAction::Save(draft))
            })();
            if result.is_err() {
                store.data = before;
            }
            result
        }
        AdminAction::Save(draft) => {
            draft.validate(&store.data)?;
            let user_id = draft.user_id.unwrap_or_else(Uuid::new_v4);
            ensure!(
                user_id != Uuid::nil()
                    || (draft.active && draft.role == crate::access::Role::Administrator),
                "The installation administrator must remain active with Administrator access."
            );
            ensure!(
                !store
                    .data
                    .accounts
                    .iter()
                    .any(|a| a.user_id != user_id
                        && a.email.eq_ignore_ascii_case(draft.email.trim())),
                "That account already exists."
            );
            ensure!(
                !draft.active
                    || draft.employee_id.is_none()
                    || !store.data.accounts.iter().any(|a| a.active
                        && a.user_id != user_id
                        && a.employee_id == draft.employee_id),
                "This employee already has an active account."
            );
            let person =
                if let Some(existing) = store.data.accounts.iter().find(|a| a.user_id == user_id) {
                    ensure!(
                        draft.employee_id.is_none() || draft.employee_id == existing.employee_id,
                        "This account already has an employee record. Edit it in Employees."
                    );
                    existing.employee_id
                } else {
                    draft.employee_id
                };
            let person = match person {
                Some(id) => id,
                None => {
                    let name = if draft.employee_name.trim().is_empty() {
                        draft
                            .email
                            .split('@')
                            .next()
                            .unwrap_or("New employee")
                            .to_owned()
                    } else {
                        draft.employee_name.trim().to_owned()
                    };
                    let employee = EmployeeDraft {
                        employee_code: format!(
                            "EMP-{}",
                            Uuid::new_v4().simple().to_string().get(..20).unwrap()
                        ),
                        full_name: name,
                        annual_leave_days: store.data.company.annual_leave_days,
                        ..Default::default()
                    };
                    let code = employee.employee_code.to_uppercase();
                    store.execute(Action::SaveEmployee(employee))?;
                    store
                        .data
                        .employees
                        .iter()
                        .find(|e| e.employee_code == code)
                        .unwrap()
                        .id
                }
            };
            ensure!(
                !(draft.active && draft.role == crate::access::Role::Employee)
                    || !store.data.active().any(|e| e.reports_to == Some(person)),
                "Reassign direct reports before changing this supervisor to Employee access."
            );
            let account = ManagedAccount {
                user_id,
                email: draft.email.trim().into(),
                role: draft.role,
                active: draft.active,
                employee_id: Some(person),
                manager_user_id: None,
                email_confirmed: true,
                is_owner: user_id == Uuid::nil(),
            };
            if let Some(old) = store
                .data
                .accounts
                .iter_mut()
                .find(|a| a.user_id == user_id)
            {
                *old = account;
            } else {
                store.data.accounts.push(account);
            }
            store.data.access_events.insert(
                0,
                AccessEvent {
                    id: Uuid::new_v4(),
                    actor_id: store.data.access.user_id,
                    user_id,
                    action: if draft.user_id.is_some() {
                        "access_changed"
                    } else {
                        "account_added"
                    }
                    .into(),
                    created_at: Utc::now().to_rfc3339(),
                    details: json!({"role":draft.role,"active":draft.active}),
                },
            );
            Ok("Demo account access updated in memory. No account or email was created.".into())
        }
        AdminAction::ResendConfirmation(_) => anyhow::bail!("Demo accounts do not send email."),
    }
}

#[cfg(test)]
mod staff_record_tests {
    use super::*;
    #[test]
    fn failed_staff_access_rolls_back_the_new_employee_and_contacts_in_demo() {
        let mut store = DemoStore::new();
        let count = store.data.employees.len();
        let accounts = store.data.accounts.len();
        let contacts_count = store.data.contacts.len();
        let employee = EmployeeDraft {
            employee_code: "STAFF-ROLLBACK".into(),
            full_name: "Synthetic Staff".into(),
            annual_leave_days: 15,
            ..Default::default()
        };
        let contacts = crate::contacts::EmployeeContacts::empty(store.data.company.id, Uuid::nil());
        let invalid = crate::access::AccountDraft {
            email: "invalid".into(),
            employee_name: "Synthetic Staff".into(),
            role: crate::access::Role::HrGeneralist,
            ..Default::default()
        };
        assert!(
            execute_demo_admin(
                &mut store,
                AdminAction::SaveStaff(invalid, employee, contacts)
            )
            .is_err()
        );
        assert_eq!(store.data.employees.len(), count);
        assert_eq!(store.data.accounts.len(), accounts);
        assert_eq!(store.data.contacts.len(), contacts_count);
    }
}

fn execute_demo_account(profile: &mut AccountProfile, action: AccountAction) -> Result<String> {
    match action {
        AccountAction::SaveDetails(details) => {
            details.validate()?;
            profile.details = details.trimmed();
        }
        AccountAction::SaveDisplay(display) => {
            display.validate()?;
            profile.display = display;
        }
        AccountAction::LoadReportViews { company_id } => {
            profile.report_views_company = Some(company_id);
        }
        AccountAction::SaveReportViews {
            company_id,
            expected_revision,
            views,
        } => {
            crate::report_views::validate_collection(&views)?;
            ensure!(
                profile.report_views_revision == expected_revision,
                "Reload saved views before saving changes."
            );
            profile.report_views = views;
            profile.report_views_company = Some(company_id);
            profile.report_views_revision += 1;
        }
        AccountAction::UploadPhoto(path) => {
            profile.photo = Some(account::photo_from_file(&path)?);
        }
        AccountAction::RemovePhoto => profile.photo = None,
        _ => bail!(
            "Account security changes require a signed-in HRIS account. Demo changes never reach a server."
        ),
    }
    Ok("Updated for this demo session. Changes are not saved.".into())
}
