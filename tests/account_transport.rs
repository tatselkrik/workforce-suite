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
fn response_at<'a>(replies: &'a mut [Reply], prefix: &str) -> &'a mut Reply {
    replies
        .iter_mut()
        .find(|r| r.prefix.starts_with(prefix))
        .expect("Expected fixture response")
}
fn request_at<'a>(requests: &'a [Request], prefix: &str, n: usize) -> &'a Request {
    requests
        .iter()
        .filter(|r| r.line.starts_with(prefix))
        .nth(n)
        .expect("Expected request")
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

#[test]
fn applicant_creation_retries_one_identity_after_saved_write_and_failed_refresh() {
    use hris_desktop::{domain::Action, hiring::*};
    let mut demo = DemoStore::new();
    let company = demo.data.company.clone();
    let mut draft = ApplicantDraft::new(company.today());
    draft.full_name = "Synthetic Applicant".into();
    draft.position = "HR Associate".into();
    draft.email = "candidate@example.invalid".into();
    let action = HiringAction::Create {
        draft,
        reason: "Application recorded".into(),
    };
    demo.execute(Action::Hiring(action.clone())).unwrap();
    let saved = demo.data.hiring.records[0].clone();
    let mut replies = baseline(&company);
    replies.push(reply(
        "POST /rest/v1/rpc/manage_applicant ",
        200,
        json!(saved),
    ));
    replies.push(reply(
        "POST /rest/v1/rpc/my_hris_access ",
        500,
        json!({"message":"Synthetic refresh failure"}),
    ));
    replies.push(reply(
        "POST /rest/v1/rpc/manage_applicant ",
        200,
        json!(saved),
    ));
    let mut after = baseline(&company);
    for r in &mut after {
        if r.prefix.starts_with("GET /rest/v1/applicants?") {
            r.body = serde_json::to_vec(&vec![saved.clone()]).unwrap();
        }
        if r.prefix.starts_with("GET /rest/v1/applicant_events?") {
            r.body = serde_json::to_vec(&demo.data.hiring.events).unwrap();
        }
    }
    replies.extend(
        after
            .into_iter()
            .skip(1)
            .take_while(|r| !r.prefix.starts_with("GET /auth/v1/user ")),
    );
    let (config, server, requests) = fixture(replies);
    let (mut client, data) = CloudClient::connect(
        config,
        "person@example.invalid".into(),
        "synthetic passphrase".into(),
    )
    .unwrap();
    assert!(
        client
            .execute(Action::Hiring(action.clone()), &data)
            .unwrap_err()
            .to_string()
            .contains("Application saved, but refresh failed")
    );
    let result = client.execute(Action::Hiring(action), &data).unwrap();
    assert_eq!(result.hiring.records, vec![saved.clone()]);
    assert_eq!(result.hiring.events.len(), 1);
    server.join().unwrap();
    let req = requests.lock().unwrap();
    let writes: Vec<_> = req
        .iter()
        .filter(|r| r.line.starts_with("POST /rest/v1/rpc/manage_applicant "))
        .collect();
    assert_eq!(writes.len(), 2);
    assert_eq!(writes[0].json(), writes[1].json());
    let p = writes[0].json();
    assert_eq!(p["p_company_id"], json!(company.id));
    assert_eq!(p["p_record"]["id"], json!(saved.id));
    assert_eq!(p["p_operation"], "create");
    for protected in [
        "company_id",
        "actor_id",
        "revision",
        "stage",
        "employee_id",
        "user_id",
        "password",
    ] {
        assert!(p["p_record"].get(protected).is_none());
    }
    for r in req
        .iter()
        .filter(|r| r.line.starts_with("GET /rest/v1/applicant"))
    {
        assert!(r.line.contains("company_id=eq.") && r.line.contains("order=id.asc"));
    }
    assert!(
        req.iter()
            .all(|r| !r.line.starts_with("POST /rest/v1/employees")
                && !r.line.starts_with("POST /functions/v1/account-admin"))
    );
}
#[test]
fn applicant_response_from_another_record_or_company_is_rejected_before_refresh() {
    use hris_desktop::{domain::Action, hiring::*};
    for foreign_company in [false, true] {
        let mut demo = DemoStore::new();
        let company = demo.data.company.clone();
        let mut d = ApplicantDraft::new(company.today());
        d.full_name = "Synthetic Applicant".into();
        d.position = "Associate".into();
        let action = HiringAction::Create {
            draft: d,
            reason: "Application recorded".into(),
        };
        demo.execute(Action::Hiring(action.clone())).unwrap();
        let mut saved = demo.data.hiring.records[0].clone();
        if foreign_company {
            saved.company_id = uuid::Uuid::new_v4();
        } else {
            saved.id = uuid::Uuid::new_v4();
        }
        let mut replies = baseline(&company);
        replies.push(reply(
            "POST /rest/v1/rpc/manage_applicant ",
            200,
            json!(saved),
        ));
        let (config, server, _) = fixture(replies);
        let (mut client, data) = CloudClient::connect(
            config,
            "person@example.invalid".into(),
            "synthetic passphrase".into(),
        )
        .unwrap();
        assert!(
            client
                .execute(Action::Hiring(action), &data)
                .unwrap_err()
                .to_string()
                .contains("company/record")
        );
        server.join().unwrap();
    }
}

#[test]
fn notification_read_operations_use_own_guarded_rpc_and_reload_composite_key_state() {
    use hris_desktop::{domain::Action, notifications::ReadAction};
    let company = Company::default();
    let key = "checkout:00000000-0000-0000-0000-000000000001:0";
    let saved = json!({"user_id":company.owner_id,"company_id":company.id,"read_keys":[key]});
    let mut replies = baseline(&company);
    replies.push(reply(
        "POST /rest/v1/rpc/set_notification_reads ",
        200,
        saved.clone(),
    ));
    let mut after = baseline(&company);
    response_at(&mut after, "GET /rest/v1/notification_read_state?").body =
        serde_json::to_vec(&json!([saved])).unwrap();
    replies.extend(
        after
            .into_iter()
            .skip(1)
            .take_while(|r| !r.prefix.starts_with("GET /auth/v1/user ")),
    );
    let (config, server, requests) = fixture(replies);
    let (mut client, data) = CloudClient::connect(
        config,
        "person@example.invalid".into(),
        "synthetic passphrase".into(),
    )
    .unwrap();
    let next = client
        .execute(
            Action::NotificationReads(ReadAction::Mark {
                keys: vec![key.into()],
                read: true,
            }),
            &data,
        )
        .unwrap();
    assert_eq!(next.notification_reads.read_keys, vec![key]);
    server.join().unwrap();
    let req = requests.lock().unwrap();
    let body = req
        .iter()
        .find(|r| {
            r.line
                .starts_with("POST /rest/v1/rpc/set_notification_reads ")
        })
        .unwrap()
        .json();
    assert_eq!(
        body,
        json!({"p_company_id":company.id,"p_keys":[key],"p_read":true,"p_reset":false})
    );
    assert!(
        req.iter()
            .filter(|r| r.line.starts_with("GET /rest/v1/notification_read_state?"))
            .all(|r| r.line.contains("order=user_id.asc%2Ccompany_id.asc")
                && r.line.contains("company_id=eq."))
    );
    assert!(!req.iter().any(|r| r.line.starts_with("PUT /auth/v1/user")));
}

#[test]
fn notification_history_from_another_account_is_rejected_before_refresh() {
    use hris_desktop::{domain::Action, notifications::ReadAction};
    let company = Company::default();
    let mut replies = baseline(&company);
    replies.push(reply(
        "POST /rest/v1/rpc/set_notification_reads ",
        200,
        json!({"user_id":uuid::Uuid::new_v4(),"company_id":company.id,"read_keys":[]}),
    ));
    let (config, server, _) = fixture(replies);
    let (mut client, data) = CloudClient::connect(
        config,
        "person@example.invalid".into(),
        "synthetic passphrase".into(),
    )
    .unwrap();
    assert!(
        client
            .execute(Action::NotificationReads(ReadAction::Reset), &data)
            .unwrap_err()
            .to_string()
            .contains("account/company")
    );
    server.join().unwrap();
}

#[test]
fn saved_report_preferences_create_edit_reload_and_delete_only_the_own_guarded_row() {
    use hris_desktop::{
        report_views::{Period, View},
        reports::{Filter, Kind},
    };
    let company = Company::default();
    let mut filter = Filter::new(company.today());
    filter.columns = Some(vec!["Work date".into(), "Employee code".into()]);
    let view = View::new(
        company.id,
        "Synthetic team attendance".into(),
        filter,
        Period::Last30,
    )
    .unwrap();
    let mut edited = view.clone();
    edited.name = "Synthetic monthly leave".into();
    edited.period = Period::ThisMonth;
    edited.filter.kind = Kind::Leave;
    edited.filter.columns = Some(vec!["Charge date".into(), "Full name".into()]);
    let row = |views: Value, revision| json!({"user_id":company.owner_id,"company_id":company.id,"views":views,"revision":revision});
    let mut replies = baseline(&company);
    replies.extend([
        reply("GET /rest/v1/report_preferences?", 200, json!([])),
        reply(
            "POST /rest/v1/rpc/save_report_preferences ",
            200,
            row(json!([view]), 1),
        ),
        reply(
            "POST /rest/v1/rpc/save_report_preferences ",
            200,
            row(json!([edited]), 2),
        ),
        reply("GET /auth/v1/user ", 200, user(&company)),
        reply(
            format!(
                "GET /storage/v1/object/authenticated/profile-photos/{}/avatar.png ",
                company.owner_id
            ),
            404,
            json!({"code":"NoSuchKey"}),
        ),
        reply(
            "GET /rest/v1/report_preferences?",
            200,
            json!([row(json!([edited]), 2)]),
        ),
        reply(
            "POST /rest/v1/rpc/save_report_preferences ",
            200,
            row(json!([]), 3),
        ),
    ]);
    let (config, server, requests) = fixture(replies);
    let mut client = connect(config);
    client
        .execute_account(AccountAction::LoadReportViews {
            company_id: company.id,
        })
        .unwrap();
    client
        .execute_account(AccountAction::SaveReportViews {
            company_id: company.id,
            expected_revision: 0,
            views: vec![view.clone()],
        })
        .unwrap();
    assert_eq!(client.account().report_views, vec![view.clone()]);
    client
        .execute_account(AccountAction::SaveReportViews {
            company_id: company.id,
            expected_revision: 1,
            views: vec![edited.clone()],
        })
        .unwrap();
    client.refresh_account().unwrap();
    assert_eq!(client.account().report_views, vec![edited.clone()]);
    assert_eq!(client.account().report_views_revision, 2);
    client
        .execute_account(AccountAction::LoadReportViews {
            company_id: company.id,
        })
        .unwrap();
    assert_eq!(client.account().report_views, vec![edited.clone()]);
    assert_eq!(client.account().details.preferred_name, "Alex");
    assert!(
        client
            .execute_account(AccountAction::SaveReportViews {
                company_id: company.id,
                expected_revision: 2,
                views: vec![view.clone(), view.clone()]
            })
            .is_err()
    );
    assert!(
        client
            .execute_account(AccountAction::SaveReportViews {
                company_id: company.id,
                expected_revision: 1,
                views: vec![]
            })
            .is_err()
    );
    client
        .execute_account(AccountAction::SaveReportViews {
            company_id: company.id,
            expected_revision: 2,
            views: vec![],
        })
        .unwrap();
    assert!(client.account().report_views.is_empty());
    server.join().unwrap();
    let requests = requests.lock().unwrap();
    let writes: Vec<_> = requests
        .iter()
        .filter(|r| {
            r.line
                .starts_with("POST /rest/v1/rpc/save_report_preferences ")
        })
        .collect();
    assert_eq!(writes.len(), 3);
    for (write, revision, views) in [
        (writes[0], 0, json!([view])),
        (writes[1], 1, json!([edited])),
        (writes[2], 2, json!([])),
    ] {
        assert_eq!(
            write.json(),
            json!({"p_company_id":company.id,"p_expected_revision":revision,"p_views":views})
        );
        assert!(write.headers.contains("profile-jwt"));
    }
    assert!(
        requests
            .iter()
            .all(|r| !r.line.starts_with("PUT /auth/v1/user "))
    );
}
#[test]
fn saved_view_conflicts_and_wrong_account_responses_preserve_the_loaded_preferences() {
    use hris_desktop::{
        report_views::{Period, View},
        reports::Filter,
    };
    let company = Company::default();
    let view = View::new(
        company.id,
        "Synthetic existing".into(),
        Filter::new(company.today()),
        Period::Fixed,
    )
    .unwrap();
    let mut replies = baseline(&company);
    replies.extend([
        reply("GET /rest/v1/report_preferences?",200,json!([{"user_id":company.owner_id,"company_id":company.id,"views":[view],"revision":4}])),
        reply("POST /rest/v1/rpc/save_report_preferences ",409,json!({"code":"PT409","message":"Saved views changed in another session. Reload views."})),
        reply("GET /rest/v1/report_preferences?",200,json!([{"user_id":uuid::Uuid::new_v4(),"company_id":company.id,"views":[],"revision":8}])),
    ]);
    let (config, server, _) = fixture(replies);
    let mut client = connect(config);
    client
        .execute_account(AccountAction::LoadReportViews {
            company_id: company.id,
        })
        .unwrap();
    assert!(
        client
            .execute_account(AccountAction::SaveReportViews {
                company_id: company.id,
                expected_revision: 4,
                views: vec![]
            })
            .unwrap_err()
            .to_string()
            .contains("another session")
    );
    assert!(
        client
            .execute_account(AccountAction::LoadReportViews {
                company_id: company.id
            })
            .is_err()
    );
    assert_eq!(client.account().report_views, vec![view]);
    assert_eq!(client.account().report_views_revision, 4);
    server.join().unwrap();
}

