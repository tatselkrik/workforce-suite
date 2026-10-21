use hris_desktop::{
    access::{AccountDraft, AdminAction, Role},
    account::{self, AccountAction, DisplayPreferences, PersonalDetails},
    backend::{self, CloudClient, Command, ConnectionConfig},
    domain::{Company, DemoStore},
};
use serde_json::{Value, json};
use std::{
    io::{Cursor, Read, Write},
    net::TcpListener,
    sync::{Arc, Mutex},
    thread,
    time::{Duration, Instant},
};

struct Reply {
    prefix: String,
    status: u16,
    body: Vec<u8>,
    mime: &'static str,
}
fn reply(prefix: impl Into<String>, status: u16, body: Value) -> Reply {
    Reply {
        prefix: prefix.into(),
        status,
        body: serde_json::to_vec(&body).unwrap(),
        mime: "application/json",
    }
}
struct Request {
    line: String,
    headers: String,
    body: Vec<u8>,
}
impl Request {
    fn json(&self) -> Value {
        serde_json::from_slice(&self.body).unwrap()
    }
}
type Requests = Arc<Mutex<Vec<Request>>>;

fn fixture(replies: Vec<Reply>) -> (ConnectionConfig, thread::JoinHandle<()>, Requests) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let requests = Arc::new(Mutex::new(Vec::new()));
    let observed = requests.clone();
    let server = thread::spawn(move || {
        let deadline = Instant::now() + Duration::from_secs(20);
        for reply in replies {
            let (mut stream, _) = loop {
                match listener.accept() {
                    Ok(stream) => break stream,
                    Err(error)
                        if error.kind() == std::io::ErrorKind::WouldBlock
                            && Instant::now() < deadline =>
                    {
                        thread::sleep(Duration::from_millis(5))
                    }
                    Err(error) => panic!("Fixture did not receive {}: {error}", reply.prefix),
                }
            };
            stream.set_nonblocking(false).unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(3)))
                .unwrap();
            let mut bytes = Vec::new();
            let mut buffer = [0u8; 8192];
            let header_end = loop {
                let n = stream.read(&mut buffer).unwrap();
                assert!(n > 0);
                bytes.extend_from_slice(&buffer[..n]);
                if let Some(end) = bytes.windows(4).position(|w| w == b"\r\n\r\n") {
                    break end;
                }
            };
            let headers = String::from_utf8(bytes[..header_end].to_vec()).unwrap();
            let length = headers
                .lines()
                .find_map(|line| {
                    line.to_lowercase()
                        .strip_prefix("content-length:")
                        .map(|v| v.trim().parse::<usize>().unwrap())
                })
                .unwrap_or(0);
            assert!(length < 3 * 1024 * 1024);
            while bytes.len() < header_end + 4 + length {
                let n = stream.read(&mut buffer).unwrap();
                assert!(n > 0);
                bytes.extend_from_slice(&buffer[..n]);
            }
            let line = headers.lines().next().unwrap().to_owned();
            assert!(
                line.starts_with(&reply.prefix),
                "Expected {}, got {line}",
                reply.prefix
            );
            observed.lock().unwrap().push(Request {
                line,
                headers,
                body: bytes[header_end + 4..].to_vec(),
            });
            write!(stream,"HTTP/1.1 {} Fixture\r\nContent-Type: {}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",reply.status,reply.mime,reply.body.len()).unwrap();
            stream.write_all(&reply.body).unwrap();
        }
    });
    (
        ConnectionConfig {
            url,
            publishable_key: "sb_publishable_local_profile_fixture".into(),
        },
        server,
        requests,
    )
}

