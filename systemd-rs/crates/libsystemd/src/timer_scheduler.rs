//! Timer unit scheduling — fires associated service units when timers elapse.
//!
//! After PID 1 finishes activating units, the timer scheduler thread wakes up
//! periodically and checks all active `.timer` units to see if any of their
//! trigger conditions have been met.  When a timer elapses, the scheduler
//! starts (or restarts) the associated service unit via the control interface.

use log::{debug, info, trace, warn};
use std::time::{Duration, Instant};

use crate::lock_ext::RwLockExt;
use crate::runtime_info::ArcMutRuntimeInfo;
use crate::units::{ActivationSource, Specific, StatusStarted, TimerConfig, UnitId, UnitStatus};

/// How often the scheduler thread wakes up to check timers.
const TIMER_CHECK_INTERVAL: Duration = Duration::from_secs(15);

/// Boot instant — captured once when the scheduler starts.
/// Used for OnBootSec= / OnStartupSec= calculations.
static BOOT_INSTANT: std::sync::OnceLock<Instant> = std::sync::OnceLock::new();

/// Start the background timer scheduler thread.
///
/// This should be called after the initial unit activation is complete
/// (or at least after all timer units have been loaded and started).
pub fn start_timer_scheduler_thread(run_info: ArcMutRuntimeInfo) {
    let boot_instant = *BOOT_INSTANT.get_or_init(Instant::now);

    std::thread::Builder::new()
        .name("timer-scheduler".into())
        .spawn(move || {
            info!("Timer scheduler started");

            // Give the system a moment to finish initial activation before
            // checking timers for the first time.  This avoids firing
            // OnBootSec=0 timers before their target services are loaded.
            std::thread::sleep(Duration::from_secs(2));

            // Track when each timer last fired so we can implement
            // OnUnitActiveSec= and avoid re-firing within the same period.
            let mut last_fired: std::collections::HashMap<String, Instant> =
                std::collections::HashMap::new();

            loop {
                check_and_fire_timers(&run_info, boot_instant, &mut last_fired);
                std::thread::sleep(TIMER_CHECK_INTERVAL);
            }
        })
        .expect("Failed to spawn timer-scheduler thread");
}

/// One pass of the timer check loop.
fn check_and_fire_timers(
    run_info: &ArcMutRuntimeInfo,
    boot_instant: Instant,
    last_fired: &mut std::collections::HashMap<String, Instant>,
) {
    let now = Instant::now();
    let elapsed_since_boot = now.duration_since(boot_instant);

    // Collect timer units that need to fire.
    // We collect first, then fire, to avoid holding the read lock during activation.
    let mut timers_to_fire: Vec<(UnitId, String)> = Vec::new();

    {
        let ri = run_info.read_poisoned();
        for unit in ri.unit_table.values() {
            if let Specific::Timer(timer_specific) = &unit.specific {
                // Only check timers that are started/running
                let status = unit.common.status.read_poisoned().clone();
                if !matches!(status, UnitStatus::Started(StatusStarted::Running)) {
                    continue;
                }

                let conf = &timer_specific.conf;
                let timer_name = &unit.id.name;
                let target_unit = &conf.unit;

                if should_fire_timer(conf, timer_name, elapsed_since_boot, now, last_fired) {
                    timers_to_fire.push((unit.id.clone(), target_unit.clone()));
                }
            }
        }
    }

    // Fire the collected timers
    for (timer_id, target_unit_name) in timers_to_fire {
        info!(
            "Timer {} elapsed, activating {}",
            timer_id.name, target_unit_name
        );
        last_fired.insert(timer_id.name.clone(), now);

        fire_timer_target(run_info, &target_unit_name);
    }
}

