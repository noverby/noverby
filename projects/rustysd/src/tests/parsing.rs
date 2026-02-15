#[test]
fn test_service_parsing() {
    let descr = "This is a description";
    let unit_before1 = "unit_before2";
    let unit_before2 = "unit_before1";
    let unit_after1 = "unit_after1";
    let unit_after2 = "unit_after2,unit_after3";

    let install_required_by = "install_req_by";
    let install_wanted_by = "install_wanted_by";

    let service_execstart = "/path/to/startbin arg1 arg2 arg3";
    let service_execpre = "--/path/to/startprebin arg1 arg2 arg3";
    let service_execpost = "/path/to/startpostbin arg1 arg2 arg3";
    let service_stop = "/path/to/stopbin arg1 arg2 arg3";
    let service_sockets = "socket_name1,socket_name2";

    let test_service_str = format!(
        r#"
    [Unit]
    Description = {}
    Before = {}
    Before = {}
    After = {}
    After = {}

    [Install]
    RequiredBy = {}
    WantedBy = {}

    [Service]
    ExecStart = {}
    ExecStartPre = {}
    ExecStartPost = {}
    ExecStop = {}
    Sockets = {}

    "#,
        descr,
        unit_before1,
        unit_before2,
        unit_after1,
        unit_after2,
        install_required_by,
        install_wanted_by,
        service_execstart,
        service_execpre,
        service_execpost,
        service_stop,
        service_sockets,
    );

    let parsed_file = crate::units::parse_file(&test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    // check all the values

    assert_eq!(service.common.unit.description, descr);
    assert_eq!(
        service.common.unit.before,
        vec![unit_before1.to_owned(), unit_before2.to_owned()]
    );
    assert_eq!(
        service.common.unit.after,
        vec![
            unit_after1.to_owned(),
            "unit_after2".to_owned(),
            "unit_after3".to_owned()
        ]
    );

    assert_eq!(
        service.common.install.required_by,
        vec![install_required_by.to_owned()]
    );
    assert_eq!(
        service.common.install.wanted_by,
        vec![install_wanted_by.to_owned()]
    );

    assert_eq!(
        service.srvc.exec,
        Some(crate::units::Commandline {
            cmd: "/path/to/startbin".into(),
            args: vec!["arg1".into(), "arg2".into(), "arg3".into()],
            prefixes: vec![],
        })
    );
    assert_eq!(
        service.srvc.startpre,
        vec![crate::units::Commandline {
            cmd: "/path/to/startprebin".into(),
            args: vec!["arg1".into(), "arg2".into(), "arg3".into()],
            prefixes: vec![
                crate::units::CommandlinePrefix::Minus,
                crate::units::CommandlinePrefix::Minus,
            ],
        }]
    );
    assert_eq!(
        service.srvc.startpost,
        vec![crate::units::Commandline {
            cmd: "/path/to/startpostbin".into(),
            args: vec!["arg1".into(), "arg2".into(), "arg3".into()],
            prefixes: vec![],
        }]
    );
    assert_eq!(
        service.srvc.stop,
        vec![crate::units::Commandline {
            cmd: "/path/to/stopbin".into(),
            args: vec!["arg1".into(), "arg2".into(), "arg3".into()],
            prefixes: vec![],
        }]
    );
    assert_eq!(
        service.srvc.sockets,
        vec!["socket_name1".to_owned(), "socket_name2".to_owned()]
    );

    // WorkingDirectory should be None when not specified
    assert_eq!(service.srvc.exec_section.working_directory, None);

    // StateDirectory should be empty when not specified
    assert!(service.srvc.exec_section.state_directory.is_empty());
}

#[test]
fn test_service_working_directory_absolute_path() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    WorkingDirectory = /var/lib/myapp
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.exec_section.working_directory,
        Some(std::path::PathBuf::from("/var/lib/myapp"))
    );
}

#[test]
fn test_service_working_directory_tilde() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    WorkingDirectory = ~
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.exec_section.working_directory,
        Some(std::path::PathBuf::from("~"))
    );
}

#[test]
fn test_service_working_directory_dash_prefix() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    WorkingDirectory = -/var/lib/myapp
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    // The '-' prefix should be stripped; path is stored without it
    assert_eq!(
        service.srvc.exec_section.working_directory,
        Some(std::path::PathBuf::from("/var/lib/myapp"))
    );
}

#[test]
fn test_service_working_directory_dash_tilde() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    WorkingDirectory = -~
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    // The '-' prefix should be stripped; ~ is preserved for later resolution
    assert_eq!(
        service.srvc.exec_section.working_directory,
        Some(std::path::PathBuf::from("~"))
    );
}

#[test]
fn test_service_working_directory_too_many_values() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    WorkingDirectory = /var/lib/first
    WorkingDirectory = /var/lib/second
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let result = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    );

    assert!(result.is_err());
}

#[test]
fn test_socket_working_directory() {
    let test_socket_str = r#"
    [Socket]
    ListenStream = /path/to/socket
    WorkingDirectory = /var/lib/socketapp
    "#;

    let parsed_file = crate::units::parse_file(test_socket_str).unwrap();
    let socket_unit = crate::units::parse_socket(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.socket"),
    )
    .unwrap();

    assert_eq!(
        socket_unit.sock.exec_section.working_directory,
        Some(std::path::PathBuf::from("/var/lib/socketapp"))
    );
}

#[test]
fn test_socket_working_directory_not_set() {
    let test_socket_str = r#"
    [Socket]
    ListenStream = /path/to/socket
    "#;

    let parsed_file = crate::units::parse_file(test_socket_str).unwrap();
    let socket_unit = crate::units::parse_socket(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.socket"),
    )
    .unwrap();

    assert_eq!(socket_unit.sock.exec_section.working_directory, None);
}

#[test]
fn test_default_dependencies_defaults_to_true() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert!(
        service.common.unit.default_dependencies,
        "DefaultDependencies should default to true when not specified"
    );
}

#[test]
fn test_default_dependencies_explicit_yes() {
    let test_service_str = r#"
    [Unit]
    DefaultDependencies = yes
    [Service]
    ExecStart = /bin/true
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert!(
        service.common.unit.default_dependencies,
        "DefaultDependencies=yes should be true"
    );
}

#[test]
fn test_default_dependencies_explicit_no() {
    let test_service_str = r#"
    [Unit]
    DefaultDependencies = no
    [Service]
    ExecStart = /bin/true
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert!(
        !service.common.unit.default_dependencies,
        "DefaultDependencies=no should be false"
    );
}

#[test]
fn test_default_dependencies_explicit_true() {
    let test_service_str = r#"
    [Unit]
    DefaultDependencies = true
    [Service]
    ExecStart = /bin/true
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert!(
        service.common.unit.default_dependencies,
        "DefaultDependencies=true should be true"
    );
}

#[test]
fn test_default_dependencies_explicit_false() {
    let test_service_str = r#"
    [Unit]
    DefaultDependencies = false
    [Service]
    ExecStart = /bin/true
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert!(
        !service.common.unit.default_dependencies,
        "DefaultDependencies=false should be false"
    );
}

