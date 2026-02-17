//! systemd-path — List and query well-known system and user runtime paths.
//!
//! A drop-in replacement for `systemd-path(1)`. When invoked without
//! arguments it prints all known path names and their current values, one
//! per line.  When invoked with one or more path names it prints only the
//! requested paths.
//!
//! Supported options:
//!
//! - `--suffix=SUFFIX` — Append the given suffix to each printed path.
//!
//! The tool mirrors the output format of the real `systemd-path`:
//!
//!     <name>: <path>
//!
//! or, when a single path is queried, just the bare path.

use clap::Parser;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process;

#[derive(Parser, Debug)]
#[command(
    name = "systemd-path",
    about = "List and query well-known system and user runtime paths",
    version
)]
struct Cli {
    /// Append SUFFIX to all printed paths.
    #[arg(long, value_name = "SUFFIX")]
    suffix: Option<String>,

    /// Path names to query.  If none are given, all known paths are printed.
    names: Vec<String>,
}

/// Build a map of all well-known paths.
///
/// The keys match the identifiers accepted by the real `systemd-path`
/// command.  The values are resolved at runtime so that XDG overrides and
/// the actual system layout are respected.
fn known_paths() -> BTreeMap<&'static str, PathBuf> {
    let mut m = BTreeMap::new();

    // --- System paths ---
    m.insert(
        "temporary",
        PathBuf::from(std::env::var("TMPDIR").unwrap_or_else(|_| "/tmp".into())),
    );
    m.insert("temporary-large", PathBuf::from("/var/tmp"));
    m.insert("system-binaries", PathBuf::from("/usr/bin"));
    m.insert("system-library-private", PathBuf::from("/usr/lib"));
    m.insert("system-library-arch", PathBuf::from("/usr/lib"));
    m.insert("system-shared", PathBuf::from("/usr/share"));
    m.insert(
        "system-configuration-factory",
        PathBuf::from("/usr/share/factory/etc"),
    );
    m.insert(
        "system-state-factory",
        PathBuf::from("/usr/share/factory/var"),
    );
    m.insert("system-configuration", PathBuf::from("/etc"));
    m.insert("system-runtime", PathBuf::from("/run"));
    m.insert("system-runtime-logs", PathBuf::from("/run/log"));
    m.insert("system-state-private", PathBuf::from("/var/lib"));
    m.insert("system-state-logs", PathBuf::from("/var/log"));
    m.insert("system-state-cache", PathBuf::from("/var/cache"));
    m.insert("system-state-spool", PathBuf::from("/var/spool"));

    // --- systemd search paths ---
    m.insert(
        "system-generator",
        PathBuf::from("/usr/lib/systemd/system-generators"),
    );
    m.insert(
        "system-generator-early",
        PathBuf::from("/run/systemd/system-generators"),
    );
    m.insert(
        "system-generator-late",
        PathBuf::from("/usr/lib/systemd/system-generators"),
    );
    m.insert(
        "user-generator",
        PathBuf::from("/usr/lib/systemd/user-generators"),
    );
    m.insert(
        "user-generator-early",
        PathBuf::from("/run/systemd/user-generators"),
    );
    m.insert(
        "user-generator-late",
        PathBuf::from("/usr/lib/systemd/user-generators"),
    );
    m.insert(
        "systemd-system-unit",
        PathBuf::from("/usr/lib/systemd/system"),
    );
    m.insert("systemd-user-unit", PathBuf::from("/usr/lib/systemd/user"));
    m.insert("systemd-system-conf", PathBuf::from("/etc/systemd/system"));
    m.insert("systemd-user-conf", PathBuf::from("/etc/systemd/user"));
    m.insert(
        "systemd-search-system-unit",
        PathBuf::from("/etc/systemd/system"),
    );
    m.insert(
        "systemd-search-user-unit",
        PathBuf::from("/etc/systemd/user"),
    );
    m.insert(
        "systemd-system-preset",
        PathBuf::from("/usr/lib/systemd/system-preset"),
    );
    m.insert(
        "systemd-user-preset",
        PathBuf::from("/usr/lib/systemd/user-preset"),
    );

    // --- Catalog & documentation ---
    m.insert("catalog", PathBuf::from("/usr/lib/systemd/catalog"));

    // --- User paths (XDG) ---
    let home = std::env::var("HOME").unwrap_or_else(|_| "/root".into());
    let home_path = Path::new(&home);

    let xdg_runtime = std::env::var("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from(format!("/run/user/{}", unsafe { libc::getuid() })));

    let xdg_config_home = std::env::var("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| home_path.join(".config"));

    let xdg_cache_home = std::env::var("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| home_path.join(".cache"));

    let xdg_data_home = std::env::var("XDG_DATA_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| home_path.join(".local/share"));

    let xdg_state_home = std::env::var("XDG_STATE_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| home_path.join(".local/state"));

    m.insert("user", home_path.to_path_buf());
    m.insert("user-runtime", xdg_runtime.clone());
    m.insert("user-configuration", xdg_config_home.clone());
    m.insert("user-cache", xdg_cache_home.clone());
    m.insert("user-shared", xdg_data_home.clone());
    m.insert("user-state", xdg_state_home.clone());
    m.insert("user-library-private", home_path.join(".local/lib"));
    m.insert("user-binaries", home_path.join(".local/bin"));
    m.insert("user-documents", home_path.join("Documents"));
    m.insert("user-music", home_path.join("Music"));
    m.insert("user-pictures", home_path.join("Pictures"));
    m.insert("user-videos", home_path.join("Videos"));
    m.insert("user-download", home_path.join("Downloads"));
    m.insert("user-public", home_path.join("Public"));
    m.insert("user-templates", home_path.join("Templates"));
    m.insert("user-desktop", home_path.join("Desktop"));

    // --- Derived search paths ---
    m.insert(
        "search-binaries",
        PathBuf::from("/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin"),
    );
    m.insert(
        "search-library-private",
        PathBuf::from("/usr/local/lib:/usr/lib"),
    );
    m.insert(
        "search-library-arch",
        PathBuf::from("/usr/local/lib:/usr/lib"),
    );
    m.insert(
        "search-shared",
        PathBuf::from("/usr/local/share:/usr/share"),
    );
    m.insert(
        "search-configuration-factory",
        PathBuf::from("/usr/local/share/factory/etc:/usr/share/factory/etc"),
    );
    m.insert(
        "search-state-factory",
        PathBuf::from("/usr/local/share/factory/var:/usr/share/factory/var"),
    );
    m.insert(
        "search-configuration",
        PathBuf::from("/etc:/run:/usr/local/lib:/usr/lib"),
    );

    m
}

fn apply_suffix(path: &Path, suffix: Option<&str>) -> PathBuf {
    match suffix {
        Some(s) => path.join(s),
        None => path.to_path_buf(),
    }
}

fn main() {
    let cli = Cli::parse();
    let paths = known_paths();
    let suffix = cli.suffix.as_deref();

    if cli.names.is_empty() {
        // Print all known paths
        for (name, path) in &paths {
            let display_path = apply_suffix(path, suffix);
            println!("{name}: {}", display_path.display());
        }
    } else if cli.names.len() == 1 {
        // Single query: print just the path (no name prefix)
        let name = &cli.names[0];
        match paths.get(name.as_str()) {
            Some(path) => {
                let display_path = apply_suffix(path, suffix);
                println!("{}", display_path.display());
            }
            None => {
                eprintln!("Unknown path: {name}");
                process::exit(1);
            }
        }
    } else {
        // Multiple queries: print name: path for each
        let mut exit_code = 0;
        for name in &cli.names {
            match paths.get(name.as_str()) {
                Some(path) => {
                    let display_path = apply_suffix(path, suffix);
                    println!("{name}: {}", display_path.display());
                }
                None => {
                    eprintln!("Unknown path: {name}");
                    exit_code = 1;
                }
            }
        }
        if exit_code != 0 {
            process::exit(exit_code);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_known_paths_not_empty() {
        let paths = known_paths();
        assert!(!paths.is_empty());
    }

    #[test]
    fn test_known_paths_has_system_configuration() {
        let paths = known_paths();
        assert_eq!(
            paths.get("system-configuration").unwrap().as_os_str(),
            "/etc"
        );
    }

    #[test]
    fn test_known_paths_has_temporary() {
        let paths = known_paths();
        assert!(paths.contains_key("temporary"));
    }

    #[test]
    fn test_apply_suffix_none() {
        let p = PathBuf::from("/etc");
        assert_eq!(apply_suffix(&p, None), PathBuf::from("/etc"));
    }

    #[test]
    fn test_apply_suffix_some() {
        let p = PathBuf::from("/etc");
        assert_eq!(
            apply_suffix(&p, Some("systemd")),
            PathBuf::from("/etc/systemd")
        );
    }

    #[test]
    fn test_known_paths_has_user_runtime() {
        let paths = known_paths();
        assert!(paths.contains_key("user-runtime"));
    }

    #[test]
    fn test_known_paths_has_system_runtime() {
        let paths = known_paths();
        assert_eq!(paths.get("system-runtime").unwrap().as_os_str(), "/run");
    }
}
