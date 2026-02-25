use log::trace;

use crate::units::{
    ParsedCommonConfig, ParsedFile, ParsedSliceConfig, ParsedSliceSection, ParsingErrorReason,
    parse_install_section, parse_unit_section,
};
use std::path::PathBuf;

pub fn parse_slice(
    parsed_file: ParsedFile,
    path: &PathBuf,
) -> Result<ParsedSliceConfig, ParsingErrorReason> {
    let mut install_config = None;
    let mut unit_config = None;
    let mut slice_section = ParsedSliceSection::default();

    for (name, section) in parsed_file {
        match name.as_str() {
            "[Unit]" => {
                unit_config = Some(parse_unit_section(section)?);
            }
            "[Install]" => {
                install_config = Some(parse_install_section(section)?);
            }
            "[Slice]" => {
                parse_slice_section(&section, &mut slice_section)?;
            }
            _ if name.starts_with("[X-") || name.starts_with("[x-") => {
                trace!("Silently ignoring vendor extension section in slice unit {path:?}: {name}");
            }
            _ => {
                trace!("Ignoring unknown section in slice unit {path:?}: {name}");
            }
        }
    }

    Ok(ParsedSliceConfig {
        common: ParsedCommonConfig {
            name: path.file_name().unwrap().to_str().unwrap().to_owned(),
            unit: unit_config.unwrap_or_default(),
            install: install_config.unwrap_or_default(),
            fragment_path: Some(path.clone()),
        },
        slice: slice_section,
    })
}

