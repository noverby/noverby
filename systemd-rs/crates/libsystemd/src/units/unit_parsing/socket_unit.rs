use log::trace;

use crate::sockets::{
    FifoConfig, NetlinkSocketConfig, SocketKind, SpecialFileConfig, SpecializedSocketConfig,
    TcpSocketConfig, UdpSocketConfig, UnixSocketConfig,
};
use crate::units::{
    ParsedCommonConfig, ParsedFile, ParsedSection, ParsedSingleSocketConfig, ParsedSocketConfig,
    ParsedSocketSection, ParsingErrorReason, parse_install_section, parse_unit_section,
};
use std::path::PathBuf;

pub fn parse_socket(
    parsed_file: ParsedFile,
    path: &PathBuf,
) -> Result<ParsedSocketConfig, ParsingErrorReason> {
    let mut socket_config = None;
    let mut install_config = None;
    let mut unit_config = None;

    for (name, section) in parsed_file {
        match name.as_str() {
            "[Socket]" => {
                socket_config = match parse_socket_section(section) {
                    Ok(conf) => Some(conf),
                    Err(e) => return Err(e),
                };
            }
            "[Unit]" => {
                unit_config = Some(parse_unit_section(section)?);
            }
            "[Install]" => {
                install_config = Some(parse_install_section(section)?);
            }

            _ if name.starts_with("[X-") || name.starts_with("[x-") => {
                trace!(
                    "Silently ignoring vendor extension section in socket unit {path:?}: {name}"
                );
            }
            _ => {
                trace!("Ignoring unknown section in socket unit {path:?}: {name}");
            }
        }
    }

    let Some(socket_config) = socket_config else {
        return Err(ParsingErrorReason::SectionNotFound("Socket".to_owned()));
    };

    Ok(ParsedSocketConfig {
        common: ParsedCommonConfig {
            name: path.file_name().unwrap().to_str().unwrap().to_owned(),
            unit: unit_config.unwrap_or_else(Default::default),
            install: install_config.unwrap_or_else(Default::default),
        },
        sock: socket_config,
    })
}

fn parse_ipv4_addr(addr: &str) -> Result<std::net::SocketAddrV4, std::net::AddrParseError> {
    let sock: Result<std::net::SocketAddrV4, std::net::AddrParseError> = addr.parse();
    sock
}

fn parse_ipv6_addr(addr: &str) -> Result<std::net::SocketAddrV6, std::net::AddrParseError> {
    let sock: Result<std::net::SocketAddrV6, std::net::AddrParseError> = addr.parse();
    sock
}

fn parse_unix_addr(addr: &str) -> Result<String, ()> {
    if addr.starts_with('/') || addr.starts_with("./") {
        Ok(addr.to_owned())
    } else {
        Err(())
    }
}

/// Parses a ListenNetlink= value into (family, group).
///
/// The format is: `family [group]`
/// where `family` is a netlink family name (e.g. "kobject-uevent", "audit", "route")
/// or a numeric protocol number, and `group` is an optional multicast group number
/// (defaults to 0).
///
/// Examples:
/// - "kobject-uevent 1" → ("kobject-uevent", 1)
/// - "audit" → ("audit", 0)
/// - "route 0" → ("route", 0)
fn parse_netlink_addr(value: &str) -> Result<(String, u32), ParsingErrorReason> {
    let parts: Vec<&str> = value.split_whitespace().collect();
    match parts.len() {
        0 => Err(ParsingErrorReason::Generic(
            "ListenNetlink value is empty".to_owned(),
        )),
        1 => Ok((parts[0].to_owned(), 0)),
        2 => {
            let group = parts[1].parse::<u32>().map_err(|_| {
                ParsingErrorReason::Generic(format!(
                    "ListenNetlink multicast group is not a valid non-negative integer: {}",
                    parts[1]
                ))
            })?;
            Ok((parts[0].to_owned(), group))
        }
        _ => Err(ParsingErrorReason::Generic(format!(
            "ListenNetlink value has too many parts (expected 'family [group]'): {value}"
        ))),
    }
}

