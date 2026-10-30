//! Native updater for the GPUI HyperType app.
//!
//! Matches `tauri-plugin-updater` 2.11.0 against the same GitHub
//! `latest.json` feed and the same per-user NSIS installer.

use std::ffi::{OsStr, OsString};
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
const MAX_DOWNLOAD_BYTES: usize = 512 * 1024 * 1024;
const MAX_REDIRECTS: u32 = 10;
const USER_AGENT: &str = concat!(env!("CARGO_PKG_NAME"), "/", env!("CARGO_PKG_VERSION"));
const PKG_NAME: &str = env!("CARGO_PKG_NAME");

/// Public API for the future UI update flow.
#[allow(dead_code)] // Public API for the future UI update flow.
pub struct UpdateInfo {
    pub version: String,
    pub body: Option<String>,
    url: String,
    signature: String,
}

impl UpdateInfo {
    /// Feed signature string (base64 of the minisign `.sig` file). For probes.
    #[allow(dead_code)] // Public API for the future UI update flow.
    pub fn signature(&self) -> &str {
        &self.signature
    }
}

/// Public API for the future UI update flow.
#[allow(dead_code)] // Public API for the future UI update flow.
pub enum Progress {
    Started { content_length: Option<u64> },
    Chunk(usize),
    Finished,
}

/// Blocking. Must run on a worker thread (never the GPUI UI thread).
///
/// 15 s total timeout via ureq `.timeout`. Target `windows-x86_64-nsis`
/// with Tauri fallbacks. `Ok(None)` when the feed version is not newer
/// than `CARGO_PKG_VERSION`.
#[allow(dead_code)] // Public API for the future UI update flow.
pub fn check() -> Result<Option<UpdateInfo>, String> {
    check_with(env!("CARGO_PKG_VERSION"))
}

/// Blocking. Must run on a worker thread (never the GPUI UI thread).
///
/// Same as [`check`], with an explicit current version string.
#[allow(dead_code)] // Public API for the future UI update flow.
pub fn check_with(current_version: &str) -> Result<Option<UpdateInfo>, String> {
    let current = parse_version(current_version)?;
    let url = feed_url();
    let agent = build_agent(AgentKind::Check, &url);

    let response = match agent
        .get(&url)
        .set("Accept", "application/json")
        .call()
    {
        Ok(r) => r,
        Err(e) => return Err(map_ureq_err("update check failed", e)),
    };

    // Tauri updater.rs:530–533 — HTTP 204 means no update.
    if response.status() == 204 {
        return Ok(None);
    }

    if !(200..300).contains(&response.status()) {
        return Err(format!("update check failed: HTTP {}", response.status()));
    }

    let body = response
        .into_string()
        .map_err(|_| "update check failed: I/O error".to_string())?;
    let feed = parse_feed_json(&body)?;

    // Resolve platform BEFORE version comparison (Tauri updater.rs:576–582).
    let platform = resolve_platform(&feed)?;

    let remote = parse_version(&feed.version)?;
    // tauri-plugin-updater updater.rs:578 — release.version > current_version
    if remote <= current {
        return Ok(None);
    }

    Ok(Some(UpdateInfo {
        // Semver Display has no leading `v` (Tauri updater.rs:589).
        version: remote.to_string(),
        body: feed.notes,
        url: platform.url,
        signature: platform.signature,
    }))
}

/// Blocking. Must run on a worker thread (never the GPUI UI thread).
///
/// Download, verify minisign, write the installer to a temp file, launch it
/// in passive mode like Tauri, then exit the process.
#[allow(dead_code)] // Public API for the future UI update flow.
pub fn download_and_install(
    info: &UpdateInfo,
    on_progress: impl FnMut(Progress),
) -> Result<(), String> {
    let path = download_verified_to_temp(info, on_progress)?;
    match launch_nsis_passive(&path) {
        Ok(()) => {
            // tauri-plugin-updater updater.rs:876 — exit after ShellExecute succeeds
            std::process::exit(0);
        }
        Err(e) => {
            let _ = std::fs::remove_file(&path);
            if let Some(dir) = path.parent() {
                let _ = std::fs::remove_dir_all(dir);
            }
            Err(e)
        }
    }
}

