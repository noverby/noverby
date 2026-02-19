//! Network link management via the Linux netlink (rtnetlink) interface.
//!
//! This module provides functions to:
//! - Enumerate network interfaces and their addresses
//! - Add/remove IPv4 addresses on interfaces
//! - Add/remove IPv4 routes
//! - Bring interfaces up/down
//! - Set MTU
//! - Read interface MAC addresses
//!
//! All operations use raw netlink sockets (AF_NETLINK / NETLINK_ROUTE) so
//! there is no dependency on external crates beyond `libc` and `nix`.

use std::collections::HashMap;
use std::fmt;
use std::io;
use std::net::Ipv4Addr;

// ---------------------------------------------------------------------------
// Netlink constants (from linux/netlink.h, linux/rtnetlink.h, linux/if.h)
// ---------------------------------------------------------------------------

const NETLINK_ROUTE: i32 = 0;

// Netlink message types (RTM_*)
const RTM_NEWLINK: u16 = 16;
const RTM_DELLINK: u16 = 17;
const RTM_GETLINK: u16 = 18;
const RTM_NEWADDR: u16 = 20;
const RTM_DELADDR: u16 = 21;
const RTM_GETADDR: u16 = 22;
const RTM_NEWROUTE: u16 = 24;
const RTM_DELROUTE: u16 = 25;
const RTM_GETROUTE: u16 = 26;

// Netlink flags
const NLM_F_REQUEST: u16 = 0x0001;
const NLM_F_MULTI: u16 = 0x0002;
const NLM_F_ACK: u16 = 0x0004;
const NLM_F_ROOT: u16 = 0x0100;
const NLM_F_MATCH: u16 = 0x0200;
const NLM_F_DUMP: u16 = NLM_F_ROOT | NLM_F_MATCH;
const NLM_F_CREATE: u16 = 0x0400;
const NLM_F_EXCL: u16 = 0x0200;
const NLM_F_REPLACE: u16 = 0x0100;

// Netlink message type for errors/acks
const NLMSG_ERROR: u16 = 2;
const NLMSG_DONE: u16 = 3;
const NLMSG_NOOP: u16 = 1;

// Address families
const AF_INET: u8 = 2;
const AF_UNSPEC: u8 = 0;

// Interface link attributes (IFLA_*)
const IFLA_IFNAME: u16 = 3;
const IFLA_MTU: u16 = 4;
const IFLA_ADDRESS: u16 = 1; // hardware address
const IFLA_OPERSTATE: u16 = 16;

// Address attributes (IFA_*)
const IFA_ADDRESS: u16 = 1;
const IFA_LOCAL: u16 = 2;
const IFA_LABEL: u16 = 3;
const IFA_BROADCAST: u16 = 4;

// Route attributes (RTA_*)
const RTA_DST: u16 = 1;
const RTA_GATEWAY: u16 = 5;
const RTA_OIF: u16 = 4;
const RTA_PRIORITY: u16 = 6;
const RTA_TABLE: u16 = 15;
const RTA_PREFSRC: u16 = 7;

// Route table
const RT_TABLE_MAIN: u8 = 254;

// Route protocol
const RTPROT_DHCP: u8 = 16;
const RTPROT_STATIC: u8 = 4;
const RTPROT_BOOT: u8 = 3;

// Route types
const RTN_UNICAST: u8 = 1;

// Route scopes
const RT_SCOPE_UNIVERSE: u8 = 0;
const RT_SCOPE_LINK: u8 = 253;

// IFF_* flags
const IFF_UP: u32 = 0x1;
const IFF_RUNNING: u32 = 0x40;
const IFF_LOOPBACK: u32 = 0x8;

// Operstate values
const IF_OPER_UP: u8 = 6;
const IF_OPER_DOWN: u8 = 2;
const IF_OPER_UNKNOWN: u8 = 0;

// Netlink header size
const NLMSG_HDR_LEN: usize = 16;
const NLMSG_ALIGN: usize = 4;

// ifinfomsg size (family + pad + type + index + flags + change)
const IFINFOMSG_LEN: usize = 16;

// ifaddrmsg size
const IFADDRMSG_LEN: usize = 8;

// rtmsg size
const RTMSG_LEN: usize = 12;

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// Information about a network interface (link).
#[derive(Debug, Clone)]
pub struct LinkInfo {
    /// Kernel interface index.
    pub index: u32,
    /// Interface name (e.g. `eth0`, `ens3`).
    pub name: String,
    /// Hardware (MAC) address as colon-separated hex string.
    pub mac: String,
    /// Raw MAC bytes.
    pub mac_bytes: Vec<u8>,
    /// MTU in bytes.
    pub mtu: u32,
    /// IFF_* flags.
    pub flags: u32,
    /// Operational state.
    pub operstate: u8,
}

impl LinkInfo {
    pub fn is_up(&self) -> bool {
        (self.flags & IFF_UP) != 0
    }

    pub fn is_running(&self) -> bool {
        (self.flags & IFF_RUNNING) != 0
    }

    pub fn is_loopback(&self) -> bool {
        (self.flags & IFF_LOOPBACK) != 0
    }

    /// Human-readable operational state.
    pub fn oper_state_str(&self) -> &'static str {
        match self.operstate {
            IF_OPER_UP => "up",
            IF_OPER_DOWN => "down",
            IF_OPER_UNKNOWN => "unknown",
            1 => "not-present",
            3 => "testing",
            4 => "dormant",
            5 => "lower-layer-down",
            _ => "unknown",
        }
    }

    /// Formatted MAC address for display.
    pub fn mac_display(&self) -> &str {
        &self.mac
    }
}

impl fmt::Display for LinkInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}: {} mac={} mtu={} state={}",
            self.index,
            self.name,
            self.mac,
            self.mtu,
            self.oper_state_str()
        )
    }
}

/// Information about an address assigned to an interface.
#[derive(Debug, Clone)]
pub struct AddrInfo {
    /// Interface index this address belongs to.
    pub ifindex: u32,
    /// Address family (AF_INET = 2, AF_INET6 = 10).
    pub family: u8,
    /// Prefix length.
    pub prefix_len: u8,
    /// The address (for IPv4, stored as Ipv4Addr).
    pub address: Ipv4Addr,
    /// Local address (may differ for point-to-point).
    pub local: Option<Ipv4Addr>,
    /// Broadcast address.
    pub broadcast: Option<Ipv4Addr>,
    /// Label (e.g. `eth0:1`).
    pub label: Option<String>,
    /// Scope.
    pub scope: u8,
}

