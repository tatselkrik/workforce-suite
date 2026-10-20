use crate::account::{self, AccountAction, AccountProfile, DisplayPreferences, PersonalDetails};
use crate::domain::*;
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
}

impl CloudClient {
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

    fn rows<T: DeserializeOwned>(&mut self, table: &str, company: Option<Uuid>) -> Result<Vec<T>> {
        let mut all = Vec::new();
        for page in 0..200 {
            let mut request = self
                .request(Method::GET, table)?
                .query(&[("select", "*"), ("order", "id.asc")]);
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

    pub fn snapshot(&mut self) -> Result<Snapshot> {
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
            company.owner_id == self.session.user.id,
            "This HR account is not assigned to this company's records."
        );
        let mut employees: Vec<Employee> = self.rows("employees", Some(company.id))?;
        employees.sort_by(|a, b| a.full_name.cmp(&b.full_name));
        let mut attendance: Vec<Attendance> = self.rows("attendance", Some(company.id))?;
        attendance.sort_by(|a, b| b.checked_in_at.cmp(&a.checked_in_at));
        let mut leave: Vec<LeaveRequest> = self.rows("leave_requests", Some(company.id))?;
        leave.sort_by(|a, b| b.starts_on.cmp(&a.starts_on));
        Ok(Snapshot {
            company,
            employees,
            attendance,
            leave,
            mode: Mode::Supabase,
        })
    }

    pub fn execute(&mut self, action: Action, current: &Snapshot) -> Result<Snapshot> {
        let (method, table, id, body) = match action {
            Action::SaveEmployee(draft) => {
                let employee = draft.employee(current.company.id)?;
                let mut body = json!({"employee_code": employee.employee_code, "full_name":employee.full_name,
                    "department":employee.department,"job_title":employee.job_title,"joined_on":employee.joined_on,
                    "annual_leave_days":employee.annual_leave_days,"status":employee.status});
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
                let row = draft.request(current.company.id)?;
                (
                    Method::POST,
                    "leave_requests",
                    None,
                    json!({"company_id":row.company_id,"employee_id":row.employee_id,
                    "kind":row.kind,"starts_on":row.starts_on,"ends_on":row.ends_on,"reason":row.reason}),
                )
            }
            Action::DecideLeave(id, status) => {
                ensure!(status != LeaveStatus::Pending, "Choose approve or reject.");
                (
                    Method::PATCH,
                    "leave_requests",
                    Some(id),
                    json!({"status":status}),
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
                "These dates overlap an existing pending or approved leave request.".into()
            }
            (_, "42501") => "This operation is not permitted for your company workspace.".into(),
            (_, "PGRST205" | "42P01") => {
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
    Change(Action),
    Refresh,
    SignOut,
    SignOutEverywhere,
    Account(AccountAction),
    Connect {
        config: ConnectionConfig,
        email: String,
        password: String,
    },
}

pub struct Update {
    pub result: std::result::Result<Snapshot, String>,
    pub message: String,
    pub account: Option<AccountProfile>,
}

pub fn worker(
    initial: Option<DemoStore>,
    repaint: eframe::egui::Context,
) -> (mpsc::Sender<Command>, mpsc::Receiver<Update>) {
    let (commands, receiver) = mpsc::channel();
    let (events, updates) = mpsc::channel();
    thread::spawn(move || {
        let mut demo = initial;
        let mut demo_account = demo.as_ref().map(|_| AccountProfile::demo());
        let mut cloud: Option<CloudClient> = None;
        let mut current = demo
            .as_ref()
            .map(|store| store.data.clone())
            .unwrap_or_else(Snapshot::signed_out);
        while let Ok(command) = receiver.recv() {
            let mut message = String::new();
            let result: Result<Snapshot> = match command {
                Command::Connect {
                    config,
                    email,
                    password,
                } => match CloudClient::connect(config, email, password) {
                    Ok((client, data)) => {
                        cloud = Some(client);
                        message = "Signed in to your company workspace.".into();
                        Ok(data)
                    }
                    Err(error) => Err(error),
                },
                Command::SignOut | Command::SignOutEverywhere => {
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
            };
            if let Ok(data) = &result {
                current = data.clone();
            }
            if events
                .send(Update {
                    result: result.map_err(|e| e.to_string()),
                    message,
                    account: cloud
                        .as_ref()
                        .map(|client| client.account.clone())
                        .or_else(|| demo_account.clone()),
                })
                .is_err()
            {
                break;
            }
            repaint.request_repaint();
        }
    });
    (commands, updates)
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
