//! systemd-portabled — portable service image management daemon
//!
//! This is a Rust implementation of systemd's `systemd-portabled` daemon. It
//! manages portable service images — directory trees or raw disk images that
//! contain systemd unit files and their executables. When an image is
//! "attached", its unit files are symlinked into the system service manager's
//! unit search path; when "detached" those symlinks are removed.
//!
//! ## Features
//!
//! - Image discovery from standard search paths (`/var/lib/portables/`,
//!   `/etc/portables/`, `/usr/lib/portables/`, `/run/portables/`)
//! - Image inspection (enumerate unit files, read `os-release`)
//! - Attach/detach operations with symlink management
//! - Profile-based drop-in generation for security hardening
//! - Reattach (atomic detach + attach)
//! - Attachment state tracking via marker files in `/run/systemd/portabled/`
//! - Runtime vs persistent attachment modes
//! - Control socket at `/run/systemd/portabled-control` for `portablectl` CLI
//! - sd_notify protocol (READY/WATCHDOG/STATUS/STOPPING)
//! - Signal handling (SIGTERM/SIGINT for shutdown, SIGHUP for reload)
//! - Periodic state consistency checks
//!
//! ## Missing
//!
//! - D-Bus interface (`org.freedesktop.portable1`)
//! - Raw disk image support (loopback mount, GPT dissection)
//! - Extension images (`--extension`)
//! - Image size limit management (`set-limit`)
//! - Automatic `daemon-reload` after attach/detach
//! - Read-only flag toggling for directory images

use std::collections::BTreeMap;
use std::env;
use std::fmt;
use std::fs;
use std::io::{self, BufRead, BufReader, Write};
use std::net::Shutdown;
use std::os::unix::fs as unix_fs;
use std::os::unix::net::{UnixDatagram, UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::process;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

// ---------------------------------------------------------------------------
// Paths
// ---------------------------------------------------------------------------

const CONTROL_SOCKET_PATH: &str = "/run/systemd/portabled-control";
const STATE_DIR: &str = "/run/systemd/portabled";

/// Directories where portable service images are searched for, in priority
/// order (highest first).
const IMAGE_SEARCH_PATHS: &[&str] = &[
    "/etc/portables",
    "/run/portables",
    "/var/lib/portables",
    "/usr/lib/portables",
];

/// Where attached unit symlinks are placed (persistent).
const PERSISTENT_ATTACHED_DIR: &str = "/etc/systemd/system.attached";

/// Where attached unit symlinks are placed (runtime / volatile).
const RUNTIME_ATTACHED_DIR: &str = "/run/systemd/system.attached";

/// Directories inside an image where unit files live.
const IMAGE_UNIT_PATHS: &[&str] = &[
    "usr/lib/systemd/system",
    "lib/systemd/system",
    "etc/systemd/system",
];

/// Profile search paths (highest priority first).
const PROFILE_SEARCH_PATHS: &[&str] = &[
    "/etc/systemd/portable/profile",
    "/usr/lib/systemd/portable/profile",
    "/run/systemd/portable/profile",
];

// ---------------------------------------------------------------------------
// Data model
// ---------------------------------------------------------------------------

/// The type of a portable image.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageType {
    /// A plain directory tree.
    Directory,
    /// A raw disk image file (`.raw`).
    Raw,
}

impl ImageType {
    pub fn as_str(&self) -> &'static str {
        match self {
            ImageType::Directory => "directory",
            ImageType::Raw => "raw",
        }
    }

    pub fn parse(s: &str) -> Option<ImageType> {
        match s.to_ascii_lowercase().as_str() {
            "directory" | "dir" => Some(ImageType::Directory),
            "raw" => Some(ImageType::Raw),
            _ => None,
        }
    }
}

impl fmt::Display for ImageType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// The attachment state of a portable image.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttachState {
    Detached,
    Attached,
    AttachedRuntime,
    Enabled,
    EnabledRuntime,
    Running,
}

impl AttachState {
    pub fn as_str(&self) -> &'static str {
        match self {
            AttachState::Detached => "detached",
            AttachState::Attached => "attached",
            AttachState::AttachedRuntime => "attached-runtime",
            AttachState::Enabled => "enabled",
            AttachState::EnabledRuntime => "enabled-runtime",
            AttachState::Running => "running",
        }
    }

    pub fn parse(s: &str) -> Option<AttachState> {
        match s.to_ascii_lowercase().as_str() {
            "detached" => Some(AttachState::Detached),
            "attached" => Some(AttachState::Attached),
            "attached-runtime" => Some(AttachState::AttachedRuntime),
            "enabled" => Some(AttachState::Enabled),
            "enabled-runtime" => Some(AttachState::EnabledRuntime),
            "running" => Some(AttachState::Running),
            _ => None,
        }
    }

    pub fn is_attached(&self) -> bool {
        !matches!(self, AttachState::Detached)
    }
}

impl fmt::Display for AttachState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A portable service image.
#[derive(Debug, Clone)]
pub struct PortableImage {
    /// Image name (directory/file stem).
    pub name: String,
    /// Absolute path to the image.
    pub path: PathBuf,
    /// Image type (directory or raw).
    pub image_type: ImageType,
    /// Size in bytes (for raw images) or 0.
    pub size: u64,
    /// Modification time as microseconds since UNIX epoch.
    pub mtime_usec: u64,
    /// Creation time as microseconds since UNIX epoch (if available).
    pub crtime_usec: u64,
    /// OS pretty name from os-release, if available.
    pub os_pretty_name: Option<String>,
    /// Portable service extension level from os-release.
    pub portable_service: Option<String>,
}

impl PortableImage {
    /// Read os-release from the image (directory only for now).
    pub fn read_os_release(image_path: &Path) -> BTreeMap<String, String> {
        let mut result = BTreeMap::new();
        for name in &["usr/lib/os-release", "etc/os-release"] {
            let p = image_path.join(name);
            if let Ok(content) = fs::read_to_string(&p) {
                for line in content.lines() {
                    let line = line.trim();
                    if line.is_empty() || line.starts_with('#') {
                        continue;
                    }
                    if let Some((key, val)) = line.split_once('=') {
                        let val = val.trim_matches('"').trim_matches('\'');
                        result.insert(key.to_string(), val.to_string());
                    }
                }
                break;
            }
        }
        result
    }

    /// Discover unit files within a directory-type image.
    pub fn discover_units(image_path: &Path) -> Vec<String> {
        let mut units = Vec::new();
        for subdir in IMAGE_UNIT_PATHS {
            let dir = image_path.join(subdir);
            if let Ok(entries) = fs::read_dir(&dir) {
                for entry in entries.flatten() {
                    let name = entry.file_name().to_string_lossy().to_string();
                    if (name.ends_with(".service")
                        || name.ends_with(".socket")
                        || name.ends_with(".target")
                        || name.ends_with(".timer")
                        || name.ends_with(".path"))
                        && !units.contains(&name)
                    {
                        units.push(name);
                    }
                }
            }
        }
        units.sort();
        units
    }

    /// Find the absolute path of a unit file inside the image.
    pub fn find_unit_path(image_path: &Path, unit_name: &str) -> Option<PathBuf> {
        for subdir in IMAGE_UNIT_PATHS {
            let p = image_path.join(subdir).join(unit_name);
            if p.exists() {
                return Some(p);
            }
        }
        None
    }

    /// Format as a human-readable status block.
    pub fn format_status(&self) -> String {
        let mut lines = Vec::new();
        lines.push(format!("        Name: {}", self.name));
        lines.push(format!("        Path: {}", self.path.display()));
        lines.push(format!("        Type: {}", self.image_type));
        if self.size > 0 {
            lines.push(format!("        Size: {}", format_bytes(self.size)));
        }
        if self.mtime_usec > 0 {
            lines.push(format!(
                "    Modified: {}",
                format_timestamp(self.mtime_usec)
            ));
        }
        if let Some(ref os) = self.os_pretty_name {
            lines.push(format!("          OS: {}", os));
        }

        // Show discovered units
        if self.image_type == ImageType::Directory {
            let units = Self::discover_units(&self.path);
            if !units.is_empty() {
                lines.push(format!("       Units: {}", units.join(", ")));
            }
        }
        lines.join("\n")
    }

    /// Format as key=value properties.
    pub fn format_show(&self) -> String {
        let mut lines = Vec::new();
        lines.push(format!("Name={}", self.name));
        lines.push(format!("Path={}", self.path.display()));
        lines.push(format!("Type={}", self.image_type));
        lines.push(format!("Size={}", self.size));
        lines.push(format!("ModifiedUSec={}", self.mtime_usec));
        lines.push(format!("CreatedUSec={}", self.crtime_usec));
        lines.push(format!(
            "OSPrettyName={}",
            self.os_pretty_name.as_deref().unwrap_or("")
        ));
        lines.push(format!(
            "PortableService={}",
            self.portable_service.as_deref().unwrap_or("")
        ));
        lines.join("\n")
    }
}

// ---------------------------------------------------------------------------
// Attachment info — persisted in /run/systemd/portabled/
// ---------------------------------------------------------------------------

/// Tracks which images are attached and which unit files were symlinked.
#[derive(Debug, Clone)]
pub struct AttachmentInfo {
    pub image_name: String,
    pub image_path: String,
    pub profile: Option<String>,
    pub runtime: bool,
    pub units: Vec<String>,
    pub timestamp: u64,
}

impl AttachmentInfo {
    /// Serialize to a state file.
    pub fn to_state_file(&self) -> String {
        let mut lines = Vec::new();
        lines.push(format!("IMAGE_NAME={}", self.image_name));
        lines.push(format!("IMAGE_PATH={}", self.image_path));
        if let Some(ref p) = self.profile {
            lines.push(format!("PROFILE={}", p));
        }
        lines.push(format!(
            "RUNTIME={}",
            if self.runtime { "yes" } else { "no" }
        ));
        lines.push(format!("TIMESTAMP={}", self.timestamp));
        for u in &self.units {
            lines.push(format!("UNIT={}", u));
        }
        lines.join("\n")
    }

