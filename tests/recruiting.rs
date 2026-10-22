use chrono::{Duration, Utc};
use hris_desktop::{
    access::Role,
    domain::{Action, DemoStore},
    hiring::{ApplicantDraft, HiringAction},
    recruiting::*,
};
fn fixture() -> (DemoStore, uuid::Uuid, InterviewDraft) {
    let mut s = DemoStore::new();
    let own = s.data.access.employee_id.unwrap();
    s.data.recruiting.interviewers.push(Interviewer {
        employee_id: own,
        full_name: "Fictional Reviewer".into(),
        role: Role::Administrator,
    });
    let mut app = ApplicantDraft::new(s.data.company.today());
    app.full_name = "Synthetic Recruiting Candidate".into();
    app.position = "Associate".into();
    let id = app.id.unwrap();
    s.execute(Action::Hiring(HiringAction::Create {
        draft: app,
        reason: "Application received".into(),
    }))
    .unwrap();
    let mut d = InterviewDraft::new(&s.data, id);
    d.interviewer = Some(own);
    d.date = (s.data.company.today() - Duration::days(2)).to_string();
    (s, id, d)
}
#[test]
fn interview_schedule_keeps_retry_identity_and_guards_overlap_time_and_revision() {
    let (mut s, _, d) = fixture();
    let payload = d.payload(&s.data).unwrap();
    assert_eq!(payload["p_record"]["id"], serde_json::json!(d.id));
    for k in ["actor_id", "company_id", "applicant_name", "status"] {
        assert!(payload["p_record"].get(k).is_none());
    }
    s.execute(Action::Interview(InterviewAction::Save(d.clone())))
        .unwrap();
    s.execute(Action::Interview(InterviewAction::Save(d.clone())))
        .unwrap();
    assert_eq!(s.data.recruiting.events.len(), 1);
    let mut overlap = d.clone();
    overlap.id = uuid::Uuid::new_v4();
    assert!(overlap.payload(&s.data).is_err());
    let mut adjacent = overlap.clone();
    adjacent.start = "10:00".into();
    adjacent.end = "11:00".into();
    adjacent.payload(&s.data).unwrap();
    for (start, end) in [("11:00", "10:00"), ("09:00", "18:00"), ("25:00", "26:00")] {
        let mut bad = d.clone();
        bad.start = start.into();
        bad.end = end.into();
        assert!(bad.payload(&s.data).is_err());
    }
    let mut edit = InterviewDraft::existing(&s.data, &s.data.recruiting.interviews[0]);
    edit.reason = "Rescheduled".into();
    edit.location = "Another fictional room".into();
    s.execute(Action::Interview(InterviewAction::Save(edit.clone())))
        .unwrap();
    assert!(edit.payload(&s.data).is_err());
    assert_eq!(s.data.recruiting.interviews[0].revision, 1);
    s.execute(Action::Interview(InterviewAction::Save(d)))
        .unwrap();
    assert_eq!(
        s.data.recruiting.interviews[0].location,
        "Another fictional room"
    );
    assert_eq!(s.data.recruiting.events.len(), 2);
}
#[test]
fn interviewer_feedback_requires_current_assignment_reason_and_started_interview() {
    let (mut s, _, d) = fixture();
    s.execute(Action::Interview(InterviewAction::Save(d.clone())))
        .unwrap();
    let mut f = FeedbackDraft::new(&s.data, d.id);
    f.assessment = "Fictional competency assessment".into();
    f.rating = 4;
    let action = InterviewAction::Feedback(f.clone());
    action.payload(&s.data, Utc::now()).unwrap();
    assert!(
        action
            .payload(&s.data, Utc::now() - Duration::days(10))
            .is_err()
    );
    s.data.access.role = Role::Manager;
    s.execute(Action::Interview(action)).unwrap();
    let saved = &s.data.recruiting.feedback[0];
    assert_eq!(saved.author_id, s.data.access.user_id);
    assert_eq!(saved.author_role, Role::Manager);
    assert_eq!(saved.revision, 1);
    assert!(
        InterviewAction::Feedback(f.clone())
            .payload(&s.data, Utc::now())
            .is_err()
    );
    f.expected_revision = 1;
    f.reason.clear();
    assert!(
        InterviewAction::Feedback(f.clone())
            .payload(&s.data, Utc::now())
            .is_err()
    );
    f.reason = "Corrected assessment".into();
    f.rating = 6;
    assert!(
        InterviewAction::Feedback(f.clone())
            .payload(&s.data, Utc::now())
            .is_err()
    );
    f.rating = 3;
    s.data.access.employee_id = Some(uuid::Uuid::new_v4());
    assert!(
        InterviewAction::Feedback(f)
            .payload(&s.data, Utc::now())
            .is_err()
    );
    assert!(d.payload(&s.data).is_err());
}
#[test]
fn interview_status_retains_history_without_creating_employees_or_accounts() {
    let (mut s, _, d) = fixture();
    let before = serde_json::to_value(&s.data.employees).unwrap();
    let accounts = serde_json::to_value(&s.data.accounts).unwrap();
    s.execute(Action::Interview(InterviewAction::Save(d.clone())))
        .unwrap();
    s.execute(Action::Interview(InterviewAction::Status {
        id: d.id,
        expected_revision: 0,
        status: InterviewStatus::Completed,
        reason: "Interview held".into(),
    }))
    .unwrap();
    assert_eq!(
        s.data.recruiting.interviews[0].status,
        InterviewStatus::Completed
    );
    assert_eq!(s.data.recruiting.events.len(), 2);
    let e = &s.data.recruiting.events[1];
    assert_eq!(e.before_record.as_ref().unwrap()["status"], "scheduled");
    assert_eq!(e.after_record["status"], "completed");
    assert!(
        InterviewDraft::existing(&s.data, &s.data.recruiting.interviews[0])
            .payload(&s.data)
            .is_err()
    );
    assert_eq!(serde_json::to_value(&s.data.employees).unwrap(), before);
    assert_eq!(serde_json::to_value(&s.data.accounts).unwrap(), accounts);
}
#[test]
fn private_resume_upload_download_and_archive_leave_employee_documents_separate() {
    let (mut s, id, _) = fixture();
    let original = serde_json::to_value(&s.data.employees).unwrap();
    let root = std::env::temp_dir().join(format!("hris-resume-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&root).unwrap();
    let file = root.join("resume.pdf");
    let bytes = b"%PDF-1.4\nSynthetic resume\n%%EOF";
    std::fs::write(&file, bytes).unwrap();
    let mut d = ResumeDraft::new(id);
    d.document.file = Some(file);
    let prepared = d.prepared(&s.data).unwrap();
    assert_eq!(prepared.payload["applicant_id"], serde_json::json!(id));
    assert!(prepared.payload.get("employee_id").is_none());
    let mut files = Default::default();
    resume_demo(&mut s.data, &mut files, ResumeAction::Upload(d.clone())).unwrap();
    assert_eq!(s.data.recruiting.documents.len(), 1);
    assert!(s.data.documents.is_empty());
    let doc = s.data.recruiting.documents[0].clone();
    doc.validate().unwrap();
    let destination = root.join("download.pdf");
    resume_demo(
        &mut s.data,
        &mut files,
        ResumeAction::Download {
            id: doc.id,
            destination: destination.clone(),
        },
    )
    .unwrap();
    assert_eq!(std::fs::read(&destination).unwrap(), bytes);
    assert!(
        resume_demo(
            &mut s.data,
            &mut files,
            ResumeAction::Download {
                id: doc.id,
                destination
            }
        )
        .is_err()
    );
    resume_demo(
        &mut s.data,
        &mut files,
        ResumeAction::Archive {
            id: doc.id,
            revision: doc.revision,
            archived: true,
        },
    )
    .unwrap();
    assert!(s.data.recruiting.documents[0].archived);
    assert_eq!(s.data.recruiting.document_events.len(), 2);
    s.data.access.role = Role::Manager;
    assert!(attachment(&s.data, doc.id).is_err());
    assert!(d.prepared(&s.data).is_err());
    assert_eq!(serde_json::to_value(&s.data.employees).unwrap(), original);
    std::fs::remove_dir_all(root).unwrap();
}
