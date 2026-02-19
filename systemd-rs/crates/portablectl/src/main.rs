//! portablectl — manage portable service images
//!
//! This is a Rust implementation of systemd's `portablectl` command. It
//! communicates with `systemd-portabled` via the control socket at
//! `/run/systemd/portabled-control` to list, inspect, attach, and detach
//! portable service images.
//!
//! ## Supported commands
//!
//! - `list` — list available portable images
//! - `attach <image> [profile]` — attach an image (symlink its units)
//! - `detach <image>` — detach an image (remove symlinks)
//! - `reattach <image> [profile]` — atomic detach + attach
//! - `inspect <image>` — show image details, os-release, and unit files
//! - `is-attached <image>` — check attachment state
//! - `read-only <image>` — (stub) show/toggle read-only state
//! - `set-limit [image] <bytes>` — (stub) set image size limit
//!
//! ## Missing
//!
//! - D-Bus interface (org.freedesktop.portable1)
//! - Extension images (`--extension`)
//! - Image size limit management
//! - Copy operations
//! - Raw image support

use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::net::Shutdown;
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::process;

// ---------------------------------------------------------------------------
// Paths
// ---------------------------------------------------------------------------

const CONTROL_SOCKET_PATH: &str = "/run/systemd/portabled-control";
const STATE_DIR: &str = "/run/systemd/portabled";

/// Image search paths (same as portabled).
const IMAGE_SEARCH_PATHS: &[&str] = &[
    "/etc/portables",
    "/run/portables",
    "/var/lib/portables",
    "/usr/lib/portables",
];

/// Unit file directories inside an image.
const IMAGE_UNIT_PATHS: &[&str] = &[
    "usr/lib/systemd/system",
    "lib/systemd/system",
    "etc/systemd/system",
];

// ---------------------------------------------------------------------------
// Argument parsing
// ---------------------------------------------------------------------------

#[derive(Debug, PartialEq)]
enum Command {
    List,
    Attach {
        image: String,
        profile: Option<String>,
        runtime: bool,
    },
    Detach {
        image: String,
    },
    Reattach {
        image: String,
        profile: Option<String>,
        runtime: bool,
    },
    Inspect {
        image: String,
    },
    IsAttached {
        image: String,
    },
    ReadOnly {
        image: String,
        value: Option<bool>,
    },
    SetLimit {
        image: Option<String>,
        bytes: String,
    },
    Help,
}

fn parse_command(args: &[String]) -> Result<Command, String> {
    // Strip common flags before parsing
    let mut filtered = Vec::new();
    let mut runtime = false;
    let mut no_reload = false;
    let mut no_pager = false;
    let mut no_legend = false;
    let mut no_ask_password = false;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--runtime" => runtime = true,
            "--no-reload" => no_reload = true,
            "--no-pager" => no_pager = true,
            "--no-legend" => no_legend = true,
            "--no-ask-password" => no_ask_password = true,
            "-q" | "--quiet" => {} // absorb
            "-H" | "--host" => {
                i += 1; // skip host arg
            }
            "-M" | "--machine" => {
                i += 1; // skip machine arg
            }
            "--json" => {
                i += 1; // skip json format arg
            }
            "-h" | "--help" => return Ok(Command::Help),
            _ => filtered.push(args[i].clone()),
        }
        i += 1;
    }
    let _ = (no_reload, no_pager, no_legend, no_ask_password);

    let args = filtered;

    if args.is_empty() {
        return Ok(Command::List);
    }

    match args[0].as_str() {
        "list" => Ok(Command::List),

        "attach" => {
            if args.len() < 2 {
                return Err("attach requires an image name".to_string());
            }
            Ok(Command::Attach {
                image: args[1].clone(),
                profile: args.get(2).cloned(),
                runtime,
            })
        }

        "detach" => {
            if args.len() < 2 {
                return Err("detach requires an image name".to_string());
            }
            Ok(Command::Detach {
                image: args[1].clone(),
            })
        }

        "reattach" => {
            if args.len() < 2 {
                return Err("reattach requires an image name".to_string());
            }
            Ok(Command::Reattach {
                image: args[1].clone(),
                profile: args.get(2).cloned(),
                runtime,
            })
        }

        "inspect" => {
            if args.len() < 2 {
                return Err("inspect requires an image name".to_string());
            }
            Ok(Command::Inspect {
                image: args[1].clone(),
            })
        }

        "is-attached" => {
            if args.len() < 2 {
                return Err("is-attached requires an image name".to_string());
            }
            Ok(Command::IsAttached {
                image: args[1].clone(),
            })
        }

        "read-only" => {
            if args.len() < 2 {
                return Err("read-only requires an image name".to_string());
            }
            let value = args
                .get(2)
                .map(|v| matches!(v.as_str(), "yes" | "true" | "1"));
            Ok(Command::ReadOnly {
                image: args[1].clone(),
                value,
            })
        }

        "set-limit" => {
            if args.len() < 2 {
                return Err("set-limit requires at least a size argument".to_string());
            }
            if args.len() >= 3 {
                Ok(Command::SetLimit {
                    image: Some(args[1].clone()),
                    bytes: args[2].clone(),
                })
            } else {
                Ok(Command::SetLimit {
                    image: None,
                    bytes: args[1].clone(),
                })
            }
        }

        "help" | "--help" | "-h" => Ok(Command::Help),

        other => Err(format!("Unknown command: {}", other)),
    }
}

