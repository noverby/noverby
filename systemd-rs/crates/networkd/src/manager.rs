//! Network manager — coordinates `.network` config files, DHCP clients,
//! static address configuration, and interface lifecycle.
//!
//! This is the core orchestration layer of `systemd-networkd`. It:
//! - Loads `.network` configuration files
//! - Enumerates network interfaces
//! - Matches configs to interfaces
//! - Runs DHCP clients for interfaces with `DHCP=yes`/`DHCP=ipv4`
//! - Applies static addresses and routes
//! - Writes DNS configuration to `/run/systemd/resolve/resolv.conf`
//! - Sends sd_notify status updates

use std::collections::HashMap;
use std::fmt;
use std::net::Ipv4Addr;

use crate::config::{self, DhcpMode, NetworkConfig};
use crate::dhcp::{self, DhcpClient, DhcpClientConfig, DhcpLease, DhcpState};
use crate::link::{self, LinkInfo};

// ---------------------------------------------------------------------------
// Managed link — per-interface state
// ---------------------------------------------------------------------------

/// The configuration + runtime state of a single managed network interface.
#[derive(Debug)]
pub struct ManagedLink {
    /// Kernel link information.
    pub link: LinkInfo,

    /// The `.network` config file that matched this interface (if any).
    pub config: Option<NetworkConfig>,

    /// DHCP client state machine (if DHCP is enabled).
    pub dhcp_client: Option<DhcpClient>,

    /// Current DHCP lease (if obtained).
    pub lease: Option<DhcpLease>,

    /// Administrative state we want the link to be in.
    pub admin_state: AdminState,

    /// Whether static addresses have been applied.
    pub static_configured: bool,

    /// Whether the link has carrier (physical connection).
    pub has_carrier: bool,

    /// DNS servers collected from DHCP and/or static config.
    pub dns_servers: Vec<Ipv4Addr>,

    /// Search domains collected from DHCP and/or static config.
    pub search_domains: Vec<String>,
}

/// Desired administrative state of a link.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdminState {
    /// The link should be brought up and configured.
    Up,
    /// The link should be left alone (unmanaged).
    Unmanaged,
    /// The link should be kept down.
    Down,
}

impl fmt::Display for AdminState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Up => write!(f, "configured"),
            Self::Unmanaged => write!(f, "unmanaged"),
            Self::Down => write!(f, "down"),
        }
    }
}

/// Summary of the operational state of a managed link (for networkctl output).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OperState {
    /// No configuration matched.
    Unmanaged,
    /// Configuration matched but no carrier / not configured yet.
    Configuring,
    /// Waiting for DHCP lease.
    Pending,
    /// Fully configured (addresses and routes applied).
    Configured,
    /// Link is degraded (e.g. DHCP failed, using fallback).
    Degraded,
    /// Link has no carrier.
    NoCarrier,
}

impl fmt::Display for OperState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unmanaged => write!(f, "unmanaged"),
            Self::Configuring => write!(f, "configuring"),
            Self::Pending => write!(f, "pending"),
            Self::Configured => write!(f, "configured"),
            Self::Degraded => write!(f, "degraded"),
            Self::NoCarrier => write!(f, "no-carrier"),
        }
    }
}

impl ManagedLink {
    /// Compute the operational state of this link.
    pub fn oper_state(&self) -> OperState {
        if self.config.is_none() {
            return OperState::Unmanaged;
        }
        if !self.has_carrier && !self.link.is_loopback() {
            return OperState::NoCarrier;
        }
        if let Some(ref cfg) = self.config {
            let needs_dhcp = matches!(cfg.network_section.dhcp, DhcpMode::Yes | DhcpMode::Ipv4);
            if needs_dhcp && self.lease.is_none() {
                return OperState::Pending;
            }
        }
        if !self.static_configured {
            return OperState::Configuring;
        }
        OperState::Configured
    }
}

// ---------------------------------------------------------------------------
// Network Manager
// ---------------------------------------------------------------------------

/// The main network manager that coordinates all managed links.
pub struct NetworkManager {
    /// All managed links, keyed by interface index.
    pub links: HashMap<u32, ManagedLink>,

    /// Loaded `.network` config files.
    pub configs: Vec<NetworkConfig>,

    /// Global DNS servers (aggregated from all links).
    pub dns_servers: Vec<Ipv4Addr>,

    /// Global search domains.
    pub search_domains: Vec<String>,

    /// Whether we've completed initial configuration.
    pub initial_config_done: bool,
}

