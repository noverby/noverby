//! networkctl — query the status of network links managed by systemd-networkd.
//!
//! Usage:
//!   networkctl                  # List all links (same as `list`)
//!   networkctl list             # List all links with status
//!   networkctl status [LINK]    # Show detailed status of a link
//!   networkctl lldp             # Show LLDP neighbors (stub)
//!   networkctl --help           # Show help
//!   networkctl --version        # Show version

use std::collections::HashMap;
use std::fmt;
use std::fs;
use std::net::Ipv4Addr;
use std::path::Path;

fn main() {
    let args: Vec<String> = std::env::args().collect();

    let command = args.get(1).map(|s| s.as_str()).unwrap_or("list");

    match command {
        "--help" | "-h" | "help" => print_help(),
        "--version" => print_version(),
        "list" => cmd_list(),
        "status" => {
            let link_name = args.get(2).map(|s| s.as_str());
            cmd_status(link_name);
        }
        "lldp" => cmd_lldp(),
        "persistent-storage" => {
            let enable = args.get(2).map(|s| s.as_str()).unwrap_or("yes");
            cmd_persistent_storage(enable);
        }
        other => {
            // If it looks like a flag, ignore and list.
            if other.starts_with('-') {
                cmd_list();
            } else {
                eprintln!("Unknown command: {other}");
                eprintln!("Try 'networkctl --help' for more information.");
                std::process::exit(1);
            }
        }
    }
}

fn print_help() {
    println!("networkctl — query the status of network links");
    println!();
    println!("Usage: networkctl [COMMAND] [OPTIONS]");
    println!();
    println!("Commands:");
    println!("  list              List all network links (default)");
    println!("  status [LINK]     Show detailed status of a link or all links");
    println!("  lldp              Show LLDP neighbor information");
    println!("  persistent-storage [BOOL]  Enable/disable persistent storage for networkd");
    println!();
    println!("Options:");
    println!("  -h, --help        Show this help message");
    println!("  --version         Show version information");
}

/// Enable or disable persistent storage for networkd state.
///
/// When enabled, networkd persists link state, LLDP neighbors, etc. to
/// `/var/lib/systemd/network/`. This is called by the
/// `systemd-networkd-persistent-storage.service` unit.
fn cmd_persistent_storage(enable: &str) {
    let enabled = matches!(enable, "yes" | "true" | "1" | "on");

    let state_dir = Path::new("/var/lib/systemd/network");

    if enabled {
        // Create the persistent storage directory
        if let Err(e) = fs::create_dir_all(state_dir) {
            eprintln!("Failed to create {}: {}", state_dir.display(), e);
            std::process::exit(1);
        }
        // Also ensure the runtime state directories exist
        let _ = fs::create_dir_all("/run/systemd/netif/links");
        let _ = fs::create_dir_all("/run/systemd/netif/leases");
        let _ = fs::create_dir_all("/run/systemd/netif/lldp");
    } else {
        // When disabling, we don't remove the directory — just acknowledge
        // the request. Real systemd doesn't remove it either.
    }
}

fn print_version() {
    println!("networkctl (systemd-rs) 0.1.0");
}

// ---------------------------------------------------------------------------
// Link enumeration via /sys/class/net and /proc
// ---------------------------------------------------------------------------

/// Basic information about a network link read from sysfs/procfs.
struct LinkEntry {
    index: u32,
    name: String,
    mac: String,
    mtu: u32,
    flags: u32,
    operstate: String,
    link_type: String,
    addresses: Vec<String>,
    admin_state: String,
    config_file: Option<String>,
    dns_servers: Vec<String>,
    search_domains: Vec<String>,
    lease_address: Option<String>,
    lease_gateway: Option<String>,
    lease_server: Option<String>,
    lease_lifetime: Option<String>,
}

impl LinkEntry {
    fn is_up(&self) -> bool {
        (self.flags & 0x1) != 0
    }

    fn is_loopback(&self) -> bool {
        (self.flags & 0x8) != 0
    }

    fn state_icon(&self) -> &'static str {
        if self.is_loopback() {
            "●"
        } else if self.operstate == "up" || self.operstate == "unknown" {
            "●"
        } else if self.operstate == "dormant" {
            "○"
        } else {
            "○"
        }
    }

    fn setup_state(&self) -> &str {
        &self.admin_state
    }
}

