//! Journal storage — manages journal file storage.
//!
//! This module provides a simple append-only journal file format that stores
//! structured log entries on disk.  The format is designed to be:
//!
//! - **Append-only** — entries are always appended, never modified in place.
//! - **Crash-safe** — entries are delimited so that a truncated write at the
//!   end of the file can be detected and skipped during replay.
//! - **Queryable** — entries can be iterated, filtered by field values,
//!   and sought by timestamp or sequence number.
//! - **Rotatable** — when a journal file exceeds its configured size limit
//!   a new file is started and old files can be vacuumed.
//!
//! ## On-disk format
//!
//! Each journal file has the extension `.journal` and consists of:
//!
//! 1. A 64-byte file header (magic, version, file ID, head/tail seqnum, …)
//! 2. A sequence of **entry frames**, each structured as:
//!    - `u32` LE — frame length (excluding this u32 itself)
//!    - `u64` LE — realtime timestamp (µs since UNIX epoch)
//!    - `u64` LE — monotonic timestamp (µs since boot)
//!    - `u64` LE — sequence number
//!    - `u32` LE — number of fields
//!    - For each field:
//!      - `u16` LE — key length
//!      - `u32` LE — value length
//!      - key bytes (UTF-8)
//!      - value bytes (arbitrary)
//!    - `u32` LE — frame length again (trailer, for reverse iteration)
//!
//! The header is written once when the file is created and updated
//! (head/tail seqnum, entry count, file size) after each append via a
//! single atomic `pwrite`.

use super::entry::JournalEntry;
use std::collections::BTreeMap;
use std::fs::{self, File, OpenOptions};
use std::io::{self, BufReader, BufWriter, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Magic bytes at the start of every journal file.
const JOURNAL_MAGIC: &[u8; 8] = b"JRNL_RS\0";

/// Current format version.
const FORMAT_VERSION: u32 = 1;

/// Size of the file header in bytes.
const HEADER_SIZE: u64 = 64;

/// Default maximum size of a single journal file (64 MiB).
const DEFAULT_MAX_FILE_SIZE: u64 = 64 * 1024 * 1024;

/// Default maximum total disk usage across all journal files (512 MiB).
const DEFAULT_MAX_DISK_USAGE: u64 = 512 * 1024 * 1024;

/// Default maximum number of journal files to keep.
const DEFAULT_MAX_FILES: usize = 100;

// ---------------------------------------------------------------------------
// File header
// ---------------------------------------------------------------------------

/// On-disk file header (64 bytes).
#[derive(Debug, Clone)]
struct FileHeader {
    /// Magic bytes (`JRNL_RS\0`).
    magic: [u8; 8],
    /// Format version.
    version: u32,
    /// Unique file ID (random, for cursor addressing).
    file_id: u128,
    /// Sequence number of the first entry in this file.
    head_seqnum: u64,
    /// Sequence number of the last entry in this file.
    tail_seqnum: u64,
    /// Number of entries in this file.
    entry_count: u64,
    /// Total file size in bytes (updated after each append).
    file_size: u64,
}

impl FileHeader {
    fn new() -> Self {
        // Generate a random file ID from /dev/urandom or fallback
        let file_id = generate_random_u128();
        FileHeader {
            magic: *JOURNAL_MAGIC,
            version: FORMAT_VERSION,
            file_id,
            head_seqnum: 0,
            tail_seqnum: 0,
            entry_count: 0,
            file_size: HEADER_SIZE,
        }
    }

    fn serialize(&self) -> [u8; 64] {
        let mut buf = [0u8; 64];
        buf[0..8].copy_from_slice(&self.magic);
        buf[8..12].copy_from_slice(&self.version.to_le_bytes());
        buf[12..28].copy_from_slice(&self.file_id.to_le_bytes());
        buf[28..36].copy_from_slice(&self.head_seqnum.to_le_bytes());
        buf[36..44].copy_from_slice(&self.tail_seqnum.to_le_bytes());
        buf[44..52].copy_from_slice(&self.entry_count.to_le_bytes());
        buf[52..60].copy_from_slice(&self.file_size.to_le_bytes());
        // bytes 60..64 reserved (zeros)
        buf
    }

    fn deserialize(buf: &[u8; 64]) -> io::Result<Self> {
        let mut magic = [0u8; 8];
        magic.copy_from_slice(&buf[0..8]);
        if &magic != JOURNAL_MAGIC {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "invalid journal file magic",
            ));
        }

        let version = u32::from_le_bytes(buf[8..12].try_into().unwrap());
        if version != FORMAT_VERSION {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "unsupported journal format version {} (expected {})",
                    version, FORMAT_VERSION
                ),
            ));
        }

        Ok(FileHeader {
            magic,
            version,
            file_id: u128::from_le_bytes(buf[12..28].try_into().unwrap()),
            head_seqnum: u64::from_le_bytes(buf[28..36].try_into().unwrap()),
            tail_seqnum: u64::from_le_bytes(buf[36..44].try_into().unwrap()),
            entry_count: u64::from_le_bytes(buf[44..52].try_into().unwrap()),
            file_size: u64::from_le_bytes(buf[52..60].try_into().unwrap()),
        })
    }
}

