//! systemd-creds — Lists, shows, encrypts and decrypts service credentials.
//!
//! A drop-in replacement for `systemd-creds(1)` supporting the following
//! subcommands:
//!
//! - `list`          — List credentials passed into the current execution context
//! - `cat`           — Show contents of specified credentials
//! - `setup`         — Generate a host encryption key for credentials
//! - `encrypt`       — Encrypt a credential for use with LoadCredentialEncrypted=/SetCredentialEncrypted=
//! - `decrypt`       — Decrypt an encrypted credential
//! - `has-tpm2`      — Report whether TPM2 is available
//!
//! Encryption uses AES-256-GCM keyed by a SHA-256 hash of the host secret
//! concatenated with the credential name (matching systemd's approach).
//! Encrypted credentials are Base64-encoded for safe embedding in unit files.
//!
//! The host key is stored at `/var/lib/systemd/credential.secret` (256 bytes
//! of random data, readable only by root).
//!
//! TPM2 sealing is detected but not yet implemented — `--with-key=host` is
//! the default when TPM2 is unavailable.

use aes_gcm::aead::{Aead, KeyInit, OsRng};
use aes_gcm::{AeadCore, Aes256Gcm, Nonce};
use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64;
use clap::{Parser, Subcommand};
use sha2::{Digest, Sha256};
use std::fs;
use std::io::{self, Read, Write};
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::process;
use std::time::{SystemTime, UNIX_EPOCH};

// ---------------------------------------------------------------------------
// CLI definition
// ---------------------------------------------------------------------------

#[derive(Parser, Debug)]
#[command(
    name = "systemd-creds",
    about = "Lists, shows, encrypts and decrypts service credentials",
    version
)]
struct Cli {
    #[command(subcommand)]
    command: Command,

    /// When used with list/cat, operate on system credentials instead of
    /// the current execution context.
    #[arg(long, global = true)]
    system: bool,

    /// Suppress additional informational output.
    #[arg(short, long, global = true)]
    quiet: bool,

    /// Do not print column headers or footer hints.
    #[arg(long, global = true)]
    no_legend: bool,

    /// Do not pipe output into a pager.
    #[arg(long, global = true)]
    no_pager: bool,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// List credentials passed into the current execution context.
    List,

    /// Show contents of specified credentials.
    Cat {
        /// Credential names to display.
        #[arg(required = true)]
        credentials: Vec<String>,

        /// Transcode output: base64, unbase64, hex, unhex.
        #[arg(long)]
        transcode: Option<String>,

        /// Control trailing newline: auto, yes, no.
        #[arg(long, default_value = "auto")]
        newline: String,
    },

    /// Generate a host encryption key for credentials.
    Setup,

    /// Encrypt a credential.
    Encrypt {
        /// Input file path or "-" for stdin.
        input: String,

        /// Output file path or "-" for stdout.
        output: String,

        /// Credential name to embed (derived from output filename by default).
        #[arg(long)]
        name: Option<String>,

        /// Encryption key type: host, tpm2, host+tpm2, null, auto, auto-initrd.
        #[arg(long, default_value = "auto")]
        with_key: String,

        /// Shortcut for --with-key=host.
        #[arg(short = 'H', long)]
        host: bool,

        /// Shortcut for --with-key=tpm2.
        #[arg(short = 'T', long)]
        tpm2: bool,

        /// Timestamp to embed (microseconds since epoch, or "now").
        #[arg(long)]
        timestamp: Option<String>,

        /// Expiry timestamp (0 or empty = never).
        #[arg(long)]
        not_after: Option<String>,

        /// Show output as SetCredentialEncrypted= line (requires --name and output="-").
        #[arg(short = 'p', long)]
        pretty: bool,
    },

    /// Decrypt an encrypted credential.
    Decrypt {
        /// Input file path or "-" for stdin.
        input: String,

        /// Output file path or "-" for stdout (default).
        output: Option<String>,

        /// Credential name to validate against the embedded name.
        #[arg(long)]
        name: Option<String>,

        /// Transcode output: base64, unbase64, hex, unhex.
        #[arg(long)]
        transcode: Option<String>,

        /// Control trailing newline: auto, yes, no.
        #[arg(long, default_value = "auto")]
        newline: String,

        /// Allow decrypting null-key credentials even on secure-boot systems.
        #[arg(long)]
        allow_null: bool,
    },

    /// Report whether a TPM2 device is available.
    #[command(name = "has-tpm2")]
    HasTpm2,
}

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Path to the host encryption key.
const HOST_KEY_PATH: &str = "/var/lib/systemd/credential.secret";

/// Size of the host key in bytes.
const HOST_KEY_SIZE: usize = 256;

/// Path for system-level credentials (container pass-through).
const SYSTEM_CREDENTIALS_DIR: &str = "/run/credentials/@system";

/// Magic bytes identifying an encrypted credential blob (before base64).
/// "sHc\0" — matches systemd's CRED_MAGIC.
const CRED_MAGIC: [u8; 4] = [0x73, 0x48, 0x63, 0x00];

/// AES-256-GCM IV (nonce) size.
const AES_IV_SIZE: usize = 12;

/// AES-256-GCM authentication tag size.
const AES_TAG_SIZE: usize = 16;

// Sealing types (stored in the credential header).
const SEAL_NULL: u32 = 0;
const SEAL_HOST: u32 = 1;
const SEAL_TPM2: u32 = 2;
const _SEAL_HOST_TPM2: u32 = 3;

// ---------------------------------------------------------------------------
// Credential header wire format
// ---------------------------------------------------------------------------
// All integers are little-endian.
//
// Offset  Size   Field
// ------  ----   -----
//  0       4     magic ("sHc\0")
//  4       4     seal_type (le32)
//  8       8     timestamp (le64, usec since epoch)
// 16       8     not_after (le64, usec since epoch, 0=never)
// 24       4     name_len (le32)
// 28       N     name (UTF-8, not NUL-terminated)
// 28+N    12     iv (AES-GCM nonce)
// 40+N    ...    ciphertext (includes 16-byte GCM tag appended by aes-gcm)
//
// The whole blob is then base64-encoded for storage/transport.