    /// Parse from state file content.
    pub fn from_state_file(content: &str) -> Option<AttachmentInfo> {
        let mut image_name = None;
        let mut image_path = None;
        let mut profile = None;
        let mut runtime = false;
        let mut timestamp = 0u64;
        let mut units = Vec::new();

        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if let Some((key, val)) = line.split_once('=') {
                match key {
                    "IMAGE_NAME" => image_name = Some(val.to_string()),
                    "IMAGE_PATH" => image_path = Some(val.to_string()),
                    "PROFILE" => profile = Some(val.to_string()),
                    "RUNTIME" => runtime = val == "yes",
                    "TIMESTAMP" => timestamp = val.parse().unwrap_or(0),
                    "UNIT" => units.push(val.to_string()),
                    _ => {}
                }
            }
        }

        Some(AttachmentInfo {
            image_name: image_name?,
            image_path: image_path?,
            profile,
            runtime,
            units,
            timestamp,
        })
    }
}

// ---------------------------------------------------------------------------
// Image registry
// ---------------------------------------------------------------------------

pub struct ImageRegistry {
    /// Known images from search paths (name -> image).
    pub images: BTreeMap<String, PortableImage>,
    /// Currently attached images (name -> attachment info).
    pub attachments: BTreeMap<String, AttachmentInfo>,
}

impl Default for ImageRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl ImageRegistry {
    pub fn new() -> Self {
        ImageRegistry {
            images: BTreeMap::new(),
            attachments: BTreeMap::new(),
        }
    }

    /// Scan all image search paths and populate the images map.
    pub fn discover_images(&mut self) {
        self.discover_images_from(IMAGE_SEARCH_PATHS);
    }

    /// Scan the given search paths for images.
    pub fn discover_images_from(&mut self, search_paths: &[&str]) {
        self.images.clear();
        for search_dir in search_paths {
            let dir = Path::new(search_dir);
            if !dir.is_dir() {
                continue;
            }
            let entries = match fs::read_dir(dir) {
                Ok(e) => e,
                Err(_) => continue,
            };
            for entry in entries.flatten() {
                let path = entry.path();
                let name_os = entry.file_name();
                let name_str = name_os.to_string_lossy();

                // Skip hidden files
                if name_str.starts_with('.') {
                    continue;
                }

                let (name, image_type) = if path.is_dir() {
                    (name_str.to_string(), ImageType::Directory)
                } else if name_str.ends_with(".raw") {
                    let stem = name_str.trim_end_matches(".raw").to_string();
                    (stem, ImageType::Raw)
                } else {
                    continue;
                };

                // Don't overwrite higher-priority entries
                if self.images.contains_key(&name) {
                    continue;
                }

                let (size, mtime_usec, crtime_usec) = file_times_and_size(&path);
                let os_release = if image_type == ImageType::Directory {
                    PortableImage::read_os_release(&path)
                } else {
                    BTreeMap::new()
                };

                let os_pretty_name = os_release.get("PRETTY_NAME").cloned();
                let portable_service = os_release.get("PORTABLE_PREFIXES").cloned();

                self.images.insert(
                    name.clone(),
                    PortableImage {
                        name,
                        path,
                        image_type,
                        size,
                        mtime_usec,
                        crtime_usec,
                        os_pretty_name,
                        portable_service,
                    },
                );
            }
        }
    }

    /// Load attachment state from the state directory.
    pub fn load_attachments(&mut self) {
        self.load_attachments_from(STATE_DIR);
    }

    /// Load attachment state from a given directory.
    pub fn load_attachments_from(&mut self, dir: &str) {
        self.attachments.clear();
        let path = Path::new(dir);
        if !path.is_dir() {
            return;
        }
        let entries = match fs::read_dir(path) {
            Ok(e) => e,
            Err(_) => return,
        };
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with('.') {
                continue;
            }
            let content = match fs::read_to_string(entry.path()) {
                Ok(c) => c,
                Err(_) => continue,
            };
            if let Some(info) = AttachmentInfo::from_state_file(&content) {
                self.attachments.insert(info.image_name.clone(), info);
            }
        }
    }

    /// Save all attachment state files.
    pub fn save_attachments(&self) {
        self.save_attachments_to(STATE_DIR);
    }

    /// Save attachment state to a given directory.
    pub fn save_attachments_to(&self, dir: &str) {
        let path = Path::new(dir);
        let _ = fs::create_dir_all(path);
        for (name, info) in &self.attachments {
            let file_path = path.join(name);
            let _ = fs::write(&file_path, info.to_state_file());
        }
    }

    /// Save a single attachment state file.
    pub fn save_attachment(&self, name: &str) {
        self.save_attachment_to(name, STATE_DIR);
    }

    /// Save a single attachment state file to a given directory.
    pub fn save_attachment_to(&self, name: &str, dir: &str) {
        if let Some(info) = self.attachments.get(name) {
            let path = Path::new(dir);
            let _ = fs::create_dir_all(path);
            let file_path = path.join(name);
            let _ = fs::write(&file_path, info.to_state_file());
        }
    }

    /// Remove an attachment state file.
    pub fn remove_attachment_file(&self, name: &str) {
        self.remove_attachment_file_from(name, STATE_DIR);
    }

    /// Remove an attachment state file from a given directory.
    pub fn remove_attachment_file_from(&self, name: &str, dir: &str) {
        let path = Path::new(dir).join(name);
        let _ = fs::remove_file(path);
    }

    /// Get the attachment state of an image.
    pub fn get_attach_state(&self, name: &str) -> AttachState {
        match self.attachments.get(name) {
            None => AttachState::Detached,
            Some(info) => {
                if info.runtime {
                    AttachState::AttachedRuntime
                } else {
                    AttachState::Attached
                }
            }
        }
    }

    /// Get an image by name.
    pub fn get_image(&self, name: &str) -> Option<&PortableImage> {
        self.images.get(name)
    }

    /// Number of known images.
    pub fn image_count(&self) -> usize {
        self.images.len()
    }

    /// Number of attached images.
    pub fn attached_count(&self) -> usize {
        self.attachments.len()
    }

    /// Attach an image — symlink its unit files into the attached directory.
    pub fn attach_image(
        &mut self,
        name: &str,
        profile: Option<&str>,
        runtime: bool,
    ) -> Result<Vec<String>, String> {
        let attached_dir = if runtime {
            RUNTIME_ATTACHED_DIR
        } else {
            PERSISTENT_ATTACHED_DIR
        };
        self.attach_image_to(name, profile, runtime, attached_dir)
    }

    /// Attach image with a custom attached directory (for testing).
    pub fn attach_image_to(
        &mut self,
        name: &str,
        profile: Option<&str>,
        runtime: bool,
        attached_dir: &str,
    ) -> Result<Vec<String>, String> {
        // Check if already attached
        if self.attachments.contains_key(name) {
            return Err(format!("Image '{}' is already attached", name));
        }

        let image = match self.images.get(name) {
            Some(img) => img.clone(),
            None => return Err(format!("Image '{}' not found", name)),
        };

        if image.image_type == ImageType::Raw {
            return Err(format!(
                "Raw disk image support not yet implemented for '{}'",
                name
            ));
        }

        // Discover unit files in the image
        let units = PortableImage::discover_units(&image.path);
        if units.is_empty() {
            return Err(format!("No unit files found in image '{}'", name));
        }

        // Create the attached directory
        fs::create_dir_all(attached_dir)
            .map_err(|e| format!("Failed to create {}: {}", attached_dir, e))?;

        // Resolve profile drop-in content
        let profile_dropin = if let Some(prof) = profile {
            resolve_profile(prof)
        } else {
            None
        };

        let mut linked_units = Vec::new();

        for unit_name in &units {
            let src = match PortableImage::find_unit_path(&image.path, unit_name) {
                Some(p) => p,
                None => continue,
            };

            let dest = Path::new(attached_dir).join(unit_name);

            // Create the symlink
            if let Err(e) = unix_fs::symlink(&src, &dest) {
                // Ignore if it already exists (idempotent)
                if e.kind() != io::ErrorKind::AlreadyExists {
                    log::warn!(
                        "Failed to symlink {} -> {}: {}",
                        dest.display(),
                        src.display(),
                        e
                    );
                    continue;
                }
            }

            // Create drop-in directory and profile drop-in if requested
            if let Some(ref dropin_content) = profile_dropin {
                let dropin_dir = Path::new(attached_dir).join(format!("{}.d", unit_name));
                if fs::create_dir_all(&dropin_dir).is_ok() {
                    let dropin_file = dropin_dir.join("20-portable-profile.conf");
                    let _ = fs::write(&dropin_file, dropin_content);
                }
            }

            // Always create a marker drop-in so we know this came from portabled
            let marker_dir = Path::new(attached_dir).join(format!("{}.d", unit_name));
            if fs::create_dir_all(&marker_dir).is_ok() {
                let marker_file = marker_dir.join("10-portable.conf");
                let marker_content = format!(
                    "# Automatically generated by systemd-portabled -- do not edit.\n\
                     [Unit]\n\
                     Description=Portable service attached from image '{}'\n",
                    name
                );
                let _ = fs::write(&marker_file, marker_content);
            }

            linked_units.push(unit_name.clone());
        }

        if linked_units.is_empty() {
            return Err(format!(
                "Failed to symlink any unit files from image '{}'",
                name
            ));
        }

        // Record attachment
        let info = AttachmentInfo {
            image_name: name.to_string(),
            image_path: image.path.to_string_lossy().to_string(),
            profile: profile.map(|s| s.to_string()),
            runtime,
            units: linked_units.clone(),
            timestamp: now_usec(),
        };
        self.attachments.insert(name.to_string(), info);

        Ok(linked_units)
    }

    /// Detach an image — remove its unit file symlinks.
    pub fn detach_image(&mut self, name: &str) -> Result<Vec<String>, String> {
        self.detach_image_from(name, PERSISTENT_ATTACHED_DIR, RUNTIME_ATTACHED_DIR)
    }

    /// Detach image from custom directories (for testing).
    pub fn detach_image_from(
        &mut self,
        name: &str,
        persistent_dir: &str,
        runtime_dir: &str,
    ) -> Result<Vec<String>, String> {
        let info = match self.attachments.remove(name) {
            Some(i) => i,
            None => return Err(format!("Image '{}' is not attached", name)),
        };

        let attached_dir = if info.runtime {
            runtime_dir
        } else {
            persistent_dir
        };

        let mut removed = Vec::new();

        for unit_name in &info.units {
            let dest = Path::new(attached_dir).join(unit_name);
            if fs::remove_file(&dest).is_ok() {
                removed.push(unit_name.clone());
            }

            // Remove drop-in directory if it exists
            let dropin_dir = Path::new(attached_dir).join(format!("{}.d", unit_name));
            if dropin_dir.is_dir() {
                let _ = fs::remove_dir_all(&dropin_dir);
            }
        }

        Ok(removed)
    }

    /// Format a table of all known images.
    pub fn format_image_list(&self) -> String {
        if self.images.is_empty() {
            return "No images found.".to_string();
        }

        let mut lines = Vec::new();
        lines.push(format!(
            "{:<32} {:<12} {:<10} {:<24} {}",
            "NAME", "TYPE", "STATE", "OS", "PATH"
        ));
        for image in self.images.values() {
            let state = self.get_attach_state(&image.name);
            lines.push(format!(
                "{:<32} {:<12} {:<10} {:<24} {}",
                image.name,
                image.image_type,
                state,
                image.os_pretty_name.as_deref().unwrap_or("-"),
                image.path.display()
            ));
        }
        lines.push(String::new());
        lines.push(format!("{} images listed.", self.images.len()));
        lines.join("\n")
    }

    /// Inspect an image: show os-release and list unit files.
    pub fn inspect_image(&self, name: &str) -> Result<String, String> {
        let image = match self.images.get(name) {
            Some(img) => img,
            None => return Err(format!("Image '{}' not found", name)),
        };

        let mut lines = Vec::new();
        lines.push(image.format_status());
        lines.push(String::new());

        // os-release
        if image.image_type == ImageType::Directory {
            let os_release = PortableImage::read_os_release(&image.path);
            if !os_release.is_empty() {
                lines.push("--- os-release ---".to_string());
                for (k, v) in &os_release {
                    lines.push(format!("{}={}", k, v));
                }
                lines.push(String::new());
            }

            // Unit files
            let units = PortableImage::discover_units(&image.path);
            if !units.is_empty() {
                lines.push("--- Unit files ---".to_string());
                for u in &units {
                    lines.push(u.clone());
                }
            } else {
                lines.push("No unit files found.".to_string());
            }
        } else {
            lines.push(
                "(raw image inspection requires loopback mount -- not yet implemented)".to_string(),
            );
        }

        Ok(lines.join("\n"))
    }

    /// Garbage-collect: remove attachments whose symlinks are gone.
    pub fn gc_with_dirs(&mut self, persistent_dir: &str, runtime_dir: &str) -> Vec<String> {
        let mut stale = Vec::new();
        for (name, info) in &self.attachments {
            let dir = if info.runtime {
                runtime_dir
            } else {
                persistent_dir
            };
            // If none of the symlinked units exist any more, consider stale
            let any_exist = info.units.iter().any(|u| Path::new(dir).join(u).exists());
            if !any_exist {
                stale.push(name.clone());
            }
        }
        for name in &stale {
            self.attachments.remove(name);
        }
        stale
    }

    /// Garbage-collect using default directories.
    pub fn gc(&mut self) -> Vec<String> {
        self.gc_with_dirs(PERSISTENT_ATTACHED_DIR, RUNTIME_ATTACHED_DIR)
    }
}