#[test]
fn test_default_dependencies_explicit_1() {
    let test_service_str = r#"
    [Unit]
    DefaultDependencies = 1
    [Service]
    ExecStart = /bin/true
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert!(
        service.common.unit.default_dependencies,
        "DefaultDependencies=1 should be true"
    );
}

#[test]
fn test_default_dependencies_explicit_0() {
    let test_service_str = r#"
    [Unit]
    DefaultDependencies = 0
    [Service]
    ExecStart = /bin/true
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert!(
        !service.common.unit.default_dependencies,
        "DefaultDependencies=0 should be false"
    );
}

#[test]
fn test_default_dependencies_target_defaults_to_true() {
    let test_target_str = r#"
    [Unit]
    Description = Test target
    "#;

    let parsed_file = crate::units::parse_file(test_target_str).unwrap();
    let target = crate::units::parse_target(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.target"),
    )
    .unwrap();

    assert!(
        target.common.unit.default_dependencies,
        "Target DefaultDependencies should default to true"
    );
}

#[test]
fn test_default_dependencies_target_explicit_no() {
    let test_target_str = r#"
    [Unit]
    Description = Test target
    DefaultDependencies = no
    "#;

    let parsed_file = crate::units::parse_file(test_target_str).unwrap();
    let target = crate::units::parse_target(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.target"),
    )
    .unwrap();

    assert!(
        !target.common.unit.default_dependencies,
        "Target DefaultDependencies=no should be false"
    );
}

#[test]
fn test_default_dependencies_socket_defaults_to_true() {
    let test_socket_str = r#"
    [Socket]
    ListenStream = /path/to/socket
    "#;

    let parsed_file = crate::units::parse_file(test_socket_str).unwrap();
    let socket = crate::units::parse_socket(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.socket"),
    )
    .unwrap();

    assert!(
        socket.common.unit.default_dependencies,
        "Socket DefaultDependencies should default to true"
    );
}

#[test]
fn test_default_dependencies_socket_explicit_no() {
    let test_socket_str = r#"
    [Unit]
    DefaultDependencies = no
    [Socket]
    ListenStream = /path/to/socket
    "#;

    let parsed_file = crate::units::parse_file(test_socket_str).unwrap();
    let socket = crate::units::parse_socket(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.socket"),
    )
    .unwrap();

    assert!(
        !socket.common.unit.default_dependencies,
        "Socket DefaultDependencies=no should be false"
    );
}

#[test]
fn test_socket_parsing() {
    let descr = "This is a description";
    let unit_before1 = "unit_before2";
    let unit_before2 = "unit_before1";
    let unit_after1 = "unit_after1";
    let unit_after2 = "unit_after2,unit_after3";

    let install_required_by = "install_req_by";
    let install_wanted_by = "install_wanted_by";

    let socket_fdname = "SocketyMcSockface";
    let socket_ipv4 = "127.0.0.1:8080";
    let socket_ipv6 = "[fe81::]:8080";
    let socket_unix = "/path/to/socket";
    let socket_service = "other_name";

    let test_service_str = format!(
        r#"
    [Unit]
    Description = {}
    Before = {}
    Before = {}
    After = {}
    After = {}

    [Install]
    RequiredBy = {}
    WantedBy = {}

    [Socket]
    ListenStream = {}
    ListenStream = {}
    ListenStream = {}

    ListenDatagram = {}
    ListenDatagram = {}
    ListenDatagram = {}

    ListenSequentialPacket = {}
    ListenFifo = {}
    Service= {}
    FileDescriptorName= {}

    "#,
        descr,
        unit_before1,
        unit_before2,
        unit_after1,
        unit_after2,
        install_required_by,
        install_wanted_by,
        socket_ipv4,
        socket_ipv6,
        socket_unix,
        socket_ipv4,
        socket_ipv6,
        socket_unix,
        socket_unix,
        socket_unix,
        socket_service,
        socket_fdname,
    );

    let parsed_file = crate::units::parse_file(&test_service_str).unwrap();
    let socket_unit = crate::units::parse_socket(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.socket"),
    )
    .unwrap();

    // check all the values

    assert_eq!(socket_unit.common.unit.description, descr);
    assert_eq!(
        socket_unit.common.unit.before,
        vec![unit_before1.to_owned(), unit_before2.to_owned()]
    );
    assert_eq!(
        socket_unit.common.unit.after,
        vec![
            unit_after1.to_owned(),
            "unit_after2".to_owned(),
            "unit_after3".to_owned()
        ]
    );

    assert_eq!(
        socket_unit.common.install.required_by,
        vec![install_required_by.to_owned()]
    );
    assert_eq!(
        socket_unit.common.install.wanted_by,
        vec![install_wanted_by.to_owned()]
    );
    if socket_unit.sock.sockets.len() == 8 {
        // streaming sockets
        if let crate::sockets::SpecializedSocketConfig::TcpSocket(tcpconf) =
            &socket_unit.sock.sockets[0].specialized
        {
            if !tcpconf.addr.is_ipv4() {
                panic!("Should have been an ipv4 address but wasnt");
            }
        } else {
            panic!("Sockets[0] should have been a tcp socket, but wasnt");
        }
        if let crate::sockets::SpecializedSocketConfig::TcpSocket(tcpconf) =
            &socket_unit.sock.sockets[1].specialized
        {
            if !tcpconf.addr.is_ipv6() {
                panic!("Should have been an ipv6 address but wasnt");
            }
        } else {
            panic!("Sockets[1] should have been a tcp socket, but wasnt");
        }
        if let crate::sockets::SpecializedSocketConfig::UnixSocket(
            crate::sockets::UnixSocketConfig::Stream(addr),
        ) = &socket_unit.sock.sockets[2].specialized
        {
            assert_eq!(addr, socket_unix);
        } else {
            panic!("Sockets[2] should have been a streaming unix socket, but wasnt");
        }

        // Datagram sockets
        if let crate::sockets::SpecializedSocketConfig::UdpSocket(tcpconf) =
            &socket_unit.sock.sockets[3].specialized
        {
            if !tcpconf.addr.is_ipv4() {
                panic!("Should have been an ipv4 address but wasnt");
            }
        } else {
            panic!("Sockets[3] should have been a udp socket, but wasnt");
        }
        if let crate::sockets::SpecializedSocketConfig::UdpSocket(tcpconf) =
            &socket_unit.sock.sockets[4].specialized
        {
            if !tcpconf.addr.is_ipv6() {
                panic!("Should have been an ipv6 address but wasnt");
            }
        } else {
            panic!("Sockets[4] should have been a udp socket, but wasnt");
        }
        if let crate::sockets::SpecializedSocketConfig::UnixSocket(
            crate::sockets::UnixSocketConfig::Datagram(addr),
        ) = &socket_unit.sock.sockets[5].specialized
        {
            assert_eq!(addr, socket_unix);
        } else {
            panic!("Sockets[5] should have been a datagram unix socket, but wasnt");
        }

        // SeqPacket socket
        if let crate::sockets::SpecializedSocketConfig::UnixSocket(
            crate::sockets::UnixSocketConfig::Sequential(addr),
        ) = &socket_unit.sock.sockets[6].specialized
        {
            assert_eq!(addr, socket_unix);
        } else {
            panic!("Sockets[6] should have been a sequential packet unix socket, but wasnt");
        }
        // SeqPacket socket
        if let crate::sockets::SpecializedSocketConfig::Fifo(fifoconf) =
            &socket_unit.sock.sockets[7].specialized
        {
            assert_eq!(fifoconf.path, std::path::PathBuf::from(socket_unix));
        } else {
            panic!("Sockets[6] should have been a sequential packet unix socket, but wasnt");
        }
    } else {
        panic!("Not enough sockets parsed");
    }
}