// ---------------------------------------------------------------------------
// JournalFile — a single on-disk journal file
// ---------------------------------------------------------------------------

/// Handle to an open journal file.
#[derive(Debug)]
struct JournalFile {
    /// Path on disk.
    path: PathBuf,
    /// Cached header.
    header: FileHeader,
    /// Open file handle for appending.
    writer: Option<BufWriter<File>>,
}

impl JournalFile {
    /// Create a new journal file at `path`.
    fn create(path: &Path) -> io::Result<Self> {
        let header = FileHeader::new();

        let file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(path)?;

        let mut writer = BufWriter::new(file);
        writer.write_all(&header.serialize())?;
        writer.flush()?;

        Ok(JournalFile {
            path: path.to_path_buf(),
            header,
            writer: Some(writer),
        })
    }

    /// Open an existing journal file for reading (and optionally appending).
    fn open(path: &Path, writable: bool) -> io::Result<Self> {
        let mut file = OpenOptions::new().read(true).open(path)?;

        let mut header_buf = [0u8; 64];
        file.read_exact(&mut header_buf)?;
        let header = FileHeader::deserialize(&header_buf)?;

        let writer = if writable {
            let append_file = OpenOptions::new().append(true).open(path)?;
            Some(BufWriter::new(append_file))
        } else {
            None
        };

        Ok(JournalFile {
            path: path.to_path_buf(),
            header,
            writer,
        })
    }

    /// Append an entry to this file.  Returns the sequence number assigned.
    fn append(&mut self, entry: &JournalEntry, seqnum: u64) -> io::Result<u64> {
        let writer = self
            .writer
            .as_mut()
            .ok_or_else(|| io::Error::new(io::ErrorKind::Other, "journal file not writable"))?;

        let frame = serialize_entry(entry, seqnum);
        writer.write_all(&frame)?;
        writer.flush()?;

        // Update header
        if self.header.entry_count == 0 {
            self.header.head_seqnum = seqnum;
        }
        self.header.tail_seqnum = seqnum;
        self.header.entry_count += 1;
        self.header.file_size += frame.len() as u64;

        // Write the updated header to disk (seek to beginning).
        // We do this by opening the file again briefly for a pwrite-style
        // update, since BufWriter is in append mode.
        let header_bytes = self.header.serialize();
        let mut header_file = OpenOptions::new().write(true).open(&self.path)?;
        header_file.write_all(&header_bytes)?;
        header_file.flush()?;

        Ok(seqnum)
    }

    /// Read all entries from this file.
    fn read_all(&self) -> io::Result<Vec<JournalEntry>> {
        let file = File::open(&self.path)?;
        let mut reader = BufReader::new(file);

        // Skip header
        reader.seek(SeekFrom::Start(HEADER_SIZE))?;

        let mut entries = Vec::new();
        loop {
            match deserialize_entry(&mut reader) {
                Ok(entry) => entries.push(entry),
                Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => break,
                Err(e) => {
                    // Possibly a truncated entry at the end — stop reading
                    eprintln!(
                        "journald: Warning: truncated entry in {}: {}",
                        self.path.display(),
                        e
                    );
                    break;
                }
            }
        }

        Ok(entries)
    }

    /// Get the on-disk size of this file.
    fn size(&self) -> u64 {
        self.header.file_size
    }
}

// ---------------------------------------------------------------------------
// Entry frame serialisation
// ---------------------------------------------------------------------------

