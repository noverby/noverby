//! IPv6 Router Advertisement (RA) handling and SLAAC address generation.
//!
//! This module implements:
//! - ICMPv6 Router Solicitation (RS) message construction and sending
//! - ICMPv6 Router Advertisement (RA) message parsing
//! - RA option parsing (Prefix Information, RDNSS, Route Information, MTU)
//! - SLAAC (Stateless Address Autoconfiguration) address generation via EUI-64
//! - IPv6 link-local address generation from MAC
//! - State tracking for received RAs per link

use std::fmt;
use std::net::Ipv6Addr;
use std::time::{Duration, Instant};

// ---------------------------------------------------------------------------
// ICMPv6 constants (RFC 4861, RFC 8106)
// ---------------------------------------------------------------------------

/// ICMPv6 type: Router Solicitation
const ICMPV6_TYPE_RS: u8 = 133;
/// ICMPv6 type: Router Advertisement
const ICMPV6_TYPE_RA: u8 = 134;

/// IPv6 all-routers multicast address (ff02::2)
const ALL_ROUTERS_MULTICAST: Ipv6Addr = Ipv6Addr::new(0xff02, 0, 0, 0, 0, 0, 0, 2);

/// NDP option type: Source Link-Layer Address
const NDP_OPT_SOURCE_LL_ADDR: u8 = 1;
/// NDP option type: Target Link-Layer Address
const NDP_OPT_TARGET_LL_ADDR: u8 = 2;
/// NDP option type: Prefix Information
const NDP_OPT_PREFIX_INFO: u8 = 3;
/// NDP option type: Redirected Header
const NDP_OPT_REDIRECTED_HEADER: u8 = 4;
/// NDP option type: MTU
const NDP_OPT_MTU: u8 = 5;
/// NDP option type: Route Information (RFC 4191)
const NDP_OPT_ROUTE_INFO: u8 = 24;
/// NDP option type: Recursive DNS Server (RFC 8106)
const NDP_OPT_RDNSS: u8 = 25;
/// NDP option type: DNS Search List (RFC 8106)
const NDP_OPT_DNSSL: u8 = 31;

/// Prefix Information option flag: On-link (L)
const PREFIX_FLAG_ON_LINK: u8 = 0x80;
/// Prefix Information option flag: Autonomous address-configuration (A)
const PREFIX_FLAG_AUTONOMOUS: u8 = 0x40;

/// Default Router Solicitation retransmit interval (4 seconds, RFC 4861 §6.3.7)
const RS_RETRANSMIT_INTERVAL: Duration = Duration::from_secs(4);
/// Maximum number of RS retransmissions (3, RFC 4861 §10)
const MAX_RS_RETRANSMISSIONS: u32 = 3;

/// Route protocol for RA-learned routes
const RTPROT_RA: u8 = 9;

// ---------------------------------------------------------------------------
// RA option types
// ---------------------------------------------------------------------------

/// Parsed Prefix Information option from a Router Advertisement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrefixInfo {
    /// Prefix length in bits (typically 64 for SLAAC).
    pub prefix_len: u8,
    /// On-link flag (L) — prefix is on-link for the interface.
    pub on_link: bool,
    /// Autonomous flag (A) — prefix can be used for SLAAC.
    pub autonomous: bool,
    /// Valid lifetime in seconds (0xFFFFFFFF = infinity).
    pub valid_lifetime: u32,
    /// Preferred lifetime in seconds (0xFFFFFFFF = infinity).
    pub preferred_lifetime: u32,
    /// The IPv6 prefix (network portion).
    pub prefix: Ipv6Addr,
}

/// Parsed Recursive DNS Server (RDNSS) option from a Router Advertisement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RdnssInfo {
    /// Lifetime in seconds (0 means remove).
    pub lifetime: u32,
    /// DNS server IPv6 addresses.
    pub servers: Vec<Ipv6Addr>,
}

/// Parsed DNS Search List (DNSSL) option from a Router Advertisement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DnsslInfo {
    /// Lifetime in seconds (0 means remove).
    pub lifetime: u32,
    /// Search domain names.
    pub domains: Vec<String>,
}

/// Parsed Route Information option (RFC 4191).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RouteInfo {
    /// Prefix length in bits.
    pub prefix_len: u8,
    /// Route preference (0 = medium, 1 = high, 3 = low).
    pub preference: u8,
    /// Route lifetime in seconds.
    pub lifetime: u32,
    /// Route prefix.
    pub prefix: Ipv6Addr,
}

/// Parsed MTU option.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MtuOption {
    /// Recommended MTU value.
    pub mtu: u32,
}

/// A fully parsed Router Advertisement message.
#[derive(Debug, Clone)]
pub struct RouterAdvertisement {
    /// Current hop limit recommended by the router (0 = unspecified).
    pub cur_hop_limit: u8,
    /// Managed address configuration flag (M).
    pub managed: bool,
    /// Other configuration flag (O).
    pub other: bool,
    /// Router lifetime in seconds (0 = not a default router).
    pub router_lifetime: u16,
    /// Reachable time in milliseconds (0 = unspecified).
    pub reachable_time: u32,
    /// Retransmit timer in milliseconds (0 = unspecified).
    pub retrans_timer: u32,
    /// Source (router) IPv6 address.
    pub source: Ipv6Addr,
    /// Prefix Information options.
    pub prefixes: Vec<PrefixInfo>,
    /// RDNSS options.
    pub rdnss: Vec<RdnssInfo>,
    /// DNSSL options.
    pub dnssl: Vec<DnsslInfo>,
    /// Route Information options.
    pub routes: Vec<RouteInfo>,
    /// MTU option (if present).
    pub mtu: Option<MtuOption>,
    /// Source link-layer address (if present).
    pub source_ll_addr: Option<[u8; 6]>,
}

impl fmt::Display for RouterAdvertisement {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "RA from {} (hop_limit={}, lifetime={}s, managed={}, other={}, {} prefix(es), {} RDNSS, {} route(s))",
            self.source,
            self.cur_hop_limit,
            self.router_lifetime,
            self.managed,
            self.other,
            self.prefixes.len(),
            self.rdnss.iter().map(|r| r.servers.len()).sum::<usize>(),
            self.routes.len(),
        )
    }
}

// ---------------------------------------------------------------------------
// RA state tracking per link
// ---------------------------------------------------------------------------

/// Per-link state for IPv6 Router Advertisement processing.
#[derive(Debug)]
pub struct RaState {
    /// Interface index.
    pub ifindex: u32,
    /// Interface name.
    pub ifname: String,
    /// Interface MAC address (6 bytes).
    pub mac: [u8; 6],
    /// Whether RA is enabled for this link.
    pub enabled: bool,
    /// Number of RS messages sent.
    pub rs_count: u32,
    /// When the last RS was sent.
    pub last_rs: Option<Instant>,
    /// Whether we have received at least one RA.
    pub ra_received: bool,
    /// The most recently received RA.
    pub last_ra: Option<RouterAdvertisement>,
    /// SLAAC addresses that we have configured.
    pub slaac_addresses: Vec<(Ipv6Addr, u8)>,
    /// Default router address (if any).
    pub default_router: Option<Ipv6Addr>,
    /// DNS servers learned from RDNSS.
    pub dns_servers: Vec<Ipv6Addr>,
    /// Search domains learned from DNSSL.
    pub search_domains: Vec<String>,
    /// Link-local address for this interface.
    pub link_local: Option<Ipv6Addr>,
}

impl RaState {
    /// Create a new RA state for an interface.
    pub fn new(ifindex: u32, ifname: String, mac: [u8; 6]) -> Self {
        let link_local = mac_to_link_local(&mac);
        Self {
            ifindex,
            ifname,
            mac,
            enabled: true,
            rs_count: 0,
            last_rs: None,
            ra_received: false,
            last_ra: None,
            slaac_addresses: Vec::new(),
            default_router: None,
            dns_servers: Vec::new(),
            search_domains: Vec::new(),
            link_local: Some(link_local),
        }
    }

    /// Check if we should send another Router Solicitation.
    pub fn should_send_rs(&self) -> bool {
        if !self.enabled || self.ra_received {
            return false;
        }
        if self.rs_count >= MAX_RS_RETRANSMISSIONS {
            return false;
        }
        match self.last_rs {
            None => true,
            Some(last) => last.elapsed() >= RS_RETRANSMIT_INTERVAL,
        }
    }

    /// Record that we sent an RS.
    pub fn mark_rs_sent(&mut self) {
        self.rs_count += 1;
        self.last_rs = Some(Instant::now());
    }

    /// Process a received Router Advertisement and update state.
    ///
    /// Returns a list of `RaAction` describing what the caller should do
    /// (add addresses, add routes, update DNS, etc.).
    pub fn process_ra(&mut self, ra: RouterAdvertisement) -> Vec<RaAction> {
        let mut actions = Vec::new();
        self.ra_received = true;

        // Default router
        if ra.router_lifetime > 0 {
            self.default_router = Some(ra.source);
            actions.push(RaAction::AddDefaultRoute {
                gateway: ra.source,
                lifetime: ra.router_lifetime,
            });
        } else if self.default_router == Some(ra.source) {
            // Router lifetime 0 means this router is no longer a default router.
            self.default_router = None;
            actions.push(RaAction::RemoveDefaultRoute { gateway: ra.source });
        }

        // Prefix Information — SLAAC
        for prefix in &ra.prefixes {
            if prefix.autonomous
                && prefix.prefix_len == 64
                && prefix.valid_lifetime > 0
                && let Some(addr) = slaac_eui64(&prefix.prefix, &self.mac)
            {
                let already = self.slaac_addresses.iter().any(|(a, _)| *a == addr);
                if !already {
                    self.slaac_addresses.push((addr, prefix.prefix_len));
                }
                actions.push(RaAction::AddAddress {
                    address: addr,
                    prefix_len: prefix.prefix_len,
                    valid_lifetime: prefix.valid_lifetime,
                    preferred_lifetime: prefix.preferred_lifetime,
                });
            }

            if prefix.on_link && prefix.valid_lifetime > 0 {
                actions.push(RaAction::AddOnLinkRoute {
                    prefix: prefix.prefix,
                    prefix_len: prefix.prefix_len,
                    lifetime: prefix.valid_lifetime,
                });
            }
        }

        // Route Information options
        for route in &ra.routes {
            if route.lifetime > 0 {
                actions.push(RaAction::AddRoute {
                    prefix: route.prefix,
                    prefix_len: route.prefix_len,
                    gateway: ra.source,
                    lifetime: route.lifetime,
                    preference: route.preference,
                });
            }
        }

        // RDNSS
        let mut new_dns = Vec::new();
        for rdnss in &ra.rdnss {
            if rdnss.lifetime > 0 {
                for server in &rdnss.servers {
                    if !new_dns.contains(server) {
                        new_dns.push(*server);
                    }
                }
            }
        }
        if !new_dns.is_empty() {
            self.dns_servers = new_dns.clone();
            actions.push(RaAction::UpdateDns { servers: new_dns });
        }

        // DNSSL
        let mut new_domains = Vec::new();
        for dnssl in &ra.dnssl {
            if dnssl.lifetime > 0 {
                for domain in &dnssl.domains {
                    if !new_domains.contains(domain) {
                        new_domains.push(domain.clone());
                    }
                }
            }
        }
        if !new_domains.is_empty() {
            self.search_domains = new_domains.clone();
            actions.push(RaAction::UpdateSearchDomains {
                domains: new_domains,
            });
        }

        // MTU
        if let Some(ref mtu_opt) = ra.mtu
            && mtu_opt.mtu >= 1280
        {
            actions.push(RaAction::SetMtu { mtu: mtu_opt.mtu });
        }

        self.last_ra = Some(ra);
        actions
    }
}

