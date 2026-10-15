//! Signed portable updates. Private signing material is never part of the app.
use crate::Result;
use base64::{Engine, engine::general_purpose::STANDARD};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
    time::Duration,
};

pub const PUBLIC_FEED: &str =
    "https://github.com/Rubber-Ducks-Development/duckworkshop/releases/latest/download/latest.json";
pub const PUBLIC_KEY: &[u8] = include_bytes!("../assets/UpdatePublicKey.der");
pub const MANIFEST_LIMIT: u64 = 131_072;
const DOWNLOAD_LIMIT: u64 = 256 * 1024 * 1024;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "PascalCase", deny_unknown_fields)]
pub struct Release {
    pub product: String,
    pub version: String,
    pub download_url: String,
    pub sha256: String,
    pub size: u64,
    pub notes: String,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "PascalCase", deny_unknown_fields)]
pub struct Envelope {
    pub payload: String,
    pub signature: String,
}

pub struct VerifiedRelease {
    pub release: Release,
    envelope: Vec<u8>,
}

pub struct StagedUpdate {
    pub release: Release,
    folder: tempfile::TempDir,
}

pub fn https_url(value: &str) -> Result<url::Url> {
    let url = url::Url::parse(value).map_err(|_| "Use a complete HTTPS update URL.")?;
    if url.scheme() != "https"
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.fragment().is_some()
    {
        return Err("Update URLs must use HTTPS without credentials or fragments.".into());
    }
    Ok(url)
}

pub fn verify_manifest(bytes: &[u8], public_key: &[u8]) -> Result<Release> {
    if bytes.len() as u64 > MANIFEST_LIMIT {
        return Err("Update manifest is too large.".into());
    }
    let envelope: Envelope =
        serde_json::from_slice(bytes).map_err(|_| "Invalid signed update envelope.")?;
    let payload = STANDARD
        .decode(envelope.payload)
        .map_err(|_| "Invalid update payload encoding.")?;
    let signature = STANDARD
        .decode(envelope.signature)
        .map_err(|_| "Invalid update signature encoding.")?;
    ring::signature::UnparsedPublicKey::new(&ring::signature::RSA_PSS_2048_8192_SHA256, public_key)
        .verify(&payload, &signature)
        .map_err(|_| "Update signature does not match Duck Workshop's release key.")?;
    let release: Release =
        serde_json::from_slice(&payload).map_err(|_| "Invalid signed release details.")?;
    let version =
        semver::Version::parse(&release.version).map_err(|_| "Invalid update version.")?;
    if release.product != "duck-workshop"
        || !version.pre.is_empty()
        || !version.build.is_empty()
        || release.size == 0
        || release.size > DOWNLOAD_LIMIT
        || release.sha256.len() != 64
        || !release.sha256.bytes().all(|b| b.is_ascii_hexdigit())
        || release.notes.len() > 16_384
    {
        return Err("Update product, version, size or checksum is invalid.".into());
    }
    https_url(&release.download_url)?;
    Ok(release)
}

pub fn newer(version: &str, current: &str) -> Result<bool> {
    Ok(semver::Version::parse(version).map_err(|e| e.to_string())?
        > semver::Version::parse(current).map_err(|e| e.to_string())?)
}

// Disable implicit redirects so every destination is checked before any request.
fn open_https(location: &str, timeout: Duration) -> Result<ureq::http::Response<ureq::Body>> {
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .max_redirects(0)
        .timeout_global(Some(timeout))
        .build()
        .into();
    let mut url = https_url(location)?;
    for _ in 0..=8 {
        let response = agent
            .get(url.as_str())
            .header("User-Agent", "DuckWorkshop-Updater")
            .call()
            .map_err(|e| {
                format!("Could not access updates: {e}. Check that releases are public.")
            })?;
        if response.status().is_redirection() {
            let next = response
                .headers()
                .get("location")
                .and_then(|v| v.to_str().ok())
                .ok_or("Update redirect is missing its location.")?;
            url = https_url(url.join(next).map_err(|e| e.to_string())?.as_str())?;
        } else if response.status().is_success() {
            return Ok(response);
        } else {
            return Err(format!(
                "Unexpected update HTTP status: {}",
                response.status()
            ));
        }
    }
    Err("Too many update redirects.".into())
}

