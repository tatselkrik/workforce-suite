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
        reply("GET /rest/v1/attendance_corrections?", 200, json!([])),
        reply("GET /rest/v1/leave_requests?", 200, json!([])),
        reply("POST /rest/v1/rpc/list_company_accounts ", 200, json!([])),
        reply("GET /rest/v1/account_access_events?", 200, json!([])),
        reply(
            "GET /rest/v1/employment_types?",
            200,
            json!(hris_desktop::employment::demo_choices(
                hris_desktop::employment::EmploymentKind::EmploymentType,
                company.id
            )),
        ),
        reply(
            "GET /rest/v1/work_locations?",
            200,
            json!(hris_desktop::employment::demo_choices(
                hris_desktop::employment::EmploymentKind::WorkLocation,
                company.id
            )),
        ),
        reply("GET /rest/v1/employment_history?", 200, json!([])),
        reply("GET /rest/v1/employee_documents?", 200, json!([])),
        reply("GET /rest/v1/employee_document_events?", 200, json!([])),
        reply(
            "GET /rest/v1/leave_policies?",
            200,
            json!([hris_desktop::leave::LeavePolicy {
                company_id: company.id,
                ..Default::default()
            }]),
        ),
        reply("GET /rest/v1/leave_events?", 200, json!([])),
        reply("GET /rest/v1/shift_templates?", 200, json!([])),
        reply("GET /rest/v1/shift_assignments?", 200, json!([])),
        reply("GET /rest/v1/shift_template_versions?", 200, json!([])),
        reply("GET /rest/v1/attendance_breaks?", 200, json!([])),
        reply("GET /rest/v1/attendance_break_events?", 200, json!([])),
        reply(
            "GET /rest/v1/attendance_calculation_policies?",
            200,
            json!([hris_desktop::calculations::CalculationPolicy {
                company_id: company.id,
                ..Default::default()
            }]),
        ),
        reply(
            "GET /rest/v1/attendance_calculation_policy_events?",
            200,
            json!([]),
        ),
        reply(
            "GET /rest/v1/attendance_calculation_events?",
            200,
            json!([]),
        ),
        reply("GET /auth/v1/user ", 200, user(company)),
        reply(
            "GET /storage/v1/object/authenticated/profile-photos/",
            404,
            json!({"code":"NoSuchKey","message":"Object not found"}),
        ),
    ]
}

#[test]
fn leave_writes_use_revisioned_rpcs_accept_scalar_results_and_reload_saved_charges() {
    use hris_desktop::{domain::*, leave::*};
    let mut store = DemoStore::new();
    store.data.leave.clear();
    let company = store.data.company.clone();
    let draft = LeaveDraft {
        employee_id: Some(store.data.employees[3].id),
        starts_on: "2028-03-01".into(),
        ends_on: "2028-03-01".into(),
        options: LeaveOptions {
            type_code: "annual".into(),
            day_part: DayPart::Am,
        },
        ..Default::default()
    };
    let row = prepare(&store.data, &draft).unwrap();
    let mut replies = baseline(&company);
    replies[3] = reply("GET /rest/v1/employees?", 200, json!(store.data.employees));
    replies.push(reply("POST /rest/v1/rpc/submit_leave ", 200, json!(row.id)));
    let mut after = baseline(&company);
    after[3] = reply("GET /rest/v1/employees?", 200, json!(store.data.employees));
    after[7] = reply("GET /rest/v1/leave_requests?", 200, json!([row]));
    replies.extend(after.into_iter().skip(1).take(24));
    replies.push(reply(
        "POST /rest/v1/rpc/cancel_leave ",
        409,
        json!({"code":"PT409","message":"This request changed. Refresh before cancelling."}),
    ));
    let (config, server, requests) = fixture(replies);
    let (mut client, data) = CloudClient::connect(
        config,
        "person@example.invalid".into(),
        "synthetic passphrase".into(),
    )
    .unwrap();
    let saved = client.execute(Action::RequestLeave(draft), &data).unwrap();
    assert_eq!(saved.leave[0].units(), 1);
    assert!(
        client
            .execute(
                Action::CancelLeave(row.id, 0, "Synthetic reason".into()),
                &saved
            )
            .is_err()
    );
    server.join().unwrap();
    let requests = requests.lock().unwrap();
    let sent = requests
        .iter()
        .find(|r| r.line.starts_with("POST /rest/v1/rpc/submit_leave "))
        .unwrap()
        .json();
    assert_eq!(sent["p_part"], "am");
    assert_eq!(sent["p_type_code"], "annual");
    assert!(sent.get("charged_units").is_none());
    assert!(sent.get("approval_steps").is_none());
    assert!(
        requests
            .iter()
            .filter(|r| r.line.contains(" /rest/v1/"))
            .all(|r| r
                .headers
                .to_lowercase()
                .contains("x-hris-leave-contract: 2"))
    );
    assert!(
        requests
            .iter()
            .filter(|r| r.line.starts_with("GET /rest/v1/leave_policies?"))
            .all(|r| r.line.contains("order=company_id.asc"))
    );
    assert!(
        !requests
            .iter()
            .any(|r| r.line.starts_with("PATCH /rest/v1/leave_requests")
                || r.line.starts_with("POST /rest/v1/leave_requests"))
    );
}