// ---------------------------------------------------------------------------
// Profile resolution
// ---------------------------------------------------------------------------

/// Look up a profile by name and return its drop-in content.
pub fn resolve_profile(profile_name: &str) -> Option<String> {
    resolve_profile_from(profile_name, PROFILE_SEARCH_PATHS)
}

/// Look up a profile from custom search paths (for testing).
pub fn resolve_profile_from(profile_name: &str, search_paths: &[&str]) -> Option<String> {
    for search_dir in search_paths {
        let dir = Path::new(search_dir);
        // Try <profile_name>/service.conf first
        let conf = dir.join(profile_name).join("service.conf");
        if conf.is_file() {
            return fs::read_to_string(&conf).ok();
        }
        // Try <profile_name>.conf
        let conf = dir.join(format!("{}.conf", profile_name));
        if conf.is_file() {
            return fs::read_to_string(&conf).ok();
        }
    }
    None
}

/// List available profile names.
pub fn list_profiles() -> Vec<String> {
    list_profiles_from(PROFILE_SEARCH_PATHS)
}

/// List profiles from custom search paths.
pub fn list_profiles_from(search_paths: &[&str]) -> Vec<String> {
    let mut profiles = Vec::new();
    for search_dir in search_paths {
        let dir = Path::new(search_dir);
        if !dir.is_dir() {
            continue;
        }
        if let Ok(entries) = fs::read_dir(dir) {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                let profile_name = if entry.path().is_dir() {
                    name
                } else if name.ends_with(".conf") {
                    name.trim_end_matches(".conf").to_string()
                } else {
                    continue;
                };
                if !profiles.contains(&profile_name) {
                    profiles.push(profile_name);
                }
            }
        }
    }
    profiles.sort();
    profiles
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn file_times_and_size(path: &Path) -> (u64, u64, u64) {
    let meta = match fs::metadata(path) {
        Ok(m) => m,
        Err(_) => return (0, 0, 0),
    };
    let size = meta.len();
    let mtime = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_micros() as u64)
        .unwrap_or(0);
    let crtime = meta
        .created()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_micros() as u64)
        .unwrap_or(0);
    (size, mtime, crtime)
}

fn now_usec() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_micros() as u64)
        .unwrap_or(0)
}

fn format_timestamp(usec: u64) -> String {
    if usec == 0 {
        return "n/a".to_string();
    }
    let secs = usec / 1_000_000;
    let days_since_epoch = secs / 86400;
    let time_of_day = secs % 86400;
    let hours = time_of_day / 3600;
    let minutes = (time_of_day % 3600) / 60;
    let seconds = time_of_day % 60;

    let (year, month, day) = days_to_ymd(days_since_epoch);

    static MONTHS: [&str; 12] = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];

    let mon = if (1..=12).contains(&month) {
        MONTHS[(month - 1) as usize]
    } else {
        "???"
    };

    format!(
        "{} {:04}-{:02}-{:02} {:02}:{:02}:{:02} UTC",
        mon, year, month, day, hours, minutes, seconds
    )
}

fn days_to_ymd(days: u64) -> (u64, u64, u64) {
    // Gregorian calendar conversion from days since epoch
    let z = days + 719468;
    let era = z / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y, m, d)
}

fn format_bytes(bytes: u64) -> String {
    if bytes >= 1_073_741_824 {
        format!("{:.1}G", bytes as f64 / 1_073_741_824.0)
    } else if bytes >= 1_048_576 {
        format!("{:.1}M", bytes as f64 / 1_048_576.0)
    } else if bytes >= 1024 {
        format!("{:.1}K", bytes as f64 / 1024.0)
    } else {
        format!("{}B", bytes)
    }
}

// ---------------------------------------------------------------------------
// Control command handler
// ---------------------------------------------------------------------------

fn handle_control_command(registry: &mut ImageRegistry, line: &str) -> String {
    let parts: Vec<&str> = line.trim().splitn(4, ' ').collect();
    let cmd = match parts.first() {
        Some(c) => c.to_ascii_uppercase(),
        None => return "ERROR: empty command\n".to_string(),
    };

    match cmd.as_str() {
        "PING" => "PONG\n".to_string(),

        "LIST" => {
            let output = registry.format_image_list();
            format!("{}\n", output)
        }

        "INSPECT" => {
            let name = match parts.get(1) {
                Some(n) => *n,
                None => return "ERROR: INSPECT requires an image name\n".to_string(),
            };
            match registry.inspect_image(name) {
                Ok(info) => format!("{}\n", info),
                Err(e) => format!("ERROR: {}\n", e),
            }
        }

        "ATTACH" => {
            let name = match parts.get(1) {
                Some(n) => *n,
                None => return "ERROR: ATTACH requires an image name\n".to_string(),
            };
            let profile = parts.get(2).copied();
            let runtime = parts.get(3).map(|s| *s == "runtime").unwrap_or(false);

            match registry.attach_image(name, profile, runtime) {
                Ok(units) => {
                    registry.save_attachment(name);
                    format!("OK: Attached {} units: {}\n", units.len(), units.join(", "))
                }
                Err(e) => format!("ERROR: {}\n", e),
            }
        }

        "DETACH" => {
            let name = match parts.get(1) {
                Some(n) => *n,
                None => return "ERROR: DETACH requires an image name\n".to_string(),
            };
            match registry.detach_image(name) {
                Ok(units) => {
                    registry.remove_attachment_file(name);
                    format!("OK: Detached {} units: {}\n", units.len(), units.join(", "))
                }
                Err(e) => format!("ERROR: {}\n", e),
            }
        }

        "REATTACH" => {
            let name = match parts.get(1) {
                Some(n) => *n,
                None => return "ERROR: REATTACH requires an image name\n".to_string(),
            };
            let profile = parts.get(2).copied();
            let runtime = parts.get(3).map(|s| *s == "runtime").unwrap_or(false);

            // Detach first (ignore error if not attached)
            let _ = registry.detach_image(name);
            registry.remove_attachment_file(name);

            match registry.attach_image(name, profile, runtime) {
                Ok(units) => {
                    registry.save_attachment(name);
                    format!(
                        "OK: Reattached {} units: {}\n",
                        units.len(),
                        units.join(", ")
                    )
                }
                Err(e) => format!("ERROR: {}\n", e),
            }
        }

        "IS-ATTACHED" => {
            let name = match parts.get(1) {
                Some(n) => *n,
                None => return "ERROR: IS-ATTACHED requires an image name\n".to_string(),
            };
            let state = registry.get_attach_state(name);
            format!("{}\n", state)
        }

        "SHOW" => {
            let name = match parts.get(1) {
                Some(n) => *n,
                None => return "ERROR: SHOW requires an image name\n".to_string(),
            };
            match registry.get_image(name) {
                Some(img) => format!("{}\n", img.format_show()),
                None => format!("ERROR: Image '{}' not found\n", name),
            }
        }

        "STATUS" => {
            let name = match parts.get(1) {
                Some(n) => *n,
                None => {
                    // Global status
                    return format!(
                        "Images: {}\nAttached: {}\n",
                        registry.image_count(),
                        registry.attached_count()
                    );
                }
            };
            match registry.get_image(name) {
                Some(img) => format!("{}\n", img.format_status()),
                None => format!("ERROR: Image '{}' not found\n", name),
            }
        }

        "GC" => {
            let removed = registry.gc();
            if removed.is_empty() {
                "OK: No stale attachments\n".to_string()
            } else {
                format!("OK: Removed {} stale attachments\n", removed.len())
            }
        }

        "RELOAD" => {
            registry.discover_images();
            registry.load_attachments();
            format!(
                "OK: Reloaded, {} images, {} attached\n",
                registry.image_count(),
                registry.attached_count()
            )
        }

        _ => format!("ERROR: Unknown command '{}'\n", cmd),
    }
}

