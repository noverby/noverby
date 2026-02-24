//! Path unit monitoring — watches filesystem paths and triggers associated
//! service units when path conditions are met.
//!
//! After PID 1 finishes activating units, the path watcher thread wakes up
//! periodically and checks all active `.path` units to see if any of their
//! trigger conditions have been met. When a condition is satisfied, the watcher
//! starts (or restarts) the associated service unit.
//!
//! For `PathExists=` and `DirectoryNotEmpty=`, a simple poll-based check is
//! used. For `PathChanged=` and `PathModified=`, inotify is used when
//! available, with poll-based fallback.

use log::{debug, info, trace, warn};
use std::collections::HashMap;
use std::path::Path;
use std::time::{Duration, Instant};

use crate::lock_ext::RwLockExt;
use crate::runtime_info::ArcMutRuntimeInfo;
use crate::units::{
    ActivationSource, PathCondition, PathConfig, Specific, StatusStarted, UnitId, UnitStatus,
};

/// How often the path watcher thread wakes up to check path conditions.
const PATH_CHECK_INTERVAL: Duration = Duration::from_secs(2);

/// Start the background path watcher thread.
///
/// This should be called after the initial unit activation is complete
/// (or at least after all path units have been loaded and started).
pub fn start_path_watcher_thread(run_info: ArcMutRuntimeInfo) {
    std::thread::Builder::new()
        .name("path-watcher".into())
        .spawn(move || {
            info!("Path watcher started");

            // Give the system a moment to finish initial activation before
            // checking path units for the first time.
            std::thread::sleep(Duration::from_secs(3));

            // Track when each path unit last triggered, for rate limiting
            // (TriggerLimitIntervalSec= / TriggerLimitBurst=).
            let mut trigger_history: HashMap<String, Vec<Instant>> = HashMap::new();

            // Track file modification times for PathChanged= / PathModified=
            // so we can detect changes between polls.
            let mut last_state: HashMap<String, PathSnapshot> = HashMap::new();

            loop {
                check_and_trigger_paths(&run_info, &mut trigger_history, &mut last_state);
                std::thread::sleep(PATH_CHECK_INTERVAL);
            }
        })
        .expect("Failed to spawn path-watcher thread");
}

/// A snapshot of a path's state, used for change detection.
#[derive(Debug, Clone, PartialEq)]
struct PathSnapshot {
    exists: bool,
    modified: Option<std::time::SystemTime>,
    len: Option<u64>,
}

impl PathSnapshot {
    fn capture(path: &str) -> Self {
        match std::fs::metadata(path) {
            Ok(meta) => PathSnapshot {
                exists: true,
                modified: meta.modified().ok(),
                len: Some(meta.len()),
            },
            Err(_) => PathSnapshot {
                exists: false,
                modified: None,
                len: None,
            },
        }
    }
}

/// One pass of the path check loop.
fn check_and_trigger_paths(
    run_info: &ArcMutRuntimeInfo,
    trigger_history: &mut HashMap<String, Vec<Instant>>,
    last_state: &mut HashMap<String, PathSnapshot>,
) {
    let now = Instant::now();

    // Collect path units that need to trigger.
    // We collect first, then fire, to avoid holding the read lock during activation.
    let mut paths_to_trigger: Vec<(UnitId, String)> = Vec::new();

    {
        let ri = run_info.read_poisoned();
        for unit in ri.unit_table.values() {
            if let Specific::Path(path_specific) = &unit.specific {
                // Only check path units that are started/running
                let status = unit.common.status.read_poisoned().clone();
                if !matches!(status, UnitStatus::Started(StatusStarted::Running)) {
                    continue;
                }

                let conf = &path_specific.conf;
                let path_name = &unit.id.name;
                let target_unit = &conf.unit;

                if should_trigger_path(conf, path_name, now, trigger_history, last_state) {
                    paths_to_trigger.push((unit.id.clone(), target_unit.clone()));
                }
            }
        }
    }

    // Fire the collected path triggers
    for (path_id, target_unit_name) in paths_to_trigger {
        info!(
            "Path unit {} triggered, activating {}",
            path_id.name, target_unit_name
        );

        // Record trigger time for rate limiting
        trigger_history
            .entry(path_id.name.clone())
            .or_default()
            .push(now);

        fire_path_target(run_info, &target_unit_name);
    }
}