/// Action that the network manager should take after processing an RA.
#[derive(Debug, Clone, PartialEq)]
pub enum RaAction {
    /// Add a SLAAC address to the interface.
    AddAddress {
        address: Ipv6Addr,
        prefix_len: u8,
        valid_lifetime: u32,
        preferred_lifetime: u32,
    },
    /// Add a default route via the given gateway.
    AddDefaultRoute { gateway: Ipv6Addr, lifetime: u16 },
    /// Remove a default route via the given gateway.
    RemoveDefaultRoute { gateway: Ipv6Addr },
    /// Add an on-link prefix route.
    AddOnLinkRoute {
        prefix: Ipv6Addr,
        prefix_len: u8,
        lifetime: u32,
    },
    /// Add a more-specific route from Route Information option.
    AddRoute {
        prefix: Ipv6Addr,
        prefix_len: u8,
        gateway: Ipv6Addr,
        lifetime: u32,
        preference: u8,
    },
    /// Update DNS servers from RDNSS.
    UpdateDns { servers: Vec<Ipv6Addr> },
    /// Update search domains from DNSSL.
    UpdateSearchDomains { domains: Vec<String> },
    /// Set MTU on the interface.
    SetMtu { mtu: u32 },
}

// ---------------------------------------------------------------------------
// ICMPv6 message construction
// ---------------------------------------------------------------------------

/// Build a Router Solicitation (RS) message (ICMPv6 type 133).
///
/// The RS message format (RFC 4861 §4.1):
///   - Type (1 byte): 133
///   - Code (1 byte): 0
///   - Checksum (2 bytes): 0 (kernel computes for raw sockets)
///   - Reserved (4 bytes): 0
///   - Options: Source Link-Layer Address (optional)
///
/// Returns the raw ICMPv6 payload (the kernel handles the IPv6 header and
/// ICMPv6 checksum when using IPPROTO_ICMPV6 raw sockets).
pub fn build_rs(mac: Option<&[u8; 6]>) -> Vec<u8> {
    // Base RS: type(1) + code(1) + checksum(2) + reserved(4) = 8 bytes
    let opt_len = if mac.is_some() { 8 } else { 0 }; // option type(1) + len(1) + mac(6)
    let mut msg = vec![0u8; 8 + opt_len];

    msg[0] = ICMPV6_TYPE_RS; // Type
    msg[1] = 0; // Code
    // Checksum bytes [2..4] left as 0 — kernel fills in for IPPROTO_ICMPV6.
    // Reserved bytes [4..8] left as 0.

    // Source Link-Layer Address option
    if let Some(mac) = mac {
        msg[8] = NDP_OPT_SOURCE_LL_ADDR; // Option type
        msg[9] = 1; // Length in units of 8 bytes
        msg[10..16].copy_from_slice(mac);
    }

    msg
}

// ---------------------------------------------------------------------------
// RA message parsing
// ---------------------------------------------------------------------------

/// Parse a Router Advertisement message from raw ICMPv6 payload.
///
/// `source` is the IPv6 source address from the IPv6 header (provided
/// by the caller since we receive it from `recvfrom`).
///
/// The RA message format (RFC 4861 §4.2):
///   - Type (1 byte): 134
///   - Code (1 byte): 0
///   - Checksum (2 bytes)
///   - Cur Hop Limit (1 byte)
///   - M|O|H|Prf|P|Reserved (1 byte)
///   - Router Lifetime (2 bytes)
///   - Reachable Time (4 bytes)
///   - Retrans Timer (4 bytes)
///   - Options (variable)
pub fn parse_ra(data: &[u8], source: Ipv6Addr) -> Option<RouterAdvertisement> {
    // Minimum RA size: 16 bytes (8 fixed header + 8 for basic fields)
    // Actually: type(1) + code(1) + checksum(2) + hop_limit(1) + flags(1) + lifetime(2)
    //         + reachable(4) + retrans(4) = 16 bytes
    if data.len() < 16 {
        return None;
    }

    // Verify ICMPv6 type
    if data[0] != ICMPV6_TYPE_RA {
        return None;
    }

    let cur_hop_limit = data[4];
    let flags = data[5];
    let managed = (flags & 0x80) != 0;
    let other = (flags & 0x40) != 0;
    let router_lifetime = u16::from_be_bytes([data[6], data[7]]);
    let reachable_time = u32::from_be_bytes([data[8], data[9], data[10], data[11]]);
    let retrans_timer = u32::from_be_bytes([data[12], data[13], data[14], data[15]]);

    let mut ra = RouterAdvertisement {
        cur_hop_limit,
        managed,
        other,
        router_lifetime,
        reachable_time,
        retrans_timer,
        source,
        prefixes: Vec::new(),
        rdnss: Vec::new(),
        dnssl: Vec::new(),
        routes: Vec::new(),
        mtu: None,
        source_ll_addr: None,
    };

    // Parse options starting at offset 16
    parse_ra_options(&mut ra, &data[16..]);

    Some(ra)
}

/// Parse NDP options from a Router Advertisement.
fn parse_ra_options(ra: &mut RouterAdvertisement, mut data: &[u8]) {
    while data.len() >= 2 {
        let opt_type = data[0];
        let opt_len_units = data[1] as usize; // Length in units of 8 bytes

        if opt_len_units == 0 {
            // Length of 0 is invalid — would cause infinite loop.
            break;
        }

        let opt_len_bytes = opt_len_units * 8;
        if opt_len_bytes > data.len() {
            break;
        }

        let opt_data = &data[..opt_len_bytes];

        match opt_type {
            NDP_OPT_PREFIX_INFO => {
                if let Some(prefix) = parse_prefix_info(opt_data) {
                    ra.prefixes.push(prefix);
                }
            }
            NDP_OPT_MTU => {
                if let Some(mtu) = parse_mtu_option(opt_data) {
                    ra.mtu = Some(mtu);
                }
            }
            NDP_OPT_RDNSS => {
                if let Some(rdnss) = parse_rdnss(opt_data) {
                    ra.rdnss.push(rdnss);
                }
            }
            NDP_OPT_DNSSL => {
                if let Some(dnssl) = parse_dnssl(opt_data) {
                    ra.dnssl.push(dnssl);
                }
            }
            NDP_OPT_ROUTE_INFO => {
                if let Some(route) = parse_route_info(opt_data) {
                    ra.routes.push(route);
                }
            }
            NDP_OPT_SOURCE_LL_ADDR => {
                if opt_len_bytes >= 8 {
                    let mut mac = [0u8; 6];
                    mac.copy_from_slice(&opt_data[2..8]);
                    ra.source_ll_addr = Some(mac);
                }
            }
            _ => {
                // Unknown option — skip.
            }
        }

        data = &data[opt_len_bytes..];
    }
}

/// Parse a Prefix Information option (RFC 4861 §4.6.2).
///
/// Format (32 bytes total):
///   - Type (1): 3
///   - Length (1): 4 (= 32 bytes)
///   - Prefix Length (1)
///   - L|A|R|Reserved1 (1)
///   - Valid Lifetime (4)
///   - Preferred Lifetime (4)
///   - Reserved2 (4)
///   - Prefix (16)
fn parse_prefix_info(data: &[u8]) -> Option<PrefixInfo> {
    if data.len() < 32 {
        return None;
    }

    let prefix_len = data[2];
    let flags = data[3];
    let on_link = (flags & PREFIX_FLAG_ON_LINK) != 0;
    let autonomous = (flags & PREFIX_FLAG_AUTONOMOUS) != 0;
    let valid_lifetime = u32::from_be_bytes([data[4], data[5], data[6], data[7]]);
    let preferred_lifetime = u32::from_be_bytes([data[8], data[9], data[10], data[11]]);
    // Reserved2 at [12..16]
    let prefix = parse_ipv6(&data[16..32])?;

    Some(PrefixInfo {
        prefix_len,
        on_link,
        autonomous,
        valid_lifetime,
        preferred_lifetime,
        prefix,
    })
}

/// Parse an MTU option (RFC 4861 §4.6.4).
///
/// Format (8 bytes):
///   - Type (1): 5
///   - Length (1): 1 (= 8 bytes)
///   - Reserved (2)
///   - MTU (4)
fn parse_mtu_option(data: &[u8]) -> Option<MtuOption> {
    if data.len() < 8 {
        return None;
    }
    let mtu = u32::from_be_bytes([data[4], data[5], data[6], data[7]]);
    Some(MtuOption { mtu })
}

/// Parse an RDNSS option (RFC 8106 §5.1).
///
/// Format (8 + N*16 bytes):
///   - Type (1): 25
///   - Length (1): 1 + 2*N (N = number of addresses)
///   - Reserved (2)
///   - Lifetime (4)
///   - Addresses (N * 16 bytes)
fn parse_rdnss(data: &[u8]) -> Option<RdnssInfo> {
    if data.len() < 24 {
        // Minimum: 8 header + 16 for at least one address
        return None;
    }
    let lifetime = u32::from_be_bytes([data[4], data[5], data[6], data[7]]);

    let addr_data = &data[8..];
    let num_addrs = addr_data.len() / 16;
    let mut servers = Vec::with_capacity(num_addrs);

    for i in 0..num_addrs {
        let off = i * 16;
        if off + 16 > addr_data.len() {
            break;
        }
        if let Some(addr) = parse_ipv6(&addr_data[off..off + 16]) {
            servers.push(addr);
        }
    }

    if servers.is_empty() {
        return None;
    }

    Some(RdnssInfo { lifetime, servers })
}

/// Parse a DNSSL option (RFC 8106 §5.2).
///
/// Format (variable):
///   - Type (1): 31
///   - Length (1)
///   - Reserved (2)
///   - Lifetime (4)
///   - Domain Names (variable, DNS name encoding)
fn parse_dnssl(data: &[u8]) -> Option<DnsslInfo> {
    if data.len() < 16 {
        return None;
    }
    let lifetime = u32::from_be_bytes([data[4], data[5], data[6], data[7]]);

    let name_data = &data[8..];
    let domains = parse_dns_names(name_data);

    if domains.is_empty() {
        return None;
    }

    Some(DnsslInfo { lifetime, domains })
}

/// Parse DNS-encoded domain names from DNSSL option payload.
///
/// Names are encoded as sequences of labels (length byte + ASCII),
/// terminated by a zero-length label. Multiple names are concatenated.
fn parse_dns_names(data: &[u8]) -> Vec<String> {
    let mut domains = Vec::new();
    let mut offset = 0;

    while offset < data.len() {
        let mut labels: Vec<String> = Vec::new();
        loop {
            if offset >= data.len() {
                break;
            }
            let label_len = data[offset] as usize;
            offset += 1;

            if label_len == 0 {
                break; // End of this name
            }
            if offset + label_len > data.len() {
                break; // Truncated
            }

            if let Ok(label) = std::str::from_utf8(&data[offset..offset + label_len]) {
                labels.push(label.to_string());
            }
            offset += label_len;
        }

        if !labels.is_empty() {
            domains.push(labels.join("."));
        }

        // If remaining bytes are all zero, stop.
        if offset < data.len() && data[offset..].iter().all(|&b| b == 0) {
            break;
        }
    }

    domains
}

/// Parse a Route Information option (RFC 4191 §2.3).
///
/// Format (8, 16, or 24 bytes):
///   - Type (1): 24
///   - Length (1): 1, 2, or 3
///   - Prefix Length (1)
///   - Resvd|Prf|Resvd (1)
///   - Route Lifetime (4)
///   - Prefix (0, 8, or 16 bytes depending on Prefix Length)
fn parse_route_info(data: &[u8]) -> Option<RouteInfo> {
    if data.len() < 8 {
        return None;
    }

    let prefix_len = data[2];
    let preference = (data[3] >> 3) & 0x03;
    let lifetime = u32::from_be_bytes([data[4], data[5], data[6], data[7]]);

    // The prefix field length depends on Prefix Length:
    // - 0..=0: 0 bytes  (Length=1, total 8)
    // - 1..=64: 8 bytes (Length=2, total 16)
    // - 65..=128: 16 bytes (Length=3, total 24)
    let mut prefix_bytes = [0u8; 16];
    if prefix_len > 0 {
        let needed = if prefix_len <= 64 { 8 } else { 16 };
        if data.len() < 8 + needed {
            return None;
        }
        let available = std::cmp::min(needed, data.len() - 8);
        prefix_bytes[..available].copy_from_slice(&data[8..8 + available]);
    }

    let prefix = Ipv6Addr::from(prefix_bytes);

    Some(RouteInfo {
        prefix_len,
        preference,
        lifetime,
        prefix,
    })
}

/// Parse 16 bytes as an IPv6 address.
fn parse_ipv6(data: &[u8]) -> Option<Ipv6Addr> {
    if data.len() < 16 {
        return None;
    }
    let octets: [u8; 16] = data[..16].try_into().ok()?;
    Some(Ipv6Addr::from(octets))
}