#[test]
fn policy_saves_require_the_loaded_revision_and_refresh_the_authoritative_policy() {
    use hris_desktop::{domain::Action, leave::LeavePolicy};
    let company = Company::default();
    let mut policy = LeavePolicy {
        company_id: company.id,
        ..Default::default()
    };
    policy.workweek.push(6);
    let mut replies = baseline(&company);
    replies.push(reply("POST /rest/v1/rpc/save_leave_policy ", 200, json!(1)));
    let mut after = baseline(&company);
    let mut server_policy = policy.clone();
    server_policy.revision = 1;
    after[15] = reply("GET /rest/v1/leave_policies?", 200, json!([server_policy]));
    replies.extend(after.into_iter().skip(1).take(24));
    let (config, server, requests) = fixture(replies);
    let (mut client, data) = CloudClient::connect(
        config,
        "person@example.invalid".into(),
        "synthetic passphrase".into(),
    )
    .unwrap();
    let saved = client
        .execute(Action::SaveLeavePolicy(policy), &data)
        .unwrap();
    assert_eq!(saved.leave_setup.policy.revision, 1);
    assert!(saved.leave_setup.policy.workweek.contains(&6));
    server.join().unwrap();
    let requests = requests.lock().unwrap();
    let sent = requests
        .iter()
        .find(|r| r.line.starts_with("POST /rest/v1/rpc/save_leave_policy "))
        .unwrap()
        .json();
    assert_eq!(sent["p_expected_revision"], 0);
}

struct DocumentFixture {
    store: hris_desktop::domain::DemoStore,
    draft: hris_desktop::documents::DocumentDraft,
    folder: std::path::PathBuf,
    bytes: Vec<u8>,
}
impl DocumentFixture {
    fn new() -> Self {
        let store = hris_desktop::domain::DemoStore::new();
        let folder =
            std::env::temp_dir().join(format!("hris-document-http-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&folder).unwrap();
        let bytes = b"%PDF-1.4\n1 0 obj <</Type /Catalog>> endobj\n%%EOF\n".to_vec();
        let file = folder.join("Synthetic document.pdf");
        std::fs::write(&file, &bytes).unwrap();
        let mut draft = hris_desktop::documents::DocumentDraft::new(store.data.employees[1].id);
        draft.title = "Synthetic file".into();
        draft.file = Some(file);
        Self {
            store,
            draft,
            folder,
            bytes,
        }
    }
    fn baseline(
        &self,
        docs: Vec<hris_desktop::documents::EmployeeDocument>,
        events: Vec<hris_desktop::documents::DocumentEvent>,
    ) -> Vec<Reply> {
        let mut replies = baseline(&self.store.data.company);
        replies[3] = reply(
            "GET /rest/v1/employees?",
            200,
            json!([self.store.data.employees[1]]),
        );
        replies[13] = reply("GET /rest/v1/employee_documents?", 200, json!(docs));
        replies[14] = reply("GET /rest/v1/employee_document_events?", 200, json!(events));
        replies
    }
    fn uploaded(&mut self) -> hris_desktop::documents::EmployeeDocument {
        hris_desktop::documents::execute_demo(
            &mut self.store.data,
            &mut std::collections::HashMap::new(),
            hris_desktop::documents::DocumentAction::Upload(self.draft.clone()),
        )
        .unwrap();
        self.store.data.documents[0].clone()
    }
}
impl Drop for DocumentFixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.folder);
    }
}

#[test]
fn employee_document_upload_reserves_private_path_finalizes_and_reloads_protected_history() {
    use hris_desktop::documents::*;
    for upload_status in [200, 409] {
        let mut f = DocumentFixture::new();
        let ready = f.uploaded();
        let mut pending = ready.clone();
        pending.state = DocumentState::Pending;
        pending.revision = 0;
        let mut replies = f.baseline(vec![], vec![]);
        replies.push(reply(
            "POST /rest/v1/rpc/save_employee_document ",
            200,
            json!(pending),
        ));
        replies.push(reply(
            "POST /storage/v1/object/employee-documents/",
            upload_status,
            json!({}),
        ));
        replies.push(reply(
            "POST /rest/v1/rpc/save_employee_document ",
            200,
            json!(ready),
        ));
        replies.extend(
            f.baseline(vec![ready.clone()], f.store.data.document_events.clone())
                .into_iter()
                .skip(1)
                .take(24),
        );
        let (config, server, requests) = fixture(replies);
        let (mut client, current) = CloudClient::connect(
            config,
            "person@example.invalid".into(),
            "synthetic passphrase".into(),
        )
        .unwrap();
        let (after, _) = client
            .execute_document(DocumentAction::Upload(f.draft.clone()), &current)
            .unwrap();
        assert_eq!(after.documents, vec![ready]);
        assert_eq!(after.document_events.len(), 1);
        server.join().unwrap();
        let requests = requests.lock().unwrap();
        let operations: Vec<_> = requests
            .iter()
            .filter(|r| r.line.contains("/rpc/save_employee_document "))
            .map(|r| r.json())
            .collect();
        assert_eq!(operations[0]["p_operation"], "reserve");
        assert_eq!(operations[1]["p_operation"], "finalize");
        assert!(operations[0]["p_document"].get("created_by").is_none());
        assert!(operations[0]["p_document"].get("state").is_none());
        let upload = requests
            .iter()
            .find(|r| r.line.starts_with("POST /storage/v1/object/"))
            .unwrap();
        assert_eq!(upload.body, f.bytes);
        assert!(upload.headers.contains("profile-jwt"));
        assert!(
            upload
                .headers
                .to_lowercase()
                .contains("content-type: application/pdf")
        );
        assert!(!upload.headers.to_lowercase().contains("x-upsert"));
        assert!(
            !requests
                .iter()
                .any(|r| r.line.starts_with("DELETE /storage/"))
        );
    }
}