impl NetworkManager {
    /// Create a new network manager with no state.
    pub fn new() -> Self {
        Self {
            links: HashMap::new(),
            configs: Vec::new(),
            dns_servers: Vec::new(),
            search_domains: Vec::new(),
            initial_config_done: false,
        }
    }

    /// Load `.network` configuration files from standard paths.
    pub fn load_configs(&mut self) {
        self.configs = config::load_network_configs();
        log::info!("Loaded {} .network config file(s)", self.configs.len());
        for cfg in &self.configs {
            log::debug!(
                "  {} — match=[{}] dhcp={}",
                cfg.path.display(),
                cfg.match_section.names.join(", "),
                cfg.network_section.dhcp,
            );
        }
    }

    /// Load configs from explicit directories (for testing).
    pub fn load_configs_from(&mut self, dirs: &[std::path::PathBuf]) {
        self.configs = config::load_network_configs_from(dirs);
    }

    /// Discover network interfaces and match them against configs.
    pub fn discover_links(&mut self) -> Result<(), String> {
        let system_links =
            link::list_links().map_err(|e| format!("failed to enumerate links: {e}"))?;

        for li in system_links {
            // Skip loopback — it's always configured by the kernel.
            if li.is_loopback() {
                continue;
            }

            let ifindex = li.index;
            let mac_str = li.mac.clone();
            let name = li.name.clone();

            // Find the first matching config.
            let matched_config = self.configs.iter().find(|cfg| {
                cfg.match_section
                    .matches_interface(&name, Some(&mac_str), None)
            });

            let admin_state = match matched_config {
                Some(cfg) if cfg.link.unmanaged => AdminState::Unmanaged,
                Some(cfg) => match cfg.link.activation_policy.as_deref() {
                    Some("down") | Some("always-down") => AdminState::Down,
                    Some("manual") => AdminState::Unmanaged,
                    _ => AdminState::Up,
                },
                None => AdminState::Unmanaged,
            };

            let has_carrier = li.is_running() || li.is_loopback();

            let managed = ManagedLink {
                link: li,
                config: matched_config.cloned(),
                dhcp_client: None,
                lease: None,
                admin_state,
                static_configured: false,
                has_carrier,
                dns_servers: Vec::new(),
                search_domains: Vec::new(),
            };

            if matched_config.is_some() {
                log::info!(
                    "Link {} (idx={}) matched config {}",
                    name,
                    ifindex,
                    matched_config
                        .unwrap()
                        .path
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy(),
                );
            } else {
                log::debug!(
                    "Link {} (idx={}) — no matching config (unmanaged)",
                    name,
                    ifindex
                );
            }

            self.links.insert(ifindex, managed);
        }

        Ok(())
    }

    /// Configure all managed links: bring up interfaces, apply static
    /// addresses, start DHCP clients.
    pub fn configure_links(&mut self) -> Result<(), String> {
        let indices: Vec<u32> = self.links.keys().copied().collect();

        for ifindex in indices {
            if let Err(e) = self.configure_link(ifindex) {
                log::warn!("Failed to configure link idx={}: {}", ifindex, e);
            }
        }

        self.initial_config_done = true;
        self.update_global_dns();

        Ok(())
    }

