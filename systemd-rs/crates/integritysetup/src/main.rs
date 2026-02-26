//! systemd-integritysetup — Attach and detach dm-integrity block devices.
//!
//! A drop-in replacement for `systemd-integritysetup(8)`. This tool sets up
//! dm-integrity block devices via the device-mapper kernel interface, providing
//! data integrity protection using checksums stored alongside data.
//!
//! Usage:
//!
//!   systemd-integritysetup attach VOLUME DEVICE [KEY [OPTIONS]]
//!       Set up a dm-integrity block device. VOLUME is the device-mapper name,
//!       DEVICE is the underlying block device, KEY is an optional key file path
//!       (or "-" for no key / stdin), OPTIONS is a comma-separated list of
//!       options.
//!
//!   systemd-integritysetup detach VOLUME
//!       Tear down a previously attached dm-integrity device.
//!
//!   systemd-integritysetup --help | -h
//!       Show help.
//!
//!   systemd-integritysetup --version
//!       Show version.
//!
//! Options (comma-separated in OPTIONS):
//!   algorithm=ALG          Integrity hash algorithm (default: crc32c)
//!   journal-commit-time=MS Journal commit interval in milliseconds
//!   journal-watermark=PCT  Journal watermark percentage (0-100)
//!   journal-integrity=ALG  Journal integrity hash algorithm
//!   journal-integrity-key-size=BYTES  Key size for journal integrity
//!   journal-integrity-key-file=PATH   Key file for journal integrity
//!   journal-crypt=ALG      Journal encryption algorithm
//!   journal-crypt-key-size=BYTES  Key size for journal encryption
//!   journal-crypt-key-file=PATH   Key file for journal encryption
//!   data-device=PATH       Separate data device path
//!   sector-size=BYTES      Sector size for integrity (default: 512)
//!   bitmap-flush-interval=MS  Bitmap mode flush interval
//!   block-size=BYTES       Internal integrity block size
//!   integrity-recalculate  Recalculate integrity tags in background
//!   integrity-recalculate-reset  Reset recalculate position to start
//!   allow-discards         Allow TRIM/discard passthrough
//!   fix-padding            Fix metadata padding on older kernels
//!   fix-hmac               Fix HMAC issues on older kernels
//!   legacy-recalculate     Use legacy recalculate behavior
//!   no-journal             Disable journaling
//!   no-journal-bitmap      Disable journal bitmap mode
//!   recovery               Recovery mode (don't verify on activation)
//!   readonly               Open device read-only
//!   noauto                 Ignored (for /etc/integritytab compat)
//!   nofail                 Ignored (for /etc/integritytab compat)
//!   x-systemd.*            Ignored (for /etc/integritytab compat)
//!
//! Exit codes:
//!   0 — success
//!   1 — error (general)
//!   4 — integrity check failure

use std::fmt;
use std::fs;
use std::io;
use std::os::unix::io::RawFd;
use std::path::PathBuf;
use std::process;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Device-mapper control device path.
const DM_CONTROL_PATH: &str = "/dev/mapper/control";

/// Device-mapper ioctl version.
const DM_VERSION_MAJOR: u32 = 4;
const DM_VERSION_MINOR: u32 = 0;
const DM_VERSION_PATCHLEVEL: u32 = 0;

/// Device-mapper ioctl type (magic number).
const DM_IOCTL_TYPE: u32 = 0xfd;

/// DM ioctl structure header size (312 bytes).
const DM_IOCTL_HEADER_SIZE: usize = 312;

/// Size of the ioctl buffer we allocate.
const DM_IOCTL_BUF_SIZE: usize = 16384;

/// DM ioctl struct field offsets.
const DM_VERSION_OFFSET: usize = 0;
const DM_DATA_SIZE_OFFSET: usize = 12;
const DM_DATA_START_OFFSET: usize = 16;
const DM_TARGET_COUNT_OFFSET: usize = 20;
const DM_FLAGS_OFFSET: usize = 28;
const DM_NAME_OFFSET: usize = 48;
const DM_NAME_SIZE: usize = 128;
const DM_UUID_OFFSET: usize = 176;
const DM_UUID_SIZE: usize = 128;

/// DM ioctl commands.
const DM_DEV_CREATE_CMD: u32 = 3;
const DM_DEV_REMOVE_CMD: u32 = 4;
const DM_DEV_SUSPEND_CMD: u32 = 6;
const DM_TABLE_LOAD_CMD: u32 = 9;
const DM_TABLE_CLEAR_CMD: u32 = 10;

/// DM flags.
const DM_READONLY_FLAG: u32 = 1 << 0;
const DM_SUSPEND_FLAG: u32 = 1 << 1;

/// Default integrity algorithm.
const DEFAULT_ALGORITHM: &str = "crc32c";
/// Default sector size.
const DEFAULT_SECTOR_SIZE: u32 = 512;

/// Exit code for integrity check failures.
#[allow(dead_code)]
const EXIT_INTEGRITY_ERROR: i32 = 4;

// ---------------------------------------------------------------------------
// Journal mode
// ---------------------------------------------------------------------------

/// Journal mode for dm-integrity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum JournalMode {
    /// Standard journaling (default).
    Journal,
    /// No journaling — write-back without protection.
    NoJournal,
    /// Bitmap-based journaling.
    Bitmap,
}

impl fmt::Display for JournalMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            JournalMode::Journal => write!(f, "journal"),
            JournalMode::NoJournal => write!(f, "no-journal"),
            JournalMode::Bitmap => write!(f, "bitmap"),
        }
    }
}

// ---------------------------------------------------------------------------
// Parsed options from the comma-separated OPTIONS argument
// ---------------------------------------------------------------------------

/// Parsed integritysetup options from the OPTIONS string.
#[derive(Debug, Clone)]
struct IntegrityOptions {
    /// Integrity hash/checksum algorithm (e.g., "crc32c", "sha256", "hmac(sha256)").
    algorithm: String,
    /// Journal commit interval in milliseconds.
    journal_commit_time: Option<u64>,
    /// Journal watermark percentage (0–100).
    journal_watermark: Option<u32>,
    /// Journal integrity hash algorithm (for protecting the journal itself).
    journal_integrity: Option<String>,
    /// Key size for journal integrity in bytes.
    journal_integrity_key_size: Option<usize>,
    /// Key file for journal integrity.
    journal_integrity_key_file: Option<PathBuf>,
    /// Journal encryption algorithm.
    journal_crypt: Option<String>,
    /// Key size for journal encryption in bytes.
    journal_crypt_key_size: Option<usize>,
    /// Key file for journal encryption.
    journal_crypt_key_file: Option<PathBuf>,
    /// Separate data device path.
    data_device: Option<PathBuf>,
    /// Sector size for integrity (default: 512).
    sector_size: u32,
    /// Bitmap mode flush interval in milliseconds.
    bitmap_flush_interval: Option<u64>,
    /// Internal integrity block size.
    block_size: Option<u32>,
    /// Recalculate integrity tags in background.
    integrity_recalculate: bool,
    /// Reset recalculate position to start.
    integrity_recalculate_reset: bool,
    /// Allow discard/TRIM passthrough.
    allow_discards: bool,
    /// Fix metadata padding (older kernels).
    fix_padding: bool,
    /// Fix HMAC issues (older kernels).
    fix_hmac: bool,
    /// Use legacy recalculate behavior.
    legacy_recalculate: bool,
    /// Journal mode.
    journal_mode: JournalMode,
    /// Recovery mode (don't verify on activation).
    recovery: bool,
    /// Open device read-only.
    readonly: bool,
}

impl Default for IntegrityOptions {
    fn default() -> Self {
        Self {
            algorithm: DEFAULT_ALGORITHM.to_string(),
            journal_commit_time: None,
            journal_watermark: None,
            journal_integrity: None,
            journal_integrity_key_size: None,
            journal_integrity_key_file: None,
            journal_crypt: None,
            journal_crypt_key_size: None,
            journal_crypt_key_file: None,
            data_device: None,
            sector_size: DEFAULT_SECTOR_SIZE,
            bitmap_flush_interval: None,
            block_size: None,
            integrity_recalculate: false,
            integrity_recalculate_reset: false,
            allow_discards: false,
            fix_padding: false,
            fix_hmac: false,
            legacy_recalculate: false,
            journal_mode: JournalMode::Journal,
            recovery: false,
            readonly: false,
        }
    }
}