const HEADER_FIXED_SIZE: usize = 4 + 4 + 8 + 8 + 4; // 28 bytes

// ---------------------------------------------------------------------------
// Helper functions
// ---------------------------------------------------------------------------

/// Get the credentials directory for the current context.
fn credentials_dir(system: bool) -> Option<PathBuf> {
    if system {
        let p = PathBuf::from(SYSTEM_CREDENTIALS_DIR);
        if p.is_dir() { Some(p) } else { None }
    } else {
        std::env::var("CREDENTIALS_DIRECTORY")
            .ok()
            .map(PathBuf::from)
    }
}

/// Read the host key from disk, or return an error message.
fn read_host_key() -> Result<Vec<u8>, String> {
    fs::read(HOST_KEY_PATH).map_err(|e| {
        format!(
            "Failed to read host key from {HOST_KEY_PATH}: {e}\n\
             Hint: Run 'systemd-creds setup' first to generate a host key."
        )
    })
}

/// Derive an AES-256 key from the host key and credential name.
///
/// key = SHA-256(host_key || credential_name_bytes)
fn derive_key(host_key: &[u8], cred_name: &str) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(host_key);
    hasher.update(cred_name.as_bytes());
    hasher.finalize().into()
}

/// Derive a fixed zero-length AES-256 key for null-sealed credentials.
/// key = SHA-256("") — effectively a well-known constant.
fn derive_null_key(cred_name: &str) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(cred_name.as_bytes());
    hasher.finalize().into()
}

/// Get current time as microseconds since the Unix epoch.
fn now_usec() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_micros() as u64
}

/// Read all data from a file path or stdin ("-").
fn read_input(path: &str) -> Result<Vec<u8>, String> {
    if path == "-" {
        let mut buf = Vec::new();
        io::stdin()
            .read_to_end(&mut buf)
            .map_err(|e| format!("Failed to read from stdin: {e}"))?;
        Ok(buf)
    } else {
        fs::read(path).map_err(|e| format!("Failed to read {path:?}: {e}"))
    }
}

/// Write data to a file path or stdout ("-").
fn write_output(path: &str, data: &[u8]) -> Result<(), String> {
    if path == "-" {
        io::stdout()
            .write_all(data)
            .map_err(|e| format!("Failed to write to stdout: {e}"))
    } else {
        fs::write(path, data).map_err(|e| format!("Failed to write {path:?}: {e}"))
    }
}

/// Derive the credential name from a file path's filename component.
fn name_from_path(path: &str) -> Option<String> {
    if path == "-" {
        return None;
    }
    Path::new(path)
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        // Strip common extensions used for credential files.
        .map(|n| n.strip_suffix(".cred").unwrap_or(&n).to_string())
}

/// Classify the security state of a credential file for `list` output.
fn security_state(path: &Path) -> &'static str {
    // Check if the backing filesystem is ramfs/tmpfs (secure, non-swappable).
    // We approximate this by checking if the path is under /run (tmpfs).
    let is_tmpfs = path.to_string_lossy().starts_with("/run");

    let mode = match fs::metadata(path) {
        Ok(m) => m.mode() & 0o777,
        Err(_) => return "insecure",
    };

    if mode != 0o400 {
        "insecure"
    } else if is_tmpfs {
        "secure"
    } else {
        "weak"
    }
}

/// Check whether a TPM2 device is available.
fn tpm2_available() -> bool {
    Path::new("/dev/tpmrm0").exists() || Path::new("/dev/tpm0").exists()
}

/// Check whether we're running in a container (simple heuristic).
fn in_container() -> bool {
    // systemd-detect-virt --container
    if let Ok(content) = fs::read_to_string("/run/systemd/container") {
        return !content.trim().is_empty();
    }
    // Check for /.dockerenv or /run/.containerenv
    Path::new("/.dockerenv").exists() || Path::new("/run/.containerenv").exists()
}

/// Transcode data according to the specified mode.
fn transcode(data: &[u8], mode: &str) -> Result<Vec<u8>, String> {
    match mode {
        "base64" => Ok(BASE64.encode(data).into_bytes()),
        "unbase64" => {
            let s = String::from_utf8_lossy(data);
            let cleaned: String = s.chars().filter(|c| !c.is_whitespace()).collect();
            BASE64
                .decode(&cleaned)
                .map_err(|e| format!("Failed to decode base64: {e}"))
        }
        "hex" => Ok(hex_encode(data).into_bytes()),
        "unhex" => {
            let s = String::from_utf8_lossy(data);
            let cleaned: String = s.chars().filter(|c| c.is_ascii_hexdigit()).collect();
            hex_decode(&cleaned)
        }
        other => Err(format!(
            "Unknown transcode mode: {other:?}\nSupported: base64, unbase64, hex, unhex"
        )),
    }
}

fn hex_encode(data: &[u8]) -> String {
    data.iter().map(|b| format!("{b:02x}")).collect()
}