#[test]
fn invalid_report_metadata_does_not_break_sign_in_or_change_membership() {
    let company = Company::default();
    let mut malicious = user(&company);
    malicious["user_metadata"]["hris_report_views"] =
        json!({"role":"administrator","employee_id":"arbitrary"});
    let mut replies = baseline(&company);
    replies[0].body = serde_json::to_vec(&token(malicious.clone(), "profile-jwt")).unwrap();
    response_at(&mut replies, "GET /auth/v1/user ").body = serde_json::to_vec(&malicious).unwrap();
    let (config, server, _) = fixture(replies);
    let (client, data) = CloudClient::connect(
        config,
        "person@example.invalid".into(),
        "synthetic passphrase".into(),
    )
    .unwrap();
    assert!(client.account().report_views.is_empty());
    assert_eq!(data.access.role, Role::Administrator);
    server.join().unwrap();
}

#[test]
fn a_late_legacy_view_is_offered_until_saved_and_an_empty_database_row_never_revives_it() {
    use hris_desktop::{
        report_views::{Period, View},
        reports::Filter,
    };
    let company = Company::default();
    let legacy = View::new(
        company.id,
        "Synthetic late legacy view".into(),
        Filter::new(company.today()),
        Period::Fixed,
    )
    .unwrap();
    let mut account = user(&company);
    account["user_metadata"]["hris_report_views"] = json!([legacy]);
    let mut replies = baseline(&company);
    replies[0].body = serde_json::to_vec(&token(account.clone(), "profile-jwt")).unwrap();
    response_at(&mut replies, "GET /auth/v1/user ").body = serde_json::to_vec(&account).unwrap();
    let empty = json!({"user_id":company.owner_id,"company_id":company.id,"views":[],"revision":1});
    replies.extend([
        reply("GET /rest/v1/report_preferences?", 200, json!([])),
        reply(
            "POST /rest/v1/rpc/save_report_preferences ",
            200,
            empty.clone(),
        ),
        reply("GET /rest/v1/report_preferences?", 200, json!([empty])),
    ]);
    let (config, server, _) = fixture(replies);
    let mut client = connect(config);
    client
        .execute_account(AccountAction::LoadReportViews {
            company_id: company.id,
        })
        .unwrap();
    assert_eq!(client.account().report_views, vec![legacy]);
    assert_eq!(client.account().report_views_revision, 0);
    client
        .execute_account(AccountAction::SaveReportViews {
            company_id: company.id,
            expected_revision: 0,
            views: vec![],
        })
        .unwrap();
    client
        .execute_account(AccountAction::LoadReportViews {
            company_id: company.id,
        })
        .unwrap();
    assert!(client.account().report_views.is_empty());
    assert_eq!(client.account().report_views_revision, 1);
    server.join().unwrap();
}

#[test]
fn employee_import_retries_the_same_batch_after_a_saved_write_with_failed_refresh() {
    use hris_desktop::{
        domain::Action,
        imports::{Batch, Preview},
    };
    let company = Company::default();
    let mut seed = DemoStore::new().data;
    seed.company = company.clone();
    seed.access.company_id = company.id;
    let preview = Preview::from_csv(
        &seed,
        "Synthetic.csv".into(),
        b"employee_code,full_name\nNEW-HTTP-901,Synthetic imported employee\n",
    )
    .unwrap();
    let batch = Batch {
        id: preview.id,
        company_id: company.id,
        actor_id: company.owner_id,
        actor_name: "Synthetic HR".into(),
        actor_role: Role::Administrator,
        row_count: 1,
        payload_sha256: "a".repeat(64),
        reason: "Synthetic verified onboarding".into(),
        created_at: chrono::Utc::now(),
    };
    let mut replies = baseline(&company);
    replies.extend(
        baseline(&company)
            .into_iter()
            .skip(1)
            .take_while(|r| !r.prefix.starts_with("GET /auth/v1/user ")),
    );
    replies.push(reply(
        "POST /rest/v1/rpc/import_employees ",
        200,
        json!(preview.id),
    ));
    replies.push(reply(
        "POST /rest/v1/rpc/my_hris_access ",
        503,
        json!({"message":"Synthetic refresh failure"}),
    ));
    let after = || {
        let mut r = baseline(&company);
        for response in &mut r {
            if response
                .prefix
                .starts_with("GET /rest/v1/employee_import_batches?")
            {
                response.body = serde_json::to_vec(&vec![batch.clone()]).unwrap();
            }
            if response.prefix.starts_with("GET /rest/v1/employees?") {
                response.body =
                    serde_json::to_vec(&vec![preview.rows[0].employee.clone().unwrap()]).unwrap();
            }
        }
        r
    };
    replies.extend(
        after()
            .into_iter()
            .skip(1)
            .take_while(|r| !r.prefix.starts_with("GET /auth/v1/user ")),
    );
    replies.push(reply(
        "POST /rest/v1/rpc/import_employees ",
        200,
        json!(preview.id),
    ));
    replies.extend(
        after()
            .into_iter()
            .skip(1)
            .take_while(|r| !r.prefix.starts_with("GET /auth/v1/user ")),
    );
    let (config, server, requests) = fixture(replies);
    let (mut client, initial) = CloudClient::connect(
        config,
        "person@example.invalid".into(),
        "synthetic passphrase".into(),
    )
    .unwrap();
    let action = Action::ImportEmployees(preview.clone(), "Synthetic verified onboarding".into());
    assert!(
        client
            .execute(action.clone(), &initial)
            .unwrap_err()
            .to_string()
            .contains("Import saved")
    );
    let data = client.execute(action, &initial).unwrap();
    assert_eq!(data.imports.batches.len(), 1);
    assert_eq!(data.employees.len(), 1);
    server.join().unwrap();
    let requests = requests.lock().unwrap();
    let writes: Vec<_> = requests
        .iter()
        .filter(|r| r.line.starts_with("POST /rest/v1/rpc/import_employees "))
        .collect();
    assert_eq!(writes.len(), 2);
    assert_eq!(writes[0].json(), writes[1].json());
    assert_eq!(writes[0].json()["p_batch_id"], preview.id.to_string());
    assert!(
        writes[0].json()["p_rows"][0]["employee"]
            .get("id")
            .is_none()
    );
    assert!(writes[0].json().get("actor_id").is_none());
}

#[test]
fn report_export_reloads_scoped_data_and_rechecks_access_before_creating_csv() {
    use hris_desktop::reports::{Filter, Kind};
    let store = DemoStore::new();
    let company = store.data.company.clone();
    let mut initial = baseline(&company);
    initial[3] = reply(
        "GET /rest/v1/employees?",
        200,
        json!([store.data.employees[0]]),
    );
    let mut fresh = baseline(&company);
    let mut changed = store.data.employees[0].clone();
    changed.full_name = "Synthetic refreshed report employee".into();
    fresh[3] = reply("GET /rest/v1/employees?", 200, json!([changed]));
    initial.extend(
        fresh
            .into_iter()
            .skip(1)
            .take_while(|r| !r.prefix.starts_with("GET /auth/v1/user ")),
    );
    initial.push(reply("POST /rest/v1/rpc/my_hris_access ",200,json!({"company_id":company.id,"user_id":company.owner_id,"role":"administrator","employee_id":null,"direct_reports":[]})));
    let (config, server, requests) = fixture(initial);
    let (mut client, _) = CloudClient::connect(
        config,
        "person@example.invalid".into(),
        "synthetic passphrase".into(),
    )
    .unwrap();
    let mut f = Filter::new(company.today());
    f.kind = Kind::Summary;
    f.include_missing = false;
    let path = std::env::temp_dir().join(format!("hris-fresh-report-{}.csv", uuid::Uuid::new_v4()));
    let (_, report) = client.report(&f, Some(&path)).unwrap();
    assert_eq!(report.rows.len(), 1);
    let csv = std::fs::read_to_string(&path).unwrap();
    assert!(csv.contains("Synthetic refreshed report employee"));
    assert!(!csv.contains("Alex Rivera"));
    std::fs::remove_file(path).unwrap();
    server.join().unwrap();
    let requests = requests.lock().unwrap();
    assert_eq!(
        requests
            .iter()
            .filter(|r| r.line.starts_with("POST /rest/v1/rpc/my_hris_access "))
            .count(),
        3
    );
    assert!(
        requests
            .iter()
            .filter(|r| r.line.starts_with("POST /rest/v1/"))
            .all(|r| r.line.starts_with("POST /rest/v1/rpc/my_hris_access ")
                || r.line
                    .starts_with("POST /rest/v1/rpc/list_company_accounts ")
                || r.line
                    .starts_with("POST /rest/v1/rpc/recruiting_interviewers ")
                || r.line.starts_with("POST /rest/v1/rpc/lifecycle_notifications ")
                || r.line.starts_with("POST /rest/v1/rpc/leave_credit_context ")
                || r.line.starts_with("POST /rest/v1/rpc/leave_dated_context ")
                || r.line.starts_with("POST /rest/v1/rpc/approval_delegation_context ")
                || r.line.starts_with("POST /rest/v1/rpc/exit_readiness_list "))
    );
}

#[test]
fn report_access_changed_during_loading_prevents_local_export() {
    use hris_desktop::reports::{Filter, Kind};
    let company = Company::default();
    let mut replies = baseline(&company);
    replies.extend(
        baseline(&company)
            .into_iter()
            .skip(1)
            .take_while(|r| !r.prefix.starts_with("GET /auth/v1/user ")),
    );
    replies.push(reply("POST /rest/v1/rpc/my_hris_access ",200,json!({"company_id":company.id,"user_id":company.owner_id,"role":"manager","employee_id":null,"direct_reports":[]})));
    let (config, server, _) = fixture(replies);
    let (mut client, _) = CloudClient::connect(
        config,
        "person@example.invalid".into(),
        "synthetic passphrase".into(),
    )
    .unwrap();
    let mut f = Filter::new(company.today());
    f.kind = Kind::Summary;
    let path =
        std::env::temp_dir().join(format!("hris-denied-report-{}.csv", uuid::Uuid::new_v4()));
    assert!(client.report(&f, Some(&path)).is_err());
    assert!(!path.exists());
    server.join().unwrap();
}

#[test]
fn revoked_report_access_fails_before_reading_records_or_writing_a_file() {
    use hris_desktop::reports::Filter;
    let company = Company::default();
    let mut replies = baseline(&company);
    replies.push(reply(
        "POST /rest/v1/rpc/my_hris_access ",
        403,
        json!({"message":"Access disabled"}),
    ));
    let (config, server, requests) = fixture(replies);
    let (mut client, _) = CloudClient::connect(
        config,
        "person@example.invalid".into(),
        "synthetic passphrase".into(),
    )
    .unwrap();
    let path =
        std::env::temp_dir().join(format!("hris-revoked-report-{}.csv", uuid::Uuid::new_v4()));
    assert!(
        client
            .report(&Filter::new(company.today()), Some(&path))
            .is_err()
    );
    assert!(!path.exists());
    server.join().unwrap();
    let requests = requests.lock().unwrap();
    assert_eq!(
        requests.last().unwrap().line,
        "POST /rest/v1/rpc/my_hris_access HTTP/1.1"
    );
}

#[test]
fn attendance_requests_use_guarded_rpc_and_reload_proposals_and_decision_history() {
    use chrono::Duration;
    use hris_desktop::{
        corrections::CorrectionDraft,
        domain::*,
        requests::{RequestAction, Status},
    };
    let mut store = DemoStore::new();
    store.data.attendance.clear();
    store.data.leave.clear();
    store.data.employees[0].joined_on = None;
    let mut draft = CorrectionDraft::new(
        store.data.employees[0].id,
        store.data.company.today() - Duration::days(3),
        None,
        &store.data,
    );
    draft.checked_in = "08:25".into();
    draft.checked_out = "17:00".into();
    draft.reason = "Synthetic missing punch".into();
    let original = store.data.clone();
    store
        .execute(Action::AttendanceRequest(RequestAction::Submit(
            draft.clone(),
        )))
        .unwrap();
    let id = store.data.requests.records[0].id;
    let prepare = |data: &Snapshot| {
        let mut responses = baseline(&data.company);
        for r in &mut responses {
            r.body = if r.prefix.starts_with("POST /rest/v1/rpc/my_hris_access ") {
                serde_json::to_vec(&json!({"company_id":data.company.id,"user_id":data.access.user_id,"role":"administrator","employee_id":data.access.employee_id,"direct_reports":[]})).unwrap()
            } else if r.prefix.starts_with("GET /rest/v1/employees?") {
                serde_json::to_vec(&data.employees).unwrap()
            } else if r.prefix.starts_with("GET /rest/v1/attendance_requests?") {
                serde_json::to_vec(&data.requests.records).unwrap()
            } else if r
                .prefix
                .starts_with("GET /rest/v1/attendance_request_events?")
            {
                serde_json::to_vec(&data.requests.events).unwrap()
            } else {
                r.body.clone()
            };
        }
        responses
    };
    let mut replies = prepare(&original);
    replies.push(reply(
        "POST /rest/v1/rpc/attendance_request ",
        200,
        json!(id),
    ));
    replies.extend(
        prepare(&store.data)
            .into_iter()
            .skip(1)
            .take_while(|r| !r.prefix.starts_with("GET /auth/v1/user ")),
    );
    replies.push(reply(
        "POST /rest/v1/rpc/attendance_request ",
        409,
        json!({"code":"PT409","message":"This request changed. Refresh and reopen it."}),
    ));
    let (config, server, requests) = fixture(replies);
    let (mut client, loaded) = CloudClient::connect(
        config,
        "person@example.invalid".into(),
        "synthetic-password".into(),
    )
    .unwrap();
    let saved = client
        .execute(
            Action::AttendanceRequest(RequestAction::Submit(draft)),
            &loaded,
        )
        .unwrap();
    assert_eq!(saved.requests.records.len(), 1);
    assert_eq!(saved.requests.events.len(), 1);
    assert!(saved.attendance.is_empty());
    let error = client
        .execute(
            Action::AttendanceRequest(RequestAction::Resolve {
                id,
                expected_revision: 0,
                status: Status::Cancelled,
                reason: "Synthetic cancellation".into(),
            }),
            &saved,
        )
        .unwrap_err();
    assert!(error.to_string().contains("changed"));
    server.join().unwrap();
    let requests = requests.lock().unwrap();
    let sent: Vec<_> = requests
        .iter()
        .filter(|r| r.line.starts_with("POST /rest/v1/rpc/attendance_request "))
        .collect();
    assert_eq!(sent.len(), 2);
    let p = sent[0].json();
    assert_eq!(p["p_request"]["operation"], "submit");
    assert_eq!(p["p_request"]["expected_timezone"], loaded.company.timezone);
    for key in [
        "employee_id",
        "submitted_by",
        "actor_id",
        "status",
        "base_record",
    ] {
        assert!(p["p_request"].get(key).is_none());
    }
}