pub fn check(feed: &str) -> Result<Option<VerifiedRelease>> {
    let mut response = open_https(feed, Duration::from_secs(20))?;
    let mut bytes = Vec::new();
    response
        .body_mut()
        .as_reader()
        .take(MANIFEST_LIMIT + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    let release = verify_manifest(&bytes, PUBLIC_KEY)?;
    if newer(&release.version, env!("CARGO_PKG_VERSION"))? {
        Ok(Some(VerifiedRelease {
            release,
            envelope: bytes,
        }))
    } else {
        Ok(None)
    }
}

pub fn verify_file(path: &Path, release: &Release) -> Result<()> {
    let mut file = fs::File::open(path).map_err(|e| e.to_string())?;
    if file.metadata().map_err(|e| e.to_string())?.len() != release.size {
        return Err("Update file size does not match the signed release.".into());
    }
    let mut hash = Sha256::new();
    let mut total = 0u64;
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let count = file.read(&mut buffer).map_err(|e| e.to_string())?;
        if count == 0 {
            break;
        }
        total += count as u64;
        if total > release.size {
            return Err("Update file exceeds its signed size.".into());
        }
        hash.update(&buffer[..count]);
    }
    if total != release.size
        || !format!("{:x}", hash.finalize()).eq_ignore_ascii_case(&release.sha256)
    {
        return Err("Update checksum does not match the signed release.".into());
    }
    Ok(())
}

pub fn download(verified: VerifiedRelease, target: &Path) -> Result<StagedUpdate> {
    let parent = target.parent().ok_or("Application folder is missing.")?;
    let folder = tempfile::Builder::new()
        .prefix(".duck-workshop-update-")
        .tempdir_in(parent)
        .map_err(|e| {
            format!("Cannot write beside the app: {e}. Move Duck Workshop to a writable folder.")
        })?;
    let mut response = open_https(&verified.release.download_url, Duration::from_secs(15 * 60))?;
    let mut file = fs::File::create(folder.path().join("update.exe")).map_err(|e| e.to_string())?;
    let written = std::io::copy(
        &mut response
            .body_mut()
            .as_reader()
            .take(verified.release.size + 1),
        &mut file,
    )
    .map_err(|e| e.to_string())?;
    file.flush().map_err(|e| e.to_string())?;
    file.sync_all().map_err(|e| e.to_string())?;
    drop(file);
    if written != verified.release.size {
        return Err("Update download has an incorrect size.".into());
    }
    verify_file(&folder.path().join("update.exe"), &verified.release)?;
    fs::write(folder.path().join("latest.json"), verified.envelope).map_err(|e| e.to_string())?;
    Ok(StagedUpdate {
        release: verified.release,
        folder,
    })
}

impl StagedUpdate {
    pub fn launch(self, target: &Path) -> Result<()> {
        verify_file(&self.folder.path().join("update.exe"), &self.release)?;
        let helper = self.folder.path().join("apply.exe");
        fs::copy(std::env::current_exe().map_err(|e| e.to_string())?, &helper)
            .map_err(|e| e.to_string())?;
        let mut command = std::process::Command::new(helper);
        command
            .arg("--apply-update")
            .arg(target)
            .arg(std::process::id().to_string());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x08000000);
        }
        command
            .spawn()
            .map_err(|e| format!("Could not start update helper: {e}"))?;
        let _ = self.folder.keep();
        Ok(())
    }
}