#[test]
fn an_unconfirmed_upload_is_retained_for_retry_and_never_reported_as_filed() {
    use hris_desktop::documents::*;
    let mut f = DocumentFixture::new();
    let mut pending = f.uploaded();
    pending.state = DocumentState::Pending;
    pending.revision = 0;
    let mut replies = f.baseline(vec![], vec![]);
    replies.push(reply(
        "POST /rest/v1/rpc/save_employee_document ",
        200,
        json!(pending),
    ));
    replies.push(reply(
        "POST /storage/v1/object/employee-documents/",
        500,
        json!({"message":"Upload interrupted"}),
    ));
    replies.push(reply(
        "POST /rest/v1/rpc/save_employee_document ",
        400,
        json!({"code":"23514","message":"Incomplete upload"}),
    ));
    let (config, server, requests) = fixture(replies);
    let (mut client, current) = CloudClient::connect(
        config,
        "person@example.invalid".into(),
        "synthetic passphrase".into(),
    )
    .unwrap();
    let error = client
        .execute_document(DocumentAction::Upload(f.draft.clone()), &current)
        .unwrap_err()
        .to_string();
    assert!(error.contains("not confirmed"));
    assert!(current.documents.is_empty());
    server.join().unwrap();
    assert!(
        !requests
            .lock()
            .unwrap()
            .iter()
            .any(|r| r.line.starts_with("DELETE /storage/"))
    );
}

#[test]
fn a_mismatched_upload_reservation_sends_no_document_bytes() {
    use hris_desktop::documents::*;
    let mut f = DocumentFixture::new();
    let mut wrong = f.uploaded();
    wrong.id = uuid::Uuid::new_v4();
    wrong.object_name = object_name(
        wrong.company_id,
        wrong.employee_id,
        wrong.id,
        &wrong.mime_type,
    )
    .unwrap();
    wrong.state = DocumentState::Pending;
    let mut replies = f.baseline(vec![], vec![]);
    replies.push(reply(
        "POST /rest/v1/rpc/save_employee_document ",
        200,
        json!(wrong),
    ));
    let (config, server, requests) = fixture(replies);
    let (mut client, current) = CloudClient::connect(
        config,
        "person@example.invalid".into(),
        "synthetic passphrase".into(),
    )
    .unwrap();
    assert!(
        client
            .execute_document(DocumentAction::Upload(f.draft.clone()), &current)
            .is_err()
    );
    server.join().unwrap();
    assert!(
        !requests
            .lock()
            .unwrap()
            .iter()
            .any(|r| r.line.starts_with("POST /storage/"))
    );
}

#[test]
fn private_document_download_verifies_bytes_before_writing_a_user_selected_copy() {
    use hris_desktop::documents::*;
    for tampered in [false, true] {
        let mut f = DocumentFixture::new();
        let ready = f.uploaded();
        let mut replies = f.baseline(vec![ready.clone()], vec![]);
        replies.push(Reply {
            prefix: "GET /storage/v1/object/authenticated/employee-documents/".into(),
            status: 200,
            body: if tampered {
                b"tampered file".to_vec()
            } else {
                f.bytes.clone()
            },
            mime: "application/pdf",
        });
        if !tampered {
            replies.extend(
                f.baseline(vec![ready.clone()], vec![])
                    .into_iter()
                    .skip(1)
                    .take(24),
            );
        }
        let (config, server, requests) = fixture(replies);
        let (mut client, current) = CloudClient::connect(
            config,
            "person@example.invalid".into(),
            "synthetic passphrase".into(),
        )
        .unwrap();
        let destination = f.folder.join("download.pdf");
        let result = client.execute_document(
            DocumentAction::Download {
                id: ready.id,
                destination: destination.clone(),
            },
            &current,
        );
        assert_eq!(result.is_ok(), !tampered);
        assert_eq!(destination.exists(), !tampered);
        if !tampered {
            assert_eq!(std::fs::read(destination).unwrap(), f.bytes);
        }
        server.join().unwrap();
        let requests = requests.lock().unwrap();
        let download = requests
            .iter()
            .find(|r| r.line.contains("/authenticated/employee-documents/"))
            .unwrap();
        assert!(download.headers.contains("profile-jwt"));
        assert!(
            download
                .headers
                .to_lowercase()
                .contains("cache-control: no-cache")
        );
    }
}