    /// Configure a single link.
    fn configure_link(&mut self, ifindex: u32) -> Result<(), String> {
        let managed = match self.links.get(&ifindex) {
            Some(m) => m,
            None => return Err(format!("unknown ifindex {ifindex}")),
        };

        if managed.admin_state != AdminState::Up {
            log::debug!(
                "Skipping {} (idx={}) — state={}",
                managed.link.name,
                ifindex,
                managed.admin_state
            );
            return Ok(());
        }

        let config = match &managed.config {
            Some(c) => c.clone(),
            None => return Ok(()),
        };

        let link_name = managed.link.name.clone();
        let mac = managed.link.mac_bytes.clone();

        // 1. Bring the interface up.
        log::info!("Bringing up {link_name} (idx={ifindex})");
        link::set_link_up(ifindex, true).map_err(|e| format!("set_link_up({link_name}): {e}"))?;

        // 2. Set MTU if specified.
        if let Some(mtu) = config.link.mtu {
            log::info!("Setting MTU on {link_name} to {mtu}");
            link::set_link_mtu(ifindex, mtu).map_err(|e| format!("set_mtu({link_name}): {e}"))?;
        }

        // 3. Apply static addresses.
        for addr_cfg in &config.addresses {
            if let Some((ip, prefix)) = config::parse_ipv4_cidr(&addr_cfg.address) {
                let broadcast = addr_cfg
                    .broadcast
                    .as_ref()
                    .and_then(|b| b.parse::<Ipv4Addr>().ok())
                    .unwrap_or_else(|| config::ipv4_broadcast(ip, prefix));

                log::info!("Adding address {ip}/{prefix} brd {broadcast} to {link_name}");
                if let Err(e) = link::add_address(ifindex, ip, prefix, Some(broadcast)) {
                    // EEXIST is fine — address already configured.
                    let is_exists = e.raw_os_error() == Some(libc::EEXIST);
                    if !is_exists {
                        log::warn!("Failed to add address {ip}/{prefix} to {link_name}: {e}");
                    }
                }
            }
        }

        // 4. Apply static routes.
        for route_cfg in &config.routes {
            let gateway = route_cfg
                .gateway
                .as_ref()
                .and_then(|g| g.parse::<Ipv4Addr>().ok());

            let (dest, prefix_len) = match &route_cfg.destination {
                Some(d) => config::parse_ipv4_cidr(d).unwrap_or((Ipv4Addr::UNSPECIFIED, 0)),
                None => (Ipv4Addr::UNSPECIFIED, 0),
            };

            let metric = route_cfg.metric;

            log::info!(
                "Adding route {dest}/{prefix_len} via {:?} metric {:?} to {link_name}",
                gateway,
                metric,
            );

            if let Err(e) = link::add_route(
                dest,
                prefix_len,
                gateway,
                ifindex,
                metric,
                link::rtprot_static(),
            ) {
                let is_exists = e.raw_os_error() == Some(libc::EEXIST);
                if !is_exists {
                    log::warn!("Failed to add route to {link_name}: {e}");
                }
            }
        }

        // 5. Collect static DNS/domains.
        let managed = self.links.get_mut(&ifindex).unwrap();
        for dns in &config.network_section.dns {
            if let std::net::IpAddr::V4(v4) = dns {
                managed.dns_servers.push(*v4);
            }
        }
        managed
            .search_domains
            .extend(config.network_section.domains.clone());
        managed.static_configured = true;

        // 6. Start DHCP client if needed.
        let needs_dhcp = matches!(config.network_section.dhcp, DhcpMode::Yes | DhcpMode::Ipv4);

        if needs_dhcp {
            let mut mac_arr = [0u8; 6];
            if mac.len() >= 6 {
                mac_arr.copy_from_slice(&mac[..6]);
            }

            let hostname = config.dhcpv4.hostname.clone().or_else(|| {
                if config.dhcpv4.send_hostname {
                    nix::unistd::gethostname()
                        .ok()
                        .and_then(|h| h.into_string().ok())
                } else {
                    None
                }
            });

            let dhcp_config = DhcpClientConfig {
                ifindex,
                ifname: link_name.clone(),
                mac: mac_arr,
                hostname,
                vendor_class_id: config.dhcpv4.vendor_class_id.clone(),
                client_identifier: if config.dhcpv4.client_identifier.as_deref() == Some("duid") {
                    dhcp::ClientIdMode::Duid
                } else {
                    dhcp::ClientIdMode::Mac
                },
                request_broadcast: config.dhcpv4.request_broadcast,
                route_metric: config.dhcpv4.route_metric.unwrap_or(1024),
                max_attempts: config.dhcpv4.max_attempts.unwrap_or(0),
                ..Default::default()
            };

            log::info!("Starting DHCP client on {link_name}");
            let client = DhcpClient::new(dhcp_config);
            let managed = self.links.get_mut(&ifindex).unwrap();
            managed.dhcp_client = Some(client);
        }

        Ok(())
    }