/// Restart HyperType like `tauri::process::restart` (tauri-2.11.5 process.rs:77–88).
///
/// Blocking. Must run on a worker thread (never the GPUI UI thread). Ends the process
/// with `exit(0)` even when spawn fails.
///
/// Callers MUST release the single-instance guard and stop the keyboard hook first,
/// otherwise the new process sees the old instance and exits as secondary.
#[allow(dead_code)] // Public API for the future UI update flow.
pub fn relaunch() -> Result<(), String> {
    match std::env::current_exe() {
        Ok(exe) => {
            let args: Vec<_> = std::env::args_os().skip(1).collect();
            if let Err(e) = std::process::Command::new(&exe).args(args).spawn() {
                eprintln!("relaunch spawn failed: {e}");
            }
        }
        Err(e) => {
            eprintln!("relaunch exe: {e}");
        }
    }
    std::process::exit(0);
}

/// Download + verify + write temp installer. Does not launch. Used by
/// `--update-probe` and by [`download_and_install`].
#[allow(dead_code)] // Public API for the future UI update flow.
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
#[allow(dead_code)] // Public API for the future UI update flow.
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

struct Feed {
    version: String,
    notes: Option<String>,
    platforms: Option<std::collections::HashMap<String, Platform>>,
    /// Dynamic document shape: top-level url + signature when `platforms` is absent.
    dynamic: Option<Platform>,
}

#[derive(Debug, Clone, Deserialize)]
struct Platform {
    url: String,
    signature: String,
}

#[derive(Debug, Deserialize)]
struct RawFeed {
    #[serde(alias = "name")]
    version: Option<String>,
    notes: Option<String>,
    pub_date: Option<String>,
    platforms: Option<std::collections::HashMap<String, Platform>>,
    url: Option<String>,
    signature: Option<String>,
}

fn feed_url() -> String {
    // Probe / local checks can point at a throwaway HTTP server.
    #[cfg(feature = "update-probe")]
    if let Ok(url) = std::env::var("HYPERTYPE_UPDATER_FEED_URL")
        && !url.is_empty()
    {
        return url;
    }
    FEED_URL.to_string()
}

fn parse_feed_json(body: &str) -> Result<Feed, String> {
    let raw: RawFeed = serde_json::from_str(body).map_err(|e| format!("feed JSON: {e}"))?;

    // Tauri updater.rs:1473–1476 — invalid pub_date fails the check.
    if let Some(date) = &raw.pub_date {
        chrono::DateTime::parse_from_rfc3339(date)
            .map_err(|e| format!("invalid pub_date: {e}"))?;
    }

    let version = raw
        .version
        .ok_or_else(|| "feed JSON: missing version/name".to_string())?;

    let dynamic = match (&raw.platforms, &raw.url, &raw.signature) {
        (Some(_), _, _) => None,
        (None, Some(url), Some(signature)) => Some(Platform {
            url: url.clone(),
            signature: signature.clone(),
        }),
        (None, _, _) => {
            return Err("feed JSON: platforms missing and url/signature not set".into());
        }
    };

    Ok(Feed {
        version,
        notes: raw.notes,
        platforms: raw.platforms,
        dynamic,
    })
}

fn resolve_platform(feed: &Feed) -> Result<Platform, String> {
    if let Some(dynamic) = &feed.dynamic {
        return Ok(dynamic.clone());
    }
    let platforms = feed
        .platforms
        .as_ref()
        .ok_or_else(|| format!("no update for {TARGET_FALLBACK}"))?;
    if let Some(p) = platforms.get(TARGET_NSIS) {
        return Ok(p.clone());
    }
    if let Some(p) = platforms.get(TARGET_FALLBACK) {
        return Ok(p.clone());
    }
    // Tauri TargetNotFound — coordinator wording.
    Err(format!("no update for {TARGET_FALLBACK}"))
}