#[test]
fn archiving_documents_uses_revisioned_rpc_and_reloads_retained_files() {
    use hris_desktop::documents::*;
    let mut f = DocumentFixture::new();
    let doc = f.uploaded();
    let mut archived = doc.clone();
    archived.archived = true;
    archived.revision += 1;
    let mut replies = f.baseline(vec![doc.clone()], vec![]);
    replies.push(reply(
        "POST /rest/v1/rpc/save_employee_document ",
        200,
        json!(archived),
    ));
    replies.extend(
        f.baseline(vec![archived.clone()], vec![])
            .into_iter()
            .skip(1)
            .take(24),
    );
    let (config, server, requests) = fixture(replies);
    let (mut client, current) = CloudClient::connect(
        config,
        "person@example.invalid".into(),
        "synthetic passphrase".into(),
    )
    .unwrap();
    let (after, _) = client
        .execute_document(
            DocumentAction::Archive {
                id: doc.id,
                revision: doc.revision,
                archived: true,
            },
            &current,
        )
        .unwrap();
    assert_eq!(after.documents, vec![archived]);
    server.join().unwrap();
    let requests = requests.lock().unwrap();
    let request = requests
        .iter()
        .find(|r| r.line.contains("/rpc/save_employee_document "))
        .unwrap();
    assert_eq!(
        request.json(),
        json!({"p_operation":"archive","p_document":{"id":doc.id,"revision":1}})
    );
    assert!(
        !requests
            .iter()
            .any(|r| r.line.starts_with("DELETE /storage/"))
    );
}

#[test]
fn employment_history_is_reloaded_with_company_scope_and_never_supplied_by_the_writer() {
    use hris_desktop::{
        contacts::EmployeeContacts,
        domain::{Action, EmployeeDraft},
        history::timeline,
    };
    let mut store = DemoStore::new();
    let person = store.data.employees[1].clone();
    let company = store.data.company.clone();
    let mut draft = EmployeeDraft::from(&person);
    draft.department = "History transport department".into();
    store.execute(Action::SaveEmployee(draft.clone())).unwrap();
    let saved = store
        .data
        .employees
        .iter()
        .find(|e| e.id == person.id)
        .unwrap()
        .clone();
    let events = store
        .data
        .employment_history
        .iter()
        .filter(|e| e.employee_id == person.id)
        .cloned()
        .collect::<Vec<_>>();
    let mut replies = baseline(&company);
    replies[3] = reply("GET /rest/v1/employees?", 200, json!([person]));
    replies.push(reply(
        "POST /rest/v1/rpc/save_employee_record ",
        200,
        json!(person.id),
    ));
    let mut after = baseline(&company);
    after[3] = reply("GET /rest/v1/employees?", 200, json!([saved]));
    after[12] = reply("GET /rest/v1/employment_history?", 200, json!(events));
    replies.extend(after.into_iter().skip(1).take(24));
    let (config, server, requests) = fixture(replies);
    let (mut client, current) = CloudClient::connect(
        config,
        "person@example.invalid".into(),
        "synthetic passphrase".into(),
    )
    .unwrap();
    let refreshed = client
        .execute(
            Action::SaveEmployeeRecord(draft, EmployeeContacts::empty(company.id, person.id)),
            &current,
        )
        .unwrap();
    assert_eq!(timeline(&refreshed, person.id).len(), 2);
    assert_eq!(
        timeline(&refreshed, person.id)[0].changes()[0].after,
        "History transport department"
    );
    server.join().unwrap();
    let requests = requests.lock().unwrap();
    for request in requests
        .iter()
        .filter(|r| r.line.contains("/employment_history?"))
    {
        assert!(
            request
                .line
                .contains(&format!("company_id=eq.{}", company.id))
        );
        assert!(request.headers.contains("profile-jwt"));
    }
    let payload = requests
        .iter()
        .find(|r| {
            r.line
                .starts_with("POST /rest/v1/rpc/save_employee_record ")
        })
        .unwrap()
        .json();
    assert!(
        payload.get("history").is_none()
            && payload.get("actor_id").is_none()
            && payload.get("recorded_at").is_none()
    );
}