    /// Apply a DHCP lease to an interface: set address, routes, DNS.
    pub fn apply_lease(&mut self, ifindex: u32, lease: &DhcpLease) -> Result<(), String> {
        let managed = match self.links.get(&ifindex) {
            Some(m) => m,
            None => return Err(format!("unknown ifindex {ifindex}")),
        };

        let link_name = managed.link.name.clone();
        let use_dns = managed
            .config
            .as_ref()
            .map(|c| c.dhcpv4.use_dns)
            .unwrap_or(true);
        let use_routes = managed
            .config
            .as_ref()
            .map(|c| c.dhcpv4.use_routes)
            .unwrap_or(true);
        let use_hostname = managed
            .config
            .as_ref()
            .map(|c| c.dhcpv4.use_hostname)
            .unwrap_or(true);
        let use_mtu = managed
            .config
            .as_ref()
            .map(|c| c.dhcpv4.use_mtu)
            .unwrap_or(true);
        let route_metric = managed
            .config
            .as_ref()
            .and_then(|c| c.dhcpv4.route_metric)
            .unwrap_or(1024);

        // 1. Flush existing DHCP-assigned addresses/routes.
        // (In a real implementation we'd only flush what we previously set.)
        let _ = link::flush_addresses(ifindex);
        let _ = link::flush_routes(ifindex);

        // 2. Add the leased address.
        let broadcast = lease
            .broadcast
            .unwrap_or_else(|| config::ipv4_broadcast(lease.address, lease.prefix_len()));

        log::info!(
            "{link_name}: DHCP address {}/{} brd {broadcast}",
            lease.address,
            lease.prefix_len()
        );

        link::add_address(ifindex, lease.address, lease.prefix_len(), Some(broadcast))
            .map_err(|e| format!("add_address({link_name}): {e}"))?;

        // 3. Add routes.
        if use_routes {
            // Classless routes take priority (RFC 3442).
            if !lease.classless_routes.is_empty() {
                for &(dest, prefix_len, gw) in &lease.classless_routes {
                    log::info!(
                        "{link_name}: DHCP route {dest}/{prefix_len} via {gw} metric {route_metric}"
                    );
                    let _ = link::add_route(
                        dest,
                        prefix_len,
                        Some(gw),
                        ifindex,
                        Some(route_metric),
                        link::rtprot_dhcp(),
                    );
                }
            } else {
                // Default gateway from routers option.
                for gw in &lease.routers {
                    log::info!("{link_name}: DHCP default route via {gw} metric {route_metric}");
                    let _ = link::add_default_route(
                        *gw,
                        ifindex,
                        Some(route_metric),
                        link::rtprot_dhcp(),
                    );
                }
            }

            // On-link route for the subnet.
            let network = config::ipv4_network(lease.address, lease.prefix_len());
            let _ = link::add_route(
                network,
                lease.prefix_len(),
                None,
                ifindex,
                Some(route_metric),
                link::rtprot_dhcp(),
            );
        }

        // 4. Update MTU if offered and enabled.
        if use_mtu
            && let Some(mtu) = lease.mtu
            && mtu >= 576
        {
            log::info!("{link_name}: DHCP MTU {mtu}");
            let _ = link::set_link_mtu(ifindex, u32::from(mtu));
        }

        // 5. Update hostname if offered and enabled.
        if use_hostname && let Some(ref hostname) = lease.hostname {
            log::info!("{link_name}: DHCP hostname '{hostname}'");
            let _ = nix::unistd::sethostname(hostname);
        }

        // 6. Update DNS / domains on the managed link.
        let managed = self.links.get_mut(&ifindex).unwrap();
        managed.lease = Some(lease.clone());

        if use_dns {
            managed.dns_servers = lease.dns_servers.clone();
        }
        if let Some(ref domain) = lease.domain_name {
            managed.search_domains = vec![domain.clone()];
        }

        self.update_global_dns();

        log::info!(
            "{link_name}: DHCP lease applied — {} (lease {}s, renew {}s)",
            lease.address,
            lease.lease_time,
            lease.renewal_time,
        );

        Ok(())
    }

    /// Remove DHCP-learned configuration from a link (on lease expiry or release).
    pub fn remove_lease(&mut self, ifindex: u32) -> Result<(), String> {
        let managed = match self.links.get_mut(&ifindex) {
            Some(m) => m,
            None => return Err(format!("unknown ifindex {ifindex}")),
        };

        let link_name = managed.link.name.clone();
        log::info!("{link_name}: removing DHCP lease configuration");

        // Flush DHCP-assigned addresses and routes.
        let _ = link::flush_addresses(ifindex);
        let _ = link::flush_routes(ifindex);

        managed.lease = None;
        managed.dns_servers.clear();
        managed.search_domains.clear();

        // Re-apply static config if any.
        if let Some(ref cfg) = managed.config {
            for dns in &cfg.network_section.dns {
                if let std::net::IpAddr::V4(v4) = dns {
                    managed.dns_servers.push(*v4);
                }
            }
            managed
                .search_domains
                .extend(cfg.network_section.domains.clone());
        }

        self.update_global_dns();

        Ok(())
    }