/// Run from a copy of the currently trusted app, after the editor has saved and closed.
pub fn apply_staged(folder: &Path, target: &Path) -> Result<()> {
    if !target.is_absolute()
        || target
            .extension()
            .is_none_or(|e| !e.eq_ignore_ascii_case("exe"))
        || folder.parent() != target.parent()
    {
        return Err("Invalid portable update destination.".into());
    }
    let bytes = fs::read(folder.join("latest.json")).map_err(|e| e.to_string())?;
    let release = verify_manifest(&bytes, PUBLIC_KEY)?;
    if !newer(&release.version, env!("CARGO_PKG_VERSION"))? {
        return Err("Update is not newer than this app.".into());
    }
    let source = folder.join("update.exe");
    verify_file(&source, &release)?;
    let backup = folder.join("previous.exe");
    // Windows refuses to rename the running executable. Retry until the old editor exits.
    let mut renamed = false;
    for _ in 0..240 {
        if fs::rename(target, &backup).is_ok() {
            renamed = true;
            break;
        }
        std::thread::sleep(Duration::from_millis(250));
    }
    if !renamed {
        return Err(
            "App did not close or its folder is not writable. Original app was kept.".into(),
        );
    }
    if let Err(error) = fs::rename(&source, target) {
        let rollback = fs::rename(&backup, target);
        return Err(format!(
            "Could not install update: {error}; rollback: {rollback:?}. Backup: {}",
            backup.display()
        ));
    }
    // Recheck the final file; on any launch failure restore the old executable.
    let launch = verify_file(target, &release).and_then(|_| {
        std::process::Command::new(target)
            .current_dir(target.parent().unwrap())
            .spawn()
            .map(|_| ())
            .map_err(|e| e.to_string())
    });
    if let Err(error) = launch {
        let _ = fs::rename(target, &source);
        let rollback = fs::rename(&backup, target);
        if rollback.is_ok() {
            let _ = std::process::Command::new(target).spawn();
        }
        return Err(format!("Update failed: {error}; rollback: {rollback:?}"));
    }
    // Keep previous.exe for manual recovery. The updater is never elevated.
    Ok(())
}

pub fn helper_entry(target: PathBuf, parent_pid: u32) {
    let result = wait_for_editor(parent_pid)
        .and_then(|_| std::env::current_exe().map_err(|e| e.to_string()))
        .and_then(|exe| {
            let folder = exe.parent().ok_or("Update staging folder is missing.")?;
            apply_staged(folder, &target).inspect_err(|message| {
                let _ = fs::write(folder.join("update-error.txt"), message);
            })
        });
    if let Err(error) = result {
        rfd::MessageDialog::new()
            .set_title("Duck Workshop update failed")
            .set_description(error)
            .set_level(rfd::MessageLevel::Error)
            .show();
    }
}

#[cfg(windows)]
fn wait_for_editor(pid: u32) -> Result<()> {
    use windows_sys::Win32::{
        Foundation::{CloseHandle, WAIT_OBJECT_0},
        System::Threading::{OpenProcess, PROCESS_SYNCHRONIZE, WaitForSingleObject},
    };
    // A handle pins this process identity and avoids PID reuse during the wait.
    unsafe {
        let handle = OpenProcess(PROCESS_SYNCHRONIZE, 0, pid);
        if handle.is_null() {
            // ERROR_INVALID_PARAMETER means the editor has already exited.
            return if std::io::Error::last_os_error().raw_os_error() == Some(87) {
                Ok(())
            } else {
                Err("Could not wait for the editor to close.".into())
            };
        }
        let status = WaitForSingleObject(handle, 60_000);
        CloseHandle(handle);
        if status == WAIT_OBJECT_0 {
            Ok(())
        } else {
            Err("Editor did not close. Update was cancelled.".into())
        }
    }
}

#[cfg(not(windows))]
fn wait_for_editor(_pid: u32) -> Result<()> {
    Err("Portable updates currently support Windows only.".into())
}