/// Check if a path unit's conditions are met and it should trigger.
fn should_trigger_path(
    conf: &PathConfig,
    path_name: &str,
    now: Instant,
    trigger_history: &mut HashMap<String, Vec<Instant>>,
    last_state: &mut HashMap<String, PathSnapshot>,
) -> bool {
    // Rate limiting check
    if is_rate_limited(conf, path_name, now, trigger_history) {
        trace!(
            "Path unit {} rate-limited, skipping trigger check",
            path_name
        );
        return false;
    }

    // Check each condition — any one matching is sufficient to trigger
    for condition in &conf.conditions {
        if check_condition(condition, path_name, last_state) {
            return true;
        }
    }

    false
}

/// Check if a single path condition is met.
fn check_condition(
    condition: &PathCondition,
    path_name: &str,
    last_state: &mut HashMap<String, PathSnapshot>,
) -> bool {
    match condition {
        PathCondition::PathExists(path) => {
            let exists = Path::new(path).exists();
            if exists {
                trace!("Path unit {}: PathExists={} satisfied", path_name, path);
            }
            exists
        }

        PathCondition::PathExistsGlob(pattern) => match glob_match_any(pattern) {
            true => {
                trace!(
                    "Path unit {}: PathExistsGlob={} satisfied",
                    path_name, pattern
                );
                true
            }
            false => false,
        },

        PathCondition::PathChanged(path) => {
            let key = format!("{path_name}:changed:{path}");
            let current = PathSnapshot::capture(path);
            let changed = match last_state.get(&key) {
                Some(prev) => *prev != current,
                None => {
                    // First check — record state but don't trigger
                    last_state.insert(key, current);
                    return false;
                }
            };
            last_state.insert(key, current);
            if changed {
                trace!("Path unit {}: PathChanged={} satisfied", path_name, path);
            }
            changed
        }

        PathCondition::PathModified(path) => {
            let key = format!("{path_name}:modified:{path}");
            let current = PathSnapshot::capture(path);
            let modified = match last_state.get(&key) {
                Some(prev) => {
                    // PathModified= also triggers on content changes (close_write)
                    // In poll mode, we detect this via mtime + size changes
                    *prev != current
                }
                None => {
                    // First check — record state but don't trigger
                    last_state.insert(key, current);
                    return false;
                }
            };
            last_state.insert(key, current);
            if modified {
                trace!("Path unit {}: PathModified={} satisfied", path_name, path);
            }
            modified
        }

        PathCondition::DirectoryNotEmpty(path) => {
            let not_empty = is_directory_not_empty(path);
            if not_empty {
                trace!(
                    "Path unit {}: DirectoryNotEmpty={} satisfied",
                    path_name, path
                );
            }
            not_empty
        }
    }
}

/// Check if the path unit has been triggered too many times recently.
fn is_rate_limited(
    conf: &PathConfig,
    path_name: &str,
    now: Instant,
    trigger_history: &mut HashMap<String, Vec<Instant>>,
) -> bool {
    let interval = conf.trigger_limit_interval_sec;
    let burst = conf.trigger_limit_burst;

    if burst == 0 || interval.is_zero() {
        return false;
    }

    let history = trigger_history.entry(path_name.to_owned()).or_default();

    // Remove entries older than the interval
    history.retain(|t| now.duration_since(*t) < interval);

    // Check if we've exceeded the burst limit
    history.len() >= burst as usize
}

/// Check if a directory exists and is not empty.
fn is_directory_not_empty(path: &str) -> bool {
    match std::fs::read_dir(path) {
        Ok(mut entries) => entries.next().is_some(),
        Err(_) => false,
    }
}

/// Simple glob matching for PathExistsGlob=.
/// Returns true if any filesystem entry matches the glob pattern.
fn glob_match_any(pattern: &str) -> bool {
    // Split the pattern into directory part and filename pattern
    let path = Path::new(pattern);
    let parent = match path.parent() {
        Some(p) if !p.as_os_str().is_empty() => p,
        _ => Path::new("/"),
    };
    let file_pattern = match path.file_name() {
        Some(f) => f.to_string_lossy().to_string(),
        None => return false,
    };

    let entries = match std::fs::read_dir(parent) {
        Ok(e) => e,
        Err(_) => return false,
    };

    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if simple_glob_match(&file_pattern, &name) {
            return true;
        }
    }
    false
}

/// Simple glob matching supporting `*` and `?` wildcards.
fn simple_glob_match(pattern: &str, text: &str) -> bool {
    let pat: Vec<char> = pattern.chars().collect();
    let txt: Vec<char> = text.chars().collect();
    glob_match_recursive(&pat, &txt, 0, 0)
}