fn parse_version(raw: &str) -> Result<Version, String> {
    // Tauri updater.rs:1520 — only strip a leading `v`, no other trim.
    Version::parse(raw.trim_start_matches('v')).map_err(|e| format!("bad version '{raw}': {e}"))
}

// --- HTTP agent -------------------------------------------------------------

enum AgentKind {
    Check,
    Download,
}

fn build_agent(kind: AgentKind, target_url: &str) -> ureq::Agent {
    let mut builder = ureq::AgentBuilder::new()
        .user_agent(USER_AGENT)
        .redirects(MAX_REDIRECTS);

    match kind {
        AgentKind::Check => {
            // 15 s total covers DNS/redirects/body (Tauri updater.rs:504–505).
            // ureq cannot abort a stuck DNS lookup.
            builder = builder.timeout(CHECK_TIMEOUT);
        }
        AgentKind::Download => {
            // Tauri stores `timeout: None` on download (updater.rs:595).
            // ureq defaults to a 30 s connect timeout and does not require a
            // custom one; we keep that default and set no read/total timeout.
        }
    }

    if let Some(proxy) = resolve_proxy(target_url) {
        builder = builder.proxy(proxy);
    }

    builder.build()
}

/// System proxy like Tauri's default `system-proxy` feature.
///
/// Reads the Windows per-user Internet Settings proxy, then falls back to
/// `HTTPS_PROXY` / `HTTP_PROXY`. PAC / WinHTTP auto-detect scripts are out of scope.
fn resolve_proxy(target_url: &str) -> Option<ureq::Proxy> {
    #[cfg(windows)]
    if let Some(proxy) = windows_user_proxy(target_url) {
        return Some(proxy);
    }
    env_proxy()
}

fn env_proxy() -> Option<ureq::Proxy> {
    for key in ["HTTPS_PROXY", "https_proxy", "HTTP_PROXY", "http_proxy"] {
        if let Ok(val) = std::env::var(key) {
            if val.is_empty() {
                continue;
            }
            if let Ok(proxy) = ureq::Proxy::new(&val) {
                return Some(proxy);
            }
        }
    }
    None
}

#[cfg(windows)]
fn windows_user_proxy(target_url: &str) -> Option<ureq::Proxy> {
    use windows::Win32::System::Registry::{
        HKEY, HKEY_CURRENT_USER, KEY_QUERY_VALUE, RegCloseKey, RegOpenKeyExW,
    };
    use windows::core::w;

    // PAC / WinHTTP auto-detect scripts are out of scope.

    let mut key = HKEY::default();
    if unsafe {
        RegOpenKeyExW(
            HKEY_CURRENT_USER,
            w!("Software\\Microsoft\\Windows\\CurrentVersion\\Internet Settings"),
            0,
            KEY_QUERY_VALUE,
            &mut key,
        )
    }
    .is_err()
    {
        return None;
    }

    let enable = read_reg_dword(key, w!("ProxyEnable")).unwrap_or(0);
    if enable == 0 {
        let _ = unsafe { RegCloseKey(key) };
        return None;
    }

    let server = match read_reg_sz(key, w!("ProxyServer")) {
        Some(s) if !s.is_empty() => s,
        _ => {
            let _ = unsafe { RegCloseKey(key) };
            return None;
        }
    };
    let override_list = read_reg_sz(key, w!("ProxyOverride")).unwrap_or_default();
    let _ = unsafe { RegCloseKey(key) };

    let host = url_host(target_url)?;
    if proxy_bypass(&override_list, &host) {
        return None;
    }

    let proxy_addr = parse_proxy_server(&server, target_url.starts_with("https"))?;
    ureq::Proxy::new(&proxy_addr).ok()
}

