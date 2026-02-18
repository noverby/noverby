#![allow(dead_code)]
//! systemd-resolved — Stub DNS resolver daemon
//!
//! A Rust implementation of systemd-resolved that provides a local DNS stub
//! listener on 127.0.0.53:53, forwarding queries to upstream DNS servers.
//!
//! Features:
//! - Parses `/etc/systemd/resolved.conf` and drop-in directories
//! - Stub DNS listener on 127.0.0.53:53 (UDP + TCP)
//! - Forwards DNS queries to configured upstream servers
//! - Manages `/run/systemd/resolve/stub-resolv.conf` (stub → 127.0.0.53)
//! - Manages `/run/systemd/resolve/resolv.conf` (upstream servers)
//! - Per-link DNS from networkd state files
//! - sd_notify READY=1 / WATCHDOG=1 / STATUS= protocol
//! - Signal handling: SIGTERM/SIGINT for shutdown, SIGHUP for reload
//! - TCP listener for DNS queries (parallel to UDP)
//! - Statistics tracking

mod config;
mod dns;

use std::fs;
use std::io::{self, Read, Write};
use std::net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream, UdpSocket};
use std::os::unix::net::UnixDatagram;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use config::{
    RESOLV_CONF_PATH, ResolvedConfig, STATE_DIR, STUB_RESOLV_CONF_PATH, StubListenerMode,
};
use dns::{
    DnsMessage, HEADER_SIZE, MAX_EDNS_UDP_SIZE, MAX_TCP_SIZE, ResolverStats, build_formerr,
    build_servfail, forward_query,
};

// ── Constants ──────────────────────────────────────────────────────────────

/// Poll interval for the main loop (ms)
const POLL_INTERVAL_MS: u64 = 500;

/// UDP receive buffer size
const UDP_RECV_BUF: usize = MAX_EDNS_UDP_SIZE;

/// TCP receive buffer size
const TCP_RECV_BUF: usize = MAX_TCP_SIZE + 2; // +2 for length prefix

/// Default watchdog interval (if WATCHDOG_USEC not set)
const DEFAULT_WATCHDOG_SEC: u64 = 0; // disabled

/// Interval to refresh link DNS configuration from networkd (seconds)
const LINK_DNS_REFRESH_INTERVAL: Duration = Duration::from_secs(30);

/// TCP connection timeout
const TCP_TIMEOUT: Duration = Duration::from_secs(10);

// ── Logging ────────────────────────────────────────────────────────────────

fn setup_logging() {
    let log_level = std::env::var("RESOLVED_LOG")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(log::LevelFilter::Info);

    if fern::Dispatch::new()
        .format(|out, message, record| {
            out.finish(format_args!(
                "[{}][{}][{}] {}",
                chrono::Local::now().format("%Y-%m-%d][%H:%M:%S"),
                record.target(),
                record.level(),
                message,
            ))
        })
        .level(log_level)
        .chain(io::stderr())
        .apply()
        .is_err()
    {
        eprintln!("resolved: failed to set up logging, continuing with eprintln");
    }
}

// ── sd_notify helpers ──────────────────────────────────────────────────────

fn sd_notify(msg: &str) {
    if let Ok(path) = std::env::var("NOTIFY_SOCKET") {
        let path = if path.starts_with('@') {
            format!("\0{}", &path[1..])
        } else {
            path
        };

        if let Ok(sock) = UnixDatagram::unbound() {
            let _ = sock.send_to(msg.as_bytes(), &path);
        }
    }
}

fn sd_notify_ready() {
    sd_notify("READY=1");
}

fn sd_notify_stopping() {
    sd_notify("STOPPING=1");
}

fn sd_notify_status(status: &str) {
    sd_notify(&format!("STATUS={}", status));
}

fn sd_notify_watchdog() {
    sd_notify("WATCHDOG=1");
}

fn get_watchdog_usec() -> u64 {
    std::env::var("WATCHDOG_USEC")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(DEFAULT_WATCHDOG_SEC * 1_000_000)
}