fn hex_decode(s: &str) -> Result<Vec<u8>, String> {
    if !s.len().is_multiple_of(2) {
        return Err("Hex string has odd length".to_string());
    }
    (0..s.len())
        .step_by(2)
        .map(|i| {
            u8::from_str_radix(&s[i..i + 2], 16)
                .map_err(|e| format!("Invalid hex at offset {i}: {e}"))
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Encrypt / Decrypt
// ---------------------------------------------------------------------------

/// Encrypt plaintext into our credential wire format (before base64 encoding).
fn encrypt_credential(
    plaintext: &[u8],
    cred_name: &str,
    seal_type: u32,
    timestamp: u64,
    not_after: u64,
) -> Result<Vec<u8>, String> {
    // Derive the encryption key.
    let aes_key = match seal_type {
        SEAL_NULL => derive_null_key(cred_name),
        SEAL_HOST => {
            let host_key = read_host_key()?;
            derive_key(&host_key, cred_name)
        }
        SEAL_TPM2 => {
            return Err(
                "TPM2 sealing is not yet implemented. Use --with-key=host or --with-key=null."
                    .to_string(),
            );
        }
        _ => {
            return Err(format!("Unsupported seal type: {seal_type}"));
        }
    };

    let cipher = Aes256Gcm::new_from_slice(&aes_key)
        .map_err(|e| format!("Failed to initialize AES-256-GCM: {e}"))?;

    let nonce = Aes256Gcm::generate_nonce(&mut OsRng);

    // Encrypt (ciphertext includes the 16-byte GCM tag appended).
    let ciphertext = cipher
        .encrypt(&nonce, plaintext)
        .map_err(|e| format!("Encryption failed: {e}"))?;

    // Build the wire-format blob.
    let name_bytes = cred_name.as_bytes();
    let name_len = name_bytes.len() as u32;

    let total_size = HEADER_FIXED_SIZE + name_bytes.len() + AES_IV_SIZE + ciphertext.len();
    let mut blob = Vec::with_capacity(total_size);

    // Header
    blob.extend_from_slice(&CRED_MAGIC);
    blob.extend_from_slice(&seal_type.to_le_bytes());
    blob.extend_from_slice(&timestamp.to_le_bytes());
    blob.extend_from_slice(&not_after.to_le_bytes());
    blob.extend_from_slice(&name_len.to_le_bytes());
    blob.extend_from_slice(name_bytes);

    // IV
    blob.extend_from_slice(nonce.as_slice());

    // Ciphertext (with appended GCM tag)
    blob.extend_from_slice(&ciphertext);

    Ok(blob)
}

/// Decrypt a credential blob (after base64 decoding).
fn decrypt_credential(
    blob: &[u8],
    expected_name: Option<&str>,
    allow_null: bool,
) -> Result<(Vec<u8>, String), String> {
    if blob.len() < HEADER_FIXED_SIZE {
        return Err("Credential blob too short for header".to_string());
    }

    // Parse header.
    if blob[0..4] != CRED_MAGIC {
        return Err("Invalid credential magic — not an encrypted credential".to_string());
    }

    let seal_type = u32::from_le_bytes(blob[4..8].try_into().unwrap());
    let _timestamp = u64::from_le_bytes(blob[8..16].try_into().unwrap());
    let not_after = u64::from_le_bytes(blob[16..24].try_into().unwrap());
    let name_len = u32::from_le_bytes(blob[24..28].try_into().unwrap()) as usize;

    let name_end = HEADER_FIXED_SIZE + name_len;
    if blob.len() < name_end + AES_IV_SIZE {
        return Err("Credential blob too short for name + IV".to_string());
    }

    let cred_name = std::str::from_utf8(&blob[HEADER_FIXED_SIZE..name_end])
        .map_err(|e| format!("Invalid UTF-8 in credential name: {e}"))?
        .to_string();

    // Validate credential name if requested.
    if let Some(expected) = expected_name
        && !expected.is_empty()
        && !cred_name.is_empty()
        && expected != cred_name
    {
        return Err(format!(
            "Credential name mismatch: expected {expected:?}, got {:?}",
            cred_name
        ));
    }

    // Check expiry.
    if not_after != 0 {
        let now = now_usec();
        if now > not_after {
            return Err(format!(
                "Credential has expired (not-after: {not_after} µs, now: {now} µs)"
            ));
        }
    }

    // Extract IV and ciphertext.
    let iv_start = name_end;
    let iv_end = iv_start + AES_IV_SIZE;
    let iv = &blob[iv_start..iv_end];
    let ciphertext = &blob[iv_end..];

    if ciphertext.len() < AES_TAG_SIZE {
        return Err("Credential blob too short for ciphertext + tag".to_string());
    }

    // Derive the decryption key.
    let aes_key = match seal_type {
        SEAL_NULL => {
            if !allow_null && is_secure_boot_enabled() {
                return Err(
                    "Refusing to decrypt null-sealed credential on a Secure Boot system.\n\
                     Use --allow-null to override."
                        .to_string(),
                );
            }
            derive_null_key(&cred_name)
        }
        SEAL_HOST => {
            let host_key = read_host_key()?;
            derive_key(&host_key, &cred_name)
        }
        SEAL_TPM2 => {
            return Err(
                "TPM2-sealed credentials cannot be decrypted: TPM2 support not yet implemented."
                    .to_string(),
            );
        }
        other => {
            return Err(format!("Unknown seal type: {other}"));
        }
    };

    let cipher = Aes256Gcm::new_from_slice(&aes_key)
        .map_err(|e| format!("Failed to initialize AES-256-GCM: {e}"))?;

    let nonce = Nonce::from_slice(iv);

    let plaintext = cipher.decrypt(nonce, ciphertext).map_err(|_| {
        "Decryption failed: authentication tag mismatch.\n\
             The credential may have been encrypted with a different host key, \
             or the data may be corrupted."
            .to_string()
    })?;

    Ok((plaintext, cred_name))
}

/// Simple check for UEFI Secure Boot state.
fn is_secure_boot_enabled() -> bool {
    // Check the EFI variable (SecureBoot-8be4df61-...)
    let path = "/sys/firmware/efi/efivars/SecureBoot-8be4df61-93ca-11d2-aa0d-00e098032b8c";
    if let Ok(data) = fs::read(path) {
        // The variable is 5 bytes: 4-byte attributes + 1-byte value.
        // Value of 1 means Secure Boot is enabled.
        if data.len() >= 5 {
            return data[4] == 1;
        }
    }
    false
}

// ---------------------------------------------------------------------------
// Command implementations
// ---------------------------------------------------------------------------

/// `systemd-creds list`
fn cmd_list(system: bool, quiet: bool, no_legend: bool) {
    let dir = match credentials_dir(system) {
        Some(d) => d,
        None => {
            if !quiet {
                if system {
                    eprintln!("No system credentials directory found ({SYSTEM_CREDENTIALS_DIR}).");
                } else {
                    eprintln!(
                        "No credentials directory set.\n\
                         Hint: $CREDENTIALS_DIRECTORY is not set. This command is intended \
                         to be run from within a service context."
                    );
                }
            }
            process::exit(1);
        }
    };

    if !dir.is_dir() {
        if !quiet {
            eprintln!("Credentials directory {dir:?} does not exist or is not a directory.");
        }
        process::exit(1);
    }

    let mut entries: Vec<(String, u64, String)> = Vec::new();

    match fs::read_dir(&dir) {
        Ok(rd) => {
            for entry in rd.flatten() {
                let path = entry.path();
                if !path.is_file() {
                    continue;
                }
                let name = entry.file_name().to_string_lossy().into_owned();
                let size = fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
                let security = security_state(&path).to_string();
                entries.push((name, size, security));
            }
        }
        Err(e) => {
            eprintln!("Failed to read credentials directory {dir:?}: {e}");
            process::exit(1);
        }
    }

    entries.sort_by(|a, b| a.0.cmp(&b.0));

    if !no_legend && !entries.is_empty() {
        println!("{:<40} {:>10} SECURE", "NAME", "SIZE");
    }

    for (name, size, security) in &entries {
        println!("{name:<40} {size:>10} {security}");
    }

    if !no_legend {
        println!("\n{} credentials listed.", entries.len());
    }
}

/// `systemd-creds cat`
fn cmd_cat(
    credentials: &[String],
    system: bool,
    quiet: bool,
    transcode_mode: Option<&str>,
    newline: &str,
) {
    let dir = match credentials_dir(system) {
        Some(d) => d,
        None => {
            if !quiet {
                if system {
                    eprintln!("No system credentials directory found ({SYSTEM_CREDENTIALS_DIR}).");
                } else {
                    eprintln!(
                        "No credentials directory set.\n\
                         Hint: $CREDENTIALS_DIRECTORY is not set."
                    );
                }
            }
            process::exit(1);
        }
    };

    let mut failed = false;

    for cred_name in credentials {
        let path = dir.join(cred_name);
        if !path.is_file() {
            eprintln!("Credential {cred_name:?} not found in {dir:?}.");
            failed = true;
            continue;
        }

        let mut data = match fs::read(&path) {
            Ok(d) => d,
            Err(e) => {
                eprintln!("Failed to read credential {cred_name:?}: {e}");
                failed = true;
                continue;
            }
        };

        if let Some(mode) = transcode_mode {
            data = match transcode(&data, mode) {
                Ok(d) => d,
                Err(e) => {
                    eprintln!("Failed to transcode credential {cred_name:?}: {e}");
                    failed = true;
                    continue;
                }
            };
        }

        let stdout = io::stdout();
        let mut out = stdout.lock();
        if out.write_all(&data).is_err() {
            break;
        }

        // Handle trailing newline.
        let needs_newline = match newline {
            "yes" => true,
            "no" => false,
            _ => {
                // "auto": add newline if writing to a TTY and data doesn't
                // already end with one.
                let is_tty = unsafe { libc::isatty(libc::STDOUT_FILENO) == 1 };
                is_tty && !data.ends_with(b"\n")
            }
        };
        if needs_newline {
            let _ = out.write_all(b"\n");
        }
    }

    if failed {
        process::exit(1);
    }
}

/// `systemd-creds setup`
fn cmd_setup(quiet: bool) {
    let path = Path::new(HOST_KEY_PATH);

    if path.exists() {
        if !quiet {
            println!("Host key already exists at {HOST_KEY_PATH}.");
        }
        return;
    }

    // Ensure parent directory exists.
    if let Some(parent) = path.parent()
        && let Err(e) = fs::create_dir_all(parent)
    {
        eprintln!("Failed to create directory {parent:?}: {e}");
        process::exit(1);
    }

    // Generate random key.
    let key = generate_random_bytes(HOST_KEY_SIZE);

    // Write atomically: write to temp file, set permissions, rename.
    let tmp_path = format!("{HOST_KEY_PATH}.tmp.{}", std::process::id());
    if let Err(e) = fs::write(&tmp_path, &key) {
        eprintln!("Failed to write host key to {tmp_path:?}: {e}");
        process::exit(1);
    }

    // Set permissions to 0o400 (owner read only).
    if let Err(e) = fs::set_permissions(&tmp_path, fs::Permissions::from_mode(0o400)) {
        eprintln!("Failed to set permissions on {tmp_path:?}: {e}");
        let _ = fs::remove_file(&tmp_path);
        process::exit(1);
    }

    // Rename into place.
    if let Err(e) = fs::rename(&tmp_path, HOST_KEY_PATH) {
        eprintln!("Failed to rename {tmp_path:?} to {HOST_KEY_PATH}: {e}");
        let _ = fs::remove_file(&tmp_path);
        process::exit(1);
    }

    if !quiet {
        println!("Created host key at {HOST_KEY_PATH} ({HOST_KEY_SIZE} bytes).");
    }
}

/// `systemd-creds encrypt`
#[allow(clippy::too_many_arguments)]
fn cmd_encrypt(
    input: &str,
    output: &str,
    name: Option<&str>,
    with_key: &str,
    timestamp: Option<&str>,
    not_after: Option<&str>,
    pretty: bool,
    quiet: bool,
) {
    // Determine the credential name.
    let cred_name = match name {
        Some(n) => n.to_string(),
        None => match name_from_path(output) {
            Some(n) => n,
            None => {
                eprintln!(
                    "Cannot determine credential name from output path.\n\
                     Use --name= to specify it explicitly."
                );
                process::exit(1);
            }
        },
    };

    // Determine seal type.
    let seal_type = resolve_seal_type(with_key);

    // If seal type requires the host key, ensure it exists (auto-setup).
    if seal_type == SEAL_HOST && !Path::new(HOST_KEY_PATH).exists() {
        if !quiet {
            eprintln!("Host key not found, generating...");
        }
        cmd_setup(true);
    }

    // Read plaintext.
    let plaintext = match read_input(input) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("{e}");
            process::exit(1);
        }
    };

    // Timestamps.
    let ts = match timestamp {
        Some(s) => parse_timestamp_usec(s),
        None => now_usec(),
    };
    let na = match not_after {
        Some(s) if !s.is_empty() && s != "0" => parse_timestamp_usec(s),
        _ => 0,
    };

    // Encrypt.
    let blob = match encrypt_credential(&plaintext, &cred_name, seal_type, ts, na) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("Encryption failed: {e}");
            process::exit(1);
        }
    };

    // Base64-encode the blob.
    let b64 = BASE64.encode(&blob);

    if pretty && output == "-" {
        // Output as a unit-file–pasteable SetCredentialEncrypted= line.
        println!("SetCredentialEncrypted={cred_name}: \\");
        // Wrap at 80 chars with continuation backslashes.
        let indent = "        ";
        let wrap_width = 80 - indent.len() - 2; // -2 for " \"
        let chars: Vec<char> = b64.chars().collect();
        let chunks: Vec<String> = chars
            .chunks(wrap_width)
            .map(|c| c.iter().collect::<String>())
            .collect();
        for (i, chunk) in chunks.iter().enumerate() {
            if i < chunks.len() - 1 {
                println!("{indent}{chunk} \\");
            } else {
                println!("{indent}{chunk}");
            }
        }
    } else {
        // Write the base64 output.
        if let Err(e) = write_output(output, b64.as_bytes()) {
            eprintln!("{e}");
            process::exit(1);
        }
        // Add a trailing newline for file output.
        if output != "-" {
            // Already written as complete bytes.
        } else {
            println!();
        }
    }

    if !quiet && output != "-" {
        let type_str = seal_type_name(seal_type);
        eprintln!(
            "Encrypted credential {cred_name:?} ({} bytes plaintext) with {type_str} key.",
            plaintext.len()
        );
    }
}

