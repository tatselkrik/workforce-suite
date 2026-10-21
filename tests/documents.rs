use hris_desktop::{access::Role, documents::*, domain::*};
use std::{collections::HashMap, path::PathBuf};
use uuid::Uuid;

const PDF: &[u8] = b"%PDF-1.4\n1 0 obj <</Type /Catalog>> endobj\n%%EOF\n";
struct Fixture {
    store: DemoStore,
    files: HashMap<Uuid, Vec<u8>>,
    folder: PathBuf,
    draft: DocumentDraft,
}
impl Fixture {
    fn new() -> Self {
        let store = DemoStore::new();
        let folder = std::env::temp_dir().join(format!("hris-document-test-{}", Uuid::new_v4()));
        std::fs::create_dir(&folder).unwrap();
        std::fs::write(folder.join("Synthetic contract.pdf"), PDF).unwrap();
        let mut draft = DocumentDraft::new(store.data.employees[1].id);
        draft.title = "  Synthetic employee contract  ".into();
        draft.category = DocumentCategory::Contract;
        draft.issued_on = "2026-01-01".into();
        draft.expires_on = "2027-01-01".into();
        draft.contract_start = "2026-01-02".into();
        draft.contract_end = "2027-01-01".into();
        draft.file = Some(folder.join("Synthetic contract.pdf"));
        Self {
            store,
            files: HashMap::new(),
            folder,
            draft,
        }
    }
    fn execute(&mut self, action: DocumentAction) -> anyhow::Result<String> {
        execute_demo(&mut self.store.data, &mut self.files, action)
    }
    fn upload(&mut self) -> EmployeeDocument {
        self.execute(DocumentAction::Upload(self.draft.clone()))
            .unwrap();
        self.store.data.documents[0].clone()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.folder);
    }
}

#[test]
fn document_details_trim_and_validate_independent_dates() {
    let mut f = Fixture::new();
    assert_eq!(
        f.draft.details().unwrap().title,
        "Synthetic employee contract"
    );
    for (issue, expiry, start, end) in [
        ("bad", "", "", ""),
        ("2026-02-01", "2026-01-01", "", ""),
        ("", "", "", "2026-01-01"),
        ("", "", "2026-02-01", "2026-01-01"),
    ] {
        f.draft.issued_on = issue.into();
        f.draft.expires_on = expiry.into();
        f.draft.contract_start = start.into();
        f.draft.contract_end = end.into();
        assert!(f.draft.details().is_err());
    }
    f.draft.contract_start = "2026-01-01".into();
    f.draft.contract_end.clear();
    f.draft.category = DocumentCategory::Other;
    assert!(f.draft.details().is_err());
    f.draft.contract_start.clear();
    f.draft.title = " ".into();
    assert!(f.draft.details().is_err());
    f.draft.title = "Valid".into();
    f.draft.notes = "a".repeat(1001);
    assert!(f.draft.details().is_err());
}

#[test]
fn only_bounded_pdf_png_jpeg_with_matching_extensions_are_accepted() {
    assert_eq!(validate_bytes(PDF, "scan.PDF").unwrap(), "application/pdf");
    for name in [
        "scan.exe",
        "scan.docx",
        "scan.png",
        "scan.jpg",
        "../scan.pdf",
        "a\\scan.pdf",
        "a:scan.pdf",
        "CON.pdf",
        "scan.pdf.",
    ] {
        assert!(validate_bytes(PDF, name).is_err(), "{name}");
    }
    assert!(validate_bytes(b"not a PDF", "scan.pdf").is_err());
    assert!(validate_bytes(b"%PDF-1.4 no end", "scan.pdf").is_err());
    assert!(validate_bytes(&[], "scan.pdf").is_err());
    assert!(validate_bytes(&vec![b'x'; MAX_FILE_BYTES + 1], "scan.pdf").is_err());
    let image = image::DynamicImage::new_rgb8(4, 4);
    let mut png = std::io::Cursor::new(Vec::new());
    image.write_to(&mut png, image::ImageFormat::Png).unwrap();
    assert_eq!(
        validate_bytes(png.get_ref(), "scan.png").unwrap(),
        "image/png"
    );
    assert!(validate_bytes(png.get_ref(), "scan.jpg").is_err());
}