/// Read all network links from /sys/class/net.
fn enumerate_links() -> Vec<LinkEntry> {
    let net_dir = Path::new("/sys/class/net");
    let mut links = Vec::new();

    let entries = match fs::read_dir(net_dir) {
        Ok(e) => e,
        Err(_) => return links,
    };

    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        let dev_dir = net_dir.join(&name);

        let index = read_sysfs_u32(&dev_dir.join("ifindex")).unwrap_or(0);
        let mtu = read_sysfs_u32(&dev_dir.join("mtu")).unwrap_or(0);
        let flags = read_sysfs_u32(&dev_dir.join("flags")).unwrap_or(0);
        let mac = read_sysfs_string(&dev_dir.join("address")).unwrap_or_default();
        let operstate =
            read_sysfs_string(&dev_dir.join("operstate")).unwrap_or_else(|| "unknown".into());

        let link_type = if (flags & 0x8) != 0 {
            "loopback".to_string()
        } else {
            let type_id = read_sysfs_u32(&dev_dir.join("type")).unwrap_or(0);
            match type_id {
                1 => "ether".to_string(),
                772 => "loopback".to_string(),
                801 | 802 => "wlan".to_string(),
                _ => format!("type-{type_id}"),
            }
        };

        // Read addresses from /proc/net/if_inet6 and /proc/net/fib_trie (best effort).
        let addresses = get_addresses_for_link(&name, index);

        // Read networkd state file if available.
        let state_file = format!("/run/systemd/netif/links/{index}");
        let state = read_state_file(&state_file);

        let admin_state = state
            .get("ADMIN_STATE")
            .cloned()
            .unwrap_or_else(|| "unmanaged".into());
        let config_file = state.get("NETWORK_FILE").cloned();
        let dns_servers: Vec<String> = state
            .iter()
            .filter(|(k, _)| k == &"DNS")
            .map(|(_, v)| v.clone())
            .collect();
        let search_domains: Vec<String> = state
            .iter()
            .filter(|(k, _)| k == &"DOMAINS")
            .map(|(_, v)| v.clone())
            .collect();

        // Read lease file if available.
        let lease_file = format!("/run/systemd/netif/leases/{index}");
        let lease = read_state_file(&lease_file);
        let lease_address = lease.get("ADDRESS").cloned();
        let lease_gateway = lease.get("ROUTER").cloned();
        let lease_server = lease.get("SERVER_ADDRESS").cloned();
        let lease_lifetime = lease.get("LIFETIME").cloned();

        links.push(LinkEntry {
            index,
            name,
            mac,
            mtu,
            flags,
            operstate,
            link_type,
            addresses,
            admin_state,
            config_file,
            dns_servers,
            search_domains,
            lease_address,
            lease_gateway,
            lease_server,
            lease_lifetime,
        });
    }

    links.sort_by_key(|l| l.index);
    links
}

/// Get IPv4 addresses for a specific interface from /proc/net/fib_trie.
/// Falls back to an empty list if parsing fails.
fn get_addresses_for_link(ifname: &str, _ifindex: u32) -> Vec<String> {
    // Simple approach: parse ip addr show output equivalent from /proc.
    // For simplicity, read from the networkd state or lease files instead.
    let mut addrs = Vec::new();

    // Try to get from /sys/class/net/<if>/
    // Actually, addresses aren't in sysfs. Let's read from the state file.
    let state_file = format!("/run/systemd/netif/leases/{_ifindex}");
    if let Some(content) = read_file_string(&state_file) {
        for line in content.lines() {
            if let Some(addr) = line.strip_prefix("ADDRESS=") {
                addrs.push(addr.to_string());
            }
        }
    }

    addrs
}

// ---------------------------------------------------------------------------
// Commands
// ---------------------------------------------------------------------------

fn cmd_list() {
    let links = enumerate_links();

    // Header.
    println!(
        "{:>3} {:16} {:10} {:12} {:16}",
        "IDX", "LINK", "TYPE", "OPERATIONAL", "SETUP"
    );

    for link in &links {
        let oper_display = colorize_oper_state(&link.operstate);
        let setup_display = link.setup_state();

        println!(
            "{:>3} {:16} {:10} {:12} {:16}",
            link.index, link.name, link.link_type, oper_display, setup_display,
        );
    }

    println!();
    println!("{} links listed.", links.len());
}

