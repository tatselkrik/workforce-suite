use crate::domain::*;
use anyhow::{Context, Result, bail, ensure};
use chrono::Utc;
use reqwest::{
    Method, StatusCode,
    blocking::{Client, RequestBuilder, Response},
};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::{Value, json};
use std::{
    path::{Path, PathBuf},
    sync::mpsc,
    thread,
    time::Duration,
};
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

    pub fn path() -> PathBuf {
        let folder = std::env::var_os("HRIS_DATA_DIR")
            .map(PathBuf::from)
            .or_else(|| {
                std::env::var_os("LOCALAPPDATA")
                    .map(PathBuf::from)
                    .map(|p| p.join(crate::APP_ID))
            })
            .or_else(|| {
                std::env::var_os("XDG_CONFIG_HOME")
                    .map(PathBuf::from)
                    .map(|p| p.join(crate::APP_ID))
            })
            .or_else(|| {
                std::env::var_os("HOME")
                    .map(PathBuf::from)
                    .map(|p| p.join(".config").join(crate::APP_ID))
            })
            .unwrap_or_else(|| PathBuf::from("."));
        folder.join("hris-config.json")
    }

    pub fn load() -> Result<Self> {
        let company_path = std::env::current_exe().ok().and_then(|path| {
            path.parent()
                .map(|parent| parent.join("hris-connection.json"))
        });
        let mut config = Self::load_files(company_path.as_deref(), &Self::path())?;
        if let Ok(value) = std::env::var("HRIS_SUPABASE_URL") {
            config.url = value;
        }
        if let Ok(value) = std::env::var("HRIS_SUPABASE_PUBLISHABLE_KEY") {
            config.publishable_key = value;
        }
        Ok(config)
    }

    fn load_files(company_path: Option<&Path>, device_path: &Path) -> Result<Self> {
        // A packaged company profile takes precedence over settings left by another installation.
        let path = company_path
            .filter(|path| path.exists())
            .or_else(|| device_path.exists().then_some(device_path));
        match path {
            Some(path) => {
                let config: Self = serde_json::from_slice(
                    &std::fs::read(path).context("Could not read connection settings.")?
                ).context("Connection settings are invalid JSON. Correct the company profile or hris-config.json.")?;
                config.validate()?;
                Ok(config)
            }
            None => Ok(Self::default()),
        }
    }

    pub fn save(&self) -> Result<()> {
        self.validate()?;
        let path = Self::path();
        std::fs::create_dir_all(path.parent().unwrap())
            .context("Could not create the settings folder.")?;
        let temporary = path.with_extension("json.tmp");
        std::fs::write(&temporary, serde_json::to_vec_pretty(self)?)
            .context("Could not save connection settings.")?;
        std::fs::rename(temporary, path).context("Could not replace connection settings.")?;
        Ok(())
    }
}

#[derive(Deserialize)]
struct AuthUser {
    id: Uuid,
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
    fn company_profile_overrides_another_installations_device_settings() {
        let folder = std::env::temp_dir().join(format!("hris-config-test-{}", Uuid::new_v4()));
        std::fs::create_dir(&folder).unwrap();
        let company = folder.join("hris-connection.json");
        let device = folder.join("hris-config.json");
        let configured = ConnectionConfig {
            url: "https://company.example.invalid".into(),
            publishable_key: "sb_publishable_company_test_only".into(),
        };
        let previous = ConnectionConfig {
            url: "https://previous.example.invalid".into(),
            ..configured.clone()
        };
        std::fs::write(&company, serde_json::to_vec(&configured).unwrap()).unwrap();
        std::fs::write(&device, serde_json::to_vec(&previous).unwrap()).unwrap();
        assert_eq!(
            ConnectionConfig::load_files(Some(&company), &device)
                .unwrap()
                .url,
            configured.url
        );
        std::fs::write(&company, b"invalid JSON").unwrap();
        assert!(ConnectionConfig::load_files(Some(&company), &device).is_err());
        std::fs::remove_file(&company).unwrap();
        assert_eq!(
            ConnectionConfig::load_files(Some(&company), &device)
                .unwrap()
                .url,
            previous.url
        );
        std::fs::remove_file(&device).unwrap();
        assert!(
            ConnectionConfig::load_files(Some(&company), &device)
                .unwrap()
                .url
                .is_empty()
        );
        std::fs::remove_dir(&folder).unwrap();
    }
}