// ---------------------------------------------------------------------------
// SLAAC address generation
// ---------------------------------------------------------------------------

/// Generate an IPv6 link-local address from a MAC address using EUI-64.
///
/// Link-local address format: `fe80::<eui64>`
///
/// EUI-64 is constructed by inserting `ff:fe` in the middle of the MAC
/// and flipping the Universal/Local bit (bit 1 of the first byte).
pub fn mac_to_link_local(mac: &[u8; 6]) -> Ipv6Addr {
    let iid = mac_to_eui64(mac);
    Ipv6Addr::new(
        0xfe80,
        0,
        0,
        0,
        u16::from_be_bytes([iid[0], iid[1]]),
        u16::from_be_bytes([iid[2], iid[3]]),
        u16::from_be_bytes([iid[4], iid[5]]),
        u16::from_be_bytes([iid[6], iid[7]]),
    )
}

/// Generate an EUI-64 interface identifier from a 6-byte MAC address.
///
/// EUI-64 = MAC[0..3] ++ FF:FE ++ MAC[3..6], with the U/L bit flipped.
pub fn mac_to_eui64(mac: &[u8; 6]) -> [u8; 8] {
    let mut iid = [0u8; 8];
    iid[0] = mac[0] ^ 0x02; // Flip U/L bit
    iid[1] = mac[1];
    iid[2] = mac[2];
    iid[3] = 0xff;
    iid[4] = 0xfe;
    iid[5] = mac[3];
    iid[6] = mac[4];
    iid[7] = mac[5];
    iid
}

/// Generate a SLAAC address from a /64 prefix and a MAC address using EUI-64.
///
/// Returns `None` if the prefix length is not 64 or the resulting address
/// is not a valid global unicast address.
pub fn slaac_eui64(prefix: &Ipv6Addr, mac: &[u8; 6]) -> Option<Ipv6Addr> {
    let prefix_bytes = prefix.octets();
    let iid = mac_to_eui64(mac);

    let mut addr_bytes = [0u8; 16];
    addr_bytes[..8].copy_from_slice(&prefix_bytes[..8]);
    addr_bytes[8..].copy_from_slice(&iid);

    let addr = Ipv6Addr::from(addr_bytes);

    // Sanity check: don't generate loopback, multicast, or unspecified addresses.
    if addr.is_loopback() || addr.is_multicast() || addr.is_unspecified() {
        return None;
    }

    Some(addr)
}

// ---------------------------------------------------------------------------
// ICMPv6 socket operations
// ---------------------------------------------------------------------------

/// Open an ICMPv6 raw socket for Router Solicitation/Advertisement.
///
/// Returns the raw file descriptor, or an error.
///
/// The socket is set up with:
/// - `IPPROTO_ICMPV6` (protocol 58)
/// - Hop limit 255 (required by RFC 4861 §6.1.2)
/// - ICMPv6 filter to only receive RA messages (type 134)
/// - Bound to the specified interface via `SO_BINDTODEVICE`
pub fn open_icmpv6_socket(ifname: &str) -> Result<i32, String> {
    unsafe {
        let fd = libc::socket(libc::AF_INET6, libc::SOCK_RAW, libc::IPPROTO_ICMPV6);
        if fd < 0 {
            return Err(format!(
                "socket(AF_INET6, SOCK_RAW, IPPROTO_ICMPV6): {}",
                std::io::Error::last_os_error()
            ));
        }

        // Set hop limit to 255 (required by NDP).
        let hoplimit: libc::c_int = 255;
        if libc::setsockopt(
            fd,
            libc::IPPROTO_IPV6,
            libc::IPV6_MULTICAST_HOPS,
            &hoplimit as *const _ as *const libc::c_void,
            std::mem::size_of::<libc::c_int>() as libc::socklen_t,
        ) < 0
        {
            let err = std::io::Error::last_os_error();
            libc::close(fd);
            return Err(format!("setsockopt(IPV6_MULTICAST_HOPS): {err}"));
        }

        let unicast_hops: libc::c_int = 255;
        if libc::setsockopt(
            fd,
            libc::IPPROTO_IPV6,
            libc::IPV6_UNICAST_HOPS,
            &unicast_hops as *const _ as *const libc::c_void,
            std::mem::size_of::<libc::c_int>() as libc::socklen_t,
        ) < 0
        {
            let err = std::io::Error::last_os_error();
            libc::close(fd);
            return Err(format!("setsockopt(IPV6_UNICAST_HOPS): {err}"));
        }

        // Enable receiving packet info (to know which interface RA arrived on).
        let yes: libc::c_int = 1;
        if libc::setsockopt(
            fd,
            libc::IPPROTO_IPV6,
            libc::IPV6_RECVPKTINFO,
            &yes as *const _ as *const libc::c_void,
            std::mem::size_of::<libc::c_int>() as libc::socklen_t,
        ) < 0
        {
            let err = std::io::Error::last_os_error();
            libc::close(fd);
            return Err(format!("setsockopt(IPV6_RECVPKTINFO): {err}"));
        }

        // Enable receiving hop limit (to validate hop limit == 255).
        if libc::setsockopt(
            fd,
            libc::IPPROTO_IPV6,
            libc::IPV6_RECVHOPLIMIT,
            &yes as *const _ as *const libc::c_void,
            std::mem::size_of::<libc::c_int>() as libc::socklen_t,
        ) < 0
        {
            let err = std::io::Error::last_os_error();
            libc::close(fd);
            return Err(format!("setsockopt(IPV6_RECVHOPLIMIT): {err}"));
        }

        // Set ICMPv6 filter — only accept RA (type 134).
        // struct icmp6_filter is 32 bytes (256 bits, one per ICMPv6 type).
        // ICMP6_FILTER is not always exported by libc, so define it here.
        // Value is 1 on Linux (from <netinet/icmp6.h>).
        const ICMP6_FILTER: libc::c_int = 1;
        let mut filter = [0xFFu8; 32]; // Block all
        // Clear bit for type 134 to allow it.
        let icmpv6_type = ICMPV6_TYPE_RA as usize;
        filter[icmpv6_type / 8] &= !(1 << (icmpv6_type % 8));
        if libc::setsockopt(
            fd,
            libc::IPPROTO_ICMPV6,
            ICMP6_FILTER,
            filter.as_ptr() as *const libc::c_void,
            filter.len() as libc::socklen_t,
        ) < 0
        {
            let err = std::io::Error::last_os_error();
            libc::close(fd);
            return Err(format!("setsockopt(ICMP6_FILTER): {err}"));
        }

        // Bind to interface.
        let ifname_c = std::ffi::CString::new(ifname).map_err(|e| format!("CString: {e}"))?;
        if libc::setsockopt(
            fd,
            libc::SOL_SOCKET,
            libc::SO_BINDTODEVICE,
            ifname_c.as_ptr() as *const libc::c_void,
            ifname_c.as_bytes_with_nul().len() as libc::socklen_t,
        ) < 0
        {
            let err = std::io::Error::last_os_error();
            libc::close(fd);
            return Err(format!("setsockopt(SO_BINDTODEVICE, {ifname}): {err}"));
        }

        // Set non-blocking.
        let flags = libc::fcntl(fd, libc::F_GETFL);
        if flags >= 0 {
            libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK);
        }

        Ok(fd)
    }
}

/// Send a Router Solicitation on the given ICMPv6 socket.
///
/// The RS is sent to the all-routers multicast address (ff02::2).
pub fn send_rs(fd: i32, mac: &[u8; 6]) -> Result<(), String> {
    let msg = build_rs(Some(mac));

    let dst = ALL_ROUTERS_MULTICAST.octets();
    let mut sockaddr: libc::sockaddr_in6 = unsafe { std::mem::zeroed() };
    sockaddr.sin6_family = libc::AF_INET6 as libc::sa_family_t;
    sockaddr.sin6_addr.s6_addr = dst;

    let ret = unsafe {
        libc::sendto(
            fd,
            msg.as_ptr() as *const libc::c_void,
            msg.len(),
            0,
            &sockaddr as *const _ as *const libc::sockaddr,
            std::mem::size_of::<libc::sockaddr_in6>() as libc::socklen_t,
        )
    };

    if ret < 0 {
        Err(format!(
            "sendto(ff02::2): {}",
            std::io::Error::last_os_error()
        ))
    } else {
        Ok(())
    }
}

/// Receive a Router Advertisement from the ICMPv6 socket.
///
/// Returns `Some((ra_data, source_addr))` if a packet was available,
/// or `None` if the socket has no pending data (non-blocking).
pub fn recv_ra(fd: i32) -> Option<(Vec<u8>, Ipv6Addr)> {
    let mut buf = [0u8; 2048];
    let mut src_addr: libc::sockaddr_in6 = unsafe { std::mem::zeroed() };
    let mut addrlen = std::mem::size_of::<libc::sockaddr_in6>() as libc::socklen_t;

    let n = unsafe {
        libc::recvfrom(
            fd,
            buf.as_mut_ptr() as *mut libc::c_void,
            buf.len(),
            0,
            &mut src_addr as *mut _ as *mut libc::sockaddr,
            &mut addrlen,
        )
    };

    if n <= 0 {
        return None;
    }

    let data = buf[..n as usize].to_vec();
    let source = Ipv6Addr::from(src_addr.sin6_addr.s6_addr);

    Some((data, source))
}

// ---------------------------------------------------------------------------
// IPv6 netlink operations
// ---------------------------------------------------------------------------

/// Add an IPv6 address to an interface via netlink.
pub fn add_ipv6_address(ifindex: u32, address: Ipv6Addr, prefix_len: u8) -> std::io::Result<()> {
    use crate::link::NetlinkSocket;

    const NLMSG_HDR_LEN: usize = 16;
    const IFADDRMSG_LEN: usize = 8;
    const AF_INET6: u8 = 10;
    const IFA_ADDRESS: u16 = 1;
    const NLM_F_REQUEST: u16 = 0x0001;
    const NLM_F_ACK: u16 = 0x0004;
    const NLM_F_CREATE: u16 = 0x0400;
    const NLM_F_REPLACE: u16 = 0x0100;
    const RTM_NEWADDR: u16 = 20;
    const RT_SCOPE_UNIVERSE: u8 = 0;

    let mut nl = NetlinkSocket::open()?;
    let seq = nl.next_seq();

    // IFA_ADDRESS with 16-byte IPv6 payload: 4 header + 16 payload = 20, aligned to 20
    let addr_attr_len = (4 + 16 + 3) & !3; // rta_aligned_len(16)
    let msg_len = NLMSG_HDR_LEN + IFADDRMSG_LEN + addr_attr_len;
    let aligned_len = (msg_len + 3) & !3;
    let mut msg = vec![0u8; aligned_len];

    // nlmsghdr
    msg[0..4].copy_from_slice(&(msg_len as u32).to_ne_bytes());
    msg[4..6].copy_from_slice(&RTM_NEWADDR.to_ne_bytes());
    msg[6..8]
        .copy_from_slice(&(NLM_F_REQUEST | NLM_F_ACK | NLM_F_CREATE | NLM_F_REPLACE).to_ne_bytes());
    msg[8..12].copy_from_slice(&seq.to_ne_bytes());
    msg[12..16].copy_from_slice(&nl.pid.to_ne_bytes());

    // ifaddrmsg
    let ifa = NLMSG_HDR_LEN;
    msg[ifa] = AF_INET6; // ifa_family
    msg[ifa + 1] = prefix_len; // ifa_prefixlen
    msg[ifa + 2] = 0; // ifa_flags
    msg[ifa + 3] = RT_SCOPE_UNIVERSE; // ifa_scope
    msg[ifa + 4..ifa + 8].copy_from_slice(&ifindex.to_ne_bytes()); // ifa_index

    // IFA_ADDRESS attribute
    let off = NLMSG_HDR_LEN + IFADDRMSG_LEN;
    let rta_len: u16 = 4 + 16; // header + IPv6
    msg[off..off + 2].copy_from_slice(&rta_len.to_ne_bytes());
    msg[off + 2..off + 4].copy_from_slice(&IFA_ADDRESS.to_ne_bytes());
    msg[off + 4..off + 20].copy_from_slice(&address.octets());

    nl.request(&msg)?;
    Ok(())
}