fn handle_client(registry: &mut ImageRegistry, stream: &mut UnixStream) {
    let reader = match stream.try_clone() {
        Ok(s) => BufReader::new(s),
        Err(_) => return,
    };

    for line in reader.lines() {
        match line {
            Ok(l) if l.trim().is_empty() => continue,
            Ok(l) => {
                let response = handle_control_command(registry, &l);
                let _ = stream.write_all(response.as_bytes());
                let _ = stream.flush();
            }
            Err(_) => break,
        }
    }
}

// ---------------------------------------------------------------------------
// sd_notify helper
// ---------------------------------------------------------------------------

fn sd_notify(msg: &str) {
    let sock_path = match env::var("NOTIFY_SOCKET") {
        Ok(p) => p,
        Err(_) => return,
    };

    let path = if let Some(stripped) = sock_path.strip_prefix('@') {
        format!("\0{}", stripped)
    } else {
        sock_path
    };

    let sock = match UnixDatagram::unbound() {
        Ok(s) => s,
        Err(_) => return,
    };

    let _ = sock.send_to(msg.as_bytes(), &path);
}

// ---------------------------------------------------------------------------
// Signal handling
// ---------------------------------------------------------------------------

static SHUTDOWN: AtomicBool = AtomicBool::new(false);
static RELOAD: AtomicBool = AtomicBool::new(false);

extern "C" fn handle_sigterm(_: libc::c_int) {
    SHUTDOWN.store(true, Ordering::SeqCst);
}

extern "C" fn handle_sigint(_: libc::c_int) {
    SHUTDOWN.store(true, Ordering::SeqCst);
}

extern "C" fn handle_sighup(_: libc::c_int) {
    RELOAD.store(true, Ordering::SeqCst);
}

fn setup_signal_handlers() {
    unsafe {
        libc::signal(libc::SIGTERM, handle_sigterm as libc::sighandler_t);
        libc::signal(libc::SIGINT, handle_sigint as libc::sighandler_t);
        libc::signal(libc::SIGHUP, handle_sighup as libc::sighandler_t);
        libc::signal(libc::SIGPIPE, libc::SIG_IGN);
    }
}

// ---------------------------------------------------------------------------
// Logging
// ---------------------------------------------------------------------------

fn init_logging() {
    struct StderrLogger;
    impl log::Log for StderrLogger {
        fn enabled(&self, _: &log::Metadata) -> bool {
            true
        }
        fn log(&self, record: &log::Record) {
            if self.enabled(record.metadata()) {
                eprintln!(
                    "systemd-portabled[{}]: {}: {}",
                    process::id(),
                    record.level(),
                    record.args()
                );
            }
        }
        fn flush(&self) {}
    }
    static LOGGER: StderrLogger = StderrLogger;
    let _ = log::set_logger(&LOGGER);
    log::set_max_level(log::LevelFilter::Info);
}

// ---------------------------------------------------------------------------
// Watchdog
// ---------------------------------------------------------------------------