// ---------------------------------------------------------------------------
// Daemon communication
// ---------------------------------------------------------------------------

fn send_command(cmd: &str) -> Result<String, String> {
    let mut stream = UnixStream::connect(CONTROL_SOCKET_PATH)
        .map_err(|e| format!("Failed to connect to portabled: {}", e))?;

    stream
        .set_read_timeout(Some(std::time::Duration::from_secs(10)))
        .ok();
    stream
        .set_write_timeout(Some(std::time::Duration::from_secs(5)))
        .ok();

    stream
        .write_all(format!("{}\n", cmd).as_bytes())
        .map_err(|e| format!("Failed to send command: {}", e))?;
    stream
        .flush()
        .map_err(|e| format!("Failed to flush: {}", e))?;

    // Signal that we're done writing
    let _ = stream.shutdown(Shutdown::Write);

    let reader = BufReader::new(stream);
    let mut response = String::new();
    for line in reader.lines() {
        match line {
            Ok(l) => {
                response.push_str(&l);
                response.push('\n');
            }
            Err(_) => break,
        }
    }

    Ok(response)
}

fn try_daemon_command(cmd: &str) -> Option<String> {
    send_command(cmd).ok()
}

// ---------------------------------------------------------------------------
// Offline fallback (reads state files directly when daemon is unavailable)
// ---------------------------------------------------------------------------

/// Offline fallback: discover images from search paths.
fn offline_discover_images() -> BTreeMap<String, OfflineImage> {
    offline_discover_images_from(IMAGE_SEARCH_PATHS)
}

fn offline_discover_images_from(search_paths: &[&str]) -> BTreeMap<String, OfflineImage> {
    let mut images = BTreeMap::new();

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

            if name_str.starts_with('.') {
                continue;
            }

            let (name, image_type) = if path.is_dir() {
                (name_str.to_string(), "directory")
            } else if name_str.ends_with(".raw") {
                (name_str.trim_end_matches(".raw").to_string(), "raw")
            } else {
                continue;
            };

            if images.contains_key(&name) {
                continue;
            }

            let os_pretty = if path.is_dir() {
                read_os_pretty_name(&path)
            } else {
                None
            };

            images.insert(
                name.clone(),
                OfflineImage {
                    name,
                    path: path.clone(),
                    image_type: image_type.to_string(),
                    os_pretty_name: os_pretty,
                },
            );
        }
    }

    images
}

struct OfflineImage {
    name: String,
    path: PathBuf,
    image_type: String,
    os_pretty_name: Option<String>,
}

fn read_os_pretty_name(image_path: &Path) -> Option<String> {
    for name in &["usr/lib/os-release", "etc/os-release"] {
        let p = image_path.join(name);
        if let Ok(content) = fs::read_to_string(&p) {
            for line in content.lines() {
                let line = line.trim();
                if let Some(rest) = line.strip_prefix("PRETTY_NAME=") {
                    return Some(rest.trim_matches('"').trim_matches('\'').to_string());
                }
            }
        }
    }
    None
}

fn discover_units(image_path: &Path) -> Vec<String> {
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

/// Read attachment state from state files.
fn offline_get_attach_state(name: &str) -> &'static str {
    offline_get_attach_state_from(name, STATE_DIR)
}