/// Serialise a journal entry into a frame (see module-level docs for format).
fn serialize_entry(entry: &JournalEntry, seqnum: u64) -> Vec<u8> {
    // Calculate the frame body size first
    let mut body_size: usize = 0;
    body_size += 8; // realtime_usec
    body_size += 8; // monotonic_usec
    body_size += 8; // seqnum
    body_size += 4; // field count

    for (key, value) in &entry.fields {
        body_size += 2; // key length (u16)
        body_size += 4; // value length (u32)
        body_size += key.len();
        body_size += value.len();
    }

    let frame_len = body_size as u32;
    let total_size = 4 + body_size + 4; // leading len + body + trailing len
    let mut buf = Vec::with_capacity(total_size);

    // Leading frame length
    buf.extend_from_slice(&frame_len.to_le_bytes());

    // Timestamps and seqnum
    buf.extend_from_slice(&entry.realtime_usec.to_le_bytes());
    buf.extend_from_slice(&entry.monotonic_usec.to_le_bytes());
    buf.extend_from_slice(&seqnum.to_le_bytes());

    // Field count
    let field_count = entry.fields.len() as u32;
    buf.extend_from_slice(&field_count.to_le_bytes());

    // Fields
    for (key, value) in &entry.fields {
        let key_bytes = key.as_bytes();
        buf.extend_from_slice(&(key_bytes.len() as u16).to_le_bytes());
        buf.extend_from_slice(&(value.len() as u32).to_le_bytes());
        buf.extend_from_slice(key_bytes);
        buf.extend_from_slice(value);
    }

    // Trailing frame length
    buf.extend_from_slice(&frame_len.to_le_bytes());

    debug_assert_eq!(buf.len(), total_size);
    buf
}

/// Deserialise a single entry frame from a reader.
fn deserialize_entry<R: Read>(reader: &mut R) -> io::Result<JournalEntry> {
    // Read leading frame length
    let mut len_buf = [0u8; 4];
    reader.read_exact(&mut len_buf)?;
    let frame_len = u32::from_le_bytes(len_buf) as usize;

    if frame_len < 28 {
        // Minimum: 8+8+8+4 = 28 bytes for timestamps + seqnum + field count
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("frame too small: {} bytes", frame_len),
        ));
    }

    // Read the entire frame body
    let mut body = vec![0u8; frame_len];
    reader.read_exact(&mut body)?;

    // Read trailing frame length
    let mut trail_buf = [0u8; 4];
    reader.read_exact(&mut trail_buf)?;
    let trail_len = u32::from_le_bytes(trail_buf);
    if trail_len != frame_len as u32 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "frame length mismatch: header={}, trailer={}",
                frame_len, trail_len
            ),
        ));
    }

    // Parse body
    let mut pos = 0;

    let realtime_usec = u64::from_le_bytes(body[pos..pos + 8].try_into().unwrap());
    pos += 8;
    let monotonic_usec = u64::from_le_bytes(body[pos..pos + 8].try_into().unwrap());
    pos += 8;
    let seqnum = u64::from_le_bytes(body[pos..pos + 8].try_into().unwrap());
    pos += 8;
    let field_count = u32::from_le_bytes(body[pos..pos + 4].try_into().unwrap()) as usize;
    pos += 4;

    let mut fields = BTreeMap::new();

    for _ in 0..field_count {
        if pos + 6 > body.len() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "truncated field header",
            ));
        }

        let key_len = u16::from_le_bytes(body[pos..pos + 2].try_into().unwrap()) as usize;
        pos += 2;
        let value_len = u32::from_le_bytes(body[pos..pos + 4].try_into().unwrap()) as usize;
        pos += 4;

        if pos + key_len + value_len > body.len() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "truncated field data",
            ));
        }

        let key = String::from_utf8_lossy(&body[pos..pos + key_len]).into_owned();
        pos += key_len;
        let value = body[pos..pos + value_len].to_vec();
        pos += value_len;

        fields.insert(key, value);
    }

    let mut entry = JournalEntry::with_timestamp(realtime_usec, monotonic_usec);
    entry.seqnum = seqnum;
    entry.fields = fields;

    Ok(entry)
}

// ---------------------------------------------------------------------------
// JournalStorage — multi-file journal storage manager
// ---------------------------------------------------------------------------

/// Configuration for journal storage.
#[derive(Debug, Clone)]
pub struct StorageConfig {
    /// Base directory for journal files (e.g. `/var/log/journal/<machine-id>`
    /// or `/run/log/journal/<machine-id>`).
    pub directory: PathBuf,

    /// Maximum size of a single journal file in bytes.
    pub max_file_size: u64,

    /// Maximum total disk usage for all journal files.
    pub max_disk_usage: u64,

    /// Maximum number of journal files to keep.
    pub max_files: usize,

    /// Whether to use persistent storage (`/var/log/journal`) or volatile
    /// (`/run/log/journal`).
    pub persistent: bool,
}

impl Default for StorageConfig {
    fn default() -> Self {
        StorageConfig {
            directory: PathBuf::from("/run/log/journal"),
            max_file_size: DEFAULT_MAX_FILE_SIZE,
            max_disk_usage: DEFAULT_MAX_DISK_USAGE,
            max_files: DEFAULT_MAX_FILES,
            persistent: false,
        }
    }
}