const TYPES: &str = r#"<?xml version="1.0" encoding="UTF-8"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/></Types>"#;
const RELS: &str = r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#;
const WORD: &str = r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:t>Synthetic &amp; fictional &#65;</w:t></w:r></w:p></w:body></w:document>"#;
fn word_package(types: &str, rels: &str, word: &str, extra: Option<(&str, &[u8])>) -> Vec<u8> {
    use std::io::Write;
    let mut zip = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    for (name, bytes) in [
        ("[Content_Types].xml", types.as_bytes()),
        ("_rels/.rels", rels.as_bytes()),
        ("word/document.xml", word.as_bytes()),
    ]
    .into_iter()
    .chain(extra)
    {
        zip.start_file(name, options).unwrap();
        zip.write_all(bytes).unwrap();
    }
    zip.finish().unwrap().into_inner()
}

#[test]
fn docx_upload_and_download_preserve_the_original_word_package() {
    let bytes = word_package(TYPES, RELS, WORD, None);
    assert_eq!(
        validate_bytes(&bytes, "Synthetic contract.DOCX").unwrap(),
        DOCX_MIME
    );
    let mut f = Fixture::new();
    let source = f.folder.join("Synthetic contract.docx");
    std::fs::write(&source, &bytes).unwrap();
    f.draft.file = Some(source);
    let doc = f.upload();
    assert_eq!(doc.mime_type, DOCX_MIME);
    assert!(doc.object_name.ends_with("/file.docx"));
    let destination = f.folder.join("download.docx");
    save_download(&doc, &bytes, &destination).unwrap();
    assert_eq!(std::fs::read(destination).unwrap(), bytes);
    assert!(save_download(&doc, &bytes, &f.folder.join("download.doc")).is_err());
    assert!(validate_bytes(&bytes, "renamed.docm").is_err());
    assert!(validate_bytes(&bytes, "renamed.doc").is_err());
}

#[test]
fn docx_rejects_renamed_archives_macros_invalid_xml_and_unsafe_package_paths() {
    for (types, rels, word, extra) in [
        (
            TYPES.to_owned(),
            RELS.to_owned(),
            "<not-word/>".to_owned(),
            None,
        ),
        (
            TYPES.replace(
                "wordprocessingml.document.main",
                "wordprocessingml.template.main",
            ),
            RELS.to_owned(),
            WORD.to_owned(),
            None,
        ),
        (
            TYPES.replace(
                "openxmlformats-officedocument.wordprocessingml.document.main",
                "ms-word.document.macroEnabled.main",
            ),
            RELS.to_owned(),
            WORD.to_owned(),
            None,
        ),
        (
            TYPES.to_owned(),
            RELS.replace("word/document.xml", "https://example.invalid/file.xml"),
            WORD.to_owned(),
            None,
        ),
        (
            TYPES.to_owned(),
            RELS.to_owned(),
            WORD.replace("</w:body>", ""),
            None,
        ),
        (
            TYPES.to_owned(),
            RELS.to_owned(),
            WORD.replace("&amp;", "&unknown;"),
            None,
        ),
        (
            TYPES.to_owned(),
            RELS.to_owned(),
            format!("<!DOCTYPE x [<!ENTITY data SYSTEM 'file:///private'>]>{WORD}"),
            None,
        ),
        (
            TYPES.to_owned(),
            RELS.to_owned(),
            WORD.to_owned(),
            Some(("word/vbaProject.bin", b"macro".as_slice())),
        ),
        (
            TYPES.to_owned(),
            RELS.to_owned(),
            WORD.to_owned(),
            Some(("../escape.bin", b"x".as_slice())),
        ),
        (
            TYPES.to_owned(),
            RELS.to_owned(),
            WORD.to_owned(),
            Some(("WORD/DOCUMENT.XML", WORD.as_bytes())),
        ),
    ] {
        assert!(
            validate_bytes(&word_package(&types, &rels, &word, extra), "rejected.docx").is_err()
        );
    }
    assert!(validate_bytes(b"PK fake archive", "renamed.docx").is_err());
    let bytes = word_package(TYPES, RELS, WORD, None);
    assert!(validate_bytes(&bytes[..bytes.len() - 30], "truncated.docx").is_err());
    let mut corrupt = bytes.clone();
    let archive = zip::ZipArchive::new(std::io::Cursor::new(&bytes)).unwrap();
    let offset = archive.clone().by_index(0).unwrap().data_start().unwrap() as usize;
    corrupt[offset] ^= 0xff;
    assert!(validate_bytes(&corrupt, "corrupted.docx").is_err());
    let mut encrypted = bytes;
    let local = encrypted
        .windows(4)
        .position(|w| w == b"PK\x03\x04")
        .unwrap();
    let central = encrypted
        .windows(4)
        .position(|w| w == b"PK\x01\x02")
        .unwrap();
    encrypted[local + 6] |= 1;
    encrypted[central + 8] |= 1;
    assert!(validate_bytes(&encrypted, "encrypted.docx").is_err());
}