#[test]
fn test_documentation_empty_by_default() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert!(
        service.common.unit.documentation.is_empty(),
        "Documentation should be empty when not specified"
    );
}

#[test]
fn test_documentation_single_entry() {
    let test_service_str = r#"
    [Unit]
    Documentation = https://example.com/docs
    [Service]
    ExecStart = /bin/true
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.common.unit.documentation,
        vec!["https://example.com/docs".to_owned()]
    );
}

#[test]
fn test_documentation_multiple_entries() {
    let test_service_str = r#"
    [Unit]
    Documentation = https://example.com/docs1
    Documentation = https://example.com/docs2
    [Service]
    ExecStart = /bin/true
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.common.unit.documentation,
        vec![
            "https://example.com/docs1".to_owned(),
            "https://example.com/docs2".to_owned()
        ]
    );
}

#[test]
fn test_documentation_comma_separated() {
    let test_service_str = r#"
    [Unit]
    Documentation = https://example.com/a,https://example.com/b
    [Service]
    ExecStart = /bin/true
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.common.unit.documentation,
        vec![
            "https://example.com/a".to_owned(),
            "https://example.com/b".to_owned()
        ]
    );
}

#[test]
fn test_documentation_various_uri_schemes() {
    let test_service_str = r#"
    [Unit]
    Documentation = man:myapp(1)
    Documentation = https://example.com/docs
    Documentation = file:/usr/share/doc/myapp/README
    [Service]
    ExecStart = /bin/true
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.common.unit.documentation,
        vec![
            "man:myapp(1)".to_owned(),
            "https://example.com/docs".to_owned(),
            "file:/usr/share/doc/myapp/README".to_owned(),
        ]
    );
}

#[test]
fn test_documentation_target_unit() {
    let test_target_str = r#"
    [Unit]
    Description = Test target
    Documentation = https://example.com/target-docs
    "#;

    let parsed_file = crate::units::parse_file(test_target_str).unwrap();
    let target = crate::units::parse_target(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.target"),
    )
    .unwrap();

    assert_eq!(
        target.common.unit.documentation,
        vec!["https://example.com/target-docs".to_owned()]
    );
}

#[test]
fn test_documentation_target_unit_empty_by_default() {
    let test_target_str = r#"
    [Unit]
    Description = Test target
    "#;

    let parsed_file = crate::units::parse_file(test_target_str).unwrap();
    let target = crate::units::parse_target(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.target"),
    )
    .unwrap();

    assert!(
        target.common.unit.documentation.is_empty(),
        "Target Documentation should be empty when not specified"
    );
}

#[test]
fn test_documentation_socket_unit() {
    let test_socket_str = r#"
    [Unit]
    Documentation = man:mysocket(5)
    [Socket]
    ListenStream = /path/to/socket
    "#;

    let parsed_file = crate::units::parse_file(test_socket_str).unwrap();
    let socket = crate::units::parse_socket(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.socket"),
    )
    .unwrap();

    assert_eq!(
        socket.common.unit.documentation,
        vec!["man:mysocket(5)".to_owned()]
    );
}

#[test]
fn test_documentation_socket_unit_empty_by_default() {
    let test_socket_str = r#"
    [Socket]
    ListenStream = /path/to/socket
    "#;

    let parsed_file = crate::units::parse_file(test_socket_str).unwrap();
    let socket = crate::units::parse_socket(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.socket"),
    )
    .unwrap();

    assert!(
        socket.common.unit.documentation.is_empty(),
        "Socket Documentation should be empty when not specified"
    );
}

#[test]
fn test_documentation_preserved_after_unit_conversion_service() {
    use crate::units::Unit;
    use std::convert::TryInto;

    let test_service_str = r#"
    [Unit]
    Documentation = https://example.com/service-docs
    Documentation = man:myservice(8)
    [Service]
    ExecStart = /bin/true
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/myservice.service"),
    )
    .unwrap();
    let unit: Unit = service.try_into().unwrap();

    assert_eq!(
        unit.common.unit.documentation,
        vec![
            "https://example.com/service-docs".to_owned(),
            "man:myservice(8)".to_owned(),
        ],
        "Documentation should be preserved through TryInto<Unit> conversion"
    );
}

#[test]
fn test_documentation_preserved_after_unit_conversion_target() {
    use crate::units::Unit;
    use std::convert::TryInto;

    let test_target_str = r#"
    [Unit]
    Description = My target
    Documentation = https://example.com/target
    "#;

    let parsed_file = crate::units::parse_file(test_target_str).unwrap();
    let target =
        crate::units::parse_target(parsed_file, &std::path::PathBuf::from("/path/to/my.target"))
            .unwrap();
    let unit: Unit = target.try_into().unwrap();

    assert_eq!(
        unit.common.unit.documentation,
        vec!["https://example.com/target".to_owned()],
        "Documentation should be preserved through TryInto<Unit> conversion for targets"
    );
}

#[test]
fn test_documentation_preserved_after_unit_conversion_socket() {
    use crate::units::Unit;
    use std::convert::TryInto;

    let test_socket_str = r#"
    [Unit]
    Documentation = man:mysock(7)
    [Socket]
    ListenStream = /path/to/socket
    "#;

    let parsed_file = crate::units::parse_file(test_socket_str).unwrap();
    let socket =
        crate::units::parse_socket(parsed_file, &std::path::PathBuf::from("/path/to/my.socket"))
            .unwrap();
    let unit: Unit = socket.try_into().unwrap();

    assert_eq!(
        unit.common.unit.documentation,
        vec!["man:mysock(7)".to_owned()],
        "Documentation should be preserved through TryInto<Unit> conversion for sockets"
    );
}

#[test]
fn test_documentation_empty_preserved_after_unit_conversion() {
    use crate::units::Unit;
    use std::convert::TryInto;

    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/myservice.service"),
    )
    .unwrap();
    let unit: Unit = service.try_into().unwrap();

    assert!(
        unit.common.unit.documentation.is_empty(),
        "Empty Documentation should remain empty through TryInto<Unit> conversion"
    );
}

#[test]
fn test_conflicts_parsing_empty_by_default() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert!(
        service.common.unit.conflicts.is_empty(),
        "Conflicts should be empty when not specified"
    );
}

#[test]
fn test_conflicts_parsing_single() {
    let test_service_str = r#"
    [Unit]
    Conflicts = other.service
    [Service]
    ExecStart = /bin/true
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.common.unit.conflicts,
        vec!["other.service".to_owned()]
    );
}

#[test]
fn test_conflicts_parsing_multiple_entries() {
    let test_service_str = r#"
    [Unit]
    Conflicts = first.service
    Conflicts = second.service
    [Service]
    ExecStart = /bin/true
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.common.unit.conflicts,
        vec!["first.service".to_owned(), "second.service".to_owned()]
    );
}