/// Manages journal storage across multiple files with rotation and vacuuming.
pub struct JournalStorage {
    config: StorageConfig,
    /// The currently active (writable) journal file.
    active_file: Option<JournalFile>,
    /// Next sequence number to assign.
    next_seqnum: u64,
    /// Cached machine ID for directory naming.
    machine_id: String,
}

impl JournalStorage {
    /// Create a new journal storage manager.
    ///
    /// This creates the storage directory if it does not exist and opens
    /// (or creates) the active journal file.
    pub fn new(config: StorageConfig) -> io::Result<Self> {
        let machine_id = read_machine_id();
        let journal_dir = config.directory.join(&machine_id);
        fs::create_dir_all(&journal_dir)?;

        let mut storage = JournalStorage {
            config,
            active_file: None,
            next_seqnum: 1,
            machine_id,
        };

        // Try to resume from the newest existing journal file, otherwise
        // create a new one.
        storage.open_or_create_active_file()?;

        Ok(storage)
    }

    /// Append a journal entry.  Handles rotation if the active file is
    /// too large.  Returns the assigned sequence number.
    pub fn append(&mut self, entry: &JournalEntry) -> io::Result<u64> {
        // Check if we need to rotate
        if let Some(ref file) = self.active_file {
            if file.size() >= self.config.max_file_size {
                self.rotate()?;
            }
        }

        // Ensure we have an active file
        if self.active_file.is_none() {
            self.open_or_create_active_file()?;
        }

        let seqnum = self.next_seqnum;
        self.next_seqnum += 1;

        if let Some(ref mut file) = self.active_file {
            file.append(entry, seqnum)?;
        }

        Ok(seqnum)
    }

    /// Read all entries from all journal files, in chronological order.
    pub fn read_all(&self) -> io::Result<Vec<JournalEntry>> {
        let journal_dir = self.config.directory.join(&self.machine_id);
        let mut all_entries = Vec::new();

        let mut files = list_journal_files(&journal_dir)?;
        files.sort(); // Sorted by name (which includes timestamp)

        for file_path in &files {
            match JournalFile::open(file_path, false) {
                Ok(jf) => match jf.read_all() {
                    Ok(entries) => all_entries.extend(entries),
                    Err(e) => {
                        eprintln!(
                            "journald: Warning: could not read {}: {}",
                            file_path.display(),
                            e
                        );
                    }
                },
                Err(e) => {
                    eprintln!(
                        "journald: Warning: could not open {}: {}",
                        file_path.display(),
                        e
                    );
                }
            }
        }

        // Sort by realtime timestamp, then by sequence number for stability
        all_entries.sort_by(|a, b| {
            a.realtime_usec
                .cmp(&b.realtime_usec)
                .then_with(|| a.seqnum.cmp(&b.seqnum))
        });

        Ok(all_entries)
    }

    /// Generate a cursor string for an entry.  The cursor encodes enough
    /// information to uniquely identify an entry across files.
    pub fn make_cursor(&self, entry: &JournalEntry) -> String {
        let file_id = self
            .active_file
            .as_ref()
            .map(|f| f.header.file_id)
            .unwrap_or(0);
        format!(
            "s={:032x};i={:x};b={};m={:x};t={:x};x={:x}",
            file_id,
            entry.seqnum,
            entry.boot_id().unwrap_or_default(),
            entry.monotonic_usec,
            entry.realtime_usec,
            0u64, // xor hash, unused for now
        )
    }

    /// Rotate the active journal file: close the current file and start
    /// a new one.
    pub fn rotate(&mut self) -> io::Result<()> {
        // Drop the current active file (closes the writer)
        self.active_file = None;

        // Vacuum old files if over limits
        self.vacuum()?;

        // Create a new file
        self.create_new_active_file()?;

        Ok(())
    }

    /// Flush any buffered writes to disk.
    pub fn flush(&mut self) -> io::Result<()> {
        if let Some(ref mut file) = self.active_file {
            if let Some(ref mut writer) = file.writer {
                writer.flush()?;
            }
        }
        Ok(())
    }

    /// Return the total disk usage of all journal files.
    pub fn disk_usage(&self) -> io::Result<u64> {
        let journal_dir = self.config.directory.join(&self.machine_id);
        let files = list_journal_files(&journal_dir)?;
        let mut total = 0u64;
        for path in &files {
            if let Ok(meta) = fs::metadata(path) {
                total += meta.len();
            }
        }
        Ok(total)
    }

