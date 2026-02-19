#![allow(dead_code)]

//! systemd-networkd — network link manager daemon.
//!
//! Manages network interfaces based on `.network` configuration files.
//! Supports:
//! - Static IPv4 address and route configuration
//! - DHCPv4 client with full DORA state machine
//! - DNS resolver configuration (`/run/systemd/resolve/resolv.conf`)
//! - sd_notify protocol (READY/WATCHDOG/STATUS/STOPPING)
//! - Signal handling (SIGTERM/SIGINT for shutdown, SIGHUP for reload)
//! - Runtime state files in `/run/systemd/netif/`
//!
//! Usage:
//!   systemd-networkd              # Run as daemon
//!   systemd-networkd --help       # Show help

mod config;
mod dhcp;
mod link;
mod manager;

use std::io;
use std::net::Ipv4Addr;
use std::os::unix::net::UnixDatagram;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use manager::NetworkManager;

/// Send an sd_notify message to the service manager.
fn sd_notify(msg: &str) {
    if let Ok(path) = std::env::var("NOTIFY_SOCKET") {
        let path = if let Some(stripped) = path.strip_prefix('@') {
            // Abstract socket — replace leading '@' with '\0'.
            format!("\0{}", stripped)
        } else {
            path
        };
        if let Ok(sock) = UnixDatagram::unbound() {
            let _ = sock.send_to(msg.as_bytes(), &path);
        }
    }
}

/// Parse WATCHDOG_USEC from the environment and return the keepalive interval
/// (half the watchdog period).
fn watchdog_interval() -> Option<Duration> {
    let usec: u64 = std::env::var("WATCHDOG_USEC").ok()?.parse().ok()?;
    if usec == 0 {
        return None;
    }
    // Kick at half the watchdog interval.
    Some(Duration::from_micros(usec / 2))
}

/// Open a raw socket bound to a specific interface for sending/receiving
/// DHCP packets. Uses AF_INET + SOCK_DGRAM + SO_BINDTODEVICE.
///
/// Returns the socket fd on success.
fn open_dhcp_socket(ifname: &str) -> io::Result<i32> {
    let fd = unsafe {
        libc::socket(
            libc::AF_INET,
            libc::SOCK_DGRAM | libc::SOCK_CLOEXEC | libc::SOCK_NONBLOCK,
            libc::IPPROTO_UDP,
        )
    };
    if fd < 0 {
        return Err(io::Error::last_os_error());
    }

    // SO_BROADCAST — needed for DHCP broadcast.
    let one: libc::c_int = 1;
    unsafe {
        libc::setsockopt(
            fd,
            libc::SOL_SOCKET,
            libc::SO_BROADCAST,
            &one as *const libc::c_int as *const libc::c_void,
            std::mem::size_of::<libc::c_int>() as libc::socklen_t,
        );
    }

    // SO_REUSEADDR.
    unsafe {
        libc::setsockopt(
            fd,
            libc::SOL_SOCKET,
            libc::SO_REUSEADDR,
            &one as *const libc::c_int as *const libc::c_void,
            std::mem::size_of::<libc::c_int>() as libc::socklen_t,
        );
    }

    // SO_BINDTODEVICE — bind to the specific interface.
    let ifname_c = std::ffi::CString::new(ifname)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "invalid interface name"))?;
    let ret = unsafe {
        libc::setsockopt(
            fd,
            libc::SOL_SOCKET,
            libc::SO_BINDTODEVICE,
            ifname_c.as_ptr() as *const libc::c_void,
            ifname_c.as_bytes_with_nul().len() as libc::socklen_t,
        )
    };
    if ret < 0 {
        let err = io::Error::last_os_error();
        unsafe { libc::close(fd) };
        return Err(err);
    }

    // Bind to 0.0.0.0:68 (DHCP client port).
    let mut addr: libc::sockaddr_in = unsafe { std::mem::zeroed() };
    addr.sin_family = libc::AF_INET as libc::sa_family_t;
    addr.sin_port = 68u16.to_be();
    addr.sin_addr.s_addr = libc::INADDR_ANY;

    let ret = unsafe {
        libc::bind(
            fd,
            &addr as *const libc::sockaddr_in as *const libc::sockaddr,
            std::mem::size_of::<libc::sockaddr_in>() as libc::socklen_t,
        )
    };
    if ret < 0 {
        let err = io::Error::last_os_error();
        unsafe { libc::close(fd) };
        return Err(err);
    }

    Ok(fd)
}

