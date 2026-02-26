//! Parser for systemd `.network` and `.link` configuration files.
//!
//! Supports the INI-style format used by systemd-networkd with sections:
//! - `[Match]`   — match interfaces by name, MAC, driver, etc.
//! - `[Network]` — general network settings (DHCP, DNS, domains, etc.)
//! - `[Address]` — static address configuration
//! - `[Route]`   — static route configuration
//! - `[DHCPv4]`  — DHCPv4 client options
//! - `[Link]`    — link-level settings (MTU, etc.)
//!
//! Also parses `.link` files (systemd.link(5)) for interface naming and
//! link-level configuration:
//! - `[Match]`   — match by MAC, OriginalName, Path, Driver, Type, etc.
//! - `[Link]`    — Name, NamePolicy, MACAddressPolicy, MTUBytes, etc.
//!
//! Reference: systemd.network(5), systemd.link(5)

use std::collections::HashMap;
use std::fmt;
use std::fs;
use std::net::{IpAddr, Ipv4Addr};
use std::path::{Path, PathBuf};

// ---------------------------------------------------------------------------
// Top-level parsed network file
// ---------------------------------------------------------------------------

/// A parsed `.network` file.
#[derive(Debug, Clone)]
pub struct NetworkConfig {
    /// Original file path (for diagnostics).
    pub path: PathBuf,

    /// `[Match]` section — determines which links this config applies to.
    pub match_section: MatchSection,

    /// `[Network]` section — high-level network settings.
    pub network_section: NetworkSection,

    /// `[Address]` sections — static addresses (there may be several).
    pub addresses: Vec<AddressSection>,

    /// `[Route]` sections — static routes (there may be several).
    pub routes: Vec<RouteSection>,

    /// `[DHCPv4]` section — DHCPv4 client tunables.
    pub dhcpv4: DhcpV4Section,

    /// `[Link]` section — link-layer tunables.
    pub link: LinkSection,
}

// ---------------------------------------------------------------------------
// [Match]
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default)]
pub struct MatchSection {
    /// `Name=` — glob patterns for interface names (e.g. `en*`, `eth0`).
    pub names: Vec<String>,

    /// `MACAddress=` — match by hardware address.
    pub mac_addresses: Vec<String>,

    /// `Driver=` — match by kernel driver.
    pub drivers: Vec<String>,

    /// `Type=` — match by interface type (e.g. `ether`, `wlan`).
    pub types: Vec<String>,

    /// `Path=` — match by sysfs device path glob.
    pub paths: Vec<String>,

    /// `KernelCommandLine=` — match when kernel command line contains string.
    pub kernel_command_line: Vec<String>,

    /// `Virtualization=` — match by virtualization type.
    pub virtualization: Option<String>,

    /// `Host=` — match by hostname.
    pub host: Option<String>,

    /// `Architecture=` — match by CPU architecture.
    pub architecture: Option<String>,
}

impl MatchSection {
    /// Returns `true` if this section matches the given interface.
    pub fn matches_interface(&self, name: &str, mac: Option<&str>, driver: Option<&str>) -> bool {
        // If no match criteria are specified, match everything.
        if self.names.is_empty()
            && self.mac_addresses.is_empty()
            && self.drivers.is_empty()
            && self.types.is_empty()
            && self.paths.is_empty()
        {
            return true;
        }

        // Name matching (supports simple glob: `*` and `?`).
        if !self.names.is_empty() {
            let name_matches = self.names.iter().any(|pattern| glob_match(pattern, name));
            if !name_matches {
                return false;
            }
        }

        // MAC matching.
        if !self.mac_addresses.is_empty() {
            match mac {
                Some(m) => {
                    let mac_matches = self.mac_addresses.iter().any(|a| a.eq_ignore_ascii_case(m));
                    if !mac_matches {
                        return false;
                    }
                }
                None => return false,
            }
        }

        // Driver matching.
        if !self.drivers.is_empty() {
            match driver {
                Some(d) => {
                    if !self.drivers.iter().any(|pat| glob_match(pat, d)) {
                        return false;
                    }
                }
                None => return false,
            }
        }

        true
    }
}

// ---------------------------------------------------------------------------
// [Network]
// ---------------------------------------------------------------------------

/// How to obtain addresses on a link.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DhcpMode {
    #[default]
    No,
    Yes,
    Ipv4,
    Ipv6,
}

impl fmt::Display for DhcpMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::No => write!(f, "no"),
            Self::Yes => write!(f, "yes"),
            Self::Ipv4 => write!(f, "ipv4"),
            Self::Ipv6 => write!(f, "ipv6"),
        }
    }
}

/// How to configure IPv6 link-local addressing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LinkLocalMode {
    No,
    Yes,
    Ipv4,
    #[default]
    Ipv6,
}

/// Whether/how to accept IPv6 router advertisements.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Ipv6AcceptRa {
    No,
    #[default]
    Yes,
}

#[derive(Debug, Clone, Default)]
pub struct NetworkSection {
    /// `DHCP=` — DHCP client mode.
    pub dhcp: DhcpMode,

    /// `DNS=` — static DNS server addresses.
    pub dns: Vec<IpAddr>,

    /// `Domains=` — search domains.
    pub domains: Vec<String>,

    /// `NTP=` — NTP server addresses/hostnames.
    pub ntp: Vec<String>,

    /// `LinkLocalAddressing=`
    pub link_local: LinkLocalMode,

    /// `IPv6AcceptRA=`
    pub ipv6_accept_ra: Ipv6AcceptRa,

    /// `LLDP=`
    pub lldp: bool,

    /// `EmitLLDP=`
    pub emit_lldp: bool,

    /// `MulticastDNS=`
    pub multicast_dns: bool,

    /// `DNSOverTLS=`
    pub dns_over_tls: Option<String>,

    /// `DNSSEC=`
    pub dnssec: Option<String>,

    /// `IPForward=`
    pub ip_forward: Option<String>,

    /// `IPMasquerade=`
    pub ip_masquerade: Option<String>,

    /// `IPv6PrivacyExtensions=`
    pub ipv6_privacy_extensions: Option<String>,

    /// `IPv6StableSecretAddress=` — secret key for RFC 7217 stable privacy addresses.
    /// When set, SLAAC addresses use a hash-based interface identifier instead of
    /// EUI-64, preventing MAC address leakage. The value is a hex-encoded secret
    /// or the special value "machine-id" to derive from `/etc/machine-id`.
    pub ipv6_stable_secret_address: Option<String>,

    /// `IPv6LinkLocalAddressGenerationMode=` — how to generate the link-local address.
    /// Values: "eui64" (default, from MAC), "stable-privacy" (RFC 7217 hash),
    /// "none" (no link-local), "random".
    pub ipv6_ll_addr_gen_mode: Option<String>,

    /// `Bridge=`
    pub bridge: Option<String>,

    /// `Bond=`
    pub bond: Option<String>,

    /// `VLAN=`
    pub vlans: Vec<String>,

    /// `Description=`
    pub description: Option<String>,

    /// Whether to configure the link at all (`BindCarrier=`, `RequiredForOnline=`, etc.)
    pub required_for_online: Option<String>,
}

// ---------------------------------------------------------------------------
// [Address]
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct AddressSection {
    /// `Address=` — IPv4 or IPv6 address in CIDR notation (e.g. `192.168.1.5/24`).
    pub address: String,

    /// `Peer=` — peer address for point-to-point links.
    pub peer: Option<String>,

    /// `Broadcast=` — broadcast address override.
    pub broadcast: Option<String>,

    /// `Label=` — address label.
    pub label: Option<String>,

    /// `PreferredLifetime=` — preferred lifetime for IPv6.
    pub preferred_lifetime: Option<String>,
}

// ---------------------------------------------------------------------------
// [Route]
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct RouteSection {
    /// `Destination=` — route destination in CIDR notation.
    pub destination: Option<String>,

    /// `Gateway=` — gateway address.
    pub gateway: Option<String>,

    /// `GatewayOnLink=` — gateway is directly reachable on the link.
    pub gateway_on_link: bool,

    /// `Source=` — route source address hint.
    pub source: Option<String>,

    /// `Metric=` — route metric / priority.
    pub metric: Option<u32>,

    /// `Scope=` — route scope (`global`, `link`, `host`).
    pub scope: Option<String>,

    /// `Table=` — routing table number or name.
    pub table: Option<String>,

    /// `Type=` — route type (`unicast`, `unreachable`, `blackhole`, etc.).
    pub route_type: Option<String>,
}