/// `systemd-creds decrypt`
fn cmd_decrypt(
    input: &str,
    output: Option<&str>,
    name: Option<&str>,
    transcode_mode: Option<&str>,
    newline: &str,
    allow_null: bool,
    quiet: bool,
) {
    // Determine expected credential name.
    let expected_name = match name {
        Some(n) => Some(n.to_string()),
        None => name_from_path(input),
    };

    // Read the base64-encoded credential.
    let b64_data = match read_input(input) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("{e}");
            process::exit(1);
        }
    };

    // Decode base64.
    let b64_str = String::from_utf8_lossy(&b64_data);
    let cleaned: String = b64_str.chars().filter(|c| !c.is_whitespace()).collect();
    let blob = match BASE64.decode(&cleaned) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("Failed to decode base64 credential: {e}");
            process::exit(1);
        }
    };

    // Decrypt.
    let (mut plaintext, cred_name) =
        match decrypt_credential(&blob, expected_name.as_deref(), allow_null) {
            Ok(r) => r,
            Err(e) => {
                eprintln!("{e}");
                process::exit(1);
            }
        };

    // Transcode if requested.
    if let Some(mode) = transcode_mode {
        plaintext = match transcode(&plaintext, mode) {
            Ok(d) => d,
            Err(e) => {
                eprintln!("Failed to transcode: {e}");
                process::exit(1);
            }
        };
    }

    // Write output.
    let out_path = output.unwrap_or("-");
    if let Err(e) = write_output(out_path, &plaintext) {
        eprintln!("{e}");
        process::exit(1);
    }

    // Handle trailing newline.
    if out_path == "-" {
        let needs_newline = match newline {
            "yes" => true,
            "no" => false,
            _ => {
                let is_tty = unsafe { libc::isatty(libc::STDOUT_FILENO) == 1 };
                is_tty && !plaintext.ends_with(b"\n")
            }
        };
        if needs_newline {
            println!();
        }
    }

    if !quiet && out_path != "-" {
        eprintln!(
            "Decrypted credential {cred_name:?} ({} bytes).",
            plaintext.len()
        );
    }
}