fn user(company: &Company) -> Value {
    json!({"id":company.owner_id,"email":"person@example.invalid","email_confirmed_at":"2026-01-01T00:00:00Z",
        "created_at":"2026-01-01T00:00:00Z","last_sign_in_at":"2026-10-20T00:00:00Z",
        "user_metadata":{"hris_profile":{"full_name":"Alex Morgan","preferred_name":"Alex","contact_phone":""}}})
}
fn token(user: Value, bearer: &str) -> Value {
    json!({"access_token":bearer,"refresh_token":"synthetic-refresh","expires_at":4102444800i64,"user":user})
}
fn baseline(company: &Company) -> Vec<Reply> {
    vec![
        reply(
            "POST /auth/v1/token?grant_type=password ",
            200,
            token(user(company), "profile-jwt"),
        ),
        reply(
            "POST /rest/v1/rpc/my_hris_access ",
            200,
            json!({"company_id":company.id,"user_id":company.owner_id,"role":"administrator","employee_id":null,"direct_reports":[]}),
        ),
        reply("GET /rest/v1/companies?", 200, json!([company])),
        reply("GET /rest/v1/employees?", 200, json!([])),
        reply("GET /rest/v1/employee_contacts?", 200, json!([])),
        reply("GET /rest/v1/attendance?", 200, json!([])),
        reply("GET /rest/v1/leave_requests?", 200, json!([])),
        reply("POST /rest/v1/rpc/list_company_accounts ", 200, json!([])),
        reply("GET /rest/v1/account_access_events?", 200, json!([])),
        reply("GET /auth/v1/user ", 200, user(company)),
        reply(
            "GET /storage/v1/object/authenticated/profile-photos/",
            404,
            json!({"code":"NoSuchKey","message":"Object not found"}),
        ),
    ]
}
fn connect(config: ConnectionConfig) -> CloudClient {
    CloudClient::connect(
        config,
        "person@example.invalid".into(),
        "synthetic old passphrase".into(),
    )
    .unwrap()
    .0
}

#[test]
fn employee_desktop_sign_in_is_rejected_before_loading_hr_data_and_revokes_its_session() {
    let company = Company::default();
    let mut replies = baseline(&company);
    replies[1] = reply(
        "POST /rest/v1/rpc/my_hris_access ",
        200,
        json!({"company_id":company.id,"user_id":company.owner_id,"role":"employee","employee_id":uuid::Uuid::new_v4(),"direct_reports":[]}),
    );
    replies.truncate(2);
    replies.push(reply("POST /auth/v1/logout?scope=local ", 204, Value::Null));
    let (config, server, requests) = fixture(replies);
    let result = CloudClient::connect(
        config,
        "person@example.invalid".into(),
        "synthetic passphrase".into(),
    );
    assert!(
        result
            .err()
            .unwrap()
            .to_string()
            .contains("employee portal")
    );
    server.join().unwrap();
    let requests = requests.lock().unwrap();
    assert_eq!(requests.len(), 3);
    assert!(
        !requests
            .iter()
            .any(|r| r.line.contains("/employees") || r.line.contains("/storage/"))
    );
}

#[test]
fn manager_desktop_sign_in_does_not_request_administrator_account_data() {
    let company = Company::default();
    let mut replies = baseline(&company);
    replies[1] = reply(
        "POST /rest/v1/rpc/my_hris_access ",
        200,
        json!({"company_id":company.id,"user_id":company.owner_id,"role":"manager","employee_id":uuid::Uuid::new_v4(),"direct_reports":[]}),
    );
    replies.drain(7..9);
    let (config, server, requests) = fixture(replies);
    let (client, data) = CloudClient::connect(
        config,
        "person@example.invalid".into(),
        "synthetic passphrase".into(),
    )
    .unwrap();
    assert_eq!(data.access.role, Role::Manager);
    assert!(data.accounts.is_empty());
    drop(client);
    server.join().unwrap();
    let requests = requests.lock().unwrap();
    assert!(
        !requests
            .iter()
            .any(|r| r.line.contains("list_company_accounts")
                || r.line.contains("account_access_events"))
    );
}