fn fixture(replies: Vec<Reply>) -> (ConnectionConfig, thread::JoinHandle<()>, Requests) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let requests = Arc::new(Mutex::new(Vec::new()));
    let observed = requests.clone();
    let server = thread::spawn(move || {
        let deadline = Instant::now() + Duration::from_secs(20);
        let mut last_access=Value::Null;
        let mut last_leave_policy=Value::Null;
        for mut reply in replies {
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
            if reply.prefix=="POST /rest/v1/rpc/my_hris_access "&&reply.status==200{last_access=serde_json::from_slice(&reply.body).unwrap();}
            if reply.prefix=="GET /rest/v1/leave_policies?"&&reply.status==200{let rows:Value=serde_json::from_slice(&reply.body).unwrap();last_leave_policy=rows[0].clone();}
            if reply.prefix=="POST /rest/v1/rpc/leave_dated_context " {
                let wire:Value=serde_json::from_slice(&reply.body).unwrap();
                if wire.get("fixture_empty")==Some(&json!(true)){
                    reply.body=serde_json::to_vec(&json!({"version":"leave-dated-v1","company_id":last_access["company_id"],"user_id":last_access["user_id"],"employee_id":last_access["employee_id"],"role":last_access["role"],"as_of":wire["as_of"],"revision":0,"enabled":false,"timeline":{"baseline":last_leave_policy,"versions":[]},"anchors":[],"openings":[],"withdrawals":[],"audit":[]})).unwrap();
                }
            }
            if reply.prefix=="POST /rest/v1/rpc/approval_delegation_context " {
                let wire:Value=serde_json::from_slice(&reply.body).unwrap();
                if wire.get("fixture_empty")==Some(&json!(true)){
                    reply.body=serde_json::to_vec(&json!({"version":"approval-delegation-v1","company_id":last_access["company_id"],"user_id":last_access["user_id"],"employee_id":last_access["employee_id"],"role":last_access["role"],"as_of":wire["as_of"],"revision":0,"grants":[],"revocations":[],"receipts":[],"permits":[],"audit":[],"staff":[]})).unwrap();
                }
            }
            if reply.prefix=="POST /rest/v1/rpc/leave_credit_context " {
                let wire:Value=serde_json::from_slice(&reply.body).unwrap();
                if wire.get("fixture_empty")==Some(&json!(true)){
                    reply.body=serde_json::to_vec(&json!({"version":"leave-credits-v1","company_id":last_access["company_id"],"user_id":last_access["user_id"],"employee_id":last_access["employee_id"],"role":last_access["role"],"as_of":wire["as_of"],"revision":0,"entries":[],"audit":[]})).unwrap();
                }
            }
            if reply.prefix=="POST /rest/v1/rpc/lifecycle_notifications " {
                let mut wire:Value=serde_json::from_slice(&reply.body).unwrap();
                if wire.get("fixture_empty")==Some(&json!(true)){
                    wire=json!({"version":"lifecycle-notifications-v1","company_id":last_access["company_id"],"user_id":last_access["user_id"],"employee_id":last_access["employee_id"],"role":last_access["role"],"surface":"desktop","as_of":wire["as_of"],"generated_at":chrono::Utc::now(),"read_keys":[],"total_items":0,"truncated":false,"items":[]});
                    reply.body=serde_json::to_vec(&wire).unwrap();
                }
            }
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
        reply("POST /rest/v1/rpc/leave_credit_context ",200,json!({"fixture_empty":true,"as_of":company.today()})),
        reply("POST /rest/v1/rpc/leave_dated_context ",200,json!({"fixture_empty":true,"as_of":company.today()})),
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
        reply("GET /rest/v1/attendance_requests?", 200, json!([])),
        reply("GET /rest/v1/attendance_request_events?", 200, json!([])),
        reply("GET /rest/v1/overtime_requests?", 200, json!([])),
        reply("GET /rest/v1/overtime_events?", 200, json!([])),
        reply("GET /rest/v1/employee_import_batches?", 200, json!([])),
        reply("GET /rest/v1/employee_import_items?", 200, json!([])),
        reply("GET /rest/v1/applicants?", 200, json!([])),
        reply("GET /rest/v1/applicant_events?", 200, json!([])),
        reply("POST /rest/v1/rpc/recruiting_interviewers ", 200, json!([])),
        reply("GET /rest/v1/applicant_documents?", 200, json!([])),
        reply("GET /rest/v1/applicant_document_events?", 200, json!([])),
        reply("GET /rest/v1/recruiting_interviews?", 200, json!([])),
        reply("GET /rest/v1/interview_feedback?", 200, json!([])),
        reply("GET /rest/v1/interview_events?", 200, json!([])),
        reply("GET /rest/v1/hiring_offers?", 200, json!([])),
        reply("GET /rest/v1/hiring_offer_events?", 200, json!([])),
        reply("GET /rest/v1/hire_conversions?", 200, json!([])),
        reply("GET /rest/v1/onboarding_cases?", 200, json!([])),
        reply("GET /rest/v1/onboarding_tasks?", 200, json!([])),
        reply("GET /rest/v1/onboarding_events?", 200, json!([])),
        reply("GET /rest/v1/exit_cases?", 200, json!([])),
        reply("POST /rest/v1/rpc/exit_readiness_list ", 200, json!([])),
        reply("GET /rest/v1/clearance_tasks?", 200, json!([])),
        reply("GET /rest/v1/exit_events?", 200, json!([])),
        reply("GET /rest/v1/performance_goals?", 200, json!([])),
        reply("GET /rest/v1/performance_cycles?", 200, json!([])),
        reply("GET /rest/v1/performance_reviews?", 200, json!([])),
        reply("GET /rest/v1/performance_hr_notes?", 200, json!([])),
        reply("GET /rest/v1/performance_events?", 200, json!([])),
        reply("GET /rest/v1/employee_training?", 200, json!([])),
        reply("GET /rest/v1/employee_certifications?", 200, json!([])),
        reply("GET /rest/v1/certification_evidence?", 200, json!([])),
        reply("GET /rest/v1/learning_events?", 200, json!([])),
        reply("GET /rest/v1/career_plans?", 200, json!([])),
        reply("GET /rest/v1/career_milestones?", 200, json!([])),
        reply("GET /rest/v1/career_events?", 200, json!([])),
        reply("GET /rest/v1/payroll_slips?",200,json!([])),
        reply("GET /rest/v1/payroll_cutoff_slips?",200,json!([])),
        reply("GET /rest/v1/payroll_thirteenth_slips?",200,json!([])),
        reply("GET /rest/v1/payroll_final_slips?",200,json!([])),
        reply("GET /rest/v1/payroll_final_status?",200,json!([])),
        reply("GET /rest/v1/payroll_amendment_slips?",200,json!([])),
        reply("GET /rest/v1/payroll_amendment_status?",200,json!([])),
        reply("GET /rest/v1/payroll_tax_slips?",200,json!([])),
        reply("GET /rest/v1/payroll_amendment_cases?",200,json!([])),
        reply("GET /rest/v1/payroll_wage_classifications?",200,json!([])),
        reply("GET /rest/v1/payroll_wage_settings?",200,json!([])),
        reply("GET /rest/v1/payroll_wage_check_index?",200,json!([])),
        reply("GET /rest/v1/payroll_profiles?",200,json!([])),
        reply("GET /rest/v1/payroll_holidays?",200,json!([])),
        reply("GET /rest/v1/payroll_runs?",200,json!([])),
        reply("GET /rest/v1/payroll_items?",200,json!([])),
        reply("GET /rest/v1/payroll_events?",200,json!([])),
        reply("GET /rest/v1/payroll_cutoff_runs?",200,json!([])),
        reply("GET /rest/v1/payroll_cutoff_items?",200,json!([])),
        reply("GET /rest/v1/payroll_thirteenth_runs?",200,json!([])),
        reply("GET /rest/v1/payroll_thirteenth_items?",200,json!([])),
        reply("GET /rest/v1/payroll_final_cases?",200,json!([])),
        reply("GET /rest/v1/payroll_tax_reconciliations?",200,json!([])),
        reply("GET /rest/v1/payroll_payment_receipts?",200,json!([])),
        reply("GET /rest/v1/notification_read_state?", 200, json!([])),
        reply("POST /rest/v1/rpc/lifecycle_notifications ",200,json!({"fixture_empty":true,"as_of":company.today()})),
        reply("POST /rest/v1/rpc/approval_delegation_context ",200,json!({"fixture_empty":true,"as_of":company.today()})),
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
    replies.extend(
        after
            .into_iter()
            .skip(1)
            .take_while(|r| !r.prefix.starts_with("GET /auth/v1/user ")),
    );
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
    replies.extend(
        after
            .into_iter()
            .skip(1)
            .take_while(|r| !r.prefix.starts_with("GET /auth/v1/user ")),
    );
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
                .take_while(|r| !r.prefix.starts_with("GET /auth/v1/user ")),
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
                    .take_while(|r| !r.prefix.starts_with("GET /auth/v1/user ")),
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
            .take_while(|r| !r.prefix.starts_with("GET /auth/v1/user ")),
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
    replies.extend(
        after
            .into_iter()
            .skip(1)
            .take_while(|r| !r.prefix.starts_with("GET /auth/v1/user ")),
    );
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
    replies.extend(
        after
            .into_iter()
            .skip(1)
            .take_while(|r| !r.prefix.starts_with("GET /auth/v1/user ")),
    );
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
    replies.extend(
        after
            .into_iter()
            .skip(1)
            .take_while(|r| !r.prefix.starts_with("GET /auth/v1/user ")),
    );
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
    replies.extend(
        after
            .into_iter()
            .skip(1)
            .take_while(|r| !r.prefix.starts_with("GET /auth/v1/user ")),
    );
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
    let after: Vec<_> = after
        .into_iter()
        .take_while(|r| {
            !r.prefix
                .starts_with("GET /rest/v1/notification_read_state?")
        })
        .collect();
    replies.extend(after);
    replies.push(Reply {
        prefix: "GET /storage/v1/object/authenticated/company-branding/".into(),
        status: 200,
        body: png,
        mime: "image/png",
    });
    replies.push(reply(
        "GET /rest/v1/notification_read_state?",
        200,
        json!([]),
    ));
    replies.push(
        baseline(&revised)
            .into_iter()
            .find(|r| r.prefix.starts_with("POST /rest/v1/rpc/lifecycle_notifications "))
            .unwrap(),
    );
    replies.push(
        baseline(&revised).into_iter().find(|r|r.prefix.starts_with("POST /rest/v1/rpc/approval_delegation_context ")).unwrap(),
    );
    replies.extend(
        baseline(&revised)
            .into_iter()
            .skip(1)
            .take_while(|r| !r.prefix.starts_with("GET /auth/v1/user ")),
    );
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
    replies.extend(
        baseline(&company)
            .into_iter()
            .skip(1)
            .take_while(|r| !r.prefix.starts_with("GET /auth/v1/user ")),
    );
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
            .to_lowercase()
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
    replies.retain(|r| {
        !r.prefix.contains("/hiring_offer")
            && !r.prefix.contains("/hire_conversions?")
            && !r.prefix.contains("/onboarding_cases?")
            && !r.prefix.contains("/exit_cases?")
            && !r.prefix.contains("/performance_hr_notes?")
            && !r.prefix.contains("/certification_evidence?")
            && !r.prefix.contains("/rpc/exit_readiness_list ")
            && !r.prefix.contains("/applicant_document")
            && !r.prefix.contains("/rpc/recruiting_interviewers ")
            && !r.prefix.contains("/employee_import_")
            && !r.prefix.contains("/applicants?")
            && !r.prefix.contains("/applicant_events?")
            && !r.prefix.contains("/payroll_profiles?")
            && !r.prefix.contains("/payroll_holidays?")
            && !r.prefix.contains("/payroll_runs?")
            && !r.prefix.contains("/payroll_items?")
            && !r.prefix.contains("/payroll_events?")
            && !r.prefix.contains("/payroll_cutoff_runs?")
            && !r.prefix.contains("/payroll_cutoff_items?")
            && !r.prefix.contains("/payroll_thirteenth_runs?")
            && !r.prefix.contains("/payroll_thirteenth_items?")
            && !r.prefix.contains("/payroll_tax_reconciliations?")
            && !r.prefix.contains("/payroll_payment_receipts?")
            && !r.prefix.contains("/payroll_final_cases?")
            && !r.prefix.contains("/payroll_amendment_cases?")
            && !r.prefix.contains("/payroll_wage_")
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
                || r.line.contains("account_access_events")
                || r.line.contains("payroll_payment_receipts")
                || r.line.contains("payroll_final_cases")
                || r.line.contains("payroll_wage_")
                || r.line.contains("payroll_profiles")
                || r.line.contains("payroll_items")
                || r.line.contains("payroll_runs"))
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
            .take_while(|r| !r.prefix.starts_with("GET /auth/v1/user ")),
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
            .take_while(|r| !r.prefix.starts_with("GET /auth/v1/user ")),
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
    initial.extend(
        after
            .into_iter()
            .skip(1)
            .take_while(|r| !r.prefix.starts_with("GET /auth/v1/user ")),
    );
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
    replies.extend(
        baseline(&company)
            .into_iter()
            .skip(1)
            .take_while(|r| !r.prefix.starts_with("GET /auth/v1/user ")),
    );
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
    let body = request_at(&requests, "POST /functions/v1/account-admin ", 0).json();
    assert_eq!(body["active"], false);
    assert_eq!(body["role"], "employee");
    assert!(body["employee_id"].is_null());
    assert_eq!(body["password"], "synthetic initial passphrase");
    assert!(!body.to_string().contains("confirmation"));
    assert!(
        request_at(&requests, "POST /functions/v1/account-admin ", 0)
            .headers
            .contains("profile-jwt")
    );
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
        request_at(&requests, "PUT /auth/v1/user ", 0).json(),
        json!({"data":{"hris_profile":{"full_name":"Alex Morgan","preferred_name":"Morgan","contact_phone":""}}})
    );
    assert!(
        request_at(&requests, "PUT /auth/v1/user ", 1)
            .json()
            .get("app_metadata")
            .is_none()
    );
    assert!(
        request_at(&requests, "POST /storage/v1/object/profile-photos/", 0)
            .headers
            .to_lowercase()
            .contains("x-upsert: true")
    );
    assert_eq!(
        image::load_from_memory(
            &request_at(&requests, "POST /storage/v1/object/profile-photos/", 0).body
        )
        .unwrap()
        .width(),
        256
    );
    assert_eq!(
        request_at(&requests, "DELETE /storage/v1/object/profile-photos ", 0).json(),
        json!({"prefixes":[path]})
    );
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
        request_at(&requests, "PUT /auth/v1/user ", 0).json(),
        json!({"password":"new synthetic passphrase","current_password":"old synthetic passphrase"})
    );
    assert!(
        request_at(&requests, "PUT /auth/v1/user ", 0)
            .headers
            .to_lowercase()
            .contains("authorization: bearer fresh-password-jwt")
    );
    assert_eq!(
        request_at(&requests, "PUT /auth/v1/user ", 1).json(),
        json!({"email":"updated@example.invalid"})
    );
    assert!(
        request_at(&requests, "PUT /auth/v1/user ", 1)
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

#[test]
fn exit_creation_retries_original_payload_after_saved_write_and_failed_refresh() {
    use hris_desktop::{domain::Action, exits::*};
    let mut demo = DemoStore::new();
    let person = demo
        .data
        .employees
        .iter()
        .find(|e| Some(e.id) != demo.data.access.employee_id)
        .unwrap()
        .id;
    let mut draft = CaseDraft::new(&demo.data, Some(person));
    draft.reason = "Synthetic exit".into();
    let a = ExitAction::CaseSave(draft);
    let before = demo.data.clone();
    demo.execute(Action::Exit(a.clone())).unwrap();
    let saved = demo.data.exits.cases[0].clone();
    let prepare = |data: &hris_desktop::domain::Snapshot| {
        let mut r = baseline(&data.company);
        for response in &mut r {
            let value = match response.prefix.as_str() {
                "POST /rest/v1/rpc/my_hris_access " => Some(
                    json!({"company_id":data.company.id,"user_id":data.access.user_id,"employee_id":data.access.employee_id,"role":data.access.role,"direct_reports":data.access.direct_reports}),
                ),
                p if p.starts_with("GET /rest/v1/employees?") => Some(json!(data.employees)),
                p if p.starts_with("GET /rest/v1/exit_cases?") => Some(json!(data.exits.cases)),
                p if p.starts_with("GET /rest/v1/clearance_tasks?") => {
                    Some(json!(data.exits.tasks))
                }
                p if p.starts_with("GET /rest/v1/exit_events?") => Some(json!(data.exits.events)),
                "POST /rest/v1/rpc/exit_readiness_list " => Some(json!(
                    data.exits
                        .cases
                        .iter()
                        .filter_map(|c| readiness(data, c.id))
                        .collect::<Vec<_>>()
                )),
                _ => None,
            };
            if let Some(v) = value {
                response.body = serde_json::to_vec(&v).unwrap();
            }
        }
        r
    };
    let mut replies = prepare(&before);
    replies.push(reply("POST /rest/v1/rpc/manage_exit ", 200, json!(saved)));
    replies.push(reply(
        "POST /rest/v1/rpc/my_hris_access ",
        500,
        json!({"message":"Synthetic refresh failure"}),
    ));
    replies.push(reply("POST /rest/v1/rpc/manage_exit ", 200, json!(saved)));
    replies.extend(
        prepare(&demo.data)
            .into_iter()
            .skip(1)
            .take_while(|r| !r.prefix.starts_with("GET /auth/v1/user ")),
    );
    let (config, server, requests) = fixture(replies);
    let (mut client, current) = CloudClient::connect(
        config,
        "person@example.invalid".into(),
        "synthetic passphrase".into(),
    )
    .unwrap();
    assert!(
        client
            .execute(Action::Exit(a.clone()), &current)
            .unwrap_err()
            .to_string()
            .contains("refresh failed")
    );
    let next = client.execute(Action::Exit(a.clone()), &current).unwrap();
    assert!(a.saved(&next));
    assert_eq!(next.exits.tasks.len(), 5);
    server.join().unwrap();
    let req = requests.lock().unwrap();
    let writes: Vec<_> = req
        .iter()
        .filter(|r| r.line.starts_with("POST /rest/v1/rpc/manage_exit "))
        .collect();
    assert_eq!(writes.len(), 2);
    assert_eq!(writes[0].json(), writes[1].json());
    assert!(
        !req.iter()
            .any(|r| r.line.starts_with("PATCH /rest/v1/exit_cases"))
    );
}

#[test]
fn performance_goal_retries_original_id_after_saved_write_and_failed_refresh() {
    use hris_desktop::{domain::Action, performance::*};
    let mut demo = DemoStore::new();
    let mut d = GoalDraft::new(&demo.data, demo.data.access.employee_id);
    d.title = "Synthetic transport goal".into();
    d.reason = "Synthetic assigned goal".into();
    let action = PerformanceAction::GoalSave(d);
    let before = demo.data.clone();
    demo.execute(Action::Performance(action.clone())).unwrap();
    let saved = demo.data.performance.goals[0].clone();
    let prepare = |data: &hris_desktop::domain::Snapshot| {
        let mut r = baseline(&data.company);
        for response in &mut r {
            let v = match response.prefix.as_str() {
                "POST /rest/v1/rpc/my_hris_access " => Some(
                    json!({"company_id":data.company.id,"user_id":data.access.user_id,"employee_id":data.access.employee_id,"role":data.access.role,"direct_reports":data.access.direct_reports}),
                ),
                p if p.starts_with("GET /rest/v1/employees?") => Some(json!(data.employees)),
                p if p.starts_with("GET /rest/v1/performance_goals?") => {
                    Some(json!(data.performance.goals))
                }
                p if p.starts_with("GET /rest/v1/performance_events?") => {
                    Some(json!(data.performance.events))
                }
                _ => None,
            };
            if let Some(v) = v {
                response.body = serde_json::to_vec(&v).unwrap();
            }
        }
        r
    };
    let mut replies = prepare(&before);
    replies.push(reply(
        "POST /rest/v1/rpc/manage_performance ",
        200,
        json!(saved),
    ));
    replies.push(reply(
        "POST /rest/v1/rpc/my_hris_access ",
        500,
        json!({"message":"Synthetic refresh failure"}),
    ));
    replies.push(reply(
        "POST /rest/v1/rpc/manage_performance ",
        200,
        json!(saved),
    ));
    replies.extend(
        prepare(&demo.data)
            .into_iter()
            .skip(1)
            .take_while(|r| !r.prefix.starts_with("GET /auth/v1/user ")),
    );
    let (config, server, requests) = fixture(replies);
    let (mut client, data) = CloudClient::connect(
        config,
        "person@example.invalid".into(),
        "synthetic passphrase".into(),
    )
    .unwrap();
    assert!(
        client
            .execute(Action::Performance(action.clone()), &data)
            .unwrap_err()
            .to_string()
            .contains("refresh failed")
    );
    let result = client
        .execute(Action::Performance(action.clone()), &data)
        .unwrap();
    assert!(action.saved(&result));
    server.join().unwrap();
    let req = requests.lock().unwrap();
    let writes: Vec<_> = req
        .iter()
        .filter(|r| r.line.starts_with("POST /rest/v1/rpc/manage_performance "))
        .collect();
    assert_eq!(writes.len(), 2);
    assert_eq!(writes[0].json(), writes[1].json());
    assert!(
        !req.iter()
            .any(|r| r.line.starts_with("PATCH /rest/v1/performance_goals"))
    );
}
#[test]
fn a_performance_save_response_for_another_company_is_rejected_before_refresh() {
    use hris_desktop::{domain::Action, performance::*};
    let demo = DemoStore::new();
    let mut d = CycleDraft::new(&demo.data);
    d.name = "Synthetic response check".into();
    d.reason = "Synthetic cycle".into();
    let id = d.id;
    let mut replies = baseline(&demo.data.company);
    replies.push(reply(
        "POST /rest/v1/rpc/manage_performance ",
        200,
        json!({"id":id,"company_id":uuid::Uuid::new_v4()}),
    ));
    let (config, server, requests) = fixture(replies);
    let (mut client, data) = CloudClient::connect(
        config,
        "person@example.invalid".into(),
        "synthetic passphrase".into(),
    )
    .unwrap();
    assert!(
        client
            .execute(Action::Performance(PerformanceAction::CycleSave(d)), &data)
            .unwrap_err()
            .to_string()
            .contains("company and record")
    );
    server.join().unwrap();
    assert!(
        requests
            .lock()
            .unwrap()
            .last()
            .unwrap()
            .line
            .starts_with("POST /rest/v1/rpc/manage_performance ")
    );
}

#[test]
fn learning_training_retries_original_id_after_saved_write_and_failed_refresh() {
    use hris_desktop::{domain::Action, learning::*};
    let mut demo = DemoStore::new();
    let mut d = TrainingDraft::new(demo.data.access.employee_id);
    d.title = "Synthetic transport goal".into();
    d.reason = "Synthetic assigned goal".into();
    let action = LearningAction::TrainingSave(d);
    let before = demo.data.clone();
    demo.execute(Action::Learning(action.clone())).unwrap();
    let saved = demo.data.learning.training[0].clone();
    let prepare = |data: &hris_desktop::domain::Snapshot| {
        let mut r = baseline(&data.company);
        for response in &mut r {
            let v = match response.prefix.as_str() {
                "POST /rest/v1/rpc/my_hris_access " => Some(
                    json!({"company_id":data.company.id,"user_id":data.access.user_id,"employee_id":data.access.employee_id,"role":data.access.role,"direct_reports":data.access.direct_reports}),
                ),
                p if p.starts_with("GET /rest/v1/employees?") => Some(json!(data.employees)),
                p if p.starts_with("GET /rest/v1/employee_training?") => {
                    Some(json!(data.learning.training))
                }
                p if p.starts_with("GET /rest/v1/learning_events?") => {
                    Some(json!(data.learning.events))
                }
                _ => None,
            };
            if let Some(v) = v {
                response.body = serde_json::to_vec(&v).unwrap();
            }
        }
        r
    };
    let mut replies = prepare(&before);
    replies.push(reply(
        "POST /rest/v1/rpc/manage_learning ",
        200,
        json!(saved),
    ));
    replies.push(reply(
        "POST /rest/v1/rpc/my_hris_access ",
        500,
        json!({"message":"Synthetic refresh failure"}),
    ));
    replies.push(reply(
        "POST /rest/v1/rpc/manage_learning ",
        200,
        json!(saved),
    ));
    replies.extend(
        prepare(&demo.data)
            .into_iter()
            .skip(1)
            .take_while(|r| !r.prefix.starts_with("GET /auth/v1/user ")),
    );
    let (config, server, requests) = fixture(replies);
    let (mut client, data) = CloudClient::connect(
        config,
        "person@example.invalid".into(),
        "synthetic passphrase".into(),
    )
    .unwrap();
    assert!(
        client
            .execute(Action::Learning(action.clone()), &data)
            .unwrap_err()
            .to_string()
            .contains("refresh failed")
    );
    let result = client
        .execute(Action::Learning(action.clone()), &data)
        .unwrap();
    assert!(action.saved(&result));
    server.join().unwrap();
    let req = requests.lock().unwrap();
    let writes: Vec<_> = req
        .iter()
        .filter(|r| r.line.starts_with("POST /rest/v1/rpc/manage_learning "))
        .collect();
    assert_eq!(writes.len(), 2);
    assert_eq!(writes[0].json(), writes[1].json());
    assert!(
        !req.iter()
            .any(|r| r.line.starts_with("PATCH /rest/v1/employee_training"))
    );
}

#[test]
fn career_plan_retries_original_id_after_saved_write_and_failed_refresh() {
    use hris_desktop::{domain::Action, career::*};
    let mut demo = DemoStore::new();
    let mut d = PlanDraft::new(Some(demo.data.employees[1].id),demo.data.company.today());
    d.title = "Synthetic transport goal".into();
    d.reason = "Synthetic assigned goal".into();
    let action = CareerAction::PlanSave(d);
    let before = demo.data.clone();
    demo.execute(Action::Career(action.clone())).unwrap();
    let saved = demo.data.career.plans[0].clone();
    let prepare = |data: &hris_desktop::domain::Snapshot| {
        let mut r = baseline(&data.company);
        for response in &mut r {
            let v = match response.prefix.as_str() {
                "POST /rest/v1/rpc/my_hris_access " => Some(
                    json!({"company_id":data.company.id,"user_id":data.access.user_id,"employee_id":data.access.employee_id,"role":data.access.role,"direct_reports":data.access.direct_reports}),
                ),
                p if p.starts_with("GET /rest/v1/employees?") => Some(json!(data.employees)),
                p if p.starts_with("GET /rest/v1/career_plans?") => {
                    Some(json!(data.career.plans))
                }
                p if p.starts_with("GET /rest/v1/career_events?") => {
                    Some(json!(data.career.events))
                }
                _ => None,
            };
            if let Some(v) = v {
                response.body = serde_json::to_vec(&v).unwrap();
            }
        }
        r
    };
    let mut replies = prepare(&before);
    replies.push(reply(
        "POST /rest/v1/rpc/manage_career ",
        200,
        json!(saved),
    ));
    replies.push(reply(
        "POST /rest/v1/rpc/my_hris_access ",
        500,
        json!({"message":"Synthetic refresh failure"}),
    ));
    replies.push(reply(
        "POST /rest/v1/rpc/manage_career ",
        200,
        json!(saved),
    ));
    replies.extend(
        prepare(&demo.data)
            .into_iter()
            .skip(1)
            .take_while(|r| !r.prefix.starts_with("GET /auth/v1/user ")),
    );
    let (config, server, requests) = fixture(replies);
    let (mut client, data) = CloudClient::connect(
        config,
        "person@example.invalid".into(),
        "synthetic passphrase".into(),
    )
    .unwrap();
    assert!(
        client
            .execute(Action::Career(action.clone()), &data)
            .unwrap_err()
            .to_string()
            .contains("refresh failed")
    );
    let result = client
        .execute(Action::Career(action.clone()), &data)
        .unwrap();
    assert!(action.saved(&result));
    server.join().unwrap();
    let req = requests.lock().unwrap();
    let writes: Vec<_> = req
        .iter()
        .filter(|r| r.line.starts_with("POST /rest/v1/rpc/manage_career "))
        .collect();
    assert_eq!(writes.len(), 2);
    assert_eq!(writes[0].json(), writes[1].json());
    assert!(
        !req.iter()
            .any(|r| r.line.starts_with("PATCH /rest/v1/career_plans"))
    );
}

#[test]
fn payroll_creation_retries_original_identity_after_saved_write_and_failed_refresh() {
    use hris_desktop::{domain::Action,payroll::*};
    let mut demo = DemoStore::new();
    let action=PayrollAction::new("run_create",json!({"id":uuid::Uuid::new_v4(),"month":"2026-09-01","title":"Synthetic payroll transport","reason":"Synthetic creation"}));
    let before = demo.data.clone();
    demo.execute(Action::Payroll(action.clone())).unwrap();
    let saved = demo.data.payroll.runs[0].clone();
    let prepare = |data: &hris_desktop::domain::Snapshot| {
        let mut r = baseline(&data.company);
        for response in &mut r {
            let v = match response.prefix.as_str() {
                "POST /rest/v1/rpc/my_hris_access " => Some(
                    json!({"company_id":data.company.id,"user_id":data.access.user_id,"employee_id":data.access.employee_id,"role":data.access.role,"direct_reports":data.access.direct_reports}),
                ),
                p if p.starts_with("GET /rest/v1/employees?") => Some(json!(data.employees)),
                p if p.starts_with("GET /rest/v1/payroll_runs?") => {
                    Some(json!(data.payroll.runs))
                }
                p if p.starts_with("GET /rest/v1/payroll_events?") => {
                    Some(json!(data.payroll.events))
                }
                _ => None,
            };
            if let Some(v) = v {
                response.body = serde_json::to_vec(&v).unwrap();
            }
        }
        r
    };
    let mut replies = prepare(&before);
    replies.push(reply(
        "POST /rest/v1/rpc/manage_payroll ",
        200,
        json!(saved),
    ));
    replies.push(reply(
        "POST /rest/v1/rpc/my_hris_access ",
        500,
        json!({"message":"Synthetic refresh failure"}),
    ));
    replies.push(reply(
        "POST /rest/v1/rpc/manage_payroll ",
        200,
        json!(saved),
    ));
    replies.extend(
        prepare(&demo.data)
            .into_iter()
            .skip(1)
            .take_while(|r| !r.prefix.starts_with("GET /auth/v1/user ")),
    );
    let (config, server, requests) = fixture(replies);
    let (mut client, data) = CloudClient::connect(
        config,
        "person@example.invalid".into(),
        "synthetic passphrase".into(),
    )
    .unwrap();
    assert!(
        client
            .execute(Action::Payroll(action.clone()), &data)
            .unwrap_err()
            .to_string()
            .contains("refresh failed")
    );
    let result = client
        .execute(Action::Payroll(action.clone()), &data)
        .unwrap();
    assert!(action.saved(&result));
    server.join().unwrap();
    let req = requests.lock().unwrap();
    let writes: Vec<_> = req
        .iter()
        .filter(|r| r.line.starts_with("POST /rest/v1/rpc/manage_payroll "))
        .collect();
    assert_eq!(writes.len(), 2);
    assert_eq!(writes[0].json(), writes[1].json());
    assert!(
        !req.iter()
            .any(|r| r.line.starts_with("PATCH /rest/v1/payroll_runs"))
    );
}

#[test]
fn cutoff_creation_uses_its_guarded_rpc_and_retries_one_identity_after_refresh_failure(){
    use hris_desktop::{domain::Action,payroll::PayrollAction};
    let mut demo=DemoStore::new();let action=PayrollAction::new("cutoff_run_create",json!({"id":uuid::Uuid::new_v4(),"month":"2026-09-01","part":1,"pay_date":"2026-09-15","title":"Synthetic cutoff transport","reason":"Synthetic creation"}));
    let before=demo.data.clone();demo.execute(Action::Payroll(action.clone())).unwrap();let saved=demo.data.payroll.cutoffs.runs[0].clone();
    let prepare=|data:&hris_desktop::domain::Snapshot|{
        let mut r=baseline(&data.company);for response in &mut r{
            let value=match response.prefix.as_str(){
                "POST /rest/v1/rpc/my_hris_access "=>Some(json!({"company_id":data.company.id,"user_id":data.access.user_id,"employee_id":data.access.employee_id,"role":data.access.role,"direct_reports":data.access.direct_reports})),
                p if p.starts_with("GET /rest/v1/employees?")=>Some(json!(data.employees)),
                p if p.starts_with("GET /rest/v1/payroll_cutoff_runs?")=>Some(json!(data.payroll.cutoffs.runs)),
                p if p.starts_with("GET /rest/v1/payroll_events?")=>Some(json!(data.payroll.events)),_=>None
            };if let Some(v)=value{response.body=serde_json::to_vec(&v).unwrap();}
        }r
    };
    let mut replies=prepare(&before);replies.push(reply("POST /rest/v1/rpc/manage_payroll_cutoff ",200,json!(saved)));replies.push(reply("POST /rest/v1/rpc/my_hris_access ",500,json!({"message":"Synthetic refresh failure"})));replies.push(reply("POST /rest/v1/rpc/manage_payroll_cutoff ",200,json!(saved)));replies.extend(prepare(&demo.data).into_iter().skip(1).take_while(|r|!r.prefix.starts_with("GET /auth/v1/user ")));
    let(config,server,requests)=fixture(replies);let(mut client,data)=CloudClient::connect(config,"person@example.invalid".into(),"synthetic passphrase".into()).unwrap();
    assert!(client.execute(Action::Payroll(action.clone()),&data).unwrap_err().to_string().contains("refresh failed"));let result=client.execute(Action::Payroll(action.clone()),&data).unwrap();assert!(action.saved(&result));server.join().unwrap();
    let req=requests.lock().unwrap();let writes:Vec<_>=req.iter().filter(|r|r.line.starts_with("POST /rest/v1/rpc/manage_payroll_cutoff ")).collect();assert_eq!(writes.len(),2);assert_eq!(writes[0].json(),writes[1].json());assert_eq!(writes[0].json()["p_operation"],"run_create");assert!(!req.iter().any(|r|r.line.starts_with("POST /rest/v1/payroll_cutoff_runs")));
}

#[test]
fn thirteenth_creation_uses_its_guarded_rpc_and_retries_one_identity_after_refresh_failure(){
    use hris_desktop::{domain::Action,payroll::PayrollAction};
    let mut demo=DemoStore::new();let action=PayrollAction::new("thirteenth_run_create",json!({"id":uuid::Uuid::new_v4(),"year":2026,"as_of":"2026-09-30","pay_date":"2026-10-01","title":"Synthetic annual benefit transport","reason":"Synthetic creation"}));
    let before=demo.data.clone();demo.execute(Action::Payroll(action.clone())).unwrap();let saved=demo.data.payroll.thirteenth.runs[0].clone();
    let prepare=|data:&hris_desktop::domain::Snapshot|{
        let mut r=baseline(&data.company);for response in &mut r{
            let value=match response.prefix.as_str(){
                "POST /rest/v1/rpc/my_hris_access "=>Some(json!({"company_id":data.company.id,"user_id":data.access.user_id,"employee_id":data.access.employee_id,"role":data.access.role,"direct_reports":data.access.direct_reports})),
                p if p.starts_with("GET /rest/v1/employees?")=>Some(json!(data.employees)),
                p if p.starts_with("GET /rest/v1/payroll_thirteenth_runs?")=>Some(json!(data.payroll.thirteenth.runs)),
                p if p.starts_with("GET /rest/v1/payroll_events?")=>Some(json!(data.payroll.events)),_=>None
            };if let Some(v)=value{response.body=serde_json::to_vec(&v).unwrap();}
        }r
    };
    let mut replies=prepare(&before);replies.push(reply("POST /rest/v1/rpc/manage_payroll_thirteenth ",200,json!(saved)));replies.push(reply("POST /rest/v1/rpc/my_hris_access ",500,json!({"message":"Synthetic refresh failure"})));replies.push(reply("POST /rest/v1/rpc/manage_payroll_thirteenth ",200,json!(saved)));replies.extend(prepare(&demo.data).into_iter().skip(1).take_while(|r|!r.prefix.starts_with("GET /auth/v1/user ")));
    let(config,server,requests)=fixture(replies);let(mut client,data)=CloudClient::connect(config,"person@example.invalid".into(),"synthetic passphrase".into()).unwrap();
    assert!(client.execute(Action::Payroll(action.clone()),&data).unwrap_err().to_string().contains("refresh failed"));let result=client.execute(Action::Payroll(action.clone()),&data).unwrap();assert!(action.saved(&result));server.join().unwrap();
    let req=requests.lock().unwrap();let writes:Vec<_>=req.iter().filter(|r|r.line.starts_with("POST /rest/v1/rpc/manage_payroll_thirteenth ")).collect();assert_eq!(writes.len(),2);assert_eq!(writes[0].json(),writes[1].json());assert_eq!(writes[0].json()["p_operation"],"run_create");assert!(!req.iter().any(|r|r.line.starts_with("POST /rest/v1/payroll_thirteenth_runs")));
}

#[test]
fn annual_tax_creation_uses_its_guarded_rpc_and_retries_one_identity_after_refresh_failure(){
    use hris_desktop::{domain::Action,payroll::PayrollAction};
    let mut demo=DemoStore::new();let action=PayrollAction::new("tax_create",json!({"id":uuid::Uuid::new_v4(),"year":2026,"as_of":"2026-09-30","employee_id":demo.data.employees[1].id,"kind":"termination","service_end":"2026-09-30","title":"Synthetic annual benefit transport","reason":"Synthetic creation"}));
    let before=demo.data.clone();demo.execute(Action::Payroll(action.clone())).unwrap();let saved=demo.data.payroll.tax.records[0].clone();
    let prepare=|data:&hris_desktop::domain::Snapshot|{
        let mut r=baseline(&data.company);for response in &mut r{
            let value=match response.prefix.as_str(){
                "POST /rest/v1/rpc/my_hris_access "=>Some(json!({"company_id":data.company.id,"user_id":data.access.user_id,"employee_id":data.access.employee_id,"role":data.access.role,"direct_reports":data.access.direct_reports})),
                p if p.starts_with("GET /rest/v1/employees?")=>Some(json!(data.employees)),
                p if p.starts_with("GET /rest/v1/payroll_tax_reconciliations?")=>Some(json!(data.payroll.tax.records)),
                p if p.starts_with("GET /rest/v1/payroll_events?")=>Some(json!(data.payroll.events)),_=>None
            };if let Some(v)=value{response.body=serde_json::to_vec(&v).unwrap();}
        }r
    };
    let mut replies=prepare(&before);replies.push(reply("POST /rest/v1/rpc/manage_payroll_tax ",200,json!(saved)));replies.push(reply("POST /rest/v1/rpc/my_hris_access ",500,json!({"message":"Synthetic refresh failure"})));replies.push(reply("POST /rest/v1/rpc/manage_payroll_tax ",200,json!(saved)));replies.extend(prepare(&demo.data).into_iter().skip(1).take_while(|r|!r.prefix.starts_with("GET /auth/v1/user ")));
    let(config,server,requests)=fixture(replies);let(mut client,data)=CloudClient::connect(config,"person@example.invalid".into(),"synthetic passphrase".into()).unwrap();
    assert!(client.execute(Action::Payroll(action.clone()),&data).unwrap_err().to_string().contains("refresh failed"));let result=client.execute(Action::Payroll(action.clone()),&data).unwrap();assert!(action.saved(&result));server.join().unwrap();
    let req=requests.lock().unwrap();let writes:Vec<_>=req.iter().filter(|r|r.line.starts_with("POST /rest/v1/rpc/manage_payroll_tax ")).collect();assert_eq!(writes.len(),2);assert_eq!(writes[0].json(),writes[1].json());assert_eq!(writes[0].json()["p_operation"],"create");assert!(!req.iter().any(|r|r.line.starts_with("POST /rest/v1/payroll_tax_reconciliations")));
}

#[test]
fn final_pay_creation_uses_its_guarded_rpc_and_retries_one_identity_after_refresh_failure(){
 use hris_desktop::{domain::Action,payroll::PayrollAction};
 let mut demo=DemoStore::new();let company=demo.data.company.clone();let id=uuid::Uuid::from_u128(99391);let employee=demo.data.employees[1].id;
 let action=PayrollAction::new("final_create",json!({"id":id,"employee_id":employee,"service_end":"2026-09-30","cash_date":company.today(),"title":"Synthetic transport final pay","reason":"PRIVATE_FINAL_TRANSPORT"}));
 demo.execute(Action::Payroll(action.clone())).unwrap();let saved=demo.data.payroll.finals.records[0].clone();
 let mut replies=baseline(&company);replies.push(reply("POST /rest/v1/rpc/manage_payroll_final ",200,json!(saved)));replies.push(reply("POST /rest/v1/rpc/my_hris_access ",500,json!({"message":"Synthetic saved-write refresh failure"})));replies.push(reply("POST /rest/v1/rpc/manage_payroll_final ",200,json!(saved)));
 let mut after=baseline(&company);for r in &mut after{if r.prefix.starts_with("GET /rest/v1/payroll_final_cases?"){r.body=serde_json::to_vec(&demo.data.payroll.finals.records).unwrap();}if r.prefix.starts_with("GET /rest/v1/payroll_events?"){r.body=serde_json::to_vec(&demo.data.payroll.events).unwrap();}}
 replies.extend(after.into_iter().skip(1).take_while(|r|!r.prefix.starts_with("GET /auth/v1/user ")));
 let(config,server,requests)=fixture(replies);let(mut client,data)=CloudClient::connect(config,"person@example.invalid".into(),"synthetic passphrase".into()).unwrap();assert!(client.execute(Action::Payroll(action.clone()),&data).unwrap_err().to_string().contains("Payroll change saved, but refresh failed"));let result=client.execute(Action::Payroll(action),&data).unwrap();assert_eq!(result.payroll.finals.records[0].id,id);drop(client);server.join().unwrap();let req=requests.lock().unwrap();let writes=req.iter().filter(|r|r.line.starts_with("POST /rest/v1/rpc/manage_payroll_final ")).collect::<Vec<_>>();assert_eq!(writes.len(),2);assert_eq!(writes[0].json(),writes[1].json());assert_eq!(writes[0].json()["p_operation"],"create");assert!(!req.iter().any(|r|r.line.starts_with("POST /rest/v1/payroll_final_cases")));
}
#[test]
fn amendment_creation_retries_one_identity_and_never_writes_financial_tables_directly(){
 use hris_desktop::{domain::Action,payroll::PayrollAction,payroll_amendments as a};
 let company=DemoStore::new().data.company;let id=uuid::Uuid::from_u128(99451);let target=uuid::Uuid::from_u128(99452);let employee=uuid::Uuid::from_u128(99453);
 let action=PayrollAction::new("amendment_create",json!({"id":id,"target_kind":"cutoff","target_id":target,"cash_date":company.today(),"title":"Synthetic transport supplement","purpose":"payout","reason":"PRIVATE_AMENDMENT_TRANSPORT"}));
 let saved=json!({"id":id,"company_id":company.id,"employee_id":employee,"target_kind":"cutoff","target_id":target,"starts_on":"2026-09-01","ends_on":"2026-09-15","cash_date":company.today(),"title":"Synthetic transport supplement","employee_name":"Fictional recipient","employee_code":"QA-TRANSPORT","department":"Operations","status":"draft","revision":0,"created_by":uuid::Uuid::from_u128(1),"prepared_by":uuid::Uuid::from_u128(1),"submitted_by":null,"approved_by":null,"approved_at":null,"paid_on":null,"payment_reference":"","source_basis":{},"worksheet":a::Worksheet{purpose:"payout".into(),..Default::default()},"totals":a::Calculation::default(),"rule_version":a::RULE_VERSION});
 let mut replies=baseline(&company);replies.push(reply("POST /rest/v1/rpc/manage_payroll_amendment ",200,saved.clone()));replies.push(reply("POST /rest/v1/rpc/my_hris_access ",500,json!({"message":"Synthetic post-save refresh failure"})));replies.push(reply("POST /rest/v1/rpc/manage_payroll_amendment ",200,saved.clone()));
 let mut after=baseline(&company);for r in &mut after{if r.prefix.starts_with("GET /rest/v1/payroll_amendment_cases?"){r.body=serde_json::to_vec(&json!([saved])).unwrap();}}
 replies.extend(after.into_iter().skip(1).take_while(|r|!r.prefix.starts_with("GET /auth/v1/user ")));
 let(config,server,requests)=fixture(replies);let(mut client,data)=CloudClient::connect(config,"person@example.invalid".into(),"synthetic passphrase".into()).unwrap();assert!(client.execute(Action::Payroll(action.clone()),&data).unwrap_err().to_string().contains("Payroll change saved, but refresh failed"));let result=client.execute(Action::Payroll(action),&data).unwrap();assert_eq!(result.payroll.amendments.records[0].id,id);drop(client);server.join().unwrap();let req=requests.lock().unwrap();let writes=req.iter().filter(|r|r.line.starts_with("POST /rest/v1/rpc/manage_payroll_amendment ")).collect::<Vec<_>>();assert_eq!(writes.len(),2);assert_eq!(writes[0].json(),writes[1].json());assert_eq!(writes[0].json()["p_operation"],"create");assert!(!req.iter().any(|r|r.line.starts_with("POST /rest/v1/payroll_amendment_cases")));
}

#[test]
fn wage_review_creation_uses_guarded_rpc_and_retry_identity(){
 use hris_desktop::{domain::Action,payroll::PayrollAction};
 let mut demo=DemoStore::new();let company=demo.data.company.clone();let id=uuid::Uuid::from_u128(39391);let employee=demo.data.employees[1].id;
 let action=PayrollAction::new("wage_create",json!({"id":id,"employee_id":employee,"starts_on":"2026-01-01","ends_on":"2026-12-31","coverage":"covered","series_key":"vi-nonagri-large","workplace":"Synthetic Bacolod workplace","evidence":"PRIVATE_WAGE_TRANSPORT","note":"Reviewed industry and establishment size","reason":"Synthetic transport wage review","catalog_version":hris_desktop::payroll_wages::VERSION}));
 demo.execute(Action::Payroll(action.clone())).unwrap();let saved=demo.data.payroll.wages.classifications[0].clone();
 let mut replies=baseline(&company);for r in &mut replies{if r.prefix.starts_with("GET /rest/v1/employees?"){r.body=serde_json::to_vec(&demo.data.employees).unwrap();}}replies.push(reply("POST /rest/v1/rpc/manage_payroll_wage ",200,json!(saved)));replies.push(reply("POST /rest/v1/rpc/my_hris_access ",500,json!({"message":"Synthetic saved-write refresh failure"})));replies.push(reply("POST /rest/v1/rpc/manage_payroll_wage ",200,json!(saved)));
 let mut after=baseline(&company);for r in &mut after{if r.prefix.starts_with("GET /rest/v1/employees?"){r.body=serde_json::to_vec(&demo.data.employees).unwrap();}if r.prefix.starts_with("GET /rest/v1/payroll_wage_classifications?"){r.body=serde_json::to_vec(&demo.data.payroll.wages.classifications).unwrap();}if r.prefix.starts_with("GET /rest/v1/payroll_events?"){r.body=serde_json::to_vec(&demo.data.payroll.events).unwrap();}}
 replies.extend(after.into_iter().skip(1).take_while(|r|!r.prefix.starts_with("GET /auth/v1/user ")));
 let(config,server,requests)=fixture(replies);let(mut client,data)=CloudClient::connect(config,"person@example.invalid".into(),"synthetic passphrase".into()).unwrap();assert!(client.execute(Action::Payroll(action.clone()),&data).unwrap_err().to_string().contains("Payroll change saved, but refresh failed"));let result=client.execute(Action::Payroll(action),&data).unwrap();assert_eq!(result.payroll.wages.classifications[0].id,id);drop(client);server.join().unwrap();let req=requests.lock().unwrap();let writes=req.iter().filter(|r|r.line.starts_with("POST /rest/v1/rpc/manage_payroll_wage ")).collect::<Vec<_>>();assert_eq!(writes.len(),2);assert_eq!(writes[0].json(),writes[1].json());assert_eq!(writes[0].json()["p_operation"],"create");assert!(!req.iter().any(|r|r.line.starts_with("POST /rest/v1/payroll_wage_classifications")));
}

#[test]
fn consolidated_payroll_report_uses_one_scoped_rpc_and_rechecks_before_export(){
 use hris_desktop::payroll_reporting::{Filter,VERSION};use chrono::Utc;
 let d=DemoStore::new();let company=d.data.company.clone();
 let f:Filter=serde_json::from_value(json!({"starts_on":"2026-01-01","ends_on":"2026-12-31","axis":"issued","kind":"all","employee_id":null,"search":""})).unwrap();
 let access=json!({"company_id":company.id,"user_id":company.owner_id,"role":"administrator","employee_id":null,"direct_reports":[]});
 let wire=json!({"version":VERSION,"company_id":company.id,"company_name":company.name,"generated_at":Utc::now(),"filter":f,"rows":[]});
 let mut responses=baseline(&company);responses.push(reply("POST /rest/v1/rpc/my_hris_access ",200,access.clone()));responses.push(reply("POST /rest/v1/rpc/payroll_report_v2 ",200,wire));responses.push(reply("POST /rest/v1/rpc/my_hris_access ",200,access));
 let(config,server,requests)=fixture(responses);let(mut client,data)=CloudClient::connect(config,"person@example.invalid".into(),"synthetic passphrase".into()).unwrap();
 let report=client.consolidated_payroll_report(&data,&f,None).unwrap();assert!(report.wire.rows.is_empty());drop(client);server.join().unwrap();
 let req=requests.lock().unwrap();let reports:Vec<_>=req.iter().filter(|r|r.line.starts_with("POST /rest/v1/rpc/payroll_report_v2 ")).collect();assert_eq!(reports.len(),1);assert_eq!(reports[0].json(),json!({"p_filter":f}));
}
#[test]
fn consolidated_report_revocation_after_fetch_cannot_write_csv(){
 use hris_desktop::payroll_reporting::{Filter,VERSION};use chrono::Utc;
 let d=DemoStore::new();let company=d.data.company.clone();let f:Filter=serde_json::from_value(json!({"starts_on":"2026-01-01","ends_on":"2026-12-31","axis":"issued","kind":"all","employee_id":null,"search":""})).unwrap();
 let access=json!({"company_id":company.id,"user_id":company.owner_id,"role":"administrator","employee_id":null,"direct_reports":[]});let mut denied=access.clone();denied["role"]=json!("manager");
 let wire=json!({"version":VERSION,"company_id":company.id,"company_name":company.name,"generated_at":Utc::now(),"filter":f,"rows":[]});
 let mut responses=baseline(&company);responses.push(reply("POST /rest/v1/rpc/my_hris_access ",200,access));responses.push(reply("POST /rest/v1/rpc/payroll_report_v2 ",200,wire));responses.push(reply("POST /rest/v1/rpc/my_hris_access ",200,denied));
 let path=std::env::temp_dir().join(format!("hris-report-revoked-{}.csv",uuid::Uuid::new_v4()));assert!(!path.exists());
 let(config,server,_)=fixture(responses);let(mut client,data)=CloudClient::connect(config,"person@example.invalid".into(),"synthetic passphrase".into()).unwrap();
 assert!(client.consolidated_payroll_report(&data,&f,Some(&path)).is_err());assert!(!path.exists());drop(client);server.join().unwrap();
}

#[test]
fn government_registration_reads_one_scoped_rpc_and_discards_a_revoked_response(){
 for revoked in [false,true]{
  let company=Company{owner_id:uuid::Uuid::from_u128(170001),..Default::default()};
  let access=json!({"company_id":company.id,"user_id":company.owner_id,"role":"administrator","employee_id":null,"direct_reports":[]});let mut after=access.clone();if revoked{after["role"]=json!("manager");}
  let wire=json!({"version":"government-registration-v1","company_id":company.id,"employee_id":null,"record":null,"history":[]});
  let mut replies=baseline(&company);replies.push(reply("POST /rest/v1/rpc/my_hris_access ",200,access));replies.push(reply("POST /rest/v1/rpc/government_registration ",200,wire));replies.push(reply("POST /rest/v1/rpc/my_hris_access ",200,after));
  let(config,server,requests)=fixture(replies);let(mut client,data)=CloudClient::connect(config,"person@example.invalid".into(),"synthetic passphrase".into()).unwrap();
  let result=client.government_registration(&data,None,None);assert_eq!(result.is_err(),revoked);
  assert!(client.government_registration(&data,Some(uuid::Uuid::new_v4()),None).is_err());
  drop(client);server.join().unwrap();let requests=requests.lock().unwrap();
  let reads:Vec<_>=requests.iter().filter(|r|r.line.starts_with("POST /rest/v1/rpc/government_registration ")).collect();assert_eq!(reads.len(),1);assert_eq!(reads[0].json(),json!({"p_employee_id":null}));
  assert!(!requests.iter().any(|r|r.line.contains("/government_registration_records?")));
 }
}

#[test]
fn registration_save_retry_preserves_operation_and_unknowns_after_a_failed_refresh(){
 use hris_desktop::payroll_government::registration as reg;
 let company=Company{owner_id:uuid::Uuid::from_u128(170001),..Default::default()};let now=chrono::Utc::now();
 let id=uuid::Uuid::from_u128(170002);let op=uuid::Uuid::from_u128(170003);
 let details=reg::normalize(&[("registered_name".into(),"Fictional Legal Registration".into())].into_iter().collect(),false,company.today()).unwrap();
 let record=reg::Record{id,company_id:company.id,employee_id:None,revision:0,details:details.clone(),reference:"Synthetic registration".into(),note:"Private evidence".into(),created_by:company.owner_id,updated_by:company.owner_id,created_at:now,updated_at:now};
 let saved=reg::Snapshot{version:"government-registration-v1".into(),company_id:company.id,employee_id:None,record:Some(record.clone()),history:vec![reg::Event{id:op,company_id:company.id,record_id:id,revision:0,actor_id:company.owner_id,actor_name:"Synthetic HR".into(),reason:"Initial registration review".into(),before_record:None,after_record:record,recorded_at:now}]};
 let action=reg::Save{id,operation_id:op,employee_id:None,expected_revision:None,details,reference:"Synthetic registration".into(),note:"Private evidence".into(),reason:"Initial registration review".into(),reviewed:true};
 let access=json!({"company_id":company.id,"user_id":company.owner_id,"role":"administrator","employee_id":null,"direct_reports":[]});let receipt=json!({"operation_id":op,"record_id":id,"saved_revision":0});
 let mut replies=baseline(&company);
 for failure in [true,false]{
  replies.push(reply("POST /rest/v1/rpc/my_hris_access ",200,access.clone()));replies.push(reply("POST /rest/v1/rpc/save_government_registration ",200,receipt.clone()));replies.push(reply("POST /rest/v1/rpc/my_hris_access ",200,access.clone()));
  replies.push(if failure{reply("POST /rest/v1/rpc/government_registration ",500,json!({"message":"Synthetic refresh failure"}))}else{reply("POST /rest/v1/rpc/government_registration ",200,json!(saved))});
  if !failure{replies.push(reply("POST /rest/v1/rpc/my_hris_access ",200,access.clone()));}
 }
 let(config,server,requests)=fixture(replies);let(mut client,data)=CloudClient::connect(config,"person@example.invalid".into(),"synthetic passphrase".into()).unwrap();
 assert!(client.government_registration(&data,None,Some(&action)).is_err());let result=client.government_registration(&data,None,Some(&action)).unwrap();assert_eq!(result.record.unwrap().revision,0);
 drop(client);server.join().unwrap();let requests=requests.lock().unwrap();let writes:Vec<_>=requests.iter().filter(|r|r.line.starts_with("POST /rest/v1/rpc/save_government_registration ")).collect();assert_eq!(writes.len(),2);assert_eq!(writes[0].json(),writes[1].json());assert_eq!(writes[0].json()["p_record"]["details"]["bir_tin"],json!(""));assert_eq!(writes[0].json()["p_record"]["operation_id"],json!(op));
}

#[test]
fn agency_preparation_uses_one_fresh_rpc_and_revocation_prevents_file_creation(){
 use chrono::Datelike;
 use hris_desktop::payroll_government::{Agency,Filter};
 for revoked in[false,true]{
  let company=Company{owner_id:uuid::Uuid::from_u128(171001),..Default::default()};let month=company.today().with_day(1).unwrap();
  let access=json!({"company_id":company.id,"user_id":company.owner_id,"role":"administrator","employee_id":null,"direct_reports":[]});let mut after=access.clone();if revoked{after["role"]=json!("manager");}
  let source=json!({"version":"government-payroll-sources-v1","company_id":company.id,"company_name":company.name,"generated_at":chrono::Utc::now(),"as_of":company.today(),"month":month,"agency":"sss","facts":[]});
  let review=json!({"version":"government-reconciliation-v1","source":source,"effective_facts":[],"reviews":[],"remittances":[],"history":[],"employees":[]});
  let wire=json!({"version":"government-preparation-v1","review":review,"registrations":[]});
  let mut replies=baseline(&company);replies.push(reply("POST /rest/v1/rpc/my_hris_access ",200,access));replies.push(reply("POST /rest/v1/rpc/government_preparation ",200,wire));replies.push(reply("POST /rest/v1/rpc/my_hris_access ",200,after));
  let(config,server,requests)=fixture(replies);let(mut client,data)=CloudClient::connect(config,"person@example.invalid".into(),"synthetic passphrase".into()).unwrap();
  let path=std::env::current_dir().unwrap().join("work").join(format!("preparation-{}.csv",uuid::Uuid::new_v4()));
  let result=client.government_preparation(&data,Filter{month,agency:Agency::Sss},&path);assert_eq!(result.is_err(),revoked);
  if revoked{assert!(!path.exists());}else{let b=std::fs::read(&path).unwrap();assert!(String::from_utf8(b).unwrap().contains("INTERNAL_AGENCY_PREPARATION_NOT_AN_UPLOAD_FILE"));std::fs::remove_file(&path).unwrap();}
  drop(client);server.join().unwrap();let req=requests.lock().unwrap();assert_eq!(req.iter().filter(|r|r.line.starts_with("POST /rest/v1/rpc/government_preparation ")).count(),1);
  assert!(!req.iter().any(|r|r.line.contains("/government_registration_records?")));
 }
}

fn collection_source(company:&Company)->serde_json::Value{
 use chrono::Datelike;
 let month=company.today().with_day(1).unwrap().checked_sub_months(chrono::Months::new(1)).unwrap();let end=month.checked_add_months(chrono::Months::new(1)).unwrap().pred_opt().unwrap();
 let eid=uuid::Uuid::from_u128(181001);let receipt=uuid::Uuid::from_u128(181002);let actor=uuid::Uuid::from_u128(181003);
 let fact=json!({"company_id":company.id,"employee_id":eid,"id":uuid::Uuid::from_u128(181004),"agency":"sss","kind":"monthly","state":"received","starts_on":month,"ends_on":end,"contribution_month":month,"received_on":company.today(),"parts":{"employee_cents":150000,"employer_cents":300000,"ec_cents":3000}});
 json!({"version":"government-preparation-v1","review":{"version":"government-reconciliation-v1","source":{"version":"government-payroll-sources-v1","company_id":company.id,"company_name":company.name,"generated_at":chrono::Utc::now(),"as_of":company.today(),"month":month,"agency":"sss","facts":[fact.clone()]},"effective_facts":[fact],"reviews":[],"history":[],"employees":[{"employee_id":eid,"employee_code":"TEST-SSS","employee_name":"Fictional Staff"}],"remittances":[{"id":receipt,"company_id":company.id,"agency":"sss","paid_on":company.today(),"amount_cents":453000,"allocations":[{"month":month,"amount_cents":453000}],"reference":"QA-SBR-01","evidence":"Fictional receipt","note":"","replaces_id":null,"recorded_by":actor,"recorded_at":chrono::Utc::now(),"void_id":null,"void_reason":null,"voided_by":null,"voided_at":null,"replacement_id":null}]},"registrations":[{"id":uuid::Uuid::from_u128(181005),"company_id":company.id,"employee_id":null,"revision":0,"details":{"registered_name":"Fictional Company","sss_number":"0123456789"}},{"id":uuid::Uuid::from_u128(181006),"company_id":company.id,"employee_id":eid,"revision":0,"details":{"last_name":"Dela Cruz","first_name":"Alex","middle_name":"M","name_suffix":"","sss_number":"0212345678"}}]})
}
fn lifecycle_wire(company:&Company,employee:uuid::Uuid,read_keys:Vec<String>)->Value{
 let id=uuid::Uuid::from_u128(184601);let key=format!("payroll-monthly-issued:{id}:0");
 json!({"version":"lifecycle-notifications-v1","company_id":company.id,"user_id":company.owner_id,"employee_id":employee,"role":"administrator","surface":"desktop","as_of":company.today(),"generated_at":chrono::Utc::now(),"read_keys":read_keys,"total_items":1,"truncated":false,
 "items":[{"key":key,"kind":"payroll-monthly-issued","title":"Your monthly payslip is available","detail":"Approval is separate from payment","date":company.today(),"urgent":false,"target_id":id,"target_page":"payroll_monthly","employee_id":employee}]})
}
#[test]
fn lifecycle_projection_rejects_a_changed_protected_role_before_loading_account_data(){
 let company=Company{owner_id:uuid::Uuid::from_u128(184600),..Default::default()};let mut replies=baseline(&company);
 let at=replies.iter().position(|r|r.prefix=="POST /rest/v1/rpc/lifecycle_notifications ").unwrap();
 let mut wire=lifecycle_wire(&company,uuid::Uuid::from_u128(184602),vec![]);wire["employee_id"]=Value::Null;wire["role"]=json!("manager");replies[at]=reply("POST /rest/v1/rpc/lifecycle_notifications ",200,wire);replies.truncate(at+1);
 let(config,server,requests)=fixture(replies);assert!(CloudClient::connect(config,"person@example.invalid".into(),"synthetic passphrase".into()).is_err());server.join().unwrap();
 assert!(!requests.lock().unwrap().iter().any(|r|r.line.starts_with("GET /auth/v1/user ")));
}
#[test]
fn mixed_lifecycle_reads_use_atomic_new_rpc_and_keep_legacy_state_separate(){
 use hris_desktop::{domain::Action,notifications::ReadAction};
 let data=DemoStore::new().data;let company=data.company.clone();let person=data.employees[0].clone();let eid=person.id;
 let new_key=format!("payroll-monthly-issued:{}:0",uuid::Uuid::from_u128(184601));let old_key=format!("checkout:{}:0",uuid::Uuid::from_u128(184603));
 let prepare=|reads:Vec<String>|{
  let mut v=baseline(&company);let mut access:Value=serde_json::from_slice(&v[1].body).unwrap();access["employee_id"]=json!(eid);v[1].body=serde_json::to_vec(&access).unwrap();v[3]=reply("GET /rest/v1/employees?",200,json!([person]));
  response_at(&mut v,"POST /rest/v1/rpc/lifecycle_notifications ").body=serde_json::to_vec(&lifecycle_wire(&company,eid,reads)).unwrap();v
 };
 let mut replies=prepare(vec![]);let saved=json!({"company_id":company.id,"user_id":company.owner_id,"read_keys":[old_key]});replies.push(reply("POST /rest/v1/rpc/staff_notification_reads ",200,saved.clone()));
 let mut after=prepare(vec![new_key.clone()]);response_at(&mut after,"GET /rest/v1/notification_read_state?").body=serde_json::to_vec(&json!([saved])).unwrap();replies.extend(after.into_iter().skip(1).take_while(|r|!r.prefix.starts_with("GET /auth/v1/user ")));
 let(config,server,requests)=fixture(replies);let(mut client,current)=CloudClient::connect(config,"person@example.invalid".into(),"synthetic passphrase".into()).unwrap();
 let next=client.execute(Action::NotificationReads(ReadAction::Mark{keys:vec![old_key.clone(),new_key.clone()],read:true}),&current).unwrap();server.join().unwrap();
 assert_eq!(next.notification_reads.read_keys,vec![old_key.clone()]);assert_eq!(next.lifecycle_notifications.unwrap().read_keys,vec![new_key.clone()]);
 let req=requests.lock().unwrap();let sent=request_at(&req,"POST /rest/v1/rpc/staff_notification_reads ",0).json();assert_eq!(sent,json!({"p_company_id":company.id,"p_keys":[old_key,new_key],"p_read":true,"p_reset":false}));
}
#[test]
fn sss_collection_load_is_private_and_membership_is_rechecked_before_return(){
 use hris_desktop::payroll_government::{Agency,Filter};use chrono::Datelike;
 for revoked in[false,true]{
  let company=Company{owner_id:uuid::Uuid::from_u128(181100),..Default::default()};let month=company.today().with_day(1).unwrap().checked_sub_months(chrono::Months::new(1)).unwrap();
  let access=json!({"company_id":company.id,"user_id":company.owner_id,"role":"administrator","employee_id":null,"direct_reports":[]});let mut after=access.clone();if revoked{after["role"]=json!("manager");}
  let mut replies=baseline(&company);replies.push(reply("POST /rest/v1/rpc/my_hris_access ",200,access));replies.push(reply("POST /rest/v1/rpc/government_preparation ",200,collection_source(&company)));replies.push(reply("POST /rest/v1/rpc/my_hris_access ",200,after));
  let(config,server,requests)=fixture(replies);let(mut client,data)=CloudClient::connect(config,"person@example.invalid".into(),"synthetic passphrase".into()).unwrap();
  assert_eq!(client.sss_collection_prepare(&data,Filter{month,agency:Agency::Sss}).is_err(),revoked);drop(client);server.join().unwrap();
  let req=requests.lock().unwrap();assert_eq!(req.iter().filter(|r|r.line.starts_with("POST /rest/v1/rpc/government_preparation ")).count(),1);assert!(!req.iter().any(|r|r.line.contains("/government_registration_records?")));
 }
}
#[test]
fn sss_collection_export_refetches_and_rejects_stale_or_revoked_inputs_before_io(){
 use hris_desktop::payroll_government::{Agency,Filter,preparation::{Preparation,PreparationWire},sss_collection::{Basis,Request,Declaration},sss_r3::Remark};use chrono::Datelike;
 for case in["success","stale","revoked"]{
  let company=Company{owner_id:uuid::Uuid::from_u128(181100),..Default::default()};let month=company.today().with_day(1).unwrap().checked_sub_months(chrono::Months::new(1)).unwrap();let filter=Filter{month,agency:Agency::Sss};
  let mut wire=collection_source(&company);let original:PreparationWire=serde_json::from_value(wire.clone()).unwrap();let basis=Basis::from_preparation(&Preparation::from_wire(original,company.id,filter).unwrap()).unwrap();
  let request=Request{expected_fingerprint:basis.fingerprint.clone(),receipt_id:basis.receipt.id,employer_name:basis.employer_name.clone(),reviewed:true,declarations:vec![Declaration{employee_id:basis.people[0].employee_id,last_name:"Dela Cruz".into(),first_name:"Alex".into(),middle_initial:"M".into(),remark:Remark::Normal,event_on:None}]};
  if case=="stale"{wire["registrations"][1]["revision"]=json!(1);}
  let access=json!({"company_id":company.id,"user_id":company.owner_id,"role":"administrator","employee_id":null,"direct_reports":[]});let mut after=access.clone();if case=="revoked"{after["role"]=json!("manager");}
  let mut replies=baseline(&company);replies.push(reply("POST /rest/v1/rpc/my_hris_access ",200,access));replies.push(reply("POST /rest/v1/rpc/government_preparation ",200,wire));if case!="stale"{replies.push(reply("POST /rest/v1/rpc/my_hris_access ",200,after));}
  let(config,server,_)=fixture(replies);let(mut client,data)=CloudClient::connect(config,"person@example.invalid".into(),"synthetic passphrase".into()).unwrap();let path=std::env::current_dir().unwrap().join("work").join(format!("sss-reference-{}.txt",uuid::Uuid::new_v4()));
  let result=client.sss_collection_export(&data,filter,&request,&path);assert_eq!(result.is_ok(),case=="success");
  if case=="success"{assert_eq!(std::fs::read(&path).unwrap(),basis.render(&request).unwrap());std::fs::remove_file(path).unwrap();}else{assert!(!path.exists());}
  drop(client);server.join().unwrap();
 }
}

#[test]
fn leave_credit_post_keeps_retry_identity_after_saved_write_and_failed_refresh(){
 use hris_desktop::{domain::Action,leave::credits::{Record,Entry}};
 let data=DemoStore::new().data;let company=data.company.clone();let person=data.employees[0].clone();
 let r=Record{id:uuid::Uuid::from_u128(184700),employee_id:person.id,type_code:"annual".into(),effective_on:company.today(),units:1,reverses_id:None,expected_revision:0,reason:"Synthetic reviewed credit".into(),reference:String::new()};
 let prepare=||{let mut v=baseline(&company);response_at(&mut v,"GET /rest/v1/employees?").body=serde_json::to_vec(&json!([person])).unwrap();v};
 let saved=json!({"id":r.id,"company_revision":1});let mut replies=prepare();replies.push(reply("POST /rest/v1/rpc/post_leave_credit ",200,saved.clone()));replies.push(reply("POST /rest/v1/rpc/my_hris_access ",500,json!({"message":"Synthetic refresh failure"})));replies.push(reply("POST /rest/v1/rpc/post_leave_credit ",200,saved));
 let mut after=prepare();response_at(&mut after,"POST /rest/v1/rpc/leave_credit_context ").body=serde_json::to_vec(&json!({"version":"leave-credits-v1","company_id":company.id,"user_id":company.owner_id,"employee_id":null,"role":"administrator","as_of":company.today(),"revision":1,"entries":[Entry{id:r.id,employee_id:r.employee_id,type_code:r.type_code.clone(),effective_on:r.effective_on,units:1,reverses_id:None}],"audit":[{"id":r.id,"reason":r.reason,"reference":"","created_by_name":"Fictional Administrator","posted_at":chrono::Utc::now(),"company_revision":1}]})).unwrap();
 replies.extend(after.into_iter().skip(1).take_while(|v|!v.prefix.starts_with("GET /auth/v1/user ")));
 let(config,server,requests)=fixture(replies);let(mut client,current)=CloudClient::connect(config,"person@example.invalid".into(),"synthetic passphrase".into()).unwrap();
 assert!(client.execute(Action::LeaveCredit(r.clone()),&current).unwrap_err().to_string().contains("Credit saved, but refresh failed"));
 let fresh=client.execute(Action::LeaveCredit(r.clone()),&current).unwrap();assert_eq!(fresh.leave_setup.credits.unwrap().entries.len(),1);server.join().unwrap();
 let requests=requests.lock().unwrap();assert_eq!(request_at(&requests,"POST /rest/v1/rpc/post_leave_credit ",0).json(),request_at(&requests,"POST /rest/v1/rpc/post_leave_credit ",1).json());
 let p=request_at(&requests,"POST /rest/v1/rpc/post_leave_credit ",0).json();assert_eq!(p,json!({"p_record":r}));assert!(request_at(&requests,"POST /rest/v1/rpc/post_leave_credit ",0).headers.to_lowercase().contains("x-hris-leave-credits: 1"));
}

#[test]
fn delegation_context_rejects_a_changed_protected_role_before_any_write(){
 let company=Company::default();let mut replies=baseline(&company);let at=replies.iter().position(|r|r.prefix=="POST /rest/v1/rpc/approval_delegation_context ").unwrap();
 replies[at]=reply("POST /rest/v1/rpc/approval_delegation_context ",200,json!({"version":"approval-delegation-v1","company_id":company.id,"user_id":company.owner_id,"employee_id":null,"role":"manager","as_of":company.today(),"revision":0,"grants":[],"revocations":[],"receipts":[],"permits":[],"audit":[],"staff":[]}));replies.truncate(at+1);
 let(config,server,requests)=fixture(replies);assert!(CloudClient::connect(config,"person@example.invalid".into(),"synthetic passphrase".into()).is_err());server.join().unwrap();assert!(!requests.lock().unwrap().iter().any(|r|r.line.contains("save_approval_delegation")));
}

#[test]
fn delegation_post_keeps_identical_request_after_saved_write_and_failed_refresh(){
 use hris_desktop::{approvals::{self,Change,Scope,Context},access::{Access,Role},domain::Action};
 let mut data=DemoStore::new().data;let company=data.company.clone();data.access=Access{company_id:company.id,user_id:company.owner_id,employee_id:None,role:Role::Administrator,..Default::default()};let context=Context::empty(&data);
 let principal=context.staff.iter().find(|s|s.role==Role::Manager).unwrap();let delegate=context.staff.iter().find(|s|s.role==Role::HrGeneralist).unwrap();
 let change=Change::Grant{id:uuid::Uuid::from_u128(184800),expected_revision:0,scope:Scope::Leave,principal_id:principal.user_id,delegate_id:delegate.user_id,starts_on:company.today(),ends_on:company.today()+chrono::Duration::days(7),reason:"Synthetic documented delegation".into(),reference:"Synthetic reference".into()};
 data.approvals=Some(context.clone());approvals::apply_demo(&mut data,&change).unwrap();let after_context=data.approvals.as_ref().unwrap().clone();
 let prepare=|c:&Context|{let mut replies=baseline(&company);response_at(&mut replies,"POST /rest/v1/rpc/approval_delegation_context ").body=serde_json::to_vec(c).unwrap();replies};
 let saved=json!({"id":change.id(),"company_revision":1});let mut replies=prepare(&context);replies.push(reply("POST /rest/v1/rpc/save_approval_delegation ",200,saved.clone()));replies.push(reply("POST /rest/v1/rpc/my_hris_access ",500,json!({"message":"Synthetic refresh failure"})));replies.push(reply("POST /rest/v1/rpc/save_approval_delegation ",200,saved));
 replies.extend(prepare(&after_context).into_iter().skip(1).take_while(|v|!v.prefix.starts_with("GET /auth/v1/user ")));
 let(config,server,requests)=fixture(replies);let(mut client,current)=CloudClient::connect(config,"person@example.invalid".into(),"synthetic passphrase".into()).unwrap();
 assert!(client.execute(Action::ApprovalDelegation(change.clone()),&current).err().unwrap().to_string().contains("Delegation saved, but refresh failed"));let fresh=client.execute(Action::ApprovalDelegation(change.clone()),&current).unwrap();assert!(change.matches_history(fresh.approvals.as_ref().unwrap()));server.join().unwrap();
 let requests=requests.lock().unwrap();assert_eq!(request_at(&requests,"POST /rest/v1/rpc/save_approval_delegation ",0).json(),request_at(&requests,"POST /rest/v1/rpc/save_approval_delegation ",1).json());
}

#[test]
fn opening_post_retains_identical_retry_and_checked_leave_binds_the_entire_preview(){
 use hris_desktop::{access::{Access,Role},domain::{Action,LeaveDraft},leave::{self,dated::{Context,OpeningRecord,Change}}};
 let mut d=DemoStore::new().data;d.leave.clear();d.employees.truncate(1);let company=d.company.clone();let person=d.employees[0].clone();
 d.access=Access{company_id:company.id,user_id:company.owner_id,employee_id:None,role:Role::Administrator,..Default::default()};d.leave_setup.dated=Some(Context::empty(&d));d.leave_setup.credits=Some(leave::credits::Context::empty(&d));
 let record=OpeningRecord{id:uuid::Uuid::from_u128(184901),employee_id:person.id,type_code:"annual".into(),effective_on:company.today(),target_available_units:12,remaining_carry_units:0,source_requests_reviewed:true,expected_revision:0,reason:"Fictional reviewed opening".into(),reference:"Fictional source only".into()};
 let preview=leave::dated::preview_demo(&d,&record).unwrap();let change=Change::Opening(Box::new(preview));let initial=d.leave_setup.dated.clone().unwrap();leave::dated::apply_demo(&mut d,&change).unwrap();let after=d.leave_setup.dated.clone().unwrap();
 let prepare=|c:&Context|{let mut v=baseline(&company);response_at(&mut v,"GET /rest/v1/employees?").body=serde_json::to_vec(&json!([person])).unwrap();response_at(&mut v,"POST /rest/v1/rpc/leave_dated_context ").body=serde_json::to_vec(c).unwrap();v};
 let saved=json!({"id":record.id,"company_revision":1});let mut replies=prepare(&initial);replies.push(reply("POST /rest/v1/rpc/post_leave_opening ",200,saved.clone()));replies.push(reply("POST /rest/v1/rpc/my_hris_access ",500,json!({"message":"Fictional refresh failure"})));replies.push(reply("POST /rest/v1/rpc/post_leave_opening ",200,saved));
 replies.extend(prepare(&after).into_iter().skip(1).take_while(|v|!v.prefix.starts_with("GET /auth/v1/user ")));
 replies.push(reply("POST /rest/v1/rpc/submit_leave_checked ",200,json!(uuid::Uuid::from_u128(184902))));replies.push(reply("POST /rest/v1/rpc/my_hris_access ",500,json!({"message":"Fictional refresh failure"})));
 let(config,server,requests)=fixture(replies);let(mut client,current)=CloudClient::connect(config,"person@example.invalid".into(),"synthetic passphrase".into()).unwrap();
 assert!(client.execute(Action::DatedLeave(change.clone()),&current).unwrap_err().to_string().contains("Leave setup saved, but refresh failed"));let fresh=client.execute(Action::DatedLeave(change),&current).unwrap();assert_eq!(fresh.leave_setup.dated.as_ref().unwrap().openings.len(),1);
 let mut day=company.today()+chrono::Duration::days(1);while chrono::Datelike::weekday(&day).number_from_monday()>5{day+=chrono::Duration::days(1);}
 let draft=LeaveDraft{employee_id:Some(person.id),starts_on:day.to_string(),ends_on:day.to_string(),options:leave::LeaveOptions{type_code:"annual".into(),day_part:leave::DayPart::Full},..Default::default()};assert!(client.execute(Action::RequestLeave(draft),&fresh).is_err());server.join().unwrap();
 let seen=requests.lock().unwrap();assert_eq!(request_at(&seen,"POST /rest/v1/rpc/post_leave_opening ",0).json(),request_at(&seen,"POST /rest/v1/rpc/post_leave_opening ",1).json());
 let checked=request_at(&seen,"POST /rest/v1/rpc/submit_leave_checked ",0);let body=checked.json();assert_eq!(body["p_expected"]["setup_revision"],1);assert_eq!(body["p_expected"]["credit_revision"],0);assert_eq!(body["p_expected"]["balance"]["available_units"],12);assert_eq!(body["p_expected"]["charged_dates"],json!([{"date":day,"units":2}]));assert!(checked.headers.to_lowercase().contains("x-hris-leave-dated: 1"));
}

#[test]
fn dated_context_role_mismatch_is_rejected_before_any_mutation(){
 let company=Company::default();let mut replies=baseline(&company);let at=replies.iter().position(|r|r.prefix=="POST /rest/v1/rpc/leave_dated_context ").unwrap();
 replies[at]=reply("POST /rest/v1/rpc/leave_dated_context ",200,json!({"version":"leave-dated-v1","company_id":company.id,"user_id":company.owner_id,"employee_id":null,"role":"manager","as_of":company.today(),"revision":0,"enabled":false,"timeline":{"baseline":hris_desktop::leave::LeavePolicy{company_id:company.id,..Default::default()},"versions":[]},"anchors":[],"openings":[],"withdrawals":[],"audit":[]}));replies.truncate(at+1);
 let(config,server,requests)=fixture(replies);assert!(CloudClient::connect(config,"person@example.invalid".into(),"synthetic passphrase".into()).is_err());server.join().unwrap();assert!(requests.lock().unwrap().iter().all(|r|!r.line.starts_with("POST /rest/v1/rpc/post_leave_open")));
}