#[test]
fn test_conflicts_parsing_comma_separated() {
    let test_service_str = r#"
    [Unit]
    Conflicts = first.service,second.service
    [Service]
    ExecStart = /bin/true
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.common.unit.conflicts,
        vec!["first.service".to_owned(), "second.service".to_owned()]
    );
}

#[test]
fn test_conflicts_parsing_target_unit() {
    let test_target_str = r#"
    [Unit]
    Description = Test target
    Conflicts = other.target
    "#;

    let parsed_file = crate::units::parse_file(test_target_str).unwrap();
    let target = crate::units::parse_target(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.target"),
    )
    .unwrap();

    assert_eq!(
        target.common.unit.conflicts,
        vec!["other.target".to_owned()]
    );
}

#[test]
fn test_conflicts_parsing_socket_unit() {
    let test_socket_str = r#"
    [Unit]
    Conflicts = other.service
    [Socket]
    ListenStream = /path/to/socket
    "#;

    let parsed_file = crate::units::parse_file(test_socket_str).unwrap();
    let socket = crate::units::parse_socket(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.socket"),
    )
    .unwrap();

    assert_eq!(
        socket.common.unit.conflicts,
        vec!["other.service".to_owned()]
    );
}

#[test]
fn test_conflicts_parsing_mixed_unit_types() {
    let test_service_str = r#"
    [Unit]
    Conflicts = other.service,some.target,another.socket
    [Service]
    ExecStart = /bin/true
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.common.unit.conflicts,
        vec![
            "other.service".to_owned(),
            "some.target".to_owned(),
            "another.socket".to_owned()
        ]
    );
}

#[test]
fn test_state_directory_single() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    StateDirectory = myapp
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.exec_section.state_directory,
        vec!["myapp".to_owned()]
    );
}

#[test]
fn test_state_directory_multiple_space_separated() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    StateDirectory = myapp myapp-extra
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.exec_section.state_directory,
        vec!["myapp".to_owned(), "myapp-extra".to_owned()]
    );
}

#[test]
fn test_state_directory_multiple_entries() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    StateDirectory = myapp
    StateDirectory = other
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.exec_section.state_directory,
        vec!["myapp".to_owned(), "other".to_owned()]
    );
}

#[test]
fn test_state_directory_empty_by_default() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert!(service.srvc.exec_section.state_directory.is_empty());
}

#[test]
fn test_also_empty_by_default() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert!(service.common.install.also.is_empty());
}

#[test]
fn test_also_single() {
    let test_service_str = r#"
    [Install]
    Also = other.service

    [Service]
    ExecStart = /bin/true
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.common.install.also,
        vec!["other.service".to_owned()]
    );
}

#[test]
fn test_also_multiple_entries() {
    let test_service_str = r#"
    [Install]
    Also = other.service
    Also = another.socket

    [Service]
    ExecStart = /bin/true
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.common.install.also,
        vec!["other.service".to_owned(), "another.socket".to_owned()]
    );
}

#[test]
fn test_also_comma_separated() {
    let test_service_str = r#"
    [Install]
    Also = other.service,another.socket

    [Service]
    ExecStart = /bin/true
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.common.install.also,
        vec!["other.service".to_owned(), "another.socket".to_owned()]
    );
}

#[test]
fn test_also_target_unit() {
    let test_service_str = r#"
    [Unit]
    Description = A target

    [Install]
    Also = helper.service
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let target = crate::units::parse_target(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.target"),
    )
    .unwrap();

    assert_eq!(
        target.common.install.also,
        vec!["helper.service".to_owned()]
    );
}

#[test]
fn test_also_socket_unit() {
    let test_service_str = r#"
    [Socket]
    ListenStream = /run/test.sock

    [Install]
    Also = test.service
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let socket = crate::units::parse_socket(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.socket"),
    )
    .unwrap();

    assert_eq!(socket.common.install.also, vec!["test.service".to_owned()]);
}

#[test]
fn test_restart_sec_not_set() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(service.srvc.restart_sec, None);
}

#[test]
fn test_restart_sec_seconds() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    RestartSec = 5
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.restart_sec,
        Some(crate::units::Timeout::Duration(
            std::time::Duration::from_secs(5)
        ))
    );
}

#[test]
fn test_restart_sec_infinity() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    RestartSec = infinity
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.restart_sec,
        Some(crate::units::Timeout::Infinity)
    );
}

#[test]
fn test_restart_sec_compound_duration() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    RestartSec = 1min 30s
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.restart_sec,
        Some(crate::units::Timeout::Duration(
            std::time::Duration::from_secs(90)
        ))
    );
}

#[test]
fn test_restart_sec_with_restart_always() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    Restart = always
    RestartSec = 10
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(service.srvc.restart, crate::units::ServiceRestart::Always);
    assert_eq!(
        service.srvc.restart_sec,
        Some(crate::units::Timeout::Duration(
            std::time::Duration::from_secs(10)
        ))
    );
}

#[test]
fn test_kill_mode_defaults_to_control_group() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(service.srvc.kill_mode, crate::units::KillMode::ControlGroup);
}

#[test]
fn test_kill_mode_control_group() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    KillMode = control-group
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(service.srvc.kill_mode, crate::units::KillMode::ControlGroup);
}

#[test]
fn test_kill_mode_process() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    KillMode = process
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(service.srvc.kill_mode, crate::units::KillMode::Process);
}

#[test]
fn test_kill_mode_mixed() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    KillMode = mixed
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(service.srvc.kill_mode, crate::units::KillMode::Mixed);
}

#[test]
fn test_kill_mode_none() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    KillMode = none
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(service.srvc.kill_mode, crate::units::KillMode::None);
}

#[test]
fn test_kill_mode_case_insensitive() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    KillMode = Process
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(service.srvc.kill_mode, crate::units::KillMode::Process);
}

#[test]
fn test_kill_mode_with_restart() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    Restart = always
    KillMode = process
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(service.srvc.restart, crate::units::ServiceRestart::Always);
    assert_eq!(service.srvc.kill_mode, crate::units::KillMode::Process);
}

#[test]
fn test_delegate_defaults_to_no() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(service.srvc.delegate, crate::units::Delegate::No);
}

#[test]
fn test_delegate_yes() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    Delegate = yes
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(service.srvc.delegate, crate::units::Delegate::Yes);
}

#[test]
fn test_delegate_true() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    Delegate = true
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(service.srvc.delegate, crate::units::Delegate::Yes);
}

#[test]
fn test_delegate_no() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    Delegate = no
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(service.srvc.delegate, crate::units::Delegate::No);
}

#[test]
fn test_delegate_false() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    Delegate = false
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(service.srvc.delegate, crate::units::Delegate::No);
}

#[test]
fn test_delegate_controller_list() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    Delegate = cpu memory io
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.delegate,
        crate::units::Delegate::Controllers(vec![
            "cpu".to_owned(),
            "memory".to_owned(),
            "io".to_owned(),
        ])
    );
}

#[test]
fn test_delegate_single_controller() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    Delegate = cpu
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.delegate,
        crate::units::Delegate::Controllers(vec!["cpu".to_owned()])
    );
}

#[test]
fn test_tasks_max_not_set() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(service.srvc.tasks_max, None);
}

#[test]
fn test_tasks_max_absolute_value() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    TasksMax = 4096
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.tasks_max,
        Some(crate::units::TasksMax::Value(4096))
    );
}