// ---------------------------------------------------------------------------
// [DHCPv4]
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct DhcpV4Section {
    /// `UseDNS=`
    pub use_dns: bool,

    /// `UseNTP=`
    pub use_ntp: bool,

    /// `UseDomains=`
    pub use_domains: bool,

    /// `UseRoutes=`
    pub use_routes: bool,

    /// `UseHostname=`
    pub use_hostname: bool,

    /// `UseMTU=`
    pub use_mtu: bool,

    /// `UseTimezone=`
    pub use_timezone: bool,

    /// `SendHostname=`
    pub send_hostname: bool,

    /// `Hostname=` — hostname to send in DHCP requests.
    pub hostname: Option<String>,

    /// `ClientIdentifier=` — `mac` or `duid`.
    pub client_identifier: Option<String>,

    /// `VendorClassIdentifier=`
    pub vendor_class_id: Option<String>,

    /// `RequestBroadcast=`
    pub request_broadcast: bool,

    /// `RouteMetric=`
    pub route_metric: Option<u32>,

    /// `MaxAttempts=`
    pub max_attempts: Option<u32>,

    /// `ListenPort=`
    pub listen_port: Option<u16>,

    /// `CriticalConnection=`
    pub critical_connection: bool,

    /// `RequestOptions=`
    pub request_options: Vec<u8>,

    /// `SendOption=`
    pub send_options: Vec<String>,
}

impl Default for DhcpV4Section {
    fn default() -> Self {
        Self {
            use_dns: true,
            use_ntp: true,
            use_domains: false,
            use_routes: true,
            use_hostname: true,
            use_mtu: true,
            use_timezone: true,
            send_hostname: true,
            hostname: None,
            client_identifier: None,
            vendor_class_id: None,
            request_broadcast: false,
            route_metric: None,
            max_attempts: None,
            listen_port: None,
            critical_connection: false,
            request_options: Vec::new(),
            send_options: Vec::new(),
        }
    }
}

// ---------------------------------------------------------------------------
// [Link]
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default)]
pub struct LinkSection {
    /// `MTUBytes=`
    pub mtu: Option<u32>,

    /// `MACAddress=` — override MAC address.
    pub mac_address: Option<String>,

    /// `ARP=`
    pub arp: Option<bool>,

    /// `Multicast=`
    pub multicast: Option<bool>,

    /// `Unmanaged=` — if true networkd ignores this link.
    pub unmanaged: bool,

    /// `RequiredForOnline=`
    pub required_for_online: Option<String>,

    /// `ActivationPolicy=` — `up`, `always-up`, `manual`, `down`, `always-down`.
    pub activation_policy: Option<String>,
}

// ---------------------------------------------------------------------------
// Parsing
// ---------------------------------------------------------------------------

/// Load all `.network` files from the standard search paths.
///
/// Files are read in lexicographic order; earlier directories take priority
/// over later ones (matching systemd-networkd behaviour):
///
/// 1. `/etc/systemd/network/`
/// 2. `/run/systemd/network/`
/// 3. `/usr/lib/systemd/network/`
/// 4. `/lib/systemd/network/`
///
/// Additionally, package-relative paths are searched (for NixOS).
pub fn load_network_configs() -> Vec<NetworkConfig> {
    let mut search_dirs = vec![
        PathBuf::from("/etc/systemd/network"),
        PathBuf::from("/run/systemd/network"),
        PathBuf::from("/usr/lib/systemd/network"),
        PathBuf::from("/lib/systemd/network"),
    ];

    // Add package-relative paths for NixOS.
    if let Ok(exe) = std::env::current_exe()
        && let Some(parent) = exe.parent()
    {
        // exe is in  <pkg>/bin/ or <pkg>/lib/systemd/
        for ancestor in parent.ancestors().skip(1) {
            let candidate = ancestor.join("lib/systemd/network");
            if candidate.is_dir() && !search_dirs.contains(&candidate) {
                search_dirs.push(candidate);
                break;
            }
        }
    }

    load_network_configs_from(&search_dirs)
}

/// Load `.network` files from the given directories, deduplicating by
/// filename (first occurrence wins).
pub fn load_network_configs_from(dirs: &[PathBuf]) -> Vec<NetworkConfig> {
    let mut seen: HashMap<String, PathBuf> = HashMap::new();
    let mut configs = Vec::new();

    for dir in dirs {
        let entries = match fs::read_dir(dir) {
            Ok(e) => e,
            Err(_) => continue,
        };

        let mut files: Vec<_> = entries
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().is_some_and(|ext| ext == "network"))
            .collect();

        files.sort_by_key(|e| e.file_name());

        for entry in files {
            let name = entry.file_name().to_string_lossy().to_string();
            if seen.contains_key(&name) {
                continue;
            }
            let path = entry.path();
            seen.insert(name, path.clone());

            match parse_network_file(&path) {
                Ok(cfg) => configs.push(cfg),
                Err(e) => {
                    log::warn!("Failed to parse {}: {}", path.display(), e);
                }
            }
        }
    }

    // Sort by filename for deterministic ordering (systemd sorts
    // lexicographically across all directories).
    configs.sort_by(|a, b| {
        let a_name = a.path.file_name().unwrap_or_default();
        let b_name = b.path.file_name().unwrap_or_default();
        a_name.cmp(b_name)
    });

    configs
}

/// Parse a single `.network` file.
pub fn parse_network_file(path: &Path) -> Result<NetworkConfig, String> {
    let content =
        fs::read_to_string(path).map_err(|e| format!("read {}: {}", path.display(), e))?;

    parse_network_content(&content, path)
}

/// Parse `.network` file content (for testing without filesystem).
pub fn parse_network_content(content: &str, path: &Path) -> Result<NetworkConfig, String> {
    let mut cfg = NetworkConfig {
        path: path.to_path_buf(),
        match_section: MatchSection::default(),
        network_section: NetworkSection::default(),
        addresses: Vec::new(),
        routes: Vec::new(),
        dhcpv4: DhcpV4Section::default(),
        link: LinkSection::default(),
    };

    let mut current_section = String::new();
    // Track whether we're accumulating into a new Address/Route section.
    let mut current_address: Option<AddressSection> = None;
    let mut current_route: Option<RouteSection> = None;

    for line in content.lines() {
        let line = line.trim();

        // Skip blank lines and comments.
        if line.is_empty() || line.starts_with('#') || line.starts_with(';') {
            continue;
        }

        // Section header.
        if line.starts_with('[') && line.ends_with(']') {
            // Flush any pending address/route.
            if let Some(addr) = current_address.take() {
                cfg.addresses.push(addr);
            }
            if let Some(route) = current_route.take() {
                cfg.routes.push(route);
            }

            current_section = line[1..line.len() - 1].to_string();

            // Create new accumulator for repeatable sections.
            match current_section.as_str() {
                "Address" => {
                    current_address = Some(AddressSection {
                        address: String::new(),
                        peer: None,
                        broadcast: None,
                        label: None,
                        preferred_lifetime: None,
                    });
                }
                "Route" => {
                    current_route = Some(RouteSection {
                        destination: None,
                        gateway: None,
                        gateway_on_link: false,
                        source: None,
                        metric: None,
                        scope: None,
                        table: None,
                        route_type: None,
                    });
                }
                _ => {}
            }

            continue;
        }

        // Key=Value pair.
        let (key, value) = match line.split_once('=') {
            Some((k, v)) => (k.trim(), v.trim()),
            None => continue,
        };

        match current_section.as_str() {
            "Match" => parse_match_entry(key, value, &mut cfg.match_section),
            "Network" => parse_network_entry(key, value, &mut cfg.network_section),
            "Address" => {
                if let Some(ref mut addr) = current_address {
                    parse_address_entry(key, value, addr);
                }
            }
            "Route" => {
                if let Some(ref mut route) = current_route {
                    parse_route_entry(key, value, route);
                }
            }
            "DHCPv4" | "DHCP" => parse_dhcpv4_entry(key, value, &mut cfg.dhcpv4),
            "Link" => parse_link_entry(key, value, &mut cfg.link),
            section => {
                // Silently skip unknown sections (vendor extensions, etc.)
                log::trace!(
                    "{}: ignoring unknown section [{}] key {}",
                    path.display(),
                    section,
                    key
                );
            }
        }
    }

    // Flush trailing sections.
    if let Some(addr) = current_address {
        cfg.addresses.push(addr);
    }
    if let Some(route) = current_route {
        cfg.routes.push(route);
    }

    Ok(cfg)
}

// ---------------------------------------------------------------------------
// Per-section parsers
// ---------------------------------------------------------------------------