fn parse_slice_section(
    section: &std::collections::HashMap<String, Vec<(u32, String)>>,
    slice: &mut ParsedSliceSection,
) -> Result<(), ParsingErrorReason> {
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
        // Note: the unit file parser (`parse_section`) converts all keys to
        // uppercase, so we match against uppercase names here.
        match key {
            // --- Memory controller ---
            "MEMORYMIN" => {
                slice.memory_min = parse_memory_limit_opt(value, "MemoryMin")?;
            }
            "MEMORYLOW" => {
                slice.memory_low = parse_memory_limit_opt(value, "MemoryLow")?;
            }
            "MEMORYHIGH" => {
                slice.memory_high = parse_memory_limit_opt(value, "MemoryHigh")?;
            }
            "MEMORYMAX" => {
                slice.memory_max = parse_memory_limit_opt(value, "MemoryMax")?;
            }
            "MEMORYSWAPMAX" => {
                slice.memory_swap_max = parse_memory_limit_opt(value, "MemorySwapMax")?;
            }

            // --- CPU controller ---
            "CPUWEIGHT" => {
                slice.cpu_weight = super::parse_cpu_weight(value)
                    .map_err(|e| ParsingErrorReason::Generic(format!("CPUWeight: {e}")))?;
            }
            "STARTUPCPUWEIGHT" => {
                slice.startup_cpu_weight = super::parse_cpu_weight(value)
                    .map_err(|e| ParsingErrorReason::Generic(format!("StartupCPUWeight: {e}")))?;
            }
            "CPUQUOTA" => {
                slice.cpu_quota = super::parse_cpu_quota(value)
                    .map_err(|e| ParsingErrorReason::Generic(format!("CPUQuota: {e}")))?;
            }

            // --- IO controller ---
            "IOWEIGHT" => {
                slice.io_weight = super::parse_io_weight(value)
                    .map_err(|e| ParsingErrorReason::Generic(format!("IOWeight: {e}")))?;
            }
            "STARTUPIOWEIGHT" => {
                slice.startup_io_weight = super::parse_io_weight(value)
                    .map_err(|e| ParsingErrorReason::Generic(format!("StartupIOWeight: {e}")))?;
            }
            "IODEVICEWEIGHT" => {
                parse_io_device_list(value, &mut slice.io_device_weight, "IODeviceWeight")?;
            }
            "IOREADBANDWIDTHMAX" => {
                parse_io_device_list(
                    value,
                    &mut slice.io_read_bandwidth_max,
                    "IOReadBandwidthMax",
                )?;
            }
            "IOWRITEBANDWIDTHMAX" => {
                parse_io_device_list(
                    value,
                    &mut slice.io_write_bandwidth_max,
                    "IOWriteBandwidthMax",
                )?;
            }
            "IOREADIOPSMAX" => {
                parse_io_device_list(value, &mut slice.io_read_iops_max, "IOReadIOPSMax")?;
            }
            "IOWRITEIOPSMAX" => {
                parse_io_device_list(value, &mut slice.io_write_iops_max, "IOWriteIOPSMax")?;
            }

            // --- Tasks (pids) controller ---
            "TASKSMAX" => {
                let trimmed = value.trim();
                if trimmed.is_empty() {
                    slice.tasks_max = None;
                } else if trimmed.eq_ignore_ascii_case("infinity") {
                    slice.tasks_max = Some(super::TasksMax::Infinity);
                } else if let Some(pct) = trimmed.strip_suffix('%') {
                    let pct_val = pct.trim().parse::<u64>().map_err(|_| {
                        ParsingErrorReason::Generic(format!(
                            "TasksMax percentage is not a valid number: {value}"
                        ))
                    })?;
                    slice.tasks_max = Some(super::TasksMax::Percent(pct_val));
                } else {
                    let num = trimmed.parse::<u64>().map_err(|_| {
                        ParsingErrorReason::Generic(format!(
                            "TasksMax is not a valid value: {value}"
                        ))
                    })?;
                    slice.tasks_max = Some(super::TasksMax::Value(num));
                }
            }

            // --- Delegation ---
            "DELEGATE" => {
                let trimmed = value.trim();
                if string_to_bool(trimmed) {
                    slice.delegate = super::Delegate::Yes;
                } else if trimmed.eq_ignore_ascii_case("no")
                    || trimmed.eq_ignore_ascii_case("false")
                    || trimmed == "0"
                    || trimmed.is_empty()
                {
                    slice.delegate = super::Delegate::No;
                } else {
                    // Treat as a space-separated list of controller names
                    let controllers: Vec<String> =
                        trimmed.split_whitespace().map(|s| s.to_owned()).collect();
                    if controllers.is_empty() {
                        slice.delegate = super::Delegate::No;
                    } else {
                        slice.delegate = super::Delegate::Controllers(controllers);
                    }
                }
            }

            // --- Accounting toggles ---
            "CPUACCOUNTING" => {
                slice.cpu_accounting = Some(string_to_bool(value.trim()));
            }
            "MEMORYACCOUNTING" => {
                slice.memory_accounting = Some(string_to_bool(value.trim()));
            }
            "IOACCOUNTING" => {
                slice.io_accounting = Some(string_to_bool(value.trim()));
            }
            "TASKSACCOUNTING" => {
                slice.tasks_accounting = Some(string_to_bool(value.trim()));
            }

            // --- Device policy ---
            "DEVICEALLOW" => {
                let trimmed = value.trim();
                if trimmed.is_empty() {
                    slice.device_allow.clear();
                } else {
                    slice.device_allow.push(trimmed.to_owned());
                }
            }
            "DEVICEPOLICY" => {
                slice.device_policy = match value.trim().to_lowercase().as_str() {
                    "auto" | "" => super::DevicePolicy::Auto,
                    "closed" => super::DevicePolicy::Closed,
                    "strict" => super::DevicePolicy::Strict,
                    other => {
                        return Err(ParsingErrorReason::UnknownSetting(
                            "DevicePolicy".to_owned(),
                            other.to_owned(),
                        ));
                    }
                };
            }

            // --- IP address filtering ---
            "IPADDRESSALLOW" => {
                let trimmed = value.trim();
                if trimmed.is_empty() {
                    slice.ip_address_allow.clear();
                } else {
                    for addr in trimmed.split_whitespace() {
                        slice.ip_address_allow.push(addr.to_owned());
                    }
                }
            }
            "IPADDRESSDENY" => {
                let trimmed = value.trim();
                if trimmed.is_empty() {
                    slice.ip_address_deny.clear();
                } else {
                    for addr in trimmed.split_whitespace() {
                        slice.ip_address_deny.push(addr.to_owned());
                    }
                }
            }

            // --- Managed OOM ---
            "MANAGEDOOMSWAP" => {
                let trimmed = value.trim();
                slice.managed_oom_swap = if trimmed.is_empty() {
                    None
                } else {
                    Some(trimmed.to_owned())
                };
            }
            "MANAGEDOOMMEMORYPRESSURE" => {
                let trimmed = value.trim();
                slice.managed_oom_memory_pressure = if trimmed.is_empty() {
                    None
                } else {
                    Some(trimmed.to_owned())
                };
            }
            "MANAGEDOOMMEMORYPRESSURELIMIT" => {
                let trimmed = value.trim();
                slice.managed_oom_memory_pressure_limit = if trimmed.is_empty() {
                    None
                } else {
                    Some(trimmed.to_owned())
                };
            }
            "MANAGEDOOMPREFERENCE" => {
                let trimmed = value.trim();
                slice.managed_oom_preference = if trimmed.is_empty() {
                    None
                } else {
                    Some(trimmed.to_owned())
                };
            }

            // --- Memory pressure watch ---
            "MEMORYPRESSUREWATCH" => {
                slice.memory_pressure_watch = match value.trim().to_lowercase().as_str() {
                    "auto" | "" => super::MemoryPressureWatch::Auto,
                    "on" | "yes" | "true" | "1" => super::MemoryPressureWatch::On,
                    "off" | "no" | "false" | "0" => super::MemoryPressureWatch::Off,
                    "skip" => super::MemoryPressureWatch::Skip,
                    other => {
                        return Err(ParsingErrorReason::UnknownSetting(
                            "MemoryPressureWatch".to_owned(),
                            other.to_owned(),
                        ));
                    }
                };
            }

            other => {
                if other.starts_with("X-") || other.starts_with("x-") {
                    trace!(
                        "Silently ignoring vendor extension in [Slice] section: {other}={value}"
                    );
                } else {
                    trace!("Ignoring unsupported setting in [Slice] section: {other}={value}");
                }
            }
        }
    }

    Ok(())
}