#[test]
fn test_tasks_max_percentage() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    TasksMax = 80%
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.tasks_max,
        Some(crate::units::TasksMax::Percent(80))
    );
}

#[test]
fn test_tasks_max_infinity() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    TasksMax = infinity
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.tasks_max,
        Some(crate::units::TasksMax::Infinity)
    );
}

#[test]
fn test_tasks_max_one() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    TasksMax = 1
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.tasks_max,
        Some(crate::units::TasksMax::Value(1))
    );
}

#[test]
fn test_tasks_max_hundred_percent() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    TasksMax = 100%
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.tasks_max,
        Some(crate::units::TasksMax::Percent(100))
    );
}

#[test]
fn test_tasks_max_with_other_settings() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    Restart = always
    TasksMax = 512
    KillMode = process
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(service.srvc.restart, crate::units::ServiceRestart::Always);
    assert_eq!(
        service.srvc.tasks_max,
        Some(crate::units::TasksMax::Value(512))
    );
    assert_eq!(service.srvc.kill_mode, crate::units::KillMode::Process);
}

#[test]
fn test_limit_nofile_not_set() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(service.srvc.limit_nofile, None);
}

#[test]
fn test_limit_nofile_single_value() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    LimitNOFILE = 65536
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.limit_nofile,
        Some(crate::units::ResourceLimit {
            soft: crate::units::RLimitValue::Value(65536),
            hard: crate::units::RLimitValue::Value(65536),
        })
    );
}

#[test]
fn test_limit_nofile_soft_hard() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    LimitNOFILE = 1024:65536
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.limit_nofile,
        Some(crate::units::ResourceLimit {
            soft: crate::units::RLimitValue::Value(1024),
            hard: crate::units::RLimitValue::Value(65536),
        })
    );
}

#[test]
fn test_limit_nofile_infinity() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    LimitNOFILE = infinity
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.limit_nofile,
        Some(crate::units::ResourceLimit {
            soft: crate::units::RLimitValue::Infinity,
            hard: crate::units::RLimitValue::Infinity,
        })
    );
}

#[test]
fn test_limit_nofile_infinity_case_insensitive() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    LimitNOFILE = Infinity
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.limit_nofile,
        Some(crate::units::ResourceLimit {
            soft: crate::units::RLimitValue::Infinity,
            hard: crate::units::RLimitValue::Infinity,
        })
    );
}

#[test]
fn test_limit_nofile_soft_infinity_hard_value() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    LimitNOFILE = infinity:524288
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.limit_nofile,
        Some(crate::units::ResourceLimit {
            soft: crate::units::RLimitValue::Infinity,
            hard: crate::units::RLimitValue::Value(524288),
        })
    );
}

#[test]
fn test_limit_nofile_soft_value_hard_infinity() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    LimitNOFILE = 1024:infinity
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.limit_nofile,
        Some(crate::units::ResourceLimit {
            soft: crate::units::RLimitValue::Value(1024),
            hard: crate::units::RLimitValue::Infinity,
        })
    );
}

#[test]
fn test_limit_nofile_with_other_settings() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    Restart = always
    LimitNOFILE = 8192
    KillMode = process
    TasksMax = 512
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(service.srvc.restart, crate::units::ServiceRestart::Always);
    assert_eq!(
        service.srvc.limit_nofile,
        Some(crate::units::ResourceLimit {
            soft: crate::units::RLimitValue::Value(8192),
            hard: crate::units::RLimitValue::Value(8192),
        })
    );
    assert_eq!(service.srvc.kill_mode, crate::units::KillMode::Process);
    assert_eq!(
        service.srvc.tasks_max,
        Some(crate::units::TasksMax::Value(512))
    );
}

#[test]
fn test_limit_nofile_value_one() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    LimitNOFILE = 1
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.limit_nofile,
        Some(crate::units::ResourceLimit {
            soft: crate::units::RLimitValue::Value(1),
            hard: crate::units::RLimitValue::Value(1),
        })
    );
}

#[test]
fn test_limit_nofile_both_infinity() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    LimitNOFILE = infinity:infinity
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.limit_nofile,
        Some(crate::units::ResourceLimit {
            soft: crate::units::RLimitValue::Infinity,
            hard: crate::units::RLimitValue::Infinity,
        })
    );
}

#[test]
fn test_at_prefix_execstart() {
    let test_service_str = r#"
    [Service]
    ExecStart = @/usr/bin/foo bar arg1 arg2
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.exec,
        Some(crate::units::Commandline {
            cmd: "/usr/bin/foo".into(),
            args: vec!["bar".into(), "arg1".into(), "arg2".into()],
            prefixes: vec![crate::units::CommandlinePrefix::AtSign],
        })
    );
}

#[test]
fn test_at_prefix_execstartpre() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    ExecStartPre = @/usr/bin/setup mysetup --init
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.startpre,
        vec![crate::units::Commandline {
            cmd: "/usr/bin/setup".into(),
            args: vec!["mysetup".into(), "--init".into()],
            prefixes: vec![crate::units::CommandlinePrefix::AtSign],
        }]
    );
}

#[test]
fn test_at_prefix_combined_with_minus() {
    let test_service_str = r#"
    [Service]
    ExecStart = -@/usr/bin/foo bar arg1
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.exec,
        Some(crate::units::Commandline {
            cmd: "/usr/bin/foo".into(),
            args: vec!["bar".into(), "arg1".into()],
            prefixes: vec![
                crate::units::CommandlinePrefix::Minus,
                crate::units::CommandlinePrefix::AtSign,
            ],
        })
    );
}

#[test]
fn test_at_prefix_combined_minus_at() {
    // '@' before '-' should also work since prefixes can be in any order
    let test_service_str = r#"
    [Service]
    ExecStart = @-/usr/bin/foo bar arg1
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.exec,
        Some(crate::units::Commandline {
            cmd: "/usr/bin/foo".into(),
            args: vec!["bar".into(), "arg1".into()],
            prefixes: vec![
                crate::units::CommandlinePrefix::AtSign,
                crate::units::CommandlinePrefix::Minus,
            ],
        })
    );
}

#[test]
fn test_at_prefix_no_extra_args() {
    // With '@' prefix but only one arg (which becomes argv[0])
    let test_service_str = r#"
    [Service]
    ExecStart = @/usr/bin/foo bar
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.exec,
        Some(crate::units::Commandline {
            cmd: "/usr/bin/foo".into(),
            args: vec!["bar".into()],
            prefixes: vec![crate::units::CommandlinePrefix::AtSign],
        })
    );
}

#[test]
fn test_at_prefix_execstop() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    ExecStop = @/usr/bin/stopper mystop --graceful
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.stop,
        vec![crate::units::Commandline {
            cmd: "/usr/bin/stopper".into(),
            args: vec!["mystop".into(), "--graceful".into()],
            prefixes: vec![crate::units::CommandlinePrefix::AtSign],
        }]
    );
}

#[test]
fn test_at_prefix_execstoppost() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    ExecStopPost = @/usr/bin/cleanup mycleanup
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.stoppost,
        vec![crate::units::Commandline {
            cmd: "/usr/bin/cleanup".into(),
            args: vec!["mycleanup".into()],
            prefixes: vec![crate::units::CommandlinePrefix::AtSign],
        }]
    );
}

