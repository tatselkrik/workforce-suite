use crate::{
    access::Role,
    domain::{Snapshot, parse_date},
};
use anyhow::{Context, Result, ensure};
use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::{HashMap, HashSet},
    io::{Read, Write},
    path::{Path, PathBuf},
};
use uuid::Uuid;

pub const MAX_FILE_BYTES: usize = 10 * 1024 * 1024;
pub const DOCX_MIME: &str =
    "application/vnd.openxmlformats-officedocument.wordprocessingml.document";

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DocumentCategory {
    Contract,
    Identification,
    Certificate,
    HrForm,
    Acknowledgment,
    #[default]
    Other,
}
impl DocumentCategory {
    pub const ALL: [Self; 6] = [
        Self::Contract,
        Self::Identification,
        Self::Certificate,
        Self::HrForm,
        Self::Acknowledgment,
        Self::Other,
    ];
    pub fn label(self) -> &'static str {
        match self {
            Self::Contract => "Employment contract",
            Self::Identification => "Identification",
            Self::Certificate => "Certificate",
            Self::HrForm => "HR form",
            Self::Acknowledgment => "Acknowledgment",
            Self::Other => "Other attachment",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DocumentState {
    Pending,
    Ready,
    Discarded,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DocumentDetails {
    pub title: String,
    pub category: DocumentCategory,
    pub notes: String,
    pub issued_on: Option<NaiveDate>,
    pub expires_on: Option<NaiveDate>,
    pub contract_start: Option<NaiveDate>,
    pub contract_end: Option<NaiveDate>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EmployeeDocument {
    pub id: Uuid,
    pub company_id: Uuid,
    pub employee_id: Uuid,
    #[serde(flatten)]
    pub details: DocumentDetails,
    pub original_filename: String,
    pub mime_type: String,
    pub byte_size: usize,
    pub sha256: String,
    pub object_name: String,
    pub state: DocumentState,
    pub archived: bool,
    pub revision: i64,
    pub created_by: Uuid,
    pub created_by_name: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
impl EmployeeDocument {
    pub fn status(&self, today: NaiveDate) -> &'static str {
        match self.state {
            DocumentState::Pending => "Incomplete upload",
            DocumentState::Discarded => "Discarded upload",
            DocumentState::Ready if self.archived => "Archived",
            DocumentState::Ready if self.details.expires_on.is_some_and(|d| d < today) => "Expired",
            DocumentState::Ready
                if self
                    .details
                    .expires_on
                    .is_some_and(|d| d.signed_duration_since(today).num_days() <= 30) =>
            {
                "Expires within 30 days"
            }
            DocumentState::Ready => "Filed",
        }
    }
    pub fn validate_file_record(&self) -> Result<()> {
        ensure!(
            self.byte_size > 0 && self.byte_size <= MAX_FILE_BYTES,
            "The stored document size is invalid."
        );
        ensure!(
            self.sha256.len() == 64
                && self
                    .sha256
                    .bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)),
            "The stored document checksum is invalid."
        );
        validate_filename(&self.original_filename)?;
        ensure!(
            self.object_name
                == object_name(self.company_id, self.employee_id, self.id, &self.mime_type)?,
            "The document storage path is invalid."
        );
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DocumentEvent {
    pub id: Uuid,
    pub sequence: i64,
    pub company_id: Uuid,
    pub employee_id: Uuid,
    pub document_id: Uuid,
    pub kind: String,
    pub actor_id: Uuid,
    pub actor_name: String,
    pub actor_role: Role,
    pub before_record: Option<EmployeeDocument>,
    pub after_record: EmployeeDocument,
    pub recorded_at: DateTime<Utc>,
}

#[derive(Clone, Debug)]
pub struct DocumentDraft {
    pub id: Uuid,
    pub employee_id: Uuid,
    pub title: String,
    pub category: DocumentCategory,
    pub notes: String,
    pub issued_on: String,
    pub expires_on: String,
    pub contract_start: String,
    pub contract_end: String,
    pub file: Option<PathBuf>,
    pub original: Option<EmployeeDocument>,
}
impl DocumentDraft {
    pub fn new(employee_id: Uuid) -> Self {
        Self {
            id: Uuid::new_v4(),
            employee_id,
            title: String::new(),
            category: DocumentCategory::Other,
            notes: String::new(),
            issued_on: String::new(),
            expires_on: String::new(),
            contract_start: String::new(),
            contract_end: String::new(),
            file: None,
            original: None,
        }
    }
    pub fn existing(record: &EmployeeDocument) -> Self {
        let date = |d: Option<NaiveDate>| d.map(|d| d.to_string()).unwrap_or_default();
        Self {
            id: record.id,
            employee_id: record.employee_id,
            title: record.details.title.clone(),
            category: record.details.category,
            notes: record.details.notes.clone(),
            issued_on: date(record.details.issued_on),
            expires_on: date(record.details.expires_on),
            contract_start: date(record.details.contract_start),
            contract_end: date(record.details.contract_end),
            file: None,
            original: Some(record.clone()),
        }
    }
    pub fn details(&self) -> Result<DocumentDetails> {
        ensure!(
            !self.title.trim().is_empty()
                && self.title.trim().chars().count() <= 120
                && !self.title.chars().any(char::is_control),
            "Enter a document title of 1–120 characters."
        );
        ensure!(
            self.notes.trim().chars().count() <= 1000,
            "Keep document notes to 1,000 characters."
        );
        let date = |s: &str| {
            if s.trim().is_empty() {
                Ok(None)
            } else {
                parse_date(s).map(Some)
            }
        };
        let details = DocumentDetails {
            title: self.title.trim().into(),
            category: self.category,
            notes: self.notes.trim().into(),
            issued_on: date(&self.issued_on)?,
            expires_on: date(&self.expires_on)?,
            contract_start: date(&self.contract_start)?,
            contract_end: date(&self.contract_end)?,
        };
        ensure!(
            details
                .issued_on
                .zip(details.expires_on)
                .is_none_or(|(a, b)| b >= a),
            "Expiry date must be on or after the issue date."
        );
        ensure!(
            details.contract_end.is_none() || details.contract_start.is_some(),
            "Enter the contract start date before its end date."
        );
        ensure!(
            details
                .contract_start
                .zip(details.contract_end)
                .is_none_or(|(a, b)| b >= a),
            "Contract end must be on or after its start."
        );
        ensure!(
            details.category == DocumentCategory::Contract
                || (details.contract_start.is_none() && details.contract_end.is_none()),
            "Contract dates apply only to employment contracts."
        );
        Ok(details)
    }
    pub fn payload(&self) -> Result<Value> {
        let mut value = serde_json::to_value(self.details()?)?;
        value["id"] = json!(self.id);
        value["employee_id"] = json!(self.employee_id);
        value["revision"] = json!(self.original.as_ref().map_or(0, |d| d.revision));
        Ok(value)
    }
}

pub enum DocumentAction {
    Upload(DocumentDraft),
    Edit(DocumentDraft),
    Archive {
        id: Uuid,
        revision: i64,
        archived: bool,
    },
    Discard {
        id: Uuid,
    },
    Download {
        id: Uuid,
        destination: PathBuf,
    },
}

pub fn authorize(data: &Snapshot, employee: Uuid) -> Result<()> {
    ensure!(
        data.access.allows(crate::staff_roles::Capability::PersonnelDocuments)
            && data.access.company_id == data.company.id
            && data
                .employees
                .iter()
                .any(|e| e.company_id == data.company.id && e.id == employee),
        "Only HR and Administrators can manage this employee's documents."
    );
    Ok(())
}
pub fn record(data: &Snapshot, id: Uuid) -> Result<&EmployeeDocument> {
    let doc = data
        .documents
        .iter()
        .find(|d| d.id == id && d.company_id == data.company.id)
        .context("Document not found. Refresh the employee record.")?;
    authorize(data, doc.employee_id)?;
    Ok(doc)
}
pub fn records(data: &Snapshot, employee: Uuid) -> Vec<&EmployeeDocument> {
    if authorize(data, employee).is_err() {
        return Vec::new();
    }
    let mut rows: Vec<_> = data
        .documents
        .iter()
        .filter(|d| d.company_id == data.company.id && d.employee_id == employee)
        .collect();
    rows.sort_by(|a, b| b.created_at.cmp(&a.created_at).then(b.id.cmp(&a.id)));
    rows
}
pub fn object_name(company: Uuid, employee: Uuid, id: Uuid, mime: &str) -> Result<String> {
    let extension = match mime {
        "application/pdf" => "pdf",
        "image/png" => "png",
        "image/jpeg" => "jpg",
        DOCX_MIME => "docx",
        _ => anyhow::bail!("Use a PDF, DOCX, PNG or JPEG document."),
    };
    Ok(format!("{company}/{employee}/{id}/file.{extension}"))
}
pub fn validate_filename(name: &str) -> Result<()> {
    ensure!(
        !name.is_empty()
            && name.chars().count() <= 160
            && !name
                .chars()
                .any(|c| c.is_control() || "/\\:<>\"|?*".contains(c))
            && !name.ends_with(['.', ' ']),
        "Use a filename of 1–160 characters without path or reserved characters."
    );
    let stem = name.split('.').next().unwrap_or("").to_uppercase();
    ensure!(
        ![
            "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7",
            "COM8", "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9"
        ]
        .contains(&stem.as_str()),
        "Choose a different filename; this name is reserved by Windows."
    );
    Ok(())
}
pub fn validate_bytes(bytes: &[u8], filename: &str) -> Result<&'static str> {
    validate_filename(filename)?;
    ensure!(
        !bytes.is_empty() && bytes.len() <= MAX_FILE_BYTES,
        "Choose a document up to 10 MiB."
    );
    let extension = Path::new(filename)
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    let (mime, format) = match extension.as_str() {
        "pdf" => {
            ensure!(
                bytes.starts_with(b"%PDF-")
                    && bytes[bytes.len().saturating_sub(2048)..]
                        .windows(5)
                        .any(|w| w == b"%%EOF"),
                "This file does not have a supported PDF structure."
            );
            ("application/pdf", None)
        }
        "png" => ("image/png", Some(image::ImageFormat::Png)),
        "jpg" | "jpeg" => ("image/jpeg", Some(image::ImageFormat::Jpeg)),
        "docx" => {
            validate_docx(bytes).context("This file is not a supported DOCX document.")?;
            (DOCX_MIME, None)
        }
        _ => anyhow::bail!("Choose a PDF, DOCX, PNG or JPEG file."),
    };
    if let Some(format) = format {
        let mut reader = image::ImageReader::with_format(std::io::Cursor::new(bytes), format);
        let mut limits = image::Limits::default();
        limits.max_image_width = Some(8192);
        limits.max_image_height = Some(8192);
        limits.max_alloc = Some(128 * 1024 * 1024);
        reader.limits(limits);
        reader
            .decode()
            .context("The image is invalid or exceeds 8192 × 8192 pixels.")?;
    }
    Ok(mime)
}

// Inspect in memory only: never extract, run, render or modify a Word package.
fn validate_docx(bytes: &[u8]) -> Result<()> {
    const MAX_EXPANDED: u64 = 64 * 1024 * 1024;
    const MAX_XML: u64 = 8 * 1024 * 1024;
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes))?;
    ensure!(
        archive.len() <= 2000,
        "This Word package contains too many parts."
    );
    ensure!(
        !archive.has_overlapping_entries()?,
        "Word package entries overlap."
    );
    let mut names = HashSet::new();
    let mut total = 0u64;
    let mut parts = HashMap::new();
    for index in 0..archive.len() {
        let mut entry = archive.by_index(index)?;
        let name = entry.name()?.into_owned();
        let lower = name.to_ascii_lowercase();
        ensure!(
            !name.starts_with('/')
                && !name.contains(['\\', ':'])
                && !name.chars().any(char::is_control)
                && !name
                    .trim_end_matches('/')
                    .split('/')
                    .any(|p| p.is_empty() || p == "." || p == "..")
                && names.insert(lower.clone())
                && entry
                    .unix_mode()
                    .is_none_or(|mode| mode & 0o170000 != 0o120000),
            "Word package paths must be unique, ordinary relative paths."
        );
        ensure!(
            !entry.encrypted(),
            "Use a DOCX without password encryption."
        );
        ensure!(
            matches!(
                entry.compression(),
                zip::CompressionMethod::Stored | zip::CompressionMethod::Deflated
            ),
            "This Word package uses unsupported compression."
        );
        ensure!(
            !lower.ends_with("vbaproject.bin"),
            "Macro-enabled Word documents are not supported."
        );
        total = total
            .checked_add(entry.size())
            .context("Word package size overflow.")?;
        ensure!(
            total <= MAX_EXPANDED,
            "This Word package expands beyond 64 MiB."
        );
        let required = matches!(
            name.as_str(),
            "[Content_Types].xml" | "_rels/.rels" | "word/document.xml"
        );
        if required {
            ensure!(entry.size() <= MAX_XML, "A Word XML part exceeds 8 MiB.");
            let mut xml = Vec::new();
            (&mut entry).take(MAX_XML + 1).read_to_end(&mut xml)?;
            ensure!(
                xml.len() as u64 == entry.size(),
                "A Word XML part has an invalid size."
            );
            parts.insert(name, xml);
        } else {
            // Stream every part to EOF to check CRC and actual expanded size, with no allocation.
            let actual = std::io::copy(
                &mut (&mut entry).take(MAX_EXPANDED + 1),
                &mut std::io::sink(),
            )?;
            ensure!(
                actual == entry.size(),
                "A Word package part has an invalid size."
            );
        }
    }
    let part = |name| {
        parts
            .get(name)
            .context("The Word package is missing a required part.")
    };
    let content_ns = b"http://schemas.openxmlformats.org/package/2006/content-types".as_slice();
    let mut word_type = false;
    inspect_word_xml(
        part("[Content_Types].xml")?,
        b"Types",
        &[content_ns],
        |element, ns, depth| {
            for attribute in element.attributes() {
                let attribute = attribute?;
                let value = attribute.normalized_value(quick_xml::XmlVersion::Implicit1_0)?;
                let lower = value.to_ascii_lowercase();
                ensure!(
                    !lower.contains("macroenabled") && !lower.contains("vbaproject"),
                    "Macro-enabled Word documents are not supported."
                );
            }
            if depth == 1 && ns == content_ns && element.local_name().as_ref() == b"Override" {
                let attr = |key| -> Result<Option<String>> {
                    element
                        .try_get_attribute(key)?
                        .map(|a| {
                            Ok(a.normalized_value(quick_xml::XmlVersion::Implicit1_0)?
                                .into_owned())
                        })
                        .transpose()
                };
                if attr("PartName")?.as_deref() == Some("/word/document.xml") {
                    ensure!(!word_type, "Duplicate Word document content type.");
                    word_type = attr("ContentType")?.as_deref()
                        == Some(
                            "application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml",
                        );
                    ensure!(word_type, "The main part is not a DOCX Word document.");
                }
            }
            Ok(())
        },
    )?;
    ensure!(word_type, "The package does not declare a Word document.");
    let rel_ns = b"http://schemas.openxmlformats.org/package/2006/relationships".as_slice();
    let mut relationship = false;
    inspect_word_xml(
        part("_rels/.rels")?,
        b"Relationships",
        &[rel_ns],
        |element, ns, depth| {
            if depth == 1 && ns == rel_ns && element.local_name().as_ref() == b"Relationship" {
                let attr = |key| -> Result<Option<String>> {
                    element
                        .try_get_attribute(key)?
                        .map(|a| {
                            Ok(a.normalized_value(quick_xml::XmlVersion::Implicit1_0)?
                                .into_owned())
                        })
                        .transpose()
                };
                if matches!(
                    attr("Type")?.as_deref(),
                    Some(
                        "http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument"
                            | "http://purl.oclc.org/ooxml/officeDocument/relationships/officeDocument"
                    )
                ) {
                    ensure!(
                        !relationship
                            && matches!(
                                attr("Target")?.as_deref(),
                                Some("word/document.xml" | "/word/document.xml")
                            )
                            && attr("TargetMode")?
                                .as_deref()
                                .is_none_or(|v| v == "Internal"),
                        "Invalid Word document relationship."
                    );
                    relationship = true;
                }
            }
            Ok(())
        },
    )?;
    ensure!(
        relationship,
        "The package does not reference its Word document."
    );
    let word_ns = [
        b"http://schemas.openxmlformats.org/wordprocessingml/2006/main".as_slice(),
        b"http://purl.oclc.org/ooxml/wordprocessingml/main".as_slice(),
    ];
    let mut body = false;
    inspect_word_xml(
        part("word/document.xml")?,
        b"document",
        &word_ns,
        |element, ns, depth| {
            if depth == 1 && word_ns.contains(&ns) && element.local_name().as_ref() == b"body" {
                ensure!(!body, "Duplicate Word document body.");
                body = true;
            }
            Ok(())
        },
    )?;
    ensure!(body, "The Word document has no body.");
    Ok(())
}

fn inspect_word_xml(
    bytes: &[u8],
    root: &[u8],
    namespaces: &[&[u8]],
    mut inspect: impl FnMut(&quick_xml::events::BytesStart<'_>, &[u8], usize) -> Result<()>,
) -> Result<()> {
    use quick_xml::{events::Event, name::ResolveResult, reader::NsReader};
    let text = std::str::from_utf8(bytes).context("Use a Word document with UTF-8 XML parts.")?;
    let mut reader = NsReader::from_str(text);
    let mut depth = 0usize;
    let mut roots = 0;
    loop {
        let (ns, event) = reader.read_resolved_event()?;
        match event {
            Event::Start(ref element) | Event::Empty(ref element) => {
                let ResolveResult::Bound(ns) = ns else {
                    anyhow::bail!("A Word XML element has no valid namespace.");
                };
                if depth == 0 {
                    roots += 1;
                    ensure!(
                        roots == 1
                            && element.local_name().as_ref() == root
                            && namespaces.contains(&ns.as_ref()),
                        "Invalid Word XML root."
                    );
                }
                for attribute in element.attributes() {
                    attribute?.normalized_value(quick_xml::XmlVersion::Implicit1_0)?;
                }
                inspect(element, ns.as_ref(), depth)?;
                if matches!(event, Event::Start(_)) {
                    depth += 1;
                }
                ensure!(depth <= 128, "The Word XML nesting is too deep.");
            }
            Event::End(_) => {
                depth = depth
                    .checked_sub(1)
                    .context("Unexpected Word XML end tag.")?;
            }
            Event::DocType(_) => {
                anyhow::bail!("Word XML document type declarations are not supported.")
            }
            Event::Text(ref text) => {
                ensure!(
                    depth > 0 || text.iter().all(u8::is_ascii_whitespace),
                    "Text outside the Word XML root."
                );
            }
            Event::CData(_) => {
                ensure!(depth > 0, "Content outside the Word XML root.");
            }
            Event::GeneralRef(ref entity) => {
                ensure!(
                    depth > 0
                        && (matches!(&entity[..], b"amp" | b"lt" | b"gt" | b"quot" | b"apos")
                            || entity.resolve_char_ref()?.is_some()),
                    "Invalid Word XML entity."
                );
            }
            Event::Decl(ref decl) => {
                ensure!(
                    decl.encoding()
                        .transpose()?
                        .is_none_or(|e| e.eq_ignore_ascii_case(b"utf-8")
                            || e.eq_ignore_ascii_case(b"us-ascii")),
                    "Use UTF-8 Word XML parts."
                );
            }
            Event::Eof => {
                ensure!(roots == 1 && depth == 0, "Incomplete Word XML.");
                break;
            }
            _ => {}
        }
    }
    Ok(())
}
pub struct PreparedUpload {
    pub payload: Value,
    pub bytes: Vec<u8>,
    pub mime: &'static str,
}
pub fn prepare_upload(draft: &DocumentDraft, data: &Snapshot) -> Result<PreparedUpload> {
    authorize(data, draft.employee_id)?;
    ensure!(
        draft
            .original
            .as_ref()
            .is_none_or(|d| d.state == DocumentState::Pending),
        "Upload a new document to add another file."
    );
    let file = draft
        .file
        .as_ref()
        .context("Choose a document to upload.")?;
    let mut prepared = prepare_file(file)?;
    let mut payload = draft.payload()?;
    payload
        .as_object_mut()
        .unwrap()
        .extend(prepared.payload.as_object().unwrap().clone());
    prepared.payload = payload;
    Ok(prepared)
}
/// Shared bounded file validation for employee and applicant attachments.
pub fn prepare_file(file: &Path) -> Result<PreparedUpload> {
    let name = file
        .file_name()
        .and_then(|s| s.to_str())
        .context("Use a valid document filename.")?;
    let mut bytes = Vec::new();
    std::fs::File::open(file)
        .context("Could not read the selected document.")?
        .take(MAX_FILE_BYTES as u64 + 1)
        .read_to_end(&mut bytes)?;
    let mime = validate_bytes(&bytes, name)?;
    let mut payload = json!({});
    payload["original_filename"] = json!(name);
    payload["mime_type"] = json!(mime);
    payload["byte_size"] = json!(bytes.len());
    payload["sha256"] = json!(format!("{:x}", Sha256::digest(&bytes)));
    Ok(PreparedUpload {
        payload,
        bytes,
        mime,
    })
}
pub fn save_download(doc: &EmployeeDocument, bytes: &[u8], destination: &Path) -> Result<()> {
    doc.validate_file_record()?;
    ensure!(
        doc.state == DocumentState::Ready,
        "This document has not finished uploading."
    );
    ensure!(
        bytes.len() == doc.byte_size && format!("{:x}", Sha256::digest(bytes)) == doc.sha256,
        "The downloaded document did not match its size or checksum. No file was saved."
    );
    ensure!(
        validate_bytes(bytes, &doc.original_filename)? == doc.mime_type,
        "The downloaded document format is invalid."
    );
    let extension = |p: &Path| {
        p.extension()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_ascii_lowercase()
    };
    ensure!(
        extension(destination) == extension(Path::new(&doc.original_filename)),
        "Keep the document's original file extension."
    );
    let mut file = std::fs::OpenOptions::new().write(true).create_new(true).open(destination)
        .context("Could not save this document. Choose an accessible folder and a filename that does not already exist.")?;
    if let Err(error) = file.write_all(bytes).and_then(|_| file.sync_all()) {
        drop(file);
        let _ = std::fs::remove_file(destination);
        return Err(error).context("Could not finish saving the document. Try another folder.");
    }
    Ok(())
}

pub fn execute_demo(
    data: &mut Snapshot,
    files: &mut HashMap<Uuid, Vec<u8>>,
    action: DocumentAction,
) -> Result<String> {
    let (mut doc, before, kind) = match action {
        DocumentAction::Upload(draft) => {
            let prepared = prepare_upload(&draft, data)?;
            let existing = data.documents.iter().find(|d| d.id == draft.id).cloned();
            if let Some(existing) = existing.as_ref() {
                ensure!(
                    existing.sha256 == prepared.payload["sha256"].as_str().unwrap_or("")
                        && existing.details == draft.details()?
                        && existing.employee_id == draft.employee_id
                        && existing.created_by == data.access.user_id
                        && existing.state != DocumentState::Discarded
                        && existing.original_filename
                            == prepared.payload["original_filename"].as_str().unwrap_or(""),
                    "This upload already uses a different file."
                );
                if existing.state == DocumentState::Ready {
                    return Ok("Document already filed in the demo.".into());
                }
            }
            let filename = prepared.payload["original_filename"]
                .as_str()
                .unwrap_or("")
                .to_owned();
            let now = Utc::now();
            let mut doc = EmployeeDocument {
                id: draft.id,
                company_id: data.company.id,
                employee_id: draft.employee_id,
                details: draft.details()?,
                original_filename: filename,
                mime_type: prepared.mime.into(),
                byte_size: prepared.bytes.len(),
                sha256: format!("{:x}", Sha256::digest(&prepared.bytes)),
                object_name: object_name(
                    data.company.id,
                    draft.employee_id,
                    draft.id,
                    prepared.mime,
                )?,
                state: DocumentState::Ready,
                archived: false,
                revision: 1,
                created_by: data.access.user_id,
                created_by_name: data
                    .own_employee()
                    .map(|e| e.full_name.clone())
                    .unwrap_or_else(|| "HR staff".into()),
                created_at: now,
                updated_at: now,
            };
            if let Some(pending) = existing {
                doc.created_at = pending.created_at;
                doc.created_by_name = pending.created_by_name;
                doc.revision = pending.revision + 1;
            }
            files.insert(doc.id, prepared.bytes);
            (doc, None, "uploaded")
        }
        DocumentAction::Edit(draft) => {
            let old = record(data, draft.id)?.clone();
            ensure!(
                old.state == DocumentState::Ready
                    && draft
                        .original
                        .as_ref()
                        .is_some_and(|d| d.revision == old.revision),
                "This document changed. Refresh and reopen it before saving."
            );
            let mut edited = old.clone();
            edited.details = draft.details()?;
            if old == edited {
                return Ok("No document details changed.".into());
            }
            (edited, Some(old), "details_updated")
        }
        DocumentAction::Archive {
            id,
            revision,
            archived,
        } => {
            let old = record(data, id)?.clone();
            ensure!(
                old.state == DocumentState::Ready && old.revision == revision,
                "This document changed. Refresh before archiving or restoring it."
            );
            if old.archived == archived {
                return Ok("Document status is unchanged.".into());
            }
            let mut edited = old.clone();
            edited.archived = archived;
            (
                edited,
                Some(old),
                if archived { "archived" } else { "restored" },
            )
        }
        DocumentAction::Download { id, destination } => {
            let doc = record(data, id)?;
            save_download(doc, files.get(&id).context("This fictional preview has no uploaded file. Upload a sample document to test downloading.")?, &destination)?;
            return Ok("Document downloaded to the file you selected.".into());
        }
        DocumentAction::Discard { id } => {
            let doc = record(data, id)?.clone();
            ensure!(
                doc.state != DocumentState::Ready,
                "Filed documents are retained. Archive this document instead."
            );
            data.documents
                .iter_mut()
                .find(|d| d.id == id)
                .unwrap()
                .state = DocumentState::Discarded;
            files.remove(&id);
            return Ok("Incomplete upload discarded.".into());
        }
    };
    if before.is_some() {
        doc.revision += 1;
        doc.updated_at = Utc::now();
    }
    let sequence = data
        .document_events
        .iter()
        .map(|e| e.sequence)
        .max()
        .unwrap_or(0)
        + 1;
    data.document_events.push(DocumentEvent {
        id: Uuid::new_v4(),
        sequence,
        company_id: data.company.id,
        employee_id: doc.employee_id,
        document_id: doc.id,
        kind: kind.into(),
        actor_id: data.access.user_id,
        actor_name: data
            .own_employee()
            .map(|e| e.full_name.clone())
            .unwrap_or_else(|| "HR staff".into()),
        actor_role: data.access.role,
        before_record: before,
        after_record: doc.clone(),
        recorded_at: Utc::now(),
    });
    if let Some(existing) = data.documents.iter_mut().find(|d| d.id == doc.id) {
        *existing = doc;
    } else {
        data.documents.push(doc);
    }
    Ok("Document saved in the demo; changes reset when the app closes.".into())
}

#[cfg(windows)]
pub fn choose_file(save_as: Option<&str>) -> Result<Option<PathBuf>> {
    use std::{ffi::OsString, os::windows::ffi::OsStringExt};
    use windows_sys::Win32::UI::{Controls::Dialogs::*, Input::KeyboardAndMouse::GetActiveWindow};
    let mut filename = vec![0u16; 32768];
    if let Some(name) = save_as {
        validate_filename(name)?;
        for (slot, character) in filename.iter_mut().zip(name.encode_utf16()) {
            *slot = character;
        }
    }
    let filter: Vec<u16> = "Documents (PDF, DOCX, PNG, JPEG)\0*.pdf;*.docx;*.png;*.jpg;*.jpeg\0\0"
        .encode_utf16()
        .collect();
    let title: Vec<u16> = if save_as.is_some() {
        "Download document\0"
    } else {
        "Choose document\0"
    }
    .encode_utf16()
    .collect();
    // SAFETY: all dialog buffers outlive this synchronous UI-thread call.
    let mut dialog: OPENFILENAMEW = unsafe { std::mem::zeroed() };
    dialog.lStructSize = std::mem::size_of::<OPENFILENAMEW>() as u32;
    dialog.hwndOwner = unsafe { GetActiveWindow() };
    dialog.lpstrFilter = filter.as_ptr();
    dialog.lpstrTitle = title.as_ptr();
    dialog.lpstrFile = filename.as_mut_ptr();
    dialog.nMaxFile = filename.len() as u32;
    dialog.Flags = OFN_EXPLORER
        | OFN_PATHMUSTEXIST
        | OFN_NOCHANGEDIR
        | OFN_DONTADDTORECENT
        | if save_as.is_some() {
            OFN_OVERWRITEPROMPT
        } else {
            OFN_FILEMUSTEXIST
        };
    let selected = unsafe {
        if save_as.is_some() {
            GetSaveFileNameW(&mut dialog)
        } else {
            GetOpenFileNameW(&mut dialog)
        }
    };
    if selected != 0 {
        let length = filename
            .iter()
            .position(|c| *c == 0)
            .unwrap_or(filename.len());
        Ok(Some(PathBuf::from(OsString::from_wide(
            &filename[..length],
        ))))
    } else {
        ensure!(
            unsafe { CommDlgExtendedError() } == 0,
            "Could not open the document file chooser."
        );
        Ok(None)
    }
}
#[cfg(not(windows))]
pub fn choose_file(_: Option<&str>) -> Result<Option<PathBuf>> {
    anyhow::bail!("Document file selection currently supports Windows only.")
}
