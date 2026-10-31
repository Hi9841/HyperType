//! Release tool for the HyperType updater feed.
//!
//! `hypertype-sign <installer.exe> --version <x.y.z> --notes <notes.md> --url <download url> [--sig <file>]`
//!
//! With `--sig`, the tool does not sign. It verifies an existing signature
//! (for example from the manual `Sign release` workflow, which has the key
//! password as a repository secret) and writes `latest.json` with it.
//!
//! Signs the installer with the updater key in the same format as
//! `tauri signer sign` (base64 of a prehashed minisign signature box with the
//! trusted comment `timestamp:<unix>\tfile:<name>`), writes `<installer>.sig`,
//! and writes `latest.json` next to it with both platform keys that v1.1.x
//! updaters and the native updater look up.
//!
//! The key is the Tauri-format secret key (base64 of a minisign key box) at
//! `HYPERTYPE_SIGNING_KEY` (default `~/.tauri/hypertype.key`). The password
//! comes from `HYPERTYPE_SIGNING_PASSWORD` only, never from arguments.

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use minisign::{PublicKey, PublicKeyBox, SecretKeyBox, SignatureBox};

/// Updater public key, identical to `plugins.updater.pubkey` of v1.x and to
/// the key embedded in `crates/app/src/updater.rs`.
const PUBKEY_B64: &str = "dW50cnVzdGVkIGNvbW1lbnQ6IG1pbmlzaWduIHB1YmxpYyBrZXk6IDMxOUREQzQ1ODA1MDVCNApSV1MwQlFWWXhOMFpBd2dydTZQbXhlTXJmak9BanYxWnMxNXJiT1Q2VjRKSkVZWTh0cDF3RDltbAo=";

struct Args {
    installer: PathBuf,
    version: String,
    notes: PathBuf,
    url: String,
    sig: Option<PathBuf>,
}

fn main() {
    if let Err(message) = run() {
        eprintln!("hypertype-sign: {message}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let args = parse_args()?;
    let data = std::fs::read(&args.installer).map_err(|e| format!("read installer: {e}"))?;
    if !data.starts_with(b"MZ") {
        return Err("installer is not a Windows executable".into());
    }

    let public = public_key()?;
    let (signature, sig_path) = match &args.sig {
        Some(path) => {
            let signature = std::fs::read_to_string(path).map_err(|e| format!("read .sig: {e}"))?;
            verify(&public, &signature, &data)?;
            (signature.trim().to_owned(), path.clone())
        }
        None => {
            let signature = sign(&public, &args.installer, &data)?;
            let path = PathBuf::from(format!("{}.sig", args.installer.display()));
            std::fs::write(&path, &signature).map_err(|e| format!("write .sig: {e}"))?;
            (signature, path)
        }
    };

    let notes = std::fs::read_to_string(&args.notes).map_err(|e| format!("read notes: {e}"))?;
    let platform = serde_json::json!({ "signature": signature, "url": args.url });
    let latest = serde_json::json!({
        "version": args.version,
        "notes": notes.trim(),
        "pub_date": rfc3339_now(),
        "platforms": {
            "windows-x86_64": platform,
            "windows-x86_64-nsis": platform,
        },
    });
    let latest_path = args
        .installer
        .parent()
        .unwrap_or(Path::new("."))
        .join("latest.json");
    let text = serde_json::to_string_pretty(&latest).map_err(|e| e.to_string())?;
    std::fs::write(&latest_path, text + "\n").map_err(|e| format!("write latest.json: {e}"))?;

    println!("signature: {}", sig_path.display());
    println!("feed:      {}", latest_path.display());
    Ok(())
}

fn public_key() -> Result<PublicKey, String> {
    PublicKeyBox::from_string(&decode_text(PUBKEY_B64)?)
        .and_then(PublicKeyBox::into_public_key)
        .map_err(|e| format!("public key: {e}"))
}

/// The check the v1.x updater and the native updater run on the download.
fn verify(public: &PublicKey, signature_b64: &str, data: &[u8]) -> Result<(), String> {
    let signature_box = SignatureBox::from_string(&decode_text(signature_b64)?)
        .map_err(|e| format!("signature: {e}"))?;
    minisign::verify(
        public,
        &signature_box,
        std::io::Cursor::new(data),
        true,
        false,
        false,
    )
    .map_err(|e| format!("verify: {e}"))
}

/// Same output as `tauri signer sign`: base64 of the signature box text.
fn sign(public: &PublicKey, installer: &Path, data: &[u8]) -> Result<String, String> {
    let key_path = std::env::var_os("HYPERTYPE_SIGNING_KEY")
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("USERPROFILE")
                .map(|home| Path::new(&home).join(".tauri/hypertype.key"))
        })
        .ok_or("no signing key path")?;
    // Trimmed like the v1.x release workflow did, so a pasted password with
    // a stray space or line break opens the key.
    let password = std::env::var("HYPERTYPE_SIGNING_PASSWORD")
        .map_err(|_| "HYPERTYPE_SIGNING_PASSWORD is not set")?
        .trim()
        .to_owned();

    let key_b64 = std::fs::read_to_string(&key_path).map_err(|e| format!("read key: {e}"))?;
    let key_text = decode_text(&key_b64).map_err(|e| format!("key: {e}"))?;
    let secret = SecretKeyBox::from_string(&key_text)
        .and_then(|b| b.into_secret_key(Some(password)))
        .map_err(|e| format!("open key (wrong password?): {e}"))?;
    let file_name = installer
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or("installer file name")?;
    let trusted = format!("timestamp:{}\tfile:{file_name}", unix_now());
    // Passing the public key makes minisign verify the new signature, so a
    // key that does not match the updater pubkey fails here, not on users.
    let signature_box = minisign::sign(
        Some(public),
        &secret,
        data,
        Some(&trusted),
        Some("signature from tauri secret key"),
    )
    .map_err(|e| format!("sign: {e}"))?;
    Ok(STANDARD.encode(signature_box.into_string()))
}

fn decode_text(b64: &str) -> Result<String, String> {
    let bytes = STANDARD
        .decode(b64.trim())
        .map_err(|e| format!("base64: {e}"))?;
    String::from_utf8(bytes).map_err(|e| format!("utf-8: {e}"))
}

fn parse_args() -> Result<Args, String> {
    let mut installer = None;
    let mut version = None;
    let mut notes = None;
    let mut url = None;
    let mut sig = None;
    let mut it = std::env::args().skip(1);
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--version" => version = it.next(),
            "--notes" => notes = it.next().map(PathBuf::from),
            "--url" => url = it.next(),
            "--sig" => sig = it.next().map(PathBuf::from),
            _ if installer.is_none() => installer = Some(PathBuf::from(arg)),
            other => return Err(format!("unexpected argument {other}")),
        }
    }
    const USAGE: &str = "usage: hypertype-sign <installer.exe> --version <x.y.z> --notes <notes.md> --url <download url> [--sig <file>]";
    Ok(Args {
        installer: installer.ok_or(USAGE)?,
        version: version.ok_or(USAGE)?,
        notes: notes.ok_or(USAGE)?,
        url: url.ok_or(USAGE)?,
        sig,
    })
}

fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// UTC RFC 3339 without a date crate (civil-from-days, Howard Hinnant).
fn rfc3339_now() -> String {
    let secs = unix_now() as i64;
    let (days, rem) = (secs.div_euclid(86_400), secs.rem_euclid(86_400));
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        rem / 3_600,
        rem % 3_600 / 60,
        rem % 60
    )
}