#[test]
fn test_at_prefix_execstartpost() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    ExecStartPost = @/usr/bin/notify mynotify --ready
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.startpost,
        vec![crate::units::Commandline {
            cmd: "/usr/bin/notify".into(),
            args: vec!["mynotify".into(), "--ready".into()],
            prefixes: vec![crate::units::CommandlinePrefix::AtSign],
        }]
    );
}

#[test]
fn test_environment_file_empty_by_default() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert!(service.srvc.exec_section.environment_files.is_empty());
}

#[test]
fn test_environment_file_single() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    EnvironmentFile = /etc/default/myservice
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.exec_section.environment_files,
        vec![(std::path::PathBuf::from("/etc/default/myservice"), false)]
    );
}

#[test]
fn test_environment_file_optional_dash_prefix() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    EnvironmentFile = -/etc/default/myservice
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.exec_section.environment_files,
        vec![(std::path::PathBuf::from("/etc/default/myservice"), true)]
    );
}

#[test]
fn test_environment_file_multiple() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    EnvironmentFile = /etc/default/myservice
    EnvironmentFile = -/etc/sysconfig/myservice
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.exec_section.environment_files,
        vec![
            (std::path::PathBuf::from("/etc/default/myservice"), false),
            (std::path::PathBuf::from("/etc/sysconfig/myservice"), true),
        ]
    );
}

#[test]
fn test_environment_file_with_environment() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    Environment = FOO=bar
    EnvironmentFile = /etc/default/myservice
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    // Both Environment= and EnvironmentFile= should be present
    assert!(service.srvc.exec_section.environment.is_some());
    let env = service.srvc.exec_section.environment.as_ref().unwrap();
    assert_eq!(env.vars, vec![("FOO".to_owned(), "bar".to_owned())]);

    assert_eq!(
        service.srvc.exec_section.environment_files,
        vec![(std::path::PathBuf::from("/etc/default/myservice"), false)]
    );
}

#[test]
fn test_environment_file_nix_store_path() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    EnvironmentFile = -/nix/store/abc123-env/lib/env
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.exec_section.environment_files,
        vec![(
            std::path::PathBuf::from("/nix/store/abc123-env/lib/env"),
            true
        )]
    );
}

#[test]
fn test_environment_file_socket_unit() {
    let test_socket_str = r#"
    [Socket]
    ListenStream = /run/test.sock
    EnvironmentFile = /etc/default/mysocket
    "#;

    let parsed_file = crate::units::parse_file(test_socket_str).unwrap();
    let socket = crate::units::parse_socket(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.socket"),
    )
    .unwrap();

    assert_eq!(
        socket.sock.exec_section.environment_files,
        vec![(std::path::PathBuf::from("/etc/default/mysocket"), false)]
    );
}

#[test]
fn test_environment_path_with_equals_in_value() {
    // PATH values contain colons AND the split on '=' must only split on the
    // first '=', otherwise everything after the second '=' is dropped.
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    Environment = PATH=/nix/store/abc-iptables/bin:/nix/store/def-ip6tables/bin:/usr/bin
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    let env = service.srvc.exec_section.environment.as_ref().unwrap();
    assert_eq!(
        env.vars,
        vec![(
            "PATH".to_owned(),
            "/nix/store/abc-iptables/bin:/nix/store/def-ip6tables/bin:/usr/bin".to_owned()
        )]
    );
}

#[test]
fn test_environment_value_with_embedded_equals() {
    // Values like JAVA_OPTS="-Dfoo=bar -Dbaz=qux" contain '=' in the value
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    Environment = "JAVA_OPTS=-Dfoo=bar -Dbaz=qux"
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    let env = service.srvc.exec_section.environment.as_ref().unwrap();
    assert_eq!(
        env.vars,
        vec![("JAVA_OPTS".to_owned(), "-Dfoo=bar -Dbaz=qux".to_owned())]
    );
}

#[test]
fn test_environment_multiple_vars_with_equals() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    Environment = "FOO=a=b" "BAR=c=d=e"
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    let env = service.srvc.exec_section.environment.as_ref().unwrap();
    assert_eq!(
        env.vars,
        vec![
            ("FOO".to_owned(), "a=b".to_owned()),
            ("BAR".to_owned(), "c=d=e".to_owned()),
        ]
    );
}

#[test]
fn test_environment_empty_value() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    Environment = "EMPTY="
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    let env = service.srvc.exec_section.environment.as_ref().unwrap();
    assert_eq!(env.vars, vec![("EMPTY".to_owned(), "".to_owned())]);
}

#[test]
fn test_environment_multiple_lines() {
    // NixOS generates one Environment= line per variable.
    // All lines must be parsed, not just the first one.
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    Environment = "LOCALE_ARCHIVE=/nix/store/abc-glibc-locales/lib/locale/locale-archive"
    Environment = "PATH=/nix/store/def-iptables/bin:/nix/store/ghi-coreutils/bin"
    Environment = "TZDIR=/nix/store/jkl-tzdata/share/zoneinfo"
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    let env = service.srvc.exec_section.environment.as_ref().unwrap();
    assert_eq!(
        env.vars,
        vec![
            (
                "LOCALE_ARCHIVE".to_owned(),
                "/nix/store/abc-glibc-locales/lib/locale/locale-archive".to_owned()
            ),
            (
                "PATH".to_owned(),
                "/nix/store/def-iptables/bin:/nix/store/ghi-coreutils/bin".to_owned()
            ),
            (
                "TZDIR".to_owned(),
                "/nix/store/jkl-tzdata/share/zoneinfo".to_owned()
            ),
        ]
    );
}

#[test]
fn test_environment_nixos_firewall_pattern() {
    // Matches the exact pattern NixOS generates for firewall.service:
    // multiple Environment= lines with quoted values, ExecStart with @ prefix,
    // and a PATH containing multiple nix store paths separated by colons.
    let test_service_str = r#"
    [Unit]
    After=systemd-modules-load.service
    Before=network-pre.target shutdown.target
    Description=Firewall

    [Service]
    Environment="LOCALE_ARCHIVE=/nix/store/abc-glibc-locales/lib/locale/locale-archive"
    Environment="PATH=/nix/store/def-iptables-1.8.11/bin:/nix/store/ghi-coreutils-9.8/bin:/nix/store/jkl-findutils/bin:/nix/store/def-iptables-1.8.11/sbin:/nix/store/ghi-coreutils-9.8/sbin"
    Environment="TZDIR=/nix/store/mno-tzdata/share/zoneinfo"
    ExecStart=@/nix/store/pqr-firewall-start/bin/firewall-start firewall-start
    Type=oneshot
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/firewall.service"),
    )
    .unwrap();

    // All three Environment= lines must be parsed
    let env = service.srvc.exec_section.environment.as_ref().unwrap();
    assert_eq!(env.vars.len(), 3);

    // PATH must contain the full value with all colon-separated store paths
    let path_val = env
        .vars
        .iter()
        .find(|(k, _)| k == "PATH")
        .map(|(_, v)| v.as_str())
        .unwrap();
    assert!(
        path_val.contains("/nix/store/def-iptables-1.8.11/bin"),
        "PATH must contain iptables bin dir"
    );
    assert!(
        path_val.contains("/nix/store/def-iptables-1.8.11/sbin"),
        "PATH must contain iptables sbin dir (ip6tables lives here too)"
    );
    assert!(
        path_val.contains("/nix/store/ghi-coreutils-9.8/bin"),
        "PATH must contain coreutils bin dir"
    );

    // ExecStart must parse the @ prefix correctly
    let exec = service.srvc.exec.as_ref().expect("ExecStart should be set");
    assert_eq!(exec.prefixes, vec![crate::units::CommandlinePrefix::AtSign]);
    assert_eq!(exec.cmd, "/nix/store/pqr-firewall-start/bin/firewall-start");
    assert_eq!(exec.args, vec!["firewall-start".to_owned()]);
}

