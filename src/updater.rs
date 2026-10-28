use anyhow::{Context, Result, ensure};
use base64::{Engine, engine::general_purpose::STANDARD};
use ed25519_dalek::{Signature, VerifyingKey};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::Read,
    path::{Path, PathBuf},
    process::Command,
};

pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const MAX_BYTES: u64 = 64 * 1024 * 1024;
pub const CONFIG_FILE: &str = "hris-updates.json";
const APP_FILES: &[&str] = &[
    "HRIS-Desktop.exe",
    "BUILD.json",
    "SHA256SUMS.txt",
    "START-HERE.txt",
    "VERIFICATION.md",
];

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Version(pub u32, pub u32, pub u32);
impl Version {
    pub fn parse(value: &str) -> Result<Self> {
        let parts: Vec<_> = value.split('.').collect();
        ensure!(parts.len() == 3, "Unsupported update version.");
        let mut numbers = [0; 3];
        for (i, part) in parts.iter().enumerate() {
            ensure!(
                !part.is_empty()
                    && part.len() <= 6
                    && part.bytes().all(|b| b.is_ascii_digit())
                    && (part.len() == 1 || !part.starts_with('0')),
                "Unsupported update version."
            );
            numbers[i] = part.parse()?;
        }
        Ok(Self(numbers[0], numbers[1], numbers[2]))
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UpdateConfig {
    pub public_key: String,
}
impl UpdateConfig {
    pub fn load() -> Result<Option<Self>> {
        let file = std::env::current_exe()?
            .parent()
            .context("Application folder missing")?
            .join(CONFIG_FILE);
        if !file.exists() {
            return Ok(None);
        }
        Self::from_file(&file).map(Some)
    }
    fn from_file(file: &Path) -> Result<Self> {
        let bytes = read_bounded(file, 4096)?;
        let config: Self = serde_json::from_slice(&bytes)
            .context("The update verification profile is invalid.")?;
        config.key()?;
        Ok(config)
    }
    fn key(&self) -> Result<VerifyingKey> {
        let bytes: [u8; 32] = STANDARD
            .decode(&self.public_key)?
            .try_into()
            .map_err(|_| anyhow::anyhow!("Invalid update verification key."))?;
        VerifyingKey::from_bytes(&bytes).context("Invalid update verification key.")
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SignedRelease {
    pub payload: String,
    pub signature: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Release {
    pub application_id: String,
    pub version: String,
    pub platform: String,
    pub requires_schema: String,
    pub artifact: String,
    pub sha256: String,
    pub executable_sha256: String,
    pub size: u64,
    pub notes: String,
}

#[derive(Clone, Debug)]
pub struct VerifiedRelease {
    pub release: Release,
    pub signed: SignedRelease,
}

impl SignedRelease {
    pub fn verify(&self, config: &UpdateConfig) -> Result<VerifiedRelease> {
        ensure!(
            self.payload.len() <= 12000 && self.signature.len() <= 128,
            "The update manifest is too large."
        );
        let payload = STANDARD
            .decode(&self.payload)
            .context("Invalid update manifest.")?;
        let signature = Signature::try_from(STANDARD.decode(&self.signature)?.as_slice())
            .context("Invalid update signature.")?;
        config
            .key()?
            .verify_strict(&payload, &signature)
            .context("This update could not be verified. Nothing was installed.")?;
        let release: Release =
            serde_json::from_slice(&payload).context("Unsupported update manifest.")?;
        let _ = Version::parse(&release.version)?;
        ensure!(
            release.application_id == crate::APP_ID && release.platform == "windows-x64",
            "This update is for a different application or platform."
        );
        ensure!(
            release.artifact == format!("{0}/HRIS-Desktop-Update-{0}.exe", release.version),
            "Invalid update package path."
        );
        ensure!(
            release.requires_schema.len() == 14
                && release.requires_schema.bytes().all(|b| b.is_ascii_digit()),
            "Invalid required database version."
        );
        ensure!(
            release.size > 0 && release.size <= MAX_BYTES && release.notes.chars().count() <= 1200,
            "Unsupported update size or notes."
        );
        for hash in [&release.sha256, &release.executable_sha256] {
            ensure!(
                hash.len() == 64 && hash.bytes().all(|b| b.is_ascii_hexdigit()),
                "Invalid update checksum."
            );
        }
        Ok(VerifiedRelease {
            release,
            signed: self.clone(),
        })
    }
}

pub fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
pub fn verify_package(release: &Release, bytes: &[u8]) -> Result<()> {
    ensure!(
        bytes.len() as u64 == release.size && release.size <= MAX_BYTES,
        "The update download is incomplete or too large."
    );
    ensure!(
        digest(bytes).eq_ignore_ascii_case(&release.sha256),
        "The update download failed its integrity check. Nothing was installed."
    );
    ensure!(
        bytes.starts_with(b"MZ"),
        "The update is not a Windows installer."
    );
    Ok(())
}

fn read_bounded(path: &Path, limit: u64) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    fs::File::open(path)?
        .take(limit + 1)
        .read_to_end(&mut bytes)?;
    ensure!(bytes.len() as u64 <= limit, "Update file is too large.");
    Ok(bytes)
}
fn file_hash(path: &Path) -> Result<String> {
    Ok(digest(&read_bounded(path, MAX_BYTES)?))
}

fn cache_root() -> Result<PathBuf> {
    let local =
        std::env::var_os("LOCALAPPDATA").context("The user update folder is unavailable.")?;
    Ok(PathBuf::from(local).join("HRIS Desktop").join("updates"))
}
fn installation_root() -> Result<PathBuf> {
    Ok(
        PathBuf::from(
            std::env::var_os("LOCALAPPDATA").context("Installation folder unavailable.")?,
        )
        .join("Programs")
        .join("HRIS Desktop"),
    )
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct InstallPlan {
    parent_pid: u32,
    signed: SignedRelease,
    profile_hash: String,
    verification_hash: String,
    previous_executable_hash: String,
}

pub fn stage_and_launch(update: &VerifiedRelease, bytes: &[u8]) -> Result<()> {
    let installed = installation_root()?.canonicalize()?;
    ensure!(
        std::env::current_exe()?
            .parent()
            .context("Missing application folder")?
            .canonicalize()?
            == installed,
        "Automatic installation is available only in the installed desktop app."
    );
    let config = UpdateConfig::from_file(&installed.join(CONFIG_FILE))?;
    let verified = update.signed.verify(&config)?;
    ensure!(
        Version::parse(&verified.release.version)? > Version::parse(VERSION)?,
        "This version is already installed."
    );
    verify_package(&verified.release, bytes)?;
    fs::create_dir_all(cache_root()?)?;
    let staging = cache_root()?.join(uuid::Uuid::new_v4().to_string());
    fs::create_dir(&staging)?;
    fs::write(staging.join("update.exe"), bytes)?;
    fs::copy(std::env::current_exe()?, staging.join("HRIS-Updater.exe"))?;
    let plan = InstallPlan {
        parent_pid: std::process::id(),
        signed: verified.signed,
        profile_hash: file_hash(&installed.join("hris-connection.json"))?,
        verification_hash: file_hash(&installed.join(CONFIG_FILE))?,
        previous_executable_hash: file_hash(&installed.join("HRIS-Desktop.exe"))?,
    };
    fs::write(staging.join("plan.json"), serde_json::to_vec(&plan)?)?;
    let mut helper = Command::new(staging.join("HRIS-Updater.exe"));
    helper.arg("--apply-update").arg(staging.join("plan.json"));
    hide(&mut helper);
    helper
        .spawn()
        .context("Could not start the update helper. The current app was kept open.")?;
    Ok(())
}

fn hide(command: &mut Command) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    #[cfg(not(windows))]
    let _ = command;
}

#[cfg(windows)]
fn wait_for_parent(pid: u32, expected: &Path) -> Result<()> {
    use windows_sys::Win32::{
        Foundation::{CloseHandle, ERROR_INVALID_PARAMETER, GetLastError, WAIT_OBJECT_0},
        System::Threading::{
            OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_SYNCHRONIZE,
            QueryFullProcessImageNameW, WaitForSingleObject,
        },
    };
    ensure!(
        pid != std::process::id(),
        "The updater cannot wait for itself."
    );
    // SAFETY: the handle and owned UTF-16 buffer are checked, bounded and closed on every path.
    unsafe {
        let handle = OpenProcess(
            PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_SYNCHRONIZE,
            0,
            pid,
        );
        if handle.is_null() {
            ensure!(
                GetLastError() == ERROR_INVALID_PARAMETER,
                "Could not verify the application process."
            );
            return Ok(());
        }
        let checked = (|| {
            let mut buffer = vec![0u16; 32768];
            let mut size = buffer.len() as u32;
            ensure!(
                QueryFullProcessImageNameW(handle, 0, buffer.as_mut_ptr(), &mut size) != 0,
                "Could not verify the running application."
            );
            let actual =
                PathBuf::from(String::from_utf16(&buffer[..size as usize])?).canonicalize()?;
            ensure!(
                actual == expected.canonicalize()?,
                "The update request did not match this installation."
            );
            ensure!(
                WaitForSingleObject(handle, 120000) == WAIT_OBJECT_0,
                "HRIS is still open. Save your work and try the update again."
            );
            Ok(())
        })();
        CloseHandle(handle);
        checked
    }
}
#[cfg(not(windows))]
fn wait_for_parent(_: u32, _: &Path) -> Result<()> {
    anyhow::bail!("Automatic installation currently supports Windows only.")
}

pub fn apply_plan(path: &Path) -> Result<()> {
    let installed = installation_root()?;
    let cache = cache_root()?;
    let result = apply_plan_at(path, &installed, &cache);
    if result.is_err() {
        let verification = path.parent().map(|p| p.join("verification.json"));
        if !verification.as_ref().is_some_and(|p| p.exists()) {
            let _ = fs::create_dir_all(&cache);
            let _=fs::write(cache.join("result.json"),br#"{"success":false,"message":"The update could not finish. Please try again or run the company setup."}"#);
            if let Ok(bytes) = read_bounded(path, 20000) {
                if let Ok(plan) = serde_json::from_slice::<InstallPlan>(&bytes) {
                    if fs::read_to_string(installed.join(".hris-installation"))
                        .ok()
                        .as_deref()
                        == Some(crate::APP_ID)
                        && file_hash(&installed.join("HRIS-Desktop.exe"))
                            .ok()
                            .as_deref()
                            == Some(&plan.previous_executable_hash)
                        && wait_for_parent(plan.parent_pid, &installed.join("HRIS-Desktop.exe"))
                            .is_ok()
                    {
                        let _ = Command::new(installed.join("HRIS-Desktop.exe")).spawn();
                    }
                }
            }
        }
    }
    result
}

fn apply_plan_at(path: &Path, installation: &Path, cache: &Path) -> Result<()> {
    let installed = installation.canonicalize()?;
    let staging = path
        .parent()
        .context("Missing update staging folder")?
        .canonicalize()?;
    ensure!(
        staging.parent() == Some(cache.canonicalize()?.as_path()),
        "The update must be in the private update folder."
    );
    ensure!(
        fs::read_to_string(installed.join(".hris-installation"))? == crate::APP_ID,
        "This is not an HRIS installation."
    );
    let plan: InstallPlan = serde_json::from_slice(&read_bounded(path, 20000)?)?;
    let key_file = installed.join(CONFIG_FILE);
    let profile = installed.join("hris-connection.json");
    ensure!(
        file_hash(&key_file)? == plan.verification_hash
            && file_hash(&profile)? == plan.profile_hash,
        "The company configuration changed. Restart HRIS before updating."
    );
    let config = UpdateConfig::from_file(&key_file)?;
    let verified = plan.signed.verify(&config)?;
    let previous: serde_json::Value =
        serde_json::from_slice(&read_bounded(&installed.join("BUILD.json"), 1000000)?)?;
    ensure!(
        previous["application_id"] == crate::APP_ID,
        "The installed application identity did not match."
    );
    ensure!(
        Version::parse(&verified.release.version)?
            > Version::parse(
                previous["version"]
                    .as_str()
                    .context("Installed version missing")?
            )?,
        "An older or equal version cannot replace the current app."
    );
    ensure!(
        file_hash(&installed.join("HRIS-Desktop.exe"))? == plan.previous_executable_hash,
        "The installed app changed before the update."
    );
    let artifact = staging.join("update.exe");
    verify_package(&verified.release, &read_bounded(&artifact, MAX_BYTES)?)?;
    wait_for_parent(plan.parent_pid, &installed.join("HRIS-Desktop.exe"))?;
    let backup = staging.join("previous-installation");
    fs::create_dir(&backup)?;
    let mut saved = Vec::new();
    for name in
        APP_FILES
            .iter()
            .copied()
            .chain(["hris-connection.json", CONFIG_FILE, "company-logo.png"])
    {
        let file = installed.join(name);
        if file.exists() {
            ensure!(
                fs::symlink_metadata(&file)?.file_type().is_file(),
                "Unsupported installation file."
            );
            fs::copy(&file, backup.join(name))?;
            ensure!(
                file_hash(&file)? == file_hash(&backup.join(name))?,
                "Previous-version backup failed."
            );
            saved.push(name);
        }
    }
    let install = (|| {
        // Verify again after the parent exits, before executing the downloaded file.
        verify_package(&verified.release, &read_bounded(&artifact, MAX_BYTES)?)?;
        let mut setup = Command::new(&artifact);
        setup.arg("/S");
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            let target = installed
                .to_str()
                .context("Unsupported installation path")?;
            let target = target.strip_prefix(r"\\?\").unwrap_or(target);
            ensure!(
                !target.contains(['"', '\r', '\n']),
                "Unsupported installation path."
            );
            // NSIS requires /D to be the final, unquoted argument; no shell is involved.
            setup.raw_arg(format!("/D={target}"));
        }
        hide(&mut setup);
        ensure!(
            setup.status()?.success(),
            "The update setup did not finish successfully."
        );
        ensure!(
            file_hash(&installed.join("HRIS-Desktop.exe"))?
                .eq_ignore_ascii_case(&verified.release.executable_sha256),
            "The installed update failed its integrity check."
        );
        ensure!(
            file_hash(&profile)? == plan.profile_hash
                && file_hash(&key_file)? == plan.verification_hash,
            "The update changed company configuration."
        );
        if backup.join("company-logo.png").exists() {
            ensure!(
                file_hash(&installed.join("company-logo.png"))?
                    == file_hash(&backup.join("company-logo.png"))?,
                "The update changed company branding."
            );
        }
        let build: serde_json::Value =
            serde_json::from_slice(&read_bounded(&installed.join("BUILD.json"), 1000000)?)?;
        ensure!(
            build["version"] == verified.release.version
                && build["application_id"] == crate::APP_ID,
            "The installed version did not match the signed update."
        );
        Ok::<_, anyhow::Error>(())
    })();
    if let Err(error) = install {
        for name in &saved {
            fs::copy(backup.join(name), installed.join(name)).context(
                "The prior version could not be fully restored. Run the company setup again.",
            )?;
        }
        fs::write(
            cache.join("result.json"),
            serde_json::to_vec(
                &serde_json::json!({"success":false,"message":"The update failed. The previous app files were restored. Please try again."}),
            )?,
        )?;
        let restarted = Command::new(installed.join("HRIS-Desktop.exe"))
            .spawn()
            .ok();
        fs::write(
            staging.join("verification.json"),
            serde_json::to_vec(
                &serde_json::json!({"success":false,"previous_app_files_restored":true,"restarted_pid":restarted.map(|p|p.id())}),
            )?,
        )?;
        return Err(error);
    }
    fs::write(
        cache.join("result.json"),
        serde_json::to_vec(
            &serde_json::json!({"success":true,"message":format!("Updated to version {}.",verified.release.version)}),
        )?,
    )?;
    let restarted = Command::new(installed.join("HRIS-Desktop.exe"))
        .spawn()
        .context("Update installed. Open HRIS from the Start menu.")?;
    fs::write(
        staging.join("verification.json"),
        serde_json::to_vec(
            &serde_json::json!({"success":true,"version":verified.release.version,"configuration_preserved":true,"restarted_pid":restarted.id()}),
        )?,
    )?;
    Ok(())
}

pub fn previous_result() -> Option<(String, bool)> {
    let file = cache_root().ok()?.join("result.json");
    if !file.exists() {
        return None;
    }
    let value: serde_json::Value = serde_json::from_slice(&read_bounded(&file, 4096).ok()?).ok()?;
    let message = value["message"].as_str()?.chars().take(240).collect();
    let failed = value["success"].as_bool() != Some(true);
    let _ = fs::remove_file(file);
    Some((message, failed))
}

#[derive(Clone, Debug, Default)]
pub enum UpdateState {
    #[default]
    Idle,
    UpToDate,
    Available(VerifiedRelease),
    Failed(String),
    Restarting,
}

impl UpdateState {
    pub fn accept(&mut self, incoming: Self) {
        // Keep a verified offer visible across later transient failures and routine refreshes.
        if matches!(self, Self::Available(_))
            && matches!(incoming, Self::Failed(_) | Self::UpToDate)
        {
            return;
        }
        *self = incoming;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::{Signer, SigningKey};

    fn sample() -> (UpdateConfig, SignedRelease, Vec<u8>) {
        let key = SigningKey::from_bytes(&[17; 32]);
        let bytes = b"MZsynthetic update fixture".to_vec();
        let release = Release {
            application_id: crate::APP_ID.into(),
            version: "0.1.7".into(),
            platform: "windows-x64".into(),
            requires_schema: "20261021054356".into(),
            artifact: "0.1.7/HRIS-Desktop-Update-0.1.7.exe".into(),
            sha256: digest(&bytes),
            executable_sha256: "a".repeat(64),
            size: bytes.len() as u64,
            notes: "Synthetic update".into(),
        };
        let payload = serde_json::to_vec(&release).unwrap();
        (
            UpdateConfig {
                public_key: STANDARD.encode(key.verifying_key().as_bytes()),
            },
            SignedRelease {
                payload: STANDARD.encode(&payload),
                signature: STANDARD.encode(key.sign(&payload).to_bytes()),
            },
            bytes,
        )
    }

    #[test]
    fn numeric_versions_reject_ambiguous_or_path_like_input() {
        assert!(Version::parse("0.1.10").unwrap() > Version::parse("0.1.9").unwrap());
        for bad in [
            "0.01.6",
            "v0.1.6",
            "0.1",
            "0.1.6-beta",
            "../0.1.6",
            "0.1.6/evil",
            "1000000.0.0",
        ] {
            assert!(Version::parse(bad).is_err());
        }
    }
    #[test]
    fn verified_update_offer_survives_failed_checks_until_sign_out_or_restart() {
        let (config, signed, _) = sample();
        let mut state = UpdateState::Available(signed.verify(&config).unwrap());
        state.accept(UpdateState::Failed("Transient connection failure".into()));
        assert!(matches!(state, UpdateState::Available(_)));
        state.accept(UpdateState::UpToDate);
        assert!(matches!(state, UpdateState::Available(_)));
        state.accept(UpdateState::Idle);
        assert!(matches!(state, UpdateState::Idle));
    }
    #[test]
    fn signed_manifest_and_download_must_both_match() {
        let (config, signed, bytes) = sample();
        let verified = signed.verify(&config).unwrap();
        verify_package(&verified.release, &bytes).unwrap();
        assert!(verify_package(&verified.release, b"MZchanged update fixture!").is_err());
        let mut altered = signed.clone();
        altered.payload = STANDARD.encode(b"changed manifest");
        assert!(altered.verify(&config).is_err());
        let wrong = UpdateConfig {
            public_key: STANDARD
                .encode(SigningKey::from_bytes(&[18; 32]).verifying_key().as_bytes()),
        };
        assert!(signed.verify(&wrong).is_err());
    }
    #[test]
    fn even_validly_signed_updates_cannot_change_app_platform_or_artifact_path() {
        let (config, signed, _) = sample();
        let key = SigningKey::from_bytes(&[17; 32]);
        let original = signed.verify(&config).unwrap().release;
        for field in [
            "application_id",
            "platform",
            "artifact",
            "size",
            "requires_schema",
        ] {
            let mut value = serde_json::to_value(&original).unwrap();
            value[field] = if field == "size" {
                serde_json::json!(MAX_BYTES + 1)
            } else {
                serde_json::json!("../../other-app.exe")
            };
            let bytes = serde_json::to_vec(&value).unwrap();
            let signed = SignedRelease {
                payload: STANDARD.encode(&bytes),
                signature: STANDARD.encode(key.sign(&bytes).to_bytes()),
            };
            assert!(signed.verify(&config).is_err(), "{field}");
        }
    }

    #[cfg(windows)]
    #[test]
    #[ignore = "Run explicitly after preparing the Windows release update installer"]
    fn real_installer_preserves_company_files_and_recovers_from_a_failed_setup() {
        use windows_sys::Win32::{
            Foundation::CloseHandle,
            System::Threading::{OpenProcess, PROCESS_TERMINATE, TerminateProcess},
        };
        let setup_path = std::env::var_os("HRIS_TEST_UPDATE_SETUP").expect("Update setup required");
        // Check the real compiled artifact, so stale setup resources fail before any installation.
        {
            use std::os::windows::process::CommandExt;
            let metadata=Command::new("powershell.exe")
                .args(["-NoProfile","-NonInteractive","-Command",
                    "(Get-Item -LiteralPath $env:HRIS_INSTALLER_VERSION_PATH).VersionInfo.ProductVersion"])
                .env("HRIS_INSTALLER_VERSION_PATH",&setup_path)
                .creation_flags(0x08000000)
                .output().unwrap();
            assert!(metadata.status.success(), "Installer metadata query failed");
            assert_eq!(
                String::from_utf8(metadata.stdout).unwrap().trim(),
                VERSION,
                "Installer resources must match the bundled application version"
            );
        }
        let setup = fs::read(setup_path).unwrap();
        let new_exe =
            fs::read(std::env::var_os("HRIS_TEST_NEW_EXE").expect("New executable required"))
                .unwrap();
        let previous_exe = fs::read(
            std::env::var_os("HRIS_TEST_PREVIOUS_EXE").expect("Previous executable required"),
        )
        .unwrap();
        let root = std::env::current_dir()
            .unwrap()
            .join("work/updater-integration")
            .join(uuid::Uuid::new_v4().to_string());
        fs::create_dir_all(&root).unwrap();
        let key = SigningKey::from_bytes(&[29; 32]);
        for success in [true, false] {
            let case = root.join(if success { "success" } else { "recovery" });
            let installed = case.join("installation");
            let cache = case.join("cache");
            let staging = cache.join("fixture");
            fs::create_dir_all(&installed).unwrap();
            fs::create_dir_all(&staging).unwrap();
            fs::write(installed.join(".hris-installation"), crate::APP_ID).unwrap();
            fs::write(installed.join("HRIS-Desktop.exe"), &previous_exe).unwrap();
            fs::write(
                installed.join("BUILD.json"),
                serde_json::to_vec(
                    &serde_json::json!({"application_id":crate::APP_ID,"version":"0.1.5"}),
                )
                .unwrap(),
            )
            .unwrap();
            fs::write(installed.join("hris-connection.json"),br#"{"url":"https://synthetic.example.invalid","publishable_key":"sb_publishable_synthetic_fixture_only"}"#).unwrap();
            fs::write(
                installed.join(CONFIG_FILE),
                serde_json::to_vec(&UpdateConfig {
                    public_key: STANDARD.encode(key.verifying_key().as_bytes()),
                })
                .unwrap(),
            )
            .unwrap();
            fs::write(
                installed.join("company-logo.png"),
                b"synthetic branding fixture",
            )
            .unwrap();
            fs::write(
                installed.join("keep-unknown.txt"),
                b"preserve unrelated file",
            )
            .unwrap();
            let package = if success {
                setup.clone()
            } else {
                b"MZdeliberately invalid signed installer fixture".to_vec()
            };
            let release = Release {
                application_id: crate::APP_ID.into(),
                version: VERSION.into(),
                platform: "windows-x64".into(),
                requires_schema: "20261021054356".into(),
                artifact: format!("{0}/HRIS-Desktop-Update-{0}.exe", VERSION),
                sha256: digest(&package),
                executable_sha256: digest(&new_exe),
                size: package.len() as u64,
                notes: "Isolated installer test".into(),
            };
            let payload = serde_json::to_vec(&release).unwrap();
            let signed = SignedRelease {
                payload: STANDARD.encode(&payload),
                signature: STANDARD.encode(key.sign(&payload).to_bytes()),
            };
            fs::write(staging.join("update.exe"), package).unwrap();
            let plan = InstallPlan {
                parent_pid: 0,
                signed,
                profile_hash: file_hash(&installed.join("hris-connection.json")).unwrap(),
                verification_hash: file_hash(&installed.join(CONFIG_FILE)).unwrap(),
                previous_executable_hash: digest(&previous_exe),
            };
            fs::write(
                staging.join("plan.json"),
                serde_json::to_vec(&plan).unwrap(),
            )
            .unwrap();
            let applied = apply_plan_at(&staging.join("plan.json"), &installed, &cache);
            assert_eq!(applied.is_ok(), success, "{applied:?}");
            assert_eq!(
                file_hash(&installed.join("hris-connection.json")).unwrap(),
                plan.profile_hash
            );
            assert_eq!(
                file_hash(&installed.join(CONFIG_FILE)).unwrap(),
                plan.verification_hash
            );
            assert_eq!(
                fs::read(installed.join("company-logo.png")).unwrap(),
                b"synthetic branding fixture"
            );
            assert_eq!(
                fs::read(installed.join("keep-unknown.txt")).unwrap(),
                b"preserve unrelated file"
            );
            assert_eq!(
                file_hash(&installed.join("HRIS-Desktop.exe")).unwrap(),
                if success {
                    digest(&new_exe)
                } else {
                    digest(&previous_exe)
                }
            );
            if success {
                // Check the delivered inventory, including newly added supporting files.
                let sums = fs::read_to_string(installed.join("SHA256SUMS.txt")).unwrap();
                for line in sums.lines() {
                    let (expected, name) = line.split_once("  ").expect("Payload checksum entry");
                    assert!(!name.is_empty() && !name.contains(['/', '\\']) && name != "..");
                    assert_eq!(
                        file_hash(&installed.join(name)).unwrap(),
                        expected,
                        "Missing or stale installed payload: {name}"
                    );
                }
            }
            let evidence: serde_json::Value =
                serde_json::from_slice(&fs::read(staging.join("verification.json")).unwrap())
                    .unwrap();
            if let Some(pid) = evidence["restarted_pid"].as_u64() {
                // Only a child created by this isolated test is stopped; never the user's installed app.
                unsafe {
                    let handle = OpenProcess(PROCESS_TERMINATE, 0, pid as u32);
                    if !handle.is_null() {
                        TerminateProcess(handle, 0);
                        CloseHandle(handle);
                    }
                }
            }
        }
        println!(
            "Verified real silent update, company profile/key/logo/unknown-file preservation, and failed-installer recovery in isolated installations."
        );
    }
}