fn parse_match_entry(key: &str, value: &str, section: &mut MatchSection) {
    match key {
        "Name" => section.names.extend(split_whitespace_values(value)),
        "MACAddress" => section.mac_addresses.extend(split_whitespace_values(value)),
        "Driver" => section.drivers.extend(split_whitespace_values(value)),
        "Type" => section.types.extend(split_whitespace_values(value)),
        "Path" => section.paths.extend(split_whitespace_values(value)),
        "KernelCommandLine" => section.kernel_command_line.push(value.to_string()),
        "Virtualization" => section.virtualization = Some(value.to_string()),
        "Host" => section.host = Some(value.to_string()),
        "Architecture" => section.architecture = Some(value.to_string()),
        _ => {}
    }
}

fn parse_network_entry(key: &str, value: &str, section: &mut NetworkSection) {
    match key {
        "DHCP" => section.dhcp = parse_dhcp_mode(value),
        "DNS" => {
            for tok in split_whitespace_values(value) {
                if let Ok(ip) = tok.parse::<IpAddr>() {
                    section.dns.push(ip);
                }
            }
        }
        "Domains" => section.domains.extend(split_whitespace_values(value)),
        "NTP" => section.ntp.extend(split_whitespace_values(value)),
        "LinkLocalAddressing" => section.link_local = parse_link_local(value),
        "IPv6AcceptRA" => {
            section.ipv6_accept_ra = if parse_bool(value) {
                Ipv6AcceptRa::Yes
            } else {
                Ipv6AcceptRa::No
            }
        }
        "LLDP" => section.lldp = parse_bool(value),
        "EmitLLDP" => section.emit_lldp = parse_bool(value),
        "MulticastDNS" => section.multicast_dns = parse_bool(value),
        "DNSOverTLS" => section.dns_over_tls = Some(value.to_string()),
        "DNSSEC" => section.dnssec = Some(value.to_string()),
        "IPForward" => section.ip_forward = Some(value.to_string()),
        "IPMasquerade" => section.ip_masquerade = Some(value.to_string()),
        "IPv6PrivacyExtensions" => section.ipv6_privacy_extensions = Some(value.to_string()),
        "IPv6StableSecretAddress" => section.ipv6_stable_secret_address = Some(value.to_string()),
        "IPv6LinkLocalAddressGenerationMode" => {
            section.ipv6_ll_addr_gen_mode = Some(value.to_string())
        }
        "Bridge" => section.bridge = Some(value.to_string()),
        "Bond" => section.bond = Some(value.to_string()),
        "VLAN" => section.vlans.extend(split_whitespace_values(value)),
        "Description" => section.description = Some(value.to_string()),
        "RequiredForOnline" => section.required_for_online = Some(value.to_string()),
        _ => {}
    }
}

fn parse_address_entry(key: &str, value: &str, section: &mut AddressSection) {
    match key {
        "Address" => section.address = value.to_string(),
        "Peer" => section.peer = Some(value.to_string()),
        "Broadcast" => section.broadcast = Some(value.to_string()),
        "Label" => section.label = Some(value.to_string()),
        "PreferredLifetime" => section.preferred_lifetime = Some(value.to_string()),
        _ => {}
    }
}

fn parse_route_entry(key: &str, value: &str, section: &mut RouteSection) {
    match key {
        "Destination" => section.destination = Some(value.to_string()),
        "Gateway" => section.gateway = Some(value.to_string()),
        "GatewayOnLink" | "GatewayOnlink" => section.gateway_on_link = parse_bool(value),
        "Source" => section.source = Some(value.to_string()),
        "Metric" => section.metric = value.parse().ok(),
        "Scope" => section.scope = Some(value.to_string()),
        "Table" => section.table = Some(value.to_string()),
        "Type" => section.route_type = Some(value.to_string()),
        _ => {}
    }
}

fn parse_dhcpv4_entry(key: &str, value: &str, section: &mut DhcpV4Section) {
    match key {
        "UseDNS" => section.use_dns = parse_bool(value),
        "UseNTP" => section.use_ntp = parse_bool(value),
        "UseDomains" => section.use_domains = parse_bool(value),
        "UseRoutes" => section.use_routes = parse_bool(value),
        "UseHostname" => section.use_hostname = parse_bool(value),
        "UseMTU" => section.use_mtu = parse_bool(value),
        "UseTimezone" => section.use_timezone = parse_bool(value),
        "SendHostname" => section.send_hostname = parse_bool(value),
        "Hostname" => section.hostname = Some(value.to_string()),
        "ClientIdentifier" => section.client_identifier = Some(value.to_string()),
        "VendorClassIdentifier" => section.vendor_class_id = Some(value.to_string()),
        "RequestBroadcast" => section.request_broadcast = parse_bool(value),
        "RouteMetric" => section.route_metric = value.parse().ok(),
        "MaxAttempts" => section.max_attempts = value.parse().ok(),
        "ListenPort" => section.listen_port = value.parse().ok(),
        "CriticalConnection" => section.critical_connection = parse_bool(value),
        _ => {}
    }
}