/// Parse a memory limit value (bytes with suffix, percentage, or "infinity")
/// returning `None` for empty strings.
fn parse_memory_limit_opt(
    value: &str,
    name: &str,
) -> Result<Option<super::MemoryLimit>, ParsingErrorReason> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    super::parse_memory_limit(trimmed)
        .map_err(|e| ParsingErrorReason::Generic(format!("{name}: {e}")))
}

/// Parse and accumulate an I/O device limit entry. An empty value resets the
/// list (matching systemd semantics).
fn parse_io_device_list(
    value: &str,
    entries: &mut Vec<super::IoDeviceLimit>,
    name: &str,
) -> Result<(), ParsingErrorReason> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        entries.clear();
        return Ok(());
    }
    if let Some(limit) = super::parse_io_device_limit(trimmed)
        .map_err(|e| ParsingErrorReason::Generic(format!("{name}: {e}")))?
    {
        entries.push(limit);
    }
    Ok(())
}

fn string_to_bool(val: &str) -> bool {
    matches!(
        val.to_lowercase().as_str(),
        "true" | "yes" | "on" | "1" | "y"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::units::unit_parsing::unit_parser::parse_file;

    fn parse_slice_from_str(content: &str) -> Result<ParsedSliceConfig, ParsingErrorReason> {
        let path = PathBuf::from("/test/test.slice");
        let parsed_file = parse_file(content)?;
        parse_slice(parsed_file, &path)
    }

    #[test]
    fn test_empty_slice() {
        let content = "[Unit]\nDescription=Test Slice\n";
        let config = parse_slice_from_str(content).unwrap();
        assert_eq!(config.common.name, "test.slice");
        assert!(config.slice.memory_max.is_none());
        assert!(config.slice.cpu_quota.is_none());
        assert!(config.slice.tasks_max.is_none());
    }

    #[test]
    fn test_memory_max_bytes() {
        let content = "[Slice]\nMemoryMax=1073741824\n";
        let config = parse_slice_from_str(content).unwrap();
        assert!(config.slice.memory_max.is_some());
        if let Some(super::super::MemoryLimit::Bytes(b)) = config.slice.memory_max {
            assert_eq!(b, 1073741824);
        } else {
            panic!("Expected MemoryLimit::Bytes");
        }
    }

    #[test]
    fn test_memory_max_suffix() {
        let content = "[Slice]\nMemoryMax=1G\n";
        let config = parse_slice_from_str(content).unwrap();
        assert!(config.slice.memory_max.is_some());
        if let Some(super::super::MemoryLimit::Bytes(b)) = config.slice.memory_max {
            assert_eq!(b, 1024 * 1024 * 1024);
        } else {
            panic!("Expected MemoryLimit::Bytes");
        }
    }

    #[test]
    fn test_memory_max_percentage() {
        let content = "[Slice]\nMemoryMax=80%\n";
        let config = parse_slice_from_str(content).unwrap();
        if let Some(super::super::MemoryLimit::Percent(p)) = config.slice.memory_max {
            assert_eq!(p, 80);
        } else {
            panic!("Expected MemoryLimit::Percent");
        }
    }

    #[test]
    fn test_memory_max_infinity() {
        let content = "[Slice]\nMemoryMax=infinity\n";
        let config = parse_slice_from_str(content).unwrap();
        assert_eq!(
            config.slice.memory_max,
            Some(super::super::MemoryLimit::Infinity)
        );
    }

    #[test]
    fn test_memory_min_low_high() {
        let content = "[Slice]\nMemoryMin=64M\nMemoryLow=128M\nMemoryHigh=2G\n";
        let config = parse_slice_from_str(content).unwrap();
        assert!(config.slice.memory_min.is_some());
        assert!(config.slice.memory_low.is_some());
        assert!(config.slice.memory_high.is_some());
    }

    #[test]
    fn test_memory_swap_max() {
        let content = "[Slice]\nMemorySwapMax=512M\n";
        let config = parse_slice_from_str(content).unwrap();
        assert!(config.slice.memory_swap_max.is_some());
    }

    #[test]
    fn test_cpu_weight() {
        let content = "[Slice]\nCPUWeight=200\n";
        let config = parse_slice_from_str(content).unwrap();
        assert_eq!(config.slice.cpu_weight, Some(200));
    }

    #[test]
    fn test_cpu_weight_idle() {
        let content = "[Slice]\nCPUWeight=idle\n";
        let config = parse_slice_from_str(content).unwrap();
        assert_eq!(config.slice.cpu_weight, Some(1));
    }

    #[test]
    fn test_startup_cpu_weight() {
        let content = "[Slice]\nStartupCPUWeight=500\n";
        let config = parse_slice_from_str(content).unwrap();
        assert_eq!(config.slice.startup_cpu_weight, Some(500));
    }

    #[test]
    fn test_cpu_quota() {
        let content = "[Slice]\nCPUQuota=20%\n";
        let config = parse_slice_from_str(content).unwrap();
        assert_eq!(config.slice.cpu_quota, Some(20));
    }

    #[test]
    fn test_cpu_quota_200() {
        let content = "[Slice]\nCPUQuota=200%\n";
        let config = parse_slice_from_str(content).unwrap();
        assert_eq!(config.slice.cpu_quota, Some(200));
    }

    #[test]
    fn test_io_weight() {
        let content = "[Slice]\nIOWeight=500\n";
        let config = parse_slice_from_str(content).unwrap();
        assert_eq!(config.slice.io_weight, Some(500));
    }

    #[test]
    fn test_startup_io_weight() {
        let content = "[Slice]\nStartupIOWeight=100\n";
        let config = parse_slice_from_str(content).unwrap();
        assert_eq!(config.slice.startup_io_weight, Some(100));
    }

    #[test]
    fn test_tasks_max_value() {
        let content = "[Slice]\nTasksMax=4096\n";
        let config = parse_slice_from_str(content).unwrap();
        assert_eq!(
            config.slice.tasks_max,
            Some(super::super::TasksMax::Value(4096))
        );
    }

    #[test]
    fn test_tasks_max_infinity() {
        let content = "[Slice]\nTasksMax=infinity\n";
        let config = parse_slice_from_str(content).unwrap();
        assert_eq!(
            config.slice.tasks_max,
            Some(super::super::TasksMax::Infinity)
        );
    }

    #[test]
    fn test_tasks_max_percent() {
        let content = "[Slice]\nTasksMax=50%\n";
        let config = parse_slice_from_str(content).unwrap();
        assert_eq!(
            config.slice.tasks_max,
            Some(super::super::TasksMax::Percent(50))
        );
    }

    #[test]
    fn test_delegate_yes() {
        let content = "[Slice]\nDelegate=yes\n";
        let config = parse_slice_from_str(content).unwrap();
        assert_eq!(config.slice.delegate, super::super::Delegate::Yes);
    }

    #[test]
    fn test_delegate_no() {
        let content = "[Slice]\nDelegate=no\n";
        let config = parse_slice_from_str(content).unwrap();
        assert_eq!(config.slice.delegate, super::super::Delegate::No);
    }

    #[test]
    fn test_delegate_controllers() {
        let content = "[Slice]\nDelegate=cpu memory io\n";
        let config = parse_slice_from_str(content).unwrap();
        assert_eq!(
            config.slice.delegate,
            super::super::Delegate::Controllers(vec![
                "cpu".to_owned(),
                "memory".to_owned(),
                "io".to_owned()
            ])
        );
    }

    #[test]
    fn test_cpu_accounting() {
        let content = "[Slice]\nCPUAccounting=yes\n";
        let config = parse_slice_from_str(content).unwrap();
        assert_eq!(config.slice.cpu_accounting, Some(true));
    }

    #[test]
    fn test_memory_accounting_false() {
        let content = "[Slice]\nMemoryAccounting=no\n";
        let config = parse_slice_from_str(content).unwrap();
        assert_eq!(config.slice.memory_accounting, Some(false));
    }

    #[test]
    fn test_io_accounting() {
        let content = "[Slice]\nIOAccounting=true\n";
        let config = parse_slice_from_str(content).unwrap();
        assert_eq!(config.slice.io_accounting, Some(true));
    }

    #[test]
    fn test_tasks_accounting() {
        let content = "[Slice]\nTasksAccounting=yes\n";
        let config = parse_slice_from_str(content).unwrap();
        assert_eq!(config.slice.tasks_accounting, Some(true));
    }

    #[test]
    fn test_device_allow() {
        let content = "[Slice]\nDeviceAllow=/dev/null rwm\nDeviceAllow=/dev/zero r\n";
        let config = parse_slice_from_str(content).unwrap();
        assert_eq!(config.slice.device_allow.len(), 2);
        assert_eq!(config.slice.device_allow[0], "/dev/null rwm");
        assert_eq!(config.slice.device_allow[1], "/dev/zero r");
    }

    #[test]
    fn test_device_allow_reset() {
        let content = "[Slice]\nDeviceAllow=/dev/null rwm\nDeviceAllow=\nDeviceAllow=/dev/zero r\n";
        let config = parse_slice_from_str(content).unwrap();
        assert_eq!(config.slice.device_allow.len(), 1);
        assert_eq!(config.slice.device_allow[0], "/dev/zero r");
    }

    #[test]
    fn test_device_policy_auto() {
        let content = "[Slice]\nDevicePolicy=auto\n";
        let config = parse_slice_from_str(content).unwrap();
        assert_eq!(config.slice.device_policy, super::super::DevicePolicy::Auto);
    }

    #[test]
    fn test_device_policy_strict() {
        let content = "[Slice]\nDevicePolicy=strict\n";
        let config = parse_slice_from_str(content).unwrap();
        assert_eq!(
            config.slice.device_policy,
            super::super::DevicePolicy::Strict
        );
    }

    #[test]
    fn test_device_policy_closed() {
        let content = "[Slice]\nDevicePolicy=closed\n";
        let config = parse_slice_from_str(content).unwrap();
        assert_eq!(
            config.slice.device_policy,
            super::super::DevicePolicy::Closed
        );
    }

    #[test]
    fn test_ip_address_allow() {
        let content = "[Slice]\nIPAddressAllow=10.0.0.0/8 192.168.0.0/16\n";
        let config = parse_slice_from_str(content).unwrap();
        assert_eq!(config.slice.ip_address_allow.len(), 2);
        assert_eq!(config.slice.ip_address_allow[0], "10.0.0.0/8");
        assert_eq!(config.slice.ip_address_allow[1], "192.168.0.0/16");
    }

    #[test]
    fn test_ip_address_deny() {
        let content = "[Slice]\nIPAddressDeny=any\n";
        let config = parse_slice_from_str(content).unwrap();
        assert_eq!(config.slice.ip_address_deny, vec!["any"]);
    }

    #[test]
    fn test_ip_address_allow_reset() {
        let content =
            "[Slice]\nIPAddressAllow=10.0.0.0/8\nIPAddressAllow=\nIPAddressAllow=127.0.0.0/8\n";
        let config = parse_slice_from_str(content).unwrap();
        assert_eq!(config.slice.ip_address_allow.len(), 1);
        assert_eq!(config.slice.ip_address_allow[0], "127.0.0.0/8");
    }

    #[test]
    fn test_managed_oom_swap() {
        let content = "[Slice]\nManagedOOMSwap=kill\n";
        let config = parse_slice_from_str(content).unwrap();
        assert_eq!(config.slice.managed_oom_swap, Some("kill".to_owned()));
    }

    #[test]
    fn test_managed_oom_memory_pressure() {
        let content = "[Slice]\nManagedOOMMemoryPressure=kill\n";
        let config = parse_slice_from_str(content).unwrap();
        assert_eq!(
            config.slice.managed_oom_memory_pressure,
            Some("kill".to_owned())
        );
    }

    #[test]
    fn test_managed_oom_memory_pressure_limit() {
        let content = "[Slice]\nManagedOOMMemoryPressureLimit=50%\n";
        let config = parse_slice_from_str(content).unwrap();
        assert_eq!(
            config.slice.managed_oom_memory_pressure_limit,
            Some("50%".to_owned())
        );
    }

    #[test]
    fn test_managed_oom_preference() {
        let content = "[Slice]\nManagedOOMPreference=avoid\n";
        let config = parse_slice_from_str(content).unwrap();
        assert_eq!(
            config.slice.managed_oom_preference,
            Some("avoid".to_owned())
        );
    }

    #[test]
    fn test_memory_pressure_watch() {
        let content = "[Slice]\nMemoryPressureWatch=on\n";
        let config = parse_slice_from_str(content).unwrap();
        assert_eq!(
            config.slice.memory_pressure_watch,
            super::super::MemoryPressureWatch::On
        );
    }

    #[test]
    fn test_memory_pressure_watch_off() {
        let content = "[Slice]\nMemoryPressureWatch=off\n";
        let config = parse_slice_from_str(content).unwrap();
        assert_eq!(
            config.slice.memory_pressure_watch,
            super::super::MemoryPressureWatch::Off
        );
    }

    #[test]
    fn test_memory_pressure_watch_auto() {
        let content = "[Slice]\nMemoryPressureWatch=auto\n";
        let config = parse_slice_from_str(content).unwrap();
        assert_eq!(
            config.slice.memory_pressure_watch,
            super::super::MemoryPressureWatch::Auto
        );
    }

    #[test]
    fn test_combined_resource_control() {
        let content = "\
[Unit]
Description=Test Resource Control Slice

[Slice]
MemoryMax=2G
MemoryHigh=1G
MemoryLow=256M
CPUWeight=200
CPUQuota=50%
IOWeight=100
TasksMax=1024
Delegate=yes
CPUAccounting=yes
MemoryAccounting=yes
IOAccounting=yes
TasksAccounting=yes
DevicePolicy=closed
DeviceAllow=/dev/null rw
IPAddressDeny=any
IPAddressAllow=127.0.0.0/8
";
        let config = parse_slice_from_str(content).unwrap();
        assert!(config.slice.memory_max.is_some());
        assert!(config.slice.memory_high.is_some());
        assert!(config.slice.memory_low.is_some());
        assert_eq!(config.slice.cpu_weight, Some(200));
        assert_eq!(config.slice.cpu_quota, Some(50));
        assert_eq!(config.slice.io_weight, Some(100));
        assert_eq!(
            config.slice.tasks_max,
            Some(super::super::TasksMax::Value(1024))
        );
        assert_eq!(config.slice.delegate, super::super::Delegate::Yes);
        assert_eq!(config.slice.cpu_accounting, Some(true));
        assert_eq!(config.slice.memory_accounting, Some(true));
        assert_eq!(config.slice.io_accounting, Some(true));
        assert_eq!(config.slice.tasks_accounting, Some(true));
        assert_eq!(
            config.slice.device_policy,
            super::super::DevicePolicy::Closed
        );
        assert_eq!(config.slice.device_allow.len(), 1);
        assert_eq!(config.slice.ip_address_deny, vec!["any"]);
        assert_eq!(config.slice.ip_address_allow, vec!["127.0.0.0/8"]);
    }

    #[test]
    fn test_unknown_key_ignored() {
        let content = "[Slice]\nSomeUnknownKey=value\n";
        let config = parse_slice_from_str(content).unwrap();
        // Should parse without error — unknown keys are traced and ignored
        assert!(config.slice.memory_max.is_none());
    }

    #[test]
    fn test_vendor_extension_ignored() {
        let content = "[Slice]\nX-CustomVendor=something\n";
        let config = parse_slice_from_str(content).unwrap();
        assert!(config.slice.memory_max.is_none());
    }

    #[test]
    fn test_vendor_extension_section_ignored() {
        let content = "[X-Custom]\nFoo=bar\n";
        let config = parse_slice_from_str(content).unwrap();
        assert!(config.slice.memory_max.is_none());
    }

    #[test]
    fn test_io_device_weight() {
        let content = "[Slice]\nIODeviceWeight=/dev/sda 200\n";
        let config = parse_slice_from_str(content).unwrap();
        assert_eq!(config.slice.io_device_weight.len(), 1);
        assert_eq!(config.slice.io_device_weight[0].device, "/dev/sda");
    }

    #[test]
    fn test_io_device_weight_reset() {
        let content = "[Slice]\nIODeviceWeight=/dev/sda 200\nIODeviceWeight=\n";
        let config = parse_slice_from_str(content).unwrap();
        assert_eq!(config.slice.io_device_weight.len(), 0);
    }

    #[test]
    fn test_io_read_bandwidth_max() {
        let content = "[Slice]\nIOReadBandwidthMax=/dev/sda 10M\n";
        let config = parse_slice_from_str(content).unwrap();
        assert_eq!(config.slice.io_read_bandwidth_max.len(), 1);
    }

    #[test]
    fn test_io_write_bandwidth_max() {
        let content = "[Slice]\nIOWriteBandwidthMax=/dev/sda 5M\n";
        let config = parse_slice_from_str(content).unwrap();
        assert_eq!(config.slice.io_write_bandwidth_max.len(), 1);
    }

    #[test]
    fn test_io_read_iops_max() {
        let content = "[Slice]\nIOReadIOPSMax=/dev/sda 1000\n";
        let config = parse_slice_from_str(content).unwrap();
        assert_eq!(config.slice.io_read_iops_max.len(), 1);
    }

    #[test]
    fn test_io_write_iops_max() {
        let content = "[Slice]\nIOWriteIOPSMax=/dev/sda 500\n";
        let config = parse_slice_from_str(content).unwrap();
        assert_eq!(config.slice.io_write_iops_max.len(), 1);
    }

    #[test]
    fn test_empty_values_clear() {
        let content = "\
[Slice]
MemoryMax=1G
MemoryMax=
CPUWeight=200
CPUWeight=
IOWeight=100
IOWeight=
";
        let config = parse_slice_from_str(content).unwrap();
        // Empty assignment should clear / set to None
        assert!(config.slice.memory_max.is_none());
        assert!(config.slice.cpu_weight.is_none());
        assert!(config.slice.io_weight.is_none());
    }

    #[test]
    fn test_later_value_overrides() {
        let content = "\
[Slice]
MemoryMax=1G
MemoryMax=2G
CPUWeight=100
CPUWeight=500
";
        let config = parse_slice_from_str(content).unwrap();
        if let Some(super::super::MemoryLimit::Bytes(b)) = config.slice.memory_max {
            assert_eq!(b, 2 * 1024 * 1024 * 1024);
        } else {
            panic!("Expected MemoryLimit::Bytes(2G)");
        }
        assert_eq!(config.slice.cpu_weight, Some(500));
    }

    #[test]
    fn test_memory_swap_max_zero() {
        let content = "[Slice]\nMemorySwapMax=0\n";
        let config = parse_slice_from_str(content).unwrap();
        if let Some(super::super::MemoryLimit::Bytes(b)) = config.slice.memory_swap_max {
            assert_eq!(b, 0);
        } else {
            panic!("Expected MemoryLimit::Bytes(0)");
        }
    }

    #[test]
    fn test_device_policy_invalid() {
        let content = "[Slice]\nDevicePolicy=invalid\n";
        let result = parse_slice_from_str(content);
        assert!(result.is_err());
    }

    #[test]
    fn test_memory_pressure_watch_invalid() {
        let content = "[Slice]\nMemoryPressureWatch=invalid\n";
        let result = parse_slice_from_str(content);
        assert!(result.is_err());
    }
}