fn cmd_status(link_filter: Option<&str>) {
    let links = enumerate_links();

    let filtered: Vec<&LinkEntry> = match link_filter {
        Some(name) => links.iter().filter(|l| l.name == name).collect(),
        None => links.iter().collect(),
    };

    if filtered.is_empty() {
        if let Some(name) = link_filter {
            eprintln!("No link found: {name}");
            std::process::exit(1);
        }
        println!("No links found.");
        return;
    }

    for (i, link) in filtered.iter().enumerate() {
        if i > 0 {
            println!();
        }

        println!("{} {} - {}", link.state_icon(), link.index, link.name);
        println!(
            "             Link File: {}",
            link.config_file.as_deref().unwrap_or("n/a")
        );
        println!(
            "          Network File: {}",
            link.config_file.as_deref().unwrap_or("n/a")
        );
        println!("                  Type: {}", link.link_type);
        println!(
            "                 State: {} ({})",
            link.operstate,
            link.setup_state()
        );
        println!("    Hardware Address: {}", link.mac);
        println!("                   MTU: {} bytes", link.mtu);

        // Show addresses.
        if let Some(ref addr) = link.lease_address {
            println!("               Address: {}", addr);
        }
        for addr in &link.addresses {
            println!("               Address: {}", addr);
        }

        // Show gateway.
        if let Some(ref gw) = link.lease_gateway {
            println!("               Gateway: {}", gw);
        }

        // Show DNS.
        for dns in &link.dns_servers {
            println!("                   DNS: {}", dns);
        }

        // Show domains.
        for domain in &link.search_domains {
            println!("        Search Domains: {}", domain);
        }

        // Show DHCP lease info.
        if let Some(ref server) = link.lease_server {
            println!("           DHCP Server: {}", server);
        }
        if let Some(ref lifetime) = link.lease_lifetime {
            println!("        Lease Lifetime: {}s", lifetime);
        }
    }
}