#[test]
fn docx_decompression_and_xml_depth_are_bounded() {
    let expanded = vec![b'a'; 64 * 1024 * 1024 + 1];
    let bytes = word_package(
        TYPES,
        RELS,
        WORD,
        Some(("word/media/oversized.bin", &expanded)),
    );
    assert!(bytes.len() < MAX_FILE_BYTES);
    assert!(validate_bytes(&bytes, "oversized.docx").is_err());
    let xml = WORD.replace(
        "Synthetic &amp; fictional &#65;",
        &format!("{}x{}", "<w:r>".repeat(130), "</w:r>".repeat(130)),
    );
    assert!(validate_bytes(&word_package(TYPES, RELS, &xml, None), "deep.docx").is_err());
}

#[test]
fn uploads_preserve_original_bytes_and_record_actor_without_employment_noise() {
    let mut f = Fixture::new();
    let before = f.store.data.employment_history.clone();
    let doc = f.upload();
    assert_eq!(f.files[&doc.id], PDF);
    assert_eq!(doc.created_by_name, "Alex Rivera");
    assert_eq!(doc.details.title, "Synthetic employee contract");
    assert_eq!(doc.revision, 1);
    assert_eq!(doc.state, DocumentState::Ready);
    assert_eq!(
        doc.object_name,
        object_name(
            f.store.data.company.id,
            f.draft.employee_id,
            f.draft.id,
            "application/pdf"
        )
        .unwrap()
    );
    let event = &f.store.data.document_events[0];
    assert_eq!(event.kind, "uploaded");
    assert_eq!(event.actor_role, Role::Administrator);
    assert!(event.before_record.is_none());
    assert_eq!(event.after_record, doc);
    assert_eq!(f.store.data.employment_history, before);
    f.execute(DocumentAction::Upload(f.draft.clone())).unwrap();
    assert_eq!(f.store.data.documents.len(), 1);
    assert_eq!(f.store.data.document_events.len(), 1);
    f.draft.title = "Different document".into();
    assert!(f.execute(DocumentAction::Upload(f.draft.clone())).is_err());
}

#[test]
fn metadata_edits_preserve_file_and_reject_stale_saves_without_extra_events() {
    let mut f = Fixture::new();
    let original = f.upload();
    let mut edit = DocumentDraft::existing(&original);
    edit.title = "Corrected contract label".into();
    f.execute(DocumentAction::Edit(edit.clone())).unwrap();
    let saved = f.store.data.documents[0].clone();
    assert_eq!(saved.revision, 2);
    assert_eq!(saved.sha256, original.sha256);
    assert_eq!(saved.object_name, original.object_name);
    assert_eq!(
        f.store.data.document_events[1].before_record,
        Some(original.clone())
    );
    assert_eq!(f.files[&saved.id], PDF);
    assert!(f.execute(DocumentAction::Edit(edit)).is_err());
    assert_eq!(f.store.data.document_events.len(), 2);
    f.execute(DocumentAction::Edit(DocumentDraft::existing(&saved)))
        .unwrap();
    assert_eq!(f.store.data.document_events.len(), 2);
    assert!(
        f.execute(DocumentAction::Archive {
            id: saved.id,
            revision: 1,
            archived: true
        })
        .is_err()
    );
}