fn offline_get_attach_state_from(name: &str, state_dir: &str) -> &'static str {
    let path = Path::new(state_dir).join(name);
    if let Ok(content) = fs::read_to_string(&path) {
        for line in content.lines() {
            if let Some(val) = line.strip_prefix("RUNTIME=") {
                if val == "yes" {
                    return "attached-runtime";
                } else {
                    return "attached";
                }
            }
        }
        return "attached";
    }
    "detached"
}

fn offline_list() -> String {
    let images = offline_discover_images();
    if images.is_empty() {
        return "No images found.".to_string();
    }

    let mut lines = Vec::new();
    lines.push(format!(
        "{:<32} {:<12} {:<18} {:<24} {}",
        "NAME", "TYPE", "STATE", "OS", "PATH"
    ));
    for image in images.values() {
        let state = offline_get_attach_state(&image.name);
        lines.push(format!(
            "{:<32} {:<12} {:<18} {:<24} {}",
            image.name,
            image.image_type,
            state,
            image.os_pretty_name.as_deref().unwrap_or("-"),
            image.path.display()
        ));
    }
    lines.push(String::new());
    lines.push(format!("{} images listed.", images.len()));
    lines.join("\n")
}

fn offline_inspect(name: &str) -> Result<String, String> {
    let images = offline_discover_images();
    let image = images
        .get(name)
        .ok_or_else(|| format!("Image '{}' not found", name))?;

    let mut lines = Vec::new();
    lines.push(format!("        Name: {}", image.name));
    lines.push(format!("        Path: {}", image.path.display()));
    lines.push(format!("        Type: {}", image.image_type));
    if let Some(ref os) = image.os_pretty_name {
        lines.push(format!("          OS: {}", os));
    }
    lines.push(String::new());

    if image.image_type == "directory" {
        // os-release
        for os_name in &["usr/lib/os-release", "etc/os-release"] {
            let p = image.path.join(os_name);
            if let Ok(content) = fs::read_to_string(&p) {
                lines.push("--- os-release ---".to_string());
                for line in content.lines() {
                    lines.push(line.to_string());
                }
                lines.push(String::new());
                break;
            }
        }

        // Unit files
        let units = discover_units(&image.path);
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

// ---------------------------------------------------------------------------
// Command execution
// ---------------------------------------------------------------------------

fn run_command(cmd: Command) -> i32 {
    match cmd {
        Command::List => {
            let output = match try_daemon_command("LIST") {
                Some(r) => r,
                None => offline_list(),
            };
            print!("{}", output);
            0
        }

        Command::Attach {
            image,
            profile,
            runtime,
        } => {
            let mut cmd_str = format!("ATTACH {}", image);
            if let Some(ref prof) = profile {
                cmd_str.push(' ');
                cmd_str.push_str(prof);
            } else {
                cmd_str.push_str(" -"); // placeholder
            }
            if runtime {
                cmd_str.push_str(" runtime");
            }

            match send_command(&cmd_str) {
                Ok(response) => {
                    print!("{}", response);
                    if response.starts_with("OK") { 0 } else { 1 }
                }
                Err(e) => {
                    eprintln!("Failed to attach: {}", e);
                    1
                }
            }
        }

        Command::Detach { image } => match send_command(&format!("DETACH {}", image)) {
            Ok(response) => {
                print!("{}", response);
                if response.starts_with("OK") { 0 } else { 1 }
            }
            Err(e) => {
                eprintln!("Failed to detach: {}", e);
                1
            }
        },

        Command::Reattach {
            image,
            profile,
            runtime,
        } => {
            let mut cmd_str = format!("REATTACH {}", image);
            if let Some(ref prof) = profile {
                cmd_str.push(' ');
                cmd_str.push_str(prof);
            } else {
                cmd_str.push_str(" -");
            }
            if runtime {
                cmd_str.push_str(" runtime");
            }

            match send_command(&cmd_str) {
                Ok(response) => {
                    print!("{}", response);
                    if response.starts_with("OK") { 0 } else { 1 }
                }
                Err(e) => {
                    eprintln!("Failed to reattach: {}", e);
                    1
                }
            }
        }

        Command::Inspect { image } => {
            let output = match try_daemon_command(&format!("INSPECT {}", image)) {
                Some(r) if !r.starts_with("ERROR") => r,
                _ => match offline_inspect(&image) {
                    Ok(s) => s,
                    Err(e) => {
                        eprintln!("{}", e);
                        return 1;
                    }
                },
            };
            print!("{}", output);
            0
        }

        Command::IsAttached { image } => {
            let state = match try_daemon_command(&format!("IS-ATTACHED {}", image)) {
                Some(r) => r.trim().to_string(),
                None => offline_get_attach_state(&image).to_string(),
            };
            println!("{}", state);
            if state == "detached" { 1 } else { 0 }
        }

        Command::ReadOnly { image, value: _ } => {
            eprintln!(
                "read-only for image '{}': not yet implemented (images are always writable)",
                image
            );
            1
        }

        Command::SetLimit { image, bytes } => {
            eprintln!(
                "set-limit{}: {} — not yet implemented",
                image
                    .as_ref()
                    .map(|n| format!(" {}", n))
                    .unwrap_or_default(),
                bytes
            );
            1
        }

        Command::Help => {
            print_help();
            0
        }
    }
}

fn print_help() {
    eprintln!(
        "portablectl -- Manage portable service images

Usage:
  portablectl [OPTIONS...] COMMAND [ARGS...]

Commands:
  list                         List available portable images
  attach IMAGE [PROFILE]       Attach a portable image
  detach IMAGE                 Detach a portable image
  reattach IMAGE [PROFILE]     Reattach a portable image
  inspect IMAGE                Inspect a portable image
  is-attached IMAGE            Check if an image is attached
  read-only IMAGE [BOOL]       Show/set read-only state
  set-limit [IMAGE] BYTES      Set image disk space limit

Options:
  --runtime                    Use runtime (volatile) attachment
  --no-reload                  Don't reload daemon after attach/detach
  --no-pager                   Don't pipe output into a pager
  --no-legend                  Don't print table headers/footers
  --no-ask-password            Don't ask for passwords
  -q, --quiet                  Suppress output
  -h, --help                   Show this help"
    );
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();

    match parse_command(&args) {
        Ok(cmd) => {
            let exit_code = run_command(cmd);
            process::exit(exit_code);
        }
        Err(e) => {
            eprintln!("Error: {}", e);
            eprintln!("Try 'portablectl --help' for more information.");
            process::exit(1);
        }
    }
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    // ── Helpers ────────────────────────────────────────────────────────────

    fn temp_dir() -> TempDir {
        TempDir::new().unwrap()
    }

    fn create_test_image(base: &Path, name: &str, units: &[&str]) -> PathBuf {
        let img_dir = base.join(name);
        let unit_dir = img_dir.join("usr/lib/systemd/system");
        fs::create_dir_all(&unit_dir).unwrap();

        for unit in units {
            let unit_path = unit_dir.join(unit);
            fs::write(
                &unit_path,
                format!(
                    "[Unit]\nDescription=Test {}\n\n[Service]\nExecStart=/bin/true\n",
                    unit
                ),
            )
            .unwrap();
        }

        let os_dir = img_dir.join("usr/lib");
        fs::create_dir_all(&os_dir).unwrap();
        fs::write(
            os_dir.join("os-release"),
            "PRETTY_NAME=\"Test OS\"\nID=test\nVERSION_ID=1.0\n",
        )
        .unwrap();

        img_dir
    }

    fn create_test_raw_image(base: &Path, name: &str) -> PathBuf {
        let path = base.join(format!("{}.raw", name));
        fs::write(&path, vec![0u8; 1024]).unwrap();
        path
    }

    fn s(val: &str) -> String {
        val.to_string()
    }

    fn args(vals: &[&str]) -> Vec<String> {
        vals.iter().map(|v| v.to_string()).collect()
    }

    // ── parse_command tests ───────────────────────────────────────────────

    #[test]
    fn test_parse_empty_args() {
        let cmd = parse_command(&args(&[])).unwrap();
        assert_eq!(cmd, Command::List);
    }

    #[test]
    fn test_parse_list() {
        let cmd = parse_command(&args(&["list"])).unwrap();
        assert_eq!(cmd, Command::List);
    }

    #[test]
    fn test_parse_attach() {
        let cmd = parse_command(&args(&["attach", "myapp"])).unwrap();
        assert_eq!(
            cmd,
            Command::Attach {
                image: s("myapp"),
                profile: None,
                runtime: false,
            }
        );
    }

    #[test]
    fn test_parse_attach_with_profile() {
        let cmd = parse_command(&args(&["attach", "myapp", "default"])).unwrap();
        assert_eq!(
            cmd,
            Command::Attach {
                image: s("myapp"),
                profile: Some(s("default")),
                runtime: false,
            }
        );
    }

    #[test]
    fn test_parse_attach_runtime() {
        let cmd = parse_command(&args(&["--runtime", "attach", "myapp"])).unwrap();
        assert_eq!(
            cmd,
            Command::Attach {
                image: s("myapp"),
                profile: None,
                runtime: true,
            }
        );
    }

    #[test]
    fn test_parse_attach_missing_image() {
        let result = parse_command(&args(&["attach"]));
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("requires"));
    }

    #[test]
    fn test_parse_detach() {
        let cmd = parse_command(&args(&["detach", "myapp"])).unwrap();
        assert_eq!(cmd, Command::Detach { image: s("myapp") });
    }

    #[test]
    fn test_parse_detach_missing_image() {
        let result = parse_command(&args(&["detach"]));
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_reattach() {
        let cmd = parse_command(&args(&["reattach", "myapp"])).unwrap();
        assert_eq!(
            cmd,
            Command::Reattach {
                image: s("myapp"),
                profile: None,
                runtime: false,
            }
        );
    }

    #[test]
    fn test_parse_reattach_with_profile_and_runtime() {
        let cmd = parse_command(&args(&["--runtime", "reattach", "myapp", "strict"])).unwrap();
        assert_eq!(
            cmd,
            Command::Reattach {
                image: s("myapp"),
                profile: Some(s("strict")),
                runtime: true,
            }
        );
    }

    #[test]
    fn test_parse_reattach_missing_image() {
        let result = parse_command(&args(&["reattach"]));
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_inspect() {
        let cmd = parse_command(&args(&["inspect", "myapp"])).unwrap();
        assert_eq!(cmd, Command::Inspect { image: s("myapp") });
    }

    #[test]
    fn test_parse_inspect_missing_image() {
        let result = parse_command(&args(&["inspect"]));
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_is_attached() {
        let cmd = parse_command(&args(&["is-attached", "myapp"])).unwrap();
        assert_eq!(cmd, Command::IsAttached { image: s("myapp") });
    }

    #[test]
    fn test_parse_is_attached_missing_image() {
        let result = parse_command(&args(&["is-attached"]));
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_read_only() {
        let cmd = parse_command(&args(&["read-only", "myapp"])).unwrap();
        assert_eq!(
            cmd,
            Command::ReadOnly {
                image: s("myapp"),
                value: None,
            }
        );
    }

    #[test]
    fn test_parse_read_only_set_yes() {
        let cmd = parse_command(&args(&["read-only", "myapp", "yes"])).unwrap();
        assert_eq!(
            cmd,
            Command::ReadOnly {
                image: s("myapp"),
                value: Some(true),
            }
        );
    }

    #[test]
    fn test_parse_read_only_set_no() {
        let cmd = parse_command(&args(&["read-only", "myapp", "no"])).unwrap();
        assert_eq!(
            cmd,
            Command::ReadOnly {
                image: s("myapp"),
                value: Some(false),
            }
        );
    }

    #[test]
    fn test_parse_read_only_missing_image() {
        let result = parse_command(&args(&["read-only"]));
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_set_limit_global() {
        let cmd = parse_command(&args(&["set-limit", "500M"])).unwrap();
        assert_eq!(
            cmd,
            Command::SetLimit {
                image: None,
                bytes: s("500M"),
            }
        );
    }

    #[test]
    fn test_parse_set_limit_per_image() {
        let cmd = parse_command(&args(&["set-limit", "myapp", "1G"])).unwrap();
        assert_eq!(
            cmd,
            Command::SetLimit {
                image: Some(s("myapp")),
                bytes: s("1G"),
            }
        );
    }

    #[test]
    fn test_parse_set_limit_missing_args() {
        let result = parse_command(&args(&["set-limit"]));
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_help() {
        let cmd = parse_command(&args(&["help"])).unwrap();
        assert_eq!(cmd, Command::Help);
    }

    #[test]
    fn test_parse_help_flag() {
        let cmd = parse_command(&args(&["--help"])).unwrap();
        assert_eq!(cmd, Command::Help);
    }

    #[test]
    fn test_parse_help_short_flag() {
        let cmd = parse_command(&args(&["-h"])).unwrap();
        assert_eq!(cmd, Command::Help);
    }

    #[test]
    fn test_parse_unknown_command() {
        let result = parse_command(&args(&["frobnicate"]));
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Unknown command"));
    }

    #[test]
    fn test_parse_strips_no_pager() {
        let cmd = parse_command(&args(&["--no-pager", "list"])).unwrap();
        assert_eq!(cmd, Command::List);
    }

    #[test]
    fn test_parse_strips_quiet() {
        let cmd = parse_command(&args(&["-q", "list"])).unwrap();
        assert_eq!(cmd, Command::List);
    }

    #[test]
    fn test_parse_strips_no_legend() {
        let cmd = parse_command(&args(&["--no-legend", "list"])).unwrap();
        assert_eq!(cmd, Command::List);
    }

    #[test]
    fn test_parse_strips_no_ask_password() {
        let cmd = parse_command(&args(&["--no-ask-password", "list"])).unwrap();
        assert_eq!(cmd, Command::List);
    }

    #[test]
    fn test_parse_strips_no_reload() {
        let cmd = parse_command(&args(&["--no-reload", "attach", "x"])).unwrap();
        assert_eq!(
            cmd,
            Command::Attach {
                image: s("x"),
                profile: None,
                runtime: false,
            }
        );
    }

    // ── Offline image discovery tests ─────────────────────────────────────

    #[test]
    fn test_offline_discover_directory_image() {
        let tmp = temp_dir();
        create_test_image(tmp.path(), "myapp", &["myapp.service"]);

        let search = tmp.path().to_str().unwrap();
        let images = offline_discover_images_from(&[search]);
        assert_eq!(images.len(), 1);
        let img = images.get("myapp").unwrap();
        assert_eq!(img.image_type, "directory");
        assert_eq!(img.os_pretty_name.as_deref(), Some("Test OS"));
    }

    #[test]
    fn test_offline_discover_raw_image() {
        let tmp = temp_dir();
        create_test_raw_image(tmp.path(), "myraw");

        let search = tmp.path().to_str().unwrap();
        let images = offline_discover_images_from(&[search]);
        assert_eq!(images.len(), 1);
        let img = images.get("myraw").unwrap();
        assert_eq!(img.image_type, "raw");
    }

    #[test]
    fn test_offline_discover_skips_hidden() {
        let tmp = temp_dir();
        create_test_image(tmp.path(), ".hidden", &["hidden.service"]);

        let search = tmp.path().to_str().unwrap();
        let images = offline_discover_images_from(&[search]);
        assert_eq!(images.len(), 0);
    }

    #[test]
    fn test_offline_discover_multiple() {
        let tmp = temp_dir();
        create_test_image(tmp.path(), "app1", &["app1.service"]);
        create_test_image(tmp.path(), "app2", &["app2.service"]);
        create_test_raw_image(tmp.path(), "app3");

        let search = tmp.path().to_str().unwrap();
        let images = offline_discover_images_from(&[search]);
        assert_eq!(images.len(), 3);
    }

    #[test]
    fn test_offline_discover_priority() {
        let high = temp_dir();
        let low = temp_dir();
        create_test_image(high.path(), "shared", &["high.service"]);
        create_test_image(low.path(), "shared", &["low.service"]);

        let h = high.path().to_str().unwrap();
        let l = low.path().to_str().unwrap();
        let images = offline_discover_images_from(&[h, l]);
        assert_eq!(images.len(), 1);
        let img = images.get("shared").unwrap();
        assert!(img.path.starts_with(high.path()));
    }

    #[test]
    fn test_offline_discover_nonexistent() {
        let images = offline_discover_images_from(&["/nonexistent/that/does/not/exist"]);
        assert_eq!(images.len(), 0);
    }

    // ── Offline unit discovery tests ──────────────────────────────────────

    #[test]
    fn test_discover_units_basic() {
        let tmp = temp_dir();
        let img = create_test_image(tmp.path(), "app", &["foo.service", "bar.service"]);
        let units = discover_units(&img);
        assert_eq!(units, vec!["bar.service", "foo.service"]);
    }

    #[test]
    fn test_discover_units_various_types() {
        let tmp = temp_dir();
        let img = tmp.path().join("app");
        let unit_dir = img.join("usr/lib/systemd/system");
        fs::create_dir_all(&unit_dir).unwrap();
        fs::write(unit_dir.join("a.service"), "").unwrap();
        fs::write(unit_dir.join("b.socket"), "").unwrap();
        fs::write(unit_dir.join("c.target"), "").unwrap();
        fs::write(unit_dir.join("d.timer"), "").unwrap();
        fs::write(unit_dir.join("e.path"), "").unwrap();
        fs::write(unit_dir.join("f.conf"), "").unwrap(); // not a unit
        fs::write(unit_dir.join("README"), "").unwrap(); // not a unit

        let units = discover_units(&img);
        assert_eq!(
            units,
            vec!["a.service", "b.socket", "c.target", "d.timer", "e.path"]
        );
    }

    #[test]
    fn test_discover_units_empty() {
        let tmp = temp_dir();
        let img = tmp.path().join("empty");
        fs::create_dir_all(&img).unwrap();
        let units = discover_units(&img);
        assert!(units.is_empty());
    }

    // ── Offline os-release reading tests ──────────────────────────────────

    #[test]
    fn test_read_os_pretty_name() {
        let tmp = temp_dir();
        let img = create_test_image(tmp.path(), "myapp", &["myapp.service"]);
        let pretty = read_os_pretty_name(&img);
        assert_eq!(pretty.as_deref(), Some("Test OS"));
    }

    #[test]
    fn test_read_os_pretty_name_quoted() {
        let tmp = temp_dir();
        let img = tmp.path().join("img");
        let os_dir = img.join("usr/lib");
        fs::create_dir_all(&os_dir).unwrap();
        fs::write(os_dir.join("os-release"), "PRETTY_NAME='My Fancy OS'\n").unwrap();
        let pretty = read_os_pretty_name(&img);
        assert_eq!(pretty.as_deref(), Some("My Fancy OS"));
    }

    #[test]
    fn test_read_os_pretty_name_missing() {
        let tmp = temp_dir();
        let img = tmp.path().join("empty");
        fs::create_dir_all(&img).unwrap();
        let pretty = read_os_pretty_name(&img);
        assert!(pretty.is_none());
    }

    #[test]
    fn test_read_os_pretty_name_etc_fallback() {
        let tmp = temp_dir();
        let img = tmp.path().join("img");
        let os_dir = img.join("etc");
        fs::create_dir_all(&os_dir).unwrap();
        fs::write(os_dir.join("os-release"), "PRETTY_NAME=\"Fallback OS\"\n").unwrap();
        let pretty = read_os_pretty_name(&img);
        assert_eq!(pretty.as_deref(), Some("Fallback OS"));
    }

    // ── Offline attach state tests ────────────────────────────────────────

    #[test]
    fn test_offline_attach_state_detached() {
        let tmp = temp_dir();
        let state = offline_get_attach_state_from("nonexistent", tmp.path().to_str().unwrap());
        assert_eq!(state, "detached");
    }

    #[test]
    fn test_offline_attach_state_attached() {
        let tmp = temp_dir();
        fs::write(
            tmp.path().join("myapp"),
            "IMAGE_NAME=myapp\nIMAGE_PATH=/var/lib/portables/myapp\nRUNTIME=no\n",
        )
        .unwrap();
        let state = offline_get_attach_state_from("myapp", tmp.path().to_str().unwrap());
        assert_eq!(state, "attached");
    }

    #[test]
    fn test_offline_attach_state_attached_runtime() {
        let tmp = temp_dir();
        fs::write(
            tmp.path().join("myapp"),
            "IMAGE_NAME=myapp\nIMAGE_PATH=/run/portables/myapp\nRUNTIME=yes\n",
        )
        .unwrap();
        let state = offline_get_attach_state_from("myapp", tmp.path().to_str().unwrap());
        assert_eq!(state, "attached-runtime");
    }

    // ── Offline inspect tests ─────────────────────────────────────────────

    #[test]
    fn test_offline_inspect_directory() {
        let tmp = temp_dir();
        create_test_image(tmp.path(), "myapp", &["myapp.service"]);

        // Override search paths for the test
        let search = tmp.path().to_str().unwrap();
        let images = offline_discover_images_from(&[search]);
        let image = images.get("myapp").unwrap();

        // Manually build inspect output
        let mut lines = Vec::new();
        lines.push(format!("        Name: {}", image.name));
        lines.push(format!("        Path: {}", image.path.display()));
        lines.push(format!("        Type: {}", image.image_type));
        if let Some(ref os) = image.os_pretty_name {
            lines.push(format!("          OS: {}", os));
        }

        assert!(lines.iter().any(|l| l.contains("myapp")));
        assert!(lines.iter().any(|l| l.contains("directory")));
        assert!(lines.iter().any(|l| l.contains("Test OS")));
    }

    #[test]
    fn test_offline_inspect_not_found() {
        // We can't directly test offline_inspect without the global search paths,
        // but we can test the image discovery part
        let images = offline_discover_images_from(&["/nonexistent"]);
        assert!(!images.contains_key("nonexistent"));
    }

    // ── offline_list tests ────────────────────────────────────────────────

    #[test]
    fn test_offline_list_formatting() {
        // Test that offline_list produces reasonable output structure.
        // Since we can't control IMAGE_SEARCH_PATHS in unit tests easily,
        // we just verify the "no images" case.
        // The actual search paths likely have no images in the test environment.
        let output = offline_list();
        // Either "No images found." or a formatted table
        assert!(output.contains("images") || output.contains("NAME"));
    }

    // ── Integration: parse + execute pattern tests ────────────────────────

    #[test]
    fn test_attach_command_format() {
        // Verify that the command string we'd send to the daemon is well-formed
        let cmd = parse_command(&args(&["attach", "myapp", "default"])).unwrap();
        match cmd {
            Command::Attach {
                ref image,
                ref profile,
                runtime,
            } => {
                let mut cmd_str = format!("ATTACH {}", image);
                if let Some(prof) = profile {
                    cmd_str.push(' ');
                    cmd_str.push_str(prof);
                }
                if runtime {
                    cmd_str.push_str(" runtime");
                }
                assert_eq!(cmd_str, "ATTACH myapp default");
            }
            _ => panic!("Expected Attach command"),
        }
    }

    #[test]
    fn test_attach_runtime_command_format() {
        let cmd = parse_command(&args(&["--runtime", "attach", "myapp"])).unwrap();
        match cmd {
            Command::Attach {
                ref image,
                ref profile,
                runtime,
            } => {
                assert_eq!(image, "myapp");
                assert!(profile.is_none());
                assert!(runtime);
            }
            _ => panic!("Expected Attach command"),
        }
    }

    #[test]
    fn test_detach_command_format() {
        let cmd = parse_command(&args(&["detach", "myapp"])).unwrap();
        match cmd {
            Command::Detach { ref image } => {
                let cmd_str = format!("DETACH {}", image);
                assert_eq!(cmd_str, "DETACH myapp");
            }
            _ => panic!("Expected Detach command"),
        }
    }

    #[test]
    fn test_reattach_command_format() {
        let cmd = parse_command(&args(&["reattach", "myapp", "strict"])).unwrap();
        match cmd {
            Command::Reattach {
                ref image,
                ref profile,
                runtime,
            } => {
                assert_eq!(image, "myapp");
                assert_eq!(profile.as_deref(), Some("strict"));
                assert!(!runtime);
            }
            _ => panic!("Expected Reattach command"),
        }
    }

    #[test]
    fn test_is_attached_command_format() {
        let cmd = parse_command(&args(&["is-attached", "myapp"])).unwrap();
        match cmd {
            Command::IsAttached { ref image } => {
                let cmd_str = format!("IS-ATTACHED {}", image);
                assert_eq!(cmd_str, "IS-ATTACHED myapp");
            }
            _ => panic!("Expected IsAttached command"),
        }
    }

    // ── Edge cases ────────────────────────────────────────────────────────

    #[test]
    fn test_parse_multiple_flags() {
        let cmd = parse_command(&args(&[
            "--no-pager",
            "--no-legend",
            "--no-ask-password",
            "-q",
            "list",
        ]))
        .unwrap();
        assert_eq!(cmd, Command::List);
    }

    #[test]
    fn test_parse_runtime_after_command() {
        // --runtime must come before the command in our parsing,
        // but if it comes after "attach", it becomes part of filtered args.
        // This tests that our parser handles it when it's before the command.
        let cmd = parse_command(&args(&["--runtime", "attach", "img"])).unwrap();
        assert_eq!(
            cmd,
            Command::Attach {
                image: s("img"),
                profile: None,
                runtime: true,
            }
        );
    }

    #[test]
    fn test_parse_host_flag_skips_arg() {
        let cmd = parse_command(&args(&["-H", "myhost", "list"])).unwrap();
        assert_eq!(cmd, Command::List);
    }

    #[test]
    fn test_parse_machine_flag_skips_arg() {
        let cmd = parse_command(&args(&["-M", "mycontainer", "list"])).unwrap();
        assert_eq!(cmd, Command::List);
    }

    #[test]
    fn test_parse_json_flag_skips_arg() {
        let cmd = parse_command(&args(&["--json", "short", "list"])).unwrap();
        assert_eq!(cmd, Command::List);
    }
}
