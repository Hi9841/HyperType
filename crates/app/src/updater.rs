//! Native updater for the GPUI HyperType app.
//!
//! Matches `tauri-plugin-updater` 2.11.0 against the same GitHub
//! `latest.json` feed and the same per-user NSIS installer.

// UI agents will call this API; keep items live until that wiring lands.
#![allow(dead_code)]

use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::Duration;

use base64::Engine;
use minisign_verify::{PublicKey, Signature};
use semver::Version;
use serde::Deserialize;

/// Same feed as `src-tauri/tauri.conf.json` plugins.updater.endpoints.
const FEED_URL: &str =
    "https://github.com/Hi9841/HyperType/releases/latest/download/latest.json";

/// Base64 of the minisign public key file from `tauri.conf.json` plugins.updater.pubkey.
const PUBKEY_B64: &str = "dW50cnVzdGVkIGNvbW1lbnQ6IG1pbmlzaWduIHB1YmxpYyBrZXk6IDMxOUREQzQ1ODA1MDVCNApSV1MwQlFWWXhOMFpBd2dydTZQbXhlTXJmak9BanYxWnMxNXJiT1Q2VjRKSkVZWTh0cDF3RDltbAo=";

/// Prefer the NSIS key the UI requests, then Tauri's `{os}-{arch}` fallback
/// (updater.rs get_urls, lines 618–626).
const TARGET_NSIS: &str = "windows-x86_64-nsis";
const TARGET_FALLBACK: &str = "windows-x86_64";

const CHECK_TIMEOUT: Duration = Duration::from_secs(15);
const DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(300);
const USER_AGENT: &str = concat!(env!("CARGO_PKG_NAME"), "/", env!("CARGO_PKG_VERSION"));

pub struct UpdateInfo {
    pub version: String,
    pub body: Option<String>,
    url: String,
    signature: String,
}

impl UpdateInfo {
    /// Feed signature string (base64 of the minisign `.sig` file). For probes.
    pub fn signature(&self) -> &str {
        &self.signature
    }
}

pub enum Progress {
    Started { content_length: Option<u64> },
    Chunk(usize),
    Finished,
}

/// Blocking. 15 s timeout. Target `windows-x86_64-nsis` with Tauri fallbacks.
/// `Ok(None)` when the feed version is not newer than `CARGO_PKG_VERSION`.
pub fn check() -> Result<Option<UpdateInfo>, String> {
    check_with(env!("CARGO_PKG_VERSION"))
}

/// Blocking. Same as [`check`], with an explicit current version string.
pub fn check_with(current_version: &str) -> Result<Option<UpdateInfo>, String> {
    let current = parse_version(current_version)?;
    let feed = fetch_feed()?;
    let remote = parse_version(&feed.version)?;

    // tauri-plugin-updater updater.rs:578 — release.version > current_version
    if remote <= current {
        return Ok(None);
    }

    let platform = resolve_platform(&feed.platforms)?;
    Ok(Some(UpdateInfo {
        version: feed.version,
        body: feed.notes,
        url: platform.url.clone(),
        signature: platform.signature.clone(),
    }))
}

/// Blocking. Download, verify minisign, write the installer to a temp file,
/// launch it in passive mode like Tauri, then exit the process.
pub fn download_and_install(
    info: &UpdateInfo,
    on_progress: impl FnMut(Progress),
) -> Result<(), String> {
    let path = download_verified_to_temp(info, on_progress)?;
    launch_nsis_passive(&path)?;
    // tauri-plugin-updater updater.rs:876 — exit after ShellExecute succeeds
    std::process::exit(0);
}

/// Restart HyperType like `tauri-plugin-process` relaunch / `request_restart`.
pub fn relaunch() -> Result<(), String> {
    let exe = std::env::current_exe().map_err(|e| format!("relaunch exe: {e}"))?;
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    std::process::Command::new(&exe)
        .args(args)
        .spawn()
        .map_err(|e| format!("relaunch spawn failed: {e}"))?;
    std::process::exit(0);
}