// ── Shared atomic statistics ───────────────────────────────────────────────

/// Thread-safe statistics using atomics
#[derive(Debug)]
struct AtomicStats {
    queries_received: AtomicU64,
    queries_forwarded: AtomicU64,
    responses_ok: AtomicU64,
    responses_servfail: AtomicU64,
    responses_nxdomain: AtomicU64,
    responses_formerr: AtomicU64,
    upstream_timeouts: AtomicU64,
    upstream_failures: AtomicU64,
    tcp_queries: AtomicU64,
}

impl AtomicStats {
    fn new() -> Self {
        Self {
            queries_received: AtomicU64::new(0),
            queries_forwarded: AtomicU64::new(0),
            responses_ok: AtomicU64::new(0),
            responses_servfail: AtomicU64::new(0),
            responses_nxdomain: AtomicU64::new(0),
            responses_formerr: AtomicU64::new(0),
            upstream_timeouts: AtomicU64::new(0),
            upstream_failures: AtomicU64::new(0),
            tcp_queries: AtomicU64::new(0),
        }
    }

    fn snapshot(&self) -> ResolverStats {
        ResolverStats {
            queries_received: self.queries_received.load(Ordering::Relaxed),
            queries_forwarded: self.queries_forwarded.load(Ordering::Relaxed),
            responses_ok: self.responses_ok.load(Ordering::Relaxed),
            responses_servfail: self.responses_servfail.load(Ordering::Relaxed),
            responses_nxdomain: self.responses_nxdomain.load(Ordering::Relaxed),
            responses_formerr: self.responses_formerr.load(Ordering::Relaxed),
            upstream_timeouts: self.upstream_timeouts.load(Ordering::Relaxed),
            upstream_failures: self.upstream_failures.load(Ordering::Relaxed),
            tcp_queries: self.tcp_queries.load(Ordering::Relaxed),
        }
    }
}

// ── resolv.conf management ─────────────────────────────────────────────────

fn write_resolv_conf_files(config: &ResolvedConfig) {
    // Ensure state directory exists
    if let Err(e) = fs::create_dir_all(STATE_DIR) {
        log::warn!("Failed to create {}: {}", STATE_DIR, e);
        return;
    }

    // Write stub-resolv.conf (points to 127.0.0.53)
    let stub_content = config.stub_resolv_conf_content();
    write_file_atomic(STUB_RESOLV_CONF_PATH, &stub_content);

    // Write resolv.conf (lists upstream servers)
    let upstream_content = config.upstream_resolv_conf_content();
    write_file_atomic(RESOLV_CONF_PATH, &upstream_content);
}

fn write_file_atomic(path: &str, content: &str) {
    let tmp_path = format!("{}.tmp.{}", path, std::process::id());

    match fs::write(&tmp_path, content) {
        Ok(()) => {
            if let Err(e) = fs::rename(&tmp_path, path) {
                log::warn!("Failed to rename {} to {}: {}", tmp_path, path, e);
                let _ = fs::remove_file(&tmp_path);
            } else {
                log::debug!("Wrote {}", path);
            }
        }
        Err(e) => {
            log::warn!("Failed to write {}: {}", tmp_path, e);
        }
    }
}

// ── Handle a single DNS query ──────────────────────────────────────────────