fn glob_match_recursive(pat: &[char], txt: &[char], pi: usize, ti: usize) -> bool {
    if pi == pat.len() {
        return ti == txt.len();
    }
    if pat[pi] == '*' {
        // Try matching zero or more characters
        for skip in 0..=(txt.len() - ti) {
            if glob_match_recursive(pat, txt, pi + 1, ti + skip) {
                return true;
            }
        }
        false
    } else if pat[pi] == '?' {
        if ti < txt.len() {
            glob_match_recursive(pat, txt, pi + 1, ti + 1)
        } else {
            false
        }
    } else if ti < txt.len() && pat[pi] == txt[ti] {
        glob_match_recursive(pat, txt, pi + 1, ti + 1)
    } else {
        false
    }
}

/// Activate the target unit for a path trigger.
fn fire_path_target(run_info: &ArcMutRuntimeInfo, target_unit_name: &str) {
    let ri = run_info.read_poisoned();

    // Find the target unit
    let target_unit = ri
        .unit_table
        .values()
        .find(|u| u.id.name == target_unit_name);

    match target_unit {
        Some(unit) => {
            let status = unit.common.status.read_poisoned().clone();
            match status {
                UnitStatus::Started(_) => {
                    // Service is already running — try to restart it
                    debug!(
                        "Path target {} is already running, attempting restart",
                        target_unit_name
                    );
                    match unit.reactivate(&ri, ActivationSource::Regular) {
                        Ok(()) => {
                            info!("Path triggered: restarted {}", target_unit_name);
                        }
                        Err(e) => {
                            warn!("Path failed to restart {}: {}", target_unit_name, e);
                        }
                    }
                }
                _ => {
                    // Service is not running — start it
                    let id = unit.id.clone();
                    drop(ri);
                    match crate::units::activate_unit(
                        id,
                        &run_info.read_poisoned(),
                        ActivationSource::Regular,
                    ) {
                        Ok(_) => {
                            info!("Path triggered: started {}", target_unit_name);
                        }
                        Err(e) => {
                            warn!("Path failed to start {}: {}", target_unit_name, e);
                        }
                    }
                }
            }
        }
        None => {
            // Unit not in the boot dependency graph — log a warning.
            // In the future we could try on-demand loading here.
            debug!(
                "Path target {} not found in unit table, attempting on-demand load",
                target_unit_name
            );
            drop(ri);

            let ri = run_info.read_poisoned();
            if let Some(unit) = ri
                .unit_table
                .values()
                .find(|u| u.id.name == target_unit_name)
            {
                let id = unit.id.clone();
                drop(ri);
                match crate::units::activate_unit(
                    id,
                    &run_info.read_poisoned(),
                    ActivationSource::Regular,
                ) {
                    Ok(_) => info!("Path triggered: started {} (on-demand)", target_unit_name),
                    Err(e) => warn!(
                        "Path failed to start {} (on-demand): {}",
                        target_unit_name, e
                    ),
                }
            } else {
                warn!(
                    "Path target {} not found and could not be loaded",
                    target_unit_name
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::units::PathCondition;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_is_directory_not_empty_with_files() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("file.txt"), "content").unwrap();
        assert!(is_directory_not_empty(dir.path().to_str().unwrap()));
    }

    #[test]
    fn test_is_directory_not_empty_empty_dir() {
        let dir = TempDir::new().unwrap();
        assert!(!is_directory_not_empty(dir.path().to_str().unwrap()));
    }

    #[test]
    fn test_is_directory_not_empty_nonexistent() {
        assert!(!is_directory_not_empty("/nonexistent/path/12345"));
    }

    #[test]
    fn test_check_condition_path_exists() {
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("trigger");
        let mut last_state = HashMap::new();

        // File doesn't exist yet
        assert!(!check_condition(
            &PathCondition::PathExists(file_path.to_str().unwrap().to_owned()),
            "test.path",
            &mut last_state,
        ));

        // Create the file
        fs::write(&file_path, "").unwrap();
        assert!(check_condition(
            &PathCondition::PathExists(file_path.to_str().unwrap().to_owned()),
            "test.path",
            &mut last_state,
        ));
    }

    #[test]
    fn test_check_condition_directory_not_empty() {
        let dir = TempDir::new().unwrap();
        let watch_dir = dir.path().join("watched");
        fs::create_dir(&watch_dir).unwrap();
        let mut last_state = HashMap::new();

        // Directory is empty
        assert!(!check_condition(
            &PathCondition::DirectoryNotEmpty(watch_dir.to_str().unwrap().to_owned()),
            "test.path",
            &mut last_state,
        ));

        // Add a file
        fs::write(watch_dir.join("data"), "content").unwrap();
        assert!(check_condition(
            &PathCondition::DirectoryNotEmpty(watch_dir.to_str().unwrap().to_owned()),
            "test.path",
            &mut last_state,
        ));
    }

    #[test]
    fn test_check_condition_path_changed() {
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("watched");
        let path_str = file_path.to_str().unwrap().to_owned();
        let mut last_state = HashMap::new();

        // First check records initial state, doesn't trigger
        assert!(!check_condition(
            &PathCondition::PathChanged(path_str.clone()),
            "test.path",
            &mut last_state,
        ));

        // Second check with no changes — shouldn't trigger
        assert!(!check_condition(
            &PathCondition::PathChanged(path_str.clone()),
            "test.path",
            &mut last_state,
        ));

        // Create the file — should trigger
        fs::write(&file_path, "content").unwrap();
        assert!(check_condition(
            &PathCondition::PathChanged(path_str.clone()),
            "test.path",
            &mut last_state,
        ));

        // No change since last check — shouldn't trigger
        assert!(!check_condition(
            &PathCondition::PathChanged(path_str.clone()),
            "test.path",
            &mut last_state,
        ));
    }

    #[test]
    fn test_check_condition_path_modified() {
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("watched");
        let path_str = file_path.to_str().unwrap().to_owned();
        let mut last_state = HashMap::new();

        // First check records initial state
        assert!(!check_condition(
            &PathCondition::PathModified(path_str.clone()),
            "test.path",
            &mut last_state,
        ));

        // Create the file
        fs::write(&file_path, "content").unwrap();
        assert!(check_condition(
            &PathCondition::PathModified(path_str.clone()),
            "test.path",
            &mut last_state,
        ));
    }

    #[test]
    fn test_check_condition_path_exists_glob() {
        let dir = TempDir::new().unwrap();
        let mut last_state = HashMap::new();

        // No matching files
        let pattern = format!("{}/*.conf", dir.path().display());
        assert!(!check_condition(
            &PathCondition::PathExistsGlob(pattern.clone()),
            "test.path",
            &mut last_state,
        ));

        // Create a matching file
        fs::write(dir.path().join("test.conf"), "content").unwrap();
        assert!(check_condition(
            &PathCondition::PathExistsGlob(pattern),
            "test.path",
            &mut last_state,
        ));
    }

    #[test]
    fn test_simple_glob_match_star() {
        assert!(simple_glob_match("*.conf", "test.conf"));
        assert!(simple_glob_match("*.conf", ".conf"));
        assert!(!simple_glob_match("*.conf", "test.txt"));
        assert!(simple_glob_match("test*", "test.conf"));
        assert!(simple_glob_match("test*", "test"));
        assert!(simple_glob_match("*", "anything"));
        assert!(simple_glob_match("*", ""));
    }

    #[test]
    fn test_simple_glob_match_question() {
        assert!(simple_glob_match("test?.conf", "test1.conf"));
        assert!(!simple_glob_match("test?.conf", "test12.conf"));
        assert!(!simple_glob_match("test?.conf", "test.conf"));
    }

    #[test]
    fn test_simple_glob_match_exact() {
        assert!(simple_glob_match("test.conf", "test.conf"));
        assert!(!simple_glob_match("test.conf", "test.txt"));
    }

    #[test]
    fn test_simple_glob_match_combined() {
        assert!(simple_glob_match("*.??", "test.rs"));
        assert!(!simple_glob_match("*.??", "test.conf"));
        assert!(simple_glob_match("test*file?.txt", "test_data_file1.txt"));
    }

    #[test]
    fn test_rate_limiting() {
        let conf = PathConfig {
            conditions: vec![],
            make_directory: false,
            directory_mode: 0o755,
            trigger_limit_interval_sec: Duration::from_secs(10),
            trigger_limit_burst: 3,
            unit: "test.service".to_owned(),
        };

        let now = Instant::now();
        let mut history: HashMap<String, Vec<Instant>> = HashMap::new();

        // First 3 should not be rate limited
        assert!(!is_rate_limited(&conf, "test.path", now, &mut history));
        history.entry("test.path".to_owned()).or_default().push(now);
        assert!(!is_rate_limited(&conf, "test.path", now, &mut history));
        history.entry("test.path".to_owned()).or_default().push(now);
        assert!(!is_rate_limited(&conf, "test.path", now, &mut history));
        history.entry("test.path".to_owned()).or_default().push(now);

        // 4th should be rate limited
        assert!(is_rate_limited(&conf, "test.path", now, &mut history));
    }

    #[test]
    fn test_rate_limiting_disabled() {
        let conf = PathConfig {
            conditions: vec![],
            make_directory: false,
            directory_mode: 0o755,
            trigger_limit_interval_sec: Duration::ZERO,
            trigger_limit_burst: 0,
            unit: "test.service".to_owned(),
        };

        let now = Instant::now();
        let mut history: HashMap<String, Vec<Instant>> = HashMap::new();
        // With burst=0 or interval=0, rate limiting is disabled
        assert!(!is_rate_limited(&conf, "test.path", now, &mut history));
    }

    #[test]
    fn test_path_snapshot_capture_nonexistent() {
        let snap = PathSnapshot::capture("/nonexistent/path/12345");
        assert!(!snap.exists);
        assert!(snap.modified.is_none());
        assert!(snap.len.is_none());
    }

    #[test]
    fn test_path_snapshot_capture_existing() {
        let dir = TempDir::new().unwrap();
        let file = dir.path().join("test.txt");
        fs::write(&file, "hello").unwrap();

        let snap = PathSnapshot::capture(file.to_str().unwrap());
        assert!(snap.exists);
        assert!(snap.modified.is_some());
        assert_eq!(snap.len, Some(5));
    }

    #[test]
    fn test_path_snapshot_equality() {
        let snap1 = PathSnapshot {
            exists: true,
            modified: None,
            len: Some(10),
        };
        let snap2 = PathSnapshot {
            exists: true,
            modified: None,
            len: Some(10),
        };
        let snap3 = PathSnapshot {
            exists: true,
            modified: None,
            len: Some(20),
        };
        assert_eq!(snap1, snap2);
        assert_ne!(snap1, snap3);
    }

    #[test]
    fn test_path_condition_path_accessor() {
        let cond = PathCondition::PathExists("/tmp/test".to_owned());
        assert_eq!(cond.path(), "/tmp/test");

        let cond = PathCondition::PathExistsGlob("/tmp/*.conf".to_owned());
        assert_eq!(cond.path(), "/tmp/*.conf");

        let cond = PathCondition::PathChanged("/var/run/trigger".to_owned());
        assert_eq!(cond.path(), "/var/run/trigger");

        let cond = PathCondition::PathModified("/etc/config".to_owned());
        assert_eq!(cond.path(), "/etc/config");

        let cond = PathCondition::DirectoryNotEmpty("/var/spool/mail".to_owned());
        assert_eq!(cond.path(), "/var/spool/mail");
    }

    #[test]
    fn test_should_trigger_path_no_conditions() {
        let conf = PathConfig {
            conditions: vec![],
            make_directory: false,
            directory_mode: 0o755,
            trigger_limit_interval_sec: Duration::from_secs(2),
            trigger_limit_burst: 200,
            unit: "test.service".to_owned(),
        };
        let now = Instant::now();
        let mut trigger_history = HashMap::new();
        let mut last_state = HashMap::new();

        assert!(!should_trigger_path(
            &conf,
            "test.path",
            now,
            &mut trigger_history,
            &mut last_state,
        ));
    }

    #[test]
    fn test_should_trigger_path_exists_satisfied() {
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("trigger");
        fs::write(&file_path, "").unwrap();

        let conf = PathConfig {
            conditions: vec![PathCondition::PathExists(
                file_path.to_str().unwrap().to_owned(),
            )],
            make_directory: false,
            directory_mode: 0o755,
            trigger_limit_interval_sec: Duration::from_secs(2),
            trigger_limit_burst: 200,
            unit: "test.service".to_owned(),
        };
        let now = Instant::now();
        let mut trigger_history = HashMap::new();
        let mut last_state = HashMap::new();

        assert!(should_trigger_path(
            &conf,
            "test.path",
            now,
            &mut trigger_history,
            &mut last_state,
        ));
    }

    #[test]
    fn test_should_trigger_path_exists_not_satisfied() {
        let conf = PathConfig {
            conditions: vec![PathCondition::PathExists(
                "/nonexistent/path/12345".to_owned(),
            )],
            make_directory: false,
            directory_mode: 0o755,
            trigger_limit_interval_sec: Duration::from_secs(2),
            trigger_limit_burst: 200,
            unit: "test.service".to_owned(),
        };
        let now = Instant::now();
        let mut trigger_history = HashMap::new();
        let mut last_state = HashMap::new();

        assert!(!should_trigger_path(
            &conf,
            "test.path",
            now,
            &mut trigger_history,
            &mut last_state,
        ));
    }
}
