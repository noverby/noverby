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
        crate::units::Commandline {
            cmd: "/path/to/startbin".into(),
            args: vec!["arg1".into(), "arg2".into(), "arg3".into()],
            prefixes: vec![],
        }
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
        crate::units::Commandline {
            cmd: "/usr/bin/foo".into(),
            args: vec!["bar".into(), "arg1".into(), "arg2".into()],
            prefixes: vec![crate::units::CommandlinePrefix::AtSign],
        }
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
        crate::units::Commandline {
            cmd: "/usr/bin/foo".into(),
            args: vec!["bar".into(), "arg1".into()],
            prefixes: vec![
                crate::units::CommandlinePrefix::Minus,
                crate::units::CommandlinePrefix::AtSign,
            ],
        }
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
        crate::units::Commandline {
            cmd: "/usr/bin/foo".into(),
            args: vec!["bar".into(), "arg1".into()],
            prefixes: vec![
                crate::units::CommandlinePrefix::AtSign,
                crate::units::CommandlinePrefix::Minus,
            ],
        }
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
        crate::units::Commandline {
            cmd: "/usr/bin/foo".into(),
            args: vec!["bar".into()],
            prefixes: vec![crate::units::CommandlinePrefix::AtSign],
        }
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