fn handle_query(query_data: &[u8], upstreams: &[SocketAddr], stats: &AtomicStats) -> Vec<u8> {
    stats.queries_received.fetch_add(1, Ordering::Relaxed);

    // Parse the query to log it
    let query_info = match DnsMessage::parse(query_data) {
        Ok(msg) => {
            if !msg.is_query() {
                // Not a query — return FORMERR
                stats.responses_formerr.fetch_add(1, Ordering::Relaxed);
                return build_formerr(query_data).unwrap_or_default();
            }
            msg.query_summary()
        }
        Err(_) => {
            stats.responses_formerr.fetch_add(1, Ordering::Relaxed);
            return build_formerr(query_data).unwrap_or_default();
        }
    };

    log::debug!("Query: {}", query_info);

    if upstreams.is_empty() {
        log::warn!("No upstream DNS servers configured");
        stats.responses_servfail.fetch_add(1, Ordering::Relaxed);
        return build_servfail(query_data).unwrap_or_default();
    }

    // Forward to upstream
    stats.queries_forwarded.fetch_add(1, Ordering::Relaxed);

    match forward_query(query_data, upstreams) {
        Ok(response) => {
            // Check response code for statistics
            if response.len() >= HEADER_SIZE {
                let rcode = response[3] & 0x0F;
                match rcode {
                    0 => {
                        stats.responses_ok.fetch_add(1, Ordering::Relaxed);
                    }
                    3 => {
                        stats.responses_nxdomain.fetch_add(1, Ordering::Relaxed);
                    }
                    _ => {}
                }
            }
            response
        }
        Err(dns::DnsError::Timeout) => {
            log::debug!("Upstream timeout for {}", query_info);
            stats.upstream_timeouts.fetch_add(1, Ordering::Relaxed);
            stats.responses_servfail.fetch_add(1, Ordering::Relaxed);
            build_servfail(query_data).unwrap_or_default()
        }
        Err(e) => {
            log::debug!("Upstream failure for {}: {}", query_info, e);
            stats.upstream_failures.fetch_add(1, Ordering::Relaxed);
            stats.responses_servfail.fetch_add(1, Ordering::Relaxed);
            build_servfail(query_data).unwrap_or_default()
        }
    }
}

// ── Handle a TCP connection ────────────────────────────────────────────────

fn handle_tcp_connection(
    mut stream: TcpStream,
    upstreams: Vec<SocketAddr>,
    stats: Arc<AtomicStats>,
) {
    let _ = stream.set_read_timeout(Some(TCP_TIMEOUT));
    let _ = stream.set_write_timeout(Some(TCP_TIMEOUT));

    stats.tcp_queries.fetch_add(1, Ordering::Relaxed);

    // TCP DNS: read 2-byte length prefix, then message
    let mut len_buf = [0u8; 2];
    if stream.read_exact(&mut len_buf).is_err() {
        return;
    }
    let msg_len = u16::from_be_bytes(len_buf) as usize;

    if msg_len < HEADER_SIZE || msg_len > MAX_TCP_SIZE {
        return;
    }

    let mut query = vec![0u8; msg_len];
    if stream.read_exact(&mut query).is_err() {
        return;
    }

    let response = handle_query(&query, &upstreams, &stats);

    // Write response with length prefix
    let resp_len = (response.len() as u16).to_be_bytes();
    let _ = stream.write_all(&resp_len);
    let _ = stream.write_all(&response);
    let _ = stream.flush();
}

// ── UDP listener thread ────────────────────────────────────────────────────

fn run_udp_listener(
    socket: UdpSocket,
    upstreams: Arc<std::sync::RwLock<Vec<SocketAddr>>>,
    stats: Arc<AtomicStats>,
    shutdown: Arc<AtomicBool>,
) {
    let mut buf = vec![0u8; UDP_RECV_BUF];

    log::info!(
        "UDP stub listener ready on {}",
        socket
            .local_addr()
            .unwrap_or_else(
                |_| "unknown".parse::<SocketAddr>().unwrap_or(SocketAddr::new(
                    std::net::IpAddr::V4(Ipv4Addr::LOCALHOST),
                    53
                ))
            )
    );

    loop {
        if shutdown.load(Ordering::Relaxed) {
            break;
        }

        match socket.recv_from(&mut buf) {
            Ok((len, src)) => {
                let query = &buf[..len];
                let upstream_list = upstreams.read().unwrap_or_else(|e| e.into_inner()).clone();
                let response = handle_query(query, &upstream_list, &stats);

                if let Err(e) = socket.send_to(&response, src) {
                    log::debug!("Failed to send UDP response to {}: {}", src, e);
                }
            }
            Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                // Timeout on the non-blocking recv — just loop
                continue;
            }
            Err(ref e) if e.kind() == io::ErrorKind::Interrupted => {
                continue;
            }
            Err(e) => {
                if !shutdown.load(Ordering::Relaxed) {
                    log::warn!("UDP recv error: {}", e);
                }
                // Brief sleep to avoid spinning on persistent errors
                thread::sleep(Duration::from_millis(100));
            }
        }
    }

    log::debug!("UDP listener shut down");
}

