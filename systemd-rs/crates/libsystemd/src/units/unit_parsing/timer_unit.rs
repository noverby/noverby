use log::trace;

use crate::units::{
    ParsedCommonConfig, ParsedFile, ParsedTimerConfig, ParsedTimerSection, ParsingErrorReason,
    parse_install_section, parse_unit_section,
};
use std::path::PathBuf;

pub fn parse_timer(
    parsed_file: ParsedFile,
    path: &PathBuf,
) -> Result<ParsedTimerConfig, ParsingErrorReason> {
    let mut install_config = None;
    let mut unit_config = None;
    let mut timer_section = ParsedTimerSection::default();

    for (name, section) in parsed_file {
        match name.as_str() {
            "[Unit]" => {
                unit_config = Some(parse_unit_section(section)?);
            }
            "[Install]" => {
                install_config = Some(parse_install_section(section)?);
            }
            "[Timer]" => {
                parse_timer_section(&section, &mut timer_section);
            }
            _ if name.starts_with("[X-") || name.starts_with("[x-") => {
                trace!("Silently ignoring vendor extension section in timer unit {path:?}: {name}");
            }
            _ => {
                trace!("Ignoring unknown section in timer unit {path:?}: {name}");
            }
        }
    }

    Ok(ParsedTimerConfig {
        common: ParsedCommonConfig {
            name: path.file_name().unwrap().to_str().unwrap().to_owned(),
            unit: unit_config.unwrap_or_else(Default::default),
            install: install_config.unwrap_or_else(Default::default),
            fragment_path: Some(path.clone()),
        },
        timer: timer_section,
    })
}

fn parse_timer_section(
    section: &std::collections::HashMap<String, Vec<(u32, String)>>,
    timer: &mut ParsedTimerSection,
) {
    // RemainAfterElapse defaults to true per systemd.timer(5)
    timer.remain_after_elapse = true;

    // Collect all key-value pairs sorted by line number so that
    // later assignments override earlier ones (matching systemd semantics).
    let mut entries: Vec<(u32, &str, &str)> = Vec::new();
    for (key, values) in section {
        for (line, value) in values {
            entries.push((*line, key.as_str(), value.as_str()));
        }
    }
    entries.sort_by_key(|(line, _, _)| *line);

    for (_line, key, value) in entries {
        match key {
            "OnActiveSec" => {
                if value.is_empty() {
                    timer.on_active_sec.clear();
                } else {
                    timer.on_active_sec.push(value.to_owned());
                }
            }
            "OnBootSec" => {
                if value.is_empty() {
                    timer.on_boot_sec.clear();
                } else {
                    timer.on_boot_sec.push(value.to_owned());
                }
            }
            "OnStartupSec" => {
                if value.is_empty() {
                    timer.on_startup_sec.clear();
                } else {
                    timer.on_startup_sec.push(value.to_owned());
                }
            }
            "OnUnitActiveSec" => {
                if value.is_empty() {
                    timer.on_unit_active_sec.clear();
                } else {
                    timer.on_unit_active_sec.push(value.to_owned());
                }
            }
            "OnUnitInactiveSec" => {
                if value.is_empty() {
                    timer.on_unit_inactive_sec.clear();
                } else {
                    timer.on_unit_inactive_sec.push(value.to_owned());
                }
            }
            "OnCalendar" => {
                if value.is_empty() {
                    timer.on_calendar.clear();
                } else {
                    timer.on_calendar.push(value.to_owned());
                }
            }
            "AccuracySec" => {
                timer.accuracy_sec = if value.is_empty() {
                    None
                } else {
                    Some(value.to_owned())
                };
            }
            "RandomizedDelaySec" => {
                timer.randomized_delay_sec = if value.is_empty() {
                    None
                } else {
                    Some(value.to_owned())
                };
            }
            "FixedRandomDelay" => {
                timer.fixed_random_delay = parse_bool(value);
            }
            "Persistent" => {
                timer.persistent = parse_bool(value);
            }
            "WakeSystem" => {
                timer.wake_system = parse_bool(value);
            }
            "RemainAfterElapse" => {
                timer.remain_after_elapse = parse_bool(value);
            }
            "Unit" => {
                timer.unit = if value.is_empty() {
                    None
                } else {
                    Some(value.to_owned())
                };
            }
            other => {
                trace!("Ignoring unknown key in [Timer] section: {other}={value}");
            }
        }
    }
}

fn parse_bool(value: &str) -> bool {
    matches!(value.to_lowercase().as_str(), "true" | "yes" | "on" | "1")
}