#[test]
fn test_environment_multiple_lines_same_key_last_wins() {
    // If the same key appears in separate Environment= lines, the last value wins
    // (systemd behavior: later assignments override earlier ones for the same key).
    // Note: our parser currently collects all entries; the dedup happens at
    // start_service time. Here we verify all entries are collected.
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    Environment = "FOO=first"
    Environment = "FOO=second"
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    let env = service.srvc.exec_section.environment.as_ref().unwrap();
    // Both entries should be collected; start_service applies last-wins semantics
    assert_eq!(
        env.vars,
        vec![
            ("FOO".to_owned(), "first".to_owned()),
            ("FOO".to_owned(), "second".to_owned()),
        ]
    );
}

#[test]
fn test_environment_single_line_multiple_vars() {
    // systemd also supports multiple KEY=VALUE pairs on a single Environment= line
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    Environment = "FOO=bar" "BAZ=qux"
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    let env = service.srvc.exec_section.environment.as_ref().unwrap();
    assert_eq!(
        env.vars,
        vec![
            ("FOO".to_owned(), "bar".to_owned()),
            ("BAZ".to_owned(), "qux".to_owned()),
        ]
    );
}

#[test]
fn test_service_no_service_section() {
    // systemd allows .service files without a [Service] section (e.g.
    // systemd-reboot.service which only has a [Unit] with SuccessAction=).
    // rustysd should treat these as oneshot services with no ExecStart.
    let test_service_str = r#"
    [Unit]
    Description=System Reboot
    DefaultDependencies=no
    Requires=shutdown.target
    After=shutdown.target
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/systemd-reboot.service"),
    )
    .unwrap();

    assert_eq!(service.srvc.exec, None);
    assert_eq!(service.srvc.srcv_type, crate::units::ServiceType::OneShot);
    assert_eq!(service.common.unit.description, "System Reboot".to_owned());
    assert!(!service.common.unit.default_dependencies);
}

#[test]
fn test_service_oneshot_no_execstart() {
    // A [Service] section with Type=oneshot but no ExecStart= is valid in
    // systemd. The service succeeds immediately upon activation.
    let test_service_str = r#"
    [Unit]
    Description=Test Oneshot No Exec

    [Service]
    Type=oneshot
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test-oneshot.service"),
    )
    .unwrap();

    assert_eq!(service.srvc.exec, None);
    assert_eq!(service.srvc.srcv_type, crate::units::ServiceType::OneShot);
}

#[test]
fn test_service_no_service_section_defaults_to_oneshot() {
    // When there is no [Service] section and no Type= is given, the service
    // type should default to oneshot (since there is no ExecStart).
    let test_service_str = r#"
    [Unit]
    Description=Minimal unit-only service
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/minimal.service"),
    )
    .unwrap();

    assert_eq!(service.srvc.exec, None);
    assert_eq!(service.srvc.srcv_type, crate::units::ServiceType::OneShot);
    assert_eq!(service.srvc.restart, crate::units::ServiceRestart::No);
}

#[test]
fn test_service_implicit_oneshot_with_pre_post_commands() {
    // A service without ExecStart but with ExecStartPre/ExecStartPost should
    // still parse successfully as a oneshot with no main exec.
    let test_service_str = r#"
    [Unit]
    Description=Service with hooks only

    [Service]
    ExecStartPre=/usr/bin/echo pre
    ExecStartPost=/usr/bin/echo post
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/hooks-only.service"),
    )
    .unwrap();

    assert_eq!(service.srvc.exec, None);
    assert_eq!(service.srvc.srcv_type, crate::units::ServiceType::OneShot);
    assert_eq!(service.srvc.startpre.len(), 1);
    assert_eq!(service.srvc.startpre[0].cmd, "/usr/bin/echo");
    assert_eq!(service.srvc.startpost.len(), 1);
    assert_eq!(service.srvc.startpost[0].cmd, "/usr/bin/echo");
}

/// User= and Group= resolution is deferred to exec time (matching systemd
/// behavior). Parsing a service that references a user/group which does not
/// exist on the current system must succeed — the raw string is stored and
/// resolved later when the service is actually started.
#[test]
fn test_user_group_deferred_resolution_unknown_user() {
    let test_service_str = r#"
    [Service]
    ExecStart = /usr/bin/daemon
    User = nonexistent_test_user_42
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/daemon.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.exec_section.user,
        Some("nonexistent_test_user_42".to_owned()),
        "User= should be stored as a raw string, not resolved at parse time"
    );
}

#[test]
fn test_user_group_deferred_resolution_unknown_group() {
    let test_service_str = r#"
    [Service]
    ExecStart = /usr/bin/daemon
    Group = nonexistent_test_group_42
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/daemon.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.exec_section.group,
        Some("nonexistent_test_group_42".to_owned()),
        "Group= should be stored as a raw string, not resolved at parse time"
    );
}

#[test]
fn test_user_group_deferred_resolution_unknown_both() {
    let test_service_str = r#"
    [Service]
    ExecStart = /usr/bin/uuidd
    User = uuidd
    Group = uuidd
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/uuidd.service"),
    )
    .unwrap();

    assert_eq!(service.srvc.exec_section.user, Some("uuidd".to_owned()),);
    assert_eq!(service.srvc.exec_section.group, Some("uuidd".to_owned()),);
}

#[test]
fn test_user_group_numeric_uid_gid() {
    let test_service_str = r#"
    [Service]
    ExecStart = /usr/bin/daemon
    User = 1000
    Group = 1000
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/daemon.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.exec_section.user,
        Some("1000".to_owned()),
        "Numeric UID should be stored as a raw string"
    );
    assert_eq!(
        service.srvc.exec_section.group,
        Some("1000".to_owned()),
        "Numeric GID should be stored as a raw string"
    );
}

#[test]
fn test_user_group_not_set_defaults_to_none() {
    let test_service_str = r#"
    [Service]
    ExecStart = /usr/bin/daemon
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/daemon.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.exec_section.user, None,
        "User= not set should result in None"
    );
    assert_eq!(
        service.srvc.exec_section.group, None,
        "Group= not set should result in None"
    );
}

#[test]
fn test_supplementary_groups_deferred_resolution() {
    let test_service_str = r#"
    [Service]
    ExecStart = /usr/bin/daemon
    SupplementaryGroups = audio video nonexistent_group_99
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/daemon.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.exec_section.supplementary_groups,
        vec![
            "audio".to_owned(),
            "video".to_owned(),
            "nonexistent_group_99".to_owned(),
        ],
        "SupplementaryGroups= should be stored as raw strings, not resolved at parse time"
    );
}