// ── TCP listener thread ────────────────────────────────────────────────────

fn run_tcp_listener(
    listener: TcpListener,
    upstreams: Arc<std::sync::RwLock<Vec<SocketAddr>>>,
    stats: Arc<AtomicStats>,
    shutdown: Arc<AtomicBool>,
) {
    log::info!(
        "TCP stub listener ready on {}",
        listener
            .local_addr()
            .map(|a| a.to_string())
            .unwrap_or_else(|_| "unknown".to_string())
    );

    loop {
        if shutdown.load(Ordering::Relaxed) {
            break;
        }

        match listener.accept() {
            Ok((stream, _peer)) => {
                let upstream_list = upstreams.read().unwrap_or_else(|e| e.into_inner()).clone();
                let stats_clone = Arc::clone(&stats);

                thread::spawn(move || {
                    handle_tcp_connection(stream, upstream_list, stats_clone);
                });
            }
            Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                continue;
            }
            Err(ref e) if e.kind() == io::ErrorKind::Interrupted => {
                continue;
            }
            Err(e) => {
                if !shutdown.load(Ordering::Relaxed) {
                    log::warn!("TCP accept error: {}", e);
                }
                thread::sleep(Duration::from_millis(100));
            }
        }
    }

    log::debug!("TCP listener shut down");
}

// ── Signal handling ────────────────────────────────────────────────────────

fn setup_signal_handlers(shutdown: &Arc<AtomicBool>, reload: &Arc<AtomicBool>) {
    let shutdown_flag = Arc::clone(shutdown);
    let reload_flag = Arc::clone(reload);

    // SIGTERM / SIGINT → shutdown
    let shutdown_clone = Arc::clone(&shutdown_flag);
    if let Err(e) = signal_hook::flag::register(signal_hook::consts::SIGTERM, shutdown_clone) {
        log::warn!("Failed to register SIGTERM handler: {}", e);
    }

    let shutdown_clone2 = Arc::clone(&shutdown_flag);
    if let Err(e) = signal_hook::flag::register(signal_hook::consts::SIGINT, shutdown_clone2) {
        log::warn!("Failed to register SIGINT handler: {}", e);
    }

    // SIGHUP → reload
    if let Err(e) = signal_hook::flag::register(signal_hook::consts::SIGHUP, reload_flag) {
        log::warn!("Failed to register SIGHUP handler: {}", e);
    }
}

// ── Bind stub listeners ────────────────────────────────────────────────────

fn bind_udp_stub(addr: SocketAddr) -> io::Result<UdpSocket> {
    let socket = UdpSocket::bind(addr)?;
    // Set a read timeout so the listener loop can check the shutdown flag
    socket.set_read_timeout(Some(Duration::from_millis(POLL_INTERVAL_MS)))?;
    Ok(socket)
}

fn bind_tcp_stub(addr: SocketAddr) -> io::Result<TcpListener> {
    let listener = TcpListener::bind(addr)?;
    listener.set_nonblocking(true)?;
    Ok(listener)
}

// ── Main ───────────────────────────────────────────────────────────────────