fn parse_link_entry(key: &str, value: &str, section: &mut LinkSection) {
    match key {
        "MTUBytes" => section.mtu = parse_bytes_value(value),
        "MACAddress" => section.mac_address = Some(value.to_string()),
        "ARP" => section.arp = Some(parse_bool(value)),
        "Multicast" => section.multicast = Some(parse_bool(value)),
        "Unmanaged" => section.unmanaged = parse_bool(value),
        "RequiredForOnline" => section.required_for_online = Some(value.to_string()),
        "ActivationPolicy" => section.activation_policy = Some(value.to_string()),
        _ => {}
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn parse_dhcp_mode(value: &str) -> DhcpMode {
    match value.to_lowercase().as_str() {
        "yes" | "true" | "1" | "both" => DhcpMode::Yes,
        "ipv4" | "v4" => DhcpMode::Ipv4,
        "ipv6" | "v6" => DhcpMode::Ipv6,
        _ => DhcpMode::No,
    }
}

fn parse_link_local(value: &str) -> LinkLocalMode {
    match value.to_lowercase().as_str() {
        "yes" | "true" | "1" => LinkLocalMode::Yes,
        "ipv4" | "v4" => LinkLocalMode::Ipv4,
        "ipv6" | "v6" => LinkLocalMode::Ipv6,
        _ => LinkLocalMode::No,
    }
}

fn parse_bool(value: &str) -> bool {
    matches!(value.to_lowercase().as_str(), "yes" | "true" | "1" | "on")
}

fn parse_bytes_value(value: &str) -> Option<u32> {
    let value = value.trim();
    // Support suffixes: K, M, G (case-insensitive, with optional 'B').
    let (num_str, multiplier) =
        if let Some(s) = value.strip_suffix('G').or_else(|| value.strip_suffix("GB")) {
            (s.trim(), 1024 * 1024 * 1024)
        } else if let Some(s) = value.strip_suffix('M').or_else(|| value.strip_suffix("MB")) {
            (s.trim(), 1024 * 1024)
        } else if let Some(s) = value.strip_suffix('K').or_else(|| value.strip_suffix("KB")) {
            (s.trim(), 1024)
        } else {
            (value, 1)
        };
    num_str.parse::<u32>().ok().map(|n| n * multiplier)
}

fn split_whitespace_values(value: &str) -> Vec<String> {
    value.split_whitespace().map(|s| s.to_string()).collect()
}

/// Minimalist glob matching supporting `*` (any chars) and `?` (single char).
fn glob_match(pattern: &str, text: &str) -> bool {
    let pat: Vec<char> = pattern.chars().collect();
    let txt: Vec<char> = text.chars().collect();
    glob_match_inner(&pat, &txt)
}

fn glob_match_inner(pattern: &[char], text: &[char]) -> bool {
    let (mut pi, mut ti) = (0usize, 0usize);
    let (mut star_pi, mut star_ti) = (usize::MAX, 0usize);

    while ti < text.len() {
        if pi < pattern.len() && (pattern[pi] == '?' || pattern[pi] == text[ti]) {
            pi += 1;
            ti += 1;
        } else if pi < pattern.len() && pattern[pi] == '*' {
            star_pi = pi;
            star_ti = ti;
            pi += 1;
        } else if star_pi != usize::MAX {
            pi = star_pi + 1;
            star_ti += 1;
            ti = star_ti;
        } else {
            return false;
        }
    }

    while pi < pattern.len() && pattern[pi] == '*' {
        pi += 1;
    }

    pi == pattern.len()
}

/// Parse a CIDR address string like `192.168.1.5/24` into (address, prefix_len).
pub fn parse_cidr(cidr: &str) -> Option<(IpAddr, u8)> {
    let (addr_str, prefix_str) = cidr.split_once('/')?;
    let addr: IpAddr = addr_str.trim().parse().ok()?;
    let prefix: u8 = prefix_str.trim().parse().ok()?;
    Some((addr, prefix))
}

/// Parse a CIDR string specifically for IPv4 into (Ipv4Addr, prefix_len).
pub fn parse_ipv4_cidr(cidr: &str) -> Option<(Ipv4Addr, u8)> {
    let (addr, prefix) = parse_cidr(cidr)?;
    match addr {
        IpAddr::V4(v4) => Some((v4, prefix)),
        IpAddr::V6(_) => None,
    }
}

/// Compute the broadcast address from an IPv4 address and prefix length.
pub fn ipv4_broadcast(addr: Ipv4Addr, prefix_len: u8) -> Ipv4Addr {
    let ip: u32 = u32::from(addr);
    if prefix_len >= 32 {
        return addr;
    }
    let host_bits = 32 - prefix_len;
    let mask = !((1u32 << host_bits) - 1);
    let broadcast = (ip & mask) | !mask;
    Ipv4Addr::from(broadcast)
}

/// Compute the network address from an IPv4 address and prefix length.
pub fn ipv4_network(addr: Ipv4Addr, prefix_len: u8) -> Ipv4Addr {
    let ip: u32 = u32::from(addr);
    if prefix_len >= 32 {
        return addr;
    }
    let host_bits = 32 - prefix_len;
    let mask = !((1u32 << host_bits) - 1);
    Ipv4Addr::from(ip & mask)
}

// ---------------------------------------------------------------------------
// .link file parser (systemd.link(5)) — re-exported from libsystemd
// ---------------------------------------------------------------------------

// All `.link` file types, parsing, loading, and matching are provided by
// the shared `libsystemd::link_config` module so that both networkd and
// udevd (via the `net_setup_link` builtin) can use the same code.
#[allow(unused_imports)]
pub use libsystemd::link_config::{
    LinkFileConfig, LinkMatchSection, LinkSettingsSection, MACAddressPolicy, NamePolicy,
    find_matching_link_config, load_link_configs, load_link_configs_from, parse_link_file,
    parse_link_file_content, resolve_name_from_policy,
};

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_minimal_network() {
        let content = r#"
[Match]
Name=en*

[Network]
DHCP=yes
"#;
        let cfg = parse_network_content(content, Path::new("10-en.network")).unwrap();
        assert_eq!(cfg.match_section.names, vec!["en*"]);
        assert_eq!(cfg.network_section.dhcp, DhcpMode::Yes);
    }

    #[test]
    fn test_parse_static_address() {
        let content = r#"
[Match]
Name=eth0

[Network]
DHCP=no
DNS=8.8.8.8 8.8.4.4

[Address]
Address=192.168.1.100/24

[Route]
Gateway=192.168.1.1
"#;
        let cfg = parse_network_content(content, Path::new("20-static.network")).unwrap();
        assert_eq!(cfg.match_section.names, vec!["eth0"]);
        assert_eq!(cfg.network_section.dhcp, DhcpMode::No);
        assert_eq!(cfg.network_section.dns.len(), 2);
        assert_eq!(cfg.addresses.len(), 1);
        assert_eq!(cfg.addresses[0].address, "192.168.1.100/24");
        assert_eq!(cfg.routes.len(), 1);
        assert_eq!(cfg.routes[0].gateway.as_deref(), Some("192.168.1.1"));
    }

    #[test]
    fn test_parse_multiple_addresses_and_routes() {
        let content = r#"
[Match]
Name=br0

[Address]
Address=10.0.0.1/24

[Address]
Address=10.0.1.1/24

[Route]
Destination=10.0.2.0/24
Gateway=10.0.0.254

[Route]
Destination=0.0.0.0/0
Gateway=10.0.0.1
Metric=100
"#;
        let cfg = parse_network_content(content, Path::new("30-br0.network")).unwrap();
        assert_eq!(cfg.addresses.len(), 2);
        assert_eq!(cfg.addresses[0].address, "10.0.0.1/24");
        assert_eq!(cfg.addresses[1].address, "10.0.1.1/24");
        assert_eq!(cfg.routes.len(), 2);
        assert_eq!(cfg.routes[1].metric, Some(100));
    }

    #[test]
    fn test_parse_dhcpv4_section() {
        let content = r#"
[Match]
Name=eth0

[Network]
DHCP=ipv4

[DHCPv4]
UseDNS=yes
UseNTP=no
RouteMetric=200
SendHostname=yes
Hostname=myhost
"#;
        let cfg = parse_network_content(content, Path::new("test.network")).unwrap();
        assert_eq!(cfg.network_section.dhcp, DhcpMode::Ipv4);
        assert!(cfg.dhcpv4.use_dns);
        assert!(!cfg.dhcpv4.use_ntp);
        assert_eq!(cfg.dhcpv4.route_metric, Some(200));
        assert!(cfg.dhcpv4.send_hostname);
        assert_eq!(cfg.dhcpv4.hostname.as_deref(), Some("myhost"));
    }

    #[test]
    fn test_parse_link_section() {
        let content = r#"
[Match]
Name=wlan0

[Link]
MTUBytes=1400
RequiredForOnline=no
ActivationPolicy=manual
"#;
        let cfg = parse_network_content(content, Path::new("test.network")).unwrap();
        assert_eq!(cfg.link.mtu, Some(1400));
        assert_eq!(cfg.link.required_for_online.as_deref(), Some("no"));
        assert_eq!(cfg.link.activation_policy.as_deref(), Some("manual"));
    }

    #[test]
    fn test_parse_mac_match() {
        let content = r#"
[Match]
MACAddress=aa:bb:cc:dd:ee:ff

[Network]
DHCP=yes
"#;
        let cfg = parse_network_content(content, Path::new("test.network")).unwrap();
        assert_eq!(cfg.match_section.mac_addresses, vec!["aa:bb:cc:dd:ee:ff"]);
    }

    #[test]
    fn test_parse_dhcp_modes() {
        assert_eq!(parse_dhcp_mode("yes"), DhcpMode::Yes);
        assert_eq!(parse_dhcp_mode("true"), DhcpMode::Yes);
        assert_eq!(parse_dhcp_mode("both"), DhcpMode::Yes);
        assert_eq!(parse_dhcp_mode("ipv4"), DhcpMode::Ipv4);
        assert_eq!(parse_dhcp_mode("v4"), DhcpMode::Ipv4);
        assert_eq!(parse_dhcp_mode("ipv6"), DhcpMode::Ipv6);
        assert_eq!(parse_dhcp_mode("no"), DhcpMode::No);
        assert_eq!(parse_dhcp_mode("false"), DhcpMode::No);
    }

    #[test]
    fn test_parse_bool_values() {
        assert!(parse_bool("yes"));
        assert!(parse_bool("true"));
        assert!(parse_bool("1"));
        assert!(parse_bool("on"));
        assert!(!parse_bool("no"));
        assert!(!parse_bool("false"));
        assert!(!parse_bool("0"));
        assert!(!parse_bool("off"));
    }

    #[test]
    fn test_parse_bytes_value() {
        assert_eq!(parse_bytes_value("1500"), Some(1500));
        assert_eq!(parse_bytes_value("1K"), Some(1024));
        assert_eq!(parse_bytes_value("1KB"), Some(1024));
        assert_eq!(parse_bytes_value("2M"), Some(2 * 1024 * 1024));
        assert_eq!(parse_bytes_value("1G"), Some(1024 * 1024 * 1024));
    }

    #[test]
    fn test_glob_match() {
        assert!(glob_match("en*", "ens3"));
        assert!(glob_match("en*", "enp0s3"));
        assert!(glob_match("en*", "en"));
        assert!(!glob_match("en*", "wlan0"));
        assert!(glob_match("eth?", "eth0"));
        assert!(glob_match("eth?", "eth1"));
        assert!(!glob_match("eth?", "eth10"));
        assert!(glob_match("*", "anything"));
        assert!(glob_match("e?h*", "eth0"));
        assert!(glob_match("e?h*", "exh123"));
    }

    #[test]
    fn test_match_interface() {
        let section = MatchSection {
            names: vec!["en*".to_string()],
            ..Default::default()
        };
        assert!(section.matches_interface("ens3", None, None));
        assert!(section.matches_interface("enp0s3", None, None));
        assert!(!section.matches_interface("wlan0", None, None));

        // Empty match matches everything.
        let empty = MatchSection::default();
        assert!(empty.matches_interface("anything", None, None));
    }

    #[test]
    fn test_match_mac_address() {
        let section = MatchSection {
            mac_addresses: vec!["AA:BB:CC:DD:EE:FF".to_string()],
            ..Default::default()
        };
        assert!(section.matches_interface("eth0", Some("aa:bb:cc:dd:ee:ff"), None));
        assert!(!section.matches_interface("eth0", Some("11:22:33:44:55:66"), None));
        assert!(!section.matches_interface("eth0", None, None));
    }

    #[test]
    fn test_parse_cidr() {
        let (addr, prefix) = parse_cidr("192.168.1.5/24").unwrap();
        assert_eq!(addr, IpAddr::V4(Ipv4Addr::new(192, 168, 1, 5)));
        assert_eq!(prefix, 24);

        let (addr, prefix) = parse_cidr("10.0.0.1/8").unwrap();
        assert_eq!(addr, IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1)));
        assert_eq!(prefix, 8);

        assert!(parse_cidr("invalid").is_none());
        assert!(parse_cidr("192.168.1.1").is_none());
    }

    #[test]
    fn test_ipv4_broadcast() {
        assert_eq!(
            ipv4_broadcast(Ipv4Addr::new(192, 168, 1, 5), 24),
            Ipv4Addr::new(192, 168, 1, 255)
        );
        assert_eq!(
            ipv4_broadcast(Ipv4Addr::new(10, 0, 0, 1), 8),
            Ipv4Addr::new(10, 255, 255, 255)
        );
        assert_eq!(
            ipv4_broadcast(Ipv4Addr::new(172, 16, 5, 10), 16),
            Ipv4Addr::new(172, 16, 255, 255)
        );
    }

    #[test]
    fn test_ipv4_network() {
        assert_eq!(
            ipv4_network(Ipv4Addr::new(192, 168, 1, 5), 24),
            Ipv4Addr::new(192, 168, 1, 0)
        );
        assert_eq!(
            ipv4_network(Ipv4Addr::new(10, 1, 2, 3), 8),
            Ipv4Addr::new(10, 0, 0, 0)
        );
    }

    #[test]
    fn test_comments_and_blank_lines() {
        let content = r#"
# This is a comment
; Also a comment

[Match]
Name=eth0

# Comment between sections

[Network]
; Another comment
DHCP=yes
"#;
        let cfg = parse_network_content(content, Path::new("test.network")).unwrap();
        assert_eq!(cfg.match_section.names, vec!["eth0"]);
        assert_eq!(cfg.network_section.dhcp, DhcpMode::Yes);
    }

    #[test]
    fn test_skip_unknown_sections() {
        let content = r#"
[Match]
Name=eth0

[Network]
DHCP=yes

[SomeVendorExtension]
Foo=bar

[Address]
Address=10.0.0.1/24
"#;
        let cfg = parse_network_content(content, Path::new("test.network")).unwrap();
        assert_eq!(cfg.network_section.dhcp, DhcpMode::Yes);
        assert_eq!(cfg.addresses.len(), 1);
    }

    #[test]
    fn test_network_section_dns() {
        let content = r#"
[Network]
DNS=8.8.8.8 1.1.1.1
DNS=9.9.9.9
"#;
        let cfg = parse_network_content(content, Path::new("test.network")).unwrap();
        assert_eq!(cfg.network_section.dns.len(), 3);
    }

    #[test]
    fn test_network_section_vlans() {
        let content = r#"
[Network]
VLAN=vlan10
VLAN=vlan20
"#;
        let cfg = parse_network_content(content, Path::new("test.network")).unwrap();
        assert_eq!(cfg.network_section.vlans, vec!["vlan10", "vlan20"]);
    }

    #[test]
    fn test_load_from_dir() {
        let dir = tempfile::tempdir().unwrap();

        // Create two .network files.
        let f1 = dir.path().join("10-lan.network");
        fs::write(&f1, "[Match]\nName=eth0\n\n[Network]\nDHCP=yes\n").unwrap();

        let f2 = dir.path().join("20-wlan.network");
        fs::write(&f2, "[Match]\nName=wlan0\n\n[Network]\nDHCP=ipv4\n").unwrap();

        // Also create a non-.network file that should be ignored.
        fs::write(dir.path().join("README.txt"), "ignore me").unwrap();

        let configs = load_network_configs_from(&[dir.path().to_path_buf()]);
        assert_eq!(configs.len(), 2);
        // Should be sorted by filename.
        assert!(configs[0].path.ends_with("10-lan.network"));
        assert!(configs[1].path.ends_with("20-wlan.network"));
    }

    #[test]
    fn test_dedup_across_dirs() {
        let dir1 = tempfile::tempdir().unwrap();
        let dir2 = tempfile::tempdir().unwrap();

        // Same filename in both dirs — first dir wins.
        fs::write(
            dir1.path().join("10-lan.network"),
            "[Match]\nName=eth0\n\n[Network]\nDHCP=yes\n",
        )
        .unwrap();
        fs::write(
            dir2.path().join("10-lan.network"),
            "[Match]\nName=eth1\n\n[Network]\nDHCP=no\n",
        )
        .unwrap();

        let configs =
            load_network_configs_from(&[dir1.path().to_path_buf(), dir2.path().to_path_buf()]);
        assert_eq!(configs.len(), 1);
        assert_eq!(configs[0].match_section.names, vec!["eth0"]);
        assert_eq!(configs[0].network_section.dhcp, DhcpMode::Yes);
    }

    #[test]
    fn test_route_gateway_on_link() {
        let content = r#"
[Route]
Gateway=169.254.1.1
GatewayOnLink=yes

[Route]
Destination=0.0.0.0/0
Gateway=169.254.1.1
GatewayOnLink=true
Metric=50
"#;
        let cfg = parse_network_content(content, Path::new("test.network")).unwrap();
        assert_eq!(cfg.routes.len(), 2);
        assert!(cfg.routes[0].gateway_on_link);
        assert!(cfg.routes[1].gateway_on_link);
        assert_eq!(cfg.routes[1].metric, Some(50));
    }

    #[test]
    fn test_address_with_peer_and_label() {
        let content = r#"
[Address]
Address=10.0.0.1/32
Peer=10.0.0.2/32
Label=vpn0
"#;
        let cfg = parse_network_content(content, Path::new("test.network")).unwrap();
        assert_eq!(cfg.addresses.len(), 1);
        assert_eq!(cfg.addresses[0].address, "10.0.0.1/32");
        assert_eq!(cfg.addresses[0].peer.as_deref(), Some("10.0.0.2/32"));
        assert_eq!(cfg.addresses[0].label.as_deref(), Some("vpn0"));
    }

    #[test]
    fn test_link_local_modes() {
        assert_eq!(parse_link_local("yes"), LinkLocalMode::Yes);
        assert_eq!(parse_link_local("no"), LinkLocalMode::No);
        assert_eq!(parse_link_local("ipv4"), LinkLocalMode::Ipv4);
        assert_eq!(parse_link_local("ipv6"), LinkLocalMode::Ipv6);
    }

    #[test]
    fn test_dhcp_section_alias() {
        // systemd also accepts [DHCP] as an alias for [DHCPv4].
        let content = r#"
[DHCP]
UseDNS=no
UseNTP=yes
RouteMetric=500
"#;
        let cfg = parse_network_content(content, Path::new("test.network")).unwrap();
        assert!(!cfg.dhcpv4.use_dns);
        assert!(cfg.dhcpv4.use_ntp);
        assert_eq!(cfg.dhcpv4.route_metric, Some(500));
    }

    // ── .link file parser tests ────────────────────────────────────────────

    #[test]
    fn test_parse_link_file_basic() {
        let content = r#"
[Match]
OriginalName=en*

[Link]
Name=eth0
Description=Ethernet adapter
MTUBytes=1500
"#;
        let cfg = parse_link_file_content(content, Path::new("10-eth.link")).unwrap();
        assert_eq!(cfg.match_section.original_names, vec!["en*"]);
        assert_eq!(cfg.link_section.name.as_deref(), Some("eth0"));
        assert_eq!(
            cfg.link_section.description.as_deref(),
            Some("Ethernet adapter")
        );
        assert_eq!(cfg.link_section.mtu, Some(1500));
    }

    #[test]
    fn test_parse_link_file_name_policy() {
        let content = r#"
[Match]
OriginalName=*

[Link]
NamePolicy=kernel database onboard slot path
AlternativeNamesPolicy=database onboard slot path mac
"#;
        let cfg = parse_link_file_content(content, Path::new("99-default.link")).unwrap();
        assert_eq!(
            cfg.link_section.name_policy,
            vec![
                NamePolicy::Kernel,
                NamePolicy::Database,
                NamePolicy::Onboard,
                NamePolicy::Slot,
                NamePolicy::Path,
            ]
        );
        assert_eq!(
            cfg.link_section.alternative_names_policy,
            vec![
                NamePolicy::Database,
                NamePolicy::Onboard,
                NamePolicy::Slot,
                NamePolicy::Path,
                NamePolicy::Mac,
            ]
        );
    }

    #[test]
    fn test_parse_link_file_mac_address_policy() {
        let content = r#"
[Match]
OriginalName=*

[Link]
MACAddressPolicy=persistent
"#;
        let cfg = parse_link_file_content(content, Path::new("test.link")).unwrap();
        assert_eq!(
            cfg.link_section.mac_address_policy,
            Some(MACAddressPolicy::Persistent)
        );
    }

    #[test]
    fn test_parse_link_file_mac_address_policy_random() {
        let content = r#"
[Link]
MACAddressPolicy=random
"#;
        let cfg = parse_link_file_content(content, Path::new("test.link")).unwrap();
        assert_eq!(
            cfg.link_section.mac_address_policy,
            Some(MACAddressPolicy::Random)
        );
    }

    #[test]
    fn test_parse_link_file_mac_address_policy_none() {
        let content = r#"
[Link]
MACAddressPolicy=none
"#;
        let cfg = parse_link_file_content(content, Path::new("test.link")).unwrap();
        assert_eq!(
            cfg.link_section.mac_address_policy,
            Some(MACAddressPolicy::None)
        );
    }

    #[test]
    fn test_parse_link_file_match_mac() {
        let content = r#"
[Match]
MACAddress=aa:bb:cc:dd:ee:ff 11:22:33:44:55:66

[Link]
Name=lan0
"#;
        let cfg = parse_link_file_content(content, Path::new("test.link")).unwrap();
        assert_eq!(cfg.match_section.mac_addresses.len(), 2);
        assert_eq!(cfg.match_section.mac_addresses[0], "aa:bb:cc:dd:ee:ff");
        assert_eq!(cfg.match_section.mac_addresses[1], "11:22:33:44:55:66");
    }

    #[test]
    fn test_parse_link_file_match_driver() {
        let content = r#"
[Match]
Driver=virtio_net e1000*

[Link]
Name=vnet0
"#;
        let cfg = parse_link_file_content(content, Path::new("test.link")).unwrap();
        assert_eq!(cfg.match_section.drivers, vec!["virtio_net", "e1000*"]);
    }

    #[test]
    fn test_parse_link_file_match_type() {
        let content = r#"
[Match]
Type=ether wlan

[Link]
Description=Wired or wireless
"#;
        let cfg = parse_link_file_content(content, Path::new("test.link")).unwrap();
        assert_eq!(cfg.match_section.types, vec!["ether", "wlan"]);
    }

    #[test]
    fn test_parse_link_file_match_path() {
        let content = r#"
[Match]
Path=pci-0000:02:00.0-*

[Link]
Name=lan0
"#;
        let cfg = parse_link_file_content(content, Path::new("test.link")).unwrap();
        assert_eq!(cfg.match_section.paths, vec!["pci-0000:02:00.0-*"]);
    }

    #[test]
    fn test_parse_link_file_match_host_virt_arch() {
        let content = r#"
[Match]
Host=myhost
Virtualization=kvm
Architecture=x86-64
KernelCommandLine=debug
KernelVersion=6.*
Credential=mykey

[Link]
Name=vm0
"#;
        let cfg = parse_link_file_content(content, Path::new("test.link")).unwrap();
        assert_eq!(cfg.match_section.host.as_deref(), Some("myhost"));
        assert_eq!(cfg.match_section.virtualization.as_deref(), Some("kvm"));
        assert_eq!(cfg.match_section.architecture.as_deref(), Some("x86-64"));
        assert_eq!(
            cfg.match_section.kernel_command_line.as_deref(),
            Some("debug")
        );
        assert_eq!(cfg.match_section.kernel_version.as_deref(), Some("6.*"));
        assert_eq!(cfg.match_section.credential.as_deref(), Some("mykey"));
    }

    #[test]
    fn test_parse_link_file_offload_settings() {
        let content = r#"
[Link]
ReceiveChecksumOffload=yes
TransmitChecksumOffload=no
TCPSegmentationOffload=yes
TCP6SegmentationOffload=no
GenericReceiveOffload=yes
GenericSegmentationOffload=no
LargeReceiveOffload=yes
"#;
        let cfg = parse_link_file_content(content, Path::new("test.link")).unwrap();
        assert_eq!(cfg.link_section.rx_checksum_offload, Some(true));
        assert_eq!(cfg.link_section.tx_checksum_offload, Some(false));
        assert_eq!(cfg.link_section.tcp_segmentation_offload, Some(true));
        assert_eq!(cfg.link_section.tcp6_segmentation_offload, Some(false));
        assert_eq!(cfg.link_section.generic_receive_offload, Some(true));
        assert_eq!(cfg.link_section.generic_segmentation_offload, Some(false));
        assert_eq!(cfg.link_section.large_receive_offload, Some(true));
    }

    #[test]
    fn test_parse_link_file_channel_settings() {
        let content = r#"
[Link]
RxChannels=4
TxChannels=4
OtherChannels=2
CombinedChannels=8
"#;
        let cfg = parse_link_file_content(content, Path::new("test.link")).unwrap();
        assert_eq!(cfg.link_section.rx_channels, Some(4));
        assert_eq!(cfg.link_section.tx_channels, Some(4));
        assert_eq!(cfg.link_section.other_channels, Some(2));
        assert_eq!(cfg.link_section.combined_channels, Some(8));
    }

    #[test]
    fn test_parse_link_file_misc_settings() {
        let content = r#"
[Link]
Duplex=full
AutoNegotiation=yes
WakeOnLan=magic
Port=tp
BitsPerSecond=1G
GenericSegmentOffloadMaxBytes=65536
GenericSegmentOffloadMaxSegments=128
Unmanaged=no
RequiredForOnline=yes
RequiredFamilyForOnline=ipv4
ActivationPolicy=up
"#;
        let cfg = parse_link_file_content(content, Path::new("test.link")).unwrap();
        assert_eq!(cfg.link_section.duplex.as_deref(), Some("full"));
        assert_eq!(cfg.link_section.auto_negotiation, Some(true));
        assert_eq!(cfg.link_section.wake_on_lan.as_deref(), Some("magic"));
        assert_eq!(cfg.link_section.port.as_deref(), Some("tp"));
        assert_eq!(cfg.link_section.bits_per_second, Some(1_073_741_824));
        assert_eq!(cfg.link_section.gso_max_bytes, Some(65536));
        assert_eq!(cfg.link_section.gso_max_segments, Some(128));
        assert_eq!(cfg.link_section.unmanaged, Some(false));
        assert_eq!(cfg.link_section.required_for_online, Some(true));
        assert_eq!(
            cfg.link_section.required_family_for_online.as_deref(),
            Some("ipv4")
        );
        assert_eq!(cfg.link_section.activation_policy.as_deref(), Some("up"));
    }

    #[test]
    fn test_parse_link_file_explicit_mac() {
        let content = r#"
[Link]
MACAddress=00:11:22:33:44:55
"#;
        let cfg = parse_link_file_content(content, Path::new("test.link")).unwrap();
        assert_eq!(
            cfg.link_section.mac_address.as_deref(),
            Some("00:11:22:33:44:55")
        );
    }

    #[test]
    fn test_parse_link_file_alternative_names() {
        let content = r#"
[Link]
AlternativeName=myiface
AlternativeName=backup0
"#;
        let cfg = parse_link_file_content(content, Path::new("test.link")).unwrap();
        assert_eq!(
            cfg.link_section.alternative_names,
            vec!["myiface", "backup0"]
        );
    }

    #[test]
    fn test_parse_link_file_comments_and_blanks() {
        let content = r#"
# This is a comment
; This too

[Match]
OriginalName=eth*

# Another comment
[Link]
Name=lan0
"#;
        let cfg = parse_link_file_content(content, Path::new("test.link")).unwrap();
        assert_eq!(cfg.match_section.original_names, vec!["eth*"]);
        assert_eq!(cfg.link_section.name.as_deref(), Some("lan0"));
    }

    #[test]
    fn test_parse_link_file_unknown_sections_ignored() {
        let content = r#"
[Match]
OriginalName=en*

[SomeUnknownSection]
Foo=bar

[Link]
Name=net0
"#;
        let cfg = parse_link_file_content(content, Path::new("test.link")).unwrap();
        assert_eq!(cfg.link_section.name.as_deref(), Some("net0"));
    }

    #[test]
    fn test_parse_link_file_empty() {
        let content = "";
        let cfg = parse_link_file_content(content, Path::new("test.link")).unwrap();
        assert!(cfg.match_section.original_names.is_empty());
        assert!(cfg.link_section.name.is_none());
        assert!(cfg.link_section.name_policy.is_empty());
    }

    #[test]
    fn test_parse_link_file_match_property() {
        let content = r#"
[Match]
Property=ID_NET_DRIVER=virtio_net
"#;
        let cfg = parse_link_file_content(content, Path::new("test.link")).unwrap();
        assert_eq!(
            cfg.match_section.properties,
            vec!["ID_NET_DRIVER=virtio_net"]
        );
    }

    #[test]
    fn test_parse_link_file_mtu_with_suffix() {
        let content = r#"
[Link]
MTUBytes=9K
"#;
        let cfg = parse_link_file_content(content, Path::new("test.link")).unwrap();
        assert_eq!(cfg.link_section.mtu, Some(9216));
    }

    // ── LinkMatchSection matching tests ────────────────────────────────────

    #[test]
    fn test_link_match_empty_matches_all() {
        let section = LinkMatchSection::default();
        assert!(section.matches_interface("eth0", None, None, None, None));
        assert!(section.matches_interface("wlan0", Some("aa:bb:cc:dd:ee:ff"), None, None, None));
    }

    #[test]
    fn test_link_match_original_name_glob() {
        let section = LinkMatchSection {
            original_names: vec!["en*".to_string()],
            ..Default::default()
        };
        assert!(section.matches_interface("enp3s0", None, None, None, None));
        assert!(section.matches_interface("ens1", None, None, None, None));
        assert!(!section.matches_interface("eth0", None, None, None, None));
        assert!(!section.matches_interface("wlan0", None, None, None, None));
    }

    #[test]
    fn test_link_match_original_name_wildcard_all() {
        let section = LinkMatchSection {
            original_names: vec!["*".to_string()],
            ..Default::default()
        };
        assert!(section.matches_interface("anything", None, None, None, None));
    }

    #[test]
    fn test_link_match_original_name_exact() {
        let section = LinkMatchSection {
            original_names: vec!["eth0".to_string()],
            ..Default::default()
        };
        assert!(section.matches_interface("eth0", None, None, None, None));
        assert!(!section.matches_interface("eth1", None, None, None, None));
    }

    #[test]
    fn test_link_match_mac_address() {
        let section = LinkMatchSection {
            mac_addresses: vec!["aa:bb:cc:dd:ee:ff".to_string()],
            ..Default::default()
        };
        assert!(section.matches_interface("eth0", Some("aa:bb:cc:dd:ee:ff"), None, None, None));
        assert!(section.matches_interface("eth0", Some("AA:BB:CC:DD:EE:FF"), None, None, None));
        assert!(!section.matches_interface("eth0", Some("11:22:33:44:55:66"), None, None, None));
        assert!(!section.matches_interface("eth0", None, None, None, None));
    }

    #[test]
    fn test_link_match_driver() {
        let section = LinkMatchSection {
            drivers: vec!["virtio*".to_string()],
            ..Default::default()
        };
        assert!(section.matches_interface("eth0", None, Some("virtio_net"), None, None));
        assert!(!section.matches_interface("eth0", None, Some("e1000"), None, None));
        assert!(!section.matches_interface("eth0", None, None, None, None));
    }

    #[test]
    fn test_link_match_type() {
        let section = LinkMatchSection {
            types: vec!["ether".to_string()],
            ..Default::default()
        };
        assert!(section.matches_interface("eth0", None, None, Some("ether"), None));
        assert!(!section.matches_interface("wlan0", None, None, Some("wlan"), None));
        assert!(!section.matches_interface("eth0", None, None, None, None));
    }

    #[test]
    fn test_link_match_path() {
        let section = LinkMatchSection {
            paths: vec!["pci-0000:02:00.0-*".to_string()],
            ..Default::default()
        };
        assert!(section.matches_interface(
            "eth0",
            None,
            None,
            None,
            Some("pci-0000:02:00.0-enp2s0")
        ));
        assert!(!section.matches_interface(
            "eth0",
            None,
            None,
            None,
            Some("pci-0000:03:00.0-enp3s0")
        ));
    }

    #[test]
    fn test_link_match_multiple_criteria_all_must_match() {
        let section = LinkMatchSection {
            original_names: vec!["en*".to_string()],
            drivers: vec!["virtio*".to_string()],
            ..Default::default()
        };
        // Both name and driver match.
        assert!(section.matches_interface("enp3s0", None, Some("virtio_net"), None, None));
        // Name matches but driver doesn't.
        assert!(!section.matches_interface("enp3s0", None, Some("e1000"), None, None));
        // Driver matches but name doesn't.
        assert!(!section.matches_interface("eth0", None, Some("virtio_net"), None, None));
    }

    #[test]
    fn test_link_match_multiple_original_names() {
        let section = LinkMatchSection {
            original_names: vec!["en*".to_string(), "eth*".to_string()],
            ..Default::default()
        };
        assert!(section.matches_interface("enp3s0", None, None, None, None));
        assert!(section.matches_interface("eth0", None, None, None, None));
        assert!(!section.matches_interface("wlan0", None, None, None, None));
    }

    #[test]
    fn test_link_match_multiple_mac_addresses() {
        let section = LinkMatchSection {
            mac_addresses: vec![
                "aa:bb:cc:dd:ee:ff".to_string(),
                "11:22:33:44:55:66".to_string(),
            ],
            ..Default::default()
        };
        assert!(section.matches_interface("eth0", Some("aa:bb:cc:dd:ee:ff"), None, None, None));
        assert!(section.matches_interface("eth0", Some("11:22:33:44:55:66"), None, None, None));
        assert!(!section.matches_interface("eth0", Some("00:00:00:00:00:00"), None, None, None));
    }

    // ── NamePolicy / MACAddressPolicy parsing tests ────────────────────────

    #[test]
    fn test_name_policy_parse() {
        assert_eq!(NamePolicy::parse("kernel"), Some(NamePolicy::Kernel));
        assert_eq!(NamePolicy::parse("database"), Some(NamePolicy::Database));
        assert_eq!(NamePolicy::parse("onboard"), Some(NamePolicy::Onboard));
        assert_eq!(NamePolicy::parse("slot"), Some(NamePolicy::Slot));
        assert_eq!(NamePolicy::parse("path"), Some(NamePolicy::Path));
        assert_eq!(NamePolicy::parse("mac"), Some(NamePolicy::Mac));
        assert_eq!(NamePolicy::parse("keep"), Some(NamePolicy::Keep));
        assert_eq!(NamePolicy::parse("unknown"), None);
    }

    #[test]
    fn test_name_policy_parse_case_insensitive() {
        assert_eq!(NamePolicy::parse("KERNEL"), Some(NamePolicy::Kernel));
        assert_eq!(NamePolicy::parse("Database"), Some(NamePolicy::Database));
        assert_eq!(NamePolicy::parse("ONBOARD"), Some(NamePolicy::Onboard));
    }

    #[test]
    fn test_name_policy_as_str() {
        assert_eq!(NamePolicy::Kernel.as_str(), "kernel");
        assert_eq!(NamePolicy::Database.as_str(), "database");
        assert_eq!(NamePolicy::Onboard.as_str(), "onboard");
        assert_eq!(NamePolicy::Slot.as_str(), "slot");
        assert_eq!(NamePolicy::Path.as_str(), "path");
        assert_eq!(NamePolicy::Mac.as_str(), "mac");
        assert_eq!(NamePolicy::Keep.as_str(), "keep");
    }

    #[test]
    fn test_mac_address_policy_parse() {
        assert_eq!(
            MACAddressPolicy::parse("persistent"),
            Some(MACAddressPolicy::Persistent)
        );
        assert_eq!(
            MACAddressPolicy::parse("random"),
            Some(MACAddressPolicy::Random)
        );
        assert_eq!(
            MACAddressPolicy::parse("none"),
            Some(MACAddressPolicy::None)
        );
        assert_eq!(MACAddressPolicy::parse("bogus"), None);
    }

    #[test]
    fn test_mac_address_policy_parse_case_insensitive() {
        assert_eq!(
            MACAddressPolicy::parse("PERSISTENT"),
            Some(MACAddressPolicy::Persistent)
        );
        assert_eq!(
            MACAddressPolicy::parse("Random"),
            Some(MACAddressPolicy::Random)
        );
    }

    #[test]
    fn test_mac_address_policy_as_str() {
        assert_eq!(MACAddressPolicy::Persistent.as_str(), "persistent");
        assert_eq!(MACAddressPolicy::Random.as_str(), "random");
        assert_eq!(MACAddressPolicy::None.as_str(), "none");
    }

    // ── load_link_configs_from tests ───────────────────────────────────────

    #[test]
    fn test_load_link_configs_from_empty() {
        let dir = tempfile::tempdir().unwrap();
        let configs = load_link_configs_from(&[dir.path().to_path_buf()]);
        assert!(configs.is_empty());
    }

    #[test]
    fn test_load_link_configs_from_with_files() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("10-eth.link"),
            "[Match]\nOriginalName=en*\n\n[Link]\nName=eth0\n",
        )
        .unwrap();
        std::fs::write(
            dir.path().join("20-wlan.link"),
            "[Match]\nOriginalName=wl*\n\n[Link]\nName=wlan0\n",
        )
        .unwrap();
        let configs = load_link_configs_from(&[dir.path().to_path_buf()]);
        assert_eq!(configs.len(), 2);
        assert_eq!(configs[0].link_section.name.as_deref(), Some("eth0"));
        assert_eq!(configs[1].link_section.name.as_deref(), Some("wlan0"));
    }

    #[test]
    fn test_load_link_configs_from_skips_non_link_files() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("10-eth.link"),
            "[Match]\nOriginalName=en*\n\n[Link]\nName=eth0\n",
        )
        .unwrap();
        std::fs::write(
            dir.path().join("10-eth.network"),
            "[Match]\nName=en*\n\n[Network]\nDHCP=yes\n",
        )
        .unwrap();
        std::fs::write(dir.path().join("README"), "not a config\n").unwrap();
        let configs = load_link_configs_from(&[dir.path().to_path_buf()]);
        assert_eq!(configs.len(), 1);
    }

    #[test]
    fn test_load_link_configs_from_dedup_across_dirs() {
        let dir1 = tempfile::tempdir().unwrap();
        let dir2 = tempfile::tempdir().unwrap();
        // Same filename in both dirs — first one wins.
        std::fs::write(dir1.path().join("10-eth.link"), "[Link]\nName=from-dir1\n").unwrap();
        std::fs::write(dir2.path().join("10-eth.link"), "[Link]\nName=from-dir2\n").unwrap();
        let configs =
            load_link_configs_from(&[dir1.path().to_path_buf(), dir2.path().to_path_buf()]);
        assert_eq!(configs.len(), 1);
        assert_eq!(configs[0].link_section.name.as_deref(), Some("from-dir1"));
    }

    #[test]
    fn test_load_link_configs_from_sorted_by_filename() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("90-last.link"), "[Link]\nName=last\n").unwrap();
        std::fs::write(dir.path().join("10-first.link"), "[Link]\nName=first\n").unwrap();
        std::fs::write(dir.path().join("50-middle.link"), "[Link]\nName=middle\n").unwrap();
        let configs = load_link_configs_from(&[dir.path().to_path_buf()]);
        assert_eq!(configs.len(), 3);
        assert_eq!(configs[0].link_section.name.as_deref(), Some("first"));
        assert_eq!(configs[1].link_section.name.as_deref(), Some("middle"));
        assert_eq!(configs[2].link_section.name.as_deref(), Some("last"));
    }

    #[test]
    fn test_load_link_configs_from_nonexistent_dir() {
        let configs = load_link_configs_from(&[PathBuf::from("/nonexistent/dir/for/test")]);
        assert!(configs.is_empty());
    }

    // ── find_matching_link_config tests ────────────────────────────────────

    #[test]
    fn test_find_matching_link_config_by_name() {
        let configs = vec![
            parse_link_file_content(
                "[Match]\nOriginalName=en*\n\n[Link]\nName=eth0\n",
                Path::new("10-eth.link"),
            )
            .unwrap(),
            parse_link_file_content(
                "[Match]\nOriginalName=wl*\n\n[Link]\nName=wlan0\n",
                Path::new("20-wlan.link"),
            )
            .unwrap(),
        ];

        let result = find_matching_link_config(&configs, "enp3s0", None, None, None, None);
        assert!(result.is_some());
        assert_eq!(result.unwrap().link_section.name.as_deref(), Some("eth0"));

        let result = find_matching_link_config(&configs, "wlp2s0", None, None, None, None);
        assert!(result.is_some());
        assert_eq!(result.unwrap().link_section.name.as_deref(), Some("wlan0"));

        let result = find_matching_link_config(&configs, "lo", None, None, None, None);
        assert!(result.is_none());
    }

    #[test]
    fn test_find_matching_link_config_first_match_wins() {
        let configs = vec![
            parse_link_file_content(
                "[Match]\nOriginalName=en*\n\n[Link]\nName=first\n",
                Path::new("10-a.link"),
            )
            .unwrap(),
            parse_link_file_content(
                "[Match]\nOriginalName=en*\n\n[Link]\nName=second\n",
                Path::new("20-b.link"),
            )
            .unwrap(),
        ];

        let result = find_matching_link_config(&configs, "enp3s0", None, None, None, None);
        assert_eq!(result.unwrap().link_section.name.as_deref(), Some("first"));
    }

    #[test]
    fn test_find_matching_link_config_with_mac() {
        let configs = vec![
            parse_link_file_content(
                "[Match]\nMACAddress=aa:bb:cc:dd:ee:ff\n\n[Link]\nName=mac-match\n",
                Path::new("10-mac.link"),
            )
            .unwrap(),
        ];

        let result = find_matching_link_config(
            &configs,
            "eth0",
            Some("aa:bb:cc:dd:ee:ff"),
            None,
            None,
            None,
        );
        assert!(result.is_some());
        assert_eq!(
            result.unwrap().link_section.name.as_deref(),
            Some("mac-match")
        );

        let result = find_matching_link_config(
            &configs,
            "eth0",
            Some("00:00:00:00:00:00"),
            None,
            None,
            None,
        );
        assert!(result.is_none());
    }

    #[test]
    fn test_find_matching_link_config_no_match_section_matches_all() {
        let configs = vec![
            parse_link_file_content("[Link]\nName=fallback\n", Path::new("99-fallback.link"))
                .unwrap(),
        ];

        let result = find_matching_link_config(&configs, "anything", None, None, None, None);
        assert!(result.is_some());
        assert_eq!(
            result.unwrap().link_section.name.as_deref(),
            Some("fallback")
        );
    }

    #[test]
    fn test_find_matching_link_config_empty_list() {
        let configs: Vec<LinkFileConfig> = Vec::new();
        let result = find_matching_link_config(&configs, "eth0", None, None, None, None);
        assert!(result.is_none());
    }

    // ── NixOS-style .link file test ────────────────────────────────────────

    #[test]
    fn test_parse_nixos_style_link_file() {
        // NixOS generates .link files like this for predictable naming.
        let content = r#"
[Match]
OriginalName=*

[Link]
NamePolicy=keep kernel database onboard slot path
MACAddressPolicy=persistent
"#;
        let cfg = parse_link_file_content(content, Path::new("99-default.link")).unwrap();
        assert_eq!(cfg.match_section.original_names, vec!["*"]);
        assert_eq!(
            cfg.link_section.name_policy,
            vec![
                NamePolicy::Keep,
                NamePolicy::Kernel,
                NamePolicy::Database,
                NamePolicy::Onboard,
                NamePolicy::Slot,
                NamePolicy::Path,
            ]
        );
        assert_eq!(
            cfg.link_section.mac_address_policy,
            Some(MACAddressPolicy::Persistent)
        );
        // The wildcard [Match] should match any interface.
        assert!(
            cfg.match_section
                .matches_interface("eth0", None, None, None, None)
        );
        assert!(
            cfg.match_section
                .matches_interface("enp3s0", None, None, None, None)
        );
    }

    // ── name_policy unknown values filtered ────────────────────────────────

    #[test]
    fn test_parse_link_file_name_policy_unknown_filtered() {
        let content = r#"
[Link]
NamePolicy=kernel bogus path
"#;
        let cfg = parse_link_file_content(content, Path::new("test.link")).unwrap();
        assert_eq!(
            cfg.link_section.name_policy,
            vec![NamePolicy::Kernel, NamePolicy::Path]
        );
    }
}