    /// Aggregate DNS servers and search domains from all managed links and
    /// write `/run/systemd/resolve/resolv.conf`.
    fn update_global_dns(&mut self) {
        let mut dns = Vec::new();
        let mut domains = Vec::new();

        for managed in self.links.values() {
            for server in &managed.dns_servers {
                if !dns.contains(server) {
                    dns.push(*server);
                }
            }
            for domain in &managed.search_domains {
                if !domains.contains(domain) {
                    domains.push(domain.clone());
                }
            }
        }

        self.dns_servers = dns.clone();
        self.search_domains = domains.clone();

        if let Err(e) = link::write_resolv_conf(&dns, &domains) {
            log::warn!("Failed to write resolv.conf: {e}");
        }
    }

    /// Get a summary of the operational state of all managed links.
    pub fn status_summary(&self) -> Vec<LinkStatus> {
        let mut result = Vec::new();
        for managed in self.links.values() {
            result.push(LinkStatus {
                index: managed.link.index,
                name: managed.link.name.clone(),
                link_type: if managed.link.is_loopback() {
                    "loopback"
                } else {
                    "ether"
                }
                .to_string(),
                oper_state: managed.oper_state(),
                admin_state: managed.admin_state,
                config_file: managed
                    .config
                    .as_ref()
                    .map(|c| c.path.display().to_string()),
                address: managed
                    .lease
                    .as_ref()
                    .map(|l| format!("{}/{}", l.address, l.prefix_len())),
                gateway: managed
                    .lease
                    .as_ref()
                    .and_then(|l| l.routers.first().map(|r| r.to_string())),
                dns: managed.dns_servers.iter().map(|d| d.to_string()).collect(),
                dhcp_state: managed.dhcp_client.as_ref().map(|c| c.state.to_string()),
            });
        }
        result.sort_by_key(|s| s.index);
        result
    }

    /// Returns the overall operational state of the system.
    /// Used for sd_notify STATUS.
    pub fn overall_state(&self) -> &'static str {
        if !self.initial_config_done {
            return "initializing";
        }

        let mut any_configured = false;
        let mut any_pending = false;
        let mut any_degraded = false;

        for managed in self.links.values() {
            match managed.oper_state() {
                OperState::Configured => any_configured = true,
                OperState::Pending | OperState::Configuring => any_pending = true,
                OperState::Degraded => any_degraded = true,
                _ => {}
            }
        }