#[test]
fn archiving_employee_and_document_retains_download_and_history_then_restores() {
    let mut f = Fixture::new();
    let doc = f.upload();
    f.store
        .data
        .employees
        .iter_mut()
        .find(|e| e.id == doc.employee_id)
        .unwrap()
        .status = EmployeeStatus::Archived;
    f.execute(DocumentAction::Archive {
        id: doc.id,
        revision: 1,
        archived: true,
    })
    .unwrap();
    assert_eq!(
        f.store.data.documents[0].status(f.store.data.company.today()),
        "Archived"
    );
    assert_eq!(f.store.data.document_events[1].kind, "archived");
    let destination = f.folder.join("download.pdf");
    f.execute(DocumentAction::Download {
        id: doc.id,
        destination: destination.clone(),
    })
    .unwrap();
    assert_eq!(std::fs::read(destination).unwrap(), PDF);
    f.execute(DocumentAction::Archive {
        id: doc.id,
        revision: 2,
        archived: false,
    })
    .unwrap();
    assert!(!f.store.data.documents[0].archived);
    assert_eq!(f.store.data.document_events[2].kind, "restored");
    assert!(f.execute(DocumentAction::Discard { id: doc.id }).is_err());
    assert_eq!(f.files[&doc.id], PDF);
}

#[test]
fn manager_employee_unassigned_and_foreign_company_document_access_are_denied() {
    let mut f = Fixture::new();
    let doc = f.upload();
    for role in [Role::Manager, Role::Employee] {
        f.store.data.access.role = role;
        assert!(records(&f.store.data, doc.employee_id).is_empty());
        assert!(
            f.execute(DocumentAction::Edit(DocumentDraft::existing(&doc)))
                .is_err()
        );
        assert!(
            f.execute(DocumentAction::Download {
                id: doc.id,
                destination: f.folder.join("denied.pdf")
            })
            .is_err()
        );
    }
    f.store.data.access.role = Role::Administrator;
    f.store.data.access.company_id = Uuid::new_v4();
    assert!(records(&f.store.data, doc.employee_id).is_empty());
    f.store.data.access.company_id = f.store.data.company.id;
    f.store.data.documents[0].company_id = Uuid::new_v4();
    assert!(record(&f.store.data, doc.id).is_err());
    assert!(
        Snapshot::signed_out().documents.is_empty()
            && Snapshot::signed_out().document_events.is_empty()
    );
}

#[test]
fn downloads_check_hash_size_path_and_format_without_overwriting_existing_files() {
    let mut f = Fixture::new();
    let doc = f.upload();
    let destination = f.folder.join("untouched.pdf");
    std::fs::write(&destination, b"keep existing file").unwrap();
    assert!(save_download(&doc, PDF, &destination).is_err());
    assert_eq!(std::fs::read(&destination).unwrap(), b"keep existing file");
    let target = f.folder.join("rejected.pdf");
    for broken in [b"tampered PDF".as_slice(), &PDF[..PDF.len() - 1]] {
        assert!(save_download(&doc, broken, &target).is_err());
        assert!(!target.exists());
    }
    let mut corrupt = doc.clone();
    corrupt.object_name = "../private/file.pdf".into();
    assert!(save_download(&corrupt, PDF, &target).is_err());
    corrupt = doc.clone();
    corrupt.state = DocumentState::Pending;
    assert!(save_download(&corrupt, PDF, &target).is_err());
    assert!(save_download(&doc, PDF, &f.folder.join("wrong.exe")).is_err());
    assert!(!target.exists());
}

#[test]
fn expiry_badges_are_reference_dates_and_incomplete_uploads_cannot_be_filed_by_discard() {
    let mut f = Fixture::new();
    let mut doc = f.upload();
    let today = f.store.data.company.today();
    doc.details.expires_on = Some(today - chrono::Duration::days(1));
    assert_eq!(doc.status(today), "Expired");
    doc.details.expires_on = Some(today);
    assert_eq!(doc.status(today), "Expires within 30 days");
    doc.details.expires_on = Some(today + chrono::Duration::days(31));
    assert_eq!(doc.status(today), "Filed");
    f.store.data.documents[0].state = DocumentState::Pending;
    f.execute(DocumentAction::Discard { id: doc.id }).unwrap();
    assert_eq!(f.store.data.documents[0].state, DocumentState::Discarded);
    assert!(!f.files.contains_key(&doc.id));
    assert_eq!(f.store.data.documents.len(), 1);
}