    /// Return the number of journal files.
    pub fn file_count(&self) -> io::Result<usize> {
        let journal_dir = self.config.directory.join(&self.machine_id);
        let files = list_journal_files(&journal_dir)?;
        Ok(files.len())
    }

    /// Get the storage directory path.
    pub fn directory(&self) -> PathBuf {
        self.config.directory.join(&self.machine_id)
    }

    // ---------------------------------------------------------------
    // Internal helpers
    // ---------------------------------------------------------------

    fn open_or_create_active_file(&mut self) -> io::Result<()> {
        let journal_dir = self.config.directory.join(&self.machine_id);
        let mut files = list_journal_files(&journal_dir)?;
        files.sort();

        if let Some(newest) = files.last() {
            // Try to open the newest file for appending
            match JournalFile::open(newest, true) {
                Ok(jf) => {
                    // Resume sequence numbers from where this file left off
                    if jf.header.tail_seqnum >= self.next_seqnum {
                        self.next_seqnum = jf.header.tail_seqnum + 1;
                    }
                    // Only reuse if it's under the size limit
                    if jf.size() < self.config.max_file_size {
                        self.active_file = Some(jf);
                        return Ok(());
                    }
                    // Otherwise fall through to create a new file
                }
                Err(e) => {
                    eprintln!(
                        "journald: Could not reopen {}: {}; creating new file",
                        newest.display(),
                        e
                    );
                }
            }
        }

        self.create_new_active_file()
    }

    fn create_new_active_file(&mut self) -> io::Result<()> {
        let journal_dir = self.config.directory.join(&self.machine_id);

        // File name format: system@<hex-random>-<timestamp>.journal
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_micros();
        let random_part = generate_random_u128() & 0xFFFF_FFFF;
        let filename = format!("system@{:08x}-{:016x}.journal", random_part, timestamp);
        let path = journal_dir.join(filename);

        let jf = JournalFile::create(&path)?;
        self.active_file = Some(jf);
        Ok(())
    }

    fn vacuum(&mut self) -> io::Result<()> {
        let journal_dir = self.config.directory.join(&self.machine_id);
        let mut files = list_journal_files(&journal_dir)?;
        files.sort();

        // Remove files until we're under both the file count and disk usage limits
        while files.len() > self.config.max_files {
            if let Some(oldest) = files.first() {
                eprintln!(
                    "journald: Vacuuming {} (file count limit)",
                    oldest.display()
                );
                let _ = fs::remove_file(oldest);
                files.remove(0);
            } else {
                break;
            }
        }

        // Check total disk usage
        loop {
            let mut total: u64 = 0;
            for f in &files {
                if let Ok(meta) = fs::metadata(f) {
                    total += meta.len();
                }
            }
            if total <= self.config.max_disk_usage || files.is_empty() {
                break;
            }
            if let Some(oldest) = files.first() {
                eprintln!(
                    "journald: Vacuuming {} (disk usage limit: {} > {})",
                    oldest.display(),
                    total,
                    self.config.max_disk_usage
                );
                let _ = fs::remove_file(oldest);
                files.remove(0);
            } else {
                break;
            }
        }

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Free helper functions
// ---------------------------------------------------------------------------

/// List all `.journal` files in a directory, sorted by name.
fn list_journal_files(dir: &Path) -> io::Result<Vec<PathBuf>> {
    if !dir.exists() {
        return Ok(Vec::new());
    }

    let mut files = Vec::new();
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().map_or(false, |ext| ext == "journal") && path.is_file() {
            files.push(path);
        }
    }

    files.sort();
    Ok(files)
}

/// Read the machine ID from `/etc/machine-id`.
fn read_machine_id() -> String {
    fs::read_to_string("/etc/machine-id")
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|_| "0".repeat(32))
}