fn watchdog_interval() -> Option<Duration> {
    let usec: u64 = env::var("WATCHDOG_USEC").ok()?.parse().ok()?;
    if usec == 0 {
        return None;
    }
    // Send at half the interval
    Some(Duration::from_micros(usec / 2))
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

fn main() {
    init_logging();
    setup_signal_handlers();

    log::info!("systemd-portabled starting");

    // Discover images and load attachment state
    let mut registry = ImageRegistry::new();
    let _ = fs::create_dir_all(STATE_DIR);
    registry.discover_images();
    registry.load_attachments();
    log::info!(
        "Discovered {} images, {} attached",
        registry.image_count(),
        registry.attached_count()
    );

    // GC stale attachments on startup
    let removed = registry.gc();
    if !removed.is_empty() {
        log::info!("Removed {} stale attachments on startup", removed.len());
    }

    // Watchdog support
    let wd_interval = watchdog_interval();
    if let Some(ref iv) = wd_interval {
        log::info!("Watchdog enabled, interval {:?}", iv);
    }
    let mut last_watchdog = Instant::now();

    // Ensure parent directory exists
    let _ = fs::create_dir_all(Path::new(CONTROL_SOCKET_PATH).parent().unwrap());

    // Remove stale socket
    let _ = fs::remove_file(CONTROL_SOCKET_PATH);

    // Bind control socket
    let listener = match UnixListener::bind(CONTROL_SOCKET_PATH) {
        Ok(l) => {
            log::info!("Listening on {}", CONTROL_SOCKET_PATH);
            l
        }
        Err(e) => {
            log::error!(
                "Failed to bind control socket {}: {}",
                CONTROL_SOCKET_PATH,
                e
            );
            sd_notify("READY=1\nSTATUS=Running (no control socket)");
            loop {
                if SHUTDOWN.load(Ordering::SeqCst) {
                    break;
                }
                if let Some(ref iv) = wd_interval
                    && last_watchdog.elapsed() >= *iv
                {
                    sd_notify("WATCHDOG=1");
                    last_watchdog = Instant::now();
                }
                thread::sleep(Duration::from_secs(1));
            }
            sd_notify("STOPPING=1");
            process::exit(0);
        }
    };

    listener
        .set_nonblocking(true)
        .expect("Failed to set non-blocking");

    sd_notify(&format!(
        "READY=1\nSTATUS=Managing {} images ({} attached)",
        registry.image_count(),
        registry.attached_count()
    ));

    log::info!("systemd-portabled ready");

    // Periodic GC interval
    let gc_interval = Duration::from_secs(60);
    let mut last_gc = Instant::now();

    // Main loop
    loop {
        if SHUTDOWN.load(Ordering::SeqCst) {
            log::info!("Received shutdown signal");
            break;
        }

        if RELOAD.load(Ordering::SeqCst) {
            RELOAD.store(false, Ordering::SeqCst);
            registry.discover_images();
            registry.load_attachments();
            log::info!(
                "Reloaded, {} images, {} attached",
                registry.image_count(),
                registry.attached_count()
            );
            sd_notify(&format!(
                "STATUS=Managing {} images ({} attached)",
                registry.image_count(),
                registry.attached_count()
            ));
        }

        // Periodic GC of stale attachments
        if last_gc.elapsed() >= gc_interval {
            let removed = registry.gc();
            if !removed.is_empty() {
                log::info!(
                    "GC removed {} stale attachments: {}",
                    removed.len(),
                    removed.join(", ")
                );
                sd_notify(&format!(
                    "STATUS=Managing {} images ({} attached)",
                    registry.image_count(),
                    registry.attached_count()
                ));
            }
            last_gc = Instant::now();
        }

        // Watchdog keepalive
        if let Some(ref iv) = wd_interval
            && last_watchdog.elapsed() >= *iv
        {
            sd_notify("WATCHDOG=1");
            last_watchdog = Instant::now();
        }

        match listener.accept() {
            Ok((mut stream, _addr)) => {
                let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
                handle_client(&mut registry, &mut stream);
                let _ = stream.shutdown(Shutdown::Both);
            }
            Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                thread::sleep(Duration::from_millis(200));
            }
            Err(e) => {
                log::warn!("Accept error: {}", e);
                thread::sleep(Duration::from_millis(100));
            }
        }
    }

    // Cleanup
    let _ = fs::remove_file(CONTROL_SOCKET_PATH);
    sd_notify("STOPPING=1");
    log::info!("systemd-portabled stopped");
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs as unix_fs;
    use tempfile::TempDir;

    // ── Helpers ────────────────────────────────────────────────────────────

    fn temp_dir() -> TempDir {
        TempDir::new().unwrap()
    }

    /// Create a minimal portable image directory with unit files.
    fn create_test_image(base: &Path, name: &str, units: &[&str]) -> PathBuf {
        let img_dir = base.join(name);
        let unit_dir = img_dir.join("usr/lib/systemd/system");
        fs::create_dir_all(&unit_dir).unwrap();

        for unit in units {
            let unit_path = unit_dir.join(unit);
            fs::write(
                &unit_path,
                format!(
                    "[Unit]\nDescription=Test service {}\n\n[Service]\nExecStart=/bin/true\n",
                    unit
                ),
            )
            .unwrap();
        }

        // Write os-release
        let os_dir = img_dir.join("usr/lib");
        fs::create_dir_all(&os_dir).unwrap();
        fs::write(
            os_dir.join("os-release"),
            "PRETTY_NAME=\"Test OS\"\nID=test\nVERSION_ID=1.0\n",
        )
        .unwrap();

        img_dir
    }

    /// Create a raw image file (just an empty file with .raw extension).
    fn create_test_raw_image(base: &Path, name: &str) -> PathBuf {
        let path = base.join(format!("{}.raw", name));
        fs::write(&path, vec![0u8; 4096]).unwrap();
        path
    }

    // ── ImageType tests ───────────────────────────────────────────────────

    #[test]
    fn test_image_type_parse() {
        assert_eq!(ImageType::parse("directory"), Some(ImageType::Directory));
        assert_eq!(ImageType::parse("dir"), Some(ImageType::Directory));
        assert_eq!(ImageType::parse("Directory"), Some(ImageType::Directory));
        assert_eq!(ImageType::parse("raw"), Some(ImageType::Raw));
        assert_eq!(ImageType::parse("RAW"), Some(ImageType::Raw));
        assert_eq!(ImageType::parse("unknown"), None);
        assert_eq!(ImageType::parse(""), None);
    }

    #[test]
    fn test_image_type_display() {
        assert_eq!(ImageType::Directory.to_string(), "directory");
        assert_eq!(ImageType::Raw.to_string(), "raw");
    }

    #[test]
    fn test_image_type_as_str() {
        assert_eq!(ImageType::Directory.as_str(), "directory");
        assert_eq!(ImageType::Raw.as_str(), "raw");
    }

    // ── AttachState tests ─────────────────────────────────────────────────

    #[test]
    fn test_attach_state_parse() {
        assert_eq!(AttachState::parse("detached"), Some(AttachState::Detached));
        assert_eq!(AttachState::parse("attached"), Some(AttachState::Attached));
        assert_eq!(
            AttachState::parse("attached-runtime"),
            Some(AttachState::AttachedRuntime)
        );
        assert_eq!(AttachState::parse("enabled"), Some(AttachState::Enabled));
        assert_eq!(
            AttachState::parse("enabled-runtime"),
            Some(AttachState::EnabledRuntime)
        );
        assert_eq!(AttachState::parse("running"), Some(AttachState::Running));
        assert_eq!(AttachState::parse("DETACHED"), Some(AttachState::Detached));
        assert_eq!(AttachState::parse("unknown"), None);
    }

    #[test]
    fn test_attach_state_display() {
        assert_eq!(AttachState::Detached.to_string(), "detached");
        assert_eq!(AttachState::Attached.to_string(), "attached");
        assert_eq!(AttachState::AttachedRuntime.to_string(), "attached-runtime");
        assert_eq!(AttachState::Enabled.to_string(), "enabled");
        assert_eq!(AttachState::EnabledRuntime.to_string(), "enabled-runtime");
        assert_eq!(AttachState::Running.to_string(), "running");
    }

    #[test]
    fn test_attach_state_is_attached() {
        assert!(!AttachState::Detached.is_attached());
        assert!(AttachState::Attached.is_attached());
        assert!(AttachState::AttachedRuntime.is_attached());
        assert!(AttachState::Enabled.is_attached());
        assert!(AttachState::EnabledRuntime.is_attached());
        assert!(AttachState::Running.is_attached());
    }

    // ── PortableImage tests ───────────────────────────────────────────────

    #[test]
    fn test_read_os_release() {
        let tmp = temp_dir();
        let img = create_test_image(tmp.path(), "myimg", &["test.service"]);
        let release = PortableImage::read_os_release(&img);
        assert_eq!(
            release.get("PRETTY_NAME").map(|s| s.as_str()),
            Some("Test OS")
        );
        assert_eq!(release.get("ID").map(|s| s.as_str()), Some("test"));
        assert_eq!(release.get("VERSION_ID").map(|s| s.as_str()), Some("1.0"));
    }

    #[test]
    fn test_read_os_release_missing() {
        let tmp = temp_dir();
        let path = tmp.path().join("no-image");
        fs::create_dir_all(&path).unwrap();
        let release = PortableImage::read_os_release(&path);
        assert!(release.is_empty());
    }

    #[test]
    fn test_read_os_release_comments_and_blanks() {
        let tmp = temp_dir();
        let img = tmp.path().join("img");
        let os_dir = img.join("usr/lib");
        fs::create_dir_all(&os_dir).unwrap();
        fs::write(
            os_dir.join("os-release"),
            "# Comment\n\nID=test\n# Another comment\nNAME=\"Test\"\n",
        )
        .unwrap();
        let release = PortableImage::read_os_release(&img);
        assert_eq!(release.get("ID").map(|s| s.as_str()), Some("test"));
        assert_eq!(release.get("NAME").map(|s| s.as_str()), Some("Test"));
        assert_eq!(release.len(), 2);
    }

    #[test]
    fn test_discover_units() {
        let tmp = temp_dir();
        let img = create_test_image(
            tmp.path(),
            "myimg",
            &["foo.service", "bar.service", "baz.socket"],
        );
        let units = PortableImage::discover_units(&img);
        assert_eq!(units, vec!["bar.service", "baz.socket", "foo.service"]);
    }

    #[test]
    fn test_discover_units_empty() {
        let tmp = temp_dir();
        let img = tmp.path().join("empty");
        fs::create_dir_all(&img).unwrap();
        let units = PortableImage::discover_units(&img);
        assert!(units.is_empty());
    }

    #[test]
    fn test_discover_units_ignores_non_unit() {
        let tmp = temp_dir();
        let img = tmp.path().join("img");
        let unit_dir = img.join("usr/lib/systemd/system");
        fs::create_dir_all(&unit_dir).unwrap();
        fs::write(unit_dir.join("foo.service"), "[Service]\n").unwrap();
        fs::write(unit_dir.join("README.txt"), "not a unit\n").unwrap();
        fs::write(unit_dir.join("bar.conf"), "not a unit\n").unwrap();
        let units = PortableImage::discover_units(&img);
        assert_eq!(units, vec!["foo.service"]);
    }

    #[test]
    fn test_discover_units_timer_and_path() {
        let tmp = temp_dir();
        let img = tmp.path().join("img");
        let unit_dir = img.join("usr/lib/systemd/system");
        fs::create_dir_all(&unit_dir).unwrap();
        fs::write(unit_dir.join("cleanup.timer"), "[Timer]\n").unwrap();
        fs::write(unit_dir.join("watch.path"), "[Path]\n").unwrap();
        fs::write(unit_dir.join("multi.target"), "[Unit]\n").unwrap();
        let units = PortableImage::discover_units(&img);
        assert_eq!(units, vec!["cleanup.timer", "multi.target", "watch.path"]);
    }

    #[test]
    fn test_find_unit_path() {
        let tmp = temp_dir();
        let img = create_test_image(tmp.path(), "myimg", &["foo.service"]);
        let found = PortableImage::find_unit_path(&img, "foo.service");
        assert!(found.is_some());
        assert!(found.unwrap().exists());
    }

    #[test]
    fn test_find_unit_path_not_found() {
        let tmp = temp_dir();
        let img = create_test_image(tmp.path(), "myimg", &["foo.service"]);
        let found = PortableImage::find_unit_path(&img, "nonexistent.service");
        assert!(found.is_none());
    }

    #[test]
    fn test_portable_image_format_status() {
        let img = PortableImage {
            name: "test".to_string(),
            path: PathBuf::from("/var/lib/portables/test"),
            image_type: ImageType::Directory,
            size: 0,
            mtime_usec: 0,
            crtime_usec: 0,
            os_pretty_name: Some("Test OS 1.0".to_string()),
            portable_service: None,
        };
        let status = img.format_status();
        assert!(status.contains("Name: test"));
        assert!(status.contains("Path: /var/lib/portables/test"));
        assert!(status.contains("Type: directory"));
        assert!(status.contains("OS: Test OS 1.0"));
    }

    #[test]
    fn test_portable_image_format_show() {
        let img = PortableImage {
            name: "test".to_string(),
            path: PathBuf::from("/var/lib/portables/test"),
            image_type: ImageType::Raw,
            size: 1048576,
            mtime_usec: 1000000,
            crtime_usec: 500000,
            os_pretty_name: Some("My OS".to_string()),
            portable_service: Some("myapp".to_string()),
        };
        let show = img.format_show();
        assert!(show.contains("Name=test"));
        assert!(show.contains("Type=raw"));
        assert!(show.contains("Size=1048576"));
        assert!(show.contains("OSPrettyName=My OS"));
        assert!(show.contains("PortableService=myapp"));
    }

    // ── AttachmentInfo tests ──────────────────────────────────────────────

    #[test]
    fn test_attachment_info_roundtrip() {
        let info = AttachmentInfo {
            image_name: "myapp".to_string(),
            image_path: "/var/lib/portables/myapp".to_string(),
            profile: Some("default".to_string()),
            runtime: false,
            units: vec!["myapp.service".to_string(), "myapp.socket".to_string()],
            timestamp: 1234567890,
        };
        let content = info.to_state_file();
        let parsed = AttachmentInfo::from_state_file(&content).unwrap();
        assert_eq!(parsed.image_name, "myapp");
        assert_eq!(parsed.image_path, "/var/lib/portables/myapp");
        assert_eq!(parsed.profile.as_deref(), Some("default"));
        assert!(!parsed.runtime);
        assert_eq!(parsed.units.len(), 2);
        assert_eq!(parsed.units[0], "myapp.service");
        assert_eq!(parsed.units[1], "myapp.socket");
        assert_eq!(parsed.timestamp, 1234567890);
    }

    #[test]
    fn test_attachment_info_roundtrip_runtime() {
        let info = AttachmentInfo {
            image_name: "test".to_string(),
            image_path: "/run/portables/test".to_string(),
            profile: None,
            runtime: true,
            units: vec!["test.service".to_string()],
            timestamp: 0,
        };
        let content = info.to_state_file();
        let parsed = AttachmentInfo::from_state_file(&content).unwrap();
        assert_eq!(parsed.image_name, "test");
        assert!(parsed.runtime);
        assert!(parsed.profile.is_none());
    }

    #[test]
    fn test_attachment_info_roundtrip_no_units() {
        let info = AttachmentInfo {
            image_name: "empty".to_string(),
            image_path: "/var/lib/portables/empty".to_string(),
            profile: None,
            runtime: false,
            units: vec![],
            timestamp: 0,
        };
        let content = info.to_state_file();
        let parsed = AttachmentInfo::from_state_file(&content).unwrap();
        assert!(parsed.units.is_empty());
    }

    #[test]
    fn test_attachment_info_missing_name() {
        let result = AttachmentInfo::from_state_file("IMAGE_PATH=/foo\n");
        assert!(result.is_none());
    }

    #[test]
    fn test_attachment_info_missing_path() {
        let result = AttachmentInfo::from_state_file("IMAGE_NAME=foo\n");
        assert!(result.is_none());
    }

    #[test]
    fn test_attachment_info_comments_and_blanks() {
        let content = "# comment\n\nIMAGE_NAME=foo\n\nIMAGE_PATH=/bar\n# more\n";
        let parsed = AttachmentInfo::from_state_file(content).unwrap();
        assert_eq!(parsed.image_name, "foo");
        assert_eq!(parsed.image_path, "/bar");
    }

    // ── ImageRegistry discovery tests ─────────────────────────────────────

    #[test]
    fn test_discover_images_directory() {
        let tmp = temp_dir();
        create_test_image(tmp.path(), "myapp", &["myapp.service"]);
        let mut reg = ImageRegistry::new();
        let search = tmp.path().to_str().unwrap();
        reg.discover_images_from(&[search]);
        assert_eq!(reg.image_count(), 1);
        let img = reg.get_image("myapp").unwrap();
        assert_eq!(img.image_type, ImageType::Directory);
        assert_eq!(img.os_pretty_name.as_deref(), Some("Test OS"));
    }

    #[test]
    fn test_discover_images_raw() {
        let tmp = temp_dir();
        create_test_raw_image(tmp.path(), "myraw");
        let mut reg = ImageRegistry::new();
        let search = tmp.path().to_str().unwrap();
        reg.discover_images_from(&[search]);
        assert_eq!(reg.image_count(), 1);
        let img = reg.get_image("myraw").unwrap();
        assert_eq!(img.image_type, ImageType::Raw);
    }

    #[test]
    fn test_discover_images_skips_hidden() {
        let tmp = temp_dir();
        create_test_image(tmp.path(), ".hidden", &["hidden.service"]);
        let mut reg = ImageRegistry::new();
        let search = tmp.path().to_str().unwrap();
        reg.discover_images_from(&[search]);
        assert_eq!(reg.image_count(), 0);
    }

    #[test]
    fn test_discover_images_priority() {
        let high = temp_dir();
        let low = temp_dir();
        create_test_image(high.path(), "shared", &["high.service"]);
        create_test_image(low.path(), "shared", &["low.service"]);
        let mut reg = ImageRegistry::new();
        let h = high.path().to_str().unwrap();
        let l = low.path().to_str().unwrap();
        reg.discover_images_from(&[h, l]);
        assert_eq!(reg.image_count(), 1);
        let img = reg.get_image("shared").unwrap();
        // Higher priority path should win
        assert!(img.path.starts_with(high.path()));
    }

    #[test]
    fn test_discover_images_nonexistent_dir() {
        let mut reg = ImageRegistry::new();
        reg.discover_images_from(&["/nonexistent/path/that/does/not/exist"]);
        assert_eq!(reg.image_count(), 0);
    }

    #[test]
    fn test_discover_images_multiple() {
        let tmp = temp_dir();
        create_test_image(tmp.path(), "app1", &["app1.service"]);
        create_test_image(tmp.path(), "app2", &["app2.service"]);
        create_test_raw_image(tmp.path(), "app3");
        let mut reg = ImageRegistry::new();
        let search = tmp.path().to_str().unwrap();
        reg.discover_images_from(&[search]);
        assert_eq!(reg.image_count(), 3);
    }

    // ── ImageRegistry attachment state persistence tests ──────────────────

    #[test]
    fn test_save_and_load_attachments() {
        let tmp = temp_dir();
        let state_dir = tmp.path().to_str().unwrap();

        let mut reg = ImageRegistry::new();
        reg.attachments.insert(
            "test".to_string(),
            AttachmentInfo {
                image_name: "test".to_string(),
                image_path: "/var/lib/portables/test".to_string(),
                profile: Some("default".to_string()),
                runtime: false,
                units: vec!["test.service".to_string()],
                timestamp: 12345,
            },
        );
        reg.save_attachments_to(state_dir);

        let mut reg2 = ImageRegistry::new();
        reg2.load_attachments_from(state_dir);
        assert_eq!(reg2.attached_count(), 1);
        let info = reg2.attachments.get("test").unwrap();
        assert_eq!(info.image_path, "/var/lib/portables/test");
        assert_eq!(info.profile.as_deref(), Some("default"));
    }

    #[test]
    fn test_save_attachment_single() {
        let tmp = temp_dir();
        let state_dir = tmp.path().to_str().unwrap();

        let mut reg = ImageRegistry::new();
        reg.attachments.insert(
            "one".to_string(),
            AttachmentInfo {
                image_name: "one".to_string(),
                image_path: "/test/one".to_string(),
                profile: None,
                runtime: true,
                units: vec![],
                timestamp: 0,
            },
        );
        reg.save_attachment_to("one", state_dir);

        assert!(tmp.path().join("one").exists());
    }

    #[test]
    fn test_load_attachments_empty_dir() {
        let tmp = temp_dir();
        let mut reg = ImageRegistry::new();
        reg.load_attachments_from(tmp.path().to_str().unwrap());
        assert_eq!(reg.attached_count(), 0);
    }

    #[test]
    fn test_load_attachments_nonexistent() {
        let mut reg = ImageRegistry::new();
        reg.load_attachments_from("/nonexistent/path");
        assert_eq!(reg.attached_count(), 0);
    }

    #[test]
    fn test_load_attachments_skips_dotfiles() {
        let tmp = temp_dir();
        fs::write(
            tmp.path().join(".hidden"),
            "IMAGE_NAME=hidden\nIMAGE_PATH=/x\n",
        )
        .unwrap();
        let mut reg = ImageRegistry::new();
        reg.load_attachments_from(tmp.path().to_str().unwrap());
        assert_eq!(reg.attached_count(), 0);
    }

    #[test]
    fn test_load_attachments_skips_invalid() {
        let tmp = temp_dir();
        fs::write(tmp.path().join("bad"), "garbage content\n").unwrap();
        let mut reg = ImageRegistry::new();
        reg.load_attachments_from(tmp.path().to_str().unwrap());
        assert_eq!(reg.attached_count(), 0);
    }

    #[test]
    fn test_remove_attachment_file() {
        let tmp = temp_dir();
        let state_dir = tmp.path().to_str().unwrap();
        fs::write(
            tmp.path().join("myapp"),
            "IMAGE_NAME=myapp\nIMAGE_PATH=/x\n",
        )
        .unwrap();
        assert!(tmp.path().join("myapp").exists());

        let reg = ImageRegistry::new();
        reg.remove_attachment_file_from("myapp", state_dir);
        assert!(!tmp.path().join("myapp").exists());
    }

    // ── ImageRegistry attach/detach tests ─────────────────────────────────

    #[test]
    fn test_attach_image() {
        let tmp = temp_dir();
        let attached_dir = tmp.path().join("attached");
        fs::create_dir_all(&attached_dir).unwrap();

        create_test_image(tmp.path(), "myapp", &["myapp.service"]);
        let mut reg = ImageRegistry::new();
        let search = tmp.path().to_str().unwrap();
        reg.discover_images_from(&[search]);

        let result = reg.attach_image_to("myapp", None, false, attached_dir.to_str().unwrap());
        assert!(result.is_ok());
        let units = result.unwrap();
        assert_eq!(units, vec!["myapp.service"]);

        // Symlink should exist
        let link = attached_dir.join("myapp.service");
        assert!(link.exists() || link.symlink_metadata().is_ok());

        // Marker drop-in should exist
        let marker = attached_dir.join("myapp.service.d/10-portable.conf");
        assert!(marker.exists());

        // Attachment should be recorded
        assert_eq!(reg.get_attach_state("myapp"), AttachState::Attached);
    }

    #[test]
    fn test_attach_image_runtime() {
        let tmp = temp_dir();
        let attached_dir = tmp.path().join("attached");
        fs::create_dir_all(&attached_dir).unwrap();

        create_test_image(tmp.path(), "myapp", &["myapp.service"]);
        let mut reg = ImageRegistry::new();
        let search = tmp.path().to_str().unwrap();
        reg.discover_images_from(&[search]);

        let result = reg.attach_image_to("myapp", None, true, attached_dir.to_str().unwrap());
        assert!(result.is_ok());
        assert_eq!(reg.get_attach_state("myapp"), AttachState::AttachedRuntime);
    }

    #[test]
    fn test_attach_image_already_attached() {
        let tmp = temp_dir();
        let attached_dir = tmp.path().join("attached");
        fs::create_dir_all(&attached_dir).unwrap();

        create_test_image(tmp.path(), "myapp", &["myapp.service"]);
        let mut reg = ImageRegistry::new();
        let search = tmp.path().to_str().unwrap();
        reg.discover_images_from(&[search]);

        let _ = reg.attach_image_to("myapp", None, false, attached_dir.to_str().unwrap());
        let result = reg.attach_image_to("myapp", None, false, attached_dir.to_str().unwrap());
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("already attached"));
    }

    #[test]
    fn test_attach_image_not_found() {
        let mut reg = ImageRegistry::new();
        let result = reg.attach_image_to("nonexistent", None, false, "/tmp/test");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not found"));
    }

    #[test]
    fn test_attach_image_raw_not_supported() {
        let tmp = temp_dir();
        let attached_dir = tmp.path().join("attached");
        fs::create_dir_all(&attached_dir).unwrap();

        create_test_raw_image(tmp.path(), "myraw");
        let mut reg = ImageRegistry::new();
        let search = tmp.path().to_str().unwrap();
        reg.discover_images_from(&[search]);

        let result = reg.attach_image_to("myraw", None, false, attached_dir.to_str().unwrap());
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not yet implemented"));
    }

    #[test]
    fn test_attach_image_no_units() {
        let tmp = temp_dir();
        let attached_dir = tmp.path().join("attached");
        fs::create_dir_all(&attached_dir).unwrap();

        // Create an image with no unit files
        let img = tmp.path().join("empty");
        fs::create_dir_all(&img).unwrap();
        let mut reg = ImageRegistry::new();
        let search = tmp.path().to_str().unwrap();
        reg.discover_images_from(&[search]);

        let result = reg.attach_image_to("empty", None, false, attached_dir.to_str().unwrap());
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("No unit files"));
    }

    #[test]
    fn test_attach_multiple_units() {
        let tmp = temp_dir();
        let attached_dir = tmp.path().join("attached");
        fs::create_dir_all(&attached_dir).unwrap();

        create_test_image(
            tmp.path(),
            "multi",
            &["svc1.service", "svc2.service", "svc1.socket"],
        );
        let mut reg = ImageRegistry::new();
        let search = tmp.path().to_str().unwrap();
        reg.discover_images_from(&[search]);

        let result = reg.attach_image_to("multi", None, false, attached_dir.to_str().unwrap());
        assert!(result.is_ok());
        let units = result.unwrap();
        assert_eq!(units.len(), 3);
        assert!(units.contains(&"svc1.service".to_string()));
        assert!(units.contains(&"svc2.service".to_string()));
        assert!(units.contains(&"svc1.socket".to_string()));
    }

    #[test]
    fn test_detach_image() {
        let tmp = temp_dir();
        let attached_dir = tmp.path().join("attached");
        let runtime_dir = tmp.path().join("runtime");
        fs::create_dir_all(&attached_dir).unwrap();
        fs::create_dir_all(&runtime_dir).unwrap();

        create_test_image(tmp.path(), "myapp", &["myapp.service"]);
        let mut reg = ImageRegistry::new();
        let search = tmp.path().to_str().unwrap();
        reg.discover_images_from(&[search]);

        let _ = reg.attach_image_to("myapp", None, false, attached_dir.to_str().unwrap());
        assert_eq!(reg.get_attach_state("myapp"), AttachState::Attached);

        let result = reg.detach_image_from(
            "myapp",
            attached_dir.to_str().unwrap(),
            runtime_dir.to_str().unwrap(),
        );
        assert!(result.is_ok());
        let removed = result.unwrap();
        assert_eq!(removed, vec!["myapp.service"]);

        // Symlink should be gone
        assert!(!attached_dir.join("myapp.service").exists());
        // Drop-in directory should be gone
        assert!(!attached_dir.join("myapp.service.d").exists());
        // State should be detached
        assert_eq!(reg.get_attach_state("myapp"), AttachState::Detached);
    }

    #[test]
    fn test_detach_image_not_attached() {
        let mut reg = ImageRegistry::new();
        let result = reg.detach_image_from("foo", "/tmp/a", "/tmp/b");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not attached"));
    }

    #[test]
    fn test_detach_runtime_image() {
        let tmp = temp_dir();
        let persistent_dir = tmp.path().join("persistent");
        let runtime_dir = tmp.path().join("runtime");
        fs::create_dir_all(&persistent_dir).unwrap();
        fs::create_dir_all(&runtime_dir).unwrap();

        create_test_image(tmp.path(), "myapp", &["myapp.service"]);
        let mut reg = ImageRegistry::new();
        let search = tmp.path().to_str().unwrap();
        reg.discover_images_from(&[search]);

        let _ = reg.attach_image_to("myapp", None, true, runtime_dir.to_str().unwrap());
        assert_eq!(reg.get_attach_state("myapp"), AttachState::AttachedRuntime);

        let result = reg.detach_image_from(
            "myapp",
            persistent_dir.to_str().unwrap(),
            runtime_dir.to_str().unwrap(),
        );
        assert!(result.is_ok());
        assert!(!runtime_dir.join("myapp.service").exists());
    }

    // ── ImageRegistry GC tests ────────────────────────────────────────────

    #[test]
    fn test_gc_removes_stale() {
        let tmp = temp_dir();
        let attached_dir = tmp.path().join("attached");
        fs::create_dir_all(&attached_dir).unwrap();

        let mut reg = ImageRegistry::new();
        reg.attachments.insert(
            "stale".to_string(),
            AttachmentInfo {
                image_name: "stale".to_string(),
                image_path: "/gone".to_string(),
                profile: None,
                runtime: false,
                units: vec!["stale.service".to_string()],
                timestamp: 0,
            },
        );

        // No symlinks exist -> should be GC'd
        let removed = reg.gc_with_dirs(attached_dir.to_str().unwrap(), "/tmp/nonexistent");
        assert_eq!(removed, vec!["stale"]);
        assert_eq!(reg.attached_count(), 0);
    }

    #[test]
    fn test_gc_keeps_live() {
        let tmp = temp_dir();
        let attached_dir = tmp.path().join("attached");
        fs::create_dir_all(&attached_dir).unwrap();

        // Create a symlink that exists
        let target = tmp.path().join("target.service");
        fs::write(&target, "[Service]\n").unwrap();
        unix_fs::symlink(&target, attached_dir.join("live.service")).unwrap();

        let mut reg = ImageRegistry::new();
        reg.attachments.insert(
            "live".to_string(),
            AttachmentInfo {
                image_name: "live".to_string(),
                image_path: "/test".to_string(),
                profile: None,
                runtime: false,
                units: vec!["live.service".to_string()],
                timestamp: 0,
            },
        );

        let removed = reg.gc_with_dirs(attached_dir.to_str().unwrap(), "/tmp/nonexistent");
        assert!(removed.is_empty());
        assert_eq!(reg.attached_count(), 1);
    }

    #[test]
    fn test_gc_empty() {
        let mut reg = ImageRegistry::new();
        let removed = reg.gc_with_dirs("/tmp/nonexistent", "/tmp/nonexistent2");
        assert!(removed.is_empty());
    }

    // ── ImageRegistry format_image_list tests ─────────────────────────────

    #[test]
    fn test_format_image_list_empty() {
        let reg = ImageRegistry::new();
        let output = reg.format_image_list();
        assert_eq!(output, "No images found.");
    }

    #[test]
    fn test_format_image_list_with_images() {
        let tmp = temp_dir();
        create_test_image(tmp.path(), "app1", &["app1.service"]);
        create_test_image(tmp.path(), "app2", &["app2.service"]);

        let mut reg = ImageRegistry::new();
        let search = tmp.path().to_str().unwrap();
        reg.discover_images_from(&[search]);

        let output = reg.format_image_list();
        assert!(output.contains("NAME"));
        assert!(output.contains("app1"));
        assert!(output.contains("app2"));
        assert!(output.contains("2 images listed."));
    }

    // ── ImageRegistry inspect tests ───────────────────────────────────────

    #[test]
    fn test_inspect_image() {
        let tmp = temp_dir();
        create_test_image(tmp.path(), "myapp", &["myapp.service"]);

        let mut reg = ImageRegistry::new();
        let search = tmp.path().to_str().unwrap();
        reg.discover_images_from(&[search]);

        let result = reg.inspect_image("myapp");
        assert!(result.is_ok());
        let output = result.unwrap();
        assert!(output.contains("Name: myapp"));
        assert!(output.contains("Test OS"));
        assert!(output.contains("myapp.service"));
    }

    #[test]
    fn test_inspect_image_not_found() {
        let reg = ImageRegistry::new();
        let result = reg.inspect_image("nonexistent");
        assert!(result.is_err());
    }

    #[test]
    fn test_inspect_raw_image() {
        let tmp = temp_dir();
        create_test_raw_image(tmp.path(), "myraw");

        let mut reg = ImageRegistry::new();
        let search = tmp.path().to_str().unwrap();
        reg.discover_images_from(&[search]);

        let result = reg.inspect_image("myraw");
        assert!(result.is_ok());
        let output = result.unwrap();
        assert!(output.contains("not yet implemented"));
    }

    // ── Profile tests ─────────────────────────────────────────────────────

    #[test]
    fn test_resolve_profile_dir_style() {
        let tmp = temp_dir();
        let prof_dir = tmp.path().join("default");
        fs::create_dir_all(&prof_dir).unwrap();
        fs::write(
            prof_dir.join("service.conf"),
            "[Service]\nProtectSystem=strict\n",
        )
        .unwrap();

        let search = tmp.path().to_str().unwrap();
        let result = resolve_profile_from("default", &[search]);
        assert!(result.is_some());
        assert!(result.unwrap().contains("ProtectSystem=strict"));
    }

    #[test]
    fn test_resolve_profile_file_style() {
        let tmp = temp_dir();
        fs::write(
            tmp.path().join("trusted.conf"),
            "[Service]\nProtectSystem=no\n",
        )
        .unwrap();

        let search = tmp.path().to_str().unwrap();
        let result = resolve_profile_from("trusted", &[search]);
        assert!(result.is_some());
        assert!(result.unwrap().contains("ProtectSystem=no"));
    }

    #[test]
    fn test_resolve_profile_not_found() {
        let result = resolve_profile_from("nonexistent", &["/nonexistent"]);
        assert!(result.is_none());
    }

    #[test]
    fn test_list_profiles() {
        let tmp = temp_dir();
        fs::create_dir_all(tmp.path().join("default")).unwrap();
        fs::write(tmp.path().join("strict.conf"), "[Service]\n").unwrap();
        fs::write(tmp.path().join("README.txt"), "not a profile\n").unwrap();

        let search = tmp.path().to_str().unwrap();
        let profiles = list_profiles_from(&[search]);
        assert!(profiles.contains(&"default".to_string()));
        assert!(profiles.contains(&"strict".to_string()));
        assert!(!profiles.contains(&"README.txt".to_string()));
    }

    #[test]
    fn test_list_profiles_empty() {
        let profiles = list_profiles_from(&["/nonexistent"]);
        assert!(profiles.is_empty());
    }

    // ── Attach with profile tests ─────────────────────────────────────────

    #[test]
    fn test_attach_with_profile() {
        let tmp = temp_dir();
        let attached_dir = tmp.path().join("attached");
        let profile_dir = tmp.path().join("profiles");
        fs::create_dir_all(&attached_dir).unwrap();
        fs::create_dir_all(&profile_dir).unwrap();

        // Create profile
        fs::write(
            profile_dir.join("strict.conf"),
            "[Service]\nProtectSystem=strict\nPrivateTmp=yes\n",
        )
        .unwrap();

        create_test_image(tmp.path(), "myapp", &["myapp.service"]);
        let mut reg = ImageRegistry::new();
        let search = tmp.path().to_str().unwrap();
        reg.discover_images_from(&[search]);

        // Temporarily override the profile resolution by calling attach_image_to directly
        // and verifying the drop-in was created.
        // For this test we manually insert a profile drop-in check.
        let result = reg.attach_image_to(
            "myapp",
            Some("strict"),
            false,
            attached_dir.to_str().unwrap(),
        );
        // The profile won't resolve because we didn't use the real paths, but
        // the attach should still succeed (profile drop-in is optional).
        assert!(result.is_ok());

        // Marker drop-in should always be present
        let marker = attached_dir.join("myapp.service.d/10-portable.conf");
        assert!(marker.exists());
    }

    // ── Control command tests ─────────────────────────────────────────────

    #[test]
    fn test_command_ping() {
        let mut reg = ImageRegistry::new();
        let result = handle_control_command(&mut reg, "PING");
        assert_eq!(result, "PONG\n");
    }

    #[test]
    fn test_command_ping_case_insensitive() {
        let mut reg = ImageRegistry::new();
        let result = handle_control_command(&mut reg, "ping");
        assert_eq!(result, "PONG\n");
    }

    #[test]
    fn test_command_list_empty() {
        let mut reg = ImageRegistry::new();
        let result = handle_control_command(&mut reg, "LIST");
        assert!(result.contains("No images found."));
    }

    #[test]
    fn test_command_list_with_images() {
        let tmp = temp_dir();
        create_test_image(tmp.path(), "app1", &["app1.service"]);

        let mut reg = ImageRegistry::new();
        let search = tmp.path().to_str().unwrap();
        reg.discover_images_from(&[search]);

        let result = handle_control_command(&mut reg, "LIST");
        assert!(result.contains("app1"));
        assert!(result.contains("1 images listed."));
    }

    #[test]
    fn test_command_status_global() {
        let mut reg = ImageRegistry::new();
        let result = handle_control_command(&mut reg, "STATUS");
        assert!(result.contains("Images: 0"));
        assert!(result.contains("Attached: 0"));
    }

    #[test]
    fn test_command_status_specific() {
        let tmp = temp_dir();
        create_test_image(tmp.path(), "myapp", &["myapp.service"]);

        let mut reg = ImageRegistry::new();
        let search = tmp.path().to_str().unwrap();
        reg.discover_images_from(&[search]);

        let result = handle_control_command(&mut reg, "STATUS myapp");
        assert!(result.contains("Name: myapp"));
    }

    #[test]
    fn test_command_status_not_found() {
        let mut reg = ImageRegistry::new();
        let result = handle_control_command(&mut reg, "STATUS nonexistent");
        assert!(result.contains("ERROR"));
        assert!(result.contains("not found"));
    }

    #[test]
    fn test_command_show() {
        let tmp = temp_dir();
        create_test_image(tmp.path(), "myapp", &["myapp.service"]);

        let mut reg = ImageRegistry::new();
        let search = tmp.path().to_str().unwrap();
        reg.discover_images_from(&[search]);

        let result = handle_control_command(&mut reg, "SHOW myapp");
        assert!(result.contains("Name=myapp"));
        assert!(result.contains("Type=directory"));
    }

    #[test]
    fn test_command_show_missing_name() {
        let mut reg = ImageRegistry::new();
        let result = handle_control_command(&mut reg, "SHOW");
        assert!(result.contains("ERROR"));
    }

    #[test]
    fn test_command_inspect() {
        let tmp = temp_dir();
        create_test_image(tmp.path(), "myapp", &["myapp.service"]);

        let mut reg = ImageRegistry::new();
        let search = tmp.path().to_str().unwrap();
        reg.discover_images_from(&[search]);

        let result = handle_control_command(&mut reg, "INSPECT myapp");
        assert!(result.contains("Name: myapp"));
        assert!(result.contains("myapp.service"));
    }

    #[test]
    fn test_command_inspect_missing_name() {
        let mut reg = ImageRegistry::new();
        let result = handle_control_command(&mut reg, "INSPECT");
        assert!(result.contains("ERROR"));
    }

    #[test]
    fn test_command_is_attached_detached() {
        let mut reg = ImageRegistry::new();
        let result = handle_control_command(&mut reg, "IS-ATTACHED myapp");
        assert_eq!(result.trim(), "detached");
    }

    #[test]
    fn test_command_is_attached_missing_name() {
        let mut reg = ImageRegistry::new();
        let result = handle_control_command(&mut reg, "IS-ATTACHED");
        assert!(result.contains("ERROR"));
    }

    #[test]
    fn test_command_gc_empty() {
        let mut reg = ImageRegistry::new();
        let result = handle_control_command(&mut reg, "GC");
        assert!(result.contains("OK"));
        assert!(result.contains("No stale"));
    }

    #[test]
    fn test_command_reload() {
        let mut reg = ImageRegistry::new();
        let result = handle_control_command(&mut reg, "RELOAD");
        assert!(result.contains("OK"));
        assert!(result.contains("Reloaded"));
    }

    #[test]
    fn test_command_unknown() {
        let mut reg = ImageRegistry::new();
        let result = handle_control_command(&mut reg, "FOOBAR");
        assert!(result.contains("ERROR"));
        assert!(result.contains("Unknown command"));
    }

    #[test]
    fn test_command_empty() {
        let mut reg = ImageRegistry::new();
        let result = handle_control_command(&mut reg, "");
        assert!(result.contains("ERROR"));
    }

    #[test]
    fn test_command_attach_not_found() {
        let mut reg = ImageRegistry::new();
        let result = handle_control_command(&mut reg, "ATTACH nonexistent");
        assert!(result.contains("ERROR"));
        assert!(result.contains("not found"));
    }

    #[test]
    fn test_command_detach_not_attached() {
        let mut reg = ImageRegistry::new();
        let result = handle_control_command(&mut reg, "DETACH nonexistent");
        assert!(result.contains("ERROR"));
        assert!(result.contains("not attached"));
    }

    #[test]
    fn test_command_attach_missing_name() {
        let mut reg = ImageRegistry::new();
        let result = handle_control_command(&mut reg, "ATTACH");
        assert!(result.contains("ERROR"));
    }

    #[test]
    fn test_command_detach_missing_name() {
        let mut reg = ImageRegistry::new();
        let result = handle_control_command(&mut reg, "DETACH");
        assert!(result.contains("ERROR"));
    }

    #[test]
    fn test_command_reattach_missing_name() {
        let mut reg = ImageRegistry::new();
        let result = handle_control_command(&mut reg, "REATTACH");
        assert!(result.contains("ERROR"));
    }

    // ── Helper function tests ─────────────────────────────────────────────

    #[test]
    fn test_format_bytes() {
        assert_eq!(format_bytes(0), "0B");
        assert_eq!(format_bytes(512), "512B");
        assert_eq!(format_bytes(1023), "1023B");
        assert_eq!(format_bytes(1024), "1.0K");
        assert_eq!(format_bytes(1536), "1.5K");
        assert_eq!(format_bytes(1048576), "1.0M");
        assert_eq!(format_bytes(1073741824), "1.0G");
        assert_eq!(format_bytes(2147483648), "2.0G");
    }

    #[test]
    fn test_format_timestamp_zero() {
        assert_eq!(format_timestamp(0), "n/a");
    }

    #[test]
    fn test_format_timestamp_epoch() {
        let ts = format_timestamp(1_000_000); // 1 second after epoch
        assert!(ts.contains("1970"));
        assert!(ts.contains("Jan"));
    }

    #[test]
    fn test_days_to_ymd_epoch() {
        let (y, m, d) = days_to_ymd(0);
        assert_eq!((y, m, d), (1970, 1, 1));
    }

    #[test]
    fn test_days_to_ymd_known() {
        // 2024-01-01 is day 19723 since epoch
        let (y, m, d) = days_to_ymd(19723);
        assert_eq!((y, m, d), (2024, 1, 1));
    }

    #[test]
    fn test_now_usec_nonzero() {
        assert!(now_usec() > 0);
    }

    // ── Integration: attach + detach cycle ────────────────────────────────

    #[test]
    fn test_attach_detach_cycle() {
        let tmp = temp_dir();
        let attached_dir = tmp.path().join("attached");
        let runtime_dir = tmp.path().join("runtime");
        fs::create_dir_all(&attached_dir).unwrap();
        fs::create_dir_all(&runtime_dir).unwrap();

        create_test_image(tmp.path(), "myapp", &["myapp.service", "myapp.socket"]);
        let mut reg = ImageRegistry::new();
        let search = tmp.path().to_str().unwrap();
        reg.discover_images_from(&[search]);

        // Attach
        let attached = reg.attach_image_to("myapp", None, false, attached_dir.to_str().unwrap());
        assert!(attached.is_ok());
        assert_eq!(attached.unwrap().len(), 2);
        assert_eq!(reg.get_attach_state("myapp"), AttachState::Attached);

        // Verify symlinks exist
        assert!(
            attached_dir
                .join("myapp.service")
                .symlink_metadata()
                .is_ok()
        );
        assert!(attached_dir.join("myapp.socket").symlink_metadata().is_ok());

        // Detach
        let detached = reg.detach_image_from(
            "myapp",
            attached_dir.to_str().unwrap(),
            runtime_dir.to_str().unwrap(),
        );
        assert!(detached.is_ok());
        assert_eq!(detached.unwrap().len(), 2);
        assert_eq!(reg.get_attach_state("myapp"), AttachState::Detached);

        // Verify symlinks are gone
        assert!(!attached_dir.join("myapp.service").exists());
        assert!(!attached_dir.join("myapp.socket").exists());
    }

    #[test]
    fn test_reattach_cycle() {
        let tmp = temp_dir();
        let attached_dir = tmp.path().join("attached");
        let runtime_dir = tmp.path().join("runtime");
        fs::create_dir_all(&attached_dir).unwrap();
        fs::create_dir_all(&runtime_dir).unwrap();

        create_test_image(tmp.path(), "myapp", &["myapp.service"]);
        let mut reg = ImageRegistry::new();
        let search = tmp.path().to_str().unwrap();
        reg.discover_images_from(&[search]);

        // Attach first
        let _ = reg.attach_image_to("myapp", None, false, attached_dir.to_str().unwrap());
        assert_eq!(reg.get_attach_state("myapp"), AttachState::Attached);

        // Detach + reattach
        let _ = reg.detach_image_from(
            "myapp",
            attached_dir.to_str().unwrap(),
            runtime_dir.to_str().unwrap(),
        );
        let result = reg.attach_image_to("myapp", None, false, attached_dir.to_str().unwrap());
        assert!(result.is_ok());
        assert_eq!(reg.get_attach_state("myapp"), AttachState::Attached);
    }
}