fn main() {
    setup_logging();
    log::info!("systemd-resolved starting");

    // Load configuration
    let mut config = ResolvedConfig::load();
    log::info!(
        "Loaded configuration: {} DNS servers, {} fallback servers, stub_listener={}",
        config.dns.len(),
        config.fallback_dns.len(),
        config.dns_stub_listener.as_str(),
    );

    // Update per-link DNS from networkd
    config.update_link_dns_from_networkd();
    if !config.link_dns.is_empty() {
        log::info!(
            "Found {} link(s) with DNS configuration from networkd",
            config.link_dns.len()
        );
    }

    // Write resolv.conf files
    write_resolv_conf_files(&config);

    // Build upstream server list
    let upstream_servers: Vec<SocketAddr> = config
        .effective_dns_servers()
        .iter()
        .map(|s| s.socket_addr())
        .collect();

    log::info!(
        "Upstream DNS servers: {}",
        if upstream_servers.is_empty() {
            "(none)".to_string()
        } else {
            upstream_servers
                .iter()
                .map(|s| s.to_string())
                .collect::<Vec<_>>()
                .join(", ")
        }
    );

    // Shared state
    let shutdown = Arc::new(AtomicBool::new(false));
    let reload = Arc::new(AtomicBool::new(false));
    let stats = Arc::new(AtomicStats::new());
    let upstreams = Arc::new(std::sync::RwLock::new(upstream_servers));

    // Set up signal handlers
    setup_signal_handlers(&shutdown, &reload);

    // Determine stub listener address
    let stub_addr: SocketAddr = SocketAddr::new(
        config::STUB_LISTENER_ADDR
            .parse()
            .unwrap_or(std::net::IpAddr::V4(Ipv4Addr::new(127, 0, 0, 53))),
        config::DNS_PORT,
    );

    let mut listener_threads: Vec<thread::JoinHandle<()>> = Vec::new();

    // Start stub listeners if enabled
    match config.dns_stub_listener {
        StubListenerMode::No => {
            log::info!("DNS stub listener disabled by configuration");
        }
        mode => {
            if mode.udp_enabled() {
                match bind_udp_stub(stub_addr) {
                    Ok(socket) => {
                        log::info!("Bound UDP stub listener on {}", stub_addr);
                        let upstreams_clone = Arc::clone(&upstreams);
                        let stats_clone = Arc::clone(&stats);
                        let shutdown_clone = Arc::clone(&shutdown);
                        listener_threads.push(thread::spawn(move || {
                            run_udp_listener(socket, upstreams_clone, stats_clone, shutdown_clone);
                        }));
                    }
                    Err(e) => {
                        log::warn!(
                            "Failed to bind UDP stub listener on {}: {} (continuing without UDP)",
                            stub_addr,
                            e
                        );
                    }
                }
            }

            if mode.tcp_enabled() {
                match bind_tcp_stub(stub_addr) {
                    Ok(listener) => {
                        log::info!("Bound TCP stub listener on {}", stub_addr);
                        let upstreams_clone = Arc::clone(&upstreams);
                        let stats_clone = Arc::clone(&stats);
                        let shutdown_clone = Arc::clone(&shutdown);
                        listener_threads.push(thread::spawn(move || {
                            run_tcp_listener(
                                listener,
                                upstreams_clone,
                                stats_clone,
                                shutdown_clone,
                            );
                        }));
                    }
                    Err(e) => {
                        log::warn!(
                            "Failed to bind TCP stub listener on {}: {} (continuing without TCP)",
                            stub_addr,
                            e
                        );
                    }
                }
            }
        }
    }

    // Start extra stub listeners
    for extra_addr in &config.dns_stub_listener_extra {
        match bind_udp_stub(*extra_addr) {
            Ok(socket) => {
                log::info!("Bound extra UDP stub listener on {}", extra_addr);
                let upstreams_clone = Arc::clone(&upstreams);
                let stats_clone = Arc::clone(&stats);
                let shutdown_clone = Arc::clone(&shutdown);
                listener_threads.push(thread::spawn(move || {
                    run_udp_listener(socket, upstreams_clone, stats_clone, shutdown_clone);
                }));
            }
            Err(e) => {
                log::warn!(
                    "Failed to bind extra stub listener on {}: {}",
                    extra_addr,
                    e
                );
            }
        }
    }

    // Signal readiness
    sd_notify_ready();
    sd_notify_status("Processing requests...");
    log::info!("systemd-resolved is ready");

    // Watchdog setup
    let watchdog_usec = get_watchdog_usec();
    let watchdog_interval = if watchdog_usec > 0 {
        // Send watchdog at half the configured interval
        Some(Duration::from_micros(watchdog_usec / 2))
    } else {
        None
    };
    let mut last_watchdog = Instant::now();
    let mut last_link_refresh = Instant::now();

    // ── Main loop ──────────────────────────────────────────────────────
    loop {
        if shutdown.load(Ordering::Relaxed) {
            log::info!("Received shutdown signal");
            break;
        }

        // Handle reload (SIGHUP)
        if reload.swap(false, Ordering::Relaxed) {
            log::info!("Reloading configuration...");
            let mut new_config = ResolvedConfig::load();
            new_config.update_link_dns_from_networkd();

            let new_upstreams: Vec<SocketAddr> = new_config
                .effective_dns_servers()
                .iter()
                .map(|s| s.socket_addr())
                .collect();

            log::info!("Reloaded: {} upstream servers", new_upstreams.len());

            // Update upstream list
            if let Ok(mut list) = upstreams.write() {
                *list = new_upstreams;
            }

            // Rewrite resolv.conf files
            write_resolv_conf_files(&new_config);

            sd_notify_status("Processing requests...");
        }

        // Periodically refresh link DNS from networkd
        if last_link_refresh.elapsed() >= LINK_DNS_REFRESH_INTERVAL {
            let mut refresh_config = ResolvedConfig::load();
            refresh_config.update_link_dns_from_networkd();

            let new_upstreams: Vec<SocketAddr> = refresh_config
                .effective_dns_servers()
                .iter()
                .map(|s| s.socket_addr())
                .collect();

            if let Ok(mut list) = upstreams.write() {
                if *list != new_upstreams {
                    log::info!("Link DNS updated: {} upstream servers", new_upstreams.len());
                    *list = new_upstreams;
                    write_resolv_conf_files(&refresh_config);
                }
            }

            last_link_refresh = Instant::now();
        }

        // Watchdog
        if let Some(interval) = watchdog_interval {
            if last_watchdog.elapsed() >= interval {
                sd_notify_watchdog();
                last_watchdog = Instant::now();
            }
        }

        thread::sleep(Duration::from_millis(POLL_INTERVAL_MS));
    }

    // ── Shutdown ───────────────────────────────────────────────────────
    sd_notify_stopping();
    sd_notify_status("Shutting down...");
    log::info!("Shutting down...");

    // Signal listener threads to stop
    shutdown.store(true, Ordering::Relaxed);

    // Log final statistics
    let final_stats = stats.snapshot();
    log::info!(
        "Final statistics: {} queries received, {} forwarded, {} OK, {} SERVFAIL, {} NXDOMAIN",
        final_stats.queries_received,
        final_stats.queries_forwarded,
        final_stats.responses_ok,
        final_stats.responses_servfail,
        final_stats.responses_nxdomain,
    );

    // Wait for listener threads (with timeout)
    for handle in listener_threads {
        // The threads check the shutdown flag and will exit
        let _ = handle.join();
    }

    // Clean up resolv.conf files (optional — real systemd doesn't do this,
    // but we clean up gracefully)
    log::info!("systemd-resolved stopped");
}

// ── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::Ipv4Addr;

    #[test]
    fn test_atomic_stats_new() {
        let stats = AtomicStats::new();
        assert_eq!(stats.queries_received.load(Ordering::Relaxed), 0);
        assert_eq!(stats.queries_forwarded.load(Ordering::Relaxed), 0);
        assert_eq!(stats.responses_ok.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn test_atomic_stats_increment() {
        let stats = AtomicStats::new();
        stats.queries_received.fetch_add(1, Ordering::Relaxed);
        stats.queries_received.fetch_add(1, Ordering::Relaxed);
        assert_eq!(stats.queries_received.load(Ordering::Relaxed), 2);
    }

    #[test]
    fn test_atomic_stats_snapshot() {
        let stats = AtomicStats::new();
        stats.queries_received.fetch_add(10, Ordering::Relaxed);
        stats.queries_forwarded.fetch_add(8, Ordering::Relaxed);
        stats.responses_ok.fetch_add(7, Ordering::Relaxed);
        stats.responses_servfail.fetch_add(1, Ordering::Relaxed);

        let snap = stats.snapshot();
        assert_eq!(snap.queries_received, 10);
        assert_eq!(snap.queries_forwarded, 8);
        assert_eq!(snap.responses_ok, 7);
        assert_eq!(snap.responses_servfail, 1);
    }

    #[test]
    fn test_handle_query_empty_upstreams() {
        let stats = AtomicStats::new();

        // Build a minimal valid DNS query
        let mut query = vec![0u8; 12]; // minimal header
        query[2] = 0x01; // RD=1
        query[5] = 0; // QDCOUNT=0

        let response = handle_query(&query, &[], &stats);

        // Should get SERVFAIL
        assert!(response.len() >= HEADER_SIZE);
        let rcode = response[3] & 0x0F;
        assert_eq!(rcode, 2); // SERVFAIL

        assert_eq!(stats.queries_received.load(Ordering::Relaxed), 1);
        assert_eq!(stats.responses_servfail.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn test_handle_query_malformed() {
        let stats = AtomicStats::new();

        // Too short to be a valid DNS message but has a header
        let query = vec![0u8; HEADER_SIZE];
        // QDCOUNT=1 but no question data
        let mut query = query;
        query[5] = 1; // QDCOUNT=1

        let response = handle_query(&query, &[], &stats);
        // Should get FORMERR since the message can't be parsed (truncated question)
        assert!(response.len() >= HEADER_SIZE);
        let rcode = response[3] & 0x0F;
        assert_eq!(rcode, 1); // FORMERR

        assert_eq!(stats.responses_formerr.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn test_handle_query_response_not_query() {
        let stats = AtomicStats::new();

        // Send a response (QR=1) as if it were a query
        let mut data = vec![0u8; HEADER_SIZE];
        data[2] = 0x80; // QR=1 (response)
        data[5] = 0; // QDCOUNT=0

        let response = handle_query(&data, &[], &stats);
        let rcode = response[3] & 0x0F;
        assert_eq!(rcode, 1); // FORMERR
    }

    #[test]
    fn test_write_resolv_conf_files_creates_dir() {
        let dir = tempfile::tempdir().unwrap();
        let _state_path = dir.path().join("resolve");

        // Can't easily test write_resolv_conf_files without mocking paths,
        // but we can test write_file_atomic directly
        let test_path = dir.path().join("test.conf");
        write_file_atomic(test_path.to_str().unwrap(), "nameserver 127.0.0.53\n");
        assert!(test_path.exists());
        assert_eq!(
            fs::read_to_string(&test_path).unwrap(),
            "nameserver 127.0.0.53\n"
        );
    }

    #[test]
    fn test_write_file_atomic_overwrites() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("conf");
        write_file_atomic(path.to_str().unwrap(), "first");
        write_file_atomic(path.to_str().unwrap(), "second");
        assert_eq!(fs::read_to_string(&path).unwrap(), "second");
    }

    #[test]
    fn test_get_watchdog_usec_default() {
        // Without WATCHDOG_USEC env var, should return 0
        // SAFETY: This test is not run in parallel with other tests that
        // depend on WATCHDOG_USEC.
        unsafe {
            std::env::remove_var("WATCHDOG_USEC");
        }
        let usec = get_watchdog_usec();
        assert_eq!(usec, 0);
    }

    #[test]
    fn test_stub_addr_construction() {
        let addr: SocketAddr = SocketAddr::new(
            config::STUB_LISTENER_ADDR
                .parse()
                .unwrap_or(std::net::IpAddr::V4(Ipv4Addr::new(127, 0, 0, 53))),
            config::DNS_PORT,
        );
        assert_eq!(
            addr,
            SocketAddr::new(std::net::IpAddr::V4(Ipv4Addr::new(127, 0, 0, 53)), 53)
        );
    }
}