        if any_pending {
            "configuring"
        } else if any_degraded {
            "degraded"
        } else if any_configured {
            "configured"
        } else {
            "no-carrier"
        }
    }

    /// Write runtime state files to `/run/systemd/netif/`.
    pub fn write_state_files(&self) {
        let state_dir = std::path::Path::new("/run/systemd/netif/links");
        if let Err(e) = std::fs::create_dir_all(state_dir) {
            log::debug!("Cannot create {}: {}", state_dir.display(), e);
            return;
        }

        let lease_dir = std::path::Path::new("/run/systemd/netif/leases");
        let _ = std::fs::create_dir_all(lease_dir);

        for managed in self.links.values() {
            // Write link state file.
            let link_file = state_dir.join(managed.link.index.to_string());
            let mut content = String::new();
            content.push_str("# systemd-networkd state file\n");
            content.push_str(&format!("ADMIN_STATE={}\n", managed.admin_state));
            content.push_str(&format!("OPER_STATE={}\n", managed.oper_state()));
            if let Some(ref cfg) = managed.config {
                content.push_str(&format!("NETWORK_FILE={}\n", cfg.path.display()));
            }
            for dns in &managed.dns_servers {
                content.push_str(&format!("DNS={dns}\n"));
            }
            for domain in &managed.search_domains {
                content.push_str(&format!("DOMAINS={domain}\n"));
            }
            let _ = std::fs::write(&link_file, &content);

            // Write lease state file if we have a lease.
            if let Some(ref lease) = managed.lease {
                let lease_file = lease_dir.join(managed.link.index.to_string());
                let mut lc = String::new();
                lc.push_str(&format!("ADDRESS={}\n", lease.address));
                lc.push_str(&format!("NETMASK={}\n", lease.subnet_mask));
                lc.push_str(&format!("SERVER_ADDRESS={}\n", lease.server_id));
                lc.push_str(&format!("LIFETIME={}\n", lease.lease_time));
                lc.push_str(&format!("T1={}\n", lease.renewal_time));
                lc.push_str(&format!("T2={}\n", lease.rebinding_time));
                for gw in &lease.routers {
                    lc.push_str(&format!("ROUTER={gw}\n"));
                }
                for dns in &lease.dns_servers {
                    lc.push_str(&format!("DNS={dns}\n"));
                }
                if let Some(ref hostname) = lease.hostname {
                    lc.push_str(&format!("HOSTNAME={hostname}\n"));
                }
                if let Some(ref domain) = lease.domain_name {
                    lc.push_str(&format!("DOMAINNAME={domain}\n"));
                }
                let _ = std::fs::write(&lease_file, &lc);
            }
        }

        // Write overall state.
        let state_file = std::path::Path::new("/run/systemd/netif/state");
        let mut content = String::new();
        content.push_str("# systemd-networkd overall state\n");
        content.push_str(&format!("OPER_STATE={}\n", self.overall_state()));
        for dns in &self.dns_servers {
            content.push_str(&format!("DNS={dns}\n"));
        }
        for domain in &self.search_domains {
            content.push_str(&format!("DOMAINS={domain}\n"));
        }
        let _ = std::fs::write(state_file, &content);
    }

    /// Return the list of interface indices that have active DHCP clients
    /// needing to send/receive packets.
    pub fn dhcp_active_links(&self) -> Vec<u32> {
        self.links
            .iter()
            .filter_map(|(&idx, managed)| {
                if managed.dhcp_client.is_some() {
                    Some(idx)
                } else {
                    None
                }
            })
            .collect()
    }

    /// Check if all links that need DHCP have obtained leases.
    pub fn all_dhcp_bound(&self) -> bool {
        self.links.values().all(|managed| {
            match &managed.dhcp_client {
                Some(client) => client.state == DhcpState::Bound,
                None => true, // No DHCP needed.
            }
        })
    }

    /// Release all DHCP leases (called on shutdown).
    pub fn release_all(&mut self) {
        let indices: Vec<u32> = self.dhcp_active_links();
        for ifindex in indices {
            if let Some(managed) = self.links.get(&ifindex)
                && let Some(ref client) = managed.dhcp_client
                && let Some(release_pkt) = client.build_release()
            {
                log::info!(
                    "{}: sending DHCPRELEASE for {}",
                    managed.link.name,
                    client
                        .lease
                        .as_ref()
                        .map(|l| l.address.to_string())
                        .unwrap_or_default(),
                );
                // In a real implementation, we'd send this packet.
                // For now we just log it.
                let _ = release_pkt;
            }
            let _ = self.remove_lease(ifindex);
        }
    }
}

/// Summary status of a managed link (for display by `networkctl`).
#[derive(Debug, Clone)]
pub struct LinkStatus {
    pub index: u32,
    pub name: String,
    pub link_type: String,
    pub oper_state: OperState,
    pub admin_state: AdminState,
    pub config_file: Option<String>,
    pub address: Option<String>,
    pub gateway: Option<String>,
    pub dns: Vec<String>,
    pub dhcp_state: Option<String>,
}