#[test]
fn employee_contacts_save_and_reload_separately_with_scoped_updates_and_zero_row_errors() {
    use hris_desktop::{contacts::EmployeeContacts, domain::Action};
    let company = Company::default();
    let employee = DemoStore::new().data.employees[0].clone();
    let mut contacts = EmployeeContacts::empty(company.id, employee.id);
    contacts.phone = " 000 000 0000 ".into();
    contacts.personal_email = " sample@example.invalid ".into();
    let normalized = contacts.normalized().unwrap();
    let snapshot_replies = |contacts: Vec<EmployeeContacts>| {
        let mut replies = baseline(&company);
        replies[3] = reply("GET /rest/v1/employees?", 200, json!([employee]));
        replies[4] = reply("GET /rest/v1/employee_contacts?", 200, json!(contacts));
        replies
    };
    let mut replies = snapshot_replies(vec![]);
    replies.push(reply(
        "POST /rest/v1/employee_contacts ",
        201,
        json!([normalized]),
    ));
    replies.extend(
        snapshot_replies(vec![normalized.clone()])
            .into_iter()
            .skip(1)
            .take(8),
    );
    let mut updated = normalized.clone();
    updated.address = "Updated fictional address".into();
    replies.push(reply(
        "PATCH /rest/v1/employee_contacts?",
        200,
        json!([updated]),
    ));
    replies.extend(
        snapshot_replies(vec![updated.clone()])
            .into_iter()
            .skip(1)
            .take(8),
    );
    replies.push(reply("PATCH /rest/v1/employee_contacts?", 200, json!([])));
    let (config, server, observed) = fixture(replies);
    let (mut client, initial) = CloudClient::connect(
        config,
        "person@example.invalid".into(),
        "synthetic passphrase".into(),
    )
    .unwrap();
    assert!(initial.contacts.is_empty());
    let saved = client
        .execute(Action::SaveEmployeeContacts(contacts), &initial)
        .unwrap();
    assert_eq!(saved.contacts, vec![normalized]);
    let saved = client
        .execute(Action::SaveEmployeeContacts(updated.clone()), &saved)
        .unwrap();
    assert_eq!(saved.contacts, vec![updated.clone()]);
    assert!(
        client
            .execute(Action::SaveEmployeeContacts(updated), &saved)
            .unwrap_err()
            .to_string()
            .contains("No contact record changed")
    );
    server.join().unwrap();
    let observed = observed.lock().unwrap();
    let create = observed
        .iter()
        .find(|r| r.line.starts_with("POST /rest/v1/employee_contacts "))
        .unwrap();
    assert_eq!(create.json()["phone"], "000 000 0000");
    assert_eq!(create.json()["company_id"], company.id.to_string());
    assert_eq!(create.json()["employee_id"], employee.id.to_string());
    assert!(create.json().get("updated_at").is_none());
    let edit = observed
        .iter()
        .find(|r| r.line.starts_with("PATCH /rest/v1/employee_contacts?"))
        .unwrap();
    assert!(edit.line.contains(&format!("company_id=eq.{}", company.id)));
    assert!(
        edit.line
            .contains(&format!("employee_id=eq.{}", employee.id))
    );
    assert!(edit.json().get("company_id").is_none());
    assert!(edit.json().get("employee_id").is_none());
    assert!(edit.json().get("updated_at").is_none());
    assert!(
        !observed
            .iter()
            .any(|r| r.line.starts_with("PUT /auth/v1/user "))
    );
}

