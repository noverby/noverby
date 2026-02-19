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

use log::trace;
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

/// Augment the process's `PATH` environment variable with binary directories
/// derived from the unit search paths and the running executable.
///
/// Many upstream systemd unit files use bare command names in `ExecStart=`
/// (e.g. `systemd-tmpfiles`, `udevadm`).  Real systemd resolves these via
/// compiled-in prefix paths.  systemd-rs instead adds the relevant package
/// directories to `PATH` so that both `which` look-ups and `Command::new`
/// find the binaries.
///
/// Directories added (if they exist):
/// - For each unit dir of the form `<root>/lib/systemd/system`, add
///   `<root>/bin` and `<root>/lib/systemd` (libexec-style helpers).
/// - For each unit dir of the form `<root>/etc/systemd/system`, add
///   `<root>/bin` and `<root>/lib/systemd`.
/// - The NixOS well-known path `/run/current-system/sw/bin`.
pub fn augment_path_from_unit_dirs(unit_dirs: &[PathBuf]) {
    let current_path = std::env::var("PATH").unwrap_or_default();
    let mut extra_dirs: Vec<PathBuf> = Vec::new();

    for unit_dir in unit_dirs {
        // Walk up from the unit directory to find the package root.
        // Typical layouts:
        //   <root>/lib/systemd/system   → root is 3 levels up
        //   <root>/etc/systemd/system   → root is 3 levels up
        //   <root>/example/systemd/system → root is 3 levels up
        if let Some(root) = unit_dir
            .parent()
            .and_then(|p| p.parent())
            .and_then(|p| p.parent())
        {
            let bin_dir = root.join("bin");
            if bin_dir.is_dir() && !extra_dirs.contains(&bin_dir) {
                extra_dirs.push(bin_dir);
            }
            // lib/systemd/ contains libexec-style helpers like systemd-udevd,
            // systemd-journald, systemd-logind, etc.
            let libexec_dir = root.join("lib/systemd");
            if libexec_dir.is_dir() && !extra_dirs.contains(&libexec_dir) {
                extra_dirs.push(libexec_dir);
            }
        }
    }

    // NixOS well-known system binary path (created by activation script
    // before the service manager starts).
    let nixos_sw_bin = PathBuf::from("/run/current-system/sw/bin");
    if nixos_sw_bin.is_dir() && !extra_dirs.contains(&nixos_sw_bin) {
        extra_dirs.push(nixos_sw_bin);
    }

    // NixOS suid wrappers path.
    let nixos_wrappers = PathBuf::from("/run/wrappers/bin");
    if nixos_wrappers.is_dir() && !extra_dirs.contains(&nixos_wrappers) {
        extra_dirs.push(nixos_wrappers);
    }

    if extra_dirs.is_empty() {
        return;
    }

    // Filter out directories already present in PATH.
    let existing: Vec<&str> = current_path.split(':').collect();
    let new_dirs: Vec<String> = extra_dirs
        .iter()
        .filter(|d| {
            let s = d.to_string_lossy();
            !existing.iter().any(|e| *e == s.as_ref())
        })
        .map(|d| d.to_string_lossy().into_owned())
        .collect();

    if new_dirs.is_empty() {
        return;
    }

    let new_path = if current_path.is_empty() {
        new_dirs.join(":")
    } else {
        format!("{}:{}", current_path, new_dirs.join(":"))
    };

    trace!(
        "Augmenting PATH with package directories: {:?} → PATH={}",
        new_dirs, new_path
    );
    // SAFETY: This is called early in startup, before any other threads are
    // spawned, so modifying the environment is safe.
    unsafe { std::env::set_var("PATH", &new_path) };
}

pub fn load_config() -> (LoggingConfig, Config) {
    // Collect unit directories: standard system paths + package-local dir.
    // Only include directories that actually exist on this system.
    let mut unit_dirs: Vec<PathBuf> = SYSTEM_UNIT_DIRS
        .iter()
        .map(PathBuf::from)
        .filter(|p| p.is_dir())
        .collect();

    if let Some(pkg_dir) = package_unit_dir()
        && !unit_dirs.contains(&pkg_dir)
    {
        unit_dirs.push(pkg_dir);
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