fn cmd_lldp() {
    println!(
        "{:>3} {:16} {:20} {:20} {:16}",
        "IDX", "LINK", "CHASSIS ID", "SYSTEM NAME", "PORT ID"
    );
    println!();
    println!("0 neighbors listed.");
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn read_sysfs_string(path: &Path) -> Option<String> {
    fs::read_to_string(path).ok().map(|s| s.trim().to_string())
}

fn read_sysfs_u32(path: &Path) -> Option<u32> {
    let s = read_sysfs_string(path)?;
    // Handle hex (0x...) or decimal.
    if let Some(hex) = s.strip_prefix("0x") {
        u32::from_str_radix(hex, 16).ok()
    } else {
        s.parse().ok()
    }
}

fn read_file_string(path: &str) -> Option<String> {
    fs::read_to_string(path).ok()
}

/// Read a key=value state file into a list of (key, value) pairs.
/// Allows duplicate keys (e.g. multiple DNS= lines).
fn read_state_file(path: &str) -> Vec<(String, String)> {
    let content = match read_file_string(path) {
        Some(c) => c,
        None => return Vec::new(),
    };

    content
        .lines()
        .filter(|line| !line.starts_with('#') && !line.is_empty())
        .filter_map(|line| {
            let (key, value) = line.split_once('=')?;
            Some((key.trim().to_string(), value.trim().to_string()))
        })
        .collect()
}

/// Extension trait to look up a key in a Vec<(String, String)>.
trait StateFileLookup {
    fn get(&self, key: &str) -> Option<&String>;
}

impl StateFileLookup for Vec<(String, String)> {
    fn get(&self, key: &str) -> Option<&String> {
        self.iter().find(|(k, _)| k == key).map(|(_, v)| v)
    }
}

fn colorize_oper_state(state: &str) -> &str {
    // In a real terminal we'd use ANSI colors, but for simplicity
    // we just return the state string as-is.
    state
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_read_state_file_empty() {
        let pairs = read_state_file("/tmp/definitely_does_not_exist_networkctl_test");
        assert!(pairs.is_empty());
    }

    #[test]
    fn test_read_state_file_with_content() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test-state");
        fs::write(
            &path,
            "# comment\nADMIN_STATE=configured\nOPER_STATE=routable\nDNS=8.8.8.8\nDNS=1.1.1.1\n",
        )
        .unwrap();

        let pairs = read_state_file(path.to_str().unwrap());
        assert_eq!(pairs.len(), 4);
        assert_eq!(pairs.get("ADMIN_STATE"), Some(&"configured".to_string()));
        assert_eq!(pairs.get("OPER_STATE"), Some(&"routable".to_string()));

        // Count DNS entries.
        let dns_count = pairs.iter().filter(|(k, _)| k == "DNS").count();
        assert_eq!(dns_count, 2);
    }

    #[test]
    fn test_state_file_lookup() {
        let pairs: Vec<(String, String)> = vec![
            ("KEY1".into(), "val1".into()),
            ("KEY2".into(), "val2".into()),
        ];
        assert_eq!(pairs.get("KEY1"), Some(&"val1".to_string()));
        assert_eq!(pairs.get("KEY2"), Some(&"val2".to_string()));
        assert_eq!(pairs.get("KEY3"), None);
    }

    #[test]
    fn test_read_sysfs_u32_hex() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("flags");
        fs::write(&path, "0x1003\n").unwrap();
        assert_eq!(read_sysfs_u32(&path), Some(0x1003));
    }

    #[test]
    fn test_read_sysfs_u32_decimal() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("mtu");
        fs::write(&path, "1500\n").unwrap();
        assert_eq!(read_sysfs_u32(&path), Some(1500));
    }

    #[test]
    fn test_read_sysfs_string() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("operstate");
        fs::write(&path, "up\n").unwrap();
        assert_eq!(read_sysfs_string(&path), Some("up".to_string()));
    }

    #[test]
    fn test_link_entry_flags() {
        let link = LinkEntry {
            index: 1,
            name: "lo".into(),
            mac: "00:00:00:00:00:00".into(),
            mtu: 65536,
            flags: 0x1 | 0x8, // UP | LOOPBACK
            operstate: "unknown".into(),
            link_type: "loopback".into(),
            addresses: Vec::new(),
            admin_state: "unmanaged".into(),
            config_file: None,
            dns_servers: Vec::new(),
            search_domains: Vec::new(),
            lease_address: None,
            lease_gateway: None,
            lease_server: None,
            lease_lifetime: None,
        };

        assert!(link.is_up());
        assert!(link.is_loopback());
        assert_eq!(link.state_icon(), "●");
    }

    #[test]
    fn test_link_entry_ether() {
        let link = LinkEntry {
            index: 2,
            name: "eth0".into(),
            mac: "52:54:00:12:34:56".into(),
            mtu: 1500,
            flags: 0x1 | 0x40, // UP | RUNNING
            operstate: "up".into(),
            link_type: "ether".into(),
            addresses: vec!["192.168.1.100/24".into()],
            admin_state: "configured".into(),
            config_file: Some("/etc/systemd/network/10-eth.network".into()),
            dns_servers: vec!["8.8.8.8".into()],
            search_domains: vec!["example.com".into()],
            lease_address: Some("192.168.1.100".into()),
            lease_gateway: Some("192.168.1.1".into()),
            lease_server: Some("192.168.1.1".into()),
            lease_lifetime: Some("3600".into()),
        };

        assert!(link.is_up());
        assert!(!link.is_loopback());
        assert_eq!(link.state_icon(), "●");
        assert_eq!(link.setup_state(), "configured");
    }

    #[test]
    fn test_link_entry_down() {
        let link = LinkEntry {
            index: 3,
            name: "wlan0".into(),
            mac: "aa:bb:cc:dd:ee:ff".into(),
            mtu: 1500,
            flags: 0,
            operstate: "down".into(),
            link_type: "wlan".into(),
            addresses: Vec::new(),
            admin_state: "unmanaged".into(),
            config_file: None,
            dns_servers: Vec::new(),
            search_domains: Vec::new(),
            lease_address: None,
            lease_gateway: None,
            lease_server: None,
            lease_lifetime: None,
        };

        assert!(!link.is_up());
        assert!(!link.is_loopback());
        assert_eq!(link.state_icon(), "○");
    }

    #[test]
    fn test_colorize_oper_state() {
        assert_eq!(colorize_oper_state("up"), "up");
        assert_eq!(colorize_oper_state("down"), "down");
        assert_eq!(colorize_oper_state("unknown"), "unknown");
    }
}