pub struct CloudClient {
    config: ConnectionConfig,
    http: Client,
    session: AuthSession,
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
            "Enter your Supabase account email and password."
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
            .context("Cannot reach Supabase. Check your connection and project URL.")?;
        let mut session: AuthSession = decode(response)?;
        if session.expires_at == 0 {
            session.expires_at = Utc::now().timestamp() + session.expires_in;
        }
        let mut client = Self {
            config,
            http,
            session,
        };
        let snapshot = client.snapshot()?;
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
            "This HR account is not assigned to this installation's company. Ask the project administrator to finish account setup."
        );
        ensure!(
            companies.len() == 1,
            "This installation requires exactly one company. Apply all included database migrations."
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
        let response = self
            .http
            .post(format!(
                "{}/auth/v1/logout",
                self.config.url.trim().trim_end_matches('/')
            ))
            .query(&[("scope", "local")])
            .header("apikey", self.config.publishable_key.trim())
            .bearer_auth(&self.session.access_token)
            .send()
            .context("Could not contact Supabase to revoke this session.")?;
        ensure!(
            response.status().is_success(),
            "Supabase could not revoke the session; local tokens have still been discarded."
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
        let message = match (status,code) {
            (StatusCode::UNAUTHORIZED,_) => "Sign-in or session validation failed. Check your account and sign in again.".into(),
            (_,"23505") => "That record already exists. Refresh before trying again.".into(),
            (_,"23P01") => "These dates overlap an existing pending or approved leave request.".into(),
            (_,"42501") => "This operation is not permitted for your company workspace.".into(),
            (_,"PGRST205" | "42P01") => "HRIS tables were not found. Apply the included migration to the dedicated Supabase project.".into(),
            _ => value.get("message").or_else(||value.get("msg")).and_then(Value::as_str)
                .map(|s|s.chars().take(240).collect::<String>()).unwrap_or_else(||format!("Supabase returned HTTP {}.",status.as_u16())),
        };
        bail!("{message}");
    }
    response
        .json()
        .context("Supabase returned an unexpected response format.")
}

pub enum Command {
    Change(Action),
    Refresh,
    Demo,
    Connect {
        config: ConnectionConfig,
        email: String,
        password: String,
    },
}

pub struct Update {
    pub result: std::result::Result<Snapshot, String>,
    pub message: String,
}

pub fn worker(
    initial: DemoStore,
    repaint: eframe::egui::Context,
) -> (mpsc::Sender<Command>, mpsc::Receiver<Update>) {
    let (commands, receiver) = mpsc::channel();
    let (events, updates) = mpsc::channel();
    thread::spawn(move || {
        let mut demo = initial;
        let mut cloud: Option<CloudClient> = None;
        let mut current = demo.data.clone();
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
                        message = "Connected to Supabase.".into();
                        Ok(data)
                    }
                    Err(error) => Err(error),
                },
                Command::Demo => {
                    if let Some(mut client) = cloud.take() {
                        if let Err(error) = client.sign_out() {
                            message = format!("Local session cleared. {error}");
                        }
                    }
                    demo = DemoStore::new();
                    if message.is_empty() {
                        message =
                            "Demo workspace restored. Sample changes reset when the app closes."
                                .into();
                    }
                    Ok(demo.data.clone())
                }
                Command::Refresh => match cloud.as_mut() {
                    Some(client) => client.snapshot(),
                    None => Ok(demo.data.clone()),
                },
                Command::Change(action) => {
                    message = if cloud.is_some() {
                        "Saved to Supabase."
                    } else {
                        "Updated in the demo workspace."
                    }
                    .into();
                    match cloud.as_mut() {
                        Some(client) => client.execute(action, &current),
                        None => demo.execute(action).map(|_| demo.data.clone()),
                    }
                }
            };
            if let Ok(data) = &result {
                current = data.clone();
            }
            if events
                .send(Update {
                    result: result.map_err(|e| e.to_string()),
                    message,
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