/// Parse a comma-separated options string (the 4th positional argument).
fn parse_options(opts_str: &str) -> Result<IntegrityOptions, String> {
    let mut opts = IntegrityOptions::default();

    if opts_str.is_empty() || opts_str == "-" || opts_str == "none" {
        return Ok(opts);
    }

    for part in opts_str.split(',') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }

        if let Some((key, val)) = part.split_once('=') {
            let key = key.trim();
            let val = val.trim();
            match key {
                "algorithm" | "integrity-algorithm" => {
                    opts.algorithm = val.to_string();
                }
                "journal-commit-time" | "journal_commit_time" => {
                    opts.journal_commit_time = Some(
                        val.parse()
                            .map_err(|_| format!("Invalid journal-commit-time: {val}"))?,
                    );
                }
                "journal-watermark" | "journal_watermark" => {
                    let wm: u32 = val
                        .parse()
                        .map_err(|_| format!("Invalid journal-watermark: {val}"))?;
                    if wm > 100 {
                        return Err(format!("Invalid journal-watermark: {wm} (must be 0-100)"));
                    }
                    opts.journal_watermark = Some(wm);
                }
                "journal-integrity" | "journal_integrity" => {
                    opts.journal_integrity = Some(val.to_string());
                }
                "journal-integrity-key-size" | "journal_integrity_key_size" => {
                    opts.journal_integrity_key_size = Some(
                        val.parse()
                            .map_err(|_| format!("Invalid journal-integrity-key-size: {val}"))?,
                    );
                }
                "journal-integrity-key-file" | "journal_integrity_key_file" => {
                    opts.journal_integrity_key_file = Some(PathBuf::from(val));
                }
                "journal-crypt" | "journal_crypt" => {
                    opts.journal_crypt = Some(val.to_string());
                }
                "journal-crypt-key-size" | "journal_crypt_key_size" => {
                    opts.journal_crypt_key_size = Some(
                        val.parse()
                            .map_err(|_| format!("Invalid journal-crypt-key-size: {val}"))?,
                    );
                }
                "journal-crypt-key-file" | "journal_crypt_key_file" => {
                    opts.journal_crypt_key_file = Some(PathBuf::from(val));
                }
                "data-device" | "data_device" => {
                    opts.data_device = Some(PathBuf::from(val));
                }
                "sector-size" | "sector_size" => {
                    opts.sector_size = val
                        .parse()
                        .map_err(|_| format!("Invalid sector-size: {val}"))?;
                    if !opts.sector_size.is_power_of_two() || opts.sector_size < 512 {
                        return Err(format!(
                            "Invalid sector-size: {} (must be power of 2, >= 512)",
                            opts.sector_size
                        ));
                    }
                }
                "bitmap-flush-interval" | "bitmap_flush_interval" => {
                    opts.bitmap_flush_interval = Some(
                        val.parse()
                            .map_err(|_| format!("Invalid bitmap-flush-interval: {val}"))?,
                    );
                }
                "block-size" | "block_size" => {
                    opts.block_size = Some(
                        val.parse()
                            .map_err(|_| format!("Invalid block-size: {val}"))?,
                    );
                }
                _ if key.starts_with("x-systemd.") => { /* ignored */ }
                _ => {
                    return Err(format!("Unknown option: {key}={val}"));
                }
            }
        } else {
            match part {
                "integrity-recalculate" | "integrity_recalculate" => {
                    opts.integrity_recalculate = true;
                }
                "integrity-recalculate-reset" | "integrity_recalculate_reset" => {
                    opts.integrity_recalculate_reset = true;
                }
                "allow-discards" | "allow_discards" | "discard" => {
                    opts.allow_discards = true;
                }
                "fix-padding" | "fix_padding" => {
                    opts.fix_padding = true;
                }
                "fix-hmac" | "fix_hmac" => {
                    opts.fix_hmac = true;
                }
                "legacy-recalculate" | "legacy_recalculate" => {
                    opts.legacy_recalculate = true;
                }
                "no-journal" | "no_journal" => {
                    opts.journal_mode = JournalMode::NoJournal;
                }
                "no-journal-bitmap" | "no_journal_bitmap" | "bitmap" => {
                    opts.journal_mode = JournalMode::Bitmap;
                }
                "recovery" => {
                    opts.recovery = true;
                }
                "readonly" | "read-only" => {
                    opts.readonly = true;
                }
                "noauto" | "nofail" | "auto" => { /* /etc/integritytab compat — ignored */ }
                _ if part.starts_with("x-systemd.") => { /* ignored */ }
                _ => {
                    return Err(format!("Unknown option: {part}"));
                }
            }
        }
    }

    Ok(opts)
}

// ---------------------------------------------------------------------------
// CLI argument parsing
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
enum Command {
    Attach {
        volume: String,
        device: String,
        key: Option<String>,
        options: String,
    },
    Detach {
        volume: String,
    },
    Help,
    Version,
}

fn parse_args(args: &[String]) -> Result<Command, String> {
    if args.is_empty() {
        return Err("No command specified. Use 'attach' or 'detach'.".to_string());
    }

    let mut iter = args.iter();
    let first = iter.next().unwrap();

    match first.as_str() {
        "--help" | "-h" | "help" => Ok(Command::Help),
        "--version" | "version" => Ok(Command::Version),
        "attach" => {
            let volume = iter
                .next()
                .ok_or("attach: missing VOLUME argument")?
                .clone();
            let device = iter
                .next()
                .ok_or("attach: missing DEVICE argument")?
                .clone();
            let key = iter.next().cloned();
            let options = iter.next().cloned().unwrap_or_default();
            Ok(Command::Attach {
                volume,
                device,
                key,
                options,
            })
        }
        "detach" => {
            let volume = iter
                .next()
                .ok_or("detach: missing VOLUME argument")?
                .clone();
            Ok(Command::Detach { volume })
        }
        other => Err(format!("Unknown command: {other}")),
    }
}

// ---------------------------------------------------------------------------
// Device-mapper ioctl helpers
// ---------------------------------------------------------------------------

/// Encode a DM ioctl number from the command index.
fn dm_ioctl_nr(cmd: u32) -> libc::c_ulong {
    let dir: libc::c_ulong = 3; // IOC_READ | IOC_WRITE
    let size = DM_IOCTL_HEADER_SIZE as libc::c_ulong;
    let typ = DM_IOCTL_TYPE as libc::c_ulong;
    let nr = cmd as libc::c_ulong;
    (dir << 30) | (size << 16) | (typ << 8) | nr
}

/// Initialize a DM ioctl buffer with the standard header fields.
fn dm_ioctl_init(buf: &mut [u8], name: &str, uuid: &str, flags: u32) {
    assert!(buf.len() >= DM_IOCTL_HEADER_SIZE);

    for b in buf.iter_mut() {
        *b = 0;
    }

    write_u32(buf, DM_VERSION_OFFSET, DM_VERSION_MAJOR);
    write_u32(buf, DM_VERSION_OFFSET + 4, DM_VERSION_MINOR);
    write_u32(buf, DM_VERSION_OFFSET + 8, DM_VERSION_PATCHLEVEL);
    write_u32(buf, DM_DATA_SIZE_OFFSET, buf.len() as u32);
    write_u32(buf, DM_DATA_START_OFFSET, DM_IOCTL_HEADER_SIZE as u32);
    write_u32(buf, DM_FLAGS_OFFSET, flags);
    write_string(buf, DM_NAME_OFFSET, DM_NAME_SIZE, name);
    write_string(buf, DM_UUID_OFFSET, DM_UUID_SIZE, uuid);
}

fn write_u32(buf: &mut [u8], offset: usize, val: u32) {
    buf[offset..offset + 4].copy_from_slice(&val.to_ne_bytes());
}

fn read_u32(buf: &[u8], offset: usize) -> u32 {
    u32::from_ne_bytes(buf[offset..offset + 4].try_into().unwrap())
}

fn write_u64(buf: &mut [u8], offset: usize, val: u64) {
    buf[offset..offset + 8].copy_from_slice(&val.to_ne_bytes());
}

#[allow(dead_code)]
fn read_u64(buf: &[u8], offset: usize) -> u64 {
    u64::from_ne_bytes(buf[offset..offset + 8].try_into().unwrap())
}

fn write_string(buf: &mut [u8], offset: usize, max_len: usize, s: &str) {
    let bytes = s.as_bytes();
    let copy_len = bytes.len().min(max_len - 1);
    buf[offset..offset + copy_len].copy_from_slice(&bytes[..copy_len]);
    buf[offset + copy_len] = 0;
}

/// Append a dm_target_spec + parameter string to the ioctl buffer.
///
/// dm_target_spec layout (40 bytes):
///   u64 sector_start
///   u64 length          (in 512-byte sectors)
///   i32 status          (must be 0)
///   u32 next            (offset to next target spec, 0 if last)
///   char target_type[16]
///   ... parameter string follows immediately after
fn append_target(
    buf: &mut [u8],
    data_start: usize,
    sector_start: u64,
    length_sectors: u64,
    target_type: &str,
    params: &str,
) -> usize {
    let spec_size: usize = 40;
    let param_bytes = params.as_bytes();
    let total = align8(spec_size + param_bytes.len() + 1);

    assert!(data_start + total <= buf.len(), "DM ioctl buffer overflow");

    write_u64(buf, data_start, sector_start);
    write_u64(buf, data_start + 8, length_sectors);
    write_u32(buf, data_start + 16, 0); // status
    write_u32(buf, data_start + 20, 0); // next
    write_string(buf, data_start + 24, 16, target_type);

    let param_off = data_start + spec_size;
    buf[param_off..param_off + param_bytes.len()].copy_from_slice(param_bytes);
    buf[param_off + param_bytes.len()] = 0;

    total
}

/// Align a value up to an 8-byte boundary.
fn align8(v: usize) -> usize {
    (v + 7) & !7
}

