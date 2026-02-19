#![allow(dead_code)]
//! DNS protocol implementation for systemd-resolved
//!
//! Implements the minimal DNS wire format parsing and construction needed for
//! a stub resolver that forwards queries to upstream servers. Supports:
//! - DNS header parsing and construction (RFC 1035 §4.1.1)
//! - Question section parsing (RFC 1035 §4.1.2)
//! - Domain name compression (RFC 1035 §4.1.4)
//! - Query forwarding via UDP with timeout and retry
//! - TCP fallback for truncated responses
//! - Basic response validation

use std::fmt;
use std::io::{self, Read, Write};
use std::net::{SocketAddr, TcpStream, UdpSocket};
use std::time::Duration;

// ── Constants ──────────────────────────────────────────────────────────────

/// Maximum DNS UDP message size (standard)
pub const MAX_UDP_SIZE: usize = 512;

/// Maximum DNS UDP message size with EDNS0
pub const MAX_EDNS_UDP_SIZE: usize = 4096;

/// Maximum DNS message size (TCP)
pub const MAX_TCP_SIZE: usize = 65535;

/// Maximum domain name length
pub const MAX_NAME_LENGTH: usize = 255;

/// Maximum label length
pub const MAX_LABEL_LENGTH: usize = 63;

/// DNS header size in bytes
pub const HEADER_SIZE: usize = 12;

/// Default query timeout
pub const QUERY_TIMEOUT: Duration = Duration::from_secs(5);

/// Retry timeout (shorter for retries)
pub const RETRY_TIMEOUT: Duration = Duration::from_secs(2);

/// Maximum number of upstream query attempts
pub const MAX_ATTEMPTS: usize = 3;

/// Maximum compression pointer hops (to prevent loops)
const MAX_COMPRESSION_HOPS: usize = 128;

// ── DNS opcodes ────────────────────────────────────────────────────────────

/// DNS operation codes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Opcode {
    Query = 0,
    IQuery = 1,
    Status = 2,
    Notify = 4,
    Update = 5,
    Unknown(u8),
}

impl From<u8> for Opcode {
    fn from(v: u8) -> Self {
        match v {
            0 => Self::Query,
            1 => Self::IQuery,
            2 => Self::Status,
            4 => Self::Notify,
            5 => Self::Update,
            other => Self::Unknown(other),
        }
    }
}

impl From<Opcode> for u8 {
    fn from(op: Opcode) -> u8 {
        match op {
            Opcode::Query => 0,
            Opcode::IQuery => 1,
            Opcode::Status => 2,
            Opcode::Notify => 4,
            Opcode::Update => 5,
            Opcode::Unknown(v) => v,
        }
    }
}

// ── DNS response codes ─────────────────────────────────────────────────────

/// DNS response codes (RCODE)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Rcode {
    NoError = 0,
    FormErr = 1,
    ServFail = 2,
    NXDomain = 3,
    NotImp = 4,
    Refused = 5,
    YXDomain = 6,
    YXRRSet = 7,
    NXRRSet = 8,
    NotAuth = 9,
    NotZone = 10,
    Unknown(u8),
}

impl From<u8> for Rcode {
    fn from(v: u8) -> Self {
        match v {
            0 => Self::NoError,
            1 => Self::FormErr,
            2 => Self::ServFail,
            3 => Self::NXDomain,
            4 => Self::NotImp,
            5 => Self::Refused,
            6 => Self::YXDomain,
            7 => Self::YXRRSet,
            8 => Self::NXRRSet,
            9 => Self::NotAuth,
            10 => Self::NotZone,
            other => Self::Unknown(other),
        }
    }
}

impl fmt::Display for Rcode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoError => write!(f, "NOERROR"),
            Self::FormErr => write!(f, "FORMERR"),
            Self::ServFail => write!(f, "SERVFAIL"),
            Self::NXDomain => write!(f, "NXDOMAIN"),
            Self::NotImp => write!(f, "NOTIMP"),
            Self::Refused => write!(f, "REFUSED"),
            Self::YXDomain => write!(f, "YXDOMAIN"),
            Self::YXRRSet => write!(f, "YXRRSET"),
            Self::NXRRSet => write!(f, "NXRRSET"),
            Self::NotAuth => write!(f, "NOTAUTH"),
            Self::NotZone => write!(f, "NOTZONE"),
            Self::Unknown(v) => write!(f, "RCODE({})", v),
        }
    }
}

// ── DNS record types ───────────────────────────────────────────────────────

/// Common DNS record types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(clippy::upper_case_acronyms)]
pub enum RecordType {
    A,
    AAAA,
    CNAME,
    MX,
    NS,
    PTR,
    SOA,
    SRV,
    TXT,
    ANY,
    OPT,
    Other(u16),
}

impl RecordType {
    pub fn from_u16(v: u16) -> Self {
        match v {
            1 => Self::A,
            28 => Self::AAAA,
            5 => Self::CNAME,
            15 => Self::MX,
            2 => Self::NS,
            12 => Self::PTR,
            6 => Self::SOA,
            33 => Self::SRV,
            16 => Self::TXT,
            255 => Self::ANY,
            41 => Self::OPT,
            other => Self::Other(other),
        }
    }