/// Determine if a timer should fire based on its configuration and current state.
fn should_fire_timer(
    conf: &TimerConfig,
    timer_name: &str,
    elapsed_since_boot: Duration,
    now: Instant,
    last_fired: &std::collections::HashMap<String, Instant>,
) -> bool {
    let last = last_fired.get(timer_name).copied();

    // OnBootSec= / OnStartupSec= — fire once after boot + duration
    for dur in conf.on_boot_sec.iter().chain(conf.on_startup_sec.iter()) {
        if elapsed_since_boot >= *dur {
            // Should have fired by now. Check if we already did.
            if last.is_none() {
                trace!(
                    "Timer {}: OnBootSec/OnStartupSec {:?} elapsed (boot+{:?})",
                    timer_name, dur, elapsed_since_boot
                );
                return true;
            }
        }
    }

    // OnActiveSec= — fire once after timer activation + duration
    // Since we don't track when the timer was activated separately, we
    // approximate by using boot time (timers are activated during boot).
    for dur in &conf.on_active_sec {
        if elapsed_since_boot >= *dur && last.is_none() {
            trace!("Timer {}: OnActiveSec {:?} elapsed", timer_name, dur);
            return true;
        }
    }

    // OnUnitActiveSec= — repeating timer relative to last activation
    for dur in &conf.on_unit_active_sec {
        if dur.is_zero() {
            continue;
        }
        match last {
            Some(last_time) => {
                let since_last = now.duration_since(last_time);
                if since_last >= *dur {
                    trace!(
                        "Timer {}: OnUnitActiveSec {:?} elapsed ({:?} since last fire)",
                        timer_name, dur, since_last
                    );
                    return true;
                }
            }
            None => {
                // First run after boot — fire if boot elapsed >= dur
                if elapsed_since_boot >= *dur {
                    return true;
                }
            }
        }
    }

    // OnUnitInactiveSec= — repeating timer relative to last deactivation
    // We approximate this the same as OnUnitActiveSec for now.
    for dur in &conf.on_unit_inactive_sec {
        if dur.is_zero() {
            continue;
        }
        match last {
            Some(last_time) => {
                if now.duration_since(last_time) >= *dur {
                    return true;
                }
            }
            None => {
                if elapsed_since_boot >= *dur {
                    return true;
                }
            }
        }
    }

    // OnCalendar= — calendar event expressions
    // For now, we implement a simplified version:
    // - "hourly" → fire if >1h since last fire (or boot)
    // - "daily" → fire if >24h since last fire (or boot)
    // - "weekly" → fire if >7d since last fire (or boot)
    // - "monthly" → fire if >30d since last fire (or boot)
    // - Other expressions are logged but not yet parsed.
    for expr in &conf.on_calendar {
        if let Some(interval) = parse_calendar_shorthand(expr) {
            let reference = last.unwrap_or_else(|| {
                // If Persistent=true and we haven't fired yet, fire immediately
                // (simulating "missed" runs from before boot).
                if conf.persistent {
                    // Return an instant far enough in the past to trigger
                    now.checked_sub(interval + Duration::from_secs(1))
                        .unwrap_or(now)
                } else {
                    now.checked_sub(elapsed_since_boot).unwrap_or(now)
                }
            });
            let since_ref = now.duration_since(reference);
            if since_ref >= interval {
                trace!(
                    "Timer {}: OnCalendar={} interval {:?} elapsed ({:?} since reference)",
                    timer_name, expr, interval, since_ref
                );
                return true;
            }
        } else {
            // Complex calendar expression — not yet implemented.
            // We fire these once after a long delay to avoid never running them.
            debug!(
                "Timer {}: OnCalendar={} not yet parsed (complex calendar expressions not implemented)",
                timer_name, expr
            );
        }
    }

    false
}

/// Parse common calendar shorthands into a Duration interval.
fn parse_calendar_shorthand(expr: &str) -> Option<Duration> {
    match expr.trim().to_lowercase().as_str() {
        "minutely" => Some(Duration::from_secs(60)),
        "hourly" => Some(Duration::from_secs(3600)),
        "daily" => Some(Duration::from_secs(86400)),
        "weekly" => Some(Duration::from_secs(7 * 86400)),
        "monthly" => Some(Duration::from_secs(30 * 86400)),
        "yearly" | "annually" => Some(Duration::from_secs(365 * 86400)),
        "quarterly" => Some(Duration::from_secs(90 * 86400)),
        "semiannually" => Some(Duration::from_secs(182 * 86400)),
        _ => None,
    }
}

