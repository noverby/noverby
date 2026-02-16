//! Configuration for systemd-rs.
//!
//! As a drop-in replacement for systemd, systemd-rs uses the same well-known
//! default paths that systemd uses.  There is no config file and no
//! systemd-rs-specific environment variables — unit directories, the default
//! target, and all other settings match systemd's compiled-in defaults.
//!
//! In addition to the standard system-wide unit directories, systemd-rs
//! discovers its own package's `lib/systemd/system/` directory by walking
//! up from its executable path.  This mirrors what systemd does with its
//! compile-time `rootlibdir` setting (e.g. on NixOS the systemd package
//! searches its own store path for shipped unit files).

use std::path::PathBuf;

#[derive(Debug)]
pub struct LoggingConfig {
    pub log_to_stdout: bool,
    pub log_to_disk: bool,
    pub log_dir: PathBuf,
}

#[derive(Debug, Clone)]
pub struct Config {
    pub unit_dirs: Vec<PathBuf>,
    pub target_unit: String,
    pub notification_sockets_dir: PathBuf,
    pub self_path: PathBuf,
}

/// Well-known systemd unit search directories (system instance), in priority
/// order.  This matches the paths systemd itself searches.
const SYSTEM_UNIT_DIRS: &[&str] = &[
    "/etc/systemd/system",
    "/run/systemd/system",
    "/usr/local/lib/systemd/system",
    "/usr/lib/systemd/system",
    "/lib/systemd/system",
];

/// Try to find a `lib/systemd/system/` directory that belongs to the same
/// package / prefix as the running executable.  This mirrors systemd's
/// compile-time `rootlibdir` — on NixOS the systemd package includes its
/// own store path as a unit search directory so that upstream-shipped targets
/// (e.g. `time-set.target`, `time-sync.target`) are found.
///
/// We walk up from the executable's directory (at most 5 levels) and check
/// whether `<ancestor>/lib/systemd/system` exists.  This handles both
/// `$out/bin/systemd-rs` and `$out/lib/systemd/systemd` layouts.
fn package_unit_dir() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let mut dir = exe.parent()?;
    for _ in 0..5 {
        let candidate = dir.join("lib/systemd/system");
        if candidate.is_dir() {
            return Some(candidate);
        }
        dir = dir.parent()?;
    }
    None
}

pub fn load_config() -> (LoggingConfig, Config) {
    // Collect unit directories: standard system paths + package-local dir.
    // Only include directories that actually exist on this system.
    let mut unit_dirs: Vec<PathBuf> = SYSTEM_UNIT_DIRS
        .iter()
        .map(PathBuf::from)
        .filter(|p| p.is_dir())
        .collect();

    if let Some(pkg_dir) = package_unit_dir() {
        if !unit_dirs.contains(&pkg_dir) {
            unit_dirs.push(pkg_dir);
        }
    }

    let self_path = std::env::current_exe().expect("Could not determine own executable path");

    let config = Config {
        unit_dirs,
        target_unit: "default.target".to_owned(),
        notification_sockets_dir: PathBuf::from("/run/systemd/systemd-rs-notify"),
        self_path,
    };

    let logging_config = LoggingConfig {
        log_to_stdout: true,
        log_to_disk: false,
        log_dir: PathBuf::from("/var/log/systemd-rs"),
    };

    (logging_config, config)
}