    pub fn to_u16(self) -> u16 {
        match self {
            Self::A => 1,
            Self::AAAA => 28,
            Self::CNAME => 5,
            Self::MX => 15,
            Self::NS => 2,
            Self::PTR => 12,
            Self::SOA => 6,
            Self::SRV => 33,
            Self::TXT => 16,
            Self::ANY => 255,
            Self::OPT => 41,
            Self::Other(v) => v,
        }
    }
}

impl fmt::Display for RecordType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::A => write!(f, "A"),
            Self::AAAA => write!(f, "AAAA"),
            Self::CNAME => write!(f, "CNAME"),
            Self::MX => write!(f, "MX"),
            Self::NS => write!(f, "NS"),
            Self::PTR => write!(f, "PTR"),
            Self::SOA => write!(f, "SOA"),
            Self::SRV => write!(f, "SRV"),
            Self::TXT => write!(f, "TXT"),
            Self::ANY => write!(f, "ANY"),
            Self::OPT => write!(f, "OPT"),
            Self::Other(v) => write!(f, "TYPE{}", v),
        }
    }
}

// ── DNS record classes ─────────────────────────────────────────────────────

/// DNS record classes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(clippy::upper_case_acronyms)]
pub enum RecordClass {
    IN,
    CH,
    HS,
    ANY,
    Other(u16),
}

impl RecordClass {
    pub fn from_u16(v: u16) -> Self {
        match v {
            1 => Self::IN,
            3 => Self::CH,
            4 => Self::HS,
            255 => Self::ANY,
            other => Self::Other(other),
        }
    }

    pub fn to_u16(self) -> u16 {
        match self {
            Self::IN => 1,
            Self::CH => 3,
            Self::HS => 4,
            Self::ANY => 255,
            Self::Other(v) => v,
        }
    }
}

// ── DNS header ─────────────────────────────────────────────────────────────

/// DNS message header (RFC 1035 §4.1.1)
#[derive(Debug, Clone)]
pub struct DnsHeader {
    /// Query/response identifier
    pub id: u16,
    /// Query (false) or Response (true)
    pub qr: bool,
    /// Operation code
    pub opcode: Opcode,
    /// Authoritative answer
    pub aa: bool,
    /// Truncation
    pub tc: bool,
    /// Recursion desired
    pub rd: bool,
    /// Recursion available
    pub ra: bool,
    /// Authentic data (DNSSEC)
    pub ad: bool,
    /// Checking disabled (DNSSEC)
    pub cd: bool,
    /// Response code
    pub rcode: Rcode,
    /// Number of questions
    pub qdcount: u16,
    /// Number of answer records
    pub ancount: u16,
    /// Number of authority records
    pub nscount: u16,
    /// Number of additional records
    pub arcount: u16,
}

impl DnsHeader {
    /// Parse a DNS header from a byte slice (must be at least 12 bytes)
    pub fn parse(data: &[u8]) -> Result<Self, DnsError> {
        if data.len() < HEADER_SIZE {
            return Err(DnsError::TooShort);
        }

        let id = u16::from_be_bytes([data[0], data[1]]);
        let flags1 = data[2];
        let flags2 = data[3];

        let qr = (flags1 & 0x80) != 0;
        let opcode = Opcode::from((flags1 >> 3) & 0x0F);
        let aa = (flags1 & 0x04) != 0;
        let tc = (flags1 & 0x02) != 0;
        let rd = (flags1 & 0x01) != 0;

        let ra = (flags2 & 0x80) != 0;
        let ad = (flags2 & 0x20) != 0;
        let cd = (flags2 & 0x10) != 0;
        let rcode = Rcode::from(flags2 & 0x0F);

        let qdcount = u16::from_be_bytes([data[4], data[5]]);
        let ancount = u16::from_be_bytes([data[6], data[7]]);
        let nscount = u16::from_be_bytes([data[8], data[9]]);
        let arcount = u16::from_be_bytes([data[10], data[11]]);

        Ok(Self {
            id,
            qr,
            opcode,
            aa,
            tc,
            rd,
            ra,
            ad,
            cd,
            rcode,
            qdcount,
            ancount,
            nscount,
            arcount,
        })
    }

    /// Serialize the header to bytes
    pub fn to_bytes(&self) -> [u8; HEADER_SIZE] {
        let mut buf = [0u8; HEADER_SIZE];

        buf[0..2].copy_from_slice(&self.id.to_be_bytes());

        let mut flags1: u8 = 0;
        if self.qr {
            flags1 |= 0x80;
        }
        flags1 |= (u8::from(self.opcode) & 0x0F) << 3;
        if self.aa {
            flags1 |= 0x04;
        }
        if self.tc {
            flags1 |= 0x02;
        }
        if self.rd {
            flags1 |= 0x01;
        }
        buf[2] = flags1;

        let mut flags2: u8 = 0;
        if self.ra {
            flags2 |= 0x80;
        }
        if self.ad {
            flags2 |= 0x20;
        }
        if self.cd {
            flags2 |= 0x10;
        }
        flags2 |= match self.rcode {
            Rcode::NoError => 0,
            Rcode::FormErr => 1,
            Rcode::ServFail => 2,
            Rcode::NXDomain => 3,
            Rcode::NotImp => 4,
            Rcode::Refused => 5,
            Rcode::YXDomain => 6,
            Rcode::YXRRSet => 7,
            Rcode::NXRRSet => 8,
            Rcode::NotAuth => 9,
            Rcode::NotZone => 10,
            Rcode::Unknown(v) => v & 0x0F,
        };
        buf[3] = flags2;

        buf[4..6].copy_from_slice(&self.qdcount.to_be_bytes());
        buf[6..8].copy_from_slice(&self.ancount.to_be_bytes());
        buf[8..10].copy_from_slice(&self.nscount.to_be_bytes());
        buf[10..12].copy_from_slice(&self.arcount.to_be_bytes());

        buf
    }
}