fn parse_socket_section(
    mut section: ParsedSection,
) -> Result<ParsedSocketSection, ParsingErrorReason> {
    let fdname = section.remove("FILEDESCRIPTORNAME");
    let services = section.remove("SERVICE");
    let streams = section.remove("LISTENSTREAM");
    let datagrams = section.remove("LISTENDATAGRAM");
    let seqpacks = section.remove("LISTENSEQUENTIALPACKET");
    let fifos = section.remove("LISTENFIFO");
    let netlinks = section.remove("LISTENNETLINK");
    let specials = section.remove("LISTENSPECIAL");
    let defer_trigger = section.remove("DEFERTRIGGER");
    let accept = section.remove("ACCEPT");
    let max_connections = section.remove("MAXCONNECTIONS");
    let max_connections_per_source = section.remove("MAXCONNECTIONSPERSOURCE");
    let socket_mode = section.remove("SOCKETMODE");
    let directory_mode = section.remove("DIRECTORYMODE");
    let pass_credentials = section.remove("PASSCREDENTIALS");
    let pass_security = section.remove("PASSSECURITY");
    let accept_file_descriptors = section.remove("ACCEPTFILEDESCRIPTORS");
    let receive_buffer = section.remove("RECEIVEBUFFER");
    let send_buffer = section.remove("SENDBUFFER");
    let symlinks = section.remove("SYMLINKS");
    let timestamping = section.remove("TIMESTAMPING");
    let remove_on_stop = section.remove("REMOVEONSTOP");
    let writable = section.remove("WRITABLE");

    let exec_config = super::parse_exec_section(&mut section)?;

    for key in section.keys() {
        if key.starts_with("X-") {
            trace!("Silently ignoring vendor extension in [Socket] section: {key}");
            continue;
        }
        trace!("Ignoring unsupported setting in [Socket] section: {key}");
    }
    let fdname = match fdname {
        None => None,
        Some(mut vec) => {
            if vec.len() > 1 {
                return Err(ParsingErrorReason::SettingTooManyValues(
                    "FileDescriptorName".to_owned(),
                    super::map_tuples_to_second(vec),
                ));
            } else if vec.is_empty() {
                None
            } else {
                Some(vec.remove(0).1)
            }
        }
    };

    let services = services
        .map(super::map_tuples_to_second)
        .unwrap_or_default();

    let mut socket_kinds: Vec<(u32, SocketKind)> = Vec::new();
    if let Some(mut streams) = streams {
        for _ in 0..streams.len() {
            let (entry_num, value) = streams.remove(0);
            socket_kinds.push((entry_num, SocketKind::Stream(value)));
        }
    }
    if let Some(mut datagrams) = datagrams {
        for _ in 0..datagrams.len() {
            let (entry_num, value) = datagrams.remove(0);
            socket_kinds.push((entry_num, SocketKind::Datagram(value)));
        }
    }
    if let Some(mut seqpacks) = seqpacks {
        for _ in 0..seqpacks.len() {
            let (entry_num, value) = seqpacks.remove(0);
            socket_kinds.push((entry_num, SocketKind::Sequential(value)));
        }
    }
    if let Some(mut fifos) = fifos {
        for _ in 0..fifos.len() {
            let (entry_num, value) = fifos.remove(0);
            socket_kinds.push((entry_num, SocketKind::Fifo(value)));
        }
    }
    if let Some(mut netlinks) = netlinks {
        for _ in 0..netlinks.len() {
            let (entry_num, value) = netlinks.remove(0);
            socket_kinds.push((entry_num, SocketKind::Netlink(value)));
        }
    }
    if let Some(mut specials) = specials {
        for _ in 0..specials.len() {
            let (entry_num, value) = specials.remove(0);
            socket_kinds.push((entry_num, SocketKind::Special(value)));
        }
    }

    // we need to preserve the original ordering
    socket_kinds.sort_by(|l, r| u32::cmp(&l.0, &r.0));
    let socket_kinds: Vec<SocketKind> = socket_kinds.into_iter().map(|(_, kind)| kind).collect();

    let mut socket_configs = Vec::new();

    for kind in socket_kinds {
        let specialized: SpecializedSocketConfig = match &kind {
            SocketKind::Fifo(addr) => {
                if parse_unix_addr(addr).is_ok() {
                    SpecializedSocketConfig::Fifo(FifoConfig {
                        path: std::path::PathBuf::from(addr),
                    })
                } else {
                    return Err(ParsingErrorReason::UnknownSocketAddr(addr.to_owned()));
                }
            }
            SocketKind::Netlink(value) => {
                let (family, group) = parse_netlink_addr(value)?;
                SpecializedSocketConfig::NetlinkSocket(NetlinkSocketConfig { family, group })
            }
            SocketKind::Special(addr) => {
                if parse_unix_addr(addr).is_ok() {
                    SpecializedSocketConfig::SpecialFile(SpecialFileConfig {
                        path: std::path::PathBuf::from(addr),
                    })
                } else {
                    return Err(ParsingErrorReason::UnknownSocketAddr(addr.to_owned()));
                }
            }
            SocketKind::Sequential(addr) => {
                if parse_unix_addr(addr).is_ok() {
                    SpecializedSocketConfig::UnixSocket(UnixSocketConfig::Sequential(addr.clone()))
                } else {
                    return Err(ParsingErrorReason::UnknownSocketAddr(addr.to_owned()));
                }
            }
            SocketKind::Stream(addr) => {
                if parse_unix_addr(addr).is_ok() {
                    SpecializedSocketConfig::UnixSocket(UnixSocketConfig::Stream(addr.clone()))
                } else if let Ok(addr) = parse_ipv4_addr(addr) {
                    SpecializedSocketConfig::TcpSocket(TcpSocketConfig {
                        addr: std::net::SocketAddr::V4(addr),
                    })
                } else if let Ok(addr) = parse_ipv6_addr(addr) {
                    SpecializedSocketConfig::TcpSocket(TcpSocketConfig {
                        addr: std::net::SocketAddr::V6(addr),
                    })
                } else {
                    return Err(ParsingErrorReason::UnknownSocketAddr(addr.to_owned()));
                }
            }
            SocketKind::Datagram(addr) => {
                if parse_unix_addr(addr).is_ok() {
                    SpecializedSocketConfig::UnixSocket(UnixSocketConfig::Datagram(addr.clone()))
                } else if let Ok(addr) = parse_ipv4_addr(addr) {
                    SpecializedSocketConfig::UdpSocket(UdpSocketConfig {
                        addr: std::net::SocketAddr::V4(addr),
                    })
                } else if let Ok(addr) = parse_ipv6_addr(addr) {
                    SpecializedSocketConfig::UdpSocket(UdpSocketConfig {
                        addr: std::net::SocketAddr::V6(addr),
                    })
                } else {
                    return Err(ParsingErrorReason::UnknownSocketAddr(addr.to_owned()));
                }
            }
        };

        socket_configs.push(ParsedSingleSocketConfig { kind, specialized });
    }

    let max_connections: u64 = match max_connections {
        Some(vec) => {
            if vec.len() == 1 {
                let val = vec[0].1.trim();
                if val.is_empty() {
                    64
                } else {
                    val.parse::<u64>().map_err(|_| {
                        ParsingErrorReason::Generic(format!(
                            "MaxConnections is not a valid non-negative integer: {val}"
                        ))
                    })?
                }
            } else {
                return Err(ParsingErrorReason::SettingTooManyValues(
                    "MaxConnections".to_owned(),
                    super::map_tuples_to_second(vec),
                ));
            }
        }
        None => 64,
    };

    let max_connections_per_source: u64 = match max_connections_per_source {
        Some(vec) => {
            if vec.len() == 1 {
                let val = vec[0].1.trim();
                if val.is_empty() {
                    max_connections
                } else {
                    val.parse::<u64>().map_err(|_| {
                        ParsingErrorReason::Generic(format!(
                            "MaxConnectionsPerSource is not a valid non-negative integer: {val}"
                        ))
                    })?
                }
            } else {
                return Err(ParsingErrorReason::SettingTooManyValues(
                    "MaxConnectionsPerSource".to_owned(),
                    super::map_tuples_to_second(vec),
                ));
            }
        }
        None => max_connections,
    };

    let accept = match accept {
        Some(vec) => {
            if vec.len() == 1 {
                super::string_to_bool(&vec[0].1)
            } else {
                return Err(ParsingErrorReason::SettingTooManyValues(
                    "Accept".to_owned(),
                    super::map_tuples_to_second(vec),
                ));
            }
        }
        None => false,
    };

    let socket_mode: Option<u32> = match socket_mode {
        None => None,
        Some(vec) => {
            if vec.len() == 1 {
                let trimmed = vec[0].1.trim();
                if trimmed.is_empty() {
                    None
                } else {
                    let val = u32::from_str_radix(trimmed, 8).map_err(|_| {
                        ParsingErrorReason::Generic(format!(
                            "SocketMode is not a valid octal mode: {trimmed}"
                        ))
                    })?;
                    if val > 0o7777 {
                        return Err(ParsingErrorReason::Generic(format!(
                            "SocketMode value out of range (must be a valid octal mode, max 7777): {trimmed}"
                        )));
                    }
                    Some(val)
                }
            } else {
                return Err(ParsingErrorReason::SettingTooManyValues(
                    "SocketMode".to_owned(),
                    super::map_tuples_to_second(vec),
                ));
            }
        }
    };

    let directory_mode: Option<u32> = match directory_mode {
        None => None,
        Some(vec) => {
            if vec.len() == 1 {
                let trimmed = vec[0].1.trim();
                if trimmed.is_empty() {
                    None
                } else {
                    let val = u32::from_str_radix(trimmed, 8).map_err(|_| {
                        ParsingErrorReason::Generic(format!(
                            "DirectoryMode is not a valid octal mode: {trimmed}"
                        ))
                    })?;
                    if val > 0o7777 {
                        return Err(ParsingErrorReason::Generic(format!(
                            "DirectoryMode value out of range (must be a valid octal mode, max 7777): {trimmed}"
                        )));
                    }
                    Some(val)
                }
            } else {
                return Err(ParsingErrorReason::SettingTooManyValues(
                    "DirectoryMode".to_owned(),
                    super::map_tuples_to_second(vec),
                ));
            }
        }
    };

    let defer_trigger = match defer_trigger {
        Some(vec) => {
            if vec.len() == 1 {
                let val = vec[0].1.trim();
                if val.eq_ignore_ascii_case("patient") {
                    crate::units::DeferTrigger::Patient
                } else if super::string_to_bool(val) {
                    crate::units::DeferTrigger::Yes
                } else {
                    crate::units::DeferTrigger::No
                }
            } else {
                return Err(ParsingErrorReason::SettingTooManyValues(
                    "DeferTrigger".to_owned(),
                    super::map_tuples_to_second(vec),
                ));
            }
        }
        None => crate::units::DeferTrigger::No,
    };

    let pass_credentials = match pass_credentials {
        Some(vec) => {
            if vec.len() == 1 {
                super::string_to_bool(&vec[0].1)
            } else {
                return Err(ParsingErrorReason::SettingTooManyValues(
                    "PassCredentials".to_owned(),
                    super::map_tuples_to_second(vec),
                ));
            }
        }
        None => false,
    };

    let pass_security = match pass_security {
        Some(vec) => {
            if vec.len() == 1 {
                super::string_to_bool(&vec[0].1)
            } else {
                return Err(ParsingErrorReason::SettingTooManyValues(
                    "PassSecurity".to_owned(),
                    super::map_tuples_to_second(vec),
                ));
            }
        }
        None => false,
    };

    let accept_file_descriptors = match accept_file_descriptors {
        Some(vec) => {
            if vec.len() == 1 {
                super::string_to_bool(&vec[0].1)
            } else {
                return Err(ParsingErrorReason::SettingTooManyValues(
                    "AcceptFileDescriptors".to_owned(),
                    super::map_tuples_to_second(vec),
                ));
            }
        }
        // systemd default: true
        None => true,
    };

    let remove_on_stop = match remove_on_stop {
        Some(vec) => {
            if vec.len() == 1 {
                super::string_to_bool(&vec[0].1)
            } else {
                return Err(ParsingErrorReason::SettingTooManyValues(
                    "RemoveOnStop".to_owned(),
                    super::map_tuples_to_second(vec),
                ));
            }
        }
        None => false,
    };

    let writable = match writable {
        Some(vec) => {
            if vec.len() == 1 {
                super::string_to_bool(&vec[0].1)
            } else {
                return Err(ParsingErrorReason::SettingTooManyValues(
                    "Writable".to_owned(),
                    super::map_tuples_to_second(vec),
                ));
            }
        }
        None => false,
    };

    let receive_buffer: Option<u64> =
        match receive_buffer {
            Some(vec) => {
                if vec.len() == 1 {
                    let val = vec[0].1.trim();
                    if val.is_empty() {
                        None
                    } else {
                        Some(super::parse_byte_size(val).map_err(|e| {
                            ParsingErrorReason::Generic(format!("ReceiveBuffer: {e}"))
                        })?)
                    }
                } else {
                    return Err(ParsingErrorReason::SettingTooManyValues(
                        "ReceiveBuffer".to_owned(),
                        super::map_tuples_to_second(vec),
                    ));
                }
            }
            None => None,
        };

    let send_buffer: Option<u64> = match send_buffer {
        Some(vec) => {
            if vec.len() == 1 {
                let val = vec[0].1.trim();
                if val.is_empty() {
                    None
                } else {
                    Some(
                        super::parse_byte_size(val)
                            .map_err(|e| ParsingErrorReason::Generic(format!("SendBuffer: {e}")))?,
                    )
                }
            } else {
                return Err(ParsingErrorReason::SettingTooManyValues(
                    "SendBuffer".to_owned(),
                    super::map_tuples_to_second(vec),
                ));
            }
        }
        None => None,
    };

    let timestamping = match timestamping {
        Some(vec) => {
            if vec.len() == 1 {
                let val = vec[0].1.trim();
                match val.to_lowercase().as_str() {
                    "off" | "" => crate::units::Timestamping::Off,
                    "us" | "usec" | "μs" => crate::units::Timestamping::Microseconds,
                    "ns" | "nsec" => crate::units::Timestamping::Nanoseconds,
                    _ => {
                        return Err(ParsingErrorReason::Generic(format!(
                            "Timestamping: invalid value '{val}', expected one of: off, us, usec, μs, ns, nsec"
                        )));
                    }
                }
            } else {
                return Err(ParsingErrorReason::SettingTooManyValues(
                    "Timestamping".to_owned(),
                    super::map_tuples_to_second(vec),
                ));
            }
        }
        None => crate::units::Timestamping::Off,
    };

    // Symlinks= takes a space-separated list of file system paths per line.
    // Multiple lines extend the list. An empty value resets the list.
    let symlinks: Vec<String> = match symlinks {
        Some(vec) => {
            let mut paths: Vec<String> = Vec::new();
            for (_line_num, value) in vec {
                let val = value.trim();
                if val.is_empty() {
                    paths.clear();
                } else {
                    for path in val.split_whitespace() {
                        paths.push(path.to_owned());
                    }
                }
            }
            paths
        }
        None => Vec::new(),
    };

    Ok(ParsedSocketSection {
        filedesc_name: fdname,
        services,
        accept,
        sockets: socket_configs,
        max_connections,
        max_connections_per_source,
        socket_mode,
        directory_mode,
        pass_credentials,
        pass_security,
        accept_file_descriptors,
        remove_on_stop,
        receive_buffer,
        send_buffer,
        symlinks,
        timestamping,
        defer_trigger,
        writable,
        exec_section: exec_config,
    })
}