#[test]
fn account_provisioning_preserves_disabled_access_and_sends_password_only_to_the_function() {
    let company = Company::default();
    let mut replies = baseline(&company);
    replies.push(reply(
        "POST /functions/v1/account-admin ",
        201,
        json!({"confirmation_sent":false}),
    ));
    replies.extend(baseline(&company).into_iter().skip(1).take(8));
    let (config, server, requests) = fixture(replies);
    let (mut client, data) = CloudClient::connect(
        config,
        "person@example.invalid".into(),
        "synthetic passphrase".into(),
    )
    .unwrap();
    let mut draft = AccountDraft::new();
    draft.email = "employee@example.invalid".into();
    draft.active = false;
    draft.password = "synthetic initial passphrase".into();
    draft.confirmation = draft.password.clone();
    let (_, message) = client
        .execute_admin(AdminAction::Save(draft), &data)
        .unwrap();
    assert!(message.contains("planned portal"));
    server.join().unwrap();
    let requests = requests.lock().unwrap();
    let body = requests[11].json();
    assert_eq!(body["active"], false);
    assert_eq!(body["role"], "employee");
    assert!(body["employee_id"].is_null());
    assert_eq!(body["password"], "synthetic initial passphrase");
    assert!(!body.to_string().contains("confirmation"));
    assert!(requests[11].headers.contains("profile-jwt"));
    assert!(
        requests
            .iter()
            .enumerate()
            .filter(|(i, _)| *i != 11)
            .all(
                |(_, r)| !String::from_utf8_lossy(&r.body).contains("synthetic initial passphrase")
            )
    );
}
fn photo() -> Vec<u8> {
    let mut bytes = Cursor::new(Vec::new());
    image::DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(
        64,
        32,
        image::Rgba([40, 150, 120, 255]),
    ))
    .write_to(&mut bytes, image::ImageFormat::Png)
    .unwrap();
    bytes.into_inner()
}

#[test]
fn personal_details_preferences_and_private_photo_roundtrip_use_only_the_signed_in_account() {
    let company = Company::default();
    let path = format!("{}/avatar.png", company.owner_id);
    let mut changed = user(&company);
    changed["user_metadata"]["hris_profile"] =
        json!({"full_name":"Alex Morgan","preferred_name":"Morgan","contact_phone":""});
    let mut preferred = changed.clone();
    preferred["user_metadata"]["hris_preferences"] = json!({"scale_percent":115,"clock_24h":false});
    let normalized = account::normalize_photo(&photo()).unwrap();
    let mut replies = baseline(&company);
    replies.extend([
        reply("PUT /auth/v1/user ", 200, changed),
        reply("PUT /auth/v1/user ", 200, preferred.clone()),
        reply(
            format!("POST /storage/v1/object/profile-photos/{path} "),
            200,
            json!({"Key":path}),
        ),
        reply("GET /auth/v1/user ", 200, preferred),
        Reply {
            prefix: format!("GET /storage/v1/object/authenticated/profile-photos/{path} "),
            status: 200,
            body: normalized,
            mime: "image/png",
        },
        reply("DELETE /storage/v1/object/profile-photos ", 200, json!([])),
        reply("POST /auth/v1/logout?scope=others ", 204, Value::Null),
    ]);
    let (config, server, requests) = fixture(replies);
    let mut client = connect(config);
    client
        .execute_account(AccountAction::SaveDetails(PersonalDetails {
            full_name: " Alex Morgan ".into(),
            preferred_name: " Morgan ".into(),
            contact_phone: String::new(),
        }))
        .unwrap();
    assert_eq!(client.account().details.display_name(), "Morgan");
    client
        .execute_account(AccountAction::SaveDisplay(DisplayPreferences {
            scale_percent: 115,
            clock_24h: false,
        }))
        .unwrap();
    let input = std::env::temp_dir().join(format!("hris-profile-{}.png", uuid::Uuid::new_v4()));
    std::fs::write(&input, photo()).unwrap();
    client
        .execute_account(AccountAction::UploadPhoto(input.clone()))
        .unwrap();
    std::fs::remove_file(input).unwrap();
    client.refresh_account().unwrap();
    assert!(client.account().photo.is_some());
    assert_eq!(client.account().display.scale_percent, 115);
    assert_eq!(client.account().details.display_name(), "Morgan");
    client.execute_account(AccountAction::RemovePhoto).unwrap();
    assert!(client.account().photo.is_none());
    client
        .execute_account(AccountAction::SignOutOthers)
        .unwrap();
    server.join().unwrap();
    let requests = requests.lock().unwrap();
    assert_eq!(
        requests[11].json(),
        json!({"data":{"hris_profile":{"full_name":"Alex Morgan","preferred_name":"Morgan","contact_phone":""}}})
    );
    assert!(requests[12].json().get("app_metadata").is_none());
    assert!(
        requests[13]
            .headers
            .to_lowercase()
            .contains("x-upsert: true")
    );
    assert_eq!(
        image::load_from_memory(&requests[13].body).unwrap().width(),
        256
    );
    assert_eq!(requests[16].json(), json!({"prefixes":[path]}));
    for request in &requests[1..] {
        assert!(
            request
                .headers
                .to_lowercase()
                .contains("authorization: bearer profile-jwt")
        );
    }
}