/// `systemd-creds has-tpm2`
fn cmd_has_tpm2(quiet: bool) {
    let firmware = tpm2_available();
    let in_ctr = in_container();

    if !quiet {
        if firmware && !in_ctr {
            println!("yes");
            println!("+firmware  (TPM2 device found)");
        } else if firmware && in_ctr {
            println!("yes");
            println!("+firmware  (TPM2 device found, but running in container)");
        } else {
            println!("no");
            println!("-firmware  (no TPM2 device found)");
        }
    }

    if firmware && !in_ctr {
        process::exit(0);
    } else {
        process::exit(1);
    }
}

// ---------------------------------------------------------------------------
// Utilities
// ---------------------------------------------------------------------------

/// Read random bytes from /dev/urandom.
fn generate_random_bytes(len: usize) -> Vec<u8> {
    let mut buf = vec![0u8; len];
    let mut f = fs::File::open("/dev/urandom").expect("Failed to open /dev/urandom");
    io::Read::read_exact(&mut f, &mut buf).expect("Failed to read from /dev/urandom");
    buf
}

/// Resolve the `--with-key` argument to a seal type constant.
fn resolve_seal_type(with_key: &str) -> u32 {
    match with_key {
        "host" => SEAL_HOST,
        "tpm2" => {
            if !tpm2_available() || in_container() {
                eprintln!("TPM2 not available. Use --with-key=host or --with-key=null instead.");
                process::exit(1);
            }
            SEAL_TPM2
        }
        "host+tpm2" => {
            eprintln!("host+tpm2 combined sealing is not yet implemented.");
            process::exit(1);
        }
        "null" => SEAL_NULL,
        "auto" => {
            // Prefer host key if /var/lib/systemd/ is on persistent media.
            // Fall back to null if nothing works.
            if Path::new("/var/lib/systemd").exists() {
                SEAL_HOST
            } else if tpm2_available() && !in_container() {
                SEAL_TPM2
            } else {
                eprintln!(
                    "Cannot determine encryption key: /var/lib/systemd/ not found and \
                     no TPM2 available.\nUse --with-key=null for testing (no security!)."
                );
                process::exit(1);
            }
        }
        "auto-initrd" => {
            if tpm2_available() && !in_container() {
                SEAL_TPM2
            } else {
                SEAL_NULL
            }
        }
        other => {
            eprintln!(
                "Unknown key type: {other:?}\n\
                 Supported: host, tpm2, host+tpm2, null, auto, auto-initrd"
            );
            process::exit(1);
        }
    }
}