// ── DNS question ───────────────────────────────────────────────────────────

/// A DNS question entry
#[derive(Debug, Clone)]
pub struct DnsQuestion {
    /// Domain name being queried
    pub name: String,
    /// Record type
    pub qtype: RecordType,
    /// Record class
    pub qclass: RecordClass,
}

impl fmt::Display for DnsQuestion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} {} {}", self.name, self.qtype, self.qclass.to_u16())
    }
}

// ── DNS message ────────────────────────────────────────────────────────────

/// A parsed DNS message (at least the header and questions)
#[derive(Debug, Clone)]
pub struct DnsMessage {
    /// Parsed header
    pub header: DnsHeader,
    /// Parsed question entries
    pub questions: Vec<DnsQuestion>,
    /// The raw message bytes (for forwarding)
    pub raw: Vec<u8>,
}

impl DnsMessage {
    /// Parse a DNS message from raw bytes
    pub fn parse(data: &[u8]) -> Result<Self, DnsError> {
        let header = DnsHeader::parse(data)?;

        let mut offset = HEADER_SIZE;
        let mut questions = Vec::with_capacity(header.qdcount as usize);

        for _ in 0..header.qdcount {
            let (name, new_offset) = parse_name(data, offset)?;
            offset = new_offset;

            if offset + 4 > data.len() {
                return Err(DnsError::TooShort);
            }

            let qtype = RecordType::from_u16(u16::from_be_bytes([data[offset], data[offset + 1]]));
            let qclass =
                RecordClass::from_u16(u16::from_be_bytes([data[offset + 2], data[offset + 3]]));
            offset += 4;

            questions.push(DnsQuestion {
                name,
                qtype,
                qclass,
            });
        }

        Ok(Self {
            header,
            questions,
            raw: data.to_vec(),
        })
    }

    /// Check if this is a query
    pub fn is_query(&self) -> bool {
        !self.header.qr
    }

    /// Get a short summary of the query for logging
    pub fn query_summary(&self) -> String {
        if self.questions.is_empty() {
            return format!("id={} (no questions)", self.header.id);
        }
        let q = &self.questions[0];
        format!("id={} {} {}", self.header.id, q.name, q.qtype)
    }
}

// ── DNS error ──────────────────────────────────────────────────────────────

/// Errors that can occur during DNS parsing or forwarding
#[derive(Debug)]
pub enum DnsError {
    /// Message too short
    TooShort,
    /// Invalid domain name
    InvalidName,
    /// Compression loop detected
    CompressionLoop,
    /// Label too long
    LabelTooLong,
    /// Name too long
    NameTooLong,
    /// I/O error during forwarding
    Io(io::Error),
    /// All upstream servers failed
    AllServersFailed,
    /// Timeout waiting for response
    Timeout,
}

impl fmt::Display for DnsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooShort => write!(f, "DNS message too short"),
            Self::InvalidName => write!(f, "invalid domain name"),
            Self::CompressionLoop => write!(f, "DNS compression pointer loop"),
            Self::LabelTooLong => write!(f, "DNS label exceeds 63 bytes"),
            Self::NameTooLong => write!(f, "domain name exceeds 255 bytes"),
            Self::Io(e) => write!(f, "DNS I/O error: {}", e),
            Self::AllServersFailed => write!(f, "all upstream DNS servers failed"),
            Self::Timeout => write!(f, "DNS query timed out"),
        }
    }
}

impl From<io::Error> for DnsError {
    fn from(e: io::Error) -> Self {
        if e.kind() == io::ErrorKind::TimedOut || e.kind() == io::ErrorKind::WouldBlock {
            Self::Timeout
        } else {
            Self::Io(e)
        }
    }
}

// ── Name parsing ───────────────────────────────────────────────────────────