#[cfg(windows)]
fn read_reg_dword(
    key: windows::Win32::System::Registry::HKEY,
    name: windows::core::PCWSTR,
) -> Option<u32> {
    use windows::Win32::System::Registry::{REG_DWORD, RegQueryValueExW};
    let mut ty = REG_DWORD;
    let mut data = [0u8; 4];
    let mut len = data.len() as u32;
    let status = unsafe {
        RegQueryValueExW(
            key,
            name,
            None,
            Some(&mut ty),
            Some(data.as_mut_ptr()),
            Some(&mut len),
        )
    };
    if status.is_err() || ty != REG_DWORD || len < 4 {
        return None;
    }
    Some(u32::from_le_bytes(data))
}

#[cfg(windows)]
fn read_reg_sz(
    key: windows::Win32::System::Registry::HKEY,
    name: windows::core::PCWSTR,
) -> Option<String> {
    use windows::Win32::System::Registry::{REG_SZ, RegQueryValueExW};
    let mut ty = REG_SZ;
    let mut len = 0u32;
    let probe = unsafe { RegQueryValueExW(key, name, None, Some(&mut ty), None, Some(&mut len)) };
    if probe.is_err() || len == 0 {
        return None;
    }
    let mut buf = vec![0u8; len as usize];
    let status = unsafe {
        RegQueryValueExW(
            key,
            name,
            None,
            Some(&mut ty),
            Some(buf.as_mut_ptr()),
            Some(&mut len),
        )
    };
    if status.is_err() {
        return None;
    }
    // REG_SZ is UTF-16LE.
    let wide_len = (len as usize) / 2;
    let (pairs, _) = buf.as_chunks::<2>();
    let mut wide: Vec<u16> = pairs
        .iter()
        .take(wide_len)
        .map(|chunk| u16::from_le_bytes(*chunk))
        .collect();
    while wide.last().copied() == Some(0) {
        wide.pop();
    }
    String::from_utf16(&wide).ok()
}

/// Parse `ProxyServer`: plain `host:port` or `http=..;https=..` forms.
fn parse_proxy_server(server: &str, prefer_https: bool) -> Option<String> {
    if !server.contains('=') {
        let addr = server.trim();
        if addr.is_empty() {
            return None;
        }
        if addr.contains("://") {
            return Some(addr.to_string());
        }
        return Some(format!("http://{addr}"));
    }

    let mut http = None;
    let mut https = None;
    let mut other = None;
    for part in server.split(';') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        if let Some((scheme, addr)) = part.split_once('=') {
            let addr = addr.trim();
            if addr.is_empty() {
                continue;
            }
            let url = if addr.contains("://") {
                addr.to_string()
            } else {
                format!("http://{addr}")
            };
            match scheme.trim().to_ascii_lowercase().as_str() {
                "https" => https = Some(url),
                "http" => http = Some(url),
                _ => {
                    if other.is_none() {
                        other = Some(url);
                    }
                }
            }
        }
    }
    if prefer_https {
        https.or(http).or(other)
    } else {
        http.or(https).or(other)
    }
}

fn url_host(url: &str) -> Option<String> {
    let rest = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))?;
    let host = rest.split('/').next()?.split('@').next_back()?;
    let host = host.split(':').next()?.trim();
    if host.is_empty() {
        None
    } else {
        Some(host.to_ascii_lowercase())
    }
}

/// Honor `ProxyOverride` tokens, including `<local>`.
fn proxy_bypass(override_list: &str, host: &str) -> bool {
    let host = host.to_ascii_lowercase();
    for token in override_list.split([';', ',']) {
        let token = token.trim().to_ascii_lowercase();
        if token.is_empty() {
            continue;
        }
        if token == "<local>" {
            if host == "localhost"
                || host == "127.0.0.1"
                || host == "::1"
                || !host.contains('.')
            {
                return true;
            }
            continue;
        }
        if token == host {
            return true;
        }
        let suffix = token.trim_start_matches('*');
        if !suffix.is_empty() && host.ends_with(suffix) {
            return true;
        }
    }
    false
}