impl fmt::Display for LinkStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{:>3} {} {:10} {:12}",
            self.index, self.name, self.link_type, self.oper_state,
        )?;
        if let Some(ref addr) = self.address {
            write!(f, " {addr}")?;
        }
        if let Some(ref gw) = self.gateway {
            write!(f, " gw={gw}")?;
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::*;

    fn make_test_config(name: &str, dhcp: DhcpMode) -> NetworkConfig {
        NetworkConfig {
            path: std::path::PathBuf::from(format!("10-{name}.network")),
            match_section: MatchSection {
                names: vec![name.to_string()],
                ..Default::default()
            },
            network_section: NetworkSection {
                dhcp,
                ..Default::default()
            },
            addresses: Vec::new(),
            routes: Vec::new(),
            dhcpv4: DhcpV4Section::default(),
            link: LinkSection::default(),
        }
    }

    fn make_test_link(index: u32, name: &str) -> LinkInfo {
        LinkInfo {
            index,
            name: name.to_string(),
            mac: "52:54:00:12:34:56".to_string(),
            mac_bytes: vec![0x52, 0x54, 0x00, 0x12, 0x34, 0x56],
            mtu: 1500,
            flags: 0x1 | 0x40, // UP | RUNNING
            operstate: 6,      // IF_OPER_UP
        }
    }

    #[test]
    fn test_new_manager() {
        let mgr = NetworkManager::new();
        assert!(mgr.links.is_empty());
        assert!(mgr.configs.is_empty());
        assert!(!mgr.initial_config_done);
    }

    #[test]
    fn test_managed_link_unmanaged_state() {
        let managed = ManagedLink {
            link: make_test_link(1, "eth0"),
            config: None,
            dhcp_client: None,
            lease: None,
            admin_state: AdminState::Up,
            static_configured: false,
            has_carrier: true,
            dns_servers: Vec::new(),
            search_domains: Vec::new(),
        };
        assert_eq!(managed.oper_state(), OperState::Unmanaged);
    }

    #[test]
    fn test_managed_link_pending_state() {
        let managed = ManagedLink {
            link: make_test_link(2, "eth0"),
            config: Some(make_test_config("eth*", DhcpMode::Yes)),
            dhcp_client: None,
            lease: None,
            admin_state: AdminState::Up,
            static_configured: true,
            has_carrier: true,
            dns_servers: Vec::new(),
            search_domains: Vec::new(),
        };
        assert_eq!(managed.oper_state(), OperState::Pending);
    }

    #[test]
    fn test_managed_link_configured_state() {
        let managed = ManagedLink {
            link: make_test_link(3, "eth0"),
            config: Some(make_test_config("eth*", DhcpMode::No)),
            dhcp_client: None,
            lease: None,
            admin_state: AdminState::Up,
            static_configured: true,
            has_carrier: true,
            dns_servers: Vec::new(),
            search_domains: Vec::new(),
        };
        assert_eq!(managed.oper_state(), OperState::Configured);
    }

    #[test]
    fn test_managed_link_no_carrier() {
        let mut link = make_test_link(4, "eth0");
        link.flags = 0; // Not UP, not RUNNING
        let managed = ManagedLink {
            link,
            config: Some(make_test_config("eth*", DhcpMode::No)),
            dhcp_client: None,
            lease: None,
            admin_state: AdminState::Up,
            static_configured: true,
            has_carrier: false,
            dns_servers: Vec::new(),
            search_domains: Vec::new(),
        };
        assert_eq!(managed.oper_state(), OperState::NoCarrier);
    }

    #[test]
    fn test_managed_link_configuring_state() {
        let managed = ManagedLink {
            link: make_test_link(5, "eth0"),
            config: Some(make_test_config("eth*", DhcpMode::No)),
            dhcp_client: None,
            lease: None,
            admin_state: AdminState::Up,
            static_configured: false,
            has_carrier: true,
            dns_servers: Vec::new(),
            search_domains: Vec::new(),
        };
        assert_eq!(managed.oper_state(), OperState::Configuring);
    }

    #[test]
    fn test_admin_state_display() {
        assert_eq!(AdminState::Up.to_string(), "configured");
        assert_eq!(AdminState::Unmanaged.to_string(), "unmanaged");
        assert_eq!(AdminState::Down.to_string(), "down");
    }

    #[test]
    fn test_oper_state_display() {
        assert_eq!(OperState::Unmanaged.to_string(), "unmanaged");
        assert_eq!(OperState::Configuring.to_string(), "configuring");
        assert_eq!(OperState::Pending.to_string(), "pending");
        assert_eq!(OperState::Configured.to_string(), "configured");
        assert_eq!(OperState::Degraded.to_string(), "degraded");
        assert_eq!(OperState::NoCarrier.to_string(), "no-carrier");
    }

    #[test]
    fn test_overall_state_initializing() {
        let mgr = NetworkManager::new();
        assert_eq!(mgr.overall_state(), "initializing");
    }

    #[test]
    fn test_overall_state_no_carrier() {
        let mut mgr = NetworkManager::new();
        mgr.initial_config_done = true;
        assert_eq!(mgr.overall_state(), "no-carrier");
    }

    #[test]
    fn test_overall_state_configured() {
        let mut mgr = NetworkManager::new();
        mgr.initial_config_done = true;
        mgr.links.insert(
            1,
            ManagedLink {
                link: make_test_link(1, "eth0"),
                config: Some(make_test_config("eth*", DhcpMode::No)),
                dhcp_client: None,
                lease: None,
                admin_state: AdminState::Up,
                static_configured: true,
                has_carrier: true,
                dns_servers: Vec::new(),
                search_domains: Vec::new(),
            },
        );
        assert_eq!(mgr.overall_state(), "configured");
    }

    #[test]
    fn test_status_summary_empty() {
        let mgr = NetworkManager::new();
        let status = mgr.status_summary();
        assert!(status.is_empty());
    }

    #[test]
    fn test_status_summary_single_link() {
        let mut mgr = NetworkManager::new();
        mgr.links.insert(
            2,
            ManagedLink {
                link: make_test_link(2, "ens3"),
                config: Some(make_test_config("ens*", DhcpMode::Yes)),
                dhcp_client: None,
                lease: None,
                admin_state: AdminState::Up,
                static_configured: true,
                has_carrier: true,
                dns_servers: vec![Ipv4Addr::new(8, 8, 8, 8)],
                search_domains: vec!["example.com".into()],
            },
        );

        let statuses = mgr.status_summary();
        assert_eq!(statuses.len(), 1);
        assert_eq!(statuses[0].name, "ens3");
        assert_eq!(statuses[0].oper_state, OperState::Pending);
        assert_eq!(statuses[0].dns, vec!["8.8.8.8"]);
    }

    #[test]
    fn test_link_status_display() {
        let status = LinkStatus {
            index: 2,
            name: "eth0".into(),
            link_type: "ether".into(),
            oper_state: OperState::Configured,
            admin_state: AdminState::Up,
            config_file: Some("/etc/systemd/network/10-eth.network".into()),
            address: Some("192.168.1.100/24".into()),
            gateway: Some("192.168.1.1".into()),
            dns: vec!["8.8.8.8".into()],
            dhcp_state: Some("BOUND".into()),
        };

        let s = status.to_string();
        assert!(s.contains("eth0"));
        assert!(s.contains("ether"));
        assert!(s.contains("configured"));
        assert!(s.contains("192.168.1.100/24"));
        assert!(s.contains("gw=192.168.1.1"));
    }

    #[test]
    fn test_dhcp_active_links() {
        let mut mgr = NetworkManager::new();

        // Link without DHCP.
        mgr.links.insert(
            1,
            ManagedLink {
                link: make_test_link(1, "lo"),
                config: None,
                dhcp_client: None,
                lease: None,
                admin_state: AdminState::Unmanaged,
                static_configured: false,
                has_carrier: true,
                dns_servers: Vec::new(),
                search_domains: Vec::new(),
            },
        );

        // Link with DHCP.
        let dhcp_config = DhcpClientConfig {
            ifindex: 2,
            ifname: "eth0".into(),
            mac: [0x52, 0x54, 0x00, 0x12, 0x34, 0x56],
            ..Default::default()
        };
        mgr.links.insert(
            2,
            ManagedLink {
                link: make_test_link(2, "eth0"),
                config: Some(make_test_config("eth*", DhcpMode::Yes)),
                dhcp_client: Some(DhcpClient::new(dhcp_config)),
                lease: None,
                admin_state: AdminState::Up,
                static_configured: true,
                has_carrier: true,
                dns_servers: Vec::new(),
                search_domains: Vec::new(),
            },
        );

        let active = mgr.dhcp_active_links();
        assert_eq!(active.len(), 1);
        assert!(active.contains(&2));
    }

    #[test]
    fn test_all_dhcp_bound_no_dhcp() {
        let mgr = NetworkManager::new();
        assert!(mgr.all_dhcp_bound());
    }

    #[test]
    fn test_all_dhcp_bound_not_bound() {
        let mut mgr = NetworkManager::new();
        let dhcp_config = DhcpClientConfig {
            ifindex: 2,
            ifname: "eth0".into(),
            mac: [0x52, 0x54, 0x00, 0x12, 0x34, 0x56],
            ..Default::default()
        };
        mgr.links.insert(
            2,
            ManagedLink {
                link: make_test_link(2, "eth0"),
                config: Some(make_test_config("eth*", DhcpMode::Yes)),
                dhcp_client: Some(DhcpClient::new(dhcp_config)),
                lease: None,
                admin_state: AdminState::Up,
                static_configured: true,
                has_carrier: true,
                dns_servers: Vec::new(),
                search_domains: Vec::new(),
            },
        );

        assert!(!mgr.all_dhcp_bound());
    }

    #[test]
    fn test_load_configs_from_empty() {
        let mut mgr = NetworkManager::new();
        let dir = tempfile::tempdir().unwrap();
        mgr.load_configs_from(&[dir.path().to_path_buf()]);
        assert!(mgr.configs.is_empty());
    }

    #[test]
    fn test_load_configs_from_with_files() {
        let mut mgr = NetworkManager::new();
        let dir = tempfile::tempdir().unwrap();

        std::fs::write(
            dir.path().join("10-eth.network"),
            "[Match]\nName=eth*\n\n[Network]\nDHCP=yes\n",
        )
        .unwrap();

        mgr.load_configs_from(&[dir.path().to_path_buf()]);
        assert_eq!(mgr.configs.len(), 1);
        assert_eq!(mgr.configs[0].match_section.names, vec!["eth*"]);
    }
}