/// Human-readable name for a seal type.
fn seal_type_name(seal_type: u32) -> &'static str {
    match seal_type {
        SEAL_NULL => "null",
        SEAL_HOST => "host",
        SEAL_TPM2 => "tpm2",
        3 => "host+tpm2",
        _ => "unknown",
    }
}

/// Parse a timestamp string into microseconds since epoch.
/// Accepts "now", plain integer (microseconds), or seconds with "s" suffix.
fn parse_timestamp_usec(s: &str) -> u64 {
    let s = s.trim();
    if s.eq_ignore_ascii_case("now") {
        return now_usec();
    }
    if let Some(secs_str) = s.strip_suffix('s')
        && let Ok(secs) = secs_str.parse::<u64>()
    {
        return secs * 1_000_000;
    }
    s.parse::<u64>().unwrap_or_else(|_| {
        eprintln!("Failed to parse timestamp: {s:?}");
        process::exit(1);
    })
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

fn main() {
    let cli = Cli::parse();

    match &cli.command {
        Command::List => {
            cmd_list(cli.system, cli.quiet, cli.no_legend);
        }

        Command::Cat {
            credentials,
            transcode,
            newline,
        } => {
            cmd_cat(
                credentials,
                cli.system,
                cli.quiet,
                transcode.as_deref(),
                newline,
            );
        }

        Command::Setup => {
            cmd_setup(cli.quiet);
        }

        Command::Encrypt {
            input,
            output,
            name,
            with_key,
            host,
            tpm2,
            timestamp,
            not_after,
            pretty,
        } => {
            let effective_key = if *host {
                "host"
            } else if *tpm2 {
                "tpm2"
            } else {
                with_key.as_str()
            };
            cmd_encrypt(
                input,
                output,
                name.as_deref(),
                effective_key,
                timestamp.as_deref(),
                not_after.as_deref(),
                *pretty,
                cli.quiet,
            );
        }

        Command::Decrypt {
            input,
            output,
            name,
            transcode,
            newline,
            allow_null,
        } => {
            cmd_decrypt(
                input,
                output.as_deref(),
                name.as_deref(),
                transcode.as_deref(),
                newline,
                *allow_null,
                cli.quiet,
            );
        }

        Command::HasTpm2 => {
            cmd_has_tpm2(cli.quiet);
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cred_magic() {
        assert_eq!(&CRED_MAGIC, b"sHc\0");
    }

    #[test]
    fn test_derive_key_deterministic() {
        let host_key = vec![0xABu8; HOST_KEY_SIZE];
        let k1 = derive_key(&host_key, "my-credential");
        let k2 = derive_key(&host_key, "my-credential");
        assert_eq!(k1, k2);
    }

    #[test]
    fn test_derive_key_different_names_differ() {
        let host_key = vec![0xABu8; HOST_KEY_SIZE];
        let k1 = derive_key(&host_key, "credential-a");
        let k2 = derive_key(&host_key, "credential-b");
        assert_ne!(k1, k2);
    }

    #[test]
    fn test_derive_key_different_keys_differ() {
        let k1_data = vec![0x01u8; HOST_KEY_SIZE];
        let k2_data = vec![0x02u8; HOST_KEY_SIZE];
        let k1 = derive_key(&k1_data, "same-name");
        let k2 = derive_key(&k2_data, "same-name");
        assert_ne!(k1, k2);
    }

    #[test]
    fn test_derive_null_key_deterministic() {
        let k1 = derive_null_key("test");
        let k2 = derive_null_key("test");
        assert_eq!(k1, k2);
    }

    #[test]
    fn test_derive_null_key_different_names_differ() {
        let k1 = derive_null_key("a");
        let k2 = derive_null_key("b");
        assert_ne!(k1, k2);
    }

    #[test]
    fn test_encrypt_decrypt_null_roundtrip() {
        let plaintext = b"hello, world!";
        let cred_name = "test-cred";
        let timestamp = 1_000_000u64;
        let not_after = 0u64;

        let blob = encrypt_credential(plaintext, cred_name, SEAL_NULL, timestamp, not_after)
            .expect("encryption should succeed");

        let (decrypted, name) =
            decrypt_credential(&blob, Some(cred_name), true).expect("decryption should succeed");

        assert_eq!(decrypted, plaintext);
        assert_eq!(name, cred_name);
    }

    #[test]
    fn test_encrypt_decrypt_null_empty_plaintext() {
        let plaintext = b"";
        let cred_name = "empty";

        let blob = encrypt_credential(plaintext, cred_name, SEAL_NULL, now_usec(), 0)
            .expect("encryption should succeed");

        let (decrypted, name) =
            decrypt_credential(&blob, Some(cred_name), true).expect("decryption should succeed");

        assert_eq!(decrypted, plaintext.to_vec());
        assert_eq!(name, cred_name);
    }

    #[test]
    fn test_encrypt_decrypt_null_large_payload() {
        let plaintext: Vec<u8> = (0..10_000).map(|i| (i % 256) as u8).collect();
        let cred_name = "big-cred";

        let blob = encrypt_credential(&plaintext, cred_name, SEAL_NULL, now_usec(), 0)
            .expect("encryption should succeed");

        let (decrypted, _name) =
            decrypt_credential(&blob, Some(cred_name), true).expect("decryption should succeed");

        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn test_encrypt_decrypt_name_mismatch() {
        let plaintext = b"secret";
        let cred_name = "original-name";

        let blob = encrypt_credential(plaintext, cred_name, SEAL_NULL, now_usec(), 0)
            .expect("encryption should succeed");

        let result = decrypt_credential(&blob, Some("wrong-name"), true);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("mismatch"));
    }

    #[test]
    fn test_encrypt_decrypt_name_empty_skips_validation() {
        let plaintext = b"secret";
        let cred_name = "some-name";

        let blob = encrypt_credential(plaintext, cred_name, SEAL_NULL, now_usec(), 0)
            .expect("encryption should succeed");

        // Empty expected name → no validation.
        let (decrypted, _name) = decrypt_credential(&blob, Some(""), true)
            .expect("decryption should succeed with empty expected name");
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn test_encrypt_decrypt_not_after_expired() {
        let plaintext = b"secret";
        let cred_name = "expiring";
        // Set not_after to 1 µs in the past (epoch + 1).
        let blob = encrypt_credential(plaintext, cred_name, SEAL_NULL, 0, 1)
            .expect("encryption should succeed");

        let result = decrypt_credential(&blob, Some(cred_name), true);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("expired"));
    }

    #[test]
    fn test_encrypt_decrypt_not_after_future() {
        let plaintext = b"secret";
        let cred_name = "future";
        // Set not_after far in the future.
        let not_after = now_usec() + 3_600_000_000; // 1 hour from now
        let blob = encrypt_credential(plaintext, cred_name, SEAL_NULL, now_usec(), not_after)
            .expect("encryption should succeed");

        let (decrypted, _) = decrypt_credential(&blob, Some(cred_name), true)
            .expect("decryption should succeed (not expired)");
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn test_encrypt_decrypt_base64_roundtrip() {
        let plaintext = b"password123";
        let cred_name = "db-password";

        let blob = encrypt_credential(plaintext, cred_name, SEAL_NULL, now_usec(), 0)
            .expect("encryption should succeed");

        // Base64 encode, then decode (simulating file storage).
        let b64 = BASE64.encode(&blob);
        let decoded = BASE64.decode(&b64).expect("base64 decode should succeed");

        let (decrypted, name) =
            decrypt_credential(&decoded, Some(cred_name), true).expect("decryption should succeed");
        assert_eq!(decrypted, plaintext);
        assert_eq!(name, cred_name);
    }

    #[test]
    fn test_decrypt_truncated_blob_header() {
        // Too short to contain even the fixed header.
        let blob = vec![0x73, 0x48, 0x63, 0x00, 0x00];
        let result = decrypt_credential(&blob, None, true);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("too short"));
    }

    #[test]
    fn test_decrypt_bad_magic() {
        let mut blob = vec![0u8; 100];
        blob[0] = 0xFF; // Bad magic
        let result = decrypt_credential(&blob, None, true);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("magic"));
    }

    #[test]
    fn test_decrypt_corrupted_ciphertext() {
        let plaintext = b"important data";
        let cred_name = "test";

        let mut blob = encrypt_credential(plaintext, cred_name, SEAL_NULL, now_usec(), 0)
            .expect("encryption should succeed");

        // Corrupt the last byte (part of the GCM tag).
        let last = blob.len() - 1;
        blob[last] ^= 0xFF;

        let result = decrypt_credential(&blob, Some(cred_name), true);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("authentication tag"));
    }

    #[test]
    fn test_hex_encode() {
        assert_eq!(hex_encode(&[0x00, 0xFF, 0xAB]), "00ffab");
        assert_eq!(hex_encode(&[]), "");
    }

    #[test]
    fn test_hex_decode() {
        assert_eq!(hex_decode("00ffab").unwrap(), vec![0x00, 0xFF, 0xAB]);
        assert_eq!(hex_decode("").unwrap(), vec![]);
        assert!(hex_decode("0").is_err()); // odd length
        assert!(hex_decode("gg").is_err()); // invalid hex
    }

    #[test]
    fn test_transcode_base64() {
        let data = b"Hello, World!";
        let encoded = transcode(data, "base64").unwrap();
        assert_eq!(encoded, b"SGVsbG8sIFdvcmxkIQ==");

        let decoded = transcode(&encoded, "unbase64").unwrap();
        assert_eq!(decoded, data);
    }

    #[test]
    fn test_transcode_hex() {
        let data = b"\x00\xff\xab";
        let encoded = transcode(data, "hex").unwrap();
        assert_eq!(encoded, b"00ffab");

        let decoded = transcode(&encoded, "unhex").unwrap();
        assert_eq!(decoded, data);
    }

    #[test]
    fn test_transcode_unknown_mode() {
        let result = transcode(b"data", "unknown");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Unknown transcode mode"));
    }

    #[test]
    fn test_name_from_path_regular() {
        assert_eq!(
            name_from_path("/etc/creds/my-password.cred"),
            Some("my-password".to_string())
        );
    }

    #[test]
    fn test_name_from_path_no_extension() {
        assert_eq!(
            name_from_path("/etc/creds/my-password"),
            Some("my-password".to_string())
        );
    }

    #[test]
    fn test_name_from_path_stdin() {
        assert_eq!(name_from_path("-"), None);
    }

    #[test]
    fn test_name_from_path_just_filename() {
        assert_eq!(
            name_from_path("password.cred"),
            Some("password".to_string())
        );
    }

    #[test]
    fn test_seal_type_name() {
        assert_eq!(seal_type_name(SEAL_NULL), "null");
        assert_eq!(seal_type_name(SEAL_HOST), "host");
        assert_eq!(seal_type_name(SEAL_TPM2), "tpm2");
        assert_eq!(seal_type_name(3), "host+tpm2");
        assert_eq!(seal_type_name(99), "unknown");
    }

    #[test]
    fn test_parse_timestamp_now() {
        let ts = parse_timestamp_usec("now");
        assert!(ts > 0);
    }

    #[test]
    fn test_parse_timestamp_plain_integer() {
        assert_eq!(parse_timestamp_usec("1000000"), 1_000_000);
    }

    #[test]
    fn test_parse_timestamp_seconds_suffix() {
        assert_eq!(parse_timestamp_usec("60s"), 60_000_000);
    }

    #[test]
    fn test_encrypt_different_nonces() {
        // Two encryptions of the same data should produce different blobs
        // because the IV/nonce is random each time.
        let plaintext = b"same data";
        let cred_name = "test";
        let ts = now_usec();

        let blob1 = encrypt_credential(plaintext, cred_name, SEAL_NULL, ts, 0).unwrap();
        let blob2 = encrypt_credential(plaintext, cred_name, SEAL_NULL, ts, 0).unwrap();

        // The blobs should differ (different random nonce).
        assert_ne!(blob1, blob2);

        // But both should decrypt to the same plaintext.
        let (d1, _) = decrypt_credential(&blob1, Some(cred_name), true).unwrap();
        let (d2, _) = decrypt_credential(&blob2, Some(cred_name), true).unwrap();
        assert_eq!(d1, plaintext);
        assert_eq!(d2, plaintext);
    }

    #[test]
    fn test_credential_header_format() {
        let plaintext = b"test";
        let cred_name = "my-cred";

        let blob = encrypt_credential(plaintext, cred_name, SEAL_NULL, 42, 100).unwrap();

        // Verify magic.
        assert_eq!(&blob[0..4], &CRED_MAGIC);

        // Verify seal type.
        let seal_type = u32::from_le_bytes(blob[4..8].try_into().unwrap());
        assert_eq!(seal_type, SEAL_NULL);

        // Verify timestamp.
        let timestamp = u64::from_le_bytes(blob[8..16].try_into().unwrap());
        assert_eq!(timestamp, 42);

        // Verify not_after.
        let not_after = u64::from_le_bytes(blob[16..24].try_into().unwrap());
        assert_eq!(not_after, 100);

        // Verify name length.
        let name_len = u32::from_le_bytes(blob[24..28].try_into().unwrap()) as usize;
        assert_eq!(name_len, cred_name.len());

        // Verify name.
        let name = std::str::from_utf8(&blob[28..28 + name_len]).unwrap();
        assert_eq!(name, cred_name);
    }

    #[test]
    fn test_encrypt_unicode_credential_name() {
        let plaintext = b"data";
        let cred_name = "cred-日本語";

        let blob = encrypt_credential(plaintext, cred_name, SEAL_NULL, now_usec(), 0).unwrap();
        let (decrypted, name) = decrypt_credential(&blob, Some(cred_name), true).unwrap();
        assert_eq!(decrypted, plaintext);
        assert_eq!(name, cred_name);
    }

    #[test]
    fn test_encrypt_binary_plaintext() {
        let plaintext: Vec<u8> = (0..=255).collect();
        let cred_name = "binary";

        let blob = encrypt_credential(&plaintext, cred_name, SEAL_NULL, now_usec(), 0).unwrap();
        let (decrypted, _) = decrypt_credential(&blob, Some(cred_name), true).unwrap();
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn test_tpm2_available_check() {
        // Just ensure the function doesn't panic.
        let _ = tpm2_available();
    }

    #[test]
    fn test_in_container_check() {
        // Just ensure the function doesn't panic.
        let _ = in_container();
    }

    #[test]
    fn test_is_secure_boot_check() {
        // Just ensure the function doesn't panic.
        let _ = is_secure_boot_enabled();
    }

    #[test]
    fn test_now_usec_reasonable() {
        let ts = now_usec();
        // Should be after 2020-01-01 and before 2100-01-01.
        assert!(ts > 1_577_836_800_000_000); // 2020-01-01
        assert!(ts < 4_102_444_800_000_000); // 2100-01-01
    }

    #[test]
    fn test_decrypt_no_name_validation() {
        let plaintext = b"data";
        let cred_name = "original";

        let blob = encrypt_credential(plaintext, cred_name, SEAL_NULL, now_usec(), 0).unwrap();

        // None expected name → no validation.
        let (decrypted, name) = decrypt_credential(&blob, None, true).unwrap();
        assert_eq!(decrypted, plaintext);
        assert_eq!(name, cred_name);
    }

    #[test]
    fn test_security_state_non_existent_path() {
        let path = Path::new("/nonexistent/path/to/credential");
        assert_eq!(security_state(path), "insecure");
    }

    #[test]
    fn test_encrypt_decrypt_with_empty_name() {
        let plaintext = b"secret";
        let cred_name = "";

        let blob = encrypt_credential(plaintext, cred_name, SEAL_NULL, now_usec(), 0).unwrap();
        let (decrypted, name) = decrypt_credential(&blob, Some(""), true).unwrap();
        assert_eq!(decrypted, plaintext);
        assert_eq!(name, "");
    }
}