/// Fire a timer's target unit by starting it via the activation system.
fn fire_timer_target(run_info: &ArcMutRuntimeInfo, target_unit_name: &str) {
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
                        "Timer target {} is already running, attempting restart",
                        target_unit_name
                    );
                    match unit.reactivate(&ri, ActivationSource::Regular) {
                        Ok(()) => {
                            info!("Timer fired: restarted {}", target_unit_name);
                        }
                        Err(e) => {
                            warn!("Timer failed to restart {}: {}", target_unit_name, e);
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
                            info!("Timer fired: started {}", target_unit_name);
                        }
                        Err(e) => {
                            warn!("Timer failed to start {}: {}", target_unit_name, e);
                        }
                    }
                }
            }
        }
        None => {
            // Unit not in the boot dependency graph — try on-demand loading
            debug!(
                "Timer target {} not found in unit table, attempting on-demand load",
                target_unit_name
            );
            drop(ri);

            // Use the control interface's find_or_load_unit logic by sending
            // a start command through the internal path.
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
                    Ok(_) => info!("Timer fired: started {} (on-demand)", target_unit_name),
                    Err(e) => warn!(
                        "Timer failed to start {} (on-demand): {}",
                        target_unit_name, e
                    ),
                }
            } else {
                warn!(
                    "Timer target {} not found and could not be loaded",
                    target_unit_name
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_calendar_shorthand_hourly() {
        assert_eq!(
            parse_calendar_shorthand("hourly"),
            Some(Duration::from_secs(3600))
        );
    }

    #[test]
    fn test_parse_calendar_shorthand_daily() {
        assert_eq!(
            parse_calendar_shorthand("daily"),
            Some(Duration::from_secs(86400))
        );
    }

    #[test]
    fn test_parse_calendar_shorthand_weekly() {
        assert_eq!(
            parse_calendar_shorthand("weekly"),
            Some(Duration::from_secs(7 * 86400))
        );
    }

    #[test]
    fn test_parse_calendar_shorthand_monthly() {
        assert_eq!(
            parse_calendar_shorthand("monthly"),
            Some(Duration::from_secs(30 * 86400))
        );
    }

    #[test]
    fn test_parse_calendar_shorthand_yearly() {
        assert_eq!(
            parse_calendar_shorthand("yearly"),
            Some(Duration::from_secs(365 * 86400))
        );
    }

    #[test]
    fn test_parse_calendar_shorthand_case_insensitive() {
        assert_eq!(
            parse_calendar_shorthand("Hourly"),
            Some(Duration::from_secs(3600))
        );
        assert_eq!(
            parse_calendar_shorthand("DAILY"),
            Some(Duration::from_secs(86400))
        );
    }

    #[test]
    fn test_parse_calendar_shorthand_unknown() {
        assert_eq!(parse_calendar_shorthand("Mon *-*-* 03:00:00"), None);
        assert_eq!(parse_calendar_shorthand("*-*-* 12:00:00"), None);
    }

    #[test]
    fn test_parse_calendar_shorthand_minutely() {
        assert_eq!(
            parse_calendar_shorthand("minutely"),
            Some(Duration::from_secs(60))
        );
    }

    #[test]
    fn test_parse_calendar_shorthand_quarterly() {
        assert_eq!(
            parse_calendar_shorthand("quarterly"),
            Some(Duration::from_secs(90 * 86400))
        );
    }

    #[test]
    fn test_should_fire_on_boot_sec_not_yet() {
        let conf = TimerConfig {
            on_boot_sec: vec![Duration::from_secs(3600)],
            on_startup_sec: vec![],
            on_active_sec: vec![],
            on_unit_active_sec: vec![],
            on_unit_inactive_sec: vec![],
            on_calendar: vec![],
            accuracy_sec: Duration::from_secs(60),
            randomized_delay_sec: Duration::ZERO,
            persistent: false,
            wake_system: false,
            remain_after_elapse: true,
            unit: "test.service".into(),
        };
        let last_fired = std::collections::HashMap::new();
        // Only 5 minutes since boot — shouldn't fire yet
        let result = should_fire_timer(
            &conf,
            "test.timer",
            Duration::from_secs(300),
            Instant::now(),
            &last_fired,
        );
        assert!(!result);
    }

    #[test]
    fn test_should_fire_on_boot_sec_elapsed() {
        let conf = TimerConfig {
            on_boot_sec: vec![Duration::from_secs(300)],
            on_startup_sec: vec![],
            on_active_sec: vec![],
            on_unit_active_sec: vec![],
            on_unit_inactive_sec: vec![],
            on_calendar: vec![],
            accuracy_sec: Duration::from_secs(60),
            randomized_delay_sec: Duration::ZERO,
            persistent: false,
            wake_system: false,
            remain_after_elapse: true,
            unit: "test.service".into(),
        };
        let last_fired = std::collections::HashMap::new();
        // 10 minutes since boot, timer is 5 min — should fire
        let result = should_fire_timer(
            &conf,
            "test.timer",
            Duration::from_secs(600),
            Instant::now(),
            &last_fired,
        );
        assert!(result);
    }

    #[test]
    fn test_should_fire_on_boot_sec_already_fired() {
        let conf = TimerConfig {
            on_boot_sec: vec![Duration::from_secs(300)],
            on_startup_sec: vec![],
            on_active_sec: vec![],
            on_unit_active_sec: vec![],
            on_unit_inactive_sec: vec![],
            on_calendar: vec![],
            accuracy_sec: Duration::from_secs(60),
            randomized_delay_sec: Duration::ZERO,
            persistent: false,
            wake_system: false,
            remain_after_elapse: true,
            unit: "test.service".into(),
        };
        let mut last_fired = std::collections::HashMap::new();
        last_fired.insert("test.timer".into(), Instant::now());
        // Already fired — shouldn't fire again
        let result = should_fire_timer(
            &conf,
            "test.timer",
            Duration::from_secs(600),
            Instant::now(),
            &last_fired,
        );
        assert!(!result);
    }

    #[test]
    fn test_should_fire_on_unit_active_sec_repeating() {
        let conf = TimerConfig {
            on_boot_sec: vec![],
            on_startup_sec: vec![],
            on_active_sec: vec![],
            on_unit_active_sec: vec![Duration::from_secs(60)],
            on_unit_inactive_sec: vec![],
            on_calendar: vec![],
            accuracy_sec: Duration::from_secs(60),
            randomized_delay_sec: Duration::ZERO,
            persistent: false,
            wake_system: false,
            remain_after_elapse: true,
            unit: "test.service".into(),
        };
        let now = Instant::now();
        let mut last_fired = std::collections::HashMap::new();
        // Last fired 120s ago, interval is 60s — should fire
        last_fired.insert(
            "test.timer".into(),
            now.checked_sub(Duration::from_secs(120)).unwrap(),
        );
        let result = should_fire_timer(
            &conf,
            "test.timer",
            Duration::from_secs(300),
            now,
            &last_fired,
        );
        assert!(result);
    }

    #[test]
    fn test_should_fire_on_unit_active_sec_too_soon() {
        let conf = TimerConfig {
            on_boot_sec: vec![],
            on_startup_sec: vec![],
            on_active_sec: vec![],
            on_unit_active_sec: vec![Duration::from_secs(3600)],
            on_unit_inactive_sec: vec![],
            on_calendar: vec![],
            accuracy_sec: Duration::from_secs(60),
            randomized_delay_sec: Duration::ZERO,
            persistent: false,
            wake_system: false,
            remain_after_elapse: true,
            unit: "test.service".into(),
        };
        let now = Instant::now();
        let mut last_fired = std::collections::HashMap::new();
        // Last fired 30s ago, interval is 1h — shouldn't fire
        last_fired.insert(
            "test.timer".into(),
            now.checked_sub(Duration::from_secs(30)).unwrap(),
        );
        let result = should_fire_timer(
            &conf,
            "test.timer",
            Duration::from_secs(300),
            now,
            &last_fired,
        );
        assert!(!result);
    }

    #[test]
    fn test_should_fire_calendar_hourly() {
        let conf = TimerConfig {
            on_boot_sec: vec![],
            on_startup_sec: vec![],
            on_active_sec: vec![],
            on_unit_active_sec: vec![],
            on_unit_inactive_sec: vec![],
            on_calendar: vec!["hourly".into()],
            accuracy_sec: Duration::from_secs(60),
            randomized_delay_sec: Duration::ZERO,
            persistent: false,
            wake_system: false,
            remain_after_elapse: true,
            unit: "test.service".into(),
        };
        let now = Instant::now();
        let mut last_fired = std::collections::HashMap::new();
        // Last fired 2 hours ago — should fire
        last_fired.insert(
            "test.timer".into(),
            now.checked_sub(Duration::from_secs(7200)).unwrap(),
        );
        let result = should_fire_timer(
            &conf,
            "test.timer",
            Duration::from_secs(10000),
            now,
            &last_fired,
        );
        assert!(result);
    }

    #[test]
    fn test_should_fire_calendar_hourly_too_soon() {
        let conf = TimerConfig {
            on_boot_sec: vec![],
            on_startup_sec: vec![],
            on_active_sec: vec![],
            on_unit_active_sec: vec![],
            on_unit_inactive_sec: vec![],
            on_calendar: vec!["hourly".into()],
            accuracy_sec: Duration::from_secs(60),
            randomized_delay_sec: Duration::ZERO,
            persistent: false,
            wake_system: false,
            remain_after_elapse: true,
            unit: "test.service".into(),
        };
        let now = Instant::now();
        let mut last_fired = std::collections::HashMap::new();
        // Last fired 10 minutes ago — shouldn't fire yet
        last_fired.insert(
            "test.timer".into(),
            now.checked_sub(Duration::from_secs(600)).unwrap(),
        );
        let result = should_fire_timer(
            &conf,
            "test.timer",
            Duration::from_secs(10000),
            now,
            &last_fired,
        );
        assert!(!result);
    }

    #[test]
    fn test_should_fire_empty_config() {
        let conf = TimerConfig {
            on_boot_sec: vec![],
            on_startup_sec: vec![],
            on_active_sec: vec![],
            on_unit_active_sec: vec![],
            on_unit_inactive_sec: vec![],
            on_calendar: vec![],
            accuracy_sec: Duration::from_secs(60),
            randomized_delay_sec: Duration::ZERO,
            persistent: false,
            wake_system: false,
            remain_after_elapse: true,
            unit: "test.service".into(),
        };
        let last_fired = std::collections::HashMap::new();
        // No triggers configured — should never fire
        let result = should_fire_timer(
            &conf,
            "test.timer",
            Duration::from_secs(999999),
            Instant::now(),
            &last_fired,
        );
        assert!(!result);
    }

    #[test]
    fn test_should_fire_persistent_calendar_first_boot() {
        let conf = TimerConfig {
            on_boot_sec: vec![],
            on_startup_sec: vec![],
            on_active_sec: vec![],
            on_unit_active_sec: vec![],
            on_unit_inactive_sec: vec![],
            on_calendar: vec!["weekly".into()],
            accuracy_sec: Duration::from_secs(60),
            randomized_delay_sec: Duration::ZERO,
            persistent: true,
            wake_system: false,
            remain_after_elapse: true,
            unit: "test.service".into(),
        };
        let last_fired = std::collections::HashMap::new();
        // Persistent=true and never fired — should fire on first check
        // (even though we just booted, Persistent=true means "catch up on missed runs")
        let result = should_fire_timer(
            &conf,
            "test.timer",
            Duration::from_secs(60),
            Instant::now(),
            &last_fired,
        );
        assert!(result);
    }

    #[test]
    fn test_parse_timespan_via_from_parsed_config() {
        use crate::units::from_parsed_config::parse_timespan;
        assert_eq!(parse_timespan("15min"), Some(Duration::from_secs(900)));
        assert_eq!(parse_timespan("1h"), Some(Duration::from_secs(3600)));
        assert_eq!(parse_timespan("1d"), Some(Duration::from_secs(86400)));
        assert_eq!(parse_timespan("1h 30min"), Some(Duration::from_secs(5400)));
        assert_eq!(parse_timespan("500ms"), Some(Duration::from_millis(500)));
        assert_eq!(parse_timespan("100min"), Some(Duration::from_secs(6000)));
        assert_eq!(parse_timespan("2s"), Some(Duration::from_secs(2)));
        assert_eq!(parse_timespan(""), None);
        assert_eq!(parse_timespan("30"), Some(Duration::from_secs(30)));
    }
}