#[test]
fn an_incorrect_current_password_prevents_any_password_update_request() {
    let company = Company::default();
    let mut replies = baseline(&company);
    replies.push(reply(
        "POST /auth/v1/token?grant_type=password ",
        400,
        json!({"code":"invalid_credentials","msg":"Invalid login credentials"}),
    ));
    let (config, server, requests) = fixture(replies);
    let mut client = connect(config);
    let result = client.execute_account(AccountAction::ChangePassword {
        current: "incorrect fixture".into(),
        new: "new synthetic passphrase".into(),
        confirmation: "new synthetic passphrase".into(),
    });
    assert!(result.unwrap_err().to_string().contains("current password"));
    server.join().unwrap();
    assert!(
        !requests
            .lock()
            .unwrap()
            .iter()
            .any(|r| r.line.starts_with("PUT "))
    );
}

#[test]
fn revoked_company_access_clears_worker_snapshot_and_account() {
    let company = Company::default();
    let mut replies = baseline(&company);
    replies.push(reply(
        "POST /rest/v1/rpc/my_hris_access ",
        403,
        json!({"code":"42501"}),
    ));
    let (config, server, _) = fixture(replies);
    let (commands, updates) = backend::worker(None, eframe::egui::Context::default());
    commands
        .send(Command::Connect {
            config,
            email: "synthetic@example.invalid".into(),
            password: "synthetic passphrase".into(),
        })
        .unwrap();
    assert_eq!(
        updates
            .recv_timeout(Duration::from_secs(10))
            .unwrap()
            .result
            .unwrap()
            .mode,
        hris_desktop::domain::Mode::Supabase
    );
    commands.send(Command::Refresh).unwrap();
    let event = updates.recv_timeout(Duration::from_secs(10)).unwrap();
    let cleared = event.result.unwrap();
    assert_eq!(cleared.mode, hris_desktop::domain::Mode::SignedOut);
    assert!(
        cleared.employees.is_empty()
            && cleared.accounts.is_empty()
            && cleared.access_events.is_empty()
    );
    assert!(event.account.is_none());
    server.join().unwrap();
}

#[test]
fn password_and_email_changes_reauthenticate_and_keep_email_pending_until_confirmation() {
    let company = Company::default();
    let original = user(&company);
    let mut pending = original.clone();
    pending["new_email"] = json!("updated@example.invalid");
    let mut replies = baseline(&company);
    replies.extend([
        reply(
            "POST /auth/v1/token?grant_type=password ",
            200,
            token(original.clone(), "fresh-password-jwt"),
        ),
        reply("PUT /auth/v1/user ", 200, original.clone()),
        reply(
            "POST /auth/v1/token?grant_type=password ",
            200,
            token(original, "fresh-email-jwt"),
        ),
        reply("PUT /auth/v1/user ", 200, pending),
    ]);
    let (config, server, requests) = fixture(replies);
    let mut client = connect(config);
    client
        .execute_account(AccountAction::ChangePassword {
            current: "old synthetic passphrase".into(),
            new: "new synthetic passphrase".into(),
            confirmation: "new synthetic passphrase".into(),
        })
        .unwrap();
    client
        .execute_account(AccountAction::ChangeEmail {
            current: "new synthetic passphrase".into(),
            new_email: "updated@example.invalid".into(),
        })
        .unwrap();
    assert_eq!(client.account().email, "person@example.invalid");
    assert_eq!(client.account().pending_email, "updated@example.invalid");
    server.join().unwrap();
    let requests = requests.lock().unwrap();
    assert_eq!(
        requests[12].json(),
        json!({"password":"new synthetic passphrase","current_password":"old synthetic passphrase"})
    );
    assert!(
        requests[12]
            .headers
            .to_lowercase()
            .contains("authorization: bearer fresh-password-jwt")
    );
    assert_eq!(
        requests[14].json(),
        json!({"email":"updated@example.invalid"})
    );
    assert!(
        requests[14]
            .headers
            .to_lowercase()
            .contains("authorization: bearer fresh-email-jwt")
    );
}