/// Add an IPv6 route via netlink.
pub fn add_ipv6_route(
    destination: Ipv6Addr,
    dst_prefix_len: u8,
    gateway: Option<Ipv6Addr>,
    ifindex: u32,
    metric: Option<u32>,
    protocol: u8,
) -> std::io::Result<()> {
    use crate::link::NetlinkSocket;

    const NLMSG_HDR_LEN: usize = 16;
    const RTMSG_LEN: usize = 12;
    const AF_INET6: u8 = 10;
    const RTM_NEWROUTE: u16 = 24;
    const NLM_F_REQUEST: u16 = 0x0001;
    const NLM_F_ACK: u16 = 0x0004;
    const NLM_F_CREATE: u16 = 0x0400;
    const NLM_F_REPLACE: u16 = 0x0100;
    const RTA_DST: u16 = 1;
    const RTA_GATEWAY: u16 = 5;
    const RTA_OIF: u16 = 4;
    const RTA_PRIORITY: u16 = 6;
    const RT_TABLE_MAIN: u8 = 254;
    const RTN_UNICAST: u8 = 1;
    const RT_SCOPE_UNIVERSE: u8 = 0;

    let mut nl = NetlinkSocket::open()?;
    let seq = nl.next_seq();

    // Calculate attribute sizes
    let ipv6_attr_len = (4 + 16 + 3) & !3; // 20 bytes
    let u32_attr_len = (4 + 4 + 3) & !3; // 8 bytes

    let dst_len = if dst_prefix_len > 0 { ipv6_attr_len } else { 0 };
    let gw_len = if gateway.is_some() { ipv6_attr_len } else { 0 };
    let oif_len = u32_attr_len;
    let metric_len = if metric.is_some() { u32_attr_len } else { 0 };

    let msg_len = NLMSG_HDR_LEN + RTMSG_LEN + dst_len + gw_len + oif_len + metric_len;
    let aligned_len = (msg_len + 3) & !3;
    let mut msg = vec![0u8; aligned_len];

    // nlmsghdr
    msg[0..4].copy_from_slice(&(msg_len as u32).to_ne_bytes());
    msg[4..6].copy_from_slice(&RTM_NEWROUTE.to_ne_bytes());
    msg[6..8]
        .copy_from_slice(&(NLM_F_REQUEST | NLM_F_ACK | NLM_F_CREATE | NLM_F_REPLACE).to_ne_bytes());
    msg[8..12].copy_from_slice(&seq.to_ne_bytes());
    msg[12..16].copy_from_slice(&nl.pid.to_ne_bytes());

    // rtmsg
    let rt = NLMSG_HDR_LEN;
    msg[rt] = AF_INET6; // rtm_family
    msg[rt + 1] = dst_prefix_len; // rtm_dst_len
    msg[rt + 2] = 0; // rtm_src_len
    msg[rt + 3] = 0; // rtm_tos
    msg[rt + 4] = RT_TABLE_MAIN; // rtm_table
    msg[rt + 5] = protocol; // rtm_protocol
    msg[rt + 6] = RT_SCOPE_UNIVERSE; // rtm_scope
    msg[rt + 7] = RTN_UNICAST; // rtm_type

    let mut off = NLMSG_HDR_LEN + RTMSG_LEN;

    // RTA_DST (IPv6)
    if dst_prefix_len > 0 {
        let rta_len: u16 = 4 + 16;
        msg[off..off + 2].copy_from_slice(&rta_len.to_ne_bytes());
        msg[off + 2..off + 4].copy_from_slice(&RTA_DST.to_ne_bytes());
        msg[off + 4..off + 20].copy_from_slice(&destination.octets());
        off += dst_len;
    }

    // RTA_GATEWAY (IPv6)
    if let Some(gw) = gateway {
        let rta_len: u16 = 4 + 16;
        msg[off..off + 2].copy_from_slice(&rta_len.to_ne_bytes());
        msg[off + 2..off + 4].copy_from_slice(&RTA_GATEWAY.to_ne_bytes());
        msg[off + 4..off + 20].copy_from_slice(&gw.octets());
        off += gw_len;
    }

    // RTA_OIF
    let rta_len: u16 = 8;
    msg[off..off + 2].copy_from_slice(&rta_len.to_ne_bytes());
    msg[off + 2..off + 4].copy_from_slice(&RTA_OIF.to_ne_bytes());
    msg[off + 4..off + 8].copy_from_slice(&ifindex.to_ne_bytes());
    off += oif_len;

    // RTA_PRIORITY (metric)
    if let Some(m) = metric {
        let rta_len: u16 = 8;
        msg[off..off + 2].copy_from_slice(&rta_len.to_ne_bytes());
        msg[off + 2..off + 4].copy_from_slice(&RTA_PRIORITY.to_ne_bytes());
        msg[off + 4..off + 8].copy_from_slice(&m.to_ne_bytes());
    }

    nl.request(&msg)?;
    Ok(())
}

/// Add an IPv6 default route (`::/0`) via the given gateway.
pub fn add_ipv6_default_route(
    gateway: Ipv6Addr,
    ifindex: u32,
    metric: Option<u32>,
) -> std::io::Result<()> {
    add_ipv6_route(
        Ipv6Addr::UNSPECIFIED,
        0,
        Some(gateway),
        ifindex,
        metric,
        RTPROT_RA,
    )
}

/// Route protocol constant for RA-learned routes.
pub fn rtprot_ra() -> u8 {
    RTPROT_RA
}

// ---------------------------------------------------------------------------
// Public constants
// ---------------------------------------------------------------------------

/// Re-export for use by the manager.
pub fn all_routers_multicast() -> Ipv6Addr {
    ALL_ROUTERS_MULTICAST
}

pub fn rs_retransmit_interval() -> Duration {
    RS_RETRANSMIT_INTERVAL
}