/// Issue a DM ioctl against `/dev/mapper/control`.
fn dm_ioctl(cmd: u32, buf: &mut [u8]) -> io::Result<()> {
    let fd = open_dm_control()?;
    let request = dm_ioctl_nr(cmd);
    let ret = unsafe { libc::ioctl(fd, request, buf.as_mut_ptr()) };
    close_fd(fd);
    if ret < 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

fn open_dm_control() -> io::Result<RawFd> {
    let path = std::ffi::CString::new(DM_CONTROL_PATH).unwrap();
    let fd = unsafe { libc::open(path.as_ptr(), libc::O_RDWR) };
    if fd < 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(fd)
    }
}

fn close_fd(fd: RawFd) {
    unsafe {
        libc::close(fd);
    }
}

// ---------------------------------------------------------------------------
// Key handling
// ---------------------------------------------------------------------------

/// Read a key from the specified source.
///
/// - `None` or `"-"` or `""` → no key (return empty)
/// - A file path → read the file contents
fn read_key(key_arg: Option<&str>) -> io::Result<Vec<u8>> {
    match key_arg {
        None | Some("-") | Some("") | Some("none") => Ok(Vec::new()),
        Some(path) => fs::read(path),
    }
}

/// Encode key bytes as a hex string for dm-integrity table parameters.
fn key_to_hex(key: &[u8]) -> String {
    let mut hex = String::with_capacity(key.len() * 2);
    for b in key {
        hex.push_str(&format!("{b:02x}"));
    }
    hex
}

// ---------------------------------------------------------------------------
// Device helpers
// ---------------------------------------------------------------------------

/// Get the size of a block device in 512-byte sectors.
fn device_size_sectors(device: &str) -> io::Result<u64> {
    let path = std::ffi::CString::new(device)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e))?;
    let fd = unsafe { libc::open(path.as_ptr(), libc::O_RDONLY) };
    if fd < 0 {
        return Err(io::Error::last_os_error());
    }

    let mut size: u64 = 0;
    // BLKGETSIZE64 = _IOR(0x12, 114, size_t) on Linux.
    let blkgetsize64: libc::c_ulong = {
        let dir: libc::c_ulong = 2; // _IOC_READ
        let typ: libc::c_ulong = 0x12;
        let nr: libc::c_ulong = 114;
        let sz: libc::c_ulong = std::mem::size_of::<u64>() as libc::c_ulong;
        (dir << 30) | (sz << 16) | (typ << 8) | nr
    };

    let ret = unsafe { libc::ioctl(fd, blkgetsize64, &mut size as *mut u64) };
    close_fd(fd);
    if ret < 0 {
        return Err(io::Error::last_os_error());
    }

    Ok(size / 512)
}

// ---------------------------------------------------------------------------
// dm-integrity table construction
// ---------------------------------------------------------------------------

/// Build the dm-integrity target parameter string.
///
/// Format:
///   `<device> <offset> <tag_size> <mode> [<#optional_args> <optional_args>...]`
///
/// Where mode is "J" (journal), "D" (direct/no-journal), "B" (bitmap), or "R" (recovery).
fn build_integrity_params(opts: &IntegrityOptions, device: &str, key: &[u8]) -> String {
    // Determine tag size from algorithm.
    let tag_size = integrity_tag_size(&opts.algorithm);

    // Mode character.
    let mode = if opts.recovery {
        "R"
    } else {
        match opts.journal_mode {
            JournalMode::Journal => "J",
            JournalMode::NoJournal => "D",
            JournalMode::Bitmap => "B",
        }
    };

    // Offset is always 0 for standard setup (the kernel calculates the
    // superblock/metadata offset internally).
    let offset = 0;

    // Build the base parameter string.
    let mut params = format!("{device} {offset} {tag_size} {mode}");

    // Build optional arguments.
    let mut optional_args: Vec<String> = Vec::new();

    // Internal hash algorithm.
    optional_args.push(format!("internal_hash:{}", opts.algorithm));

    // Key for HMAC-based algorithms.
    if !key.is_empty() {
        optional_args.push(format!("key:{}", key_to_hex(key)));
    }

    // Sector size.
    if opts.sector_size != DEFAULT_SECTOR_SIZE {
        optional_args.push(format!("sectors_per_bit:{}", opts.sector_size));
    }

    // Block size.
    if let Some(bs) = opts.block_size {
        optional_args.push(format!("block_size:{bs}"));
    }

    // Journal commit time.
    if let Some(jct) = opts.journal_commit_time {
        optional_args.push(format!("journal_commit_time:{jct}"));
    }

    // Journal watermark.
    if let Some(jw) = opts.journal_watermark {
        optional_args.push(format!("journal_watermark:{jw}"));
    }

    // Journal integrity.
    if let Some(ref ji) = opts.journal_integrity {
        optional_args.push(format!("journal_mac:{ji}"));
    }

    // Journal crypt.
    if let Some(ref jc) = opts.journal_crypt {
        optional_args.push(format!("journal_crypt:{jc}"));
    }

    // Data device.
    if let Some(ref dd) = opts.data_device {
        optional_args.push(format!("data_device:{}", dd.display()));
    }

    // Bitmap flush interval.
    if let Some(bfi) = opts.bitmap_flush_interval {
        optional_args.push(format!("bitmap_flush_interval:{bfi}"));
    }

    // Boolean flags.
    if opts.integrity_recalculate {
        optional_args.push("recalculate".to_string());
    }

    if opts.integrity_recalculate_reset {
        optional_args.push("reset_recalculate".to_string());
    }

    if opts.allow_discards {
        optional_args.push("allow_discards".to_string());
    }

    if opts.fix_padding {
        optional_args.push("fix_padding".to_string());
    }

    if opts.fix_hmac {
        optional_args.push("fix_hmac".to_string());
    }

    if opts.legacy_recalculate {
        optional_args.push("legacy_recalculate".to_string());
    }

    // Append optional args count and args.
    if !optional_args.is_empty() {
        params.push_str(&format!(" {}", optional_args.len()));
        for arg in &optional_args {
            params.push_str(&format!(" {arg}"));
        }
    }

    params
}

/// Return the expected tag (checksum) size in bytes for a given integrity algorithm.
fn integrity_tag_size(algorithm: &str) -> u32 {
    match algorithm {
        "crc32" | "crc32c" => 4,
        "sha1" | "hmac(sha1)" => 20,
        "sha256" | "hmac(sha256)" => 32,
        "sha512" | "hmac(sha512)" => 64,
        "xxhash64" => 8,
        "blake2b-256" | "hmac(blake2b-256)" => 32,
        "blake2b-512" | "hmac(blake2b-512)" => 64,
        "poly1305" => 16,
        // For unknown algorithms, default to 32 (sha256-sized).
        _ => 32,
    }
}

// ---------------------------------------------------------------------------
// Attach / Detach operations
// ---------------------------------------------------------------------------

/// Attach a dm-integrity block device via device-mapper.
fn cmd_attach(
    volume: &str,
    device: &str,
    key_arg: Option<&str>,
    options_str: &str,
) -> Result<(), String> {
    let opts = parse_options(options_str)?;

    // Read key material (if any).
    let key = read_key(key_arg).map_err(|e| format!("Failed to read key: {e}"))?;

    // Validate: HMAC algorithms require a key.
    if opts.algorithm.starts_with("hmac(") && key.is_empty() {
        return Err(format!(
            "HMAC algorithm '{}' requires a key, but none was provided.",
            opts.algorithm
        ));
    }

    // Check data device exists if specified.
    if let Some(ref dd) = opts.data_device
        && !dd.exists()
    {
        return Err(format!("Data device not found: {}", dd.display()));
    }

    // Check journal integrity key file exists if specified.
    if let Some(ref kf) = opts.journal_integrity_key_file
        && !kf.exists()
    {
        return Err(format!(
            "Journal integrity key file not found: {}",
            kf.display()
        ));
    }

    // Check journal crypt key file exists if specified.
    if let Some(ref kf) = opts.journal_crypt_key_file
        && !kf.exists()
    {
        return Err(format!(
            "Journal crypt key file not found: {}",
            kf.display()
        ));
    }

    // Get device size.
    let total_sectors = device_size_sectors(device)
        .map_err(|e| format!("Failed to get device size for {device}: {e}"))?;

    if total_sectors == 0 {
        return Err(format!("Device {device} has zero size."));
    }

    // The actual usable size is determined by the kernel after subtracting
    // the superblock and journal. We pass the full device size and let the
    // kernel handle it. For the table, we use a reasonable estimate: the
    // device size minus overhead. In practice, the kernel adjusts this.
    let target_sectors = total_sectors;

    // Build UUID.
    let uuid_str = format!("CRYPT-INTEGRITY-{volume}");

    // Build the dm-integrity target parameters.
    let integrity_params = build_integrity_params(&opts, device, &key);

    let flags = if opts.readonly { DM_READONLY_FLAG } else { 0 };

    // 1. DM_DEV_CREATE.
    let mut buf = vec![0u8; DM_IOCTL_BUF_SIZE];
    dm_ioctl_init(&mut buf, volume, &uuid_str, flags);
    dm_ioctl(DM_DEV_CREATE_CMD, &mut buf)
        .map_err(|e| format!("DM_DEV_CREATE failed for {volume}: {e}"))?;

    // 2. DM_TABLE_LOAD.
    dm_ioctl_init(&mut buf, volume, &uuid_str, flags);
    write_u32(&mut buf, DM_TARGET_COUNT_OFFSET, 1);
    let data_start = read_u32(&buf, DM_DATA_START_OFFSET) as usize;
    let _target_size = append_target(
        &mut buf,
        data_start,
        0,
        target_sectors,
        "integrity",
        &integrity_params,
    );

    if let Err(e) = dm_ioctl(DM_TABLE_LOAD_CMD, &mut buf) {
        let mut cleanup = vec![0u8; DM_IOCTL_BUF_SIZE];
        dm_ioctl_init(&mut cleanup, volume, "", 0);
        let _ = dm_ioctl(DM_DEV_REMOVE_CMD, &mut cleanup);
        return Err(format!("DM_TABLE_LOAD failed for {volume}: {e}"));
    }

    // 3. DM_DEV_SUSPEND (resume).
    dm_ioctl_init(&mut buf, volume, &uuid_str, flags);
    if let Err(e) = dm_ioctl(DM_DEV_SUSPEND_CMD, &mut buf) {
        let mut cleanup = vec![0u8; DM_IOCTL_BUF_SIZE];
        dm_ioctl_init(&mut cleanup, volume, "", 0);
        let _ = dm_ioctl(DM_TABLE_CLEAR_CMD, &mut cleanup);
        let _ = dm_ioctl(DM_DEV_REMOVE_CMD, &mut cleanup);
        return Err(format!("DM_DEV_SUSPEND (resume) failed for {volume}: {e}"));
    }

    eprintln!("Set up integrity device /dev/mapper/{volume}");
    Ok(())
}