/// Download + verify + write temp installer. Does not launch. Used by
/// `--update-probe` and by [`download_and_install`].
pub fn download_verified_to_temp(
    info: &UpdateInfo,
    mut on_progress: impl FnMut(Progress),
) -> Result<PathBuf, String> {
    let bytes = download_bytes(info, &mut on_progress)?;
    verify_signature(&bytes, &info.signature)?;
    write_installer_temp(&bytes, &info.version)
}

/// Verify raw bytes against the embedded pubkey and a feed signature.
/// Exposed for the probe tamper path.
pub fn verify_signature(data: &[u8], release_signature: &str) -> Result<(), String> {
    // tauri-plugin-updater updater.rs:1524–1533
    let pub_key_decoded = b64_to_string(PUBKEY_B64)?;
    let public_key =
        PublicKey::decode(&pub_key_decoded).map_err(|e| format!("pubkey decode: {e}"))?;
    let signature_decoded = b64_to_string(release_signature)?;
    let signature =
        Signature::decode(&signature_decoded).map_err(|e| format!("sig decode: {e}"))?;
    public_key
        .verify(data, &signature, true)
        .map_err(|e| format!("sig mismatch: {e}"))?;
    Ok(())
}

// --- feed -------------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct Feed {
    version: String,
    notes: Option<String>,
    platforms: std::collections::HashMap<String, Platform>,
}

#[derive(Debug, Deserialize)]
struct Platform {
    url: String,
    signature: String,
}

fn fetch_feed() -> Result<Feed, String> {
    let agent = ureq::AgentBuilder::new()
        .timeout_connect(CHECK_TIMEOUT)
        .timeout_read(CHECK_TIMEOUT)
        .user_agent(USER_AGENT)
        .build();

    let response = agent
        .get(FEED_URL)
        .set("Accept", "application/json")
        .call()
        .map_err(|e| format!("feed fetch failed: {e}"))?;

    if !(200..300).contains(&response.status()) {
        return Err(format!("feed HTTP {}", response.status()));
    }

    let body = response
        .into_string()
        .map_err(|e| format!("feed body read: {e}"))?;
    serde_json::from_str(&body).map_err(|e| format!("feed JSON: {e}"))
}

fn resolve_platform(
    platforms: &std::collections::HashMap<String, Platform>,
) -> Result<&Platform, String> {
    if let Some(p) = platforms.get(TARGET_NSIS) {
        return Ok(p);
    }
    if let Some(p) = platforms.get(TARGET_FALLBACK) {
        return Ok(p);
    }
    Err(format!("no target {TARGET_NSIS}"))
}

fn parse_version(raw: &str) -> Result<Version, String> {
    Version::parse(raw.trim().trim_start_matches('v'))
        .map_err(|e| format!("bad version '{raw}': {e}"))
}

// --- download ---------------------------------------------------------------

fn download_bytes(
    info: &UpdateInfo,
    on_progress: &mut dyn FnMut(Progress),
) -> Result<Vec<u8>, String> {
    let agent = ureq::AgentBuilder::new()
        .timeout_connect(CHECK_TIMEOUT)
        .timeout_read(DOWNLOAD_TIMEOUT)
        .user_agent(USER_AGENT)
        .build();

    let response = agent
        .get(&info.url)
        .set("Accept", "application/octet-stream")
        .call()
        .map_err(|e| format!("download failed: {e}"))?;

    if !(200..300).contains(&response.status()) {
        return Err(format!("download HTTP {}", response.status()));
    }

    let content_length = response
        .header("Content-Length")
        .and_then(|v| v.parse::<u64>().ok());
    on_progress(Progress::Started { content_length });

    let mut reader = response.into_reader();
    let mut buffer = Vec::with_capacity(content_length.unwrap_or(0) as usize);
    let mut chunk = [0u8; 64 * 1024];
    loop {
        let n = reader
            .read(&mut chunk)
            .map_err(|e| format!("download read: {e}"))?;
        if n == 0 {
            break;
        }
        on_progress(Progress::Chunk(n));
        buffer.extend_from_slice(&chunk[..n]);
    }
    on_progress(Progress::Finished);
    Ok(buffer)
}