/// Send a DHCP packet via broadcast on the given socket.
fn send_dhcp_broadcast(fd: i32, pkt: &dhcp::DhcpPacket) -> io::Result<()> {
    let data = pkt.serialize();

    let mut dst: libc::sockaddr_in = unsafe { std::mem::zeroed() };
    dst.sin_family = libc::AF_INET as libc::sa_family_t;
    dst.sin_port = 67u16.to_be(); // DHCP server port
    dst.sin_addr.s_addr = u32::from(Ipv4Addr::BROADCAST).to_be();

    let sent = unsafe {
        libc::sendto(
            fd,
            data.as_ptr() as *const libc::c_void,
            data.len(),
            0,
            &dst as *const libc::sockaddr_in as *const libc::sockaddr,
            std::mem::size_of::<libc::sockaddr_in>() as libc::socklen_t,
        )
    };
    if sent < 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

/// Receive a DHCP packet from the given socket (non-blocking).
fn recv_dhcp_packet(fd: i32) -> Option<dhcp::DhcpPacket> {
    let mut buf = [0u8; 4096];
    let n = unsafe {
        libc::recv(
            fd,
            buf.as_mut_ptr() as *mut libc::c_void,
            buf.len(),
            libc::MSG_DONTWAIT,
        )
    };
    if n <= 0 {
        return None;
    }
    let n = n as usize;
    dhcp::DhcpPacket::parse(&buf[..n]).ok()
}

fn print_help() {
    eprintln!("systemd-networkd — network link manager daemon");
    eprintln!();
    eprintln!("Usage: systemd-networkd [OPTIONS]");
    eprintln!();
    eprintln!("Options:");
    eprintln!("  --help, -h    Show this help message");
    eprintln!("  --version     Show version information");
}

fn print_version() {
    eprintln!("systemd-networkd (systemd-rs) 0.1.0");
}

fn setup_logging() {
    let level = std::env::var("SYSTEMD_LOG_LEVEL")
        .ok()
        .and_then(|l| match l.to_lowercase().as_str() {
            "debug" | "7" => Some(log::LevelFilter::Debug),
            "info" | "6" => Some(log::LevelFilter::Info),
            "notice" | "5" | "warning" | "4" => Some(log::LevelFilter::Warn),
            "err" | "3" | "crit" | "2" | "alert" | "1" | "emerg" | "0" => {
                Some(log::LevelFilter::Error)
            }
            _ => None,
        })
        .unwrap_or(log::LevelFilter::Info);

    fern::Dispatch::new()
        .format(|out, message, record| {
            out.finish(format_args!(
                "[{}][systemd-networkd][{}] {}",
                chrono::Local::now().format("%H:%M:%S"),
                record.level(),
                message
            ))
        })
        .level(level)
        .chain(std::io::stderr())
        .apply()
        .ok();
}

fn main() {
    // Parse arguments.
    let args: Vec<String> = std::env::args().collect();
    for arg in &args[1..] {
        match arg.as_str() {
            "--help" | "-h" => {
                print_help();
                return;
            }
            "--version" => {
                print_version();
                return;
            }
            _ => {
                // Ignore unknown flags (systemd passes various flags).
            }
        }
    }

    setup_logging();
    log::info!("systemd-networkd starting");

    // Set up signal handling.
    let shutdown = Arc::new(AtomicBool::new(false));
    let reload = Arc::new(AtomicBool::new(false));

    // Register signal handlers.
    signal_hook::flag::register(signal_hook::consts::SIGTERM, Arc::clone(&shutdown)).ok();
    signal_hook::flag::register(signal_hook::consts::SIGINT, Arc::clone(&shutdown)).ok();
    signal_hook::flag::register(signal_hook::consts::SIGHUP, Arc::clone(&reload)).ok();

    // Create the network manager.
    let mut mgr = NetworkManager::new();

    // Load .network configuration files.
    mgr.load_configs();

    if mgr.configs.is_empty() {
        log::info!("No .network configuration files found");
    }

    // Discover and match network interfaces.
    if let Err(e) = mgr.discover_links() {
        log::error!("Failed to discover links: {}", e);
        sd_notify("STATUS=Failed to discover links");
        std::process::exit(1);
    }

    // Configure links (bring up, apply static config, start DHCP).
    if let Err(e) = mgr.configure_links() {
        log::error!("Failed to configure links: {}", e);
    }

    // Open DHCP sockets for links that need DHCP.
    let mut dhcp_sockets: std::collections::HashMap<u32, i32> = std::collections::HashMap::new();
    for ifindex in mgr.dhcp_active_links() {
        if let Some(managed) = mgr.links.get(&ifindex) {
            match open_dhcp_socket(&managed.link.name) {
                Ok(fd) => {
                    log::debug!(
                        "Opened DHCP socket fd={} for {} (idx={})",
                        fd,
                        managed.link.name,
                        ifindex
                    );
                    dhcp_sockets.insert(ifindex, fd);
                }
                Err(e) => {
                    log::warn!(
                        "Failed to open DHCP socket for {}: {}",
                        managed.link.name,
                        e
                    );
                }
            }
        }
    }

    // Send initial DHCP discovers.
    for (&ifindex, &fd) in &dhcp_sockets {
        if let Some(managed) = mgr.links.get_mut(&ifindex)
            && let Some(ref mut client) = managed.dhcp_client
            && let Some(pkt) = client.next_packet()
        {
            log::info!(
                "{}: sending DHCPDISCOVER (xid={:#010x})",
                client.config.ifname,
                client.xid
            );
            if let Err(e) = send_dhcp_broadcast(fd, &pkt) {
                log::warn!("{}: failed to send DISCOVER: {}", client.config.ifname, e);
            }
        }
    }

    // Write initial state files.
    mgr.write_state_files();

    // Notify systemd we're ready.
    sd_notify("READY=1\nSTATUS=Configured network interfaces");
    log::info!(
        "systemd-networkd ready (managing {} link(s))",
        mgr.links.len()
    );

    // Get watchdog interval.
    let watchdog = watchdog_interval();
    let mut last_watchdog = Instant::now();

    // Main event loop.
    let poll_interval = Duration::from_millis(500);

    while !shutdown.load(Ordering::Relaxed) {
        // Handle reload signal.
        if reload.swap(false, Ordering::Relaxed) {
            log::info!("Reloading configuration (SIGHUP)");
            mgr.load_configs();
            if let Err(e) = mgr.discover_links() {
                log::warn!("Failed to rediscover links on reload: {}", e);
            }
            if let Err(e) = mgr.configure_links() {
                log::warn!("Failed to reconfigure links on reload: {}", e);
            }
            sd_notify(&format!("STATUS={}", mgr.overall_state()));
        }

        // Process DHCP: receive replies, handle timeouts, send retransmits.
        for (&ifindex, &fd) in &dhcp_sockets {
            // Try to receive a DHCP reply.
            while let Some(reply) = recv_dhcp_packet(fd) {
                if let Some(managed) = mgr.links.get_mut(&ifindex)
                    && let Some(ref mut client) = managed.dhcp_client
                {
                    if let Some(lease) = client.process_reply(&reply) {
                        // Lease obtained or renewed — apply it.
                        let _ = client; // Release borrow.
                        if let Err(e) = mgr.apply_lease(ifindex, &lease) {
                            log::warn!("Failed to apply DHCP lease on idx={}: {}", ifindex, e);
                        }
                        mgr.write_state_files();
                        sd_notify(&format!("STATUS={}", mgr.overall_state()));
                        break;
                    }

                    // If the client moved to REQUESTING after an OFFER,
                    // immediately send the REQUEST.
                    if client.state == dhcp::DhcpState::Requesting
                        && let Some(request_pkt) = client.next_packet()
                    {
                        log::info!(
                            "{}: sending DHCPREQUEST for {}",
                            client.config.ifname,
                            client
                                .last_offer
                                .as_ref()
                                .map(|o| o.yiaddr.to_string())
                                .unwrap_or_default()
                        );
                        if let Err(e) = send_dhcp_broadcast(fd, &request_pkt) {
                            log::warn!("{}: failed to send REQUEST: {}", client.config.ifname, e);
                        }
                    }
                }
            }

            // Check for retransmission timeouts.
            if let Some(managed) = mgr.links.get_mut(&ifindex)
                && let Some(ref mut client) = managed.dhcp_client
            {
                let should_retransmit = match client.last_send {
                    Some(last) => last.elapsed() >= client.retransmit_timeout(),
                    None => true,
                };

                if should_retransmit
                    && !client.max_attempts_reached()
                    && client.state != dhcp::DhcpState::Bound
                    && let Some(pkt) = client.next_packet()
                {
                    let msg_type = pkt
                        .message_type()
                        .map(dhcp::dhcp_message_type_name)
                        .unwrap_or("UNKNOWN");
                    log::debug!(
                        "{}: retransmitting DHCP{} (attempt {})",
                        client.config.ifname,
                        msg_type,
                        client.attempts
                    );
                    if let Err(e) = send_dhcp_broadcast(fd, &pkt) {
                        log::warn!("{}: failed to retransmit: {}", client.config.ifname, e);
                    }
                }

                // Check for lease renewal / rebinding.
                if client.state == dhcp::DhcpState::Bound
                    && let Some(ref lease) = client.lease
                {
                    if lease.is_expired() {
                        log::warn!("{}: DHCP lease expired", client.config.ifname);
                        let ifname = client.config.ifname.clone();
                        client.state = dhcp::DhcpState::Init;
                        client.lease = None;
                        let _ = client;
                        if let Err(e) = mgr.remove_lease(ifindex) {
                            log::warn!("{}: failed to remove expired lease: {}", ifname, e);
                        }
                        mgr.write_state_files();
                    } else if lease.needs_renewal() {
                        // Transition to renewing and send a request.
                        if let Some(pkt) = client.next_packet() {
                            log::info!("{}: DHCP lease renewal (T1 reached)", client.config.ifname);
                            let _ = send_dhcp_broadcast(fd, &pkt);
                        }
                    }
                }
            }
        }

        // Watchdog keepalive.
        if let Some(interval) = watchdog
            && last_watchdog.elapsed() >= interval
        {
            sd_notify("WATCHDOG=1");
            last_watchdog = Instant::now();
        }

        // Sleep until next poll.
        std::thread::sleep(poll_interval);
    }

    // Shutdown.
    log::info!("systemd-networkd shutting down");
    sd_notify("STOPPING=1\nSTATUS=Shutting down");

    // Release all DHCP leases.
    for (&ifindex, &fd) in &dhcp_sockets {
        if let Some(managed) = mgr.links.get(&ifindex)
            && let Some(ref client) = managed.dhcp_client
            && let Some(release_pkt) = client.build_release()
        {
            log::info!("{}: sending DHCPRELEASE", managed.link.name);
            let _ = send_dhcp_broadcast(fd, &release_pkt);
        }
    }

    // Close DHCP sockets.
    for (_, fd) in dhcp_sockets {
        unsafe { libc::close(fd) };
    }

    // Write final state.
    mgr.write_state_files();

    log::info!("systemd-networkd stopped");
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sd_notify_no_socket() {
        // Should not panic when NOTIFY_SOCKET is not set.
        unsafe { std::env::remove_var("NOTIFY_SOCKET") };
        sd_notify("READY=1");
    }

    #[test]
    fn test_watchdog_interval_not_set() {
        unsafe { std::env::remove_var("WATCHDOG_USEC") };
        assert!(watchdog_interval().is_none());
    }

    #[test]
    fn test_watchdog_interval_zero() {
        unsafe { std::env::set_var("WATCHDOG_USEC", "0") };
        assert!(watchdog_interval().is_none());
        unsafe { std::env::remove_var("WATCHDOG_USEC") };
    }

    #[test]
    fn test_watchdog_interval_valid() {
        unsafe { std::env::set_var("WATCHDOG_USEC", "10000000") }; // 10s
        let interval = watchdog_interval();
        assert!(interval.is_some());
        // Half of 10s = 5s.
        assert_eq!(interval.unwrap(), Duration::from_secs(5));
        unsafe { std::env::remove_var("WATCHDOG_USEC") };
    }

    #[test]
    fn test_watchdog_interval_invalid() {
        unsafe { std::env::set_var("WATCHDOG_USEC", "not_a_number") };
        assert!(watchdog_interval().is_none());
        unsafe { std::env::remove_var("WATCHDOG_USEC") };
    }
}