/// Parse a DNS domain name from the message, handling compression pointers.
/// Returns (name, offset after the name in the original position).
pub fn parse_name(data: &[u8], start: usize) -> Result<(String, usize), DnsError> {
    let mut name = String::with_capacity(64);
    let mut offset = start;
    let mut hops = 0;
    let mut end_offset: Option<usize> = None;

    loop {
        if offset >= data.len() {
            return Err(DnsError::TooShort);
        }

        let len = data[offset] as usize;

        if len == 0 {
            // Root label — end of name
            if end_offset.is_none() {
                end_offset = Some(offset + 1);
            }
            break;
        }

        if (len & 0xC0) == 0xC0 {
            // Compression pointer
            if offset + 1 >= data.len() {
                return Err(DnsError::TooShort);
            }
            let ptr = ((len & 0x3F) << 8) | (data[offset + 1] as usize);
            if end_offset.is_none() {
                end_offset = Some(offset + 2);
            }
            offset = ptr;
            hops += 1;
            if hops > MAX_COMPRESSION_HOPS {
                return Err(DnsError::CompressionLoop);
            }
            continue;
        }

        if len > MAX_LABEL_LENGTH {
            return Err(DnsError::LabelTooLong);
        }

        offset += 1;
        if offset + len > data.len() {
            return Err(DnsError::TooShort);
        }

        if !name.is_empty() {
            name.push('.');
        }

        // Validate and append label bytes
        let label = &data[offset..offset + len];
        for &b in label {
            if b.is_ascii() {
                name.push(b as char);
            } else {
                // Non-ASCII in DNS name — escape as \DDD
                name.push_str(&format!("\\{:03}", b));
            }
        }

        offset += len;

        if name.len() > MAX_NAME_LENGTH {
            return Err(DnsError::NameTooLong);
        }
    }

    if name.is_empty() {
        name.push('.');
    }

    Ok((name, end_offset.unwrap_or(offset)))
}

/// Encode a domain name into DNS wire format (uncompressed)
pub fn encode_name(name: &str) -> Result<Vec<u8>, DnsError> {
    let mut buf = Vec::with_capacity(name.len() + 2);

    if name == "." || name.is_empty() {
        buf.push(0);
        return Ok(buf);
    }

    let name = name.trim_end_matches('.');

    for label in name.split('.') {
        let len = label.len();
        if len > MAX_LABEL_LENGTH {
            return Err(DnsError::LabelTooLong);
        }
        if len == 0 {
            return Err(DnsError::InvalidName);
        }
        buf.push(len as u8);
        buf.extend_from_slice(label.as_bytes());
    }
    buf.push(0); // root label

    if buf.len() > MAX_NAME_LENGTH + 1 {
        return Err(DnsError::NameTooLong);
    }

    Ok(buf)
}

// ── Response construction ──────────────────────────────────────────────────

/// Build a SERVFAIL response for a given query
pub fn build_servfail(query: &[u8]) -> Option<Vec<u8>> {
    if query.len() < HEADER_SIZE {
        return None;
    }

    let mut response = query.to_vec();

    // Set QR=1 (response), keep opcode/RD, set RA=1, RCODE=SERVFAIL
    response[2] = (query[2] & 0x79) | 0x80; // QR=1, keep opcode+RD
    response[3] = 0x82; // RA=1, RCODE=2 (SERVFAIL)

    // Zero out answer/authority/additional counts
    response[6..8].copy_from_slice(&[0, 0]);
    response[8..10].copy_from_slice(&[0, 0]);
    response[10..12].copy_from_slice(&[0, 0]);

    Some(response)
}

/// Build a REFUSED response for a given query
pub fn build_refused(query: &[u8]) -> Option<Vec<u8>> {
    if query.len() < HEADER_SIZE {
        return None;
    }

    let mut response = query.to_vec();
    response[2] = (query[2] & 0x79) | 0x80;
    response[3] = 0x85; // RA=1, RCODE=5 (REFUSED)
    response[6..8].copy_from_slice(&[0, 0]);
    response[8..10].copy_from_slice(&[0, 0]);
    response[10..12].copy_from_slice(&[0, 0]);

    Some(response)
}

/// Build a FORMERR response for malformed queries
pub fn build_formerr(query: &[u8]) -> Option<Vec<u8>> {
    if query.len() < HEADER_SIZE {
        return None;
    }

    let mut response = vec![0u8; HEADER_SIZE];
    response[0..2].copy_from_slice(&query[0..2]); // Copy ID
    response[2] = 0x80; // QR=1
    response[3] = 0x81; // RA=1, RCODE=1 (FORMERR)

    Some(response)
}

// ── DNS forwarding ─────────────────────────────────────────────────────────

/// Forward a DNS query to an upstream server via UDP
pub fn forward_udp(
    query: &[u8],
    upstream: SocketAddr,
    timeout: Duration,
) -> Result<Vec<u8>, DnsError> {
    if query.len() < HEADER_SIZE {
        return Err(DnsError::TooShort);
    }

    let local_addr: SocketAddr = if upstream.is_ipv4() {
        "0.0.0.0:0".parse().unwrap()
    } else {
        "[::]:0".parse().unwrap()
    };

    let socket = UdpSocket::bind(local_addr)?;
    socket.set_read_timeout(Some(timeout))?;
    socket.set_write_timeout(Some(timeout))?;

    socket.send_to(query, upstream)?;

    let mut buf = vec![0u8; MAX_EDNS_UDP_SIZE];
    let (len, _from) = socket.recv_from(&mut buf)?;
    buf.truncate(len);

    // Basic validation: response ID must match query ID
    if buf.len() >= 2 && query.len() >= 2 && buf[0..2] != query[0..2] {
        return Err(DnsError::InvalidName); // ID mismatch
    }

    Ok(buf)
}