#[test]
fn attendance_correction_uses_the_guarded_rpc_and_reloads_immutable_history() {
    use hris_desktop::{
        corrections::CorrectionDraft,
        domain::{Action, DemoStore},
    };
    let mut data = DemoStore::new().data;
    data.attendance.clear();
    let company = data.company.clone();
    let person = data.employees[0].clone();
    let day = company.today().pred_opt().unwrap();
    let mut draft = CorrectionDraft::new(person.id, day, None, &data);
    draft.checked_in = "08:00".into();
    draft.checked_out = "17:00".into();
    draft.reason = "Synthetic verified correction".into();
    let change = draft.change(&data, chrono::Utc::now()).unwrap();
    let row = change.record.clone();
    let event = hris_desktop::corrections::CorrectionEvent {
        id: uuid::Uuid::new_v4(),
        company_id: company.id,
        attendance_id: row.id,
        employee_id: person.id,
        work_date: day,
        actor_id: company.owner_id,
        actor_name: "Synthetic HR".into(),
        actor_role: Role::Administrator,
        reason: change.reason.clone(),
        before_record: None,
        after_record: row.clone(),
        created_at: chrono::Utc::now(),
    };
    let mut replies = baseline(&company);
    replies[3] = reply("GET /rest/v1/employees?", 200, json!([person]));
    replies.push(reply(
        "POST /rest/v1/rpc/correct_attendance ",
        200,
        json!(row.id),
    ));
    let mut after = baseline(&company);
    after[3] = reply("GET /rest/v1/employees?", 200, json!([person]));
    after[5] = reply("GET /rest/v1/attendance?", 200, json!([row]));
    after[6] = reply("GET /rest/v1/attendance_corrections?", 200, json!([event]));
    replies.extend(after.into_iter().skip(1).take(24));
    let (config, server, requests) = fixture(replies);
    let (mut client, current) = CloudClient::connect(
        config,
        "person@example.invalid".into(),
        "synthetic passphrase".into(),
    )
    .unwrap();
    let refreshed = client
        .execute(Action::CorrectAttendance(change), &current)
        .unwrap();
    assert_eq!(refreshed.corrections.len(), 1);
    assert_eq!(
        refreshed.corrections[0].reason,
        "Synthetic verified correction"
    );
    assert_eq!(refreshed.attendance[0].work_date, day);
    server.join().unwrap();
    let requests = requests.lock().unwrap();
    let body = requests
        .iter()
        .find(|r| r.line.starts_with("POST /rest/v1/rpc/correct_attendance "))
        .unwrap()
        .json();
    assert!(body["p_change"]["attendance_id"].is_null());
    assert!(body["p_change"]["expected_revision"].is_null());
    assert!(body["p_change"].get("actor_id").is_none());
    assert!(
        !requests
            .iter()
            .any(|r| r.line.starts_with("PATCH /rest/v1/attendance"))
    );
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
fn employment_options_use_guarded_rpc_and_reload_inactive_labels_without_changing_people() {
    use hris_desktop::{
        domain::Action,
        employment::{EmploymentKind, EmploymentOptionDraft},
    };
    let company = Company::default();
    let mut replies = baseline(&company);
    let options = hris_desktop::employment::demo_choices(EmploymentKind::WorkLocation, company.id);
    let option = options[0].clone();
    replies.push(reply(
        "POST /rest/v1/rpc/save_employment_option ",
        200,
        json!(option.id),
    ));
    let mut after = baseline(&company);
    let mut revised = options.clone();
    revised[0].active = false;
    revised[0].revision = 1;
    revised[0].name = "Main office renamed".into();
    after[11] = reply("GET /rest/v1/work_locations?", 200, json!(revised));
    replies.extend(after.into_iter().skip(1).take(24));
    let (config, server, requests) = fixture(replies);
    let (mut client, current) = CloudClient::connect(
        config,
        "person@example.invalid".into(),
        "synthetic passphrase".into(),
    )
    .unwrap();
    let mut draft = EmploymentOptionDraft::existing(EmploymentKind::WorkLocation, &option);
    draft.active = false;
    draft.name = "Main office renamed".into();
    let next = client
        .execute(Action::SaveEmploymentOption(draft), &current)
        .unwrap();
    assert_eq!(next.employees, current.employees);
    assert_eq!(next.contacts, current.contacts);
    assert_eq!(
        EmploymentKind::WorkLocation.display(&next, Some(option.id)),
        "Main office renamed (inactive)"
    );
    server.join().unwrap();
    let requests = requests.lock().unwrap();
    let request = requests
        .iter()
        .find(|r| {
            r.line
                .starts_with("POST /rest/v1/rpc/save_employment_option ")
        })
        .unwrap();
    assert_eq!(
        request.json(),
        json!({"p_kind":"work_location","p_option":{"id":option.id,"name":"Main office renamed","active":false,"expected_revision":0}})
    );
    assert!(request.headers.contains("profile-jwt"));
    assert!(
        !requests
            .iter()
            .any(|r| r.line.starts_with("PATCH /rest/v1/work_locations"))
    );
}

#[test]
fn a_server_attendance_overlap_is_not_misreported_as_a_leave_overlap() {
    use hris_desktop::{
        corrections::CorrectionDraft,
        domain::{Action, DemoStore},
    };
    let mut data = DemoStore::new().data;
    data.attendance.clear();
    let person = data.employees[0].clone();
    let company = data.company.clone();
    let day = company.today().pred_opt().unwrap();
    let mut draft = CorrectionDraft::new(person.id, day, None, &data);
    draft.checked_in = "08:00".into();
    draft.checked_out = "17:00".into();
    draft.reason = "Synthetic overlap check".into();
    let change = draft.change(&data, chrono::Utc::now()).unwrap();
    let mut replies = baseline(&company);
    replies[3] = reply("GET /rest/v1/employees?", 200, json!([person]));
    replies.push(reply("POST /rest/v1/rpc/correct_attendance ",400,json!({"code":"23P01","message":"These times overlap another attendance record. Review both shifts before saving."})));
    let (config, server, _) = fixture(replies);
    let (mut client, current) = CloudClient::connect(
        config,
        "person@example.invalid".into(),
        "synthetic passphrase".into(),
    )
    .unwrap();
    let error = client
        .execute(Action::CorrectAttendance(change), &current)
        .unwrap_err()
        .to_string();
    assert!(error.contains("attendance record") && !error.contains("leave"));
    server.join().unwrap();
}

#[test]
fn editing_staff_saves_the_same_employee_contacts_and_role_in_one_rpc() {
    use hris_desktop::{contacts::EmployeeContacts, domain::EmployeeDraft};
    let company = Company::default();
    let person = DemoStore::new().data.employees[2].clone();
    let mut replies = baseline(&company);
    replies[3] = reply("GET /rest/v1/employees?", 200, json!([person]));
    replies.push(reply(
        "POST /rest/v1/rpc/save_staff_account ",
        200,
        json!(person.id),
    ));
    let mut after = baseline(&company);
    after[3] = reply("GET /rest/v1/employees?", 200, json!([person]));
    replies.extend(after.into_iter().skip(1).take(24));
    let (config, server, requests) = fixture(replies);
    let (mut client, data) = CloudClient::connect(
        config,
        "person@example.invalid".into(),
        "synthetic passphrase".into(),
    )
    .unwrap();
    let mut contacts = EmployeeContacts::empty(company.id, person.id);
    contacts.phone = "000 000 0000".into();
    client
        .execute_admin(
            AdminAction::SaveStaff(
                AccountDraft {
                    user_id: Some(uuid::Uuid::from_u128(88)),
                    email: "staff@example.invalid".into(),
                    role: Role::HrGeneralist,
                    active: true,
                    employee_id: Some(person.id),
                    ..Default::default()
                },
                EmployeeDraft::from(&person),
                contacts,
            ),
            &data,
        )
        .unwrap();
    server.join().unwrap();
    let requests = requests.lock().unwrap();
    let saved = requests
        .iter()
        .find(|r| r.line.starts_with("POST /rest/v1/rpc/save_staff_account "))
        .unwrap();
    assert_eq!(saved.json()["p_employee"]["id"], person.id.to_string());
    assert_eq!(
        saved.json()["p_employee"]["employment_type_id"],
        json!(person.employment_type_id)
    );
    assert_eq!(
        saved.json()["p_employee"]["work_location_id"],
        json!(person.work_location_id)
    );
    assert_eq!(saved.json()["p_contacts"]["phone"], "000 000 0000");
    assert_eq!(saved.json()["p_role"], "hr_generalist");
    assert!(
        !requests
            .iter()
            .any(|r| r.line.starts_with("POST /rest/v1/employees")
                || r.line.starts_with("POST /rest/v1/rpc/save_company_account"))
    );
}

#[test]
fn company_logo_upload_refresh_and_removal_use_private_storage_and_revision_cache() {
    use hris_desktop::domain::Action;
    let company = Company::default();
    let mut revised = company.clone();
    revised.logo_version = Some(uuid::Uuid::from_u128(77));
    let mut encoded = Cursor::new(Vec::new());
    image::RgbaImage::from_pixel(4, 2, image::Rgba([20, 40, 60, 128]))
        .write_to(&mut encoded, image::ImageFormat::Png)
        .unwrap();
    let png = encoded.into_inner();
    let file = std::env::temp_dir().join(format!("hris-logo-http-{}.png", uuid::Uuid::new_v4()));
    std::fs::write(&file, &png).unwrap();
    let mut replies = baseline(&company);
    replies.push(reply(
        "POST /storage/v1/object/company-branding/",
        200,
        json!({"Key":"synthetic"}),
    ));
    replies.push(reply(
        "POST /rest/v1/rpc/set_company_logo ",
        200,
        json!(revised.logo_version),
    ));
    let mut after = baseline(&revised);
    after.remove(0);
    after.truncate(24);
    replies.extend(after);
    replies.push(Reply {
        prefix: "GET /storage/v1/object/authenticated/company-branding/".into(),
        status: 200,
        body: png,
        mime: "image/png",
    });
    replies.extend(baseline(&revised).into_iter().skip(1).take(24));
    replies.push(reply(
        "DELETE /storage/v1/object/company-branding ",
        200,
        json!([]),
    ));
    replies.push(reply(
        "POST /rest/v1/rpc/set_company_logo ",
        200,
        Value::Null,
    ));
    replies.extend(baseline(&company).into_iter().skip(1).take(24));
    let (config, server, requests) = fixture(replies);
    let (mut client, data) = CloudClient::connect(
        config,
        "person@example.invalid".into(),
        "synthetic passphrase".into(),
    )
    .unwrap();
    let saved = client
        .execute(Action::UploadCompanyLogo(file.clone()), &data)
        .unwrap();
    assert!(saved.logo_png.is_some());
    assert_eq!(saved.company.logo_version, revised.logo_version);
    let cached = client.snapshot().unwrap();
    assert_eq!(cached.logo_png, saved.logo_png);
    let removed = client.execute(Action::RemoveCompanyLogo, &cached).unwrap();
    assert!(removed.logo_png.is_none());
    server.join().unwrap();
    std::fs::remove_file(file).unwrap();
    let requests = requests.lock().unwrap();
    assert_eq!(
        requests
            .iter()
            .filter(|r| r
                .line
                .starts_with("GET /storage/v1/object/authenticated/company-branding/"))
            .count(),
        1
    );
    let upload = requests
        .iter()
        .find(|r| {
            r.line
                .starts_with("POST /storage/v1/object/company-branding/")
        })
        .unwrap();
    assert!(upload.headers.to_lowercase().contains("x-upsert: true"));
    assert!(upload.headers.contains("Bearer profile-jwt"));
    assert!(!upload.body.is_empty());
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
    replies.drain(8..10);
    replies.retain(|r| {
        !r.prefix
            .starts_with("GET /rest/v1/shift_template_versions?")
            && !r
                .prefix
                .starts_with("GET /rest/v1/attendance_break_events?")
            && !r
                .prefix
                .starts_with("GET /rest/v1/attendance_calculation_policy_events?")
            && !r
                .prefix
                .starts_with("GET /rest/v1/attendance_calculation_events?")
    });
    replies.remove(6); // Correction reasons are HR-only, including their loading path.
    replies.retain(|r| !r.prefix.contains("/employment_history?"));
    replies.retain(|r| {
        !r.prefix.contains("/employee_documents?")
            && !r.prefix.contains("/employee_document_events?")
    });
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
            .take(24),
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
            .take(24),
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
fn unified_employee_save_uses_one_transaction_rpc_instead_of_separate_partial_writes() {
    use hris_desktop::{
        contacts::EmployeeContacts,
        domain::{Action, EmployeeDraft},
    };
    let company = Company::default();
    let person = DemoStore::new().data.employees[2].clone();
    let mut initial = baseline(&company);
    initial[3] = reply("GET /rest/v1/employees?", 200, json!([person]));
    initial.push(reply(
        "POST /rest/v1/rpc/save_employee_record ",
        200,
        json!(person.id),
    ));
    let mut after = baseline(&company);
    after[3] = reply("GET /rest/v1/employees?", 200, json!([person]));
    initial.extend(after.into_iter().skip(1).take(24));
    let (config, server, requests) = fixture(initial);
    let (mut client, data) = CloudClient::connect(
        config,
        "person@example.invalid".into(),
        "synthetic passphrase".into(),
    )
    .unwrap();
    let mut employee = EmployeeDraft::from(&person);
    employee.joined_on.clear();
    employee.department.clear();
    let mut contacts = EmployeeContacts::empty(company.id, person.id);
    contacts.phone = " 000 000 0000 ".into();
    client
        .execute(Action::SaveEmployeeRecord(employee, contacts), &data)
        .unwrap();
    server.join().unwrap();
    let requests = requests.lock().unwrap();
    let rpc = requests
        .iter()
        .find(|r| {
            r.line
                .starts_with("POST /rest/v1/rpc/save_employee_record ")
        })
        .unwrap();
    assert_eq!(rpc.json()["p_employee"]["id"], person.id.to_string());
    assert!(rpc.json()["p_employee"]["joined_on"].is_null());
    assert_eq!(rpc.json()["p_contacts"]["phone"], "000 000 0000");
    assert!(
        !requests
            .iter()
            .any(|r| r.line.starts_with("PATCH /rest/v1/employees")
                || r.line.starts_with("POST /rest/v1/employee_contacts"))
    );
}

#[test]
fn update_checks_verify_signatures_and_schema_before_offering_a_new_version() {
    use base64::{Engine, engine::general_purpose::STANDARD};
    use ed25519_dalek::{Signer, SigningKey};
    use hris_desktop::updater::{Release, SignedRelease, UpdateConfig, UpdateState, digest};
    let company = Company::default();
    let key = SigningKey::from_bytes(&[61; 32]);
    let config_key = UpdateConfig {
        public_key: STANDARD.encode(key.verifying_key().as_bytes()),
    };
    let current = hris_desktop::updater::Version::parse(hris_desktop::updater::VERSION).unwrap();
    let next = format!("{}.{}.{}", current.0, current.1, current.2 + 1);
    let release = Release {
        application_id: hris_desktop::APP_ID.into(),
        version: next.clone(),
        platform: "windows-x64".into(),
        requires_schema: "20261021054356".into(),
        artifact: format!("{next}/HRIS-Desktop-Update-{next}.exe"),
        sha256: digest(b"MZfixture"),
        executable_sha256: "a".repeat(64),
        size: 9,
        notes: "Synthetic update".into(),
    };
    let payload = serde_json::to_vec(&release).unwrap();
    let signed = SignedRelease {
        payload: STANDARD.encode(&payload),
        signature: STANDARD.encode(key.sign(&payload).to_bytes()),
    };
    let mut replies = baseline(&company);
    replies.push(reply(
        "POST /rest/v1/rpc/desktop_update ",
        200,
        json!({"schema_version":"20261021054356","release":signed}),
    ));
    let mut bad = signed.clone();
    bad.signature = STANDARD.encode([0; 64]);
    replies.push(reply(
        "POST /rest/v1/rpc/desktop_update ",
        200,
        json!({"schema_version":"20261021054356","release":bad}),
    ));
    replies.push(reply(
        "POST /rest/v1/rpc/desktop_update ",
        200,
        json!({"schema_version":"20000101000000","release":signed}),
    ));
    replies.push(reply(
        "POST /rest/v1/rpc/desktop_update ",
        200,
        json!({"schema_version":"20261021054356","release":null}),
    ));
    let (config, server, requests) = fixture(replies);
    let mut client = connect(config);
    assert!(matches!(
        client.check_update(&config_key).unwrap(),
        UpdateState::Available(_)
    ));
    assert!(client.check_update(&config_key).is_err());
    assert!(client.check_update(&config_key).is_err());
    assert!(matches!(
        client.check_update(&config_key).unwrap(),
        UpdateState::UpToDate
    ));
    server.join().unwrap();
    let requests = requests.lock().unwrap();
    assert!(!requests.iter().any(|r| r.line.contains("desktop-updates/")));
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
    replies.extend(baseline(&company).into_iter().skip(1).take(24));
    let (config, server, requests) = fixture(replies);
    let (mut client, data) = CloudClient::connect(
        config,
        "person@example.invalid".into(),
        "synthetic passphrase".into(),
    )
    .unwrap();
    let mut draft = AccountDraft::new();
    draft.email = "employee@example.invalid".into();
    draft.employee_name = "Synthetic Employee".into();
    draft.active = false;
    draft.password = "synthetic initial passphrase".into();
    draft.confirmation = draft.password.clone();
    let (_, message) = client
        .execute_admin(AdminAction::Save(draft), &data)
        .unwrap();
    assert!(message.contains("planned portal"));
    server.join().unwrap();
    let requests = requests.lock().unwrap();
    let body = requests[27].json();
    assert_eq!(body["active"], false);
    assert_eq!(body["role"], "employee");
    assert!(body["employee_id"].is_null());
    assert_eq!(body["password"], "synthetic initial passphrase");
    assert!(!body.to_string().contains("confirmation"));
    assert!(requests[27].headers.contains("profile-jwt"));
    assert!(
        requests
            .iter()
            .filter(|r| !r.line.starts_with("POST /functions/v1/account-admin "))
            .all(|r| !String::from_utf8_lossy(&r.body).contains("synthetic initial passphrase"))
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
            ..Default::default()
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
        requests[27].json(),
        json!({"data":{"hris_profile":{"full_name":"Alex Morgan","preferred_name":"Morgan","contact_phone":""}}})
    );
    assert!(requests[28].json().get("app_metadata").is_none());
    assert!(
        requests[29]
            .headers
            .to_lowercase()
            .contains("x-upsert: true")
    );
    assert_eq!(
        image::load_from_memory(&requests[29].body).unwrap().width(),
        256
    );
    assert_eq!(requests[32].json(), json!({"prefixes":[path]}));
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
        requests[28].json(),
        json!({"password":"new synthetic passphrase","current_password":"old synthetic passphrase"})
    );
    assert!(
        requests[28]
            .headers
            .to_lowercase()
            .contains("authorization: bearer fresh-password-jwt")
    );
    assert_eq!(
        requests[30].json(),
        json!({"email":"updated@example.invalid"})
    );
    assert!(
        requests[30]
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
fn automatic_demo_account_employee_has_a_creation_history_and_role_only_edit_adds_none() {
    let (commands, updates) =
        backend::worker(Some(DemoStore::new()), eframe::egui::Context::default());
    let mut draft = AccountDraft::new();
    draft.email = "history-auto@example.invalid".into();
    draft.employee_name = "Synthetic automatic history".into();
    draft.role = Role::HrGeneralist;
    draft.password = "synthetic fixture password".into();
    draft.confirmation = draft.password.clone();
    commands
        .send(Command::Admin(AdminAction::Save(draft)))
        .unwrap();
    let data = updates
        .recv_timeout(Duration::from_secs(5))
        .unwrap()
        .result
        .unwrap();
    let account = data
        .accounts
        .iter()
        .find(|a| a.email == "history-auto@example.invalid")
        .unwrap();
    let employee = account.employee_id.unwrap();
    let events = hris_desktop::history::timeline(&data, employee);
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].kind, hris_desktop::history::HistoryKind::Created);
    let before = data.employment_history.clone();
    let mut edit = AccountDraft::from_account(account);
    edit.active = false;
    commands
        .send(Command::Admin(AdminAction::Save(edit)))
        .unwrap();
    let changed = updates
        .recv_timeout(Duration::from_secs(5))
        .unwrap()
        .result
        .unwrap();
    assert_eq!(changed.employment_history, before);
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