fn write_installer_temp(bytes: &[u8], version: &str) -> Result<PathBuf, String> {
    // Named like Tauri: `{app}-{version}-updater-` / `{app}-{version}-installer.exe`
    // (updater.rs:956–1015). Keep the file; install exits without Drop cleanup.
    let dir = std::env::temp_dir().join(format!("HyperType-{version}-updater"));
    std::fs::create_dir_all(&dir).map_err(|e| format!("temp dir: {e}"))?;
    let path = dir.join(format!("HyperType-{version}-installer.exe"));
    std::fs::write(&path, bytes).map_err(|e| format!("temp write: {e}"))?;
    Ok(path)
}

// --- Windows NSIS launch ----------------------------------------------------

#[cfg(windows)]
fn launch_nsis_passive(installer: &Path) -> Result<(), String> {
    use std::ffi::OsStr;

    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::Shell::ShellExecuteW;
    use windows::Win32::UI::WindowsAndMessaging::SW_SHOW;
    use windows::core::{PCWSTR, w};

    // Passive mode from tauri.conf.json plugins.updater.windows.installMode.
    // nsis_args: /P ; always /UPDATE ; restart_after_install: /R /ARGS …
    // (config.rs:41–57, updater.rs:884–908).
    let mut params = String::from("/P /UPDATE /R /ARGS");
    for arg in std::env::args_os().skip(1) {
        params.push(' ');
        params.push_str(&escape_nsis_arg(&arg));
    }

    let file = encode_wide(installer.as_os_str());
    let parameters = encode_wide(OsStr::new(&params));

    let result = unsafe {
        ShellExecuteW(
            HWND::default(),
            w!("open"),
            PCWSTR(file.as_ptr()),
            PCWSTR(parameters.as_ptr()),
            PCWSTR::null(),
            SW_SHOW,
        )
    };
    // ShellExecuteW returns > 32 on success (updater.rs:872–874).
    if (result.0 as isize) <= 32 {
        return Err(format!(
            "installer launch failed: {}",
            std::io::Error::last_os_error()
        ));
    }
    Ok(())
}

#[cfg(not(windows))]
fn launch_nsis_passive(_installer: &Path) -> Result<(), String> {
    Err("NSIS install is Windows-only".into())
}

/// Escape one arg for NSIS `/ARGS` (updater.rs:1572–1608, simplified for UTF-8).
fn escape_nsis_arg(arg: &std::ffi::OsStr) -> String {
    let s = arg.to_string_lossy();
    let needs_quote = s.is_empty()
        || s.chars()
            .any(|c| c == ' ' || c == '\t' || c == '/');
    if !needs_quote {
        return s.into_owned();
    }
    let mut out = String::from("\"");
    let mut backslashes = 0usize;
    for c in s.chars() {
        if c == '\\' {
            backslashes += 1;
            out.push(c);
            continue;
        }
        if c == '"' {
            for _ in 0..=backslashes {
                out.push('\\');
            }
        }
        backslashes = 0;
        out.push(c);
    }
    for _ in 0..backslashes {
        out.push('\\');
    }
    out.push('"');
    out
}

#[cfg(windows)]
fn encode_wide(string: &std::ffi::OsStr) -> Vec<u16> {
    use std::os::windows::ffi::OsStrExt;
    string.encode_wide().chain(std::iter::once(0)).collect()
}

fn b64_to_string(b64: &str) -> Result<String, String> {
    let decoded = base64::engine::general_purpose::STANDARD
        .decode(b64)
        .map_err(|e| format!("b64 decode: {e}"))?;
    String::from_utf8(decoded).map_err(|_| "b64 not utf8".into())
}