/// Forward a DNS query to an upstream server via TCP
pub fn forward_tcp(
    query: &[u8],
    upstream: SocketAddr,
    timeout: Duration,
) -> Result<Vec<u8>, DnsError> {
    if query.len() < HEADER_SIZE {
        return Err(DnsError::TooShort);
    }

    let mut stream = TcpStream::connect_timeout(&upstream, timeout)?;
    stream.set_read_timeout(Some(timeout))?;
    stream.set_write_timeout(Some(timeout))?;

    // TCP DNS uses a 2-byte length prefix
    let len_bytes = (query.len() as u16).to_be_bytes();
    stream.write_all(&len_bytes)?;
    stream.write_all(query)?;
    stream.flush()?;

    // Read response length
    let mut len_buf = [0u8; 2];
    stream.read_exact(&mut len_buf)?;
    let resp_len = u16::from_be_bytes(len_buf) as usize;

    if !(HEADER_SIZE..=MAX_TCP_SIZE).contains(&resp_len) {
        return Err(DnsError::TooShort);
    }

    let mut response = vec![0u8; resp_len];
    stream.read_exact(&mut response)?;

    Ok(response)
}

/// Forward a DNS query to upstream servers, trying each in order.
///
/// First attempts UDP; if the response is truncated (TC bit set),
/// retries via TCP. Tries each server up to MAX_ATTEMPTS times.
pub fn forward_query(query: &[u8], upstreams: &[SocketAddr]) -> Result<Vec<u8>, DnsError> {
    if upstreams.is_empty() {
        return Err(DnsError::AllServersFailed);
    }

    let mut last_error = DnsError::AllServersFailed;

    for upstream in upstreams {
        for attempt in 0..MAX_ATTEMPTS {
            let timeout = if attempt == 0 {
                QUERY_TIMEOUT
            } else {
                RETRY_TIMEOUT
            };

            match forward_udp(query, *upstream, timeout) {
                Ok(response) => {
                    // Check for truncation — retry via TCP
                    if response.len() >= HEADER_SIZE && (response[2] & 0x02) != 0 {
                        match forward_tcp(query, *upstream, QUERY_TIMEOUT) {
                            Ok(tcp_response) => return Ok(tcp_response),
                            Err(e) => {
                                last_error = e;
                                continue;
                            }
                        }
                    }
                    return Ok(response);
                }
                Err(DnsError::Timeout) => {
                    last_error = DnsError::Timeout;
                    // Try again with this server or move to next
                    continue;
                }
                Err(e) => {
                    last_error = e;
                    break; // Move to next server
                }
            }
        }
    }

    Err(last_error)
}

// ── Statistics ──────────────────────────────────────────────────────────────

/// Resolver statistics counters
#[derive(Debug, Default, Clone)]
pub struct ResolverStats {
    /// Total queries received
    pub queries_received: u64,
    /// Queries forwarded to upstream
    pub queries_forwarded: u64,
    /// Successful responses
    pub responses_ok: u64,
    /// SERVFAIL responses sent
    pub responses_servfail: u64,
    /// NXDOMAIN responses received
    pub responses_nxdomain: u64,
    /// FORMERR responses sent
    pub responses_formerr: u64,
    /// Upstream timeouts
    pub upstream_timeouts: u64,
    /// Upstream failures
    pub upstream_failures: u64,
    /// TCP queries (fallback from truncated UDP)
    pub tcp_queries: u64,
}

impl ResolverStats {
    pub fn new() -> Self {
        Self::default()
    }

    /// Format statistics for display
    pub fn display(&self) -> String {
        format!(
            "Queries received: {}\n\
             Queries forwarded: {}\n\
             Responses OK: {}\n\
             Responses SERVFAIL: {}\n\
             Responses NXDOMAIN: {}\n\
             Responses FORMERR: {}\n\
             Upstream timeouts: {}\n\
             Upstream failures: {}\n\
             TCP fallback queries: {}",
            self.queries_received,
            self.queries_forwarded,
            self.responses_ok,
            self.responses_servfail,
            self.responses_nxdomain,
            self.responses_formerr,
            self.upstream_timeouts,
            self.upstream_failures,
            self.tcp_queries,
        )
    }
}

// ── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a minimal DNS query for "example.com" type A class IN
    fn build_test_query() -> Vec<u8> {
        let mut buf = Vec::new();

        // Header: ID=0x1234, QR=0, RD=1, QDCOUNT=1
        buf.extend_from_slice(&[0x12, 0x34]); // ID
        buf.push(0x01); // flags1: RD=1
        buf.push(0x00); // flags2
        buf.extend_from_slice(&[0x00, 0x01]); // QDCOUNT=1
        buf.extend_from_slice(&[0x00, 0x00]); // ANCOUNT=0
        buf.extend_from_slice(&[0x00, 0x00]); // NSCOUNT=0
        buf.extend_from_slice(&[0x00, 0x00]); // ARCOUNT=0

        // Question: example.com A IN
        buf.push(7); // label length
        buf.extend_from_slice(b"example");
        buf.push(3); // label length
        buf.extend_from_slice(b"com");
        buf.push(0); // root

        buf.extend_from_slice(&[0x00, 0x01]); // TYPE=A
        buf.extend_from_slice(&[0x00, 0x01]); // CLASS=IN

        buf
    }

    /// Build a minimal DNS response
    fn build_test_response(query_id: u16) -> Vec<u8> {
        let mut buf = Vec::new();

        // Header
        buf.extend_from_slice(&query_id.to_be_bytes());
        buf.push(0x81); // QR=1, RD=1
        buf.push(0x80); // RA=1, RCODE=0
        buf.extend_from_slice(&[0x00, 0x01]); // QDCOUNT=1
        buf.extend_from_slice(&[0x00, 0x01]); // ANCOUNT=1
        buf.extend_from_slice(&[0x00, 0x00]); // NSCOUNT=0
        buf.extend_from_slice(&[0x00, 0x00]); // ARCOUNT=0

        // Question section (same as query)
        buf.push(7);
        buf.extend_from_slice(b"example");
        buf.push(3);
        buf.extend_from_slice(b"com");
        buf.push(0);
        buf.extend_from_slice(&[0x00, 0x01]); // TYPE=A
        buf.extend_from_slice(&[0x00, 0x01]); // CLASS=IN

        // Answer: example.com A 93.184.216.34 TTL=300
        buf.extend_from_slice(&[0xC0, 0x0C]); // compression pointer to offset 12
        buf.extend_from_slice(&[0x00, 0x01]); // TYPE=A
        buf.extend_from_slice(&[0x00, 0x01]); // CLASS=IN
        buf.extend_from_slice(&[0x00, 0x00, 0x01, 0x2C]); // TTL=300
        buf.extend_from_slice(&[0x00, 0x04]); // RDLENGTH=4
        buf.extend_from_slice(&[93, 184, 216, 34]); // RDATA

        buf
    }

    #[test]
    fn test_header_parse_query() {
        let query = build_test_query();
        let header = DnsHeader::parse(&query).unwrap();

        assert_eq!(header.id, 0x1234);
        assert!(!header.qr);
        assert_eq!(header.opcode, Opcode::Query);
        assert!(!header.aa);
        assert!(!header.tc);
        assert!(header.rd);
        assert!(!header.ra);
        assert_eq!(header.rcode, Rcode::NoError);
        assert_eq!(header.qdcount, 1);
        assert_eq!(header.ancount, 0);
        assert_eq!(header.nscount, 0);
        assert_eq!(header.arcount, 0);
    }

    #[test]
    fn test_header_parse_response() {
        let response = build_test_response(0x1234);
        let header = DnsHeader::parse(&response).unwrap();

        assert_eq!(header.id, 0x1234);
        assert!(header.qr);
        assert!(header.rd);
        assert!(header.ra);
        assert_eq!(header.rcode, Rcode::NoError);
        assert_eq!(header.qdcount, 1);
        assert_eq!(header.ancount, 1);
    }

    #[test]
    fn test_header_roundtrip() {
        let original = build_test_query();
        let header = DnsHeader::parse(&original).unwrap();
        let bytes = header.to_bytes();
        assert_eq!(&bytes, &original[..HEADER_SIZE]);
    }

    #[test]
    fn test_header_too_short() {
        let short = [0u8; 5];
        assert!(matches!(DnsHeader::parse(&short), Err(DnsError::TooShort)));
    }

    #[test]
    fn test_parse_name_simple() {
        let query = build_test_query();
        let (name, offset) = parse_name(&query, HEADER_SIZE).unwrap();
        assert_eq!(name, "example.com");
        assert_eq!(offset, HEADER_SIZE + 13); // 1+7+1+3+1 = 13 bytes
    }

    #[test]
    fn test_parse_name_root() {
        let data = [0u8; 1]; // single zero byte = root
        let (name, offset) = parse_name(&data, 0).unwrap();
        assert_eq!(name, ".");
        assert_eq!(offset, 1);
    }

    #[test]
    fn test_parse_name_with_compression() {
        let response = build_test_response(0x1234);
        // The answer section starts after the question section
        // Find the compression pointer in the answer (0xC0 0x0C)
        let answer_start = HEADER_SIZE + 13 + 4; // header + qname(13) + qtype(2) + qclass(2)
        let (name, _) = parse_name(&response, answer_start).unwrap();
        assert_eq!(name, "example.com");
    }

    #[test]
    fn test_parse_name_compression_loop() {
        // Create a packet with a compression loop: offset 12 points to offset 12
        let mut data = vec![0u8; 14];
        data[12] = 0xC0;
        data[13] = 0x0C; // points back to self
        assert!(matches!(
            parse_name(&data, 12),
            Err(DnsError::CompressionLoop)
        ));
    }

    #[test]
    fn test_parse_name_label_too_long() {
        let mut data = vec![0u8; 70];
        data[0] = 64; // label length 64 > MAX_LABEL_LENGTH (63)
        assert!(matches!(parse_name(&data, 0), Err(DnsError::LabelTooLong)));
    }

    #[test]
    fn test_encode_name_simple() {
        let encoded = encode_name("example.com").unwrap();
        assert_eq!(
            encoded,
            vec![
                7, b'e', b'x', b'a', b'm', b'p', b'l', b'e', 3, b'c', b'o', b'm', 0
            ]
        );
    }

    #[test]
    fn test_encode_name_root() {
        let encoded = encode_name(".").unwrap();
        assert_eq!(encoded, vec![0]);
    }

    #[test]
    fn test_encode_name_trailing_dot() {
        let encoded1 = encode_name("example.com.").unwrap();
        let encoded2 = encode_name("example.com").unwrap();
        assert_eq!(encoded1, encoded2);
    }

    #[test]
    fn test_encode_name_empty() {
        let encoded = encode_name("").unwrap();
        assert_eq!(encoded, vec![0]);
    }

    #[test]
    fn test_encode_name_label_too_long() {
        let long_label = "a".repeat(64);
        assert!(matches!(
            encode_name(&long_label),
            Err(DnsError::LabelTooLong)
        ));
    }

    #[test]
    fn test_encode_name_empty_label() {
        assert!(matches!(
            encode_name("example..com"),
            Err(DnsError::InvalidName)
        ));
    }

    #[test]
    fn test_parse_message_query() {
        let query = build_test_query();
        let msg = DnsMessage::parse(&query).unwrap();

        assert!(msg.is_query());
        assert_eq!(msg.questions.len(), 1);
        assert_eq!(msg.questions[0].name, "example.com");
        assert_eq!(msg.questions[0].qtype, RecordType::A);
        assert_eq!(msg.questions[0].qclass, RecordClass::IN);
    }

    #[test]
    fn test_parse_message_response() {
        let response = build_test_response(0xABCD);
        let msg = DnsMessage::parse(&response).unwrap();

        assert!(!msg.is_query());
        assert_eq!(msg.header.id, 0xABCD);
        assert_eq!(msg.questions.len(), 1);
        assert_eq!(msg.questions[0].name, "example.com");
    }

    #[test]
    fn test_query_summary() {
        let query = build_test_query();
        let msg = DnsMessage::parse(&query).unwrap();
        let summary = msg.query_summary();
        assert!(summary.contains("4660")); // 0x1234 = 4660
        assert!(summary.contains("example.com"));
        assert!(summary.contains("A"));
    }

    #[test]
    fn test_build_servfail() {
        let query = build_test_query();
        let response = build_servfail(&query).unwrap();

        let header = DnsHeader::parse(&response).unwrap();
        assert!(header.qr);
        assert_eq!(header.rcode, Rcode::ServFail);
        assert!(header.ra);
        assert_eq!(header.id, 0x1234);
    }

    #[test]
    fn test_build_refused() {
        let query = build_test_query();
        let response = build_refused(&query).unwrap();

        let header = DnsHeader::parse(&response).unwrap();
        assert!(header.qr);
        assert_eq!(header.rcode, Rcode::Refused);
        assert!(header.ra);
    }

    #[test]
    fn test_build_formerr() {
        let query = build_test_query();
        let response = build_formerr(&query).unwrap();

        let header = DnsHeader::parse(&response).unwrap();
        assert!(header.qr);
        assert_eq!(header.rcode, Rcode::FormErr);
    }

    #[test]
    fn test_build_servfail_too_short() {
        assert!(build_servfail(&[0u8; 5]).is_none());
    }

    #[test]
    fn test_build_refused_too_short() {
        assert!(build_refused(&[0u8; 5]).is_none());
    }

    #[test]
    fn test_build_formerr_too_short() {
        assert!(build_formerr(&[0u8; 5]).is_none());
    }

    #[test]
    fn test_header_all_flags() {
        // Create a header with all flags set
        let header = DnsHeader {
            id: 0xFFFF,
            qr: true,
            opcode: Opcode::Query,
            aa: true,
            tc: true,
            rd: true,
            ra: true,
            ad: true,
            cd: true,
            rcode: Rcode::NoError,
            qdcount: 1,
            ancount: 2,
            nscount: 3,
            arcount: 4,
        };

        let bytes = header.to_bytes();
        let parsed = DnsHeader::parse(&bytes).unwrap();

        assert_eq!(parsed.id, 0xFFFF);
        assert!(parsed.qr);
        assert_eq!(parsed.opcode, Opcode::Query);
        assert!(parsed.aa);
        assert!(parsed.tc);
        assert!(parsed.rd);
        assert!(parsed.ra);
        assert!(parsed.ad);
        assert!(parsed.cd);
        assert_eq!(parsed.rcode, Rcode::NoError);
        assert_eq!(parsed.qdcount, 1);
        assert_eq!(parsed.ancount, 2);
        assert_eq!(parsed.nscount, 3);
        assert_eq!(parsed.arcount, 4);
    }

    #[test]
    fn test_opcode_roundtrip() {
        for code in [0u8, 1, 2, 4, 5, 7, 15] {
            let opcode = Opcode::from(code);
            let back = u8::from(opcode);
            assert_eq!(back, code);
        }
    }

    #[test]
    fn test_rcode_display() {
        assert_eq!(format!("{}", Rcode::NoError), "NOERROR");
        assert_eq!(format!("{}", Rcode::ServFail), "SERVFAIL");
        assert_eq!(format!("{}", Rcode::NXDomain), "NXDOMAIN");
        assert_eq!(format!("{}", Rcode::Unknown(15)), "RCODE(15)");
    }

    #[test]
    fn test_record_type_roundtrip() {
        for val in [1u16, 2, 5, 6, 12, 15, 16, 28, 33, 41, 255, 999] {
            let rt = RecordType::from_u16(val);
            assert_eq!(rt.to_u16(), val);
        }
    }

    #[test]
    fn test_record_type_display() {
        assert_eq!(format!("{}", RecordType::A), "A");
        assert_eq!(format!("{}", RecordType::AAAA), "AAAA");
        assert_eq!(format!("{}", RecordType::CNAME), "CNAME");
        assert_eq!(format!("{}", RecordType::Other(999)), "TYPE999");
    }

    #[test]
    fn test_record_class_roundtrip() {
        for val in [1u16, 3, 4, 255, 500] {
            let rc = RecordClass::from_u16(val);
            assert_eq!(rc.to_u16(), val);
        }
    }

    #[test]
    fn test_dns_error_display() {
        assert_eq!(format!("{}", DnsError::TooShort), "DNS message too short");
        assert_eq!(format!("{}", DnsError::InvalidName), "invalid domain name");
        assert_eq!(
            format!("{}", DnsError::CompressionLoop),
            "DNS compression pointer loop"
        );
        assert_eq!(
            format!("{}", DnsError::AllServersFailed),
            "all upstream DNS servers failed"
        );
        assert_eq!(format!("{}", DnsError::Timeout), "DNS query timed out");
    }

    #[test]
    fn test_resolver_stats_default() {
        let stats = ResolverStats::new();
        assert_eq!(stats.queries_received, 0);
        assert_eq!(stats.queries_forwarded, 0);
        assert_eq!(stats.responses_ok, 0);
    }

    #[test]
    fn test_resolver_stats_display() {
        let mut stats = ResolverStats::new();
        stats.queries_received = 100;
        stats.queries_forwarded = 90;
        stats.responses_ok = 85;
        stats.responses_servfail = 5;
        let display = stats.display();
        assert!(display.contains("Queries received: 100"));
        assert!(display.contains("Queries forwarded: 90"));
        assert!(display.contains("Responses OK: 85"));
        assert!(display.contains("Responses SERVFAIL: 5"));
    }

    #[test]
    fn test_parse_name_multi_label() {
        // Build a packet with "sub.domain.example.com"
        let mut data = vec![0u8; 12]; // dummy header
        data.push(3);
        data.extend_from_slice(b"sub");
        data.push(6);
        data.extend_from_slice(b"domain");
        data.push(7);
        data.extend_from_slice(b"example");
        data.push(3);
        data.extend_from_slice(b"com");
        data.push(0);

        let (name, _) = parse_name(&data, 12).unwrap();
        assert_eq!(name, "sub.domain.example.com");
    }

    #[test]
    fn test_encode_then_parse_name() {
        let names = ["example.com", "sub.domain.example.com", "a.b.c.d.e.f", "x"];

        for &original in &names {
            let encoded = encode_name(original).unwrap();
            // Create a fake packet with just the name
            let (parsed, _) = parse_name(&encoded, 0).unwrap();
            assert_eq!(parsed, original, "roundtrip failed for {}", original);
        }
    }

    #[test]
    fn test_parse_message_too_short() {
        assert!(DnsMessage::parse(&[0u8; 5]).is_err());
    }

    #[test]
    fn test_parse_message_truncated_question() {
        // Header says QDCOUNT=1 but no question data follows
        let mut data = [0u8; HEADER_SIZE];
        data[4] = 0; // QDCOUNT high
        data[5] = 1; // QDCOUNT low
        assert!(DnsMessage::parse(&data).is_err());
    }

    #[test]
    fn test_question_display() {
        let q = DnsQuestion {
            name: "example.com".to_string(),
            qtype: RecordType::A,
            qclass: RecordClass::IN,
        };
        let s = format!("{}", q);
        assert!(s.contains("example.com"));
        assert!(s.contains("A"));
    }

    #[test]
    fn test_parse_name_truncated_label() {
        // Label says length=10 but only 3 bytes follow
        let data = vec![10, b'a', b'b', b'c'];
        assert!(matches!(parse_name(&data, 0), Err(DnsError::TooShort)));
    }

    #[test]
    fn test_parse_name_truncated_pointer() {
        // Compression pointer (0xC0) but missing second byte
        let data = vec![0xC0];
        assert!(matches!(parse_name(&data, 0), Err(DnsError::TooShort)));
    }

    #[test]
    fn test_forward_query_empty_servers() {
        assert!(matches!(
            forward_query(&build_test_query(), &[]),
            Err(DnsError::AllServersFailed)
        ));
    }

    #[test]
    fn test_header_servfail_preserves_rd() {
        let query = build_test_query();
        let response = build_servfail(&query).unwrap();
        let header = DnsHeader::parse(&response).unwrap();
        assert!(header.rd, "RD bit should be preserved from query");
    }

    #[test]
    fn test_parse_name_non_ascii() {
        // Build a name with non-ASCII byte in a label
        let data = vec![3, b'a', 0xFF, b'b', 0];
        let (name, _) = parse_name(&data, 0).unwrap();
        assert!(name.contains("a"));
        assert!(name.contains("\\255")); // 0xFF = 255
        assert!(name.contains("b"));
    }

    #[test]
    fn test_dns_error_from_io_timeout() {
        let io_err = io::Error::new(io::ErrorKind::TimedOut, "timed out");
        let dns_err = DnsError::from(io_err);
        assert!(matches!(dns_err, DnsError::Timeout));
    }

    #[test]
    fn test_dns_error_from_io_other() {
        let io_err = io::Error::new(io::ErrorKind::ConnectionRefused, "refused");
        let dns_err = DnsError::from(io_err);
        assert!(matches!(dns_err, DnsError::Io(_)));
    }
}