#[test]
fn a_failed_photo_upload_does_not_report_success_or_change_the_local_photo() {
    let company = Company::default();
    let mut replies = baseline(&company);
    replies.push(reply(
        "POST /storage/v1/object/profile-photos/",
        403,
        json!({"message":"Permission denied"}),
    ));
    let (config, server, _) = fixture(replies);
    let mut client = connect(config);
    let input =
        std::env::temp_dir().join(format!("hris-profile-denied-{}.png", uuid::Uuid::new_v4()));
    std::fs::write(&input, photo()).unwrap();
    assert!(
        client
            .execute_account(AccountAction::UploadPhoto(input.clone()))
            .is_err()
    );
    assert!(client.account().photo.is_none());
    std::fs::remove_file(input).unwrap();
    server.join().unwrap();
}

#[test]
fn demo_profile_edits_are_in_memory_and_security_changes_require_a_real_account() {
    let (commands, updates) =
        backend::worker(Some(DemoStore::new()), eframe::egui::Context::default());
    commands
        .send(Command::Account(AccountAction::SaveDetails(
            PersonalDetails {
                full_name: "Demo Person".into(),
                ..Default::default()
            },
        )))
        .unwrap();
    let update = updates.recv_timeout(Duration::from_secs(3)).unwrap();
    assert!(update.result.is_ok());
    assert_eq!(update.account.unwrap().details.full_name, "Demo Person");
    commands.send(Command::Refresh).unwrap();
    assert_eq!(
        updates
            .recv_timeout(Duration::from_secs(3))
            .unwrap()
            .account
            .unwrap()
            .details
            .full_name,
        "Demo Person"
    );
    commands
        .send(Command::Account(AccountAction::ChangePassword {
            current: "old fixture".into(),
            new: "new synthetic passphrase".into(),
            confirmation: "new synthetic passphrase".into(),
        }))
        .unwrap();
    assert!(
        updates
            .recv_timeout(Duration::from_secs(3))
            .unwrap()
            .result
            .is_err()
    );
    commands.send(Command::SignOut).unwrap();
    assert_eq!(
        updates
            .recv_timeout(Duration::from_secs(3))
            .unwrap()
            .account
            .unwrap()
            .details
            .full_name,
        "Alex Morgan"
    );
}

#[test]
fn sign_out_everywhere_clears_local_account_data_even_when_the_server_is_unreachable() {
    let company = Company::default();
    let (config, server, _) = fixture(baseline(&company));
    let (commands, updates) = backend::worker(None, eframe::egui::Context::default());
    commands
        .send(Command::Connect {
            config,
            email: "person@example.invalid".into(),
            password: "synthetic fixture passphrase".into(),
        })
        .unwrap();
    let signed_in = updates.recv_timeout(Duration::from_secs(10)).unwrap();
    assert!(signed_in.result.is_ok());
    assert!(signed_in.account.is_some());
    server.join().unwrap();
    commands.send(Command::SignOutEverywhere).unwrap();
    let signed_out = updates.recv_timeout(Duration::from_secs(10)).unwrap();
    assert!(signed_out.message.contains("Local session cleared"));
    assert_eq!(
        signed_out.result.unwrap().mode,
        hris_desktop::domain::Mode::SignedOut
    );
    assert!(signed_out.account.is_none());
}