fn map_ureq_err(prefix: &str, err: ureq::Error) -> String {
    match err {
        ureq::Error::Status(code, _) => format!("{prefix}: HTTP {code}"),
        ureq::Error::Transport(t) => {
            if transport_timed_out(&t) {
                return format!("{prefix}: timed out");
            }
            let phrase = match t.kind() {
                ureq::ErrorKind::Dns => "DNS failed",
                ureq::ErrorKind::ConnectionFailed => "connection failed",
                ureq::ErrorKind::TooManyRedirects => "too many redirects",
                ureq::ErrorKind::InvalidProxyUrl => "bad proxy",
                ureq::ErrorKind::ProxyConnect => "proxy connect failed",
                ureq::ErrorKind::ProxyUnauthorized => "proxy unauthorized",
                ureq::ErrorKind::Io => "I/O error",
                ureq::ErrorKind::InvalidUrl => "bad URL",
                ureq::ErrorKind::UnknownScheme => "bad URL scheme",
                _ => "transport error",
            };
            format!("{prefix}: {phrase}")
        }
    }
}

fn transport_timed_out(t: &ureq::Transport) -> bool {
    if let Some(msg) = t.message() {
        let lower = msg.to_ascii_lowercase();
        if lower.contains("timeout") || lower.contains("timed out") {
            return true;
        }
    }
    let mut src = std::error::Error::source(t);
    while let Some(e) = src {
        if let Some(ioe) = e.downcast_ref::<std::io::Error>()
            && ioe.kind() == std::io::ErrorKind::TimedOut
        {
            return true;
        }
        let lower = e.to_string().to_ascii_lowercase();
        if lower.contains("timeout") || lower.contains("timed out") {
            return true;
        }
        src = e.source();
    }
    false
}

// --- download ---------------------------------------------------------------

fn max_download_bytes() -> usize {
    #[cfg(feature = "update-probe")]
    if let Ok(v) = std::env::var("HYPERTYPE_UPDATER_MAX_DOWNLOAD")
        && let Ok(n) = v.parse::<usize>()
    {
        return n;
    }
    MAX_DOWNLOAD_BYTES
}

fn download_bytes(
    info: &UpdateInfo,
    on_progress: &mut dyn FnMut(Progress),
) -> Result<Vec<u8>, String> {
    let agent = build_agent(AgentKind::Download, &info.url);

    let response = match agent
        .get(&info.url)
        .set("Accept", "application/octet-stream")
        .call()
    {
        Ok(r) => r,
        Err(e) => return Err(map_ureq_err("download failed", e)),
    };

    if !(200..300).contains(&response.status()) {
        return Err(format!("download failed: HTTP {}", response.status()));
    }

    let content_length = response
        .header("Content-Length")
        .and_then(|v| v.parse::<u64>().ok());

    // Never allocate from Content-Length (hostile headers must not panic).
    let mut reader = response.into_reader();
    let mut buffer = Vec::new();
    let mut chunk = [0u8; 64 * 1024];
    let mut first_chunk = true;
    let limit = max_download_bytes();
    loop {
        let n = reader.read(&mut chunk).map_err(|e| {
            if e.kind() == std::io::ErrorKind::TimedOut {
                "download failed: timed out".to_string()
            } else {
                "download failed: I/O error".to_string()
            }
        })?;
        if n == 0 {
            break;
        }
        if buffer.len().saturating_add(n) > limit {
            return Err("download too large".into());
        }
        // Tauri commands.rs:124–129 — Started on the first received chunk.
        if first_chunk {
            first_chunk = false;
            on_progress(Progress::Started { content_length });
        }
        on_progress(Progress::Chunk(n));
        buffer.extend_from_slice(&chunk[..n]);
    }
    // Empty body emits only Finished.
    on_progress(Progress::Finished);
    Ok(buffer)
}