pub fn max_rs_retransmissions() -> u32 {
    MAX_RS_RETRANSMISSIONS
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // -----------------------------------------------------------------------
    // EUI-64 and link-local generation
    // -----------------------------------------------------------------------

    #[test]
    fn test_mac_to_eui64_basic() {
        // Example: MAC 00:1a:2b:3c:4d:5e
        let mac = [0x00, 0x1a, 0x2b, 0x3c, 0x4d, 0x5e];
        let eui64 = mac_to_eui64(&mac);
        // First byte: 0x00 ^ 0x02 = 0x02
        assert_eq!(eui64[0], 0x02);
        assert_eq!(eui64[1], 0x1a);
        assert_eq!(eui64[2], 0x2b);
        assert_eq!(eui64[3], 0xff);
        assert_eq!(eui64[4], 0xfe);
        assert_eq!(eui64[5], 0x3c);
        assert_eq!(eui64[6], 0x4d);
        assert_eq!(eui64[7], 0x5e);
    }

    #[test]
    fn test_mac_to_eui64_ul_bit_flip() {
        // MAC with U/L bit already set: 02:00:00:00:00:01
        let mac = [0x02, 0x00, 0x00, 0x00, 0x00, 0x01];
        let eui64 = mac_to_eui64(&mac);
        // 0x02 ^ 0x02 = 0x00
        assert_eq!(eui64[0], 0x00);
        assert_eq!(eui64[3], 0xff);
        assert_eq!(eui64[4], 0xfe);
        assert_eq!(eui64[7], 0x01);
    }

    #[test]
    fn test_mac_to_eui64_all_ff() {
        let mac = [0xff, 0xff, 0xff, 0xff, 0xff, 0xff];
        let eui64 = mac_to_eui64(&mac);
        assert_eq!(eui64[0], 0xfd); // 0xff ^ 0x02
        assert_eq!(eui64[1], 0xff);
        assert_eq!(eui64[2], 0xff);
        assert_eq!(eui64[3], 0xff);
        assert_eq!(eui64[4], 0xfe);
        assert_eq!(eui64[5], 0xff);
        assert_eq!(eui64[6], 0xff);
        assert_eq!(eui64[7], 0xff);
    }

    #[test]
    fn test_mac_to_eui64_zeros() {
        let mac = [0x00, 0x00, 0x00, 0x00, 0x00, 0x00];
        let eui64 = mac_to_eui64(&mac);
        assert_eq!(eui64[0], 0x02); // 0x00 ^ 0x02
        assert_eq!(eui64[3], 0xff);
        assert_eq!(eui64[4], 0xfe);
    }

    #[test]
    fn test_mac_to_link_local() {
        let mac = [0x52, 0x54, 0x00, 0x12, 0x34, 0x56];
        let ll = mac_to_link_local(&mac);
        // fe80::5054:ff:fe12:3456
        // First byte: 0x52 ^ 0x02 = 0x50
        let expected = Ipv6Addr::new(0xfe80, 0, 0, 0, 0x5054, 0x00ff, 0xfe12, 0x3456);
        assert_eq!(ll, expected);
    }

    #[test]
    fn test_mac_to_link_local_is_link_local() {
        let mac = [0x00, 0x1a, 0x2b, 0x3c, 0x4d, 0x5e];
        let ll = mac_to_link_local(&mac);
        // Check that it's in the fe80::/10 range
        let octets = ll.octets();
        assert_eq!(octets[0], 0xfe);
        assert_eq!(octets[1], 0x80);
        // Bytes 2..8 should be zero (link-local prefix is fe80::/64)
        assert!(octets[2..8].iter().all(|&b| b == 0));
    }

    // -----------------------------------------------------------------------
    // SLAAC address generation
    // -----------------------------------------------------------------------

    #[test]
    fn test_slaac_eui64_basic() {
        let prefix = Ipv6Addr::new(0x2001, 0x0db8, 0, 1, 0, 0, 0, 0);
        let mac = [0x52, 0x54, 0x00, 0x12, 0x34, 0x56];
        let addr = slaac_eui64(&prefix, &mac).unwrap();
        let expected = Ipv6Addr::new(0x2001, 0x0db8, 0, 1, 0x5054, 0x00ff, 0xfe12, 0x3456);
        assert_eq!(addr, expected);
    }

    #[test]
    fn test_slaac_eui64_preserves_prefix() {
        let prefix = Ipv6Addr::new(0x2a02, 0x1234, 0x5678, 0x9abc, 0, 0, 0, 0);
        let mac = [0x00, 0x11, 0x22, 0x33, 0x44, 0x55];
        let addr = slaac_eui64(&prefix, &mac).unwrap();
        let octets = addr.octets();
        // First 8 bytes should match the prefix
        let prefix_octets = prefix.octets();
        assert_eq!(&octets[..8], &prefix_octets[..8]);
    }

    #[test]
    fn test_slaac_eui64_different_macs_different_addrs() {
        let prefix = Ipv6Addr::new(0x2001, 0x0db8, 0, 0, 0, 0, 0, 0);
        let mac1 = [0x00, 0x11, 0x22, 0x33, 0x44, 0x55];
        let mac2 = [0x00, 0x11, 0x22, 0x33, 0x44, 0x66];
        let addr1 = slaac_eui64(&prefix, &mac1).unwrap();
        let addr2 = slaac_eui64(&prefix, &mac2).unwrap();
        assert_ne!(addr1, addr2);
    }

    // -----------------------------------------------------------------------
    // RS message construction
    // -----------------------------------------------------------------------

    #[test]
    fn test_build_rs_with_mac() {
        let mac = [0x52, 0x54, 0x00, 0x12, 0x34, 0x56];
        let rs = build_rs(Some(&mac));
        // 8 bytes base + 8 bytes source LL option = 16
        assert_eq!(rs.len(), 16);
        assert_eq!(rs[0], ICMPV6_TYPE_RS); // type
        assert_eq!(rs[1], 0); // code
        // Checksum should be 0 (kernel fills it)
        assert_eq!(rs[2], 0);
        assert_eq!(rs[3], 0);
        // Reserved
        assert_eq!(&rs[4..8], &[0, 0, 0, 0]);
        // Source LL option
        assert_eq!(rs[8], NDP_OPT_SOURCE_LL_ADDR);
        assert_eq!(rs[9], 1); // Length in 8-byte units
        assert_eq!(&rs[10..16], &mac);
    }

    #[test]
    fn test_build_rs_without_mac() {
        let rs = build_rs(None);
        assert_eq!(rs.len(), 8);
        assert_eq!(rs[0], ICMPV6_TYPE_RS);
        assert_eq!(rs[1], 0);
    }

    // -----------------------------------------------------------------------
    // RA message parsing
    // -----------------------------------------------------------------------

    fn build_test_ra(
        hop_limit: u8,
        flags: u8,
        router_lifetime: u16,
        reachable_time: u32,
        retrans_timer: u32,
    ) -> Vec<u8> {
        let mut data = vec![0u8; 16];
        data[0] = ICMPV6_TYPE_RA;
        data[1] = 0; // code
        // checksum [2..4]
        data[4] = hop_limit;
        data[5] = flags;
        data[6..8].copy_from_slice(&router_lifetime.to_be_bytes());
        data[8..12].copy_from_slice(&reachable_time.to_be_bytes());
        data[12..16].copy_from_slice(&retrans_timer.to_be_bytes());
        data
    }

    #[test]
    fn test_parse_ra_minimal() {
        let data = build_test_ra(64, 0, 1800, 0, 0);
        let source = Ipv6Addr::new(0xfe80, 0, 0, 0, 1, 2, 3, 4);
        let ra = parse_ra(&data, source).unwrap();
        assert_eq!(ra.cur_hop_limit, 64);
        assert!(!ra.managed);
        assert!(!ra.other);
        assert_eq!(ra.router_lifetime, 1800);
        assert_eq!(ra.reachable_time, 0);
        assert_eq!(ra.retrans_timer, 0);
        assert_eq!(ra.source, source);
        assert!(ra.prefixes.is_empty());
        assert!(ra.rdnss.is_empty());
        assert!(ra.routes.is_empty());
        assert!(ra.mtu.is_none());
        assert!(ra.source_ll_addr.is_none());
    }

    #[test]
    fn test_parse_ra_managed_flag() {
        let data = build_test_ra(64, 0x80, 1800, 0, 0);
        let source = Ipv6Addr::LOCALHOST;
        let ra = parse_ra(&data, source).unwrap();
        assert!(ra.managed);
        assert!(!ra.other);
    }

    #[test]
    fn test_parse_ra_other_flag() {
        let data = build_test_ra(64, 0x40, 1800, 0, 0);
        let source = Ipv6Addr::LOCALHOST;
        let ra = parse_ra(&data, source).unwrap();
        assert!(!ra.managed);
        assert!(ra.other);
    }

    #[test]
    fn test_parse_ra_both_flags() {
        let data = build_test_ra(64, 0xC0, 1800, 0, 0);
        let source = Ipv6Addr::LOCALHOST;
        let ra = parse_ra(&data, source).unwrap();
        assert!(ra.managed);
        assert!(ra.other);
    }

    #[test]
    fn test_parse_ra_too_short() {
        let data = vec![0u8; 15]; // Too short
        assert!(parse_ra(&data, Ipv6Addr::LOCALHOST).is_none());
    }

    #[test]
    fn test_parse_ra_wrong_type() {
        let mut data = build_test_ra(64, 0, 1800, 0, 0);
        data[0] = ICMPV6_TYPE_RS; // Wrong type
        assert!(parse_ra(&data, Ipv6Addr::LOCALHOST).is_none());
    }

    #[test]
    fn test_parse_ra_with_reachable_and_retrans() {
        let data = build_test_ra(128, 0, 600, 30000, 1000);
        let source = Ipv6Addr::LOCALHOST;
        let ra = parse_ra(&data, source).unwrap();
        assert_eq!(ra.cur_hop_limit, 128);
        assert_eq!(ra.router_lifetime, 600);
        assert_eq!(ra.reachable_time, 30000);
        assert_eq!(ra.retrans_timer, 1000);
    }

    #[test]
    fn test_parse_ra_zero_router_lifetime() {
        let data = build_test_ra(64, 0, 0, 0, 0);
        let ra = parse_ra(&data, Ipv6Addr::LOCALHOST).unwrap();
        assert_eq!(ra.router_lifetime, 0);
    }

    // -----------------------------------------------------------------------
    // Prefix Information option
    // -----------------------------------------------------------------------

    fn build_prefix_option(
        prefix_len: u8,
        on_link: bool,
        autonomous: bool,
        valid_lifetime: u32,
        preferred_lifetime: u32,
        prefix: Ipv6Addr,
    ) -> Vec<u8> {
        let mut opt = vec![0u8; 32];
        opt[0] = NDP_OPT_PREFIX_INFO;
        opt[1] = 4; // Length in 8-byte units (32 bytes)
        opt[2] = prefix_len;
        let mut flags = 0u8;
        if on_link {
            flags |= PREFIX_FLAG_ON_LINK;
        }
        if autonomous {
            flags |= PREFIX_FLAG_AUTONOMOUS;
        }
        opt[3] = flags;
        opt[4..8].copy_from_slice(&valid_lifetime.to_be_bytes());
        opt[8..12].copy_from_slice(&preferred_lifetime.to_be_bytes());
        // Reserved2 at [12..16]
        opt[16..32].copy_from_slice(&prefix.octets());
        opt
    }

    #[test]
    fn test_parse_prefix_info_basic() {
        let prefix = Ipv6Addr::new(0x2001, 0x0db8, 0, 1, 0, 0, 0, 0);
        let opt = build_prefix_option(64, true, true, 86400, 3600, prefix);
        let info = parse_prefix_info(&opt).unwrap();
        assert_eq!(info.prefix_len, 64);
        assert!(info.on_link);
        assert!(info.autonomous);
        assert_eq!(info.valid_lifetime, 86400);
        assert_eq!(info.preferred_lifetime, 3600);
        assert_eq!(info.prefix, prefix);
    }

    #[test]
    fn test_parse_prefix_info_no_flags() {
        let prefix = Ipv6Addr::new(0x2001, 0x0db8, 0, 0, 0, 0, 0, 0);
        let opt = build_prefix_option(64, false, false, 1000, 500, prefix);
        let info = parse_prefix_info(&opt).unwrap();
        assert!(!info.on_link);
        assert!(!info.autonomous);
    }

    #[test]
    fn test_parse_prefix_info_infinity() {
        let prefix = Ipv6Addr::new(0x2001, 0x0db8, 0, 0, 0, 0, 0, 0);
        let opt = build_prefix_option(64, true, true, 0xFFFFFFFF, 0xFFFFFFFF, prefix);
        let info = parse_prefix_info(&opt).unwrap();
        assert_eq!(info.valid_lifetime, 0xFFFFFFFF);
        assert_eq!(info.preferred_lifetime, 0xFFFFFFFF);
    }

    #[test]
    fn test_parse_prefix_info_too_short() {
        let data = vec![0u8; 31]; // Too short (need 32)
        assert!(parse_prefix_info(&data).is_none());
    }

    // -----------------------------------------------------------------------
    // RA with prefix option
    // -----------------------------------------------------------------------

    #[test]
    fn test_parse_ra_with_prefix() {
        let prefix = Ipv6Addr::new(0x2001, 0x0db8, 0, 1, 0, 0, 0, 0);
        let mut data = build_test_ra(64, 0, 1800, 0, 0);
        data.extend_from_slice(&build_prefix_option(64, true, true, 86400, 3600, prefix));

        let ra = parse_ra(&data, Ipv6Addr::LOCALHOST).unwrap();
        assert_eq!(ra.prefixes.len(), 1);
        assert_eq!(ra.prefixes[0].prefix, prefix);
        assert_eq!(ra.prefixes[0].prefix_len, 64);
        assert!(ra.prefixes[0].on_link);
        assert!(ra.prefixes[0].autonomous);
    }

    #[test]
    fn test_parse_ra_with_multiple_prefixes() {
        let p1 = Ipv6Addr::new(0x2001, 0x0db8, 0, 1, 0, 0, 0, 0);
        let p2 = Ipv6Addr::new(0x2001, 0x0db8, 0, 2, 0, 0, 0, 0);
        let mut data = build_test_ra(64, 0, 1800, 0, 0);
        data.extend_from_slice(&build_prefix_option(64, true, true, 86400, 3600, p1));
        data.extend_from_slice(&build_prefix_option(64, true, true, 43200, 1800, p2));

        let ra = parse_ra(&data, Ipv6Addr::LOCALHOST).unwrap();
        assert_eq!(ra.prefixes.len(), 2);
        assert_eq!(ra.prefixes[0].prefix, p1);
        assert_eq!(ra.prefixes[1].prefix, p2);
    }

    // -----------------------------------------------------------------------
    // MTU option
    // -----------------------------------------------------------------------

    fn build_mtu_option(mtu: u32) -> Vec<u8> {
        let mut opt = vec![0u8; 8];
        opt[0] = NDP_OPT_MTU;
        opt[1] = 1; // Length in 8-byte units
        // Reserved [2..4]
        opt[4..8].copy_from_slice(&mtu.to_be_bytes());
        opt
    }

    #[test]
    fn test_parse_mtu_option() {
        let opt = build_mtu_option(1500);
        let mtu = parse_mtu_option(&opt).unwrap();
        assert_eq!(mtu.mtu, 1500);
    }

    #[test]
    fn test_parse_mtu_option_jumbo() {
        let opt = build_mtu_option(9000);
        let mtu = parse_mtu_option(&opt).unwrap();
        assert_eq!(mtu.mtu, 9000);
    }

    #[test]
    fn test_parse_mtu_option_too_short() {
        let data = vec![0u8; 7];
        assert!(parse_mtu_option(&data).is_none());
    }

    #[test]
    fn test_parse_ra_with_mtu() {
        let mut data = build_test_ra(64, 0, 1800, 0, 0);
        data.extend_from_slice(&build_mtu_option(1400));

        let ra = parse_ra(&data, Ipv6Addr::LOCALHOST).unwrap();
        assert_eq!(ra.mtu.as_ref().unwrap().mtu, 1400);
    }

    // -----------------------------------------------------------------------
    // RDNSS option
    // -----------------------------------------------------------------------

    fn build_rdnss_option(lifetime: u32, servers: &[Ipv6Addr]) -> Vec<u8> {
        let opt_len_units = 1 + (servers.len() * 2) as u8; // 1 unit header + 2 units per addr
        let opt_len_bytes = opt_len_units as usize * 8;
        let mut opt = vec![0u8; opt_len_bytes];
        opt[0] = NDP_OPT_RDNSS;
        opt[1] = opt_len_units;
        // Reserved [2..4]
        opt[4..8].copy_from_slice(&lifetime.to_be_bytes());
        for (i, server) in servers.iter().enumerate() {
            let off = 8 + i * 16;
            opt[off..off + 16].copy_from_slice(&server.octets());
        }
        opt
    }

    #[test]
    fn test_parse_rdnss_single_server() {
        let dns = Ipv6Addr::new(0x2001, 0x4860, 0x4860, 0, 0, 0, 0, 0x8888);
        let opt = build_rdnss_option(3600, &[dns]);
        let rdnss = parse_rdnss(&opt).unwrap();
        assert_eq!(rdnss.lifetime, 3600);
        assert_eq!(rdnss.servers.len(), 1);
        assert_eq!(rdnss.servers[0], dns);
    }

    #[test]
    fn test_parse_rdnss_multiple_servers() {
        let dns1 = Ipv6Addr::new(0x2001, 0x4860, 0x4860, 0, 0, 0, 0, 0x8888);
        let dns2 = Ipv6Addr::new(0x2001, 0x4860, 0x4860, 0, 0, 0, 0, 0x8844);
        let opt = build_rdnss_option(7200, &[dns1, dns2]);
        let rdnss = parse_rdnss(&opt).unwrap();
        assert_eq!(rdnss.lifetime, 7200);
        assert_eq!(rdnss.servers.len(), 2);
        assert_eq!(rdnss.servers[0], dns1);
        assert_eq!(rdnss.servers[1], dns2);
    }

    #[test]
    fn test_parse_rdnss_zero_lifetime() {
        let dns = Ipv6Addr::new(0x2001, 0x4860, 0x4860, 0, 0, 0, 0, 0x8888);
        let opt = build_rdnss_option(0, &[dns]);
        let rdnss = parse_rdnss(&opt).unwrap();
        assert_eq!(rdnss.lifetime, 0);
    }

    #[test]
    fn test_parse_rdnss_too_short() {
        let data = vec![0u8; 23]; // Need at least 24 (8 header + 16 addr)
        assert!(parse_rdnss(&data).is_none());
    }

    #[test]
    fn test_parse_ra_with_rdnss() {
        let dns = Ipv6Addr::new(0x2001, 0x4860, 0x4860, 0, 0, 0, 0, 0x8888);
        let mut data = build_test_ra(64, 0, 1800, 0, 0);
        data.extend_from_slice(&build_rdnss_option(3600, &[dns]));

        let ra = parse_ra(&data, Ipv6Addr::LOCALHOST).unwrap();
        assert_eq!(ra.rdnss.len(), 1);
        assert_eq!(ra.rdnss[0].servers[0], dns);
    }

    // -----------------------------------------------------------------------
    // DNSSL option
    // -----------------------------------------------------------------------

    fn build_dnssl_option(lifetime: u32, domains: &[&str]) -> Vec<u8> {
        let mut name_payload = Vec::new();
        for domain in domains {
            for label in domain.split('.') {
                name_payload.push(label.len() as u8);
                name_payload.extend_from_slice(label.as_bytes());
            }
            name_payload.push(0); // Terminate this name
        }
        // Pad to 8-byte boundary
        while (8 + name_payload.len()) % 8 != 0 {
            name_payload.push(0);
        }

        let opt_len_units = ((8 + name_payload.len()) / 8) as u8;
        let mut opt = vec![0u8; 8 + name_payload.len()];
        opt[0] = NDP_OPT_DNSSL;
        opt[1] = opt_len_units;
        opt[4..8].copy_from_slice(&lifetime.to_be_bytes());
        opt[8..8 + name_payload.len()].copy_from_slice(&name_payload);
        opt
    }

    #[test]
    fn test_parse_dnssl_single_domain() {
        let opt = build_dnssl_option(3600, &["example.com"]);
        let dnssl = parse_dnssl(&opt).unwrap();
        assert_eq!(dnssl.lifetime, 3600);
        assert_eq!(dnssl.domains.len(), 1);
        assert_eq!(dnssl.domains[0], "example.com");
    }

    #[test]
    fn test_parse_dnssl_multiple_domains() {
        let opt = build_dnssl_option(7200, &["example.com", "test.local"]);
        let dnssl = parse_dnssl(&opt).unwrap();
        assert_eq!(dnssl.lifetime, 7200);
        assert_eq!(dnssl.domains.len(), 2);
        assert_eq!(dnssl.domains[0], "example.com");
        assert_eq!(dnssl.domains[1], "test.local");
    }

    #[test]
    fn test_parse_dnssl_subdomain() {
        let opt = build_dnssl_option(3600, &["corp.example.com"]);
        let dnssl = parse_dnssl(&opt).unwrap();
        assert_eq!(dnssl.domains[0], "corp.example.com");
    }

    #[test]
    fn test_parse_dnssl_too_short() {
        let data = vec![0u8; 15];
        assert!(parse_dnssl(&data).is_none());
    }

    // -----------------------------------------------------------------------
    // Route Information option
    // -----------------------------------------------------------------------

    fn build_route_info_option(
        prefix_len: u8,
        preference: u8,
        lifetime: u32,
        prefix: Ipv6Addr,
    ) -> Vec<u8> {
        // Determine size based on prefix length
        let opt_len_units: u8 = if prefix_len == 0 {
            1
        } else if prefix_len <= 64 {
            2
        } else {
            3
        };
        let opt_len_bytes = opt_len_units as usize * 8;
        let mut opt = vec![0u8; opt_len_bytes];
        opt[0] = NDP_OPT_ROUTE_INFO;
        opt[1] = opt_len_units;
        opt[2] = prefix_len;
        opt[3] = (preference & 0x03) << 3;
        opt[4..8].copy_from_slice(&lifetime.to_be_bytes());
        if prefix_len > 0 {
            let prefix_octets = prefix.octets();
            let needed = if prefix_len <= 64 { 8 } else { 16 };
            let available = std::cmp::min(needed, opt_len_bytes - 8);
            opt[8..8 + available].copy_from_slice(&prefix_octets[..available]);
        }
        opt
    }

    #[test]
    fn test_parse_route_info_64_prefix() {
        let prefix = Ipv6Addr::new(0x2001, 0x0db8, 0xabcd, 0, 0, 0, 0, 0);
        let opt = build_route_info_option(48, 1, 7200, prefix);
        let route = parse_route_info(&opt).unwrap();
        assert_eq!(route.prefix_len, 48);
        assert_eq!(route.preference, 1); // high
        assert_eq!(route.lifetime, 7200);
        // First 8 bytes of prefix should match
        let route_octets = route.prefix.octets();
        let prefix_octets = prefix.octets();
        assert_eq!(&route_octets[..8], &prefix_octets[..8]);
    }

    #[test]
    fn test_parse_route_info_default_route() {
        // Prefix length 0 = default route
        let prefix = Ipv6Addr::UNSPECIFIED;
        let opt = build_route_info_option(0, 0, 1800, prefix);
        let route = parse_route_info(&opt).unwrap();
        assert_eq!(route.prefix_len, 0);
        assert_eq!(route.preference, 0); // medium
        assert_eq!(route.lifetime, 1800);
    }

    #[test]
    fn test_parse_route_info_low_preference() {
        let prefix = Ipv6Addr::new(0xfc00, 0, 0, 0, 0, 0, 0, 0);
        let opt = build_route_info_option(8, 3, 600, prefix);
        let route = parse_route_info(&opt).unwrap();
        assert_eq!(route.preference, 3); // low
    }

    #[test]
    fn test_parse_route_info_too_short() {
        let data = vec![0u8; 7];
        assert!(parse_route_info(&data).is_none());
    }

    // -----------------------------------------------------------------------
    // Source Link-Layer Address option
    // -----------------------------------------------------------------------

    fn build_source_ll_option(mac: &[u8; 6]) -> Vec<u8> {
        let mut opt = vec![0u8; 8];
        opt[0] = NDP_OPT_SOURCE_LL_ADDR;
        opt[1] = 1; // 8 bytes
        opt[2..8].copy_from_slice(mac);
        opt
    }

    #[test]
    fn test_parse_ra_with_source_ll_addr() {
        let mac = [0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff];
        let mut data = build_test_ra(64, 0, 1800, 0, 0);
        data.extend_from_slice(&build_source_ll_option(&mac));

        let ra = parse_ra(&data, Ipv6Addr::LOCALHOST).unwrap();
        assert_eq!(ra.source_ll_addr, Some(mac));
    }

    // -----------------------------------------------------------------------
    // Full RA with all options
    // -----------------------------------------------------------------------

    #[test]
    fn test_parse_ra_full() {
        let prefix = Ipv6Addr::new(0x2001, 0x0db8, 0, 1, 0, 0, 0, 0);
        let dns = Ipv6Addr::new(0x2001, 0x4860, 0x4860, 0, 0, 0, 0, 0x8888);
        let router_mac = [0x00, 0x11, 0x22, 0x33, 0x44, 0x55];

        let mut data = build_test_ra(64, 0xC0, 1800, 30000, 1000);
        data.extend_from_slice(&build_source_ll_option(&router_mac));
        data.extend_from_slice(&build_mtu_option(1400));
        data.extend_from_slice(&build_prefix_option(64, true, true, 86400, 3600, prefix));
        data.extend_from_slice(&build_rdnss_option(7200, &[dns]));
        data.extend_from_slice(&build_dnssl_option(7200, &["example.com"]));

        let source = Ipv6Addr::new(0xfe80, 0, 0, 0, 0x0211, 0x22ff, 0xfe33, 0x4455);
        let ra = parse_ra(&data, source).unwrap();

        assert_eq!(ra.cur_hop_limit, 64);
        assert!(ra.managed);
        assert!(ra.other);
        assert_eq!(ra.router_lifetime, 1800);
        assert_eq!(ra.reachable_time, 30000);
        assert_eq!(ra.retrans_timer, 1000);
        assert_eq!(ra.source, source);
        assert_eq!(ra.source_ll_addr, Some(router_mac));
        assert_eq!(ra.mtu.as_ref().unwrap().mtu, 1400);
        assert_eq!(ra.prefixes.len(), 1);
        assert_eq!(ra.prefixes[0].prefix, prefix);
        assert_eq!(ra.rdnss.len(), 1);
        assert_eq!(ra.rdnss[0].servers[0], dns);
        assert_eq!(ra.dnssl.len(), 1);
        assert_eq!(ra.dnssl[0].domains[0], "example.com");
    }

    // -----------------------------------------------------------------------
    // RA option with zero length (invalid)
    // -----------------------------------------------------------------------

    #[test]
    fn test_parse_ra_option_zero_length_stops() {
        let mut data = build_test_ra(64, 0, 1800, 0, 0);
        // Add an option with length=0 which should stop parsing
        data.push(NDP_OPT_PREFIX_INFO);
        data.push(0); // Zero length — invalid
        // Add more data that shouldn't be parsed
        data.extend_from_slice(&[0u8; 32]);

        let ra = parse_ra(&data, Ipv6Addr::LOCALHOST).unwrap();
        // The zero-length option should have stopped parsing, so no prefixes
        assert!(ra.prefixes.is_empty());
    }

    // -----------------------------------------------------------------------
    // DNS name parsing
    // -----------------------------------------------------------------------

    #[test]
    fn test_parse_dns_names_single() {
        // "example.com" = [7]example[3]com[0]
        let data = [
            7, b'e', b'x', b'a', b'm', b'p', b'l', b'e', 3, b'c', b'o', b'm', 0,
        ];
        let names = parse_dns_names(&data);
        assert_eq!(names, vec!["example.com"]);
    }

    #[test]
    fn test_parse_dns_names_multiple() {
        // "a.b" then "c.d"
        let mut data = Vec::new();
        data.push(1);
        data.push(b'a');
        data.push(1);
        data.push(b'b');
        data.push(0); // end of first name
        data.push(1);
        data.push(b'c');
        data.push(1);
        data.push(b'd');
        data.push(0); // end of second name
        let names = parse_dns_names(&data);
        assert_eq!(names, vec!["a.b", "c.d"]);
    }

    #[test]
    fn test_parse_dns_names_empty() {
        let data = [0u8; 4]; // All zeros
        let names = parse_dns_names(&data);
        assert!(names.is_empty());
    }

    #[test]
    fn test_parse_dns_names_single_label() {
        let data = [4, b'h', b'o', b's', b't', 0];
        let names = parse_dns_names(&data);
        assert_eq!(names, vec!["host"]);
    }

    // -----------------------------------------------------------------------
    // RaState
    // -----------------------------------------------------------------------

    #[test]
    fn test_ra_state_new() {
        let mac = [0x52, 0x54, 0x00, 0x12, 0x34, 0x56];
        let state = RaState::new(2, "eth0".to_string(), mac);
        assert_eq!(state.ifindex, 2);
        assert_eq!(state.ifname, "eth0");
        assert!(state.enabled);
        assert_eq!(state.rs_count, 0);
        assert!(state.last_rs.is_none());
        assert!(!state.ra_received);
        assert!(state.last_ra.is_none());
        assert!(state.slaac_addresses.is_empty());
        assert!(state.default_router.is_none());
        assert!(state.dns_servers.is_empty());
        assert!(state.search_domains.is_empty());
        assert!(state.link_local.is_some());
    }

    #[test]
    fn test_ra_state_link_local() {
        let mac = [0x52, 0x54, 0x00, 0x12, 0x34, 0x56];
        let state = RaState::new(2, "eth0".to_string(), mac);
        let expected = mac_to_link_local(&mac);
        assert_eq!(state.link_local, Some(expected));
    }

    #[test]
    fn test_ra_state_should_send_rs_initially() {
        let state = RaState::new(2, "eth0".to_string(), [0; 6]);
        assert!(state.should_send_rs());
    }

    #[test]
    fn test_ra_state_should_not_send_rs_after_ra() {
        let mut state = RaState::new(2, "eth0".to_string(), [0; 6]);
        state.ra_received = true;
        assert!(!state.should_send_rs());
    }

    #[test]
    fn test_ra_state_should_not_send_rs_when_disabled() {
        let mut state = RaState::new(2, "eth0".to_string(), [0; 6]);
        state.enabled = false;
        assert!(!state.should_send_rs());
    }

    #[test]
    fn test_ra_state_should_not_send_rs_after_max() {
        let mut state = RaState::new(2, "eth0".to_string(), [0; 6]);
        state.rs_count = MAX_RS_RETRANSMISSIONS;
        assert!(!state.should_send_rs());
    }

    #[test]
    fn test_ra_state_mark_rs_sent() {
        let mut state = RaState::new(2, "eth0".to_string(), [0; 6]);
        state.mark_rs_sent();
        assert_eq!(state.rs_count, 1);
        assert!(state.last_rs.is_some());
    }

    // -----------------------------------------------------------------------
    // RaState::process_ra
    // -----------------------------------------------------------------------

    #[test]
    fn test_process_ra_default_route() {
        let mut state = RaState::new(2, "eth0".to_string(), [0x52, 0x54, 0x00, 0x12, 0x34, 0x56]);
        let source = Ipv6Addr::new(0xfe80, 0, 0, 0, 1, 2, 3, 4);
        let ra = RouterAdvertisement {
            cur_hop_limit: 64,
            managed: false,
            other: false,
            router_lifetime: 1800,
            reachable_time: 0,
            retrans_timer: 0,
            source,
            prefixes: vec![],
            rdnss: vec![],
            dnssl: vec![],
            routes: vec![],
            mtu: None,
            source_ll_addr: None,
        };

        let actions = state.process_ra(ra);
        assert!(state.ra_received);
        assert_eq!(state.default_router, Some(source));
        assert!(actions.iter().any(|a| matches!(
            a,
            RaAction::AddDefaultRoute { gateway, .. } if *gateway == source
        )));
    }

    #[test]
    fn test_process_ra_zero_lifetime_removes_default() {
        let mut state = RaState::new(2, "eth0".to_string(), [0; 6]);
        let source = Ipv6Addr::new(0xfe80, 0, 0, 0, 1, 2, 3, 4);
        state.default_router = Some(source);

        let ra = RouterAdvertisement {
            cur_hop_limit: 64,
            managed: false,
            other: false,
            router_lifetime: 0, // Not a default router anymore
            reachable_time: 0,
            retrans_timer: 0,
            source,
            prefixes: vec![],
            rdnss: vec![],
            dnssl: vec![],
            routes: vec![],
            mtu: None,
            source_ll_addr: None,
        };

        let actions = state.process_ra(ra);
        assert!(state.default_router.is_none());
        assert!(actions.iter().any(|a| matches!(
            a,
            RaAction::RemoveDefaultRoute { gateway } if *gateway == source
        )));
    }

    #[test]
    fn test_process_ra_slaac_address() {
        let mac = [0x52, 0x54, 0x00, 0x12, 0x34, 0x56];
        let mut state = RaState::new(2, "eth0".to_string(), mac);
        let prefix = Ipv6Addr::new(0x2001, 0x0db8, 0, 1, 0, 0, 0, 0);
        let expected_addr = slaac_eui64(&prefix, &mac).unwrap();

        let ra = RouterAdvertisement {
            cur_hop_limit: 64,
            managed: false,
            other: false,
            router_lifetime: 1800,
            reachable_time: 0,
            retrans_timer: 0,
            source: Ipv6Addr::new(0xfe80, 0, 0, 0, 1, 2, 3, 4),
            prefixes: vec![PrefixInfo {
                prefix_len: 64,
                on_link: true,
                autonomous: true,
                valid_lifetime: 86400,
                preferred_lifetime: 3600,
                prefix,
            }],
            rdnss: vec![],
            dnssl: vec![],
            routes: vec![],
            mtu: None,
            source_ll_addr: None,
        };

        let actions = state.process_ra(ra);
        assert_eq!(state.slaac_addresses.len(), 1);
        assert_eq!(state.slaac_addresses[0], (expected_addr, 64));
        assert!(actions.iter().any(|a| matches!(
            a,
            RaAction::AddAddress { address, prefix_len, .. } if *address == expected_addr && *prefix_len == 64
        )));
    }

    #[test]
    fn test_process_ra_slaac_not_duplicate() {
        let mac = [0x52, 0x54, 0x00, 0x12, 0x34, 0x56];
        let mut state = RaState::new(2, "eth0".to_string(), mac);
        let prefix = Ipv6Addr::new(0x2001, 0x0db8, 0, 1, 0, 0, 0, 0);

        let make_ra = || RouterAdvertisement {
            cur_hop_limit: 64,
            managed: false,
            other: false,
            router_lifetime: 1800,
            reachable_time: 0,
            retrans_timer: 0,
            source: Ipv6Addr::new(0xfe80, 0, 0, 0, 1, 2, 3, 4),
            prefixes: vec![PrefixInfo {
                prefix_len: 64,
                on_link: true,
                autonomous: true,
                valid_lifetime: 86400,
                preferred_lifetime: 3600,
                prefix,
            }],
            rdnss: vec![],
            dnssl: vec![],
            routes: vec![],
            mtu: None,
            source_ll_addr: None,
        };

        state.process_ra(make_ra());
        state.process_ra(make_ra());
        // Address should only appear once in slaac_addresses
        assert_eq!(state.slaac_addresses.len(), 1);
    }

    #[test]
    fn test_process_ra_non_autonomous_prefix_no_slaac() {
        let mac = [0x52, 0x54, 0x00, 0x12, 0x34, 0x56];
        let mut state = RaState::new(2, "eth0".to_string(), mac);
        let prefix = Ipv6Addr::new(0x2001, 0x0db8, 0, 1, 0, 0, 0, 0);

        let ra = RouterAdvertisement {
            cur_hop_limit: 64,
            managed: false,
            other: false,
            router_lifetime: 1800,
            reachable_time: 0,
            retrans_timer: 0,
            source: Ipv6Addr::new(0xfe80, 0, 0, 0, 1, 2, 3, 4),
            prefixes: vec![PrefixInfo {
                prefix_len: 64,
                on_link: true,
                autonomous: false, // Not autonomous
                valid_lifetime: 86400,
                preferred_lifetime: 3600,
                prefix,
            }],
            rdnss: vec![],
            dnssl: vec![],
            routes: vec![],
            mtu: None,
            source_ll_addr: None,
        };

        let actions = state.process_ra(ra);
        assert!(state.slaac_addresses.is_empty());
        assert!(
            !actions
                .iter()
                .any(|a| matches!(a, RaAction::AddAddress { .. }))
        );
        // But on-link route should still be added
        assert!(
            actions
                .iter()
                .any(|a| matches!(a, RaAction::AddOnLinkRoute { .. }))
        );
    }

    #[test]
    fn test_process_ra_on_link_route() {
        let mut state = RaState::new(2, "eth0".to_string(), [0; 6]);
        let prefix = Ipv6Addr::new(0x2001, 0x0db8, 0, 1, 0, 0, 0, 0);

        let ra = RouterAdvertisement {
            cur_hop_limit: 64,
            managed: false,
            other: false,
            router_lifetime: 1800,
            reachable_time: 0,
            retrans_timer: 0,
            source: Ipv6Addr::new(0xfe80, 0, 0, 0, 1, 2, 3, 4),
            prefixes: vec![PrefixInfo {
                prefix_len: 64,
                on_link: true,
                autonomous: false,
                valid_lifetime: 86400,
                preferred_lifetime: 3600,
                prefix,
            }],
            rdnss: vec![],
            dnssl: vec![],
            routes: vec![],
            mtu: None,
            source_ll_addr: None,
        };

        let actions = state.process_ra(ra);
        assert!(actions.iter().any(|a| matches!(
            a,
            RaAction::AddOnLinkRoute { prefix: p, prefix_len: 64, .. } if *p == prefix
        )));
    }

    #[test]
    fn test_process_ra_rdnss() {
        let mut state = RaState::new(2, "eth0".to_string(), [0; 6]);
        let dns = Ipv6Addr::new(0x2001, 0x4860, 0x4860, 0, 0, 0, 0, 0x8888);

        let ra = RouterAdvertisement {
            cur_hop_limit: 64,
            managed: false,
            other: false,
            router_lifetime: 1800,
            reachable_time: 0,
            retrans_timer: 0,
            source: Ipv6Addr::new(0xfe80, 0, 0, 0, 1, 2, 3, 4),
            prefixes: vec![],
            rdnss: vec![RdnssInfo {
                lifetime: 3600,
                servers: vec![dns],
            }],
            dnssl: vec![],
            routes: vec![],
            mtu: None,
            source_ll_addr: None,
        };

        let actions = state.process_ra(ra);
        assert_eq!(state.dns_servers, vec![dns]);
        assert!(actions.iter().any(|a| matches!(
            a,
            RaAction::UpdateDns { servers } if servers == &[dns]
        )));
    }

    #[test]
    fn test_process_ra_dnssl() {
        let mut state = RaState::new(2, "eth0".to_string(), [0; 6]);

        let ra = RouterAdvertisement {
            cur_hop_limit: 64,
            managed: false,
            other: false,
            router_lifetime: 1800,
            reachable_time: 0,
            retrans_timer: 0,
            source: Ipv6Addr::new(0xfe80, 0, 0, 0, 1, 2, 3, 4),
            prefixes: vec![],
            rdnss: vec![],
            dnssl: vec![DnsslInfo {
                lifetime: 3600,
                domains: vec!["example.com".to_string()],
            }],
            routes: vec![],
            mtu: None,
            source_ll_addr: None,
        };

        let actions = state.process_ra(ra);
        assert_eq!(state.search_domains, vec!["example.com".to_string()]);
        assert!(actions.iter().any(|a| matches!(
            a,
            RaAction::UpdateSearchDomains { domains } if domains == &["example.com".to_string()]
        )));
    }

    #[test]
    fn test_process_ra_mtu() {
        let mut state = RaState::new(2, "eth0".to_string(), [0; 6]);

        let ra = RouterAdvertisement {
            cur_hop_limit: 64,
            managed: false,
            other: false,
            router_lifetime: 1800,
            reachable_time: 0,
            retrans_timer: 0,
            source: Ipv6Addr::new(0xfe80, 0, 0, 0, 1, 2, 3, 4),
            prefixes: vec![],
            rdnss: vec![],
            dnssl: vec![],
            routes: vec![],
            mtu: Some(MtuOption { mtu: 1400 }),
            source_ll_addr: None,
        };

        let actions = state.process_ra(ra);
        assert!(
            actions
                .iter()
                .any(|a| matches!(a, RaAction::SetMtu { mtu: 1400 }))
        );
    }

    #[test]
    fn test_process_ra_mtu_too_small() {
        let mut state = RaState::new(2, "eth0".to_string(), [0; 6]);

        let ra = RouterAdvertisement {
            cur_hop_limit: 64,
            managed: false,
            other: false,
            router_lifetime: 1800,
            reachable_time: 0,
            retrans_timer: 0,
            source: Ipv6Addr::new(0xfe80, 0, 0, 0, 1, 2, 3, 4),
            prefixes: vec![],
            rdnss: vec![],
            dnssl: vec![],
            routes: vec![],
            mtu: Some(MtuOption { mtu: 1000 }), // Below IPv6 minimum of 1280
            source_ll_addr: None,
        };

        let actions = state.process_ra(ra);
        // MTU below 1280 should be ignored
        assert!(!actions.iter().any(|a| matches!(a, RaAction::SetMtu { .. })));
    }

    #[test]
    fn test_process_ra_route_info() {
        let mut state = RaState::new(2, "eth0".to_string(), [0; 6]);
        let route_prefix = Ipv6Addr::new(0x2001, 0x0db8, 0xabcd, 0, 0, 0, 0, 0);
        let source = Ipv6Addr::new(0xfe80, 0, 0, 0, 1, 2, 3, 4);

        let ra = RouterAdvertisement {
            cur_hop_limit: 64,
            managed: false,
            other: false,
            router_lifetime: 1800,
            reachable_time: 0,
            retrans_timer: 0,
            source,
            prefixes: vec![],
            rdnss: vec![],
            dnssl: vec![],
            routes: vec![RouteInfo {
                prefix_len: 48,
                preference: 1,
                lifetime: 7200,
                prefix: route_prefix,
            }],
            mtu: None,
            source_ll_addr: None,
        };

        let actions = state.process_ra(ra);
        assert!(actions.iter().any(|a| matches!(
            a,
            RaAction::AddRoute {
                prefix,
                prefix_len: 48,
                gateway,
                lifetime: 7200,
                preference: 1,
            } if *prefix == route_prefix && *gateway == source
        )));
    }

    #[test]
    fn test_process_ra_full_scenario() {
        let mac = [0x52, 0x54, 0x00, 0x12, 0x34, 0x56];
        let mut state = RaState::new(2, "eth0".to_string(), mac);
        let prefix = Ipv6Addr::new(0x2001, 0x0db8, 0, 1, 0, 0, 0, 0);
        let dns = Ipv6Addr::new(0x2001, 0x4860, 0x4860, 0, 0, 0, 0, 0x8888);
        let source = Ipv6Addr::new(0xfe80, 0, 0, 0, 1, 2, 3, 4);

        let ra = RouterAdvertisement {
            cur_hop_limit: 64,
            managed: false,
            other: false,
            router_lifetime: 1800,
            reachable_time: 30000,
            retrans_timer: 1000,
            source,
            prefixes: vec![PrefixInfo {
                prefix_len: 64,
                on_link: true,
                autonomous: true,
                valid_lifetime: 86400,
                preferred_lifetime: 3600,
                prefix,
            }],
            rdnss: vec![RdnssInfo {
                lifetime: 3600,
                servers: vec![dns],
            }],
            dnssl: vec![DnsslInfo {
                lifetime: 3600,
                domains: vec!["example.com".to_string()],
            }],
            routes: vec![],
            mtu: Some(MtuOption { mtu: 1400 }),
            source_ll_addr: Some([0x00, 0x11, 0x22, 0x33, 0x44, 0x55]),
        };

        let actions = state.process_ra(ra);

        // Verify state was updated
        assert!(state.ra_received);
        assert_eq!(state.default_router, Some(source));
        assert_eq!(state.slaac_addresses.len(), 1);
        assert_eq!(state.dns_servers, vec![dns]);
        assert_eq!(state.search_domains, vec!["example.com".to_string()]);

        // Verify actions were generated
        assert!(
            actions
                .iter()
                .any(|a| matches!(a, RaAction::AddDefaultRoute { .. }))
        );
        assert!(
            actions
                .iter()
                .any(|a| matches!(a, RaAction::AddAddress { .. }))
        );
        assert!(
            actions
                .iter()
                .any(|a| matches!(a, RaAction::AddOnLinkRoute { .. }))
        );
        assert!(
            actions
                .iter()
                .any(|a| matches!(a, RaAction::UpdateDns { .. }))
        );
        assert!(
            actions
                .iter()
                .any(|a| matches!(a, RaAction::UpdateSearchDomains { .. }))
        );
        assert!(
            actions
                .iter()
                .any(|a| matches!(a, RaAction::SetMtu { mtu: 1400 }))
        );
    }

    // -----------------------------------------------------------------------
    // RouterAdvertisement Display
    // -----------------------------------------------------------------------

    #[test]
    fn test_ra_display() {
        let source = Ipv6Addr::new(0xfe80, 0, 0, 0, 1, 2, 3, 4);
        let ra = RouterAdvertisement {
            cur_hop_limit: 64,
            managed: true,
            other: false,
            router_lifetime: 1800,
            reachable_time: 0,
            retrans_timer: 0,
            source,
            prefixes: vec![PrefixInfo {
                prefix_len: 64,
                on_link: true,
                autonomous: true,
                valid_lifetime: 86400,
                preferred_lifetime: 3600,
                prefix: Ipv6Addr::new(0x2001, 0x0db8, 0, 1, 0, 0, 0, 0),
            }],
            rdnss: vec![RdnssInfo {
                lifetime: 3600,
                servers: vec![
                    Ipv6Addr::new(0x2001, 0x4860, 0x4860, 0, 0, 0, 0, 0x8888),
                    Ipv6Addr::new(0x2001, 0x4860, 0x4860, 0, 0, 0, 0, 0x8844),
                ],
            }],
            dnssl: vec![],
            routes: vec![],
            mtu: None,
            source_ll_addr: None,
        };

        let display = format!("{ra}");
        assert!(display.contains("fe80::1:2:3:4"));
        assert!(display.contains("lifetime=1800s"));
        assert!(display.contains("managed=true"));
        assert!(display.contains("1 prefix(es)"));
        assert!(display.contains("2 RDNSS"));
    }

    // -----------------------------------------------------------------------
    // parse_ipv6 helper
    // -----------------------------------------------------------------------

    #[test]
    fn test_parse_ipv6_valid() {
        let expected = Ipv6Addr::new(0x2001, 0x0db8, 0, 0, 0, 0, 0, 1);
        let bytes = expected.octets();
        assert_eq!(parse_ipv6(&bytes), Some(expected));
    }

    #[test]
    fn test_parse_ipv6_too_short() {
        let bytes = [0u8; 15];
        assert!(parse_ipv6(&bytes).is_none());
    }

    #[test]
    fn test_parse_ipv6_loopback() {
        let expected = Ipv6Addr::LOCALHOST;
        let bytes = expected.octets();
        assert_eq!(parse_ipv6(&bytes), Some(expected));
    }

    #[test]
    fn test_parse_ipv6_unspecified() {
        let expected = Ipv6Addr::UNSPECIFIED;
        let bytes = expected.octets();
        assert_eq!(parse_ipv6(&bytes), Some(expected));
    }

    // -----------------------------------------------------------------------
    // Constants
    // -----------------------------------------------------------------------

    #[test]
    fn test_constants() {
        assert_eq!(ICMPV6_TYPE_RS, 133);
        assert_eq!(ICMPV6_TYPE_RA, 134);
        assert_eq!(NDP_OPT_PREFIX_INFO, 3);
        assert_eq!(NDP_OPT_MTU, 5);
        assert_eq!(NDP_OPT_RDNSS, 25);
        assert_eq!(NDP_OPT_DNSSL, 31);
        assert_eq!(NDP_OPT_ROUTE_INFO, 24);
        assert_eq!(NDP_OPT_SOURCE_LL_ADDR, 1);
        assert_eq!(PREFIX_FLAG_ON_LINK, 0x80);
        assert_eq!(PREFIX_FLAG_AUTONOMOUS, 0x40);
        assert_eq!(RTPROT_RA, 9);
        assert_eq!(MAX_RS_RETRANSMISSIONS, 3);
        assert_eq!(RS_RETRANSMIT_INTERVAL, Duration::from_secs(4));
    }

    #[test]
    fn test_all_routers_multicast() {
        assert_eq!(
            all_routers_multicast(),
            Ipv6Addr::new(0xff02, 0, 0, 0, 0, 0, 0, 2)
        );
    }

    #[test]
    fn test_rs_retransmit_interval() {
        assert_eq!(rs_retransmit_interval(), Duration::from_secs(4));
    }

    #[test]
    fn test_max_rs_retransmissions() {
        assert_eq!(max_rs_retransmissions(), 3);
    }

    #[test]
    fn test_rtprot_ra() {
        assert_eq!(rtprot_ra(), 9);
    }

    // -----------------------------------------------------------------------
    // Edge cases
    // -----------------------------------------------------------------------

    #[test]
    fn test_slaac_eui64_from_different_prefixes() {
        let mac = [0x52, 0x54, 0x00, 0x12, 0x34, 0x56];
        let p1 = Ipv6Addr::new(0x2001, 0x0db8, 0, 1, 0, 0, 0, 0);
        let p2 = Ipv6Addr::new(0x2001, 0x0db8, 0, 2, 0, 0, 0, 0);
        let addr1 = slaac_eui64(&p1, &mac).unwrap();
        let addr2 = slaac_eui64(&p2, &mac).unwrap();
        // Same host part, different prefix
        assert_ne!(addr1, addr2);
        // Interface ID should be the same
        assert_eq!(addr1.octets()[8..], addr2.octets()[8..]);
    }

    #[test]
    fn test_process_ra_expired_prefix_no_slaac() {
        let mac = [0x52, 0x54, 0x00, 0x12, 0x34, 0x56];
        let mut state = RaState::new(2, "eth0".to_string(), mac);
        let prefix = Ipv6Addr::new(0x2001, 0x0db8, 0, 1, 0, 0, 0, 0);

        let ra = RouterAdvertisement {
            cur_hop_limit: 64,
            managed: false,
            other: false,
            router_lifetime: 1800,
            reachable_time: 0,
            retrans_timer: 0,
            source: Ipv6Addr::new(0xfe80, 0, 0, 0, 1, 2, 3, 4),
            prefixes: vec![PrefixInfo {
                prefix_len: 64,
                on_link: true,
                autonomous: true,
                valid_lifetime: 0, // Expired
                preferred_lifetime: 0,
                prefix,
            }],
            rdnss: vec![],
            dnssl: vec![],
            routes: vec![],
            mtu: None,
            source_ll_addr: None,
        };

        let actions = state.process_ra(ra);
        // Zero valid_lifetime should not generate SLAAC address
        assert!(state.slaac_addresses.is_empty());
        assert!(
            !actions
                .iter()
                .any(|a| matches!(a, RaAction::AddAddress { .. }))
        );
    }

    #[test]
    fn test_process_ra_non_64_prefix_no_slaac() {
        let mac = [0x52, 0x54, 0x00, 0x12, 0x34, 0x56];
        let mut state = RaState::new(2, "eth0".to_string(), mac);
        let prefix = Ipv6Addr::new(0x2001, 0x0db8, 0, 0, 0, 0, 0, 0);

        let ra = RouterAdvertisement {
            cur_hop_limit: 64,
            managed: false,
            other: false,
            router_lifetime: 1800,
            reachable_time: 0,
            retrans_timer: 0,
            source: Ipv6Addr::new(0xfe80, 0, 0, 0, 1, 2, 3, 4),
            prefixes: vec![PrefixInfo {
                prefix_len: 48, // Not /64
                on_link: true,
                autonomous: true,
                valid_lifetime: 86400,
                preferred_lifetime: 3600,
                prefix,
            }],
            rdnss: vec![],
            dnssl: vec![],
            routes: vec![],
            mtu: None,
            source_ll_addr: None,
        };

        let actions = state.process_ra(ra);
        // Non-/64 prefix should not generate SLAAC address
        assert!(state.slaac_addresses.is_empty());
        assert!(
            !actions
                .iter()
                .any(|a| matches!(a, RaAction::AddAddress { .. }))
        );
    }

    #[test]
    fn test_process_ra_rdnss_zero_lifetime_no_update() {
        let mut state = RaState::new(2, "eth0".to_string(), [0; 6]);

        let ra = RouterAdvertisement {
            cur_hop_limit: 64,
            managed: false,
            other: false,
            router_lifetime: 1800,
            reachable_time: 0,
            retrans_timer: 0,
            source: Ipv6Addr::new(0xfe80, 0, 0, 0, 1, 2, 3, 4),
            prefixes: vec![],
            rdnss: vec![RdnssInfo {
                lifetime: 0, // Expire these servers
                servers: vec![Ipv6Addr::new(0x2001, 0x4860, 0x4860, 0, 0, 0, 0, 0x8888)],
            }],
            dnssl: vec![],
            routes: vec![],
            mtu: None,
            source_ll_addr: None,
        };

        let actions = state.process_ra(ra);
        assert!(state.dns_servers.is_empty());
        assert!(
            !actions
                .iter()
                .any(|a| matches!(a, RaAction::UpdateDns { .. }))
        );
    }

    #[test]
    fn test_process_ra_route_zero_lifetime_no_action() {
        let mut state = RaState::new(2, "eth0".to_string(), [0; 6]);

        let ra = RouterAdvertisement {
            cur_hop_limit: 64,
            managed: false,
            other: false,
            router_lifetime: 1800,
            reachable_time: 0,
            retrans_timer: 0,
            source: Ipv6Addr::new(0xfe80, 0, 0, 0, 1, 2, 3, 4),
            prefixes: vec![],
            rdnss: vec![],
            dnssl: vec![],
            routes: vec![RouteInfo {
                prefix_len: 48,
                preference: 0,
                lifetime: 0, // Expired
                prefix: Ipv6Addr::new(0x2001, 0x0db8, 0, 0, 0, 0, 0, 0),
            }],
            mtu: None,
            source_ll_addr: None,
        };

        let actions = state.process_ra(ra);
        assert!(
            !actions
                .iter()
                .any(|a| matches!(a, RaAction::AddRoute { .. }))
        );
    }
}