/// Converting a parsed service with an unknown user/group into a Unit must
/// also succeed — the TryFrom<ParsedExecSection> no longer resolves names.
#[test]
fn test_user_group_deferred_through_unit_conversion() {
    use crate::units::Unit;
    use std::convert::TryInto;

    let test_service_str = r#"
    [Unit]
    Description=UUID daemon
    [Service]
    ExecStart = /usr/bin/uuidd
    User = uuidd
    Group = uuidd
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/uuidd.service"),
    )
    .unwrap();

    // This previously failed with "Couldnt get uid for username: uuidd"
    let unit: Unit = service.try_into().expect(
        "Unit conversion must succeed even when User=/Group= reference unknown users/groups",
    );

    if let crate::units::Specific::Service(ref srvc) = unit.specific {
        assert_eq!(srvc.conf.exec_config.user, Some("uuidd".to_owned()));
        assert_eq!(srvc.conf.exec_config.group, Some("uuidd".to_owned()));
    } else {
        panic!("Expected a Service unit");
    }
}

#[test]
fn test_numeric_uid_gid_through_unit_conversion() {
    use crate::units::Unit;
    use std::convert::TryInto;

    let test_service_str = r#"
    [Service]
    ExecStart = /usr/bin/daemon
    User = 65534
    Group = 65534
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/daemon.service"),
    )
    .unwrap();

    let unit: Unit = service.try_into().unwrap();

    if let crate::units::Specific::Service(ref srvc) = unit.specific {
        assert_eq!(srvc.conf.exec_config.user, Some("65534".to_owned()));
        assert_eq!(srvc.conf.exec_config.group, Some("65534".to_owned()));
    } else {
        panic!("Expected a Service unit");
    }
}

/// Exec-less oneshot service (no [Service] section) should convert to a
/// valid Unit with exec: None.
#[test]
fn test_no_service_section_through_unit_conversion() {
    use crate::units::Unit;
    use std::convert::TryInto;

    let test_service_str = r#"
    [Unit]
    Description=System Reboot
    DefaultDependencies=no
    Requires=shutdown.target
    After=shutdown.target
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/systemd-reboot.service"),
    )
    .unwrap();

    let unit: Unit = service.try_into().unwrap();

    assert_eq!(unit.common.unit.description, "System Reboot");
    if let crate::units::Specific::Service(ref srvc) = unit.specific {
        assert_eq!(srvc.conf.exec, None);
        assert_eq!(srvc.conf.srcv_type, crate::units::ServiceType::OneShot);
        assert_eq!(srvc.conf.exec_config.user, None);
        assert_eq!(srvc.conf.exec_config.group, None);
    } else {
        panic!("Expected a Service unit");
    }
}

#[test]
fn test_resolve_uid_numeric() {
    let result = crate::services::start_service::resolve_uid(&Some("0".to_owned()));
    assert_eq!(result.unwrap(), 0, "Numeric UID 0 (root) should resolve");
}

#[test]
fn test_resolve_uid_none_returns_current() {
    let result = crate::services::start_service::resolve_uid(&None);
    assert!(result.is_ok(), "None user should fall back to current UID");
    assert_eq!(
        result.unwrap(),
        nix::unistd::getuid().as_raw(),
        "None user should resolve to current process UID"
    );
}

#[test]
fn test_resolve_uid_unknown_user_fails() {
    let result =
        crate::services::start_service::resolve_uid(&Some("nonexistent_user_xyz_42".to_owned()));
    assert!(
        result.is_err(),
        "Unknown username should fail at resolve time"
    );
}

#[test]
fn test_resolve_gid_numeric() {
    let result = crate::services::start_service::resolve_gid(&Some("0".to_owned()));
    assert_eq!(result.unwrap(), 0, "Numeric GID 0 (root) should resolve");
}

#[test]
fn test_resolve_gid_none_returns_current() {
    let result = crate::services::start_service::resolve_gid(&None);
    assert!(result.is_ok(), "None group should fall back to current GID");
    assert_eq!(
        result.unwrap(),
        nix::unistd::getgid().as_raw(),
        "None group should resolve to current process GID"
    );
}

#[test]
fn test_resolve_gid_unknown_group_fails() {
    let result =
        crate::services::start_service::resolve_gid(&Some("nonexistent_group_xyz_42".to_owned()));
    assert!(
        result.is_err(),
        "Unknown group name should fail at resolve time"
    );
}

/// Regression test: commas in ExecStart command arguments must not be treated
/// as value separators. This mirrors the real systemd-udev-trigger.service
/// which uses `--prioritized-subsystem=module,block,tpmrm,net,tty,input`.
#[test]
fn test_execstart_commas_in_arguments_not_split() {
    let test_service_str = r#"
    [Unit]
    Description=Coldplug All udev Devices
    Documentation=man:udev(7) man:systemd-udevd.service(8)
    DefaultDependencies=no
    Wants=systemd-udevd.service
    After=systemd-udevd-kernel.socket systemd-udevd-control.socket
    Before=sysinit.target

    [Service]
    Type=oneshot
    ExecStart=-udevadm trigger --type=all --action=add --prioritized-subsystem=module,block,tpmrm,net,tty,input
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/etc/systemd/system/systemd-udev-trigger.service"),
    )
    .unwrap();

    // The ExecStart must parse as a single command line, not be rejected as
    // SettingTooManyValues.
    let exec = service
        .srvc
        .exec
        .as_ref()
        .expect("ExecStart should be parsed");
    assert_eq!(exec.cmd, "udevadm");
    assert_eq!(
        exec.args,
        vec![
            "trigger".to_owned(),
            "--type=all".to_owned(),
            "--action=add".to_owned(),
            "--prioritized-subsystem=module,block,tpmrm,net,tty,input".to_owned(),
        ]
    );
    // The leading '-' should be parsed as a Minus prefix
    assert_eq!(exec.prefixes, vec![crate::units::CommandlinePrefix::Minus]);
    assert_eq!(service.srvc.srcv_type, crate::units::ServiceType::OneShot);
}

/// Verify that commas in Environment= values are preserved and not split.
#[test]
fn test_environment_commas_in_values_preserved() {
    let test_service_str = r#"
    [Service]
    Type=simple
    ExecStart=/bin/true
    Environment=OPTS=--flag=a,b,c
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.service"),
    )
    .unwrap();

    let env = service
        .srvc
        .exec_section
        .environment
        .expect("should have env");
    assert_eq!(env.vars.len(), 1);
    assert_eq!(env.vars[0].0, "OPTS");
    assert_eq!(env.vars[0].1, "--flag=a,b,c");
}

/// Verify that Documentation= with space-separated URIs on a single line
/// is correctly split (systemd uses space-separated lists for this field).
#[test]
fn test_documentation_space_separated_single_line() {
    let test_service_str = r#"
    [Unit]
    Documentation=man:udev(7) man:systemd-udevd.service(8)
    [Service]
    ExecStart=/bin/true
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.service"),
    )
    .unwrap();

    assert_eq!(
        service.common.unit.documentation,
        vec![
            "man:udev(7)".to_owned(),
            "man:systemd-udevd.service(8)".to_owned(),
        ]
    );
}