fn write_installer_temp(bytes: &[u8], version: &str) -> Result<PathBuf, String> {
    // Unique directory per attempt; package-name file like Tauri.
    let dir = tempfile::Builder::new()
        .prefix(&format!("{PKG_NAME}-{version}-updater-"))
        .tempdir()
        .map_err(|e| format!("temp dir: {e}"))?;
    let dir_path = dir.keep();
    let path = dir_path.join(format!("{PKG_NAME}-{version}-installer.exe"));
    std::fs::write(&path, bytes).map_err(|e| format!("temp write: {e}"))?;
    Ok(path)
}

// --- Windows NSIS launch ----------------------------------------------------

#[cfg(windows)]
fn launch_nsis_passive(installer: &Path) -> Result<(), String> {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::Shell::ShellExecuteW;
    use windows::Win32::UI::WindowsAndMessaging::SW_SHOW;
    use windows::core::{PCWSTR, w};

    // Require a Windows PE executable (feed ships an NSIS exe).
    let header = std::fs::read(installer).map_err(|_| "installer read failed".to_string())?;
    if header.len() < 2 || header[0] != b'M' || header[1] != b'Z' {
        return Err("installer is not a Windows executable".into());
    }

    // Passive mode from tauri.conf.json plugins.updater.windows.installMode.
    // nsis_args: /P ; always /UPDATE ; restart_after_install: /R /ARGS …
    // (config.rs:41–57, updater.rs:884–908).
    let mut params = OsString::from("/P /UPDATE /R /ARGS");
    for arg in std::env::args_os().skip(1) {
        params.push(" ");
        params.push(escape_nsis_arg(&arg));
    }

    let file = encode_wide(installer.as_os_str());
    let parameters = encode_wide(&params);

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

/// Escape one arg for NSIS `/ARGS` on UTF-16 units (Tauri updater.rs:1572–1608).
/// Preserves unpaired surrogates; same quote rules as Tauri.
fn escape_nsis_arg(arg: &OsStr) -> OsString {
    #[cfg(windows)]
    {
        use std::os::windows::ffi::{OsStrExt, OsStringExt};

        let mut cmd: Vec<u16> = Vec::new();
        // Compared to std we also escape `/` so NSIS will not treat it as a switch.
        let quote = arg.is_empty()
            || arg
                .as_encoded_bytes()
                .iter()
                .any(|c| *c == b' ' || *c == b'\t' || *c == b'/');
        if quote {
            cmd.push(u16::from(b'"'));
        }
        let mut backslashes: usize = 0;
        for x in arg.encode_wide() {
            if x == u16::from(b'\\') {
                backslashes += 1;
            } else {
                if x == u16::from(b'"') {
                    // Add n+1 backslashes to total 2n+1 before internal '"'.
                    cmd.extend((0..=backslashes).map(|_| u16::from(b'\\')));
                }
                backslashes = 0;
            }
            cmd.push(x);
        }
        if quote {
            // Add n backslashes to total 2n before ending '"'.
            cmd.extend((0..backslashes).map(|_| u16::from(b'\\')));
            cmd.push(u16::from(b'"'));
        }
        OsString::from_wide(&cmd)
    }
    #[cfg(not(windows))]
    {
        let s = arg.to_string_lossy();
        let needs_quote = s.is_empty()
            || s.chars()
                .any(|c| c == ' ' || c == '\t' || c == '/');
        if !needs_quote {
            return s.into_owned().into();
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
        out.into()
    }
}

#[cfg(windows)]
fn encode_wide(string: &OsStr) -> Vec<u16> {
    use std::os::windows::ffi::OsStrExt;
    string.encode_wide().chain(std::iter::once(0)).collect()
}

fn b64_to_string(b64: &str) -> Result<String, String> {
    let decoded = base64::engine::general_purpose::STANDARD
        .decode(b64)
        .map_err(|e| format!("b64 decode: {e}"))?;
    String::from_utf8(decoded).map_err(|_| "b64 not utf8".into())
}