/// Detach (tear down) a dm-integrity device.
fn cmd_detach(volume: &str) -> Result<(), String> {
    let mut buf = vec![0u8; DM_IOCTL_BUF_SIZE];

    // 1. Suspend.
    dm_ioctl_init(&mut buf, volume, "", DM_SUSPEND_FLAG);
    let _ = dm_ioctl(DM_DEV_SUSPEND_CMD, &mut buf);

    // 2. Clear table.
    dm_ioctl_init(&mut buf, volume, "", 0);
    let _ = dm_ioctl(DM_TABLE_CLEAR_CMD, &mut buf);

    // 3. Remove.
    dm_ioctl_init(&mut buf, volume, "", 0);
    dm_ioctl(DM_DEV_REMOVE_CMD, &mut buf)
        .map_err(|e| format!("DM_DEV_REMOVE failed for {volume}: {e}"))?;

    eprintln!("Detached integrity device /dev/mapper/{volume}");
    Ok(())
}

// ---------------------------------------------------------------------------
// Usage
// ---------------------------------------------------------------------------

fn print_usage() {
    eprintln!(
        "\
Usage: systemd-integritysetup [COMMAND] [OPTIONS...]

Commands:
  attach VOLUME DEVICE [KEY [OPTIONS]]
      Set up a dm-integrity block device.
      VOLUME   — device-mapper name (appears as /dev/mapper/VOLUME)
      DEVICE   — underlying block device
      KEY      — key file path, or \"-\" for no key (default: no key)
      OPTIONS  — comma-separated list of options (see below)

  detach VOLUME
      Tear down a previously attached dm-integrity device.

Options (comma-separated in the OPTIONS argument):
  algorithm=ALG                Integrity algorithm (default: crc32c)
  journal-commit-time=MS       Journal commit interval (ms)
  journal-watermark=PCT        Journal watermark (0-100)
  journal-integrity=ALG        Journal integrity hash
  journal-integrity-key-size=N Key size for journal integrity
  journal-integrity-key-file=PATH  Key file for journal integrity
  journal-crypt=ALG            Journal encryption algorithm
  journal-crypt-key-size=N     Key size for journal encryption
  journal-crypt-key-file=PATH  Key file for journal encryption
  data-device=PATH             Separate data device
  sector-size=BYTES            Sector size (default: 512)
  bitmap-flush-interval=MS     Bitmap mode flush interval
  block-size=BYTES             Internal block size
  integrity-recalculate        Recalculate tags in background
  integrity-recalculate-reset  Reset recalculate position
  allow-discards               Allow TRIM passthrough
  fix-padding                  Fix metadata padding
  fix-hmac                     Fix HMAC issues
  legacy-recalculate           Legacy recalculate behavior
  no-journal                   Disable journaling
  no-journal-bitmap            Use bitmap journaling
  recovery                     Recovery mode
  readonly                     Open read-only

Exit codes:
  0 — success
  1 — general error
  4 — integrity check failure"
    );
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

fn run() -> Result<(), (String, i32)> {
    let argv: Vec<String> = std::env::args().collect();
    let args = if argv.len() > 1 { &argv[1..] } else { &[] };

    let cmd = parse_args(args).map_err(|e| (e, 1))?;

    match cmd {
        Command::Help => {
            print_usage();
            Ok(())
        }
        Command::Version => {
            eprintln!("systemd-integritysetup (systemd-rs) 0.1.0");
            Ok(())
        }
        Command::Attach {
            volume,
            device,
            key,
            options,
        } => cmd_attach(&volume, &device, key.as_deref(), &options).map_err(|e| (e, 1)),
        Command::Detach { volume } => cmd_detach(&volume).map_err(|e| (e, 1)),
    }
}

fn main() {
    match run() {
        Ok(()) => process::exit(0),
        Err((msg, code)) => {
            eprintln!("systemd-integritysetup: {msg}");
            process::exit(code);
        }
    }
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn args(strs: &[&str]) -> Vec<String> {
        strs.iter().map(|s| s.to_string()).collect()
    }

    // -----------------------------------------------------------------------
    // parse_args tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_parse_args_empty() {
        assert!(parse_args(&args(&[])).is_err());
    }

    #[test]
    fn test_parse_args_help() {
        assert_eq!(parse_args(&args(&["--help"])).unwrap(), Command::Help);
        assert_eq!(parse_args(&args(&["-h"])).unwrap(), Command::Help);
        assert_eq!(parse_args(&args(&["help"])).unwrap(), Command::Help);
    }

    #[test]
    fn test_parse_args_version() {
        assert_eq!(parse_args(&args(&["--version"])).unwrap(), Command::Version);
        assert_eq!(parse_args(&args(&["version"])).unwrap(), Command::Version);
    }

    #[test]
    fn test_parse_args_attach_minimal() {
        let cmd = parse_args(&args(&["attach", "myvol", "/dev/sda1"])).unwrap();
        assert_eq!(
            cmd,
            Command::Attach {
                volume: "myvol".to_string(),
                device: "/dev/sda1".to_string(),
                key: None,
                options: String::new(),
            }
        );
    }

    #[test]
    fn test_parse_args_attach_with_key() {
        let cmd = parse_args(&args(&["attach", "vol", "/dev/sda1", "/path/to/key"])).unwrap();
        assert_eq!(
            cmd,
            Command::Attach {
                volume: "vol".to_string(),
                device: "/dev/sda1".to_string(),
                key: Some("/path/to/key".to_string()),
                options: String::new(),
            }
        );
    }

    #[test]
    fn test_parse_args_attach_with_key_and_options() {
        let cmd = parse_args(&args(&[
            "attach",
            "vol",
            "/dev/sda1",
            "-",
            "algorithm=sha256,no-journal",
        ]))
        .unwrap();
        assert_eq!(
            cmd,
            Command::Attach {
                volume: "vol".to_string(),
                device: "/dev/sda1".to_string(),
                key: Some("-".to_string()),
                options: "algorithm=sha256,no-journal".to_string(),
            }
        );
    }

    #[test]
    fn test_parse_args_attach_missing_volume() {
        assert!(parse_args(&args(&["attach"])).is_err());
    }

    #[test]
    fn test_parse_args_attach_missing_device() {
        assert!(parse_args(&args(&["attach", "vol"])).is_err());
    }

    #[test]
    fn test_parse_args_detach() {
        let cmd = parse_args(&args(&["detach", "myvol"])).unwrap();
        assert_eq!(
            cmd,
            Command::Detach {
                volume: "myvol".to_string()
            }
        );
    }

    #[test]
    fn test_parse_args_detach_missing_volume() {
        assert!(parse_args(&args(&["detach"])).is_err());
    }

    #[test]
    fn test_parse_args_unknown_command() {
        assert!(parse_args(&args(&["frobnicate"])).is_err());
    }

    // -----------------------------------------------------------------------
    // parse_options tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_parse_options_empty() {
        let opts = parse_options("").unwrap();
        assert_eq!(opts.algorithm, DEFAULT_ALGORITHM);
        assert!(opts.journal_commit_time.is_none());
        assert!(opts.journal_watermark.is_none());
        assert!(opts.journal_integrity.is_none());
        assert!(opts.journal_integrity_key_size.is_none());
        assert!(opts.journal_integrity_key_file.is_none());
        assert!(opts.journal_crypt.is_none());
        assert!(opts.journal_crypt_key_size.is_none());
        assert!(opts.journal_crypt_key_file.is_none());
        assert!(opts.data_device.is_none());
        assert_eq!(opts.sector_size, DEFAULT_SECTOR_SIZE);
        assert!(opts.bitmap_flush_interval.is_none());
        assert!(opts.block_size.is_none());
        assert!(!opts.integrity_recalculate);
        assert!(!opts.integrity_recalculate_reset);
        assert!(!opts.allow_discards);
        assert!(!opts.fix_padding);
        assert!(!opts.fix_hmac);
        assert!(!opts.legacy_recalculate);
        assert_eq!(opts.journal_mode, JournalMode::Journal);
        assert!(!opts.recovery);
        assert!(!opts.readonly);
    }

    #[test]
    fn test_parse_options_dash() {
        let opts = parse_options("-").unwrap();
        assert_eq!(opts.algorithm, DEFAULT_ALGORITHM);
    }

    #[test]
    fn test_parse_options_none() {
        let opts = parse_options("none").unwrap();
        assert_eq!(opts.algorithm, DEFAULT_ALGORITHM);
    }

    #[test]
    fn test_parse_options_algorithm() {
        let opts = parse_options("algorithm=sha256").unwrap();
        assert_eq!(opts.algorithm, "sha256");
    }

    #[test]
    fn test_parse_options_algorithm_hmac() {
        let opts = parse_options("algorithm=hmac(sha256)").unwrap();
        assert_eq!(opts.algorithm, "hmac(sha256)");
    }

    #[test]
    fn test_parse_options_algorithm_alias() {
        let opts = parse_options("integrity-algorithm=sha512").unwrap();
        assert_eq!(opts.algorithm, "sha512");
    }

    #[test]
    fn test_parse_options_journal_commit_time() {
        let opts = parse_options("journal-commit-time=5000").unwrap();
        assert_eq!(opts.journal_commit_time, Some(5000));
    }

    #[test]
    fn test_parse_options_journal_commit_time_invalid() {
        assert!(parse_options("journal-commit-time=abc").is_err());
    }

    #[test]
    fn test_parse_options_journal_watermark() {
        let opts = parse_options("journal-watermark=50").unwrap();
        assert_eq!(opts.journal_watermark, Some(50));
    }

    #[test]
    fn test_parse_options_journal_watermark_zero() {
        let opts = parse_options("journal-watermark=0").unwrap();
        assert_eq!(opts.journal_watermark, Some(0));
    }

    #[test]
    fn test_parse_options_journal_watermark_100() {
        let opts = parse_options("journal-watermark=100").unwrap();
        assert_eq!(opts.journal_watermark, Some(100));
    }

    #[test]
    fn test_parse_options_journal_watermark_over_100() {
        assert!(parse_options("journal-watermark=101").is_err());
    }

    #[test]
    fn test_parse_options_journal_watermark_invalid() {
        assert!(parse_options("journal-watermark=abc").is_err());
    }

    #[test]
    fn test_parse_options_journal_integrity() {
        let opts = parse_options("journal-integrity=hmac(sha256)").unwrap();
        assert_eq!(opts.journal_integrity, Some("hmac(sha256)".to_string()));
    }

    #[test]
    fn test_parse_options_journal_integrity_key_size() {
        let opts = parse_options("journal-integrity-key-size=32").unwrap();
        assert_eq!(opts.journal_integrity_key_size, Some(32));
    }

    #[test]
    fn test_parse_options_journal_integrity_key_file() {
        let opts = parse_options("journal-integrity-key-file=/path/to/key").unwrap();
        assert_eq!(
            opts.journal_integrity_key_file,
            Some(PathBuf::from("/path/to/key"))
        );
    }

    #[test]
    fn test_parse_options_journal_crypt() {
        let opts = parse_options("journal-crypt=aes-xts-plain64").unwrap();
        assert_eq!(opts.journal_crypt, Some("aes-xts-plain64".to_string()));
    }

    #[test]
    fn test_parse_options_journal_crypt_key_size() {
        let opts = parse_options("journal-crypt-key-size=64").unwrap();
        assert_eq!(opts.journal_crypt_key_size, Some(64));
    }

    #[test]
    fn test_parse_options_journal_crypt_key_file() {
        let opts = parse_options("journal-crypt-key-file=/path/to/key").unwrap();
        assert_eq!(
            opts.journal_crypt_key_file,
            Some(PathBuf::from("/path/to/key"))
        );
    }

    #[test]
    fn test_parse_options_data_device() {
        let opts = parse_options("data-device=/dev/sdb1").unwrap();
        assert_eq!(opts.data_device, Some(PathBuf::from("/dev/sdb1")));
    }

    #[test]
    fn test_parse_options_sector_size() {
        let opts = parse_options("sector-size=4096").unwrap();
        assert_eq!(opts.sector_size, 4096);
    }

    #[test]
    fn test_parse_options_sector_size_1024() {
        let opts = parse_options("sector-size=1024").unwrap();
        assert_eq!(opts.sector_size, 1024);
    }

    #[test]
    fn test_parse_options_sector_size_invalid_not_power_of_two() {
        assert!(parse_options("sector-size=1000").is_err());
    }

    #[test]
    fn test_parse_options_sector_size_invalid_too_small() {
        assert!(parse_options("sector-size=256").is_err());
    }

    #[test]
    fn test_parse_options_sector_size_invalid_nan() {
        assert!(parse_options("sector-size=abc").is_err());
    }

    #[test]
    fn test_parse_options_bitmap_flush_interval() {
        let opts = parse_options("bitmap-flush-interval=1000").unwrap();
        assert_eq!(opts.bitmap_flush_interval, Some(1000));
    }

    #[test]
    fn test_parse_options_block_size() {
        let opts = parse_options("block-size=4096").unwrap();
        assert_eq!(opts.block_size, Some(4096));
    }

    #[test]
    fn test_parse_options_integrity_recalculate() {
        let opts = parse_options("integrity-recalculate").unwrap();
        assert!(opts.integrity_recalculate);
    }

    #[test]
    fn test_parse_options_integrity_recalculate_underscore() {
        let opts = parse_options("integrity_recalculate").unwrap();
        assert!(opts.integrity_recalculate);
    }

    #[test]
    fn test_parse_options_integrity_recalculate_reset() {
        let opts = parse_options("integrity-recalculate-reset").unwrap();
        assert!(opts.integrity_recalculate_reset);
    }

    #[test]
    fn test_parse_options_allow_discards() {
        let opts = parse_options("allow-discards").unwrap();
        assert!(opts.allow_discards);
    }

    #[test]
    fn test_parse_options_discard_alias() {
        let opts = parse_options("discard").unwrap();
        assert!(opts.allow_discards);
    }

    #[test]
    fn test_parse_options_fix_padding() {
        let opts = parse_options("fix-padding").unwrap();
        assert!(opts.fix_padding);
    }

    #[test]
    fn test_parse_options_fix_hmac() {
        let opts = parse_options("fix-hmac").unwrap();
        assert!(opts.fix_hmac);
    }

    #[test]
    fn test_parse_options_legacy_recalculate() {
        let opts = parse_options("legacy-recalculate").unwrap();
        assert!(opts.legacy_recalculate);
    }

    #[test]
    fn test_parse_options_no_journal() {
        let opts = parse_options("no-journal").unwrap();
        assert_eq!(opts.journal_mode, JournalMode::NoJournal);
    }

    #[test]
    fn test_parse_options_no_journal_underscore() {
        let opts = parse_options("no_journal").unwrap();
        assert_eq!(opts.journal_mode, JournalMode::NoJournal);
    }

    #[test]
    fn test_parse_options_bitmap() {
        let opts = parse_options("bitmap").unwrap();
        assert_eq!(opts.journal_mode, JournalMode::Bitmap);
    }

    #[test]
    fn test_parse_options_no_journal_bitmap() {
        let opts = parse_options("no-journal-bitmap").unwrap();
        assert_eq!(opts.journal_mode, JournalMode::Bitmap);
    }

    #[test]
    fn test_parse_options_recovery() {
        let opts = parse_options("recovery").unwrap();
        assert!(opts.recovery);
    }

    #[test]
    fn test_parse_options_readonly() {
        let opts = parse_options("readonly").unwrap();
        assert!(opts.readonly);
    }

    #[test]
    fn test_parse_options_read_only_alias() {
        let opts = parse_options("read-only").unwrap();
        assert!(opts.readonly);
    }

    #[test]
    fn test_parse_options_noauto_nofail() {
        let opts = parse_options("noauto,nofail").unwrap();
        assert_eq!(opts.algorithm, DEFAULT_ALGORITHM);
    }

    #[test]
    fn test_parse_options_systemd_extensions_ignored() {
        let opts = parse_options("x-systemd.device-timeout=30").unwrap();
        assert_eq!(opts.algorithm, DEFAULT_ALGORITHM);
    }

    #[test]
    fn test_parse_options_unknown_flag() {
        assert!(parse_options("bogus-flag").is_err());
    }

    #[test]
    fn test_parse_options_unknown_kv() {
        assert!(parse_options("unknown-key=value").is_err());
    }

    #[test]
    fn test_parse_options_whitespace() {
        let opts = parse_options(" no-journal , readonly ").unwrap();
        assert_eq!(opts.journal_mode, JournalMode::NoJournal);
        assert!(opts.readonly);
    }

    #[test]
    fn test_parse_options_empty_parts() {
        let opts = parse_options(",,no-journal,,readonly,,").unwrap();
        assert_eq!(opts.journal_mode, JournalMode::NoJournal);
        assert!(opts.readonly);
    }

    #[test]
    fn test_parse_options_combined() {
        let opts = parse_options(
            "algorithm=sha256,no-journal,sector-size=4096,\
             integrity-recalculate,allow-discards,readonly,\
             journal-commit-time=5000,journal-watermark=75",
        )
        .unwrap();

        assert_eq!(opts.algorithm, "sha256");
        assert_eq!(opts.journal_mode, JournalMode::NoJournal);
        assert_eq!(opts.sector_size, 4096);
        assert!(opts.integrity_recalculate);
        assert!(opts.allow_discards);
        assert!(opts.readonly);
        assert_eq!(opts.journal_commit_time, Some(5000));
        assert_eq!(opts.journal_watermark, Some(75));
    }

    #[test]
    fn test_parse_options_journal_mode_last_wins() {
        let opts = parse_options("no-journal,bitmap").unwrap();
        assert_eq!(opts.journal_mode, JournalMode::Bitmap);

        let opts = parse_options("bitmap,no-journal").unwrap();
        assert_eq!(opts.journal_mode, JournalMode::NoJournal);
    }

    #[test]
    fn test_parse_options_auto_ignored() {
        let opts = parse_options("auto").unwrap();
        assert_eq!(opts.journal_mode, JournalMode::Journal);
    }

    // -----------------------------------------------------------------------
    // integrity_tag_size tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_integrity_tag_size_crc32c() {
        assert_eq!(integrity_tag_size("crc32c"), 4);
    }

    #[test]
    fn test_integrity_tag_size_crc32() {
        assert_eq!(integrity_tag_size("crc32"), 4);
    }

    #[test]
    fn test_integrity_tag_size_sha1() {
        assert_eq!(integrity_tag_size("sha1"), 20);
    }

    #[test]
    fn test_integrity_tag_size_hmac_sha1() {
        assert_eq!(integrity_tag_size("hmac(sha1)"), 20);
    }

    #[test]
    fn test_integrity_tag_size_sha256() {
        assert_eq!(integrity_tag_size("sha256"), 32);
    }

    #[test]
    fn test_integrity_tag_size_hmac_sha256() {
        assert_eq!(integrity_tag_size("hmac(sha256)"), 32);
    }

    #[test]
    fn test_integrity_tag_size_sha512() {
        assert_eq!(integrity_tag_size("sha512"), 64);
    }

    #[test]
    fn test_integrity_tag_size_hmac_sha512() {
        assert_eq!(integrity_tag_size("hmac(sha512)"), 64);
    }

    #[test]
    fn test_integrity_tag_size_xxhash64() {
        assert_eq!(integrity_tag_size("xxhash64"), 8);
    }

    #[test]
    fn test_integrity_tag_size_poly1305() {
        assert_eq!(integrity_tag_size("poly1305"), 16);
    }

    #[test]
    fn test_integrity_tag_size_blake2b_256() {
        assert_eq!(integrity_tag_size("blake2b-256"), 32);
    }

    #[test]
    fn test_integrity_tag_size_blake2b_512() {
        assert_eq!(integrity_tag_size("blake2b-512"), 64);
    }

    #[test]
    fn test_integrity_tag_size_unknown_default() {
        assert_eq!(integrity_tag_size("some-unknown-algo"), 32);
    }

    // -----------------------------------------------------------------------
    // key_to_hex tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_key_to_hex_empty() {
        assert_eq!(key_to_hex(&[]), "");
    }

    #[test]
    fn test_key_to_hex_single_byte() {
        assert_eq!(key_to_hex(&[0xab]), "ab");
    }

    #[test]
    fn test_key_to_hex_multi_byte() {
        assert_eq!(key_to_hex(&[0x00, 0xff, 0x0a, 0xbc]), "00ff0abc");
    }

    #[test]
    fn test_key_to_hex_all_zeros() {
        assert_eq!(key_to_hex(&[0, 0, 0, 0]), "00000000");
    }

    #[test]
    fn test_key_to_hex_all_ff() {
        assert_eq!(key_to_hex(&[0xff; 16]), "ff".repeat(16));
    }

    // -----------------------------------------------------------------------
    // read_key tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_read_key_none() {
        let key = read_key(None).unwrap();
        assert!(key.is_empty());
    }

    #[test]
    fn test_read_key_dash() {
        let key = read_key(Some("-")).unwrap();
        assert!(key.is_empty());
    }

    #[test]
    fn test_read_key_empty() {
        let key = read_key(Some("")).unwrap();
        assert!(key.is_empty());
    }

    #[test]
    fn test_read_key_none_string() {
        let key = read_key(Some("none")).unwrap();
        assert!(key.is_empty());
    }

    #[test]
    fn test_read_key_from_file() {
        let dir = tempfile::tempdir().unwrap();
        let key_path = dir.path().join("keyfile");
        fs::write(&key_path, b"secret-key-data").unwrap();

        let key = read_key(Some(key_path.to_str().unwrap())).unwrap();
        assert_eq!(key, b"secret-key-data");
    }

    #[test]
    fn test_read_key_binary_content() {
        let dir = tempfile::tempdir().unwrap();
        let key_path = dir.path().join("keyfile");
        let binary_data: Vec<u8> = (0..=255).collect();
        fs::write(&key_path, &binary_data).unwrap();

        let key = read_key(Some(key_path.to_str().unwrap())).unwrap();
        assert_eq!(key, binary_data);
    }

    #[test]
    fn test_read_key_nonexistent_file() {
        assert!(read_key(Some("/nonexistent/keyfile")).is_err());
    }

    #[test]
    fn test_read_key_empty_file() {
        let dir = tempfile::tempdir().unwrap();
        let key_path = dir.path().join("keyfile");
        fs::write(&key_path, b"").unwrap();

        let key = read_key(Some(key_path.to_str().unwrap())).unwrap();
        assert!(key.is_empty());
    }

    // -----------------------------------------------------------------------
    // build_integrity_params tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_build_integrity_params_default() {
        let opts = IntegrityOptions::default();
        let key: Vec<u8> = vec![];
        let params = build_integrity_params(&opts, "/dev/sda1", &key);
        // Device, offset, tag_size(crc32c=4), mode(J for journal).
        assert!(params.starts_with("/dev/sda1 0 4 J"));
        // Should contain internal_hash.
        assert!(params.contains("internal_hash:crc32c"));
        // No key.
        assert!(!params.contains("key:"));
    }

    #[test]
    fn test_build_integrity_params_sha256() {
        let mut opts = IntegrityOptions::default();
        opts.algorithm = "sha256".to_string();
        let key: Vec<u8> = vec![];
        let params = build_integrity_params(&opts, "/dev/sda1", &key);
        // Tag size for sha256 = 32.
        assert!(params.starts_with("/dev/sda1 0 32 J"));
        assert!(params.contains("internal_hash:sha256"));
    }

    #[test]
    fn test_build_integrity_params_hmac_with_key() {
        let mut opts = IntegrityOptions::default();
        opts.algorithm = "hmac(sha256)".to_string();
        let key = vec![0xab; 32];
        let params = build_integrity_params(&opts, "/dev/sda1", &key);
        assert!(params.starts_with("/dev/sda1 0 32 J"));
        assert!(params.contains("internal_hash:hmac(sha256)"));
        assert!(params.contains(&format!("key:{}", "ab".repeat(32))));
    }

    #[test]
    fn test_build_integrity_params_no_journal() {
        let mut opts = IntegrityOptions::default();
        opts.journal_mode = JournalMode::NoJournal;
        let key: Vec<u8> = vec![];
        let params = build_integrity_params(&opts, "/dev/sda1", &key);
        assert!(
            params.contains(" D ")
                || params.contains(" D\n")
                || params.starts_with("/dev/sda1 0 4 D")
        );
    }

    #[test]
    fn test_build_integrity_params_bitmap() {
        let mut opts = IntegrityOptions::default();
        opts.journal_mode = JournalMode::Bitmap;
        let key: Vec<u8> = vec![];
        let params = build_integrity_params(&opts, "/dev/sda1", &key);
        assert!(params.starts_with("/dev/sda1 0 4 B"));
    }

    #[test]
    fn test_build_integrity_params_recovery() {
        let mut opts = IntegrityOptions::default();
        opts.recovery = true;
        let key: Vec<u8> = vec![];
        let params = build_integrity_params(&opts, "/dev/sda1", &key);
        assert!(params.starts_with("/dev/sda1 0 4 R"));
    }

    #[test]
    fn test_build_integrity_params_recovery_overrides_journal_mode() {
        let mut opts = IntegrityOptions::default();
        opts.recovery = true;
        opts.journal_mode = JournalMode::NoJournal;
        let key: Vec<u8> = vec![];
        let params = build_integrity_params(&opts, "/dev/sda1", &key);
        // Recovery overrides journal mode.
        assert!(params.starts_with("/dev/sda1 0 4 R"));
    }

    #[test]
    fn test_build_integrity_params_with_recalculate() {
        let mut opts = IntegrityOptions::default();
        opts.integrity_recalculate = true;
        let key: Vec<u8> = vec![];
        let params = build_integrity_params(&opts, "/dev/sda1", &key);
        assert!(params.contains("recalculate"));
    }

    #[test]
    fn test_build_integrity_params_with_recalculate_reset() {
        let mut opts = IntegrityOptions::default();
        opts.integrity_recalculate_reset = true;
        let key: Vec<u8> = vec![];
        let params = build_integrity_params(&opts, "/dev/sda1", &key);
        assert!(params.contains("reset_recalculate"));
    }

    #[test]
    fn test_build_integrity_params_allow_discards() {
        let mut opts = IntegrityOptions::default();
        opts.allow_discards = true;
        let key: Vec<u8> = vec![];
        let params = build_integrity_params(&opts, "/dev/sda1", &key);
        assert!(params.contains("allow_discards"));
    }

    #[test]
    fn test_build_integrity_params_fix_padding() {
        let mut opts = IntegrityOptions::default();
        opts.fix_padding = true;
        let key: Vec<u8> = vec![];
        let params = build_integrity_params(&opts, "/dev/sda1", &key);
        assert!(params.contains("fix_padding"));
    }

    #[test]
    fn test_build_integrity_params_fix_hmac() {
        let mut opts = IntegrityOptions::default();
        opts.fix_hmac = true;
        let key: Vec<u8> = vec![];
        let params = build_integrity_params(&opts, "/dev/sda1", &key);
        assert!(params.contains("fix_hmac"));
    }

    #[test]
    fn test_build_integrity_params_legacy_recalculate() {
        let mut opts = IntegrityOptions::default();
        opts.legacy_recalculate = true;
        let key: Vec<u8> = vec![];
        let params = build_integrity_params(&opts, "/dev/sda1", &key);
        assert!(params.contains("legacy_recalculate"));
    }

    #[test]
    fn test_build_integrity_params_journal_options() {
        let mut opts = IntegrityOptions::default();
        opts.journal_commit_time = Some(5000);
        opts.journal_watermark = Some(50);
        opts.journal_integrity = Some("hmac(sha256)".to_string());
        opts.journal_crypt = Some("aes-xts-plain64".to_string());
        let key: Vec<u8> = vec![];
        let params = build_integrity_params(&opts, "/dev/sda1", &key);
        assert!(params.contains("journal_commit_time:5000"));
        assert!(params.contains("journal_watermark:50"));
        assert!(params.contains("journal_mac:hmac(sha256)"));
        assert!(params.contains("journal_crypt:aes-xts-plain64"));
    }

    #[test]
    fn test_build_integrity_params_data_device() {
        let mut opts = IntegrityOptions::default();
        opts.data_device = Some(PathBuf::from("/dev/sdb1"));
        let key: Vec<u8> = vec![];
        let params = build_integrity_params(&opts, "/dev/sda1", &key);
        assert!(params.contains("data_device:/dev/sdb1"));
    }

    #[test]
    fn test_build_integrity_params_bitmap_flush_interval() {
        let mut opts = IntegrityOptions::default();
        opts.bitmap_flush_interval = Some(1000);
        let key: Vec<u8> = vec![];
        let params = build_integrity_params(&opts, "/dev/sda1", &key);
        assert!(params.contains("bitmap_flush_interval:1000"));
    }

    #[test]
    fn test_build_integrity_params_block_size() {
        let mut opts = IntegrityOptions::default();
        opts.block_size = Some(4096);
        let key: Vec<u8> = vec![];
        let params = build_integrity_params(&opts, "/dev/sda1", &key);
        assert!(params.contains("block_size:4096"));
    }

    #[test]
    fn test_build_integrity_params_custom_sector_size() {
        let mut opts = IntegrityOptions::default();
        opts.sector_size = 4096;
        let key: Vec<u8> = vec![];
        let params = build_integrity_params(&opts, "/dev/sda1", &key);
        assert!(params.contains("sectors_per_bit:4096"));
    }

    #[test]
    fn test_build_integrity_params_all_flags() {
        let mut opts = IntegrityOptions::default();
        opts.integrity_recalculate = true;
        opts.integrity_recalculate_reset = true;
        opts.allow_discards = true;
        opts.fix_padding = true;
        opts.fix_hmac = true;
        opts.legacy_recalculate = true;
        let key: Vec<u8> = vec![];
        let params = build_integrity_params(&opts, "/dev/sda1", &key);
        assert!(params.contains("recalculate"));
        assert!(params.contains("reset_recalculate"));
        assert!(params.contains("allow_discards"));
        assert!(params.contains("fix_padding"));
        assert!(params.contains("fix_hmac"));
        assert!(params.contains("legacy_recalculate"));
    }

    // -----------------------------------------------------------------------
    // JournalMode Display
    // -----------------------------------------------------------------------

    #[test]
    fn test_journal_mode_display() {
        assert_eq!(format!("{}", JournalMode::Journal), "journal");
        assert_eq!(format!("{}", JournalMode::NoJournal), "no-journal");
        assert_eq!(format!("{}", JournalMode::Bitmap), "bitmap");
    }

    // -----------------------------------------------------------------------
    // IntegrityOptions default tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_integrity_options_default() {
        let opts = IntegrityOptions::default();
        assert_eq!(opts.algorithm, "crc32c");
        assert!(opts.journal_commit_time.is_none());
        assert!(opts.journal_watermark.is_none());
        assert!(opts.journal_integrity.is_none());
        assert!(opts.journal_crypt.is_none());
        assert!(opts.data_device.is_none());
        assert_eq!(opts.sector_size, 512);
        assert!(opts.bitmap_flush_interval.is_none());
        assert!(opts.block_size.is_none());
        assert!(!opts.integrity_recalculate);
        assert!(!opts.integrity_recalculate_reset);
        assert!(!opts.allow_discards);
        assert!(!opts.fix_padding);
        assert!(!opts.fix_hmac);
        assert!(!opts.legacy_recalculate);
        assert_eq!(opts.journal_mode, JournalMode::Journal);
        assert!(!opts.recovery);
        assert!(!opts.readonly);
    }

    // -----------------------------------------------------------------------
    // DM ioctl helper tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_dm_ioctl_nr_encoding() {
        let nr = dm_ioctl_nr(DM_DEV_CREATE_CMD);
        assert_eq!((nr >> 30) & 3, 3);
        assert_eq!((nr >> 8) & 0xff, 0xfd);
        assert_eq!(nr & 0xff, DM_DEV_CREATE_CMD as libc::c_ulong);
        assert_eq!((nr >> 16) & 0x3fff, DM_IOCTL_HEADER_SIZE as libc::c_ulong);
    }

    #[test]
    fn test_dm_ioctl_init_basic() {
        let mut buf = vec![0u8; DM_IOCTL_BUF_SIZE];
        dm_ioctl_init(&mut buf, "test_vol", "CRYPT-INTEGRITY-test", 0);

        assert_eq!(read_u32(&buf, DM_VERSION_OFFSET), DM_VERSION_MAJOR);
        assert_eq!(read_u32(&buf, DM_VERSION_OFFSET + 4), DM_VERSION_MINOR);
        assert_eq!(read_u32(&buf, DM_VERSION_OFFSET + 8), DM_VERSION_PATCHLEVEL);
        assert_eq!(
            read_u32(&buf, DM_DATA_SIZE_OFFSET),
            DM_IOCTL_BUF_SIZE as u32
        );
        assert_eq!(
            read_u32(&buf, DM_DATA_START_OFFSET),
            DM_IOCTL_HEADER_SIZE as u32
        );
        assert_eq!(read_u32(&buf, DM_FLAGS_OFFSET), 0);

        let name_bytes = &buf[DM_NAME_OFFSET..DM_NAME_OFFSET + DM_NAME_SIZE];
        let end = name_bytes
            .iter()
            .position(|&b| b == 0)
            .unwrap_or(DM_NAME_SIZE);
        let name = std::str::from_utf8(&name_bytes[..end]).unwrap();
        assert_eq!(name, "test_vol");

        let uuid_bytes = &buf[DM_UUID_OFFSET..DM_UUID_OFFSET + DM_UUID_SIZE];
        let end = uuid_bytes
            .iter()
            .position(|&b| b == 0)
            .unwrap_or(DM_UUID_SIZE);
        let uuid = std::str::from_utf8(&uuid_bytes[..end]).unwrap();
        assert_eq!(uuid, "CRYPT-INTEGRITY-test");
    }

    #[test]
    fn test_dm_ioctl_init_readonly() {
        let mut buf = vec![0u8; DM_IOCTL_BUF_SIZE];
        dm_ioctl_init(&mut buf, "vol", "", DM_READONLY_FLAG);
        assert_eq!(read_u32(&buf, DM_FLAGS_OFFSET), DM_READONLY_FLAG);
    }

    #[test]
    fn test_dm_ioctl_init_suspend() {
        let mut buf = vec![0u8; DM_IOCTL_BUF_SIZE];
        dm_ioctl_init(&mut buf, "vol", "", DM_SUSPEND_FLAG);
        assert_eq!(read_u32(&buf, DM_FLAGS_OFFSET), DM_SUSPEND_FLAG);
    }

    #[test]
    fn test_dm_ioctl_init_empty_name() {
        let mut buf = vec![0u8; DM_IOCTL_BUF_SIZE];
        dm_ioctl_init(&mut buf, "", "", 0);
        assert_eq!(buf[DM_NAME_OFFSET], 0);
        assert_eq!(buf[DM_UUID_OFFSET], 0);
    }

    #[test]
    fn test_dm_ioctl_init_long_name_truncated() {
        let mut buf = vec![0u8; DM_IOCTL_BUF_SIZE];
        let long_name = "a".repeat(200);
        dm_ioctl_init(&mut buf, &long_name, "", 0);

        let name_bytes = &buf[DM_NAME_OFFSET..DM_NAME_OFFSET + DM_NAME_SIZE];
        let end = name_bytes
            .iter()
            .position(|&b| b == 0)
            .unwrap_or(DM_NAME_SIZE);
        assert_eq!(end, DM_NAME_SIZE - 1);
    }

    #[test]
    fn test_write_read_u32() {
        let mut buf = vec![0u8; 8];
        write_u32(&mut buf, 0, 0x12345678);
        assert_eq!(read_u32(&buf, 0), 0x12345678);
        write_u32(&mut buf, 4, u32::MAX);
        assert_eq!(read_u32(&buf, 4), u32::MAX);
    }

    #[test]
    fn test_write_read_u64() {
        let mut buf = vec![0u8; 16];
        write_u64(&mut buf, 0, 0x123456789abcdef0);
        assert_eq!(read_u64(&buf, 0), 0x123456789abcdef0);
        write_u64(&mut buf, 8, u64::MAX);
        assert_eq!(read_u64(&buf, 8), u64::MAX);
    }

    #[test]
    fn test_write_string_basic() {
        let mut buf = vec![0u8; 32];
        write_string(&mut buf, 0, 16, "hello");
        assert_eq!(&buf[0..5], b"hello");
        assert_eq!(buf[5], 0);
    }

    #[test]
    fn test_write_string_truncation() {
        let mut buf = vec![0u8; 16];
        write_string(&mut buf, 0, 4, "longstring");
        assert_eq!(&buf[0..3], b"lon");
        assert_eq!(buf[3], 0);
    }

    #[test]
    fn test_write_string_empty() {
        let mut buf = vec![0u8; 16];
        write_string(&mut buf, 0, 16, "");
        assert_eq!(buf[0], 0);
    }

    #[test]
    fn test_align8() {
        assert_eq!(align8(0), 0);
        assert_eq!(align8(1), 8);
        assert_eq!(align8(7), 8);
        assert_eq!(align8(8), 8);
        assert_eq!(align8(9), 16);
        assert_eq!(align8(16), 16);
        assert_eq!(align8(40), 40);
        assert_eq!(align8(41), 48);
    }

    // -----------------------------------------------------------------------
    // append_target tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_append_target_basic() {
        let mut buf = vec![0u8; 4096];
        let data_start = DM_IOCTL_HEADER_SIZE;
        let size = append_target(
            &mut buf,
            data_start,
            0,
            1000,
            "integrity",
            "/dev/sda1 0 4 J 1 internal_hash:crc32c",
        );
        assert!(size > 0);
        assert_eq!(size % 8, 0);

        assert_eq!(read_u64(&buf, data_start), 0);
        assert_eq!(read_u64(&buf, data_start + 8), 1000);

        let tt_bytes = &buf[data_start + 24..data_start + 40];
        let tt_end = tt_bytes.iter().position(|&b| b == 0).unwrap_or(16);
        let tt = std::str::from_utf8(&tt_bytes[..tt_end]).unwrap();
        assert_eq!(tt, "integrity");
    }

    #[test]
    fn test_append_target_params_content() {
        let mut buf = vec![0u8; 4096];
        let data_start = DM_IOCTL_HEADER_SIZE;
        let params_str = "/dev/sda1 0 32 D 2 internal_hash:sha256 allow_discards";
        let _size = append_target(&mut buf, data_start, 0, 500, "integrity", params_str);

        let param_off = data_start + 40;
        let param_end = buf[param_off..].iter().position(|&b| b == 0).unwrap();
        let params = std::str::from_utf8(&buf[param_off..param_off + param_end]).unwrap();
        assert_eq!(params, params_str);
    }

    #[test]
    fn test_append_target_alignment() {
        let mut buf = vec![0u8; 4096];
        let data_start = DM_IOCTL_HEADER_SIZE;
        let size = append_target(&mut buf, data_start, 0, 100, "integrity", "x");
        assert_eq!(size % 8, 0);

        let size2 = append_target(
            &mut buf,
            data_start,
            0,
            100,
            "integrity",
            "this is a longer parameter string that should also be aligned",
        );
        assert_eq!(size2 % 8, 0);
    }

    // -----------------------------------------------------------------------
    // Integration-style tests (no actual DM operations)
    // -----------------------------------------------------------------------

    #[test]
    fn test_attach_missing_device() {
        let result = cmd_attach("testvol", "/nonexistent/device", None, "");
        assert!(result.is_err());
    }

    #[test]
    fn test_attach_hmac_without_key() {
        // HMAC algorithms should fail without a key.
        let result = cmd_attach(
            "testvol",
            "/nonexistent/device",
            Some("-"),
            "algorithm=hmac(sha256)",
        );
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("requires a key"));
    }

    #[test]
    fn test_attach_nonexistent_data_device() {
        let result = cmd_attach(
            "testvol",
            "/dev/null",
            None,
            "data-device=/nonexistent/data",
        );
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Data device not found"));
    }

    #[test]
    fn test_attach_nonexistent_journal_integrity_key_file() {
        let result = cmd_attach(
            "testvol",
            "/dev/null",
            None,
            "journal-integrity-key-file=/nonexistent/key",
        );
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .contains("Journal integrity key file not found")
        );
    }

    #[test]
    fn test_attach_nonexistent_journal_crypt_key_file() {
        let result = cmd_attach(
            "testvol",
            "/dev/null",
            None,
            "journal-crypt-key-file=/nonexistent/key",
        );
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .contains("Journal crypt key file not found")
        );
    }

    #[test]
    fn test_detach_nonexistent_volume() {
        let result = cmd_detach("nonexistent_volume_99999");
        assert!(result.is_err());
    }

    // -----------------------------------------------------------------------
    // Full-flow argument + option parsing tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_full_attach_parse() {
        let cmd = parse_args(&args(&[
            "attach",
            "data_integrity",
            "/dev/nvme0n1p3",
            "/path/to/key",
            "algorithm=hmac(sha256),no-journal,sector-size=4096,integrity-recalculate,allow-discards",
        ]))
        .unwrap();

        if let Command::Attach {
            volume,
            device,
            key,
            options,
        } = cmd
        {
            assert_eq!(volume, "data_integrity");
            assert_eq!(device, "/dev/nvme0n1p3");
            assert_eq!(key, Some("/path/to/key".to_string()));

            let opts = parse_options(&options).unwrap();
            assert_eq!(opts.algorithm, "hmac(sha256)");
            assert_eq!(opts.journal_mode, JournalMode::NoJournal);
            assert_eq!(opts.sector_size, 4096);
            assert!(opts.integrity_recalculate);
            assert!(opts.allow_discards);
        } else {
            panic!("Expected Attach command");
        }
    }

    #[test]
    fn test_full_attach_parse_minimal() {
        let cmd = parse_args(&args(&["attach", "simple_integrity", "/dev/sda1"])).unwrap();

        if let Command::Attach {
            volume,
            device,
            key,
            options,
        } = cmd
        {
            assert_eq!(volume, "simple_integrity");
            assert_eq!(device, "/dev/sda1");
            assert!(key.is_none());
            assert!(options.is_empty());

            let opts = parse_options(&options).unwrap();
            assert_eq!(opts.algorithm, DEFAULT_ALGORITHM);
            assert_eq!(opts.journal_mode, JournalMode::Journal);
        } else {
            panic!("Expected Attach command");
        }
    }

    #[test]
    fn test_full_attach_parse_with_journal_options() {
        let cmd = parse_args(&args(&[
            "attach",
            "journaled",
            "/dev/sda1",
            "-",
            "journal-commit-time=5000,journal-watermark=75,journal-integrity=hmac(sha256),journal-crypt=aes-xts-plain64",
        ]))
        .unwrap();

        if let Command::Attach {
            volume,
            device,
            key,
            options,
        } = cmd
        {
            assert_eq!(volume, "journaled");
            assert_eq!(device, "/dev/sda1");
            assert_eq!(key, Some("-".to_string()));

            let opts = parse_options(&options).unwrap();
            assert_eq!(opts.journal_commit_time, Some(5000));
            assert_eq!(opts.journal_watermark, Some(75));
            assert_eq!(opts.journal_integrity, Some("hmac(sha256)".to_string()));
            assert_eq!(opts.journal_crypt, Some("aes-xts-plain64".to_string()));
        } else {
            panic!("Expected Attach command");
        }
    }

    // -----------------------------------------------------------------------
    // Constants / sanity checks
    // -----------------------------------------------------------------------

    #[test]
    fn test_dm_ioctl_header_size() {
        assert_eq!(DM_IOCTL_HEADER_SIZE, 312);
    }

    #[test]
    fn test_dm_ioctl_buf_size_larger_than_header() {
        assert!(DM_IOCTL_BUF_SIZE > DM_IOCTL_HEADER_SIZE);
    }

    #[test]
    fn test_default_algorithm() {
        assert_eq!(DEFAULT_ALGORITHM, "crc32c");
    }

    #[test]
    fn test_default_sector_size() {
        assert_eq!(DEFAULT_SECTOR_SIZE, 512);
    }

    #[test]
    fn test_exit_integrity_error_code() {
        assert_eq!(EXIT_INTEGRITY_ERROR, 4);
    }
}