impl fmt::Display for AddrInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}/{}", self.address, self.prefix_len)?;
        if let Some(ref brd) = self.broadcast {
            write!(f, " brd {brd}")?;
        }
        if let Some(ref label) = self.label {
            write!(f, " label {label}")?;
        }
        Ok(())
    }
}

/// Information about a route.
#[derive(Debug, Clone)]
pub struct RouteInfo {
    /// Destination network (0.0.0.0 for default route).
    pub destination: Ipv4Addr,
    /// Destination prefix length.
    pub dst_len: u8,
    /// Gateway address.
    pub gateway: Option<Ipv4Addr>,
    /// Output interface index.
    pub oif: Option<u32>,
    /// Route priority / metric.
    pub priority: Option<u32>,
    /// Route protocol (RTPROT_*).
    pub protocol: u8,
    /// Route scope.
    pub scope: u8,
    /// Route table.
    pub table: u8,
    /// Preferred source address.
    pub prefsrc: Option<Ipv4Addr>,
}

impl fmt::Display for RouteInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.dst_len == 0 {
            write!(f, "default")?;
        } else {
            write!(f, "{}/{}", self.destination, self.dst_len)?;
        }
        if let Some(gw) = self.gateway {
            write!(f, " via {gw}")?;
        }
        if let Some(oif) = self.oif {
            write!(f, " dev idx={oif}")?;
        }
        if let Some(prio) = self.priority {
            write!(f, " metric {prio}")?;
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Netlink socket wrapper
// ---------------------------------------------------------------------------

/// A netlink route socket for communicating with the kernel.
pub struct NetlinkSocket {
    fd: i32,
    seq: u32,
    pid: u32,
}

impl NetlinkSocket {
    /// Open a new NETLINK_ROUTE socket.
    pub fn open() -> io::Result<Self> {
        let fd = unsafe {
            libc::socket(
                libc::AF_NETLINK,
                libc::SOCK_RAW | libc::SOCK_CLOEXEC,
                NETLINK_ROUTE,
            )
        };
        if fd < 0 {
            return Err(io::Error::last_os_error());
        }

        // Bind to an auto-assigned port.
        let mut addr: libc::sockaddr_nl = unsafe { std::mem::zeroed() };
        addr.nl_family = libc::AF_NETLINK as u16;
        addr.nl_pid = 0; // Let kernel assign.
        addr.nl_groups = 0;

        let ret = unsafe {
            libc::bind(
                fd,
                &addr as *const libc::sockaddr_nl as *const libc::sockaddr,
                std::mem::size_of::<libc::sockaddr_nl>() as libc::socklen_t,
            )
        };
        if ret < 0 {
            let err = io::Error::last_os_error();
            unsafe { libc::close(fd) };
            return Err(err);
        }

        // Read back the assigned PID.
        let mut bound_addr: libc::sockaddr_nl = unsafe { std::mem::zeroed() };
        let mut addrlen = std::mem::size_of::<libc::sockaddr_nl>() as libc::socklen_t;
        unsafe {
            libc::getsockname(
                fd,
                &mut bound_addr as *mut libc::sockaddr_nl as *mut libc::sockaddr,
                &mut addrlen,
            );
        }

        // Set a receive timeout so we don't block forever.
        let tv = libc::timeval {
            tv_sec: 5,
            tv_usec: 0,
        };
        unsafe {
            libc::setsockopt(
                fd,
                libc::SOL_SOCKET,
                libc::SO_RCVTIMEO,
                &tv as *const libc::timeval as *const libc::c_void,
                std::mem::size_of::<libc::timeval>() as libc::socklen_t,
            );
        }

        Ok(Self {
            fd,
            seq: 1,
            pid: bound_addr.nl_pid,
        })
    }

    /// Send a netlink message and collect all response messages.
    fn request(&mut self, msg: &[u8]) -> io::Result<Vec<Vec<u8>>> {
        // Send.
        let sent =
            unsafe { libc::send(self.fd, msg.as_ptr() as *const libc::c_void, msg.len(), 0) };
        if sent < 0 {
            return Err(io::Error::last_os_error());
        }

        // Receive response(s).
        let mut responses = Vec::new();
        let mut buf = vec![0u8; 65536];

        loop {
            let n =
                unsafe { libc::recv(self.fd, buf.as_mut_ptr() as *mut libc::c_void, buf.len(), 0) };
            if n < 0 {
                let err = io::Error::last_os_error();
                if err.kind() == io::ErrorKind::WouldBlock || err.kind() == io::ErrorKind::TimedOut
                {
                    break;
                }
                return Err(err);
            }
            let n = n as usize;
            if n == 0 {
                break;
            }

            // Parse individual netlink messages from the received buffer.
            let mut offset = 0;
            let mut done = false;
            while offset + NLMSG_HDR_LEN <= n {
                let nlmsg_len =
                    u32::from_ne_bytes(buf[offset..offset + 4].try_into().unwrap()) as usize;
                let nlmsg_type =
                    u16::from_ne_bytes(buf[offset + 4..offset + 6].try_into().unwrap());
                let nlmsg_flags =
                    u16::from_ne_bytes(buf[offset + 6..offset + 8].try_into().unwrap());

                if nlmsg_len < NLMSG_HDR_LEN || offset + nlmsg_len > n {
                    break;
                }

                match nlmsg_type {
                    NLMSG_DONE => {
                        done = true;
                        break;
                    }
                    NLMSG_ERROR => {
                        // Error message: 16-byte nlmsghdr + 4-byte errno + original header.
                        if nlmsg_len >= NLMSG_HDR_LEN + 4 {
                            let errno = i32::from_ne_bytes(
                                buf[offset + NLMSG_HDR_LEN..offset + NLMSG_HDR_LEN + 4]
                                    .try_into()
                                    .unwrap(),
                            );
                            if errno < 0 {
                                return Err(io::Error::from_raw_os_error(-errno));
                            }
                            // errno == 0 means ACK.
                        }
                        done = true;
                        break;
                    }
                    NLMSG_NOOP => {}
                    _ => {
                        responses.push(buf[offset..offset + nlmsg_len].to_vec());
                    }
                }

                // Check if this is a multi-part message.
                if (nlmsg_flags & NLM_F_MULTI) == 0 && nlmsg_type != NLMSG_DONE {
                    // Single message, may still need to read more (ACK).
                }

                offset += nlmsg_align(nlmsg_len);
            }

            if done {
                break;
            }
        }

        Ok(responses)
    }

    fn next_seq(&mut self) -> u32 {
        let s = self.seq;
        self.seq = self.seq.wrapping_add(1);
        s
    }
}

impl Drop for NetlinkSocket {
    fn drop(&mut self) {
        unsafe {
            libc::close(self.fd);
        }
    }
}

// ---------------------------------------------------------------------------
// Public API — queries
// ---------------------------------------------------------------------------

/// Enumerate all network interfaces.
pub fn list_links() -> io::Result<Vec<LinkInfo>> {
    let mut nl = NetlinkSocket::open()?;
    let seq = nl.next_seq();

    // Build RTM_GETLINK dump request.
    let msg_len = NLMSG_HDR_LEN + IFINFOMSG_LEN;
    let mut msg = vec![0u8; nlmsg_align(msg_len)];

    // nlmsghdr
    put_u32(&mut msg, 0, msg_len as u32);
    put_u16(&mut msg, 4, RTM_GETLINK);
    put_u16(&mut msg, 6, NLM_F_REQUEST | NLM_F_DUMP);
    put_u32(&mut msg, 8, seq);
    put_u32(&mut msg, 12, nl.pid);

    // ifinfomsg — family = AF_UNSPEC
    msg[NLMSG_HDR_LEN] = AF_UNSPEC;

    let responses = nl.request(&msg)?;
    let mut links = Vec::new();

    for resp in &responses {
        if resp.len() < NLMSG_HDR_LEN + IFINFOMSG_LEN {
            continue;
        }
        let nlmsg_type = u16::from_ne_bytes(resp[4..6].try_into().unwrap());
        if nlmsg_type != RTM_NEWLINK {
            continue;
        }

        let payload = &resp[NLMSG_HDR_LEN..];
        let _family = payload[0];
        let _pad = payload[1];
        let _ifi_type = u16::from_ne_bytes(payload[2..4].try_into().unwrap());
        let ifi_index = i32::from_ne_bytes(payload[4..8].try_into().unwrap()) as u32;
        let ifi_flags = u32::from_ne_bytes(payload[8..12].try_into().unwrap());

        let attrs = parse_rtattrs(&payload[IFINFOMSG_LEN..]);

        let name = attrs
            .get(&IFLA_IFNAME)
            .and_then(|d| {
                let s = std::str::from_utf8(d).ok()?;
                Some(s.trim_end_matches('\0').to_string())
            })
            .unwrap_or_default();

        let mac_bytes = attrs.get(&IFLA_ADDRESS).cloned().unwrap_or_default();
        let mac = mac_bytes
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<Vec<_>>()
            .join(":");

        let mtu = attrs
            .get(&IFLA_MTU)
            .and_then(|d| {
                if d.len() >= 4 {
                    Some(u32::from_ne_bytes(d[..4].try_into().unwrap()))
                } else {
                    None
                }
            })
            .unwrap_or(0);

        let operstate = attrs
            .get(&IFLA_OPERSTATE)
            .and_then(|d| d.first().copied())
            .unwrap_or(IF_OPER_UNKNOWN);

        links.push(LinkInfo {
            index: ifi_index,
            name,
            mac,
            mac_bytes,
            mtu,
            flags: ifi_flags,
            operstate,
        });
    }

    Ok(links)
}

/// Enumerate all IPv4 addresses on all interfaces.
pub fn list_addresses() -> io::Result<Vec<AddrInfo>> {
    let mut nl = NetlinkSocket::open()?;
    let seq = nl.next_seq();

    let msg_len = NLMSG_HDR_LEN + IFADDRMSG_LEN;
    let mut msg = vec![0u8; nlmsg_align(msg_len)];

    put_u32(&mut msg, 0, msg_len as u32);
    put_u16(&mut msg, 4, RTM_GETADDR);
    put_u16(&mut msg, 6, NLM_F_REQUEST | NLM_F_DUMP);
    put_u32(&mut msg, 8, seq);
    put_u32(&mut msg, 12, nl.pid);

    // ifaddrmsg — family = AF_INET for IPv4 only.
    msg[NLMSG_HDR_LEN] = AF_INET;

    let responses = nl.request(&msg)?;
    let mut addrs = Vec::new();

    for resp in &responses {
        if resp.len() < NLMSG_HDR_LEN + IFADDRMSG_LEN {
            continue;
        }
        let nlmsg_type = u16::from_ne_bytes(resp[4..6].try_into().unwrap());
        if nlmsg_type != RTM_NEWADDR {
            continue;
        }

        let payload = &resp[NLMSG_HDR_LEN..];
        let family = payload[0];
        if family != AF_INET {
            continue;
        }
        let prefix_len = payload[1];
        let _flags = payload[2];
        let scope = payload[3];
        let ifindex = u32::from_ne_bytes(payload[4..8].try_into().unwrap());

        let attrs = parse_rtattrs(&payload[IFADDRMSG_LEN..]);

        let address = attrs
            .get(&IFA_ADDRESS)
            .and_then(|d| {
                if d.len() >= 4 {
                    Some(Ipv4Addr::new(d[0], d[1], d[2], d[3]))
                } else {
                    None
                }
            })
            .unwrap_or(Ipv4Addr::UNSPECIFIED);

        let local = attrs.get(&IFA_LOCAL).and_then(|d| {
            if d.len() >= 4 {
                Some(Ipv4Addr::new(d[0], d[1], d[2], d[3]))
            } else {
                None
            }
        });

        let broadcast = attrs.get(&IFA_BROADCAST).and_then(|d| {
            if d.len() >= 4 {
                Some(Ipv4Addr::new(d[0], d[1], d[2], d[3]))
            } else {
                None
            }
        });

        let label = attrs.get(&IFA_LABEL).and_then(|d| {
            std::str::from_utf8(d)
                .ok()
                .map(|s| s.trim_end_matches('\0').to_string())
        });

        addrs.push(AddrInfo {
            ifindex,
            family,
            prefix_len,
            address,
            local,
            broadcast,
            label,
            scope,
        });
    }

    Ok(addrs)
}

/// Enumerate all IPv4 routes (main table).
pub fn list_routes() -> io::Result<Vec<RouteInfo>> {
    let mut nl = NetlinkSocket::open()?;
    let seq = nl.next_seq();

    let msg_len = NLMSG_HDR_LEN + RTMSG_LEN;
    let mut msg = vec![0u8; nlmsg_align(msg_len)];

    put_u32(&mut msg, 0, msg_len as u32);
    put_u16(&mut msg, 4, RTM_GETROUTE);
    put_u16(&mut msg, 6, NLM_F_REQUEST | NLM_F_DUMP);
    put_u32(&mut msg, 8, seq);
    put_u32(&mut msg, 12, nl.pid);

    // rtmsg — family = AF_INET.
    let rthdr = NLMSG_HDR_LEN;
    msg[rthdr] = AF_INET; // rtm_family
    msg[rthdr + 4] = RT_TABLE_MAIN; // rtm_table

    let responses = nl.request(&msg)?;
    let mut routes = Vec::new();

    for resp in &responses {
        if resp.len() < NLMSG_HDR_LEN + RTMSG_LEN {
            continue;
        }
        let nlmsg_type = u16::from_ne_bytes(resp[4..6].try_into().unwrap());
        if nlmsg_type != RTM_NEWROUTE {
            continue;
        }

        let payload = &resp[NLMSG_HDR_LEN..];
        let family = payload[0];
        if family != AF_INET {
            continue;
        }
        let dst_len = payload[1];
        let _src_len = payload[2];
        let _tos = payload[3];
        let table = payload[4];
        let protocol = payload[5];
        let scope = payload[6];
        let _route_type = payload[7];

        // Only main table routes.
        if table != RT_TABLE_MAIN {
            continue;
        }

        let attrs = parse_rtattrs(&payload[RTMSG_LEN..]);

        let destination = attrs
            .get(&RTA_DST)
            .and_then(|d| {
                if d.len() >= 4 {
                    Some(Ipv4Addr::new(d[0], d[1], d[2], d[3]))
                } else {
                    None
                }
            })
            .unwrap_or(Ipv4Addr::UNSPECIFIED);

        let gateway = attrs.get(&RTA_GATEWAY).and_then(|d| {
            if d.len() >= 4 {
                Some(Ipv4Addr::new(d[0], d[1], d[2], d[3]))
            } else {
                None
            }
        });

        let oif = attrs.get(&RTA_OIF).and_then(|d| {
            if d.len() >= 4 {
                Some(u32::from_ne_bytes(d[..4].try_into().unwrap()))
            } else {
                None
            }
        });

        let priority = attrs.get(&RTA_PRIORITY).and_then(|d| {
            if d.len() >= 4 {
                Some(u32::from_ne_bytes(d[..4].try_into().unwrap()))
            } else {
                None
            }
        });

        let prefsrc = attrs.get(&RTA_PREFSRC).and_then(|d| {
            if d.len() >= 4 {
                Some(Ipv4Addr::new(d[0], d[1], d[2], d[3]))
            } else {
                None
            }
        });

        routes.push(RouteInfo {
            destination,
            dst_len,
            gateway,
            oif,
            priority,
            protocol,
            scope,
            table,
            prefsrc,
        });
    }

    Ok(routes)
}

/// Find a link by name.
pub fn find_link_by_name(name: &str) -> io::Result<Option<LinkInfo>> {
    let links = list_links()?;
    Ok(links.into_iter().find(|l| l.name == name))
}

/// Find a link by index.
pub fn find_link_by_index(index: u32) -> io::Result<Option<LinkInfo>> {
    let links = list_links()?;
    Ok(links.into_iter().find(|l| l.index == index))
}

/// Get all IPv4 addresses for a specific interface index.
pub fn get_addresses_for_link(ifindex: u32) -> io::Result<Vec<AddrInfo>> {
    let addrs = list_addresses()?;
    Ok(addrs.into_iter().filter(|a| a.ifindex == ifindex).collect())
}

/// Build a map from interface index to interface name.
pub fn build_ifindex_name_map() -> io::Result<HashMap<u32, String>> {
    let links = list_links()?;
    Ok(links.into_iter().map(|l| (l.index, l.name)).collect())
}

// ---------------------------------------------------------------------------
// Public API — modifications
// ---------------------------------------------------------------------------

/// Set a network interface up or down.
pub fn set_link_up(ifindex: u32, up: bool) -> io::Result<()> {
    let mut nl = NetlinkSocket::open()?;
    let seq = nl.next_seq();

    let msg_len = NLMSG_HDR_LEN + IFINFOMSG_LEN;
    let mut msg = vec![0u8; nlmsg_align(msg_len)];

    put_u32(&mut msg, 0, msg_len as u32);
    put_u16(&mut msg, 4, RTM_NEWLINK);
    put_u16(&mut msg, 6, NLM_F_REQUEST | NLM_F_ACK);
    put_u32(&mut msg, 8, seq);
    put_u32(&mut msg, 12, nl.pid);

    // ifinfomsg
    let ifi = NLMSG_HDR_LEN;
    msg[ifi] = AF_UNSPEC;
    put_i32(&mut msg, ifi + 4, ifindex as i32); // ifi_index
    let flags: u32 = if up { IFF_UP } else { 0 };
    put_u32(&mut msg, ifi + 8, flags); // ifi_flags
    put_u32(&mut msg, ifi + 12, IFF_UP); // ifi_change (mask)

    nl.request(&msg)?;
    Ok(())
}

/// Set the MTU on a network interface.
pub fn set_link_mtu(ifindex: u32, mtu: u32) -> io::Result<()> {
    let mut nl = NetlinkSocket::open()?;
    let seq = nl.next_seq();

    let attr_len = rta_aligned_len(4);
    let msg_len = NLMSG_HDR_LEN + IFINFOMSG_LEN + attr_len;
    let mut msg = vec![0u8; nlmsg_align(msg_len)];

    put_u32(&mut msg, 0, msg_len as u32);
    put_u16(&mut msg, 4, RTM_NEWLINK);
    put_u16(&mut msg, 6, NLM_F_REQUEST | NLM_F_ACK);
    put_u32(&mut msg, 8, seq);
    put_u32(&mut msg, 12, nl.pid);

    let ifi = NLMSG_HDR_LEN;
    msg[ifi] = AF_UNSPEC;
    put_i32(&mut msg, ifi + 4, ifindex as i32);

    // IFLA_MTU attribute
    let attr_off = NLMSG_HDR_LEN + IFINFOMSG_LEN;
    put_rta_u32(&mut msg, attr_off, IFLA_MTU, mtu);

    nl.request(&msg)?;
    Ok(())
}

/// Add an IPv4 address to an interface.
pub fn add_address(
    ifindex: u32,
    address: Ipv4Addr,
    prefix_len: u8,
    broadcast: Option<Ipv4Addr>,
) -> io::Result<()> {
    let mut nl = NetlinkSocket::open()?;
    let seq = nl.next_seq();

    // Calculate attribute sizes
    let addr_attr_len = rta_aligned_len(4); // IFA_LOCAL
    let addr2_attr_len = rta_aligned_len(4); // IFA_ADDRESS
    let brd_attr_len = if broadcast.is_some() {
        rta_aligned_len(4)
    } else {
        0
    };
    let msg_len = NLMSG_HDR_LEN + IFADDRMSG_LEN + addr_attr_len + addr2_attr_len + brd_attr_len;
    let mut msg = vec![0u8; nlmsg_align(msg_len)];

    put_u32(&mut msg, 0, msg_len as u32);
    put_u16(&mut msg, 4, RTM_NEWADDR);
    put_u16(
        &mut msg,
        6,
        NLM_F_REQUEST | NLM_F_ACK | NLM_F_CREATE | NLM_F_EXCL,
    );
    put_u32(&mut msg, 8, seq);
    put_u32(&mut msg, 12, nl.pid);

    // ifaddrmsg
    let ifa = NLMSG_HDR_LEN;
    msg[ifa] = AF_INET; // ifa_family
    msg[ifa + 1] = prefix_len; // ifa_prefixlen
    msg[ifa + 2] = 0; // ifa_flags
    msg[ifa + 3] = RT_SCOPE_UNIVERSE; // ifa_scope
    put_u32(&mut msg, ifa + 4, ifindex); // ifa_index

    // IFA_LOCAL
    let mut off = NLMSG_HDR_LEN + IFADDRMSG_LEN;
    put_rta_ipv4(&mut msg, off, IFA_LOCAL, address);
    off += addr_attr_len;

    // IFA_ADDRESS
    put_rta_ipv4(&mut msg, off, IFA_ADDRESS, address);
    off += addr2_attr_len;

    // IFA_BROADCAST
    if let Some(brd) = broadcast {
        put_rta_ipv4(&mut msg, off, IFA_BROADCAST, brd);
    }

    nl.request(&msg)?;
    Ok(())
}

/// Remove an IPv4 address from an interface.
pub fn del_address(ifindex: u32, address: Ipv4Addr, prefix_len: u8) -> io::Result<()> {
    let mut nl = NetlinkSocket::open()?;
    let seq = nl.next_seq();

    let addr_attr_len = rta_aligned_len(4);
    let msg_len = NLMSG_HDR_LEN + IFADDRMSG_LEN + addr_attr_len;
    let mut msg = vec![0u8; nlmsg_align(msg_len)];

    put_u32(&mut msg, 0, msg_len as u32);
    put_u16(&mut msg, 4, RTM_DELADDR);
    put_u16(&mut msg, 6, NLM_F_REQUEST | NLM_F_ACK);
    put_u32(&mut msg, 8, seq);
    put_u32(&mut msg, 12, nl.pid);

    let ifa = NLMSG_HDR_LEN;
    msg[ifa] = AF_INET;
    msg[ifa + 1] = prefix_len;
    msg[ifa + 3] = RT_SCOPE_UNIVERSE;
    put_u32(&mut msg, ifa + 4, ifindex);

    put_rta_ipv4(&mut msg, NLMSG_HDR_LEN + IFADDRMSG_LEN, IFA_LOCAL, address);

    nl.request(&msg)?;
    Ok(())
}

/// Add an IPv4 route.
pub fn add_route(
    destination: Ipv4Addr,
    dst_prefix_len: u8,
    gateway: Option<Ipv4Addr>,
    ifindex: u32,
    metric: Option<u32>,
    protocol: u8,
) -> io::Result<()> {
    let mut nl = NetlinkSocket::open()?;
    let seq = nl.next_seq();

    // Calculate size of route attributes.
    let dst_len = if dst_prefix_len > 0 {
        rta_aligned_len(4)
    } else {
        0
    };
    let gw_len = if gateway.is_some() {
        rta_aligned_len(4)
    } else {
        0
    };
    let oif_len = rta_aligned_len(4);
    let metric_len = if metric.is_some() {
        rta_aligned_len(4)
    } else {
        0
    };

    let msg_len = NLMSG_HDR_LEN + RTMSG_LEN + dst_len + gw_len + oif_len + metric_len;
    let mut msg = vec![0u8; nlmsg_align(msg_len)];

    put_u32(&mut msg, 0, msg_len as u32);
    put_u16(&mut msg, 4, RTM_NEWROUTE);
    put_u16(
        &mut msg,
        6,
        NLM_F_REQUEST | NLM_F_ACK | NLM_F_CREATE | NLM_F_EXCL,
    );
    put_u32(&mut msg, 8, seq);
    put_u32(&mut msg, 12, nl.pid);

    // rtmsg
    let rt = NLMSG_HDR_LEN;
    msg[rt] = AF_INET; // rtm_family
    msg[rt + 1] = dst_prefix_len; // rtm_dst_len
    msg[rt + 2] = 0; // rtm_src_len
    msg[rt + 3] = 0; // rtm_tos
    msg[rt + 4] = RT_TABLE_MAIN; // rtm_table
    msg[rt + 5] = protocol; // rtm_protocol
    msg[rt + 6] = if gateway.is_some() {
        RT_SCOPE_UNIVERSE
    } else {
        RT_SCOPE_LINK
    }; // rtm_scope
    msg[rt + 7] = RTN_UNICAST; // rtm_type

    let mut off = NLMSG_HDR_LEN + RTMSG_LEN;

    // RTA_DST
    if dst_prefix_len > 0 {
        put_rta_ipv4(&mut msg, off, RTA_DST, destination);
        off += dst_len;
    }

    // RTA_GATEWAY
    if let Some(gw) = gateway {
        put_rta_ipv4(&mut msg, off, RTA_GATEWAY, gw);
        off += gw_len;
    }

    // RTA_OIF
    put_rta_u32(&mut msg, off, RTA_OIF, ifindex);
    off += oif_len;

    // RTA_PRIORITY (metric)
    if let Some(m) = metric {
        put_rta_u32(&mut msg, off, RTA_PRIORITY, m);
    }

    nl.request(&msg)?;
    Ok(())
}

/// Delete an IPv4 route.
pub fn del_route(
    destination: Ipv4Addr,
    dst_prefix_len: u8,
    gateway: Option<Ipv4Addr>,
    ifindex: u32,
) -> io::Result<()> {
    let mut nl = NetlinkSocket::open()?;
    let seq = nl.next_seq();

    let dst_len = if dst_prefix_len > 0 {
        rta_aligned_len(4)
    } else {
        0
    };
    let gw_len = if gateway.is_some() {
        rta_aligned_len(4)
    } else {
        0
    };
    let oif_len = rta_aligned_len(4);

    let msg_len = NLMSG_HDR_LEN + RTMSG_LEN + dst_len + gw_len + oif_len;
    let mut msg = vec![0u8; nlmsg_align(msg_len)];

    put_u32(&mut msg, 0, msg_len as u32);
    put_u16(&mut msg, 4, RTM_DELROUTE);
    put_u16(&mut msg, 6, NLM_F_REQUEST | NLM_F_ACK);
    put_u32(&mut msg, 8, seq);
    put_u32(&mut msg, 12, nl.pid);

    let rt = NLMSG_HDR_LEN;
    msg[rt] = AF_INET;
    msg[rt + 1] = dst_prefix_len;
    msg[rt + 4] = RT_TABLE_MAIN;
    msg[rt + 6] = if gateway.is_some() {
        RT_SCOPE_UNIVERSE
    } else {
        RT_SCOPE_LINK
    };
    msg[rt + 7] = RTN_UNICAST;

    let mut off = NLMSG_HDR_LEN + RTMSG_LEN;

    if dst_prefix_len > 0 {
        put_rta_ipv4(&mut msg, off, RTA_DST, destination);
        off += dst_len;
    }

    if let Some(gw) = gateway {
        put_rta_ipv4(&mut msg, off, RTA_GATEWAY, gw);
        off += gw_len;
    }

    put_rta_u32(&mut msg, off, RTA_OIF, ifindex);

    nl.request(&msg)?;
    Ok(())
}

/// Add a default route via the given gateway.
pub fn add_default_route(
    gateway: Ipv4Addr,
    ifindex: u32,
    metric: Option<u32>,
    protocol: u8,
) -> io::Result<()> {
    add_route(
        Ipv4Addr::UNSPECIFIED,
        0,
        Some(gateway),
        ifindex,
        metric,
        protocol,
    )
}

/// Delete all IPv4 addresses on an interface (except link-local).
pub fn flush_addresses(ifindex: u32) -> io::Result<()> {
    let addrs = get_addresses_for_link(ifindex)?;
    for addr in addrs {
        if addr.address.is_link_local() {
            continue;
        }
        if let Err(e) = del_address(ifindex, addr.address, addr.prefix_len) {
            log::warn!(
                "Failed to remove address {}/{} from ifindex {}: {}",
                addr.address,
                addr.prefix_len,
                ifindex,
                e
            );
        }
    }
    Ok(())
}

/// Flush all routes going through a specific interface.
pub fn flush_routes(ifindex: u32) -> io::Result<()> {
    let routes = list_routes()?;
    for route in routes {
        if route.oif == Some(ifindex)
            && let Err(e) = del_route(route.destination, route.dst_len, route.gateway, ifindex) {
                log::warn!(
                    "Failed to remove route {} via {:?} on ifindex {}: {}",
                    route.destination,
                    route.gateway,
                    ifindex,
                    e
                );
            }
    }
    Ok(())
}

/// Write DNS server addresses to /run/systemd/resolve/resolv.conf.
///
/// When systemd-resolved is running it owns `/run/systemd/resolve/` and
/// reads per-link DNS configuration from networkd's state files in
/// `/run/systemd/netif/links/`.  In that case networkd must **not** write
/// to resolved's directory — doing so causes "Permission denied" errors
/// because resolved restricts the directory to its own user.
///
/// We detect resolved by checking for `/run/systemd/resolve/stub-resolv.conf`
/// which resolved creates on startup.
pub fn write_resolv_conf(dns_servers: &[Ipv4Addr], search_domains: &[String]) -> io::Result<()> {
    // If systemd-resolved is running it manages resolv.conf itself.
    // It picks up per-link DNS from /run/systemd/netif/links/ state files
    // that networkd already writes, so we can safely skip.
    let stub = std::path::Path::new("/run/systemd/resolve/stub-resolv.conf");
    if stub.exists() {
        log::debug!(
            "systemd-resolved is running (stub-resolv.conf exists), \
             skipping resolv.conf write — resolved reads per-link DNS from netif state files"
        );
        return Ok(());
    }

    let dir = std::path::Path::new("/run/systemd/resolve");
    if !dir.exists() {
        std::fs::create_dir_all(dir)?;
    }

    let mut content = String::new();
    content.push_str("# Generated by systemd-networkd\n");
    if !search_domains.is_empty() {
        content.push_str("search");
        for domain in search_domains {
            content.push(' ');
            content.push_str(domain);
        }
        content.push('\n');
    }
    for dns in dns_servers {
        content.push_str(&format!("nameserver {dns}\n"));
    }
    if dns_servers.is_empty() {
        // Fallback to localhost stub if no DNS configured.
        content.push_str("nameserver 127.0.0.53\n");
    }

    let path = dir.join("resolv.conf");
    std::fs::write(&path, &content)?;

    // Also update /etc/resolv.conf if it's a symlink to our file or doesn't exist.
    let etc_resolv = std::path::Path::new("/etc/resolv.conf");
    if !etc_resolv.exists() || is_our_resolv_conf(etc_resolv) {
        // Write directly — don't overwrite user-managed resolv.conf.
        // Only write if /etc/resolv.conf is a symlink to our managed file.
    }

    Ok(())
}

/// Check if /etc/resolv.conf points to our managed file.
fn is_our_resolv_conf(path: &std::path::Path) -> bool {
    match std::fs::read_link(path) {
        Ok(target) => {
            let target_str = target.to_string_lossy();
            target_str.contains("systemd/resolve")
                || target_str.contains("run/systemd/resolve")
                || target_str.contains("stub-resolv.conf")
        }
        Err(_) => false,
    }
}

/// Route protocol constant for DHCP-learned routes.
pub fn rtprot_dhcp() -> u8 {
    RTPROT_DHCP
}

/// Route protocol constant for static routes.
pub fn rtprot_static() -> u8 {
    RTPROT_STATIC
}

/// Route protocol constant for boot routes.
pub fn rtprot_boot() -> u8 {
    RTPROT_BOOT
}

// ---------------------------------------------------------------------------
// Netlink attribute parsing
// ---------------------------------------------------------------------------

/// Parse a sequence of rtattr (route attribute) TLVs into a map.
fn parse_rtattrs(data: &[u8]) -> HashMap<u16, Vec<u8>> {
    let mut attrs = HashMap::new();
    let mut offset = 0;

    while offset + 4 <= data.len() {
        let rta_len = u16::from_ne_bytes(data[offset..offset + 2].try_into().unwrap()) as usize;
        let rta_type = u16::from_ne_bytes(data[offset + 2..offset + 4].try_into().unwrap());

        if rta_len < 4 || offset + rta_len > data.len() {
            break;
        }

        // Strip the NLA_TYPE_MASK bits — we only care about the base type.
        let base_type = rta_type & 0x3FFF;
        let payload = data[offset + 4..offset + rta_len].to_vec();
        attrs.insert(base_type, payload);

        offset += rta_align(rta_len);
    }

    attrs
}

// ---------------------------------------------------------------------------
// Serialization helpers
// ---------------------------------------------------------------------------

fn nlmsg_align(len: usize) -> usize {
    (len + NLMSG_ALIGN - 1) & !(NLMSG_ALIGN - 1)
}

fn rta_align(len: usize) -> usize {
    (len + 3) & !3
}

fn rta_aligned_len(payload_len: usize) -> usize {
    rta_align(4 + payload_len) // 4 bytes for rta_len + rta_type
}

fn put_u16(buf: &mut [u8], offset: usize, val: u16) {
    buf[offset..offset + 2].copy_from_slice(&val.to_ne_bytes());
}

fn put_u32(buf: &mut [u8], offset: usize, val: u32) {
    buf[offset..offset + 4].copy_from_slice(&val.to_ne_bytes());
}

fn put_i32(buf: &mut [u8], offset: usize, val: i32) {
    buf[offset..offset + 4].copy_from_slice(&val.to_ne_bytes());
}

/// Write a route attribute with a u32 payload.
fn put_rta_u32(buf: &mut [u8], offset: usize, rta_type: u16, val: u32) {
    let rta_len: u16 = 8; // 4 header + 4 payload
    put_u16(buf, offset, rta_len);
    put_u16(buf, offset + 2, rta_type);
    put_u32(buf, offset + 4, val);
}

/// Write a route attribute with an IPv4 address payload.
fn put_rta_ipv4(buf: &mut [u8], offset: usize, rta_type: u16, addr: Ipv4Addr) {
    let rta_len: u16 = 8; // 4 header + 4 address
    put_u16(buf, offset, rta_len);
    put_u16(buf, offset + 2, rta_type);
    buf[offset + 4..offset + 8].copy_from_slice(&addr.octets());
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_nlmsg_align() {
        assert_eq!(nlmsg_align(1), 4);
        assert_eq!(nlmsg_align(4), 4);
        assert_eq!(nlmsg_align(5), 8);
        assert_eq!(nlmsg_align(16), 16);
        assert_eq!(nlmsg_align(17), 20);
    }

    #[test]
    fn test_rta_align() {
        assert_eq!(rta_align(4), 4);
        assert_eq!(rta_align(5), 8);
        assert_eq!(rta_align(8), 8);
        assert_eq!(rta_align(9), 12);
    }

    #[test]
    fn test_rta_aligned_len() {
        // 4 bytes payload => 4 header + 4 payload = 8 aligned to 8.
        assert_eq!(rta_aligned_len(4), 8);
        // 1 byte payload => 4 header + 1 = 5, aligned to 8.
        assert_eq!(rta_aligned_len(1), 8);
        // 0 bytes => 4, aligned to 4.
        assert_eq!(rta_aligned_len(0), 4);
    }

    #[test]
    fn test_put_u16() {
        let mut buf = [0u8; 4];
        put_u16(&mut buf, 0, 0x1234);
        assert_eq!(u16::from_ne_bytes([buf[0], buf[1]]), 0x1234);
    }

    #[test]
    fn test_put_u32() {
        let mut buf = [0u8; 8];
        put_u32(&mut buf, 0, 0xDEADBEEF);
        assert_eq!(
            u32::from_ne_bytes([buf[0], buf[1], buf[2], buf[3]]),
            0xDEADBEEF
        );
    }

    #[test]
    fn test_put_rta_u32() {
        let mut buf = [0u8; 16];
        put_rta_u32(&mut buf, 0, IFLA_MTU, 1500);
        let rta_len = u16::from_ne_bytes([buf[0], buf[1]]);
        let rta_type = u16::from_ne_bytes([buf[2], buf[3]]);
        let val = u32::from_ne_bytes([buf[4], buf[5], buf[6], buf[7]]);
        assert_eq!(rta_len, 8);
        assert_eq!(rta_type, IFLA_MTU);
        assert_eq!(val, 1500);
    }

    #[test]
    fn test_put_rta_ipv4() {
        let mut buf = [0u8; 16];
        put_rta_ipv4(&mut buf, 0, IFA_LOCAL, Ipv4Addr::new(192, 168, 1, 100));
        let rta_len = u16::from_ne_bytes([buf[0], buf[1]]);
        let rta_type = u16::from_ne_bytes([buf[2], buf[3]]);
        assert_eq!(rta_len, 8);
        assert_eq!(rta_type, IFA_LOCAL);
        assert_eq!(&buf[4..8], &[192, 168, 1, 100]);
    }

    #[test]
    fn test_parse_rtattrs_single() {
        // Build a single rtattr: rta_len=8 (u32 payload), rta_type=4 (IFLA_MTU)
        let mut data = vec![0u8; 8];
        put_u16(&mut data, 0, 8); // rta_len
        put_u16(&mut data, 2, IFLA_MTU); // rta_type
        data[4..8].copy_from_slice(&1500u32.to_ne_bytes()); // payload

        let attrs = parse_rtattrs(&data);
        assert!(attrs.contains_key(&IFLA_MTU));
        let mtu_data = attrs.get(&IFLA_MTU).unwrap();
        assert_eq!(mtu_data.len(), 4);
        assert_eq!(u32::from_ne_bytes(mtu_data[..4].try_into().unwrap()), 1500);
    }

    #[test]
    fn test_parse_rtattrs_multiple() {
        // Build two rtattrs back-to-back.
        let mut data = vec![0u8; 16];

        // First: IFLA_MTU = 9000
        put_u16(&mut data, 0, 8);
        put_u16(&mut data, 2, IFLA_MTU);
        data[4..8].copy_from_slice(&9000u32.to_ne_bytes());

        // Second: IFLA_OPERSTATE = 6 (UP)
        // rta_len = 5 (4 header + 1 byte), padded to 8
        put_u16(&mut data, 8, 5);
        put_u16(&mut data, 10, IFLA_OPERSTATE);
        data[12] = IF_OPER_UP;

        let attrs = parse_rtattrs(&data);
        assert!(attrs.contains_key(&IFLA_MTU));
        assert!(attrs.contains_key(&IFLA_OPERSTATE));

        let state = attrs.get(&IFLA_OPERSTATE).unwrap();
        assert_eq!(state[0], IF_OPER_UP);
    }

    #[test]
    fn test_parse_rtattrs_empty() {
        let attrs = parse_rtattrs(&[]);
        assert!(attrs.is_empty());
    }

    #[test]
    fn test_parse_rtattrs_truncated() {
        // Data too short for even a header.
        let data = [0u8; 3];
        let attrs = parse_rtattrs(&data);
        assert!(attrs.is_empty());
    }

    #[test]
    fn test_parse_rtattrs_bad_length() {
        // rta_len claims 100 but data is only 8 bytes.
        let mut data = [0u8; 8];
        put_u16(&mut data, 0, 100);
        put_u16(&mut data, 2, 1);
        let attrs = parse_rtattrs(&data);
        assert!(attrs.is_empty());
    }

    #[test]
    fn test_link_info_flags() {
        let link = LinkInfo {
            index: 1,
            name: "lo".into(),
            mac: "00:00:00:00:00:00".into(),
            mac_bytes: vec![0; 6],
            mtu: 65536,
            flags: IFF_UP | IFF_LOOPBACK | IFF_RUNNING,
            operstate: IF_OPER_UNKNOWN,
        };
        assert!(link.is_up());
        assert!(link.is_running());
        assert!(link.is_loopback());
        assert_eq!(link.oper_state_str(), "unknown");
    }

    #[test]
    fn test_link_info_not_up() {
        let link = LinkInfo {
            index: 2,
            name: "eth0".into(),
            mac: "52:54:00:12:34:56".into(),
            mac_bytes: vec![0x52, 0x54, 0x00, 0x12, 0x34, 0x56],
            mtu: 1500,
            flags: 0,
            operstate: IF_OPER_DOWN,
        };
        assert!(!link.is_up());
        assert!(!link.is_running());
        assert!(!link.is_loopback());
        assert_eq!(link.oper_state_str(), "down");
    }

    #[test]
    fn test_link_info_display() {
        let link = LinkInfo {
            index: 3,
            name: "wlan0".into(),
            mac: "aa:bb:cc:dd:ee:ff".into(),
            mac_bytes: vec![0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff],
            mtu: 1500,
            flags: IFF_UP | IFF_RUNNING,
            operstate: IF_OPER_UP,
        };
        let s = link.to_string();
        assert!(s.contains("wlan0"));
        assert!(s.contains("aa:bb:cc:dd:ee:ff"));
        assert!(s.contains("1500"));
        assert!(s.contains("up"));
    }

    #[test]
    fn test_addr_info_display() {
        let addr = AddrInfo {
            ifindex: 2,
            family: AF_INET,
            prefix_len: 24,
            address: Ipv4Addr::new(192, 168, 1, 100),
            local: Some(Ipv4Addr::new(192, 168, 1, 100)),
            broadcast: Some(Ipv4Addr::new(192, 168, 1, 255)),
            label: Some("eth0".into()),
            scope: 0,
        };
        let s = addr.to_string();
        assert!(s.contains("192.168.1.100/24"));
        assert!(s.contains("brd 192.168.1.255"));
        assert!(s.contains("label eth0"));
    }

    #[test]
    fn test_route_info_display_default() {
        let route = RouteInfo {
            destination: Ipv4Addr::UNSPECIFIED,
            dst_len: 0,
            gateway: Some(Ipv4Addr::new(192, 168, 1, 1)),
            oif: Some(2),
            priority: Some(100),
            protocol: RTPROT_DHCP,
            scope: RT_SCOPE_UNIVERSE,
            table: RT_TABLE_MAIN,
            prefsrc: None,
        };
        let s = route.to_string();
        assert!(s.contains("default"));
        assert!(s.contains("via 192.168.1.1"));
        assert!(s.contains("metric 100"));
    }

    #[test]
    fn test_route_info_display_subnet() {
        let route = RouteInfo {
            destination: Ipv4Addr::new(10, 0, 0, 0),
            dst_len: 8,
            gateway: None,
            oif: Some(3),
            priority: None,
            protocol: RTPROT_STATIC,
            scope: RT_SCOPE_LINK,
            table: RT_TABLE_MAIN,
            prefsrc: None,
        };
        let s = route.to_string();
        assert!(s.contains("10.0.0.0/8"));
        assert!(!s.contains("via"));
    }

    #[test]
    fn test_is_our_resolv_conf() {
        // A path that doesn't exist should return false.
        assert!(!is_our_resolv_conf(std::path::Path::new(
            "/tmp/definitely_does_not_exist_resolv_test"
        )));
    }

    #[test]
    fn test_rtprot_constants() {
        assert_eq!(rtprot_dhcp(), 16);
        assert_eq!(rtprot_static(), 4);
        assert_eq!(rtprot_boot(), 3);
    }
}