/// Generate a random u128 using `/dev/urandom`.
fn generate_random_u128() -> u128 {
    let mut buf = [0u8; 16];
    if let Ok(mut f) = File::open("/dev/urandom") {
        if f.read_exact(&mut buf).is_ok() {
            return u128::from_le_bytes(buf);
        }
    }
    // Fallback: use the current time as a poor man's random
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let lo = now.as_nanos() as u64;
    let hi = std::process::id() as u64;
    ((hi as u128) << 64) | (lo as u128)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn make_test_entry(msg: &str, priority: u8, seqnum: u64) -> JournalEntry {
        let mut entry = JournalEntry::with_timestamp(1_700_000_000_000_000 + seqnum, seqnum * 1000);
        entry.seqnum = seqnum;
        entry.set_field("MESSAGE", msg);
        entry.set_field("PRIORITY", priority.to_string());
        entry.set_field("SYSLOG_IDENTIFIER", "test");
        entry.set_field("_PID", "42");
        entry
    }

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("journald_test_{}", name));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn test_header_roundtrip() {
        let header = FileHeader::new();
        let bytes = header.serialize();
        let header2 = FileHeader::deserialize(&bytes).unwrap();

        assert_eq!(header.magic, header2.magic);
        assert_eq!(header.version, header2.version);
        assert_eq!(header.file_id, header2.file_id);
        assert_eq!(header.head_seqnum, header2.head_seqnum);
        assert_eq!(header.tail_seqnum, header2.tail_seqnum);
        assert_eq!(header.entry_count, header2.entry_count);
        assert_eq!(header.file_size, header2.file_size);
    }

    #[test]
    fn test_header_bad_magic() {
        let mut buf = [0u8; 64];
        buf[0..8].copy_from_slice(b"INVALID\0");
        assert!(FileHeader::deserialize(&buf).is_err());
    }

    #[test]
    fn test_entry_serialize_deserialize() {
        let entry = make_test_entry("hello world", 6, 1);
        let frame = serialize_entry(&entry, 1);

        let mut reader = io::Cursor::new(&frame);
        let entry2 = deserialize_entry(&mut reader).unwrap();

        assert_eq!(entry.realtime_usec, entry2.realtime_usec);
        assert_eq!(entry.monotonic_usec, entry2.monotonic_usec);
        assert_eq!(entry2.seqnum, 1);
        assert_eq!(entry2.message(), Some("hello world".to_string()));
        assert_eq!(entry2.priority(), Some(6));
        assert_eq!(entry2.field("SYSLOG_IDENTIFIER"), Some("test".to_string()));
        assert_eq!(entry2.pid(), Some(42));
    }

    #[test]
    fn test_entry_serialize_empty() {
        let entry = JournalEntry::with_timestamp(100, 200);
        let frame = serialize_entry(&entry, 0);

        let mut reader = io::Cursor::new(&frame);
        let entry2 = deserialize_entry(&mut reader).unwrap();

        assert_eq!(entry2.realtime_usec, 100);
        assert_eq!(entry2.monotonic_usec, 200);
        assert_eq!(entry2.seqnum, 0);
        assert!(entry2.fields.is_empty());
    }

    #[test]
    fn test_entry_serialize_binary_value() {
        let mut entry = JournalEntry::with_timestamp(1000, 2000);
        entry.set_field_bytes("BINARY", vec![0x00, 0x01, 0x0a, 0xff, 0x80]);
        entry.set_field("TEXT", "normal text");

        let frame = serialize_entry(&entry, 5);
        let mut reader = io::Cursor::new(&frame);
        let entry2 = deserialize_entry(&mut reader).unwrap();

        assert_eq!(
            entry2.field_bytes("BINARY"),
            Some(&[0x00, 0x01, 0x0a, 0xff, 0x80][..])
        );
        assert_eq!(entry2.field("TEXT"), Some("normal text".to_string()));
    }

    #[test]
    fn test_entry_serialize_many_fields() {
        let mut entry = JournalEntry::with_timestamp(42, 43);
        for i in 0..100 {
            entry.set_field(format!("FIELD_{}", i), format!("value_{}", i));
        }

        let frame = serialize_entry(&entry, 99);
        let mut reader = io::Cursor::new(&frame);
        let entry2 = deserialize_entry(&mut reader).unwrap();

        assert_eq!(entry2.fields.len(), 100);
        for i in 0..100 {
            assert_eq!(
                entry2.field(&format!("FIELD_{}", i)),
                Some(format!("value_{}", i))
            );
        }
    }

    #[test]
    fn test_multiple_entries_in_sequence() {
        let entries: Vec<JournalEntry> = (1..=5)
            .map(|i| make_test_entry(&format!("msg {}", i), 6, i))
            .collect();

        let mut buf = Vec::new();
        for (i, entry) in entries.iter().enumerate() {
            buf.extend_from_slice(&serialize_entry(entry, (i + 1) as u64));
        }

        let mut reader = io::Cursor::new(&buf);
        let mut deserialized = Vec::new();
        loop {
            match deserialize_entry(&mut reader) {
                Ok(entry) => deserialized.push(entry),
                Err(_) => break,
            }
        }

        assert_eq!(deserialized.len(), 5);
        for (i, entry) in deserialized.iter().enumerate() {
            assert_eq!(entry.message(), Some(format!("msg {}", i + 1)));
            assert_eq!(entry.seqnum, (i + 1) as u64);
        }
    }

    #[test]
    fn test_frame_length_mismatch_detected() {
        let entry = make_test_entry("test", 6, 1);
        let mut frame = serialize_entry(&entry, 1);

        // Corrupt the trailing length
        let len = frame.len();
        frame[len - 1] = 0xFF;
        frame[len - 2] = 0xFF;

        let mut reader = io::Cursor::new(&frame);
        let result = deserialize_entry(&mut reader);
        assert!(result.is_err());
    }

    #[test]
    fn test_journal_file_create_and_read() {
        let dir = temp_dir("file_create");
        let path = dir.join("test.journal");

        let mut jf = JournalFile::create(&path).unwrap();
        assert_eq!(jf.header.entry_count, 0);

        let entry1 = make_test_entry("first entry", 6, 1);
        jf.append(&entry1, 1).unwrap();

        let entry2 = make_test_entry("second entry", 4, 2);
        jf.append(&entry2, 2).unwrap();

        assert_eq!(jf.header.entry_count, 2);
        assert_eq!(jf.header.head_seqnum, 1);
        assert_eq!(jf.header.tail_seqnum, 2);

        // Read back
        let entries = jf.read_all().unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].message(), Some("first entry".to_string()));
        assert_eq!(entries[1].message(), Some("second entry".to_string()));

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_journal_file_reopen_and_read() {
        let dir = temp_dir("file_reopen");
        let path = dir.join("test.journal");

        // Create and write
        {
            let mut jf = JournalFile::create(&path).unwrap();
            let entry = make_test_entry("persisted entry", 6, 1);
            jf.append(&entry, 1).unwrap();
        }

        // Reopen and verify
        {
            let jf = JournalFile::open(&path, false).unwrap();
            assert_eq!(jf.header.entry_count, 1);
            assert_eq!(jf.header.head_seqnum, 1);
            assert_eq!(jf.header.tail_seqnum, 1);

            let entries = jf.read_all().unwrap();
            assert_eq!(entries.len(), 1);
            assert_eq!(entries[0].message(), Some("persisted entry".to_string()));
        }

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_journal_file_append_after_reopen() {
        let dir = temp_dir("file_append_reopen");
        let path = dir.join("test.journal");

        // Create and write one entry
        {
            let mut jf = JournalFile::create(&path).unwrap();
            jf.append(&make_test_entry("entry 1", 6, 1), 1).unwrap();
        }

        // Reopen for appending and add another entry
        {
            let mut jf = JournalFile::open(&path, true).unwrap();
            jf.append(&make_test_entry("entry 2", 4, 2), 2).unwrap();
        }

        // Read all
        {
            let jf = JournalFile::open(&path, false).unwrap();
            let entries = jf.read_all().unwrap();
            assert_eq!(entries.len(), 2);
            assert_eq!(entries[0].message(), Some("entry 1".to_string()));
            assert_eq!(entries[1].message(), Some("entry 2".to_string()));
        }

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_storage_basic() {
        let dir = temp_dir("storage_basic");
        let config = StorageConfig {
            directory: dir.clone(),
            max_file_size: DEFAULT_MAX_FILE_SIZE,
            max_disk_usage: DEFAULT_MAX_DISK_USAGE,
            max_files: DEFAULT_MAX_FILES,
            persistent: false,
        };

        let mut storage = JournalStorage::new(config).unwrap();

        // Write some entries
        for i in 1..=10 {
            let entry = make_test_entry(&format!("log message {}", i), 6, i);
            let seqnum = storage.append(&entry).unwrap();
            assert_eq!(seqnum, i);
        }

        // Read all entries back
        let entries = storage.read_all().unwrap();
        assert_eq!(entries.len(), 10);
        for (i, entry) in entries.iter().enumerate() {
            assert_eq!(entry.message(), Some(format!("log message {}", i + 1)));
        }

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_storage_rotation() {
        let dir = temp_dir("storage_rotation");
        let config = StorageConfig {
            directory: dir.clone(),
            // Very small max file size to force rotation
            max_file_size: 256,
            max_disk_usage: DEFAULT_MAX_DISK_USAGE,
            max_files: DEFAULT_MAX_FILES,
            persistent: false,
        };

        let mut storage = JournalStorage::new(config).unwrap();

        // Write enough entries to trigger rotation
        for i in 1..=20 {
            let entry = make_test_entry(&format!("rotation test {}", i), 6, i);
            storage.append(&entry).unwrap();
        }

        // Should have multiple files now
        let file_count = storage.file_count().unwrap();
        assert!(
            file_count > 1,
            "expected more than 1 file after rotation, got {}",
            file_count
        );

        // All entries should still be readable
        let entries = storage.read_all().unwrap();
        assert_eq!(entries.len(), 20);

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_storage_vacuum_file_count() {
        let dir = temp_dir("storage_vacuum_count");
        let config = StorageConfig {
            directory: dir.clone(),
            max_file_size: 256,
            max_disk_usage: DEFAULT_MAX_DISK_USAGE,
            max_files: 3,
            persistent: false,
        };

        let mut storage = JournalStorage::new(config).unwrap();

        // Write many entries to create many files
        for i in 1..=50 {
            let entry = make_test_entry(&format!("vacuum test {}", i), 6, i);
            storage.append(&entry).unwrap();
        }

        // File count should be limited
        let file_count = storage.file_count().unwrap();
        assert!(
            file_count <= 4, // 3 limit + 1 active being written
            "expected <= 4 files, got {}",
            file_count
        );

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_storage_disk_usage() {
        let dir = temp_dir("storage_disk_usage");
        let config = StorageConfig {
            directory: dir.clone(),
            max_file_size: DEFAULT_MAX_FILE_SIZE,
            max_disk_usage: DEFAULT_MAX_DISK_USAGE,
            max_files: DEFAULT_MAX_FILES,
            persistent: false,
        };

        let mut storage = JournalStorage::new(config).unwrap();

        let entry = make_test_entry("disk usage test", 6, 1);
        storage.append(&entry).unwrap();

        let usage = storage.disk_usage().unwrap();
        assert!(usage > 0, "disk usage should be > 0");
        // At minimum, we have the header + one entry frame
        assert!(usage >= HEADER_SIZE);

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_storage_make_cursor() {
        let dir = temp_dir("storage_cursor");
        let config = StorageConfig {
            directory: dir.clone(),
            ..StorageConfig::default()
        };

        let mut storage = JournalStorage::new(config).unwrap();
        let mut entry = make_test_entry("cursor test", 6, 1);
        entry.set_field("_BOOT_ID", "abcdef0123456789abcdef0123456789");
        storage.append(&entry).unwrap();

        let cursor = storage.make_cursor(&entry);
        assert!(cursor.contains("s="));
        assert!(cursor.contains("i="));
        assert!(cursor.contains("b="));
        assert!(cursor.contains("m="));
        assert!(cursor.contains("t="));

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_storage_flush() {
        let dir = temp_dir("storage_flush");
        let config = StorageConfig {
            directory: dir.clone(),
            ..StorageConfig::default()
        };

        let mut storage = JournalStorage::new(config).unwrap();
        let entry = make_test_entry("flush test", 6, 1);
        storage.append(&entry).unwrap();
        // flush should not panic or error
        storage.flush().unwrap();

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_storage_explicit_rotation() {
        let dir = temp_dir("storage_explicit_rotate");
        let config = StorageConfig {
            directory: dir.clone(),
            ..StorageConfig::default()
        };

        let mut storage = JournalStorage::new(config).unwrap();

        let entry1 = make_test_entry("before rotation", 6, 1);
        storage.append(&entry1).unwrap();

        storage.rotate().unwrap();

        let entry2 = make_test_entry("after rotation", 6, 2);
        storage.append(&entry2).unwrap();

        // Both entries should be readable
        let entries = storage.read_all().unwrap();
        assert_eq!(entries.len(), 2);

        let file_count = storage.file_count().unwrap();
        assert_eq!(file_count, 2);

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_list_journal_files_empty_dir() {
        let dir = temp_dir("list_empty");
        let files = list_journal_files(&dir).unwrap();
        assert!(files.is_empty());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_list_journal_files_nonexistent_dir() {
        let dir = PathBuf::from("/tmp/journald_test_nonexistent_dir_xyz");
        let files = list_journal_files(&dir).unwrap();
        assert!(files.is_empty());
    }

    #[test]
    fn test_list_journal_files_filters_non_journal() {
        let dir = temp_dir("list_filter");
        fs::write(dir.join("test.journal"), "data").unwrap();
        fs::write(dir.join("test.txt"), "not a journal").unwrap();
        fs::write(dir.join("other.log"), "also not").unwrap();

        let files = list_journal_files(&dir).unwrap();
        assert_eq!(files.len(), 1);
        assert!(files[0].ends_with("test.journal"));

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_generate_random_u128() {
        let a = generate_random_u128();
        let b = generate_random_u128();
        // They should almost certainly be different
        assert_ne!(a, b, "two random u128 values should differ");
    }
}
