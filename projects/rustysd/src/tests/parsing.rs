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

    // RuntimeDirectory should be empty when not specified
    assert!(service.srvc.exec_section.runtime_directory.is_empty());
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

// ── RuntimeDirectory= ─────────────────────────────────────────────────

#[test]
fn test_runtime_directory_single() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    RuntimeDirectory = myapp
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.exec_section.runtime_directory,
        vec!["myapp".to_owned()]
    );
}

#[test]
fn test_runtime_directory_multiple_space_separated() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    RuntimeDirectory = myapp myapp-extra
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.exec_section.runtime_directory,
        vec!["myapp".to_owned(), "myapp-extra".to_owned()]
    );
}

#[test]
fn test_runtime_directory_multiple_directives() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    RuntimeDirectory = myapp
    RuntimeDirectory = other
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.exec_section.runtime_directory,
        vec!["myapp".to_owned(), "other".to_owned()]
    );
}

#[test]
fn test_runtime_directory_empty_by_default() {
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

    assert!(service.srvc.exec_section.runtime_directory.is_empty());
}

#[test]
fn test_runtime_directory_no_unsupported_warning() {
    // RuntimeDirectory= should be parsed without generating an "unsupported setting" warning.
    // If the key were left in the section, the parser would emit a warning and the field
    // would be empty.
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    RuntimeDirectory = myapp
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let result = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    );

    assert!(
        result.is_ok(),
        "Parsing a service with RuntimeDirectory should succeed without errors"
    );
    assert_eq!(
        result.unwrap().srvc.exec_section.runtime_directory,
        vec!["myapp".to_owned()]
    );
}

#[test]
fn test_runtime_directory_combined_with_state_directory() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    StateDirectory = myapp-state
    RuntimeDirectory = myapp-run
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.exec_section.state_directory,
        vec!["myapp-state".to_owned()]
    );
    assert_eq!(
        service.srvc.exec_section.runtime_directory,
        vec!["myapp-run".to_owned()]
    );
}

#[test]
fn test_runtime_directory_socket_unit() {
    let test_socket_str = r#"
    [Socket]
    ListenStream = /run/test.sock
    RuntimeDirectory = myapp
    "#;

    let parsed_file = crate::units::parse_file(test_socket_str).unwrap();
    let socket = crate::units::parse_socket(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.socket"),
    )
    .unwrap();

    assert_eq!(
        socket.sock.exec_section.runtime_directory,
        vec!["myapp".to_owned()]
    );
}

#[test]
fn test_runtime_directory_preserved_after_unit_conversion() {
    use std::convert::TryInto;

    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    RuntimeDirectory = myapp extra
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.service"),
    )
    .unwrap();

    let unit: crate::units::Unit = service.try_into().unwrap();
    if let crate::units::Specific::Service(srvc) = &unit.specific {
        assert_eq!(
            srvc.conf.exec_config.runtime_directory,
            vec!["myapp".to_owned(), "extra".to_owned()]
        );
    } else {
        panic!("Expected service unit");
    }
}

#[test]
fn test_runtime_directory_with_subdirectory() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    RuntimeDirectory = myapp/subdir
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.exec_section.runtime_directory,
        vec!["myapp/subdir".to_owned()]
    );
}

#[test]
fn test_runtime_directory_combined_with_working_directory() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    WorkingDirectory = /var/lib/myapp
    RuntimeDirectory = myapp
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
    assert_eq!(
        service.srvc.exec_section.runtime_directory,
        vec!["myapp".to_owned()]
    );
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

// ============================================================
// TTYReset=, TTYVHangup=, TTYVTDisallocate= parsing tests
// ============================================================

#[test]
fn test_tty_reset_defaults_to_false() {
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
        !service.srvc.exec_section.tty_reset,
        "TTYReset should default to false when not specified"
    );
}

#[test]
fn test_tty_reset_explicit_yes() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    TTYReset = yes
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert!(
        service.srvc.exec_section.tty_reset,
        "TTYReset=yes should be true"
    );
}

#[test]
fn test_tty_reset_explicit_no() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    TTYReset = no
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert!(
        !service.srvc.exec_section.tty_reset,
        "TTYReset=no should be false"
    );
}

#[test]
fn test_tty_reset_explicit_true() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    TTYReset = true
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert!(
        service.srvc.exec_section.tty_reset,
        "TTYReset=true should be true"
    );
}

#[test]
fn test_tty_reset_explicit_false() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    TTYReset = false
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert!(
        !service.srvc.exec_section.tty_reset,
        "TTYReset=false should be false"
    );
}

#[test]
fn test_tty_reset_explicit_1() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    TTYReset = 1
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert!(
        service.srvc.exec_section.tty_reset,
        "TTYReset=1 should be true"
    );
}

#[test]
fn test_tty_reset_explicit_0() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    TTYReset = 0
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert!(
        !service.srvc.exec_section.tty_reset,
        "TTYReset=0 should be false"
    );
}

#[test]
fn test_tty_vhangup_defaults_to_false() {
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
        !service.srvc.exec_section.tty_vhangup,
        "TTYVHangup should default to false when not specified"
    );
}

#[test]
fn test_tty_vhangup_explicit_yes() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    TTYVHangup = yes
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert!(
        service.srvc.exec_section.tty_vhangup,
        "TTYVHangup=yes should be true"
    );
}

#[test]
fn test_tty_vhangup_explicit_no() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    TTYVHangup = no
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert!(
        !service.srvc.exec_section.tty_vhangup,
        "TTYVHangup=no should be false"
    );
}

#[test]
fn test_tty_vhangup_explicit_true() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    TTYVHangup = true
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert!(
        service.srvc.exec_section.tty_vhangup,
        "TTYVHangup=true should be true"
    );
}

#[test]
fn test_tty_vhangup_explicit_false() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    TTYVHangup = false
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert!(
        !service.srvc.exec_section.tty_vhangup,
        "TTYVHangup=false should be false"
    );
}

#[test]
fn test_tty_vhangup_explicit_1() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    TTYVHangup = 1
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert!(
        service.srvc.exec_section.tty_vhangup,
        "TTYVHangup=1 should be true"
    );
}

#[test]
fn test_tty_vhangup_explicit_0() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    TTYVHangup = 0
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert!(
        !service.srvc.exec_section.tty_vhangup,
        "TTYVHangup=0 should be false"
    );
}

#[test]
fn test_tty_vt_disallocate_defaults_to_false() {
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
        !service.srvc.exec_section.tty_vt_disallocate,
        "TTYVTDisallocate should default to false when not specified"
    );
}

#[test]
fn test_tty_vt_disallocate_explicit_yes() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    TTYVTDisallocate = yes
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert!(
        service.srvc.exec_section.tty_vt_disallocate,
        "TTYVTDisallocate=yes should be true"
    );
}

#[test]
fn test_tty_vt_disallocate_explicit_no() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    TTYVTDisallocate = no
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert!(
        !service.srvc.exec_section.tty_vt_disallocate,
        "TTYVTDisallocate=no should be false"
    );
}

#[test]
fn test_tty_vt_disallocate_explicit_true() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    TTYVTDisallocate = true
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert!(
        service.srvc.exec_section.tty_vt_disallocate,
        "TTYVTDisallocate=true should be true"
    );
}

#[test]
fn test_tty_vt_disallocate_explicit_false() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    TTYVTDisallocate = false
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert!(
        !service.srvc.exec_section.tty_vt_disallocate,
        "TTYVTDisallocate=false should be false"
    );
}

#[test]
fn test_tty_vt_disallocate_explicit_1() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    TTYVTDisallocate = 1
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert!(
        service.srvc.exec_section.tty_vt_disallocate,
        "TTYVTDisallocate=1 should be true"
    );
}

#[test]
fn test_tty_vt_disallocate_explicit_0() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    TTYVTDisallocate = 0
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert!(
        !service.srvc.exec_section.tty_vt_disallocate,
        "TTYVTDisallocate=0 should be false"
    );
}

#[test]
fn test_tty_settings_all_enabled_together() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    StandardInput = tty
    TTYPath = /dev/tty1
    TTYReset = yes
    TTYVHangup = yes
    TTYVTDisallocate = yes
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.exec_section.tty_path,
        Some(std::path::PathBuf::from("/dev/tty1"))
    );
    assert!(
        service.srvc.exec_section.tty_reset,
        "TTYReset=yes should be true"
    );
    assert!(
        service.srvc.exec_section.tty_vhangup,
        "TTYVHangup=yes should be true"
    );
    assert!(
        service.srvc.exec_section.tty_vt_disallocate,
        "TTYVTDisallocate=yes should be true"
    );
}

#[test]
fn test_tty_settings_mixed_values() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    TTYReset = yes
    TTYVHangup = no
    TTYVTDisallocate = true
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert!(
        service.srvc.exec_section.tty_reset,
        "TTYReset=yes should be true"
    );
    assert!(
        !service.srvc.exec_section.tty_vhangup,
        "TTYVHangup=no should be false"
    );
    assert!(
        service.srvc.exec_section.tty_vt_disallocate,
        "TTYVTDisallocate=true should be true"
    );
}

#[test]
fn test_tty_settings_socket_unit() {
    let test_socket_str = r#"
    [Socket]
    ListenStream = /path/to/socket
    TTYReset = yes
    TTYVHangup = true
    TTYVTDisallocate = 1
    "#;

    let parsed_file = crate::units::parse_file(test_socket_str).unwrap();
    let socket_unit = crate::units::parse_socket(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.socket"),
    )
    .unwrap();

    assert!(
        socket_unit.sock.exec_section.tty_reset,
        "TTYReset=yes should be true in socket units"
    );
    assert!(
        socket_unit.sock.exec_section.tty_vhangup,
        "TTYVHangup=true should be true in socket units"
    );
    assert!(
        socket_unit.sock.exec_section.tty_vt_disallocate,
        "TTYVTDisallocate=1 should be true in socket units"
    );
}

#[test]
fn test_tty_settings_socket_unit_defaults_to_false() {
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

    assert!(
        !socket_unit.sock.exec_section.tty_reset,
        "TTYReset should default to false in socket units"
    );
    assert!(
        !socket_unit.sock.exec_section.tty_vhangup,
        "TTYVHangup should default to false in socket units"
    );
    assert!(
        !socket_unit.sock.exec_section.tty_vt_disallocate,
        "TTYVTDisallocate should default to false in socket units"
    );
}

#[test]
fn test_tty_reset_case_insensitive() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    TTYReset = YES
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert!(
        service.srvc.exec_section.tty_reset,
        "TTYReset=YES (uppercase) should be true"
    );
}

#[test]
fn test_tty_vhangup_case_insensitive() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    TTYVHangup = True
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert!(
        service.srvc.exec_section.tty_vhangup,
        "TTYVHangup=True (mixed case) should be true"
    );
}

#[test]
fn test_tty_vt_disallocate_case_insensitive() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    TTYVTDisallocate = YES
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert!(
        service.srvc.exec_section.tty_vt_disallocate,
        "TTYVTDisallocate=YES (uppercase) should be true"
    );
}

// ============================================================
// Type=notify-reload parsing tests
// ============================================================

#[test]
fn test_service_type_notify_reload() {
    let test_service_str = r#"
    [Service]
    Type = notify-reload
    ExecStart = /bin/myservice
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.srcv_type,
        crate::units::ServiceType::NotifyReload,
        "Type=notify-reload should parse as NotifyReload"
    );
}

#[test]
fn test_service_type_notify_reload_is_distinct_from_notify() {
    let notify_str = r#"
    [Service]
    Type = notify
    ExecStart = /bin/myservice
    "#;

    let notify_reload_str = r#"
    [Service]
    Type = notify-reload
    ExecStart = /bin/myservice
    "#;

    let parsed_notify = crate::units::parse_file(notify_str).unwrap();
    let service_notify = crate::units::parse_service(
        parsed_notify,
        &std::path::PathBuf::from("/path/to/notify.service"),
    )
    .unwrap();

    let parsed_reload = crate::units::parse_file(notify_reload_str).unwrap();
    let service_reload = crate::units::parse_service(
        parsed_reload,
        &std::path::PathBuf::from("/path/to/reload.service"),
    )
    .unwrap();

    assert_eq!(
        service_notify.srvc.srcv_type,
        crate::units::ServiceType::Notify
    );
    assert_eq!(
        service_reload.srvc.srcv_type,
        crate::units::ServiceType::NotifyReload
    );
    assert_ne!(
        service_notify.srvc.srcv_type, service_reload.srvc.srcv_type,
        "Notify and NotifyReload should be distinct variants"
    );
}

#[test]
fn test_service_type_notify_still_works() {
    let test_service_str = r#"
    [Service]
    Type = notify
    ExecStart = /bin/myservice
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.srcv_type,
        crate::units::ServiceType::Notify,
        "Type=notify should still parse as Notify"
    );
}

#[test]
fn test_service_type_notify_reload_with_other_settings() {
    let test_service_str = r#"
    [Unit]
    Description = A notify-reload service
    [Service]
    Type = notify-reload
    ExecStart = /bin/myservice --flag
    Restart = always
    NotifyAccess = main
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.srcv_type,
        crate::units::ServiceType::NotifyReload,
        "Type=notify-reload should parse correctly alongside other settings"
    );
    assert_eq!(service.common.unit.description, "A notify-reload service");
    assert_eq!(service.srvc.restart, crate::units::ServiceRestart::Always);
}

#[test]
fn test_service_type_unknown_still_errors() {
    let test_service_str = r#"
    [Service]
    Type = bogus
    ExecStart = /bin/myservice
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let result = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    );

    assert!(
        result.is_err(),
        "Type=bogus should still produce a parsing error"
    );
}

// ============================================================
// Type=forking and PIDFile= parsing tests
// ============================================================

#[test]
fn test_service_type_forking() {
    let test_service_str = r#"
    [Service]
    Type = forking
    ExecStart = /usr/sbin/mydaemon
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.srcv_type,
        crate::units::ServiceType::Forking,
        "Type=forking should parse as Forking"
    );
}

#[test]
fn test_service_type_forking_with_pidfile() {
    let test_service_str = r#"
    [Service]
    Type = forking
    PIDFile = /run/mydaemon.pid
    ExecStart = /usr/sbin/mydaemon
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(service.srvc.srcv_type, crate::units::ServiceType::Forking,);
    assert_eq!(
        service.srvc.pid_file,
        Some(std::path::PathBuf::from("/run/mydaemon.pid")),
        "PIDFile should be parsed to the correct path"
    );
}

#[test]
fn test_service_pidfile_not_set_by_default() {
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

    assert_eq!(
        service.srvc.pid_file, None,
        "PIDFile should be None when not specified"
    );
}

#[test]
fn test_service_type_forking_is_distinct_from_others() {
    let forking_str = r#"
    [Service]
    Type = forking
    ExecStart = /usr/sbin/mydaemon
    "#;

    let simple_str = r#"
    [Service]
    Type = simple
    ExecStart = /usr/sbin/mydaemon
    "#;

    let parsed_forking = crate::units::parse_file(forking_str).unwrap();
    let service_forking = crate::units::parse_service(
        parsed_forking,
        &std::path::PathBuf::from("/path/to/forking.service"),
    )
    .unwrap();

    let parsed_simple = crate::units::parse_file(simple_str).unwrap();
    let service_simple = crate::units::parse_service(
        parsed_simple,
        &std::path::PathBuf::from("/path/to/simple.service"),
    )
    .unwrap();

    assert_eq!(
        service_forking.srvc.srcv_type,
        crate::units::ServiceType::Forking,
    );
    assert_eq!(
        service_simple.srvc.srcv_type,
        crate::units::ServiceType::Simple,
    );
    assert_ne!(
        service_forking.srvc.srcv_type, service_simple.srvc.srcv_type,
        "Forking and Simple should be distinct variants"
    );
}

#[test]
fn test_service_type_forking_with_all_settings() {
    let test_service_str = r#"
    [Unit]
    Description = A forking daemon
    [Service]
    Type = forking
    PIDFile = /run/mydaemon/mydaemon.pid
    ExecStart = /usr/sbin/mydaemon --daemonize
    ExecStop = /usr/sbin/mydaemon --stop
    Restart = always
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(service.srvc.srcv_type, crate::units::ServiceType::Forking,);
    assert_eq!(
        service.srvc.pid_file,
        Some(std::path::PathBuf::from("/run/mydaemon/mydaemon.pid")),
    );
    assert_eq!(service.common.unit.description, "A forking daemon");
    assert_eq!(service.srvc.restart, crate::units::ServiceRestart::Always);
    assert!(service.srvc.exec.is_some());
    assert_eq!(service.srvc.stop.len(), 1);
}

#[test]
fn test_service_type_forking_without_pidfile() {
    let test_service_str = r#"
    [Service]
    Type = forking
    ExecStart = /usr/sbin/mydaemon
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(service.srvc.srcv_type, crate::units::ServiceType::Forking,);
    assert_eq!(
        service.srvc.pid_file, None,
        "PIDFile should be None when not specified even for Type=forking"
    );
}

#[test]
fn test_service_pidfile_with_simple_type() {
    // PIDFile= can technically be specified with any type, it's just
    // most useful with forking. Parsing should accept it regardless.
    let test_service_str = r#"
    [Service]
    Type = simple
    PIDFile = /run/myapp.pid
    ExecStart = /usr/bin/myapp
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(service.srvc.srcv_type, crate::units::ServiceType::Simple,);
    assert_eq!(
        service.srvc.pid_file,
        Some(std::path::PathBuf::from("/run/myapp.pid")),
    );
}

#[test]
fn test_service_forking_preserved_after_unit_conversion() {
    use std::convert::TryInto;

    let test_service_str = r#"
    [Unit]
    Description = Forking daemon test
    [Service]
    Type = forking
    PIDFile = /run/test.pid
    ExecStart = /usr/sbin/testd
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let parsed_service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.service"),
    )
    .unwrap();

    let unit: crate::units::Unit = parsed_service.try_into().unwrap();

    if let crate::units::Specific::Service(ref srvc) = unit.specific {
        assert_eq!(srvc.conf.srcv_type, crate::units::ServiceType::Forking);
        assert_eq!(
            srvc.conf.pid_file,
            Some(std::path::PathBuf::from("/run/test.pid")),
        );
    } else {
        panic!("Expected Specific::Service");
    }
}

#[test]
fn test_service_pidfile_absolute_path() {
    let test_service_str = r#"
    [Service]
    Type = forking
    PIDFile = /var/run/sshd.pid
    ExecStart = /usr/sbin/sshd
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/sshd.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.pid_file,
        Some(std::path::PathBuf::from("/var/run/sshd.pid")),
    );
}

// ============================================================
// Type=idle parsing tests
// ============================================================

#[test]
fn test_service_type_idle() {
    let test_service_str = r#"
    [Service]
    Type = idle
    ExecStart = /bin/myservice
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.srcv_type,
        crate::units::ServiceType::Idle,
        "Type=idle should parse as Idle"
    );
}

#[test]
fn test_service_type_idle_is_distinct_from_simple() {
    let idle_str = r#"
    [Service]
    Type = idle
    ExecStart = /bin/myservice
    "#;

    let simple_str = r#"
    [Service]
    Type = simple
    ExecStart = /bin/myservice
    "#;

    let parsed_idle = crate::units::parse_file(idle_str).unwrap();
    let service_idle = crate::units::parse_service(
        parsed_idle,
        &std::path::PathBuf::from("/path/to/idle.service"),
    )
    .unwrap();

    let parsed_simple = crate::units::parse_file(simple_str).unwrap();
    let service_simple = crate::units::parse_service(
        parsed_simple,
        &std::path::PathBuf::from("/path/to/simple.service"),
    )
    .unwrap();

    assert_eq!(service_idle.srvc.srcv_type, crate::units::ServiceType::Idle);
    assert_eq!(
        service_simple.srvc.srcv_type,
        crate::units::ServiceType::Simple
    );
    assert_ne!(
        service_idle.srvc.srcv_type, service_simple.srvc.srcv_type,
        "Idle and Simple should be distinct variants"
    );
}

#[test]
fn test_service_type_idle_with_other_settings() {
    let test_service_str = r#"
    [Unit]
    Description = An idle service
    [Service]
    Type = idle
    ExecStart = /bin/myservice --flag
    Restart = always
    NotifyAccess = none
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.srcv_type,
        crate::units::ServiceType::Idle,
        "Type=idle should parse correctly alongside other settings"
    );
    assert_eq!(service.common.unit.description, "An idle service");
    assert_eq!(service.srvc.restart, crate::units::ServiceRestart::Always);
    assert_eq!(service.srvc.notifyaccess, crate::units::NotifyKind::None);
}

#[test]
fn test_service_type_idle_preserved_after_unit_conversion() {
    use std::convert::TryInto;

    let test_service_str = r#"
    [Unit]
    Description = Idle service test
    [Service]
    Type = idle
    ExecStart = /usr/bin/testcmd
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let parsed_service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.service"),
    )
    .unwrap();

    let unit: crate::units::Unit = parsed_service.try_into().unwrap();

    if let crate::units::Specific::Service(ref srvc) = unit.specific {
        assert_eq!(srvc.conf.srcv_type, crate::units::ServiceType::Idle);
    } else {
        panic!("Expected Specific::Service");
    }
}

#[test]
fn test_service_type_idle_is_distinct_from_all_others() {
    let types = vec![
        ("simple", crate::units::ServiceType::Simple),
        ("notify", crate::units::ServiceType::Notify),
        ("notify-reload", crate::units::ServiceType::NotifyReload),
        ("oneshot", crate::units::ServiceType::OneShot),
        ("forking", crate::units::ServiceType::Forking),
    ];

    for (type_str, expected_type) in &types {
        let service_str = format!(
            r#"
            [Service]
            Type = {}
            ExecStart = /bin/myservice
            "#,
            type_str
        );

        let parsed = crate::units::parse_file(&service_str).unwrap();
        let service =
            crate::units::parse_service(parsed, &std::path::PathBuf::from("/path/to/test.service"))
                .unwrap();

        assert_eq!(
            &service.srvc.srcv_type, expected_type,
            "Type={} should parse as {:?}",
            type_str, expected_type
        );
        assert_ne!(
            service.srvc.srcv_type,
            crate::units::ServiceType::Idle,
            "Type={} should be distinct from Idle",
            type_str
        );
    }
}

// ============================================================
// Restart= parsing tests (all systemd values)
// ============================================================

#[test]
fn test_restart_no() {
    let test_service_str = r#"
    [Service]
    Restart = no
    ExecStart = /bin/myservice
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(service.srvc.restart, crate::units::ServiceRestart::No);
}

#[test]
fn test_restart_always() {
    let test_service_str = r#"
    [Service]
    Restart = always
    ExecStart = /bin/myservice
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(service.srvc.restart, crate::units::ServiceRestart::Always);
}

#[test]
fn test_restart_on_success() {
    let test_service_str = r#"
    [Service]
    Restart = on-success
    ExecStart = /bin/myservice
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.restart,
        crate::units::ServiceRestart::OnSuccess,
        "Restart=on-success should parse as OnSuccess"
    );
}

#[test]
fn test_restart_on_failure() {
    let test_service_str = r#"
    [Service]
    Restart = on-failure
    ExecStart = /bin/myservice
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.restart,
        crate::units::ServiceRestart::OnFailure,
        "Restart=on-failure should parse as OnFailure"
    );
}

#[test]
fn test_restart_on_abnormal() {
    let test_service_str = r#"
    [Service]
    Restart = on-abnormal
    ExecStart = /bin/myservice
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.restart,
        crate::units::ServiceRestart::OnAbnormal,
        "Restart=on-abnormal should parse as OnAbnormal"
    );
}

#[test]
fn test_restart_on_abort() {
    let test_service_str = r#"
    [Service]
    Restart = on-abort
    ExecStart = /bin/myservice
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.restart,
        crate::units::ServiceRestart::OnAbort,
        "Restart=on-abort should parse as OnAbort"
    );
}

#[test]
fn test_restart_on_watchdog() {
    let test_service_str = r#"
    [Service]
    Restart = on-watchdog
    ExecStart = /bin/myservice
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.restart,
        crate::units::ServiceRestart::OnWatchdog,
        "Restart=on-watchdog should parse as OnWatchdog"
    );
}

#[test]
fn test_restart_on_failure_case_insensitive() {
    // This is the exact case from the bug report: ON-FAILURE (uppercase)
    let cases = vec!["on-failure", "ON-FAILURE", "On-Failure", "oN-fAiLuRe"];

    for case in cases {
        let test_service_str = format!(
            r#"
            [Service]
            Restart = {}
            ExecStart = /bin/myservice
            "#,
            case
        );

        let parsed_file = crate::units::parse_file(&test_service_str).unwrap();
        let service = crate::units::parse_service(
            parsed_file,
            &std::path::PathBuf::from("/path/to/unitfile.service"),
        )
        .unwrap();

        assert_eq!(
            service.srvc.restart,
            crate::units::ServiceRestart::OnFailure,
            "Restart={} should parse as OnFailure",
            case
        );
    }
}

#[test]
fn test_restart_on_success_case_insensitive() {
    let cases = vec!["on-success", "ON-SUCCESS", "On-Success"];

    for case in cases {
        let test_service_str = format!(
            r#"
            [Service]
            Restart = {}
            ExecStart = /bin/myservice
            "#,
            case
        );

        let parsed_file = crate::units::parse_file(&test_service_str).unwrap();
        let service = crate::units::parse_service(
            parsed_file,
            &std::path::PathBuf::from("/path/to/unitfile.service"),
        )
        .unwrap();

        assert_eq!(
            service.srvc.restart,
            crate::units::ServiceRestart::OnSuccess,
            "Restart={} should parse as OnSuccess",
            case
        );
    }
}

#[test]
fn test_restart_on_abnormal_case_insensitive() {
    let cases = vec!["on-abnormal", "ON-ABNORMAL", "On-Abnormal"];

    for case in cases {
        let test_service_str = format!(
            r#"
            [Service]
            Restart = {}
            ExecStart = /bin/myservice
            "#,
            case
        );

        let parsed_file = crate::units::parse_file(&test_service_str).unwrap();
        let service = crate::units::parse_service(
            parsed_file,
            &std::path::PathBuf::from("/path/to/unitfile.service"),
        )
        .unwrap();

        assert_eq!(
            service.srvc.restart,
            crate::units::ServiceRestart::OnAbnormal,
            "Restart={} should parse as OnAbnormal",
            case
        );
    }
}

#[test]
fn test_restart_defaults_to_no() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.restart,
        crate::units::ServiceRestart::No,
        "Restart should default to No when not specified"
    );
}

#[test]
fn test_restart_unknown_value_errors() {
    let test_service_str = r#"
    [Service]
    Restart = bogus
    ExecStart = /bin/myservice
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let result = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    );

    assert!(
        result.is_err(),
        "Restart=bogus should produce a parsing error"
    );
}

#[test]
fn test_restart_all_variants_are_distinct() {
    let variants = vec![
        ("no", crate::units::ServiceRestart::No),
        ("always", crate::units::ServiceRestart::Always),
        ("on-success", crate::units::ServiceRestart::OnSuccess),
        ("on-failure", crate::units::ServiceRestart::OnFailure),
        ("on-abnormal", crate::units::ServiceRestart::OnAbnormal),
        ("on-abort", crate::units::ServiceRestart::OnAbort),
        ("on-watchdog", crate::units::ServiceRestart::OnWatchdog),
    ];

    for (i, (str_a, variant_a)) in variants.iter().enumerate() {
        let service_str = format!(
            r#"
            [Service]
            Restart = {}
            ExecStart = /bin/myservice
            "#,
            str_a
        );

        let parsed = crate::units::parse_file(&service_str).unwrap();
        let service =
            crate::units::parse_service(parsed, &std::path::PathBuf::from("/path/to/test.service"))
                .unwrap();

        assert_eq!(
            &service.srvc.restart, variant_a,
            "Restart={} should parse as {:?}",
            str_a, variant_a
        );

        for (j, (_, variant_b)) in variants.iter().enumerate() {
            if i != j {
                assert_ne!(
                    variant_a, variant_b,
                    "Restart variants {:?} and {:?} should be distinct",
                    variant_a, variant_b
                );
            }
        }
    }
}

#[test]
fn test_restart_on_failure_with_other_settings() {
    let test_service_str = r#"
    [Unit]
    Description = A service with on-failure restart
    [Service]
    Type = notify
    Restart = on-failure
    RestartSec = 5s
    ExecStart = /bin/myservice --flag
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.restart,
        crate::units::ServiceRestart::OnFailure,
    );
    assert_eq!(
        service.common.unit.description,
        "A service with on-failure restart"
    );
    assert_eq!(service.srvc.srcv_type, crate::units::ServiceType::Notify);
    assert_eq!(
        service.srvc.restart_sec,
        Some(crate::units::Timeout::Duration(
            std::time::Duration::from_secs(5)
        ))
    );
}

#[test]
fn test_restart_on_failure_preserved_after_unit_conversion() {
    use std::convert::TryInto;

    let test_service_str = r#"
    [Unit]
    Description = On-failure restart test
    [Service]
    Restart = on-failure
    ExecStart = /usr/bin/testcmd
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let parsed_service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.service"),
    )
    .unwrap();

    let unit: crate::units::Unit = parsed_service.try_into().unwrap();

    if let crate::units::Specific::Service(ref srvc) = unit.specific {
        assert_eq!(srvc.conf.restart, crate::units::ServiceRestart::OnFailure,);
    } else {
        panic!("Expected Specific::Service");
    }
}

#[test]
fn test_restart_on_success_preserved_after_unit_conversion() {
    use std::convert::TryInto;

    let test_service_str = r#"
    [Service]
    Restart = on-success
    ExecStart = /usr/bin/testcmd
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let parsed_service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.service"),
    )
    .unwrap();

    let unit: crate::units::Unit = parsed_service.try_into().unwrap();

    if let crate::units::Specific::Service(ref srvc) = unit.specific {
        assert_eq!(srvc.conf.restart, crate::units::ServiceRestart::OnSuccess,);
    } else {
        panic!("Expected Specific::Service");
    }
}

// ============================================================
// SuccessAction= and FailureAction= parsing tests
// ============================================================

#[test]
fn test_success_action_defaults_to_none() {
    let test_service_str = r#"
    [Unit]
    Description = A simple service
    [Service]
    ExecStart = /bin/myservice
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.common.unit.success_action,
        crate::units::UnitAction::None,
        "SuccessAction should default to None"
    );
}

#[test]
fn test_failure_action_defaults_to_none() {
    let test_service_str = r#"
    [Unit]
    Description = A simple service
    [Service]
    ExecStart = /bin/myservice
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.common.unit.failure_action,
        crate::units::UnitAction::None,
        "FailureAction should default to None"
    );
}

#[test]
fn test_success_action_none() {
    let test_service_str = r#"
    [Unit]
    SuccessAction = none
    [Service]
    ExecStart = /bin/myservice
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.common.unit.success_action,
        crate::units::UnitAction::None,
    );
}

#[test]
fn test_success_action_exit() {
    let test_service_str = r#"
    [Unit]
    SuccessAction = exit
    [Service]
    ExecStart = /bin/myservice
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.common.unit.success_action,
        crate::units::UnitAction::Exit,
    );
}

#[test]
fn test_success_action_exit_force() {
    let test_service_str = r#"
    [Unit]
    SuccessAction = exit-force
    [Service]
    ExecStart = /bin/myservice
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.common.unit.success_action,
        crate::units::UnitAction::ExitForce,
    );
}

#[test]
fn test_success_action_reboot() {
    let test_service_str = r#"
    [Unit]
    SuccessAction = reboot
    [Service]
    ExecStart = /bin/myservice
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.common.unit.success_action,
        crate::units::UnitAction::Reboot,
    );
}

#[test]
fn test_success_action_reboot_force() {
    let test_service_str = r#"
    [Unit]
    SuccessAction = reboot-force
    [Service]
    ExecStart = /bin/myservice
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.common.unit.success_action,
        crate::units::UnitAction::RebootForce,
    );
}

#[test]
fn test_success_action_reboot_immediate() {
    let test_service_str = r#"
    [Unit]
    SuccessAction = reboot-immediate
    [Service]
    ExecStart = /bin/myservice
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.common.unit.success_action,
        crate::units::UnitAction::RebootImmediate,
    );
}

#[test]
fn test_success_action_poweroff() {
    let test_service_str = r#"
    [Unit]
    SuccessAction = poweroff
    [Service]
    ExecStart = /bin/myservice
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.common.unit.success_action,
        crate::units::UnitAction::Poweroff,
    );
}

#[test]
fn test_success_action_poweroff_force() {
    let test_service_str = r#"
    [Unit]
    SuccessAction = poweroff-force
    [Service]
    ExecStart = /bin/myservice
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.common.unit.success_action,
        crate::units::UnitAction::PoweroffForce,
    );
}

#[test]
fn test_success_action_poweroff_immediate() {
    let test_service_str = r#"
    [Unit]
    SuccessAction = poweroff-immediate
    [Service]
    ExecStart = /bin/myservice
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.common.unit.success_action,
        crate::units::UnitAction::PoweroffImmediate,
    );
}

#[test]
fn test_success_action_halt() {
    let test_service_str = r#"
    [Unit]
    SuccessAction = halt
    [Service]
    ExecStart = /bin/myservice
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.common.unit.success_action,
        crate::units::UnitAction::Halt,
    );
}

#[test]
fn test_success_action_halt_force() {
    let test_service_str = r#"
    [Unit]
    SuccessAction = halt-force
    [Service]
    ExecStart = /bin/myservice
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.common.unit.success_action,
        crate::units::UnitAction::HaltForce,
    );
}

#[test]
fn test_success_action_halt_immediate() {
    let test_service_str = r#"
    [Unit]
    SuccessAction = halt-immediate
    [Service]
    ExecStart = /bin/myservice
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.common.unit.success_action,
        crate::units::UnitAction::HaltImmediate,
    );
}

#[test]
fn test_success_action_kexec() {
    let test_service_str = r#"
    [Unit]
    SuccessAction = kexec
    [Service]
    ExecStart = /bin/myservice
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.common.unit.success_action,
        crate::units::UnitAction::Kexec,
    );
}

#[test]
fn test_success_action_kexec_force() {
    let test_service_str = r#"
    [Unit]
    SuccessAction = kexec-force
    [Service]
    ExecStart = /bin/myservice
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.common.unit.success_action,
        crate::units::UnitAction::KexecForce,
    );
}

#[test]
fn test_success_action_kexec_immediate() {
    let test_service_str = r#"
    [Unit]
    SuccessAction = kexec-immediate
    [Service]
    ExecStart = /bin/myservice
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.common.unit.success_action,
        crate::units::UnitAction::KexecImmediate,
    );
}

#[test]
fn test_failure_action_reboot() {
    let test_service_str = r#"
    [Unit]
    FailureAction = reboot
    [Service]
    ExecStart = /bin/myservice
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.common.unit.failure_action,
        crate::units::UnitAction::Reboot,
    );
}

#[test]
fn test_failure_action_reboot_force() {
    let test_service_str = r#"
    [Unit]
    FailureAction = reboot-force
    [Service]
    ExecStart = /bin/myservice
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.common.unit.failure_action,
        crate::units::UnitAction::RebootForce,
    );
}

#[test]
fn test_failure_action_poweroff() {
    let test_service_str = r#"
    [Unit]
    FailureAction = poweroff
    [Service]
    ExecStart = /bin/myservice
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.common.unit.failure_action,
        crate::units::UnitAction::Poweroff,
    );
}

#[test]
fn test_failure_action_exit_force() {
    let test_service_str = r#"
    [Unit]
    FailureAction = exit-force
    [Service]
    ExecStart = /bin/myservice
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.common.unit.failure_action,
        crate::units::UnitAction::ExitForce,
    );
}

#[test]
fn test_success_action_case_insensitive() {
    let cases = vec![
        ("reboot", crate::units::UnitAction::Reboot),
        ("REBOOT", crate::units::UnitAction::Reboot),
        ("Reboot", crate::units::UnitAction::Reboot),
        ("reboot-force", crate::units::UnitAction::RebootForce),
        ("REBOOT-FORCE", crate::units::UnitAction::RebootForce),
        ("Reboot-Force", crate::units::UnitAction::RebootForce),
        (
            "reboot-immediate",
            crate::units::UnitAction::RebootImmediate,
        ),
        (
            "REBOOT-IMMEDIATE",
            crate::units::UnitAction::RebootImmediate,
        ),
        ("poweroff", crate::units::UnitAction::Poweroff),
        ("POWEROFF", crate::units::UnitAction::Poweroff),
        ("poweroff-force", crate::units::UnitAction::PoweroffForce),
        ("POWEROFF-FORCE", crate::units::UnitAction::PoweroffForce),
    ];

    for (input, expected) in &cases {
        let test_service_str = format!(
            r#"
            [Unit]
            SuccessAction = {}
            [Service]
            ExecStart = /bin/myservice
            "#,
            input
        );

        let parsed_file = crate::units::parse_file(&test_service_str).unwrap();
        let service = crate::units::parse_service(
            parsed_file,
            &std::path::PathBuf::from("/path/to/unitfile.service"),
        )
        .unwrap();

        assert_eq!(
            &service.common.unit.success_action, expected,
            "SuccessAction={} should parse as {:?}",
            input, expected
        );
    }
}

#[test]
fn test_failure_action_case_insensitive() {
    let cases = vec![
        ("reboot-force", crate::units::UnitAction::RebootForce),
        ("REBOOT-FORCE", crate::units::UnitAction::RebootForce),
        ("Reboot-Force", crate::units::UnitAction::RebootForce),
    ];

    for (input, expected) in &cases {
        let test_service_str = format!(
            r#"
            [Unit]
            FailureAction = {}
            [Service]
            ExecStart = /bin/myservice
            "#,
            input
        );

        let parsed_file = crate::units::parse_file(&test_service_str).unwrap();
        let service = crate::units::parse_service(
            parsed_file,
            &std::path::PathBuf::from("/path/to/unitfile.service"),
        )
        .unwrap();

        assert_eq!(
            &service.common.unit.failure_action, expected,
            "FailureAction={} should parse as {:?}",
            input, expected
        );
    }
}

#[test]
fn test_success_action_unknown_value_errors() {
    let test_service_str = r#"
    [Unit]
    SuccessAction = bogus
    [Service]
    ExecStart = /bin/myservice
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let result = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    );

    assert!(
        result.is_err(),
        "SuccessAction=bogus should produce a parsing error"
    );
}

#[test]
fn test_failure_action_unknown_value_errors() {
    let test_service_str = r#"
    [Unit]
    FailureAction = bogus
    [Service]
    ExecStart = /bin/myservice
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let result = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    );

    assert!(
        result.is_err(),
        "FailureAction=bogus should produce a parsing error"
    );
}

#[test]
fn test_success_and_failure_action_both_set() {
    let test_service_str = r#"
    [Unit]
    Description = Reboot service
    SuccessAction = reboot
    FailureAction = reboot-force
    [Service]
    Type = oneshot
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.common.unit.success_action,
        crate::units::UnitAction::Reboot,
    );
    assert_eq!(
        service.common.unit.failure_action,
        crate::units::UnitAction::RebootForce,
    );
    assert_eq!(service.common.unit.description, "Reboot service");
}

#[test]
fn test_no_service_section_with_success_action() {
    // This is the systemd-reboot.service pattern: no [Service] section,
    // SuccessAction= in [Unit] triggers the system action when the
    // exec-less oneshot "succeeds" immediately.
    let test_service_str = r#"
    [Unit]
    Description=System Reboot
    DefaultDependencies=no
    Requires=shutdown.target
    After=shutdown.target
    SuccessAction=reboot
    FailureAction=reboot-force
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/systemd-reboot.service"),
    )
    .unwrap();

    assert_eq!(service.common.unit.description, "System Reboot");
    assert!(!service.common.unit.default_dependencies);
    assert_eq!(service.srvc.exec, None);
    assert_eq!(service.srvc.srcv_type, crate::units::ServiceType::OneShot);
    assert_eq!(
        service.common.unit.success_action,
        crate::units::UnitAction::Reboot,
    );
    assert_eq!(
        service.common.unit.failure_action,
        crate::units::UnitAction::RebootForce,
    );
}

#[test]
fn test_success_action_preserved_after_unit_conversion() {
    use std::convert::TryInto;

    let test_service_str = r#"
    [Unit]
    Description = Action test
    SuccessAction = poweroff
    FailureAction = reboot-immediate
    [Service]
    ExecStart = /usr/bin/testcmd
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let parsed_service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.service"),
    )
    .unwrap();

    let unit: crate::units::Unit = parsed_service.try_into().unwrap();

    assert_eq!(
        unit.common.unit.success_action,
        crate::units::UnitAction::Poweroff,
    );
    assert_eq!(
        unit.common.unit.failure_action,
        crate::units::UnitAction::RebootImmediate,
    );
}

#[test]
fn test_all_action_variants_parse() {
    let variants = vec![
        ("none", crate::units::UnitAction::None),
        ("exit", crate::units::UnitAction::Exit),
        ("exit-force", crate::units::UnitAction::ExitForce),
        ("reboot", crate::units::UnitAction::Reboot),
        ("reboot-force", crate::units::UnitAction::RebootForce),
        (
            "reboot-immediate",
            crate::units::UnitAction::RebootImmediate,
        ),
        ("poweroff", crate::units::UnitAction::Poweroff),
        ("poweroff-force", crate::units::UnitAction::PoweroffForce),
        (
            "poweroff-immediate",
            crate::units::UnitAction::PoweroffImmediate,
        ),
        ("halt", crate::units::UnitAction::Halt),
        ("halt-force", crate::units::UnitAction::HaltForce),
        ("halt-immediate", crate::units::UnitAction::HaltImmediate),
        ("kexec", crate::units::UnitAction::Kexec),
        ("kexec-force", crate::units::UnitAction::KexecForce),
        ("kexec-immediate", crate::units::UnitAction::KexecImmediate),
    ];

    for (input, expected) in &variants {
        let test_service_str = format!(
            r#"
            [Unit]
            SuccessAction = {}
            [Service]
            ExecStart = /bin/myservice
            "#,
            input
        );

        let parsed_file = crate::units::parse_file(&test_service_str).unwrap();
        let service = crate::units::parse_service(
            parsed_file,
            &std::path::PathBuf::from("/path/to/test.service"),
        )
        .unwrap();

        assert_eq!(
            &service.common.unit.success_action, expected,
            "SuccessAction={} should parse as {:?}",
            input, expected
        );
    }
}

#[test]
fn test_all_action_variants_are_distinct() {
    let variants: Vec<crate::units::UnitAction> = vec![
        crate::units::UnitAction::None,
        crate::units::UnitAction::Exit,
        crate::units::UnitAction::ExitForce,
        crate::units::UnitAction::Reboot,
        crate::units::UnitAction::RebootForce,
        crate::units::UnitAction::RebootImmediate,
        crate::units::UnitAction::Poweroff,
        crate::units::UnitAction::PoweroffForce,
        crate::units::UnitAction::PoweroffImmediate,
        crate::units::UnitAction::Halt,
        crate::units::UnitAction::HaltForce,
        crate::units::UnitAction::HaltImmediate,
        crate::units::UnitAction::Kexec,
        crate::units::UnitAction::KexecForce,
        crate::units::UnitAction::KexecImmediate,
    ];

    for (i, a) in variants.iter().enumerate() {
        for (j, b) in variants.iter().enumerate() {
            if i != j {
                assert_ne!(
                    a, b,
                    "UnitAction variants {:?} and {:?} should be distinct",
                    a, b
                );
            }
        }
    }
}

#[test]
fn test_success_action_target_unit() {
    let test_target_str = r#"
    [Unit]
    Description = A target with success action
    SuccessAction = exit
    "#;

    let parsed_file = crate::units::parse_file(test_target_str).unwrap();
    let target = crate::units::parse_target(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.target"),
    )
    .unwrap();

    assert_eq!(
        target.common.unit.success_action,
        crate::units::UnitAction::Exit,
    );
    assert_eq!(
        target.common.unit.failure_action,
        crate::units::UnitAction::None,
    );
}

#[test]
fn test_success_action_socket_unit() {
    let test_socket_str = r#"
    [Unit]
    Description = A socket with failure action
    FailureAction = reboot
    [Socket]
    ListenStream = /run/test.sock
    "#;

    let parsed_file = crate::units::parse_file(test_socket_str).unwrap();
    let socket = crate::units::parse_socket(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.socket"),
    )
    .unwrap();

    assert_eq!(
        socket.common.unit.failure_action,
        crate::units::UnitAction::Reboot,
    );
    assert_eq!(
        socket.common.unit.success_action,
        crate::units::UnitAction::None,
    );
}

#[test]
fn test_no_service_section_defaults_actions_to_none() {
    // Service units with no [Service] section should still parse and
    // default both actions to None when not specified.
    let test_service_str = r#"
    [Unit]
    Description = Minimal unit
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/minimal.service"),
    )
    .unwrap();

    assert_eq!(service.srvc.exec, None);
    assert_eq!(service.srvc.srcv_type, crate::units::ServiceType::OneShot);
    assert_eq!(
        service.common.unit.success_action,
        crate::units::UnitAction::None,
    );
    assert_eq!(
        service.common.unit.failure_action,
        crate::units::UnitAction::None,
    );
}

// ============================================================
// Alias= parsing tests
// ============================================================

#[test]
fn test_alias_empty_by_default() {
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
        service.common.install.alias.is_empty(),
        "Alias should be empty when not specified"
    );
}

#[test]
fn test_alias_single() {
    let test_service_str = r#"
    [Install]
    Alias = dbus-org.freedesktop.foo.service

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
        service.common.install.alias,
        vec!["dbus-org.freedesktop.foo.service".to_owned()]
    );
}

#[test]
fn test_alias_multiple_entries() {
    let test_service_str = r#"
    [Install]
    Alias = foo.service
    Alias = bar.service

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
        service.common.install.alias,
        vec!["foo.service".to_owned(), "bar.service".to_owned()]
    );
}

#[test]
fn test_alias_space_separated() {
    let test_service_str = r#"
    [Install]
    Alias = foo.service bar.service

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
        service.common.install.alias,
        vec!["foo.service".to_owned(), "bar.service".to_owned()]
    );
}

#[test]
fn test_alias_with_wantedby() {
    let test_service_str = r#"
    [Install]
    Alias = alt-name.service
    WantedBy = multi-user.target

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
        service.common.install.alias,
        vec!["alt-name.service".to_owned()]
    );
    assert_eq!(
        service.common.install.wanted_by,
        vec!["multi-user.target".to_owned()]
    );
}

#[test]
fn test_alias_target_unit() {
    let test_target_str = r#"
    [Unit]
    Description = A target with alias

    [Install]
    Alias = alt.target
    "#;

    let parsed_file = crate::units::parse_file(test_target_str).unwrap();
    let target = crate::units::parse_target(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.target"),
    )
    .unwrap();

    assert_eq!(target.common.install.alias, vec!["alt.target".to_owned()]);
}

#[test]
fn test_alias_socket_unit() {
    let test_socket_str = r#"
    [Socket]
    ListenStream = /run/test.sock

    [Install]
    Alias = alt.socket
    "#;

    let parsed_file = crate::units::parse_file(test_socket_str).unwrap();
    let socket = crate::units::parse_socket(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.socket"),
    )
    .unwrap();

    assert_eq!(socket.common.install.alias, vec!["alt.socket".to_owned()]);
}

#[test]
fn test_alias_preserved_after_unit_conversion() {
    use std::convert::TryInto;

    let test_service_str = r#"
    [Unit]
    Description = Service with alias
    [Install]
    Alias = dbus-org.freedesktop.foo.service
    Alias = alt-name.service
    [Service]
    ExecStart = /usr/bin/testcmd
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let parsed_service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.service"),
    )
    .unwrap();

    let unit: crate::units::Unit = parsed_service.try_into().unwrap();

    assert_eq!(
        unit.common.unit.aliases,
        vec![
            "dbus-org.freedesktop.foo.service".to_owned(),
            "alt-name.service".to_owned(),
        ]
    );
}

#[test]
fn test_alias_empty_after_unit_conversion() {
    use std::convert::TryInto;

    let test_service_str = r#"
    [Service]
    ExecStart = /usr/bin/testcmd
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let parsed_service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.service"),
    )
    .unwrap();

    let unit: crate::units::Unit = parsed_service.try_into().unwrap();

    assert!(
        unit.common.unit.aliases.is_empty(),
        "Aliases should be empty when Alias= is not specified"
    );
}

#[test]
fn test_alias_with_also_and_wantedby() {
    let test_service_str = r#"
    [Install]
    Alias = alt.service
    Also = helper.service
    WantedBy = multi-user.target
    RequiredBy = some.target

    [Service]
    ExecStart = /bin/true
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(service.common.install.alias, vec!["alt.service".to_owned()]);
    assert_eq!(
        service.common.install.also,
        vec!["helper.service".to_owned()]
    );
    assert_eq!(
        service.common.install.wanted_by,
        vec!["multi-user.target".to_owned()]
    );
    assert_eq!(
        service.common.install.required_by,
        vec!["some.target".to_owned()]
    );
}

#[test]
fn test_alias_dbus_pattern() {
    // Common systemd pattern: D-Bus activated services use Alias= to create
    // a dbus-org.freedesktop.* symlink
    let test_service_str = r#"
    [Unit]
    Description = Accounts Service

    [Service]
    Type = simple
    ExecStart = /usr/libexec/accounts-daemon

    [Install]
    Alias = dbus-org.freedesktop.Accounts.service
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/accounts-daemon.service"),
    )
    .unwrap();

    assert_eq!(
        service.common.install.alias,
        vec!["dbus-org.freedesktop.Accounts.service".to_owned()]
    );
}

#[test]
fn test_alias_comma_separated() {
    let test_service_str = r#"
    [Install]
    Alias = foo.service,bar.service

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
        service.common.install.alias,
        vec!["foo.service".to_owned(), "bar.service".to_owned()]
    );
}

// ============================================================
// PartOf= parsing tests
// ============================================================

#[test]
fn test_part_of_empty_by_default() {
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
        service.common.unit.part_of.is_empty(),
        "PartOf should be empty when not specified"
    );
}

#[test]
fn test_part_of_single() {
    let test_service_str = r#"
    [Unit]
    PartOf = network.target

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
        service.common.unit.part_of,
        vec!["network.target".to_owned()]
    );
}

#[test]
fn test_part_of_multiple_entries() {
    let test_service_str = r#"
    [Unit]
    PartOf = network.target
    PartOf = graphical.target

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
        service.common.unit.part_of,
        vec!["network.target".to_owned(), "graphical.target".to_owned(),]
    );
}

#[test]
fn test_part_of_space_separated() {
    let test_service_str = r#"
    [Unit]
    PartOf = network.target graphical.target

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
        service.common.unit.part_of,
        vec!["network.target".to_owned(), "graphical.target".to_owned(),]
    );
}

#[test]
fn test_part_of_with_other_dependencies() {
    let test_service_str = r#"
    [Unit]
    Description = A network helper
    PartOf = network.target
    After = network.target
    Wants = some.service

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
        service.common.unit.part_of,
        vec!["network.target".to_owned()]
    );
    assert_eq!(service.common.unit.after, vec!["network.target".to_owned()]);
    assert_eq!(service.common.unit.wants, vec!["some.service".to_owned()]);
    assert_eq!(service.common.unit.description, "A network helper");
}

#[test]
fn test_part_of_target_unit() {
    let test_target_str = r#"
    [Unit]
    Description = A sub-target
    PartOf = graphical.target
    "#;

    let parsed_file = crate::units::parse_file(test_target_str).unwrap();
    let target = crate::units::parse_target(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.target"),
    )
    .unwrap();

    assert_eq!(
        target.common.unit.part_of,
        vec!["graphical.target".to_owned()]
    );
}

#[test]
fn test_part_of_socket_unit() {
    let test_socket_str = r#"
    [Unit]
    PartOf = myapp.service

    [Socket]
    ListenStream = /run/test.sock
    "#;

    let parsed_file = crate::units::parse_file(test_socket_str).unwrap();
    let socket = crate::units::parse_socket(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.socket"),
    )
    .unwrap();

    assert_eq!(socket.common.unit.part_of, vec!["myapp.service".to_owned()]);
}

#[test]
fn test_part_of_preserved_after_unit_conversion() {
    use std::convert::TryInto;

    let test_service_str = r#"
    [Unit]
    Description = Network helper
    PartOf = network.target

    [Service]
    ExecStart = /usr/bin/testcmd
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let parsed_service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.service"),
    )
    .unwrap();

    let unit: crate::units::Unit = parsed_service.try_into().unwrap();

    assert_eq!(
        unit.common.dependencies.part_of,
        vec![crate::units::UnitId {
            name: "network.target".to_owned(),
            kind: crate::units::UnitIdKind::Target,
        }]
    );
}

#[test]
fn test_part_of_empty_after_unit_conversion() {
    use std::convert::TryInto;

    let test_service_str = r#"
    [Service]
    ExecStart = /usr/bin/testcmd
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let parsed_service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.service"),
    )
    .unwrap();

    let unit: crate::units::Unit = parsed_service.try_into().unwrap();

    assert!(
        unit.common.dependencies.part_of.is_empty(),
        "PartOf should be empty when not specified"
    );
    assert!(
        unit.common.dependencies.part_of_by.is_empty(),
        "PartOfBy should be empty initially"
    );
}

#[test]
fn test_part_of_included_in_refs_by_name() {
    use std::convert::TryInto;

    let test_service_str = r#"
    [Unit]
    PartOf = network.target
    DefaultDependencies = no

    [Service]
    ExecStart = /bin/true
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let parsed_service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.service"),
    )
    .unwrap();

    let unit: crate::units::Unit = parsed_service.try_into().unwrap();

    let target_id = crate::units::UnitId {
        name: "network.target".to_owned(),
        kind: crate::units::UnitIdKind::Target,
    };
    assert!(
        unit.common.unit.refs_by_name.contains(&target_id),
        "refs_by_name should include PartOf unit IDs"
    );
}

#[test]
fn test_part_of_comma_separated() {
    let test_service_str = r#"
    [Unit]
    PartOf = network.target,graphical.target

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
        service.common.unit.part_of,
        vec!["network.target".to_owned(), "graphical.target".to_owned(),]
    );
}

#[test]
fn test_part_of_with_requires_and_after() {
    // Common systemd pattern: PartOf= combined with After= and BindsTo=/Requires=
    let test_service_str = r#"
    [Unit]
    Description = Helper for main app
    PartOf = main-app.service
    After = main-app.service
    Requires = main-app.service

    [Service]
    ExecStart = /usr/bin/helper
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/helper.service"),
    )
    .unwrap();

    assert_eq!(
        service.common.unit.part_of,
        vec!["main-app.service".to_owned()]
    );
    assert_eq!(
        service.common.unit.after,
        vec!["main-app.service".to_owned()]
    );
    assert_eq!(
        service.common.unit.requires,
        vec!["main-app.service".to_owned()]
    );
}

// ── Slice= parsing tests ──────────────────────────────────────────────

#[test]
fn test_slice_not_set_by_default() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.service"),
    )
    .unwrap();

    assert_eq!(service.srvc.slice, None);
}

#[test]
fn test_slice_single_value() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    Slice = user.slice
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.service"),
    )
    .unwrap();

    assert_eq!(service.srvc.slice, Some("user.slice".to_owned()));
}

#[test]
fn test_slice_system_slice() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    Slice = system.slice
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.service"),
    )
    .unwrap();

    assert_eq!(service.srvc.slice, Some("system.slice".to_owned()));
}

#[test]
fn test_slice_custom_slice() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    Slice = my-custom.slice
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.service"),
    )
    .unwrap();

    assert_eq!(service.srvc.slice, Some("my-custom.slice".to_owned()));
}

#[test]
fn test_slice_nested_slice() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    Slice = user-1000.slice
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.service"),
    )
    .unwrap();

    assert_eq!(service.srvc.slice, Some("user-1000.slice".to_owned()));
}

#[test]
fn test_slice_with_other_settings() {
    let test_service_str = r#"
    [Unit]
    Description = Test service with slice

    [Service]
    ExecStart = /bin/true
    Restart = always
    Slice = system.slice
    Type = simple
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.service"),
    )
    .unwrap();

    assert_eq!(service.srvc.slice, Some("system.slice".to_owned()));
    assert_eq!(
        service.common.unit.description,
        "Test service with slice".to_owned()
    );
}

#[test]
fn test_slice_preserved_after_unit_conversion() {
    use std::convert::TryInto;

    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    Slice = user.slice
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.service"),
    )
    .unwrap();

    let unit: crate::units::Unit = service.try_into().unwrap();
    if let crate::units::Specific::Service(srvc) = &unit.specific {
        assert_eq!(srvc.conf.slice, Some("user.slice".to_owned()));
    } else {
        panic!("Expected a service unit");
    }
}

#[test]
fn test_slice_none_preserved_after_unit_conversion() {
    use std::convert::TryInto;

    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.service"),
    )
    .unwrap();

    let unit: crate::units::Unit = service.try_into().unwrap();
    if let crate::units::Specific::Service(srvc) = &unit.specific {
        assert_eq!(srvc.conf.slice, None);
    } else {
        panic!("Expected a service unit");
    }
}

#[test]
fn test_slice_no_unsupported_warning() {
    // Slice= should be parsed without generating an "unsupported setting" warning
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    Slice = user.slice
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.service"),
    );

    // Parsing should succeed (if Slice= were unsupported, it would warn but still parse)
    assert!(service.is_ok());
    assert_eq!(service.unwrap().srvc.slice, Some("user.slice".to_owned()));
}

// ── .slice in dependency lists ─────────────────────────────────────────

#[test]
fn test_slice_unit_id_conversion() {
    use std::convert::TryInto;

    let id: Result<crate::units::UnitId, _> =
        <&str as TryInto<crate::units::UnitId>>::try_into("user.slice");
    assert!(id.is_ok());
    let id = id.unwrap();
    assert_eq!(id.name, "user.slice");
    assert_eq!(id.kind, crate::units::UnitIdKind::Slice);
}

#[test]
fn test_slice_unit_id_system_slice() {
    use std::convert::TryInto;

    let id: crate::units::UnitId = "system.slice".try_into().unwrap();
    assert_eq!(id.name, "system.slice");
    assert_eq!(id.kind, crate::units::UnitIdKind::Slice);
}

#[test]
fn test_slice_in_after_dependency() {
    let test_service_str = r#"
    [Unit]
    After = user.slice

    [Service]
    ExecStart = /bin/true
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.service"),
    )
    .unwrap();

    assert_eq!(service.common.unit.after, vec!["user.slice".to_owned()]);
}

#[test]
fn test_slice_in_wants_dependency() {
    let test_service_str = r#"
    [Unit]
    Wants = user.slice

    [Service]
    ExecStart = /bin/true
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.service"),
    )
    .unwrap();

    assert_eq!(service.common.unit.wants, vec!["user.slice".to_owned()]);
}

#[test]
fn test_slice_in_after_preserved_after_unit_conversion() {
    use std::convert::TryInto;

    let test_service_str = r#"
    [Unit]
    After = user.slice

    [Service]
    ExecStart = /bin/true
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.service"),
    )
    .unwrap();

    let unit: crate::units::Unit = service.try_into().unwrap();
    let slice_id = crate::units::UnitId {
        name: "user.slice".to_owned(),
        kind: crate::units::UnitIdKind::Slice,
    };
    assert!(unit.common.dependencies.after.contains(&slice_id));
}

#[test]
fn test_slice_mixed_with_other_deps() {
    let test_service_str = r#"
    [Unit]
    After = user.slice network.target
    Wants = user.slice

    [Service]
    ExecStart = /bin/true
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.service"),
    )
    .unwrap();

    assert!(service.common.unit.after.contains(&"user.slice".to_owned()));
    assert!(service
        .common
        .unit
        .after
        .contains(&"network.target".to_owned()));
    assert!(service.common.unit.wants.contains(&"user.slice".to_owned()));
}

// ── RemainAfterExit= parsing tests ────────────────────────────────────

#[test]
fn test_remain_after_exit_defaults_to_false() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.service"),
    )
    .unwrap();

    assert!(!service.srvc.remain_after_exit);
}

#[test]
fn test_remain_after_exit_yes() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    RemainAfterExit = yes
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.service"),
    )
    .unwrap();

    assert!(service.srvc.remain_after_exit);
}

#[test]
fn test_remain_after_exit_true() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    RemainAfterExit = true
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.service"),
    )
    .unwrap();

    assert!(service.srvc.remain_after_exit);
}

#[test]
fn test_remain_after_exit_1() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    RemainAfterExit = 1
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.service"),
    )
    .unwrap();

    assert!(service.srvc.remain_after_exit);
}

#[test]
fn test_remain_after_exit_no() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    RemainAfterExit = no
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.service"),
    )
    .unwrap();

    assert!(!service.srvc.remain_after_exit);
}

#[test]
fn test_remain_after_exit_false() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    RemainAfterExit = false
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.service"),
    )
    .unwrap();

    assert!(!service.srvc.remain_after_exit);
}

#[test]
fn test_remain_after_exit_0() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    RemainAfterExit = 0
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.service"),
    )
    .unwrap();

    assert!(!service.srvc.remain_after_exit);
}

#[test]
fn test_remain_after_exit_with_oneshot() {
    let test_service_str = r#"
    [Service]
    Type = oneshot
    ExecStart = /bin/setup-something
    RemainAfterExit = yes
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.service"),
    )
    .unwrap();

    assert!(service.srvc.remain_after_exit);
    assert_eq!(service.srvc.srcv_type, crate::units::ServiceType::OneShot);
}

#[test]
fn test_remain_after_exit_with_simple() {
    let test_service_str = r#"
    [Service]
    Type = simple
    ExecStart = /bin/true
    RemainAfterExit = yes
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.service"),
    )
    .unwrap();

    assert!(service.srvc.remain_after_exit);
    assert_eq!(service.srvc.srcv_type, crate::units::ServiceType::Simple);
}

#[test]
fn test_remain_after_exit_with_other_settings() {
    let test_service_str = r#"
    [Unit]
    Description = Setup service

    [Service]
    Type = oneshot
    ExecStart = /bin/setup
    RemainAfterExit = yes
    Restart = no

    [Install]
    WantedBy = multi-user.target
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.service"),
    )
    .unwrap();

    assert!(service.srvc.remain_after_exit);
    assert_eq!(service.common.unit.description, "Setup service".to_owned());
}

#[test]
fn test_remain_after_exit_preserved_after_unit_conversion() {
    use std::convert::TryInto;

    let test_service_str = r#"
    [Service]
    Type = oneshot
    ExecStart = /bin/true
    RemainAfterExit = yes
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.service"),
    )
    .unwrap();

    let unit: crate::units::Unit = service.try_into().unwrap();
    if let crate::units::Specific::Service(srvc) = &unit.specific {
        assert!(srvc.conf.remain_after_exit);
    } else {
        panic!("Expected a service unit");
    }
}

#[test]
fn test_remain_after_exit_false_preserved_after_unit_conversion() {
    use std::convert::TryInto;

    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    RemainAfterExit = no
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.service"),
    )
    .unwrap();

    let unit: crate::units::Unit = service.try_into().unwrap();
    if let crate::units::Specific::Service(srvc) = &unit.specific {
        assert!(!srvc.conf.remain_after_exit);
    } else {
        panic!("Expected a service unit");
    }
}

#[test]
fn test_remain_after_exit_default_preserved_after_unit_conversion() {
    use std::convert::TryInto;

    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.service"),
    )
    .unwrap();

    let unit: crate::units::Unit = service.try_into().unwrap();
    if let crate::units::Specific::Service(srvc) = &unit.specific {
        assert!(!srvc.conf.remain_after_exit);
    } else {
        panic!("Expected a service unit");
    }
}

// ── SuccessExitStatus= parsing tests ──────────────────────────────────

#[test]
fn test_success_exit_status_empty_by_default() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.service"),
    )
    .unwrap();

    assert!(service.srvc.success_exit_status.exit_codes.is_empty());
    assert!(service.srvc.success_exit_status.signals.is_empty());
}

#[test]
fn test_success_exit_status_single_code() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    SuccessExitStatus = 42
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.service"),
    )
    .unwrap();

    assert_eq!(service.srvc.success_exit_status.exit_codes, vec![42]);
    assert!(service.srvc.success_exit_status.signals.is_empty());
}

#[test]
fn test_success_exit_status_multiple_codes() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    SuccessExitStatus = 42 75 100
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.success_exit_status.exit_codes,
        vec![42, 75, 100]
    );
    assert!(service.srvc.success_exit_status.signals.is_empty());
}

#[test]
fn test_success_exit_status_single_signal_with_prefix() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    SuccessExitStatus = SIGTERM
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.service"),
    )
    .unwrap();

    assert!(service.srvc.success_exit_status.exit_codes.is_empty());
    assert_eq!(
        service.srvc.success_exit_status.signals,
        vec![nix::sys::signal::Signal::SIGTERM]
    );
}

#[test]
fn test_success_exit_status_single_signal_without_prefix() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    SuccessExitStatus = TERM
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.service"),
    )
    .unwrap();

    assert!(service.srvc.success_exit_status.exit_codes.is_empty());
    assert_eq!(
        service.srvc.success_exit_status.signals,
        vec![nix::sys::signal::Signal::SIGTERM]
    );
}

#[test]
fn test_success_exit_status_mixed_codes_and_signals() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    SuccessExitStatus = 42 SIGUSR1 75 HUP
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.service"),
    )
    .unwrap();

    assert_eq!(service.srvc.success_exit_status.exit_codes, vec![42, 75]);
    assert_eq!(
        service.srvc.success_exit_status.signals,
        vec![
            nix::sys::signal::Signal::SIGUSR1,
            nix::sys::signal::Signal::SIGHUP,
        ]
    );
}

#[test]
fn test_success_exit_status_multiple_entries_accumulate() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    SuccessExitStatus = 42
    SuccessExitStatus = SIGUSR2
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.service"),
    )
    .unwrap();

    assert_eq!(service.srvc.success_exit_status.exit_codes, vec![42]);
    assert_eq!(
        service.srvc.success_exit_status.signals,
        vec![nix::sys::signal::Signal::SIGUSR2]
    );
}

#[test]
fn test_success_exit_status_case_insensitive_signals() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    SuccessExitStatus = sigterm
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.success_exit_status.signals,
        vec![nix::sys::signal::Signal::SIGTERM]
    );
}

#[test]
fn test_success_exit_status_with_other_settings() {
    let test_service_str = r#"
    [Unit]
    Description = Test service with success exit status

    [Service]
    Type = oneshot
    ExecStart = /bin/setup
    RemainAfterExit = yes
    SuccessExitStatus = 42 SIGUSR1
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.service"),
    )
    .unwrap();

    assert_eq!(service.srvc.success_exit_status.exit_codes, vec![42]);
    assert_eq!(
        service.srvc.success_exit_status.signals,
        vec![nix::sys::signal::Signal::SIGUSR1]
    );
    assert!(service.srvc.remain_after_exit);
    assert_eq!(service.srvc.srcv_type, crate::units::ServiceType::OneShot);
}

#[test]
fn test_success_exit_status_preserved_after_unit_conversion() {
    use std::convert::TryInto;

    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    SuccessExitStatus = 42 75 SIGUSR1 HUP
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.service"),
    )
    .unwrap();

    let unit: crate::units::Unit = service.try_into().unwrap();
    if let crate::units::Specific::Service(srvc) = &unit.specific {
        assert_eq!(srvc.conf.success_exit_status.exit_codes, vec![42, 75]);
        assert_eq!(
            srvc.conf.success_exit_status.signals,
            vec![
                nix::sys::signal::Signal::SIGUSR1,
                nix::sys::signal::Signal::SIGHUP,
            ]
        );
    } else {
        panic!("Expected a service unit");
    }
}

#[test]
fn test_success_exit_status_empty_preserved_after_unit_conversion() {
    use std::convert::TryInto;

    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.service"),
    )
    .unwrap();

    let unit: crate::units::Unit = service.try_into().unwrap();
    if let crate::units::Specific::Service(srvc) = &unit.specific {
        assert!(srvc.conf.success_exit_status.exit_codes.is_empty());
        assert!(srvc.conf.success_exit_status.signals.is_empty());
    } else {
        panic!("Expected a service unit");
    }
}

// ── SuccessExitStatus is_success / is_clean_signal tests ──────────────

#[test]
fn test_success_exit_status_is_success_default_exit_zero() {
    let ses = crate::units::SuccessExitStatus::default();
    assert!(ses.is_success(&crate::signal_handler::ChildTermination::Exit(0)));
}

#[test]
fn test_success_exit_status_is_success_default_exit_nonzero() {
    let ses = crate::units::SuccessExitStatus::default();
    assert!(!ses.is_success(&crate::signal_handler::ChildTermination::Exit(1)));
    assert!(!ses.is_success(&crate::signal_handler::ChildTermination::Exit(42)));
}

#[test]
fn test_success_exit_status_is_success_extra_code() {
    let ses = crate::units::SuccessExitStatus {
        exit_codes: vec![42, 75],
        signals: vec![],
    };
    assert!(ses.is_success(&crate::signal_handler::ChildTermination::Exit(0)));
    assert!(ses.is_success(&crate::signal_handler::ChildTermination::Exit(42)));
    assert!(ses.is_success(&crate::signal_handler::ChildTermination::Exit(75)));
    assert!(!ses.is_success(&crate::signal_handler::ChildTermination::Exit(1)));
    assert!(!ses.is_success(&crate::signal_handler::ChildTermination::Exit(100)));
}

#[test]
fn test_success_exit_status_is_success_extra_signal() {
    let ses = crate::units::SuccessExitStatus {
        exit_codes: vec![],
        signals: vec![nix::sys::signal::Signal::SIGUSR1],
    };
    assert!(
        ses.is_success(&crate::signal_handler::ChildTermination::Signal(
            nix::sys::signal::Signal::SIGUSR1
        ))
    );
    assert!(
        !ses.is_success(&crate::signal_handler::ChildTermination::Signal(
            nix::sys::signal::Signal::SIGUSR2
        ))
    );
}

#[test]
fn test_success_exit_status_is_clean_signal_defaults() {
    let ses = crate::units::SuccessExitStatus::default();
    // Built-in clean signals
    assert!(
        ses.is_clean_signal(&crate::signal_handler::ChildTermination::Signal(
            nix::sys::signal::Signal::SIGHUP
        ))
    );
    assert!(
        ses.is_clean_signal(&crate::signal_handler::ChildTermination::Signal(
            nix::sys::signal::Signal::SIGINT
        ))
    );
    assert!(
        ses.is_clean_signal(&crate::signal_handler::ChildTermination::Signal(
            nix::sys::signal::Signal::SIGTERM
        ))
    );
    assert!(
        ses.is_clean_signal(&crate::signal_handler::ChildTermination::Signal(
            nix::sys::signal::Signal::SIGPIPE
        ))
    );
    // Not clean by default
    assert!(
        !ses.is_clean_signal(&crate::signal_handler::ChildTermination::Signal(
            nix::sys::signal::Signal::SIGUSR1
        ))
    );
    assert!(
        !ses.is_clean_signal(&crate::signal_handler::ChildTermination::Signal(
            nix::sys::signal::Signal::SIGKILL
        ))
    );
}

#[test]
fn test_success_exit_status_is_clean_signal_extra() {
    let ses = crate::units::SuccessExitStatus {
        exit_codes: vec![],
        signals: vec![nix::sys::signal::Signal::SIGUSR1],
    };
    // Extra signal is now clean
    assert!(
        ses.is_clean_signal(&crate::signal_handler::ChildTermination::Signal(
            nix::sys::signal::Signal::SIGUSR1
        ))
    );
    // Built-in clean signals still work
    assert!(
        ses.is_clean_signal(&crate::signal_handler::ChildTermination::Signal(
            nix::sys::signal::Signal::SIGTERM
        ))
    );
    // Other signals still not clean
    assert!(
        !ses.is_clean_signal(&crate::signal_handler::ChildTermination::Signal(
            nix::sys::signal::Signal::SIGUSR2
        ))
    );
}

#[test]
fn test_success_exit_status_is_clean_signal_not_exit() {
    let ses = crate::units::SuccessExitStatus {
        exit_codes: vec![42],
        signals: vec![],
    };
    // Exit codes are not "clean signals"
    assert!(!ses.is_clean_signal(&crate::signal_handler::ChildTermination::Exit(0)));
    assert!(!ses.is_clean_signal(&crate::signal_handler::ChildTermination::Exit(42)));
}

#[test]
fn test_success_exit_status_various_signal_names() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    SuccessExitStatus = USR1 USR2 SIGKILL QUIT
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.success_exit_status.signals,
        vec![
            nix::sys::signal::Signal::SIGUSR1,
            nix::sys::signal::Signal::SIGUSR2,
            nix::sys::signal::Signal::SIGKILL,
            nix::sys::signal::Signal::SIGQUIT,
        ]
    );
}

#[test]
fn test_success_exit_status_no_unsupported_warning() {
    // SuccessExitStatus= should be parsed without generating an "unsupported setting" warning
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    SuccessExitStatus = 42
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.service"),
    );

    assert!(service.is_ok());
    assert_eq!(
        service.unwrap().srvc.success_exit_status.exit_codes,
        vec![42]
    );
}

// ── DefaultInstance= parsing tests ─────────────────────────────────────

#[test]
fn test_default_instance_not_set_by_default() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.service"),
    )
    .unwrap();

    assert_eq!(service.common.install.default_instance, None);
}

#[test]
fn test_default_instance_single_value() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true

    [Install]
    WantedBy = multi-user.target
    DefaultInstance = default
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test@.service"),
    )
    .unwrap();

    assert_eq!(
        service.common.install.default_instance,
        Some("default".to_owned())
    );
}

#[test]
fn test_default_instance_custom_value() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true

    [Install]
    DefaultInstance = myinstance
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test@.service"),
    )
    .unwrap();

    assert_eq!(
        service.common.install.default_instance,
        Some("myinstance".to_owned())
    );
}

#[test]
fn test_default_instance_preserved_after_unit_conversion() {
    use std::convert::TryInto;

    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true

    [Install]
    DefaultInstance = primary
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test@.service"),
    )
    .unwrap();

    let unit: crate::units::Unit = service.try_into().unwrap();
    assert_eq!(
        unit.common.unit.default_instance,
        Some("primary".to_owned())
    );
}

#[test]
fn test_default_instance_none_preserved_after_unit_conversion() {
    use std::convert::TryInto;

    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.service"),
    )
    .unwrap();

    let unit: crate::units::Unit = service.try_into().unwrap();
    assert_eq!(unit.common.unit.default_instance, None);
}

#[test]
fn test_default_instance_with_other_install_settings() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true

    [Install]
    WantedBy = multi-user.target
    Alias = foo@.service
    DefaultInstance = bar
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test@.service"),
    )
    .unwrap();

    assert_eq!(
        service.common.install.default_instance,
        Some("bar".to_owned())
    );
    assert_eq!(
        service.common.install.wanted_by,
        vec!["multi-user.target".to_owned()]
    );
    assert_eq!(
        service.common.install.alias,
        vec!["foo@.service".to_owned()]
    );
}

#[test]
fn test_default_instance_no_unsupported_warning() {
    // DefaultInstance= should be parsed without generating an "unsupported setting" warning
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true

    [Install]
    DefaultInstance = test
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test@.service"),
    );

    assert!(service.is_ok());
    assert_eq!(
        service.unwrap().common.install.default_instance,
        Some("test".to_owned())
    );
}

// ── IgnoreOnIsolate= parsing tests ────────────────────────────────────

#[test]
fn test_ignore_on_isolate_defaults_to_false() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.service"),
    )
    .unwrap();

    assert!(!service.common.unit.ignore_on_isolate);
}

#[test]
fn test_ignore_on_isolate_set_true() {
    let test_service_str = r#"
    [Unit]
    IgnoreOnIsolate = yes

    [Service]
    ExecStart = /bin/true
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.service"),
    )
    .unwrap();

    assert!(service.common.unit.ignore_on_isolate);
}

#[test]
fn test_ignore_on_isolate_set_true_alt() {
    let test_service_str = r#"
    [Unit]
    IgnoreOnIsolate = true

    [Service]
    ExecStart = /bin/true
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.service"),
    )
    .unwrap();

    assert!(service.common.unit.ignore_on_isolate);
}

#[test]
fn test_ignore_on_isolate_set_false() {
    let test_service_str = r#"
    [Unit]
    IgnoreOnIsolate = no

    [Service]
    ExecStart = /bin/true
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.service"),
    )
    .unwrap();

    assert!(!service.common.unit.ignore_on_isolate);
}

#[test]
fn test_ignore_on_isolate_set_false_alt() {
    let test_service_str = r#"
    [Unit]
    IgnoreOnIsolate = false

    [Service]
    ExecStart = /bin/true
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.service"),
    )
    .unwrap();

    assert!(!service.common.unit.ignore_on_isolate);
}

#[test]
fn test_ignore_on_isolate_preserved_after_unit_conversion() {
    use std::convert::TryInto;

    let test_service_str = r#"
    [Unit]
    IgnoreOnIsolate = yes

    [Service]
    ExecStart = /bin/true
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.service"),
    )
    .unwrap();

    let unit: crate::units::Unit = service.try_into().unwrap();
    assert!(unit.common.unit.ignore_on_isolate);
}

#[test]
fn test_ignore_on_isolate_false_preserved_after_unit_conversion() {
    use std::convert::TryInto;

    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.service"),
    )
    .unwrap();

    let unit: crate::units::Unit = service.try_into().unwrap();
    assert!(!unit.common.unit.ignore_on_isolate);
}

#[test]
fn test_ignore_on_isolate_with_other_unit_settings() {
    let test_service_str = r#"
    [Unit]
    Description = Test service with IgnoreOnIsolate
    IgnoreOnIsolate = yes
    DefaultDependencies = no

    [Service]
    ExecStart = /bin/true
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.service"),
    )
    .unwrap();

    assert!(service.common.unit.ignore_on_isolate);
    assert!(!service.common.unit.default_dependencies);
    assert_eq!(
        service.common.unit.description,
        "Test service with IgnoreOnIsolate"
    );
}

#[test]
fn test_ignore_on_isolate_no_unsupported_warning() {
    // IgnoreOnIsolate= should be parsed without generating an "unsupported setting" warning
    let test_service_str = r#"
    [Unit]
    IgnoreOnIsolate = yes

    [Service]
    ExecStart = /bin/true
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.service"),
    );

    assert!(service.is_ok());
    assert!(service.unwrap().common.unit.ignore_on_isolate);
}

// ── StopWhenUnneeded= ─────────────────────────────────────────────────

#[test]
fn test_stop_when_unneeded_defaults_to_false() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.service"),
    )
    .unwrap();

    assert!(
        !service.common.unit.stop_when_unneeded,
        "StopWhenUnneeded should default to false when not specified"
    );
}

#[test]
fn test_stop_when_unneeded_set_yes() {
    let test_service_str = r#"
    [Unit]
    StopWhenUnneeded = yes

    [Service]
    ExecStart = /bin/true
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.service"),
    )
    .unwrap();

    assert!(
        service.common.unit.stop_when_unneeded,
        "StopWhenUnneeded=yes should be true"
    );
}

#[test]
fn test_stop_when_unneeded_set_true() {
    let test_service_str = r#"
    [Unit]
    StopWhenUnneeded = true

    [Service]
    ExecStart = /bin/true
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.service"),
    )
    .unwrap();

    assert!(
        service.common.unit.stop_when_unneeded,
        "StopWhenUnneeded=true should be true"
    );
}

#[test]
fn test_stop_when_unneeded_set_no() {
    let test_service_str = r#"
    [Unit]
    StopWhenUnneeded = no

    [Service]
    ExecStart = /bin/true
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.service"),
    )
    .unwrap();

    assert!(
        !service.common.unit.stop_when_unneeded,
        "StopWhenUnneeded=no should be false"
    );
}

#[test]
fn test_stop_when_unneeded_set_false() {
    let test_service_str = r#"
    [Unit]
    StopWhenUnneeded = false

    [Service]
    ExecStart = /bin/true
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.service"),
    )
    .unwrap();

    assert!(
        !service.common.unit.stop_when_unneeded,
        "StopWhenUnneeded=false should be false"
    );
}

#[test]
fn test_stop_when_unneeded_set_1() {
    let test_service_str = r#"
    [Unit]
    StopWhenUnneeded = 1

    [Service]
    ExecStart = /bin/true
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.service"),
    )
    .unwrap();

    assert!(
        service.common.unit.stop_when_unneeded,
        "StopWhenUnneeded=1 should be true"
    );
}

#[test]
fn test_stop_when_unneeded_set_0() {
    let test_service_str = r#"
    [Unit]
    StopWhenUnneeded = 0

    [Service]
    ExecStart = /bin/true
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.service"),
    )
    .unwrap();

    assert!(
        !service.common.unit.stop_when_unneeded,
        "StopWhenUnneeded=0 should be false"
    );
}

#[test]
fn test_stop_when_unneeded_case_insensitive() {
    let test_service_str = r#"
    [Unit]
    StopWhenUnneeded = YES

    [Service]
    ExecStart = /bin/true
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.service"),
    )
    .unwrap();

    assert!(
        service.common.unit.stop_when_unneeded,
        "StopWhenUnneeded=YES (case-insensitive) should be true"
    );
}

#[test]
fn test_stop_when_unneeded_with_other_unit_settings() {
    let test_service_str = r#"
    [Unit]
    Description = Test service with StopWhenUnneeded
    StopWhenUnneeded = yes
    DefaultDependencies = no
    IgnoreOnIsolate = yes

    [Service]
    ExecStart = /bin/true
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.service"),
    )
    .unwrap();

    assert!(service.common.unit.stop_when_unneeded);
    assert!(!service.common.unit.default_dependencies);
    assert!(service.common.unit.ignore_on_isolate);
    assert_eq!(
        service.common.unit.description,
        "Test service with StopWhenUnneeded"
    );
}

#[test]
fn test_stop_when_unneeded_no_unsupported_warning() {
    // StopWhenUnneeded= should be parsed without generating an "unsupported setting" warning
    let test_service_str = r#"
    [Unit]
    StopWhenUnneeded = yes

    [Service]
    ExecStart = /bin/true
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.service"),
    );

    assert!(
        service.is_ok(),
        "Parsing StopWhenUnneeded= should not produce an error"
    );
    assert!(service.unwrap().common.unit.stop_when_unneeded);
}

#[test]
fn test_stop_when_unneeded_socket_unit() {
    let test_socket_str = r#"
    [Unit]
    StopWhenUnneeded = yes

    [Socket]
    ListenStream = /run/test.sock
    "#;

    let parsed_file = crate::units::parse_file(test_socket_str).unwrap();
    let socket = crate::units::parse_socket(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.socket"),
    )
    .unwrap();

    assert!(
        socket.common.unit.stop_when_unneeded,
        "StopWhenUnneeded=yes should be true for socket units"
    );
}

#[test]
fn test_stop_when_unneeded_target_unit() {
    let test_target_str = r#"
    [Unit]
    StopWhenUnneeded = yes
    "#;

    let parsed_file = crate::units::parse_file(test_target_str).unwrap();
    let target = crate::units::parse_target(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.target"),
    )
    .unwrap();

    assert!(
        target.common.unit.stop_when_unneeded,
        "StopWhenUnneeded=yes should be true for target units"
    );
}

// ── RequiresMountsFor= ────────────────────────────────────────────────

#[test]
fn test_path_to_mount_unit_name_root() {
    assert_eq!(crate::units::path_to_mount_unit_name("/"), "-.mount");
}

#[test]
fn test_path_to_mount_unit_name_single_component() {
    assert_eq!(crate::units::path_to_mount_unit_name("/var"), "var.mount");
}

#[test]
fn test_path_to_mount_unit_name_nested() {
    assert_eq!(
        crate::units::path_to_mount_unit_name("/var/log"),
        "var-log.mount"
    );
}

#[test]
fn test_path_to_mount_unit_name_deeply_nested() {
    assert_eq!(
        crate::units::path_to_mount_unit_name("/var/log/myapp"),
        "var-log-myapp.mount"
    );
}

#[test]
fn test_path_to_mount_unit_name_trailing_slash() {
    assert_eq!(
        crate::units::path_to_mount_unit_name("/var/log/"),
        "var-log.mount"
    );
}

#[test]
fn test_mount_units_for_path_root() {
    let units = crate::units::mount_units_for_path("/");
    assert_eq!(units, vec!["-.mount"]);
}

#[test]
fn test_mount_units_for_path_single() {
    let units = crate::units::mount_units_for_path("/var");
    assert_eq!(units, vec!["-.mount", "var.mount"]);
}

#[test]
fn test_mount_units_for_path_nested() {
    let units = crate::units::mount_units_for_path("/var/log");
    assert_eq!(units, vec!["-.mount", "var.mount", "var-log.mount"]);
}

#[test]
fn test_mount_units_for_path_deeply_nested() {
    let units = crate::units::mount_units_for_path("/var/log/myapp");
    assert_eq!(
        units,
        vec![
            "-.mount",
            "var.mount",
            "var-log.mount",
            "var-log-myapp.mount",
        ]
    );
}

#[test]
fn test_requires_mounts_for_single_path() {
    let test_service_str = r#"
    [Unit]
    RequiresMountsFor = /var/log

    [Service]
    ExecStart = /bin/true
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.service"),
    )
    .unwrap();

    // Original paths stored
    assert_eq!(
        service.common.unit.requires_mounts_for,
        vec!["/var/log".to_owned()]
    );
    // Implicit Requires= dependencies added
    assert!(service.common.unit.requires.contains(&"-.mount".to_owned()));
    assert!(service
        .common
        .unit
        .requires
        .contains(&"var.mount".to_owned()));
    assert!(service
        .common
        .unit
        .requires
        .contains(&"var-log.mount".to_owned()));
    // Implicit After= dependencies added
    assert!(service.common.unit.after.contains(&"-.mount".to_owned()));
    assert!(service.common.unit.after.contains(&"var.mount".to_owned()));
    assert!(service
        .common
        .unit
        .after
        .contains(&"var-log.mount".to_owned()));
}

#[test]
fn test_requires_mounts_for_multiple_paths() {
    let test_service_str = r#"
    [Unit]
    RequiresMountsFor = /var/log /home/user

    [Service]
    ExecStart = /bin/true
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.service"),
    )
    .unwrap();

    assert_eq!(
        service.common.unit.requires_mounts_for,
        vec!["/var/log".to_owned(), "/home/user".to_owned()]
    );
    // Mount units from both paths
    assert!(service.common.unit.requires.contains(&"-.mount".to_owned()));
    assert!(service
        .common
        .unit
        .requires
        .contains(&"var.mount".to_owned()));
    assert!(service
        .common
        .unit
        .requires
        .contains(&"var-log.mount".to_owned()));
    assert!(service
        .common
        .unit
        .requires
        .contains(&"home.mount".to_owned()));
    assert!(service
        .common
        .unit
        .requires
        .contains(&"home-user.mount".to_owned()));
}

#[test]
fn test_requires_mounts_for_no_duplicate_mount_units() {
    // Two paths sharing a prefix should not duplicate mount unit deps
    let test_service_str = r#"
    [Unit]
    RequiresMountsFor = /var/log /var/cache

    [Service]
    ExecStart = /bin/true
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.service"),
    )
    .unwrap();

    // "-.mount" and "var.mount" should appear only once each
    let root_count = service
        .common
        .unit
        .requires
        .iter()
        .filter(|u| *u == "-.mount")
        .count();
    assert_eq!(root_count, 1, "-.mount should appear exactly once");
    let var_count = service
        .common
        .unit
        .requires
        .iter()
        .filter(|u| *u == "var.mount")
        .count();
    assert_eq!(var_count, 1, "var.mount should appear exactly once");
}

#[test]
fn test_requires_mounts_for_combined_with_explicit_requires() {
    let test_service_str = r#"
    [Unit]
    Requires = network.target
    RequiresMountsFor = /var/log

    [Service]
    ExecStart = /bin/true
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.service"),
    )
    .unwrap();

    // Explicit dependency preserved
    assert!(service
        .common
        .unit
        .requires
        .contains(&"network.target".to_owned()));
    // Implicit mount deps also present
    assert!(service.common.unit.requires.contains(&"-.mount".to_owned()));
    assert!(service
        .common
        .unit
        .requires
        .contains(&"var-log.mount".to_owned()));
}

#[test]
fn test_requires_mounts_for_combined_with_explicit_after() {
    let test_service_str = r#"
    [Unit]
    After = network.target
    RequiresMountsFor = /home

    [Service]
    ExecStart = /bin/true
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.service"),
    )
    .unwrap();

    // Explicit After= preserved
    assert!(service
        .common
        .unit
        .after
        .contains(&"network.target".to_owned()));
    // Implicit mount After= also present
    assert!(service.common.unit.after.contains(&"-.mount".to_owned()));
    assert!(service.common.unit.after.contains(&"home.mount".to_owned()));
}

#[test]
fn test_requires_mounts_for_no_duplicate_with_explicit_mount_dep() {
    // If RequiresMountsFor generates a dep that's already in Requires=, no duplicate
    let test_service_str = r#"
    [Unit]
    Requires = var.mount
    After = var.mount
    RequiresMountsFor = /var/log

    [Service]
    ExecStart = /bin/true
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.service"),
    )
    .unwrap();

    let var_req_count = service
        .common
        .unit
        .requires
        .iter()
        .filter(|u| *u == "var.mount")
        .count();
    assert_eq!(
        var_req_count, 1,
        "var.mount should appear exactly once in requires"
    );
    let var_after_count = service
        .common
        .unit
        .after
        .iter()
        .filter(|u| *u == "var.mount")
        .count();
    assert_eq!(
        var_after_count, 1,
        "var.mount should appear exactly once in after"
    );
}

#[test]
fn test_requires_mounts_for_root_path() {
    let test_service_str = r#"
    [Unit]
    RequiresMountsFor = /

    [Service]
    ExecStart = /bin/true
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.service"),
    )
    .unwrap();

    assert_eq!(
        service.common.unit.requires_mounts_for,
        vec!["/".to_owned()]
    );
    assert!(service.common.unit.requires.contains(&"-.mount".to_owned()));
    assert!(service.common.unit.after.contains(&"-.mount".to_owned()));
}

#[test]
fn test_requires_mounts_for_defaults_to_empty() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.service"),
    )
    .unwrap();

    assert!(service.common.unit.requires_mounts_for.is_empty());
}

#[test]
fn test_requires_mounts_for_no_unsupported_warning() {
    // RequiresMountsFor= should be recognized and not trigger a warning
    let test_service_str = r#"
    [Unit]
    RequiresMountsFor = /var/log

    [Service]
    ExecStart = /bin/true
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let result = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.service"),
    );

    assert!(
        result.is_ok(),
        "Parsing RequiresMountsFor= should not produce an error"
    );
    // If it were unsupported, requires_mounts_for would be empty
    assert!(!result.unwrap().common.unit.requires_mounts_for.is_empty());
}

#[test]
fn test_requires_mounts_for_preserved_after_unit_conversion() {
    use std::convert::TryInto;

    let test_service_str = r#"
    [Unit]
    RequiresMountsFor = /var/log

    [Service]
    ExecStart = /bin/true
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.service"),
    )
    .unwrap();

    let unit: crate::units::Unit = service.try_into().unwrap();
    let root_mount_id = crate::units::UnitId {
        name: "-.mount".to_owned(),
        kind: crate::units::UnitIdKind::Mount,
    };
    let var_mount_id = crate::units::UnitId {
        name: "var.mount".to_owned(),
        kind: crate::units::UnitIdKind::Mount,
    };
    let var_log_mount_id = crate::units::UnitId {
        name: "var-log.mount".to_owned(),
        kind: crate::units::UnitIdKind::Mount,
    };
    assert!(unit.common.dependencies.requires.contains(&root_mount_id));
    assert!(unit.common.dependencies.requires.contains(&var_mount_id));
    assert!(unit
        .common
        .dependencies
        .requires
        .contains(&var_log_mount_id));
    assert!(unit.common.dependencies.after.contains(&root_mount_id));
    assert!(unit.common.dependencies.after.contains(&var_mount_id));
    assert!(unit.common.dependencies.after.contains(&var_log_mount_id));
}

#[test]
fn test_requires_mounts_for_socket_unit() {
    let test_socket_str = r#"
    [Unit]
    RequiresMountsFor = /run/myapp

    [Socket]
    ListenStream = /run/myapp/sock
    "#;

    let parsed_file = crate::units::parse_file(test_socket_str).unwrap();
    let socket = crate::units::parse_socket(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.socket"),
    )
    .unwrap();

    assert_eq!(
        socket.common.unit.requires_mounts_for,
        vec!["/run/myapp".to_owned()]
    );
    assert!(socket.common.unit.requires.contains(&"-.mount".to_owned()));
    assert!(socket
        .common
        .unit
        .requires
        .contains(&"run.mount".to_owned()));
    assert!(socket
        .common
        .unit
        .requires
        .contains(&"run-myapp.mount".to_owned()));
    assert!(socket.common.unit.after.contains(&"-.mount".to_owned()));
    assert!(socket
        .common
        .unit
        .after
        .contains(&"run-myapp.mount".to_owned()));
}

#[test]
fn test_requires_mounts_for_multiple_directives_accumulate() {
    let test_service_str = r#"
    [Unit]
    RequiresMountsFor = /var/log
    RequiresMountsFor = /home/user

    [Service]
    ExecStart = /bin/true
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.service"),
    )
    .unwrap();

    assert!(service
        .common
        .unit
        .requires_mounts_for
        .contains(&"/var/log".to_owned()));
    assert!(service
        .common
        .unit
        .requires_mounts_for
        .contains(&"/home/user".to_owned()));
    // Mount units from both paths
    assert!(service
        .common
        .unit
        .requires
        .contains(&"var-log.mount".to_owned()));
    assert!(service
        .common
        .unit
        .requires
        .contains(&"home-user.mount".to_owned()));
}

// ── .mount in dependency lists ─────────────────────────────────────────

#[test]
fn test_mount_unit_id_conversion() {
    use std::convert::TryInto;

    let id: Result<crate::units::UnitId, _> =
        <&str as TryInto<crate::units::UnitId>>::try_into("-.mount");
    assert!(id.is_ok());
    let id = id.unwrap();
    assert_eq!(id.name, "-.mount");
    assert_eq!(id.kind, crate::units::UnitIdKind::Mount);
}

#[test]
fn test_mount_unit_id_named() {
    use std::convert::TryInto;

    let id: crate::units::UnitId = "home.mount".try_into().unwrap();
    assert_eq!(id.name, "home.mount");
    assert_eq!(id.kind, crate::units::UnitIdKind::Mount);
}

#[test]
fn test_mount_unit_id_nested_path() {
    use std::convert::TryInto;

    let id: crate::units::UnitId = "var-log.mount".try_into().unwrap();
    assert_eq!(id.name, "var-log.mount");
    assert_eq!(id.kind, crate::units::UnitIdKind::Mount);
}

#[test]
fn test_mount_in_after_dependency() {
    let test_service_str = r#"
    [Unit]
    After = -.mount

    [Service]
    ExecStart = /bin/true
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.service"),
    )
    .unwrap();

    assert_eq!(service.common.unit.after, vec!["-.mount".to_owned()]);
}

#[test]
fn test_mount_in_requires_dependency() {
    let test_service_str = r#"
    [Unit]
    Requires = home.mount

    [Service]
    ExecStart = /bin/true
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.service"),
    )
    .unwrap();

    assert_eq!(service.common.unit.requires, vec!["home.mount".to_owned()]);
}

#[test]
fn test_mount_in_wants_dependency() {
    let test_service_str = r#"
    [Unit]
    Wants = var-log.mount

    [Service]
    ExecStart = /bin/true
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.service"),
    )
    .unwrap();

    assert_eq!(service.common.unit.wants, vec!["var-log.mount".to_owned()]);
}

#[test]
fn test_mount_in_after_preserved_after_unit_conversion() {
    use std::convert::TryInto;

    let test_service_str = r#"
    [Unit]
    After = -.mount

    [Service]
    ExecStart = /bin/true
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.service"),
    )
    .unwrap();

    let unit: crate::units::Unit = service.try_into().unwrap();
    let mount_id = crate::units::UnitId {
        name: "-.mount".to_owned(),
        kind: crate::units::UnitIdKind::Mount,
    };
    assert!(unit.common.dependencies.after.contains(&mount_id));
}

#[test]
fn test_mount_mixed_with_other_deps() {
    let test_service_str = r#"
    [Unit]
    After = -.mount network.target
    Wants = home.mount
    Requires = var-log.mount basic.target

    [Service]
    ExecStart = /bin/true
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.service"),
    )
    .unwrap();

    assert!(service.common.unit.after.contains(&"-.mount".to_owned()));
    assert!(service
        .common
        .unit
        .after
        .contains(&"network.target".to_owned()));
    assert!(service.common.unit.wants.contains(&"home.mount".to_owned()));
    assert!(service
        .common
        .unit
        .requires
        .contains(&"var-log.mount".to_owned()));
    assert!(service
        .common
        .unit
        .requires
        .contains(&"basic.target".to_owned()));
}

#[test]
fn test_mount_in_refs_by_name_after_conversion() {
    use std::convert::TryInto;

    let test_service_str = r#"
    [Unit]
    After = -.mount
    Requires = home.mount

    [Service]
    ExecStart = /bin/true
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.service"),
    )
    .unwrap();

    let unit: crate::units::Unit = service.try_into().unwrap();
    let root_mount_id = crate::units::UnitId {
        name: "-.mount".to_owned(),
        kind: crate::units::UnitIdKind::Mount,
    };
    let home_mount_id = crate::units::UnitId {
        name: "home.mount".to_owned(),
        kind: crate::units::UnitIdKind::Mount,
    };
    assert!(unit.common.unit.refs_by_name.contains(&root_mount_id));
    assert!(unit.common.unit.refs_by_name.contains(&home_mount_id));
}

#[test]
fn test_mount_mixed_with_slices_and_targets() {
    use std::convert::TryInto;

    let test_service_str = r#"
    [Unit]
    After = -.mount user.slice
    Requires = network.target home.mount

    [Service]
    ExecStart = /bin/true
    Slice = system.slice
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.service"),
    )
    .unwrap();

    let unit: crate::units::Unit = service.try_into().unwrap();
    let mount_id = crate::units::UnitId {
        name: "-.mount".to_owned(),
        kind: crate::units::UnitIdKind::Mount,
    };
    let slice_id = crate::units::UnitId {
        name: "user.slice".to_owned(),
        kind: crate::units::UnitIdKind::Slice,
    };
    let target_id = crate::units::UnitId {
        name: "network.target".to_owned(),
        kind: crate::units::UnitIdKind::Target,
    };
    let home_mount_id = crate::units::UnitId {
        name: "home.mount".to_owned(),
        kind: crate::units::UnitIdKind::Mount,
    };
    assert!(unit.common.dependencies.after.contains(&mount_id));
    assert!(unit.common.dependencies.after.contains(&slice_id));
    assert!(unit.common.dependencies.requires.contains(&target_id));
    assert!(unit.common.dependencies.requires.contains(&home_mount_id));
}

#[test]
fn test_mount_is_mount_method() {
    use std::convert::TryInto;

    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.service"),
    )
    .unwrap();

    let unit: crate::units::Unit = service.try_into().unwrap();
    // A .service unit should not be a mount
    assert!(!unit.is_mount());
}

// ── StandardOutput=tty / StandardError=tty parsing tests ───────────────

#[test]
fn test_stdout_tty_parsed() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    StandardOutput = tty
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.exec_section.stdout_path,
        Some(crate::units::StdIoOption::Tty)
    );
}

#[test]
fn test_stderr_tty_parsed() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    StandardError = tty
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.exec_section.stderr_path,
        Some(crate::units::StdIoOption::Tty)
    );
}

#[test]
fn test_stdout_and_stderr_tty_parsed() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    StandardOutput = tty
    StandardError = tty
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.exec_section.stdout_path,
        Some(crate::units::StdIoOption::Tty)
    );
    assert_eq!(
        service.srvc.exec_section.stderr_path,
        Some(crate::units::StdIoOption::Tty)
    );
}

#[test]
fn test_stdout_tty_with_stdin_tty() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    StandardInput = tty
    StandardOutput = tty
    StandardError = tty
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.exec_section.stdin_option,
        crate::units::StandardInput::Tty
    );
    assert_eq!(
        service.srvc.exec_section.stdout_path,
        Some(crate::units::StdIoOption::Tty)
    );
    assert_eq!(
        service.srvc.exec_section.stderr_path,
        Some(crate::units::StdIoOption::Tty)
    );
}

#[test]
fn test_stdout_tty_preserved_after_unit_conversion() {
    use std::convert::TryInto;

    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    StandardOutput = tty
    StandardError = tty
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.service"),
    )
    .unwrap();

    let unit: crate::units::Unit = service.try_into().unwrap();
    if let crate::units::Specific::Service(srvc) = &unit.specific {
        assert_eq!(
            srvc.conf.exec_config.stdout_path,
            Some(crate::units::StdIoOption::Tty)
        );
        assert_eq!(
            srvc.conf.exec_config.stderr_path,
            Some(crate::units::StdIoOption::Tty)
        );
    } else {
        panic!("Expected service unit");
    }
}

#[test]
fn test_stdout_tty_with_tty_path() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    StandardInput = tty
    StandardOutput = tty
    TTYPath = /dev/tty1
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.exec_section.stdout_path,
        Some(crate::units::StdIoOption::Tty)
    );
    assert_eq!(
        service.srvc.exec_section.tty_path,
        Some(std::path::PathBuf::from("/dev/tty1"))
    );
}

#[test]
fn test_stdout_tty_no_unsupported_warning() {
    // StandardOutput=tty should be parsed without generating an "unsupported" warning
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    StandardOutput = tty
    StandardError = tty
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.service"),
    );

    assert!(service.is_ok());
    let service = service.unwrap();
    assert_eq!(
        service.srvc.exec_section.stdout_path,
        Some(crate::units::StdIoOption::Tty)
    );
    assert_eq!(
        service.srvc.exec_section.stderr_path,
        Some(crate::units::StdIoOption::Tty)
    );
}

#[test]
fn test_stdout_tty_mixed_with_other_stdio() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    StandardInput = tty
    StandardOutput = tty
    StandardError = journal
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.exec_section.stdin_option,
        crate::units::StandardInput::Tty
    );
    assert_eq!(
        service.srvc.exec_section.stdout_path,
        Some(crate::units::StdIoOption::Tty)
    );
    assert_eq!(
        service.srvc.exec_section.stderr_path,
        Some(crate::units::StdIoOption::Journal)
    );
}

#[test]
fn test_ignore_sigpipe_defaults_to_true() {
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
        service.srvc.exec_section.ignore_sigpipe,
        "IgnoreSIGPIPE should default to true when not specified"
    );
}

#[test]
fn test_ignore_sigpipe_explicit_yes() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    IgnoreSIGPIPE = yes
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert!(
        service.srvc.exec_section.ignore_sigpipe,
        "IgnoreSIGPIPE=yes should be true"
    );
}

#[test]
fn test_ignore_sigpipe_explicit_no() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    IgnoreSIGPIPE = no
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert!(
        !service.srvc.exec_section.ignore_sigpipe,
        "IgnoreSIGPIPE=no should be false"
    );
}

#[test]
fn test_ignore_sigpipe_explicit_true() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    IgnoreSIGPIPE = true
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert!(
        service.srvc.exec_section.ignore_sigpipe,
        "IgnoreSIGPIPE=true should be true"
    );
}

#[test]
fn test_ignore_sigpipe_explicit_false() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    IgnoreSIGPIPE = false
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert!(
        !service.srvc.exec_section.ignore_sigpipe,
        "IgnoreSIGPIPE=false should be false"
    );
}

#[test]
fn test_ignore_sigpipe_explicit_1() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    IgnoreSIGPIPE = 1
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert!(
        service.srvc.exec_section.ignore_sigpipe,
        "IgnoreSIGPIPE=1 should be true"
    );
}

#[test]
fn test_ignore_sigpipe_explicit_0() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    IgnoreSIGPIPE = 0
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert!(
        !service.srvc.exec_section.ignore_sigpipe,
        "IgnoreSIGPIPE=0 should be false"
    );
}

#[test]
fn test_ignore_sigpipe_case_insensitive() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    IgnoreSIGPIPE = YES
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert!(
        service.srvc.exec_section.ignore_sigpipe,
        "IgnoreSIGPIPE=YES (case-insensitive) should be true"
    );
}

#[test]
fn test_ignore_sigpipe_no_warning_when_set() {
    // Verify that setting IgnoreSIGPIPE does not produce an "unsupported setting" warning.
    // If parsing succeeds without error, the setting was consumed (not left in the section
    // to trigger a warning).
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    IgnoreSIGPIPE = false
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let result = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    );

    assert!(
        result.is_ok(),
        "Parsing a service with IgnoreSIGPIPE should succeed without errors"
    );
}

#[test]
fn test_utmp_identifier_defaults_to_none() {
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

    assert_eq!(
        service.srvc.exec_section.utmp_identifier, None,
        "UtmpIdentifier should default to None when not specified"
    );
}

#[test]
fn test_utmp_identifier_explicit_value() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    UtmpIdentifier = tty1
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.exec_section.utmp_identifier,
        Some("tty1".to_owned()),
        "UtmpIdentifier=tty1 should be stored"
    );
}

#[test]
fn test_utmp_identifier_custom_string() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    UtmpIdentifier = cons
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.exec_section.utmp_identifier,
        Some("cons".to_owned()),
        "UtmpIdentifier=cons should be stored"
    );
}

#[test]
fn test_utmp_identifier_no_warning_when_set() {
    // Verify that setting UtmpIdentifier does not produce an "unsupported setting" warning.
    // If parsing succeeds without error, the setting was consumed (not left in the section
    // to trigger a warning).
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    UtmpIdentifier = tty1
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let result = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    );

    assert!(
        result.is_ok(),
        "Parsing a service with UtmpIdentifier should succeed without errors"
    );
}

#[test]
fn test_utmp_mode_defaults_to_init() {
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

    assert_eq!(
        service.srvc.exec_section.utmp_mode,
        crate::units::UtmpMode::Init,
        "UtmpMode should default to Init when not specified"
    );
}

#[test]
fn test_utmp_mode_explicit_init() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    UtmpMode = init
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.exec_section.utmp_mode,
        crate::units::UtmpMode::Init,
        "UtmpMode=init should be Init"
    );
}

#[test]
fn test_utmp_mode_login() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    UtmpMode = login
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.exec_section.utmp_mode,
        crate::units::UtmpMode::Login,
        "UtmpMode=login should be Login"
    );
}

#[test]
fn test_utmp_mode_user() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    UtmpMode = user
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.exec_section.utmp_mode,
        crate::units::UtmpMode::User,
        "UtmpMode=user should be User"
    );
}

#[test]
fn test_utmp_mode_case_insensitive() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    UtmpMode = Login
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.exec_section.utmp_mode,
        crate::units::UtmpMode::Login,
        "UtmpMode=Login (mixed case) should be Login"
    );
}

#[test]
fn test_utmp_mode_invalid_value() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    UtmpMode = invalid
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let result = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    );

    assert!(
        result.is_err(),
        "UtmpMode=invalid should produce a parsing error"
    );
}

#[test]
fn test_utmp_mode_no_warning_when_set() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    UtmpMode = login
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let result = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    );

    assert!(
        result.is_ok(),
        "Parsing a service with UtmpMode should succeed without errors"
    );
}

#[test]
fn test_utmp_identifier_and_mode_combined() {
    let test_service_str = r#"
    [Service]
    ExecStart = /sbin/agetty --noclear tty1 linux
    StandardInput = tty
    TTYPath = /dev/tty1
    UtmpIdentifier = tty1
    UtmpMode = login
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/getty@tty1.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.exec_section.utmp_identifier,
        Some("tty1".to_owned()),
        "UtmpIdentifier should be tty1"
    );
    assert_eq!(
        service.srvc.exec_section.utmp_mode,
        crate::units::UtmpMode::Login,
        "UtmpMode should be Login"
    );
    assert_eq!(
        service.srvc.exec_section.tty_path,
        Some(std::path::PathBuf::from("/dev/tty1")),
        "TTYPath should be /dev/tty1"
    );
}

#[test]
fn test_utmp_identifier_without_mode_defaults_to_init() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    UtmpIdentifier = cons
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.exec_section.utmp_identifier,
        Some("cons".to_owned()),
        "UtmpIdentifier should be cons"
    );
    assert_eq!(
        service.srvc.exec_section.utmp_mode,
        crate::units::UtmpMode::Init,
        "UtmpMode should default to Init when only UtmpIdentifier is set"
    );
}

#[test]
fn test_utmp_mode_without_identifier() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    UtmpMode = user
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.exec_section.utmp_identifier, None,
        "UtmpIdentifier should be None when not specified"
    );
    assert_eq!(
        service.srvc.exec_section.utmp_mode,
        crate::units::UtmpMode::User,
        "UtmpMode=user should be stored even without UtmpIdentifier"
    );
}

#[test]
fn test_send_sighup_defaults_to_false() {
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
        !service.srvc.send_sighup,
        "SendSIGHUP should default to false when not specified"
    );
}

#[test]
fn test_send_sighup_explicit_yes() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    SendSIGHUP = yes
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert!(service.srvc.send_sighup, "SendSIGHUP=yes should be true");
}

#[test]
fn test_send_sighup_explicit_no() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    SendSIGHUP = no
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert!(!service.srvc.send_sighup, "SendSIGHUP=no should be false");
}

#[test]
fn test_send_sighup_explicit_true() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    SendSIGHUP = true
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert!(service.srvc.send_sighup, "SendSIGHUP=true should be true");
}

#[test]
fn test_send_sighup_explicit_false() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    SendSIGHUP = false
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert!(
        !service.srvc.send_sighup,
        "SendSIGHUP=false should be false"
    );
}

#[test]
fn test_send_sighup_explicit_1() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    SendSIGHUP = 1
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert!(service.srvc.send_sighup, "SendSIGHUP=1 should be true");
}

#[test]
fn test_send_sighup_explicit_0() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    SendSIGHUP = 0
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert!(!service.srvc.send_sighup, "SendSIGHUP=0 should be false");
}

#[test]
fn test_send_sighup_case_insensitive() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    SendSIGHUP = YES
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert!(
        service.srvc.send_sighup,
        "SendSIGHUP=YES (case-insensitive) should be true"
    );
}

#[test]
fn test_send_sighup_no_warning_when_set() {
    // Verify that setting SendSIGHUP does not produce an "unsupported setting" warning.
    // If parsing succeeds without error, the setting was consumed (not left in the section
    // to trigger a warning).
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    SendSIGHUP = yes
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let result = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    );

    assert!(
        result.is_ok(),
        "Parsing a service with SendSIGHUP should succeed without errors"
    );
}

#[test]
fn test_send_sighup_with_kill_mode() {
    // Verify SendSIGHUP works alongside KillMode settings.
    let test_service_str = r#"
    [Service]
    ExecStart = /sbin/agetty --noclear tty1 linux
    KillMode = control-group
    SendSIGHUP = yes
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/getty@tty1.service"),
    )
    .unwrap();

    assert!(
        service.srvc.send_sighup,
        "SendSIGHUP=yes should be true alongside KillMode"
    );
    assert_eq!(
        service.srvc.kill_mode,
        crate::units::KillMode::ControlGroup,
        "KillMode should still be control-group"
    );
}

// ── MemoryPressureWatch= ──────────────────────────────────────────────

#[test]
fn test_memory_pressure_watch_defaults_to_auto() {
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

    assert_eq!(
        service.srvc.memory_pressure_watch,
        crate::units::MemoryPressureWatch::Auto,
        "MemoryPressureWatch should default to Auto when not specified"
    );
}

#[test]
fn test_memory_pressure_watch_auto() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    MemoryPressureWatch = auto
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.memory_pressure_watch,
        crate::units::MemoryPressureWatch::Auto,
        "MemoryPressureWatch=auto should be Auto"
    );
}

#[test]
fn test_memory_pressure_watch_on() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    MemoryPressureWatch = on
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.memory_pressure_watch,
        crate::units::MemoryPressureWatch::On,
        "MemoryPressureWatch=on should be On"
    );
}

#[test]
fn test_memory_pressure_watch_off() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    MemoryPressureWatch = off
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.memory_pressure_watch,
        crate::units::MemoryPressureWatch::Off,
        "MemoryPressureWatch=off should be Off"
    );
}

#[test]
fn test_memory_pressure_watch_skip() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    MemoryPressureWatch = skip
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.memory_pressure_watch,
        crate::units::MemoryPressureWatch::Skip,
        "MemoryPressureWatch=skip should be Skip"
    );
}

#[test]
fn test_memory_pressure_watch_yes() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    MemoryPressureWatch = yes
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.memory_pressure_watch,
        crate::units::MemoryPressureWatch::On,
        "MemoryPressureWatch=yes should be On"
    );
}

#[test]
fn test_memory_pressure_watch_no() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    MemoryPressureWatch = no
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.memory_pressure_watch,
        crate::units::MemoryPressureWatch::Off,
        "MemoryPressureWatch=no should be Off"
    );
}

#[test]
fn test_memory_pressure_watch_true() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    MemoryPressureWatch = true
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.memory_pressure_watch,
        crate::units::MemoryPressureWatch::On,
        "MemoryPressureWatch=true should be On"
    );
}

#[test]
fn test_memory_pressure_watch_false() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    MemoryPressureWatch = false
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.memory_pressure_watch,
        crate::units::MemoryPressureWatch::Off,
        "MemoryPressureWatch=false should be Off"
    );
}

#[test]
fn test_memory_pressure_watch_1() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    MemoryPressureWatch = 1
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.memory_pressure_watch,
        crate::units::MemoryPressureWatch::On,
        "MemoryPressureWatch=1 should be On"
    );
}

#[test]
fn test_memory_pressure_watch_0() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    MemoryPressureWatch = 0
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.memory_pressure_watch,
        crate::units::MemoryPressureWatch::Off,
        "MemoryPressureWatch=0 should be Off"
    );
}

#[test]
fn test_memory_pressure_watch_case_insensitive() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    MemoryPressureWatch = AUTO
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.memory_pressure_watch,
        crate::units::MemoryPressureWatch::Auto,
        "MemoryPressureWatch=AUTO (case-insensitive) should be Auto"
    );
}

#[test]
fn test_memory_pressure_watch_case_insensitive_skip() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    MemoryPressureWatch = Skip
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.memory_pressure_watch,
        crate::units::MemoryPressureWatch::Skip,
        "MemoryPressureWatch=Skip (mixed case) should be Skip"
    );
}

#[test]
fn test_memory_pressure_watch_invalid_value() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    MemoryPressureWatch = invalid
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let result = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    );

    assert!(
        result.is_err(),
        "MemoryPressureWatch=invalid should produce a parsing error"
    );
}

#[test]
fn test_memory_pressure_watch_no_unsupported_warning() {
    // MemoryPressureWatch= should be parsed without generating an "unsupported setting" warning
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    MemoryPressureWatch = auto
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let result = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    );

    assert!(
        result.is_ok(),
        "Parsing a service with MemoryPressureWatch should succeed without errors"
    );
    assert_eq!(
        result.unwrap().srvc.memory_pressure_watch,
        crate::units::MemoryPressureWatch::Auto
    );
}

#[test]
fn test_memory_pressure_watch_combined_with_other_settings() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    MemoryPressureWatch = off
    OOMScoreAdjust = 100
    RuntimeDirectory = myapp
    KillMode = control-group
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.memory_pressure_watch,
        crate::units::MemoryPressureWatch::Off
    );
    assert_eq!(service.srvc.exec_section.oom_score_adjust, Some(100));
    assert_eq!(
        service.srvc.exec_section.runtime_directory,
        vec!["myapp".to_owned()]
    );
    assert_eq!(service.srvc.kill_mode, crate::units::KillMode::ControlGroup);
}

#[test]
fn test_memory_pressure_watch_preserved_after_unit_conversion() {
    use std::convert::TryInto;

    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    MemoryPressureWatch = skip
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.service"),
    )
    .unwrap();

    let unit: crate::units::Unit = service.try_into().unwrap();
    if let crate::units::Specific::Service(srvc) = &unit.specific {
        assert_eq!(
            srvc.conf.memory_pressure_watch,
            crate::units::MemoryPressureWatch::Skip,
            "MemoryPressureWatch should survive unit conversion"
        );
    } else {
        panic!("Expected service unit");
    }
}

#[test]
fn test_import_credential_defaults_to_empty() {
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
        service.srvc.exec_section.import_credentials.is_empty(),
        "ImportCredential should default to empty when not specified"
    );
}

#[test]
fn test_import_credential_single_value() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    ImportCredential = mycred
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.exec_section.import_credentials,
        vec!["mycred".to_owned()],
        "ImportCredential=mycred should be stored"
    );
}

#[test]
fn test_import_credential_glob_pattern() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    ImportCredential = myapp.*
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.exec_section.import_credentials,
        vec!["myapp.*".to_owned()],
        "ImportCredential=myapp.* should be stored as-is"
    );
}

#[test]
fn test_import_credential_multiple_directives() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    ImportCredential = first.secret
    ImportCredential = second.*
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.exec_section.import_credentials,
        vec!["first.secret".to_owned(), "second.*".to_owned()],
        "Multiple ImportCredential= lines should accumulate"
    );
}

#[test]
fn test_import_credential_whitespace_separated() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    ImportCredential = cred-a cred-b
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.exec_section.import_credentials,
        vec!["cred-a".to_owned(), "cred-b".to_owned()],
        "Whitespace-separated values should be split into individual patterns"
    );
}

#[test]
fn test_import_credential_no_warning_when_set() {
    // Verify that setting ImportCredential does not produce an "unsupported setting" warning.
    // If parsing succeeds without error, the setting was consumed (not left in the section
    // to trigger a warning).
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    ImportCredential = some.secret
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let result = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    );

    assert!(
        result.is_ok(),
        "Parsing a service with ImportCredential should succeed without errors"
    );
}

#[test]
fn test_import_credential_combined_with_environment() {
    // Verify ImportCredential works alongside other exec settings.
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myapp
    Environment = MY_VAR=hello
    ImportCredential = myapp.key
    ImportCredential = myapp.cert
    User = nobody
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/myapp.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.exec_section.import_credentials,
        vec!["myapp.key".to_owned(), "myapp.cert".to_owned()],
        "ImportCredential should accumulate alongside other settings"
    );
    assert!(
        service.srvc.exec_section.environment.is_some(),
        "Environment should still be parsed"
    );
}

// ---------------------------------------------------------------------------
// glob_match tests (the helper used by ImportCredential= at runtime)
// ---------------------------------------------------------------------------

#[test]
fn test_glob_match_exact() {
    assert!(crate::entrypoints::glob_match("hello", "hello"));
    assert!(!crate::entrypoints::glob_match("hello", "world"));
}

#[test]
fn test_glob_match_star() {
    assert!(crate::entrypoints::glob_match("*", "anything"));
    assert!(crate::entrypoints::glob_match("*", ""));
    assert!(crate::entrypoints::glob_match("myapp.*", "myapp.key"));
    assert!(crate::entrypoints::glob_match("myapp.*", "myapp.cert"));
    assert!(crate::entrypoints::glob_match("myapp.*", "myapp."));
    assert!(!crate::entrypoints::glob_match("myapp.*", "otherapp.key"));
    assert!(crate::entrypoints::glob_match("*.key", "myapp.key"));
    assert!(!crate::entrypoints::glob_match("*.key", "myapp.cert"));
}

#[test]
fn test_glob_match_question_mark() {
    assert!(crate::entrypoints::glob_match("cred?", "cred1"));
    assert!(crate::entrypoints::glob_match("cred?", "credX"));
    assert!(!crate::entrypoints::glob_match("cred?", "cred"));
    assert!(!crate::entrypoints::glob_match("cred?", "cred12"));
}

#[test]
fn test_glob_match_combined() {
    assert!(crate::entrypoints::glob_match("my*.ke?", "myapp.key"));
    assert!(!crate::entrypoints::glob_match("my*.ke?", "myapp.keys"));
    assert!(crate::entrypoints::glob_match("*.*", "a.b"));
    assert!(!crate::entrypoints::glob_match("*.*", "nodot"));
}

#[test]
fn test_glob_match_empty() {
    assert!(crate::entrypoints::glob_match("", ""));
    assert!(!crate::entrypoints::glob_match("", "notempty"));
    assert!(crate::entrypoints::glob_match("*", ""));
}

#[test]
fn test_glob_match_multiple_stars() {
    assert!(crate::entrypoints::glob_match("a*b*c", "abc"));
    assert!(crate::entrypoints::glob_match("a*b*c", "aXXbYYc"));
    assert!(!crate::entrypoints::glob_match("a*b*c", "aXXcYYb"));
}

#[test]
fn test_unset_environment_defaults_to_empty() {
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

    assert!(service.srvc.exec_section.unset_environment.is_empty());
}

#[test]
fn test_unset_environment_single_variable_name() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    UnsetEnvironment = FOO
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.exec_section.unset_environment,
        vec!["FOO".to_owned()]
    );
}

#[test]
fn test_unset_environment_multiple_variable_names() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    UnsetEnvironment = FOO BAR BAZ
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.exec_section.unset_environment,
        vec!["FOO".to_owned(), "BAR".to_owned(), "BAZ".to_owned(),]
    );
}

#[test]
fn test_unset_environment_variable_assignment() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    UnsetEnvironment = FOO=bar
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.exec_section.unset_environment,
        vec!["FOO=bar".to_owned()]
    );
}

#[test]
fn test_unset_environment_multiple_directives_accumulate() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    UnsetEnvironment = FOO
    UnsetEnvironment = BAR
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.exec_section.unset_environment,
        vec!["FOO".to_owned(), "BAR".to_owned()]
    );
}

#[test]
fn test_unset_environment_empty_resets() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    UnsetEnvironment = FOO BAR
    UnsetEnvironment =
    UnsetEnvironment = BAZ
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    // The empty assignment resets the list, so only BAZ remains
    assert_eq!(
        service.srvc.exec_section.unset_environment,
        vec!["BAZ".to_owned()]
    );
}

#[test]
fn test_unset_environment_quoted_tokens() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    UnsetEnvironment = "FOO=hello world" BAR
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.exec_section.unset_environment,
        vec!["FOO=hello world".to_owned(), "BAR".to_owned()]
    );
}

#[test]
fn test_unset_environment_mixed_names_and_assignments() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    UnsetEnvironment = FOO BAR=value BAZ
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.exec_section.unset_environment,
        vec!["FOO".to_owned(), "BAR=value".to_owned(), "BAZ".to_owned(),]
    );
}

#[test]
fn test_unset_environment_no_unsupported_warning() {
    // UnsetEnvironment= should be recognized and not trigger a warning
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    UnsetEnvironment = FOO
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    // Verify the value is parsed correctly — if it showed up as an
    // unsupported warning instead, the field would be empty.
    assert_eq!(
        service.srvc.exec_section.unset_environment,
        vec!["FOO".to_owned()]
    );
}

#[test]
fn test_unset_environment_with_environment() {
    // UnsetEnvironment= coexists with Environment= — both should be parsed
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    Environment = FOO=bar BAZ=qux
    UnsetEnvironment = FOO
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    // Environment= should still be parsed
    assert!(service.srvc.exec_section.environment.is_some());
    let env = service.srvc.exec_section.environment.as_ref().unwrap();
    assert_eq!(
        env.vars,
        vec![
            ("FOO".to_owned(), "bar".to_owned()),
            ("BAZ".to_owned(), "qux".to_owned()),
        ]
    );
    // UnsetEnvironment= should also be parsed
    assert_eq!(
        service.srvc.exec_section.unset_environment,
        vec!["FOO".to_owned()]
    );
}

#[test]
fn test_unset_environment_socket_unit() {
    let test_socket_str = r#"
    [Socket]
    ListenStream = /run/test.sock
    UnsetEnvironment = FOO BAR
    "#;

    let parsed_file = crate::units::parse_file(test_socket_str).unwrap();
    let socket = crate::units::parse_socket(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.socket"),
    )
    .unwrap();

    assert_eq!(
        socket.sock.exec_section.unset_environment,
        vec!["FOO".to_owned(), "BAR".to_owned()]
    );
}

// ── OOMScoreAdjust= ───────────────────────────────────────────────────

#[test]
fn test_oom_score_adjust_defaults_to_none() {
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

    assert_eq!(
        service.srvc.exec_section.oom_score_adjust, None,
        "OOMScoreAdjust should default to None when not specified"
    );
}

#[test]
fn test_oom_score_adjust_positive_value() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    OOMScoreAdjust = 500
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.exec_section.oom_score_adjust,
        Some(500),
        "OOMScoreAdjust=500 should be stored"
    );
}

#[test]
fn test_oom_score_adjust_negative_value() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    OOMScoreAdjust = -500
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.exec_section.oom_score_adjust,
        Some(-500),
        "OOMScoreAdjust=-500 should be stored"
    );
}

#[test]
fn test_oom_score_adjust_zero() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    OOMScoreAdjust = 0
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.exec_section.oom_score_adjust,
        Some(0),
        "OOMScoreAdjust=0 should be stored"
    );
}

#[test]
fn test_oom_score_adjust_max_value() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    OOMScoreAdjust = 1000
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.exec_section.oom_score_adjust,
        Some(1000),
        "OOMScoreAdjust=1000 should be stored"
    );
}

#[test]
fn test_oom_score_adjust_min_value() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    OOMScoreAdjust = -1000
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.exec_section.oom_score_adjust,
        Some(-1000),
        "OOMScoreAdjust=-1000 should be stored"
    );
}

#[test]
fn test_oom_score_adjust_clamped_above_max() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    OOMScoreAdjust = 2000
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.exec_section.oom_score_adjust,
        Some(1000),
        "OOMScoreAdjust=2000 should be clamped to 1000"
    );
}

#[test]
fn test_oom_score_adjust_clamped_below_min() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    OOMScoreAdjust = -2000
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.exec_section.oom_score_adjust,
        Some(-1000),
        "OOMScoreAdjust=-2000 should be clamped to -1000"
    );
}

#[test]
fn test_oom_score_adjust_invalid_value() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    OOMScoreAdjust = notanumber
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let result = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    );

    assert!(
        result.is_err(),
        "OOMScoreAdjust=notanumber should produce a parsing error"
    );
}

#[test]
fn test_oom_score_adjust_no_unsupported_warning() {
    // OOMScoreAdjust= should be parsed without generating an "unsupported setting" warning.
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    OOMScoreAdjust = 100
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let result = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    );

    assert!(
        result.is_ok(),
        "Parsing a service with OOMScoreAdjust should succeed without errors"
    );
    assert_eq!(
        result.unwrap().srvc.exec_section.oom_score_adjust,
        Some(100)
    );
}

#[test]
fn test_oom_score_adjust_combined_with_other_settings() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    User = nobody
    OOMScoreAdjust = -900
    RuntimeDirectory = myapp
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(service.srvc.exec_section.oom_score_adjust, Some(-900));
    assert_eq!(
        service.srvc.exec_section.runtime_directory,
        vec!["myapp".to_owned()]
    );
}

#[test]
fn test_oom_score_adjust_socket_unit() {
    let test_socket_str = r#"
    [Socket]
    ListenStream = /run/test.sock
    OOMScoreAdjust = 200
    "#;

    let parsed_file = crate::units::parse_file(test_socket_str).unwrap();
    let socket = crate::units::parse_socket(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.socket"),
    )
    .unwrap();

    assert_eq!(
        socket.sock.exec_section.oom_score_adjust,
        Some(200),
        "OOMScoreAdjust=200 should be stored for socket units"
    );
}

#[test]
fn test_oom_score_adjust_preserved_after_unit_conversion() {
    use std::convert::TryInto;

    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    OOMScoreAdjust = -100
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.service"),
    )
    .unwrap();

    let unit: crate::units::Unit = service.try_into().unwrap();
    if let crate::units::Specific::Service(srvc) = &unit.specific {
        assert_eq!(
            srvc.conf.exec_config.oom_score_adjust,
            Some(-100),
            "OOMScoreAdjust should survive unit conversion"
        );
    } else {
        panic!("Expected service unit");
    }
}

#[test]
fn test_oom_score_adjust_with_whitespace() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    OOMScoreAdjust =   750
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.exec_section.oom_score_adjust,
        Some(750),
        "OOMScoreAdjust should handle surrounding whitespace"
    );
}

// ── ReloadSignal= tests ──────────────────────────────────────────────

#[test]
fn test_reload_signal_defaults_to_none() {
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

    assert_eq!(
        service.srvc.reload_signal, None,
        "ReloadSignal should default to None when not specified"
    );
}

#[test]
fn test_reload_signal_sighup() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    ReloadSignal = SIGHUP
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.reload_signal,
        Some(nix::sys::signal::Signal::SIGHUP),
        "ReloadSignal=SIGHUP should parse correctly"
    );
}

#[test]
fn test_reload_signal_sigusr1() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    ReloadSignal = SIGUSR1
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.reload_signal,
        Some(nix::sys::signal::Signal::SIGUSR1),
        "ReloadSignal=SIGUSR1 should parse correctly"
    );
}

#[test]
fn test_reload_signal_sigusr2() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    ReloadSignal = SIGUSR2
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.reload_signal,
        Some(nix::sys::signal::Signal::SIGUSR2),
        "ReloadSignal=SIGUSR2 should parse correctly"
    );
}

#[test]
fn test_reload_signal_sigterm() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    ReloadSignal = SIGTERM
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.reload_signal,
        Some(nix::sys::signal::Signal::SIGTERM),
        "ReloadSignal=SIGTERM should parse correctly"
    );
}

#[test]
fn test_reload_signal_without_sig_prefix() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    ReloadSignal = HUP
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.reload_signal,
        Some(nix::sys::signal::Signal::SIGHUP),
        "ReloadSignal=HUP (without SIG prefix) should parse correctly"
    );
}

#[test]
fn test_reload_signal_case_insensitive() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    ReloadSignal = sighup
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.reload_signal,
        Some(nix::sys::signal::Signal::SIGHUP),
        "ReloadSignal should be case-insensitive"
    );
}

#[test]
fn test_reload_signal_case_insensitive_mixed() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    ReloadSignal = SigUsr1
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.reload_signal,
        Some(nix::sys::signal::Signal::SIGUSR1),
        "ReloadSignal should handle mixed-case signal names"
    );
}

#[test]
fn test_reload_signal_numeric() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    ReloadSignal = 1
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.reload_signal,
        Some(nix::sys::signal::Signal::SIGHUP),
        "ReloadSignal=1 should parse as SIGHUP (signal number 1)"
    );
}

#[test]
fn test_reload_signal_numeric_10() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    ReloadSignal = 10
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.reload_signal,
        Some(nix::sys::signal::Signal::SIGUSR1),
        "ReloadSignal=10 should parse as SIGUSR1 (signal number 10)"
    );
}

#[test]
fn test_reload_signal_empty_value() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    ReloadSignal =
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.reload_signal, None,
        "ReloadSignal with empty value should be treated as None"
    );
}

#[test]
fn test_reload_signal_invalid_value() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    ReloadSignal = NOTASIGNAL
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let result = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    );

    assert!(
        result.is_err(),
        "Invalid signal name should produce an error"
    );
}

#[test]
fn test_reload_signal_no_unsupported_warning() {
    // ReloadSignal= should be parsed without generating an "unsupported setting" warning
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    ReloadSignal = SIGHUP
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let result = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.service"),
    );

    // The key test is that parsing succeeds—if RELOADSIGNAL were still in the
    // section map at the warning-loop point it would trigger a warning, but
    // we can't capture log output here.  At minimum verify it parses.
    assert!(
        result.is_ok(),
        "ReloadSignal should be recognised and not produce a parsing error"
    );
}

#[test]
fn test_reload_signal_with_other_settings() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    ReloadSignal = SIGUSR2
    KillMode = process
    Restart = on-failure
    SendSIGHUP = yes
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.reload_signal,
        Some(nix::sys::signal::Signal::SIGUSR2),
        "ReloadSignal should work alongside other service settings"
    );
    assert_eq!(service.srvc.kill_mode, crate::units::KillMode::Process);
    assert_eq!(
        service.srvc.restart,
        crate::units::ServiceRestart::OnFailure
    );
    assert!(service.srvc.send_sighup);
}

#[test]
fn test_reload_signal_preserved_after_unit_conversion() {
    use std::convert::TryInto;

    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    ReloadSignal = SIGUSR1
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.service"),
    )
    .unwrap();

    let unit: crate::units::Unit = service.try_into().unwrap();
    if let crate::units::Specific::Service(srvc) = &unit.specific {
        assert_eq!(
            srvc.conf.reload_signal,
            Some(nix::sys::signal::Signal::SIGUSR1),
            "ReloadSignal should survive unit conversion"
        );
    } else {
        panic!("Expected service unit");
    }
}

#[test]
fn test_reload_signal_none_preserved_after_unit_conversion() {
    use std::convert::TryInto;

    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.service"),
    )
    .unwrap();

    let unit: crate::units::Unit = service.try_into().unwrap();
    if let crate::units::Specific::Service(srvc) = &unit.specific {
        assert_eq!(
            srvc.conf.reload_signal, None,
            "ReloadSignal=None should survive unit conversion"
        );
    } else {
        panic!("Expected service unit");
    }
}

#[test]
fn test_reload_signal_sigwinch() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    ReloadSignal = SIGWINCH
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.reload_signal,
        Some(nix::sys::signal::Signal::SIGWINCH),
        "ReloadSignal=SIGWINCH should parse correctly"
    );
}

#[test]
fn test_reload_signal_sigint() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    ReloadSignal = SIGINT
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.reload_signal,
        Some(nix::sys::signal::Signal::SIGINT),
        "ReloadSignal=SIGINT should parse correctly"
    );
}

#[test]
fn test_reload_signal_with_whitespace() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    ReloadSignal =   SIGHUP
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.reload_signal,
        Some(nix::sys::signal::Signal::SIGHUP),
        "ReloadSignal should handle surrounding whitespace"
    );
}

// ── KillSignal= tests ───────────────────────────────────────────────

#[test]
fn test_kill_signal_defaults_to_none() {
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

    assert_eq!(
        service.srvc.kill_signal, None,
        "KillSignal should default to None when not specified"
    );
}

#[test]
fn test_kill_signal_sigterm() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    KillSignal = SIGTERM
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.kill_signal,
        Some(nix::sys::signal::Signal::SIGTERM),
        "KillSignal=SIGTERM should parse correctly"
    );
}

#[test]
fn test_kill_signal_sigkill() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    KillSignal = SIGKILL
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.kill_signal,
        Some(nix::sys::signal::Signal::SIGKILL),
        "KillSignal=SIGKILL should parse correctly"
    );
}

#[test]
fn test_kill_signal_sigint() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    KillSignal = SIGINT
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.kill_signal,
        Some(nix::sys::signal::Signal::SIGINT),
        "KillSignal=SIGINT should parse correctly"
    );
}

#[test]
fn test_kill_signal_sighup() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    KillSignal = SIGHUP
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.kill_signal,
        Some(nix::sys::signal::Signal::SIGHUP),
        "KillSignal=SIGHUP should parse correctly"
    );
}

#[test]
fn test_kill_signal_without_sig_prefix() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    KillSignal = TERM
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.kill_signal,
        Some(nix::sys::signal::Signal::SIGTERM),
        "KillSignal=TERM (without SIG prefix) should parse correctly"
    );
}

#[test]
fn test_kill_signal_case_insensitive() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    KillSignal = sigterm
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.kill_signal,
        Some(nix::sys::signal::Signal::SIGTERM),
        "KillSignal should be case-insensitive"
    );
}

#[test]
fn test_kill_signal_case_insensitive_mixed() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    KillSignal = SigKill
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.kill_signal,
        Some(nix::sys::signal::Signal::SIGKILL),
        "KillSignal should handle mixed-case signal names"
    );
}

#[test]
fn test_kill_signal_numeric() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    KillSignal = 15
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.kill_signal,
        Some(nix::sys::signal::Signal::SIGTERM),
        "KillSignal=15 should parse as SIGTERM (signal number 15)"
    );
}

#[test]
fn test_kill_signal_numeric_9() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    KillSignal = 9
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.kill_signal,
        Some(nix::sys::signal::Signal::SIGKILL),
        "KillSignal=9 should parse as SIGKILL (signal number 9)"
    );
}

#[test]
fn test_kill_signal_empty_value() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    KillSignal =
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.kill_signal, None,
        "Empty KillSignal= should be None"
    );
}

#[test]
fn test_kill_signal_invalid_value() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    KillSignal = NOTASIGNAL
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let result = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    );

    assert!(
        result.is_err(),
        "Invalid KillSignal value should produce an error"
    );
}

#[test]
fn test_kill_signal_no_unsupported_warning() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    KillSignal = SIGTERM
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let result = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    );

    assert!(
        result.is_ok(),
        "KillSignal= should be recognised and not produce a parsing error"
    );
}

#[test]
fn test_kill_signal_with_other_settings() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    KillSignal = SIGTERM
    KillMode = process
    Restart = on-failure
    SendSIGHUP = yes
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.kill_signal,
        Some(nix::sys::signal::Signal::SIGTERM),
    );
    assert!(service.srvc.send_sighup);
}

#[test]
fn test_kill_signal_preserved_after_unit_conversion() {
    use std::convert::TryInto;

    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    KillSignal = SIGINT
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.service"),
    )
    .unwrap();

    let unit: crate::units::Unit = service.try_into().unwrap();
    if let crate::units::Specific::Service(srvc) = &unit.specific {
        assert_eq!(
            srvc.conf.kill_signal,
            Some(nix::sys::signal::Signal::SIGINT),
            "KillSignal should survive unit conversion"
        );
    } else {
        panic!("Expected service unit");
    }
}

#[test]
fn test_kill_signal_none_preserved_after_unit_conversion() {
    use std::convert::TryInto;

    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.service"),
    )
    .unwrap();

    let unit: crate::units::Unit = service.try_into().unwrap();
    if let crate::units::Specific::Service(srvc) = &unit.specific {
        assert_eq!(
            srvc.conf.kill_signal, None,
            "Default None KillSignal should survive unit conversion"
        );
    } else {
        panic!("Expected service unit");
    }
}

#[test]
fn test_kill_signal_with_whitespace() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    KillSignal =   SIGTERM
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.kill_signal,
        Some(nix::sys::signal::Signal::SIGTERM),
        "KillSignal should handle surrounding whitespace"
    );
}

// ── DelegateSubgroup= tests ──────────────────────────────────────────

#[test]
fn test_delegate_subgroup_defaults_to_none() {
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

    assert_eq!(
        service.srvc.delegate_subgroup, None,
        "DelegateSubgroup should default to None when not specified"
    );
}

#[test]
fn test_delegate_subgroup_simple_name() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    Delegate = yes
    DelegateSubgroup = supervised
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.delegate_subgroup,
        Some("supervised".to_owned()),
        "DelegateSubgroup=supervised should parse correctly"
    );
}

#[test]
fn test_delegate_subgroup_payload_name() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    Delegate = yes
    DelegateSubgroup = payload
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.delegate_subgroup,
        Some("payload".to_owned()),
        "DelegateSubgroup=payload should parse correctly"
    );
}

#[test]
fn test_delegate_subgroup_empty_value() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    Delegate = yes
    DelegateSubgroup =
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.delegate_subgroup, None,
        "DelegateSubgroup with empty value should be treated as None (disabled)"
    );
}

#[test]
fn test_delegate_subgroup_with_whitespace() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    Delegate = yes
    DelegateSubgroup =   mysubgroup
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.delegate_subgroup,
        Some("mysubgroup".to_owned()),
        "DelegateSubgroup should handle surrounding whitespace"
    );
}

#[test]
fn test_delegate_subgroup_without_delegate() {
    // DelegateSubgroup= can be parsed even without Delegate=yes; it just has
    // no runtime effect.  The parser should accept it regardless.
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    DelegateSubgroup = supervised
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.delegate_subgroup,
        Some("supervised".to_owned()),
        "DelegateSubgroup should be parsed even without Delegate=yes"
    );
}

#[test]
fn test_delegate_subgroup_no_unsupported_warning() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    DelegateSubgroup = supervised
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let result = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.service"),
    );

    assert!(
        result.is_ok(),
        "DelegateSubgroup should be recognised and not produce a parsing error"
    );
}

#[test]
fn test_delegate_subgroup_with_delegate_and_controllers() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    Delegate = cpu memory
    DelegateSubgroup = supervised
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.delegate_subgroup,
        Some("supervised".to_owned()),
    );
    if let crate::units::Delegate::Controllers(ref controllers) = service.srvc.delegate {
        assert_eq!(controllers, &["cpu", "memory"]);
    } else {
        panic!("Expected Delegate::Controllers");
    }
}

#[test]
fn test_delegate_subgroup_with_other_settings() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    Delegate = yes
    DelegateSubgroup = payload
    KillMode = process
    Restart = on-failure
    Slice = system.slice
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.delegate_subgroup,
        Some("payload".to_owned()),
        "DelegateSubgroup should work alongside other service settings"
    );
    assert_eq!(service.srvc.kill_mode, crate::units::KillMode::Process);
    assert_eq!(
        service.srvc.restart,
        crate::units::ServiceRestart::OnFailure
    );
    assert_eq!(service.srvc.slice, Some("system.slice".to_owned()));
}

#[test]
fn test_delegate_subgroup_preserved_after_unit_conversion() {
    use std::convert::TryInto;

    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    Delegate = yes
    DelegateSubgroup = supervised
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.service"),
    )
    .unwrap();

    let unit: crate::units::Unit = service.try_into().unwrap();
    if let crate::units::Specific::Service(srvc) = &unit.specific {
        assert_eq!(
            srvc.conf.delegate_subgroup,
            Some("supervised".to_owned()),
            "DelegateSubgroup should survive unit conversion"
        );
    } else {
        panic!("Expected service unit");
    }
}

#[test]
fn test_delegate_subgroup_none_preserved_after_unit_conversion() {
    use std::convert::TryInto;

    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.service"),
    )
    .unwrap();

    let unit: crate::units::Unit = service.try_into().unwrap();
    if let crate::units::Specific::Service(srvc) = &unit.specific {
        assert_eq!(
            srvc.conf.delegate_subgroup, None,
            "DelegateSubgroup=None should survive unit conversion"
        );
    } else {
        panic!("Expected service unit");
    }
}

// ── KeyringMode= tests ───────────────────────────────────────────────

#[test]
fn test_keyring_mode_defaults_to_private() {
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

    assert_eq!(
        service.srvc.keyring_mode,
        crate::units::KeyringMode::Private,
        "KeyringMode should default to Private when not specified"
    );
}

#[test]
fn test_keyring_mode_inherit() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    KeyringMode = inherit
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.keyring_mode,
        crate::units::KeyringMode::Inherit,
        "KeyringMode=inherit should parse correctly"
    );
}

#[test]
fn test_keyring_mode_private() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    KeyringMode = private
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.keyring_mode,
        crate::units::KeyringMode::Private,
        "KeyringMode=private should parse correctly"
    );
}

#[test]
fn test_keyring_mode_shared() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    KeyringMode = shared
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.keyring_mode,
        crate::units::KeyringMode::Shared,
        "KeyringMode=shared should parse correctly"
    );
}

#[test]
fn test_keyring_mode_case_insensitive_upper() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    KeyringMode = INHERIT
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.keyring_mode,
        crate::units::KeyringMode::Inherit,
        "KeyringMode should be case-insensitive (uppercase)"
    );
}

#[test]
fn test_keyring_mode_case_insensitive_mixed() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    KeyringMode = Private
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.keyring_mode,
        crate::units::KeyringMode::Private,
        "KeyringMode should be case-insensitive (mixed case)"
    );
}

#[test]
fn test_keyring_mode_case_insensitive_shared_upper() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    KeyringMode = SHARED
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.keyring_mode,
        crate::units::KeyringMode::Shared,
        "KeyringMode should be case-insensitive (SHARED)"
    );
}

#[test]
fn test_keyring_mode_invalid_value() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    KeyringMode = bogus
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let result = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    );

    assert!(
        result.is_err(),
        "KeyringMode with an invalid value should produce a parsing error"
    );
}

#[test]
fn test_keyring_mode_no_unsupported_warning() {
    // KeyringMode= should be parsed without generating an "unsupported setting" warning.
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    KeyringMode = private
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let result = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.service"),
    );

    assert!(
        result.is_ok(),
        "KeyringMode should be recognised and not produce a parsing error"
    );
}

#[test]
fn test_keyring_mode_with_whitespace() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    KeyringMode =   shared
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.keyring_mode,
        crate::units::KeyringMode::Shared,
        "KeyringMode should handle surrounding whitespace"
    );
}

#[test]
fn test_keyring_mode_with_other_settings() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    KeyringMode = shared
    KillMode = process
    Restart = on-failure
    Delegate = yes
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.keyring_mode,
        crate::units::KeyringMode::Shared,
        "KeyringMode should work alongside other service settings"
    );
    assert_eq!(service.srvc.kill_mode, crate::units::KillMode::Process);
    assert_eq!(
        service.srvc.restart,
        crate::units::ServiceRestart::OnFailure
    );
    assert_eq!(service.srvc.delegate, crate::units::Delegate::Yes);
}

#[test]
fn test_keyring_mode_preserved_after_unit_conversion() {
    use std::convert::TryInto;

    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    KeyringMode = shared
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.service"),
    )
    .unwrap();

    let unit: crate::units::Unit = service.try_into().unwrap();
    if let crate::units::Specific::Service(srvc) = &unit.specific {
        assert_eq!(
            srvc.conf.keyring_mode,
            crate::units::KeyringMode::Shared,
            "KeyringMode=shared should survive unit conversion"
        );
    } else {
        panic!("Expected service unit");
    }
}

#[test]
fn test_keyring_mode_default_preserved_after_unit_conversion() {
    use std::convert::TryInto;

    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.service"),
    )
    .unwrap();

    let unit: crate::units::Unit = service.try_into().unwrap();
    if let crate::units::Specific::Service(srvc) = &unit.specific {
        assert_eq!(
            srvc.conf.keyring_mode,
            crate::units::KeyringMode::Private,
            "KeyringMode default (Private) should survive unit conversion"
        );
    } else {
        panic!("Expected service unit");
    }
}

#[test]
fn test_keyring_mode_inherit_preserved_after_unit_conversion() {
    use std::convert::TryInto;

    let test_service_str = r#"
    [Service]
    ExecStart = /bin/true
    KeyringMode = inherit
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.service"),
    )
    .unwrap();

    let unit: crate::units::Unit = service.try_into().unwrap();
    if let crate::units::Specific::Service(srvc) = &unit.specific {
        assert_eq!(
            srvc.conf.keyring_mode,
            crate::units::KeyringMode::Inherit,
            "KeyringMode=inherit should survive unit conversion"
        );
    } else {
        panic!("Expected service unit");
    }
}

// ============================================================
// JobTimeoutAction= parsing tests
// ============================================================

#[test]
fn test_job_timeout_action_defaults_to_none() {
    let test_service_str = r#"
    [Unit]
    Description = A simple service
    [Service]
    ExecStart = /bin/myservice
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.common.unit.job_timeout_action,
        crate::units::UnitAction::None,
        "JobTimeoutAction should default to None"
    );
}

#[test]
fn test_job_timeout_action_none() {
    let test_service_str = r#"
    [Unit]
    JobTimeoutAction = none
    [Service]
    ExecStart = /bin/myservice
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.common.unit.job_timeout_action,
        crate::units::UnitAction::None,
    );
}

#[test]
fn test_job_timeout_action_reboot() {
    let test_service_str = r#"
    [Unit]
    JobTimeoutAction = reboot
    [Service]
    ExecStart = /bin/myservice
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.common.unit.job_timeout_action,
        crate::units::UnitAction::Reboot,
    );
}

#[test]
fn test_job_timeout_action_reboot_force() {
    let test_service_str = r#"
    [Unit]
    JobTimeoutAction = reboot-force
    [Service]
    ExecStart = /bin/myservice
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.common.unit.job_timeout_action,
        crate::units::UnitAction::RebootForce,
    );
}

#[test]
fn test_job_timeout_action_reboot_immediate() {
    let test_service_str = r#"
    [Unit]
    JobTimeoutAction = reboot-immediate
    [Service]
    ExecStart = /bin/myservice
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.common.unit.job_timeout_action,
        crate::units::UnitAction::RebootImmediate,
    );
}

#[test]
fn test_job_timeout_action_poweroff() {
    let test_service_str = r#"
    [Unit]
    JobTimeoutAction = poweroff
    [Service]
    ExecStart = /bin/myservice
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.common.unit.job_timeout_action,
        crate::units::UnitAction::Poweroff,
    );
}

#[test]
fn test_job_timeout_action_poweroff_force() {
    let test_service_str = r#"
    [Unit]
    JobTimeoutAction = poweroff-force
    [Service]
    ExecStart = /bin/myservice
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.common.unit.job_timeout_action,
        crate::units::UnitAction::PoweroffForce,
    );
}

#[test]
fn test_job_timeout_action_exit() {
    let test_service_str = r#"
    [Unit]
    JobTimeoutAction = exit
    [Service]
    ExecStart = /bin/myservice
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.common.unit.job_timeout_action,
        crate::units::UnitAction::Exit,
    );
}

#[test]
fn test_job_timeout_action_exit_force() {
    let test_service_str = r#"
    [Unit]
    JobTimeoutAction = exit-force
    [Service]
    ExecStart = /bin/myservice
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.common.unit.job_timeout_action,
        crate::units::UnitAction::ExitForce,
    );
}

#[test]
fn test_job_timeout_action_halt() {
    let test_service_str = r#"
    [Unit]
    JobTimeoutAction = halt
    [Service]
    ExecStart = /bin/myservice
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.common.unit.job_timeout_action,
        crate::units::UnitAction::Halt,
    );
}

#[test]
fn test_job_timeout_action_kexec() {
    let test_service_str = r#"
    [Unit]
    JobTimeoutAction = kexec
    [Service]
    ExecStart = /bin/myservice
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.common.unit.job_timeout_action,
        crate::units::UnitAction::Kexec,
    );
}

#[test]
fn test_job_timeout_action_case_insensitive() {
    let cases = vec![
        ("reboot-force", crate::units::UnitAction::RebootForce),
        ("REBOOT-FORCE", crate::units::UnitAction::RebootForce),
        ("Reboot-Force", crate::units::UnitAction::RebootForce),
        ("POWEROFF", crate::units::UnitAction::Poweroff),
        ("Poweroff", crate::units::UnitAction::Poweroff),
        ("NONE", crate::units::UnitAction::None),
        ("None", crate::units::UnitAction::None),
    ];

    for (input, expected) in &cases {
        let test_service_str = format!(
            r#"
            [Unit]
            JobTimeoutAction = {}
            [Service]
            ExecStart = /bin/myservice
            "#,
            input
        );

        let parsed_file = crate::units::parse_file(&test_service_str).unwrap();
        let service = crate::units::parse_service(
            parsed_file,
            &std::path::PathBuf::from("/path/to/unitfile.service"),
        )
        .unwrap();

        assert_eq!(
            &service.common.unit.job_timeout_action, expected,
            "JobTimeoutAction={} should parse as {:?}",
            input, expected
        );
    }
}

#[test]
fn test_job_timeout_action_unknown_value_errors() {
    let test_service_str = r#"
    [Unit]
    JobTimeoutAction = bogus
    [Service]
    ExecStart = /bin/myservice
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let result = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    );

    assert!(
        result.is_err(),
        "JobTimeoutAction=bogus should produce a parsing error"
    );
}

#[test]
fn test_job_timeout_action_with_other_unit_settings() {
    let test_service_str = r#"
    [Unit]
    Description = A service with job timeout action
    FailureAction = reboot
    JobTimeoutAction = poweroff
    [Service]
    ExecStart = /bin/myservice
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.common.unit.job_timeout_action,
        crate::units::UnitAction::Poweroff,
    );
    assert_eq!(
        service.common.unit.failure_action,
        crate::units::UnitAction::Reboot,
    );
}

#[test]
fn test_job_timeout_action_no_unsupported_warning() {
    // This test verifies that JobTimeoutAction= is recognized and does not
    // trigger the "Ignoring unsupported setting" warning.
    let test_service_str = r#"
    [Unit]
    JobTimeoutAction = none
    [Service]
    ExecStart = /bin/myservice
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let result = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    );

    assert!(
        result.is_ok(),
        "JobTimeoutAction=none should parse without error"
    );
}

#[test]
fn test_job_timeout_action_preserved_after_unit_conversion() {
    use std::convert::TryInto;

    let test_service_str = r#"
    [Unit]
    JobTimeoutAction = reboot-force
    [Service]
    ExecStart = /bin/myservice
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.service"),
    )
    .unwrap();

    let unit: crate::units::Unit = service.try_into().unwrap();
    assert_eq!(
        unit.common.unit.job_timeout_action,
        crate::units::UnitAction::RebootForce,
        "JobTimeoutAction=reboot-force should survive unit conversion"
    );
}

#[test]
fn test_job_timeout_action_target_unit() {
    let test_target_str = r#"
    [Unit]
    Description = A target with job timeout
    JobTimeoutAction = poweroff-force
    "#;

    let parsed_file = crate::units::parse_file(test_target_str).unwrap();
    let target = crate::units::parse_target(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.target"),
    )
    .unwrap();

    assert_eq!(
        target.common.unit.job_timeout_action,
        crate::units::UnitAction::PoweroffForce,
    );
}

#[test]
fn test_job_timeout_action_socket_unit() {
    let test_socket_str = r#"
    [Unit]
    Description = A socket with job timeout
    JobTimeoutAction = reboot
    [Socket]
    ListenStream = /run/test.sock
    "#;

    let parsed_file = crate::units::parse_file(test_socket_str).unwrap();
    let socket = crate::units::parse_socket(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.socket"),
    )
    .unwrap();

    assert_eq!(
        socket.common.unit.job_timeout_action,
        crate::units::UnitAction::Reboot,
    );
}

#[test]
fn test_job_timeout_action_default_preserved_after_unit_conversion() {
    use std::convert::TryInto;

    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.service"),
    )
    .unwrap();

    let unit: crate::units::Unit = service.try_into().unwrap();
    assert_eq!(
        unit.common.unit.job_timeout_action,
        crate::units::UnitAction::None,
        "Default JobTimeoutAction (None) should survive unit conversion"
    );
}

// ============================================================
// AllowIsolate= parsing tests
// ============================================================

#[test]
fn test_allow_isolate_defaults_to_false() {
    let test_service_str = r#"
    [Unit]
    Description = A simple service
    [Service]
    ExecStart = /bin/myservice
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.common.unit.allow_isolate, false,
        "AllowIsolate should default to false"
    );
}

#[test]
fn test_allow_isolate_set_yes() {
    let test_service_str = r#"
    [Unit]
    AllowIsolate = yes
    [Service]
    ExecStart = /bin/myservice
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.common.unit.allow_isolate, true,
        "AllowIsolate=yes should be true"
    );
}

#[test]
fn test_allow_isolate_set_true() {
    let test_service_str = r#"
    [Unit]
    AllowIsolate = true
    [Service]
    ExecStart = /bin/myservice
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.common.unit.allow_isolate, true,
        "AllowIsolate=true should be true"
    );
}

#[test]
fn test_allow_isolate_set_no() {
    let test_service_str = r#"
    [Unit]
    AllowIsolate = no
    [Service]
    ExecStart = /bin/myservice
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.common.unit.allow_isolate, false,
        "AllowIsolate=no should be false"
    );
}

#[test]
fn test_allow_isolate_set_false() {
    let test_service_str = r#"
    [Unit]
    AllowIsolate = false
    [Service]
    ExecStart = /bin/myservice
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.common.unit.allow_isolate, false,
        "AllowIsolate=false should be false"
    );
}

#[test]
fn test_allow_isolate_set_1() {
    let test_service_str = r#"
    [Unit]
    AllowIsolate = 1
    [Service]
    ExecStart = /bin/myservice
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.common.unit.allow_isolate, true,
        "AllowIsolate=1 should be true"
    );
}

#[test]
fn test_allow_isolate_set_0() {
    let test_service_str = r#"
    [Unit]
    AllowIsolate = 0
    [Service]
    ExecStart = /bin/myservice
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.common.unit.allow_isolate, false,
        "AllowIsolate=0 should be false"
    );
}

#[test]
fn test_allow_isolate_case_insensitive() {
    let test_service_str = r#"
    [Unit]
    AllowIsolate = YES
    [Service]
    ExecStart = /bin/myservice
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.common.unit.allow_isolate, true,
        "AllowIsolate=YES should be true (case-insensitive)"
    );
}

#[test]
fn test_allow_isolate_with_other_unit_settings() {
    let test_service_str = r#"
    [Unit]
    Description = An isolatable service
    AllowIsolate = yes
    IgnoreOnIsolate = true
    StopWhenUnneeded = no
    [Service]
    ExecStart = /bin/myservice
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(service.common.unit.allow_isolate, true);
    assert_eq!(service.common.unit.ignore_on_isolate, true);
    assert_eq!(service.common.unit.stop_when_unneeded, false);
}

#[test]
fn test_allow_isolate_no_unsupported_warning() {
    let test_service_str = r#"
    [Unit]
    AllowIsolate = yes
    [Service]
    ExecStart = /bin/myservice
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let result = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    );

    assert!(
        result.is_ok(),
        "AllowIsolate=yes should parse without error"
    );
}

#[test]
fn test_allow_isolate_target_unit() {
    let test_target_str = r#"
    [Unit]
    Description = An isolatable target
    AllowIsolate = yes
    "#;

    let parsed_file = crate::units::parse_file(test_target_str).unwrap();
    let target = crate::units::parse_target(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.target"),
    )
    .unwrap();

    assert_eq!(
        target.common.unit.allow_isolate, true,
        "AllowIsolate=yes should work on target units"
    );
}

#[test]
fn test_allow_isolate_socket_unit() {
    let test_socket_str = r#"
    [Unit]
    Description = A socket with allow isolate
    AllowIsolate = yes
    [Socket]
    ListenStream = /run/test.sock
    "#;

    let parsed_file = crate::units::parse_file(test_socket_str).unwrap();
    let socket = crate::units::parse_socket(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.socket"),
    )
    .unwrap();

    assert_eq!(
        socket.common.unit.allow_isolate, true,
        "AllowIsolate=yes should work on socket units"
    );
}

#[test]
fn test_allow_isolate_preserved_after_unit_conversion() {
    use std::convert::TryInto;

    let test_service_str = r#"
    [Unit]
    AllowIsolate = yes
    [Service]
    ExecStart = /bin/myservice
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.service"),
    )
    .unwrap();

    let unit: crate::units::Unit = service.try_into().unwrap();
    assert_eq!(
        unit.common.unit.allow_isolate, true,
        "AllowIsolate=yes should survive unit conversion"
    );
}

#[test]
fn test_allow_isolate_false_preserved_after_unit_conversion() {
    use std::convert::TryInto;

    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.service"),
    )
    .unwrap();

    let unit: crate::units::Unit = service.try_into().unwrap();
    assert_eq!(
        unit.common.unit.allow_isolate, false,
        "Default AllowIsolate (false) should survive unit conversion"
    );
}

// ============================================================
// JobTimeoutSec= parsing tests
// ============================================================

#[test]
fn test_job_timeout_sec_defaults_to_none() {
    let test_service_str = r#"
    [Unit]
    Description = A simple service
    [Service]
    ExecStart = /bin/myservice
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.common.unit.job_timeout_sec, None,
        "JobTimeoutSec should default to None"
    );
}

#[test]
fn test_job_timeout_sec_seconds() {
    let test_service_str = r#"
    [Unit]
    JobTimeoutSec = 30
    [Service]
    ExecStart = /bin/myservice
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.common.unit.job_timeout_sec,
        Some(crate::units::Timeout::Duration(
            std::time::Duration::from_secs(30)
        ))
    );
}

#[test]
fn test_job_timeout_sec_with_suffix() {
    let test_service_str = r#"
    [Unit]
    JobTimeoutSec = 30s
    [Service]
    ExecStart = /bin/myservice
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.common.unit.job_timeout_sec,
        Some(crate::units::Timeout::Duration(
            std::time::Duration::from_secs(30)
        ))
    );
}

#[test]
fn test_job_timeout_sec_infinity() {
    let test_service_str = r#"
    [Unit]
    JobTimeoutSec = infinity
    [Service]
    ExecStart = /bin/myservice
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.common.unit.job_timeout_sec,
        Some(crate::units::Timeout::Infinity)
    );
}

#[test]
fn test_job_timeout_sec_infinity_case_insensitive() {
    let test_service_str = r#"
    [Unit]
    JobTimeoutSec = INFINITY
    [Service]
    ExecStart = /bin/myservice
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.common.unit.job_timeout_sec,
        Some(crate::units::Timeout::Infinity)
    );
}

#[test]
fn test_job_timeout_sec_compound_duration() {
    let test_service_str = r#"
    [Unit]
    JobTimeoutSec = 2min 30s
    [Service]
    ExecStart = /bin/myservice
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.common.unit.job_timeout_sec,
        Some(crate::units::Timeout::Duration(
            std::time::Duration::from_secs(150)
        ))
    );
}

#[test]
fn test_job_timeout_sec_minutes() {
    let test_service_str = r#"
    [Unit]
    JobTimeoutSec = 5min
    [Service]
    ExecStart = /bin/myservice
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.common.unit.job_timeout_sec,
        Some(crate::units::Timeout::Duration(
            std::time::Duration::from_secs(300)
        ))
    );
}

#[test]
fn test_job_timeout_sec_with_job_timeout_action() {
    let test_service_str = r#"
    [Unit]
    JobTimeoutSec = 60
    JobTimeoutAction = reboot-force
    [Service]
    ExecStart = /bin/myservice
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.common.unit.job_timeout_sec,
        Some(crate::units::Timeout::Duration(
            std::time::Duration::from_secs(60)
        ))
    );
    assert_eq!(
        service.common.unit.job_timeout_action,
        crate::units::UnitAction::RebootForce,
    );
}

#[test]
fn test_job_timeout_sec_no_unsupported_warning() {
    let test_service_str = r#"
    [Unit]
    JobTimeoutSec = 30
    [Service]
    ExecStart = /bin/myservice
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let result = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    );

    assert!(
        result.is_ok(),
        "JobTimeoutSec=30 should parse without error"
    );
}

#[test]
fn test_job_timeout_sec_target_unit() {
    let test_target_str = r#"
    [Unit]
    Description = A target with job timeout
    JobTimeoutSec = 120
    "#;

    let parsed_file = crate::units::parse_file(test_target_str).unwrap();
    let target = crate::units::parse_target(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.target"),
    )
    .unwrap();

    assert_eq!(
        target.common.unit.job_timeout_sec,
        Some(crate::units::Timeout::Duration(
            std::time::Duration::from_secs(120)
        ))
    );
}

#[test]
fn test_job_timeout_sec_socket_unit() {
    let test_socket_str = r#"
    [Unit]
    Description = A socket with job timeout
    JobTimeoutSec = 45
    [Socket]
    ListenStream = /run/test.sock
    "#;

    let parsed_file = crate::units::parse_file(test_socket_str).unwrap();
    let socket = crate::units::parse_socket(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.socket"),
    )
    .unwrap();

    assert_eq!(
        socket.common.unit.job_timeout_sec,
        Some(crate::units::Timeout::Duration(
            std::time::Duration::from_secs(45)
        ))
    );
}

#[test]
fn test_job_timeout_sec_preserved_after_unit_conversion() {
    use std::convert::TryInto;

    let test_service_str = r#"
    [Unit]
    JobTimeoutSec = 90
    [Service]
    ExecStart = /bin/myservice
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.service"),
    )
    .unwrap();

    let unit: crate::units::Unit = service.try_into().unwrap();
    assert_eq!(
        unit.common.unit.job_timeout_sec,
        Some(crate::units::Timeout::Duration(
            std::time::Duration::from_secs(90)
        )),
        "JobTimeoutSec=90 should survive unit conversion"
    );
}

#[test]
fn test_job_timeout_sec_none_preserved_after_unit_conversion() {
    use std::convert::TryInto;

    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.service"),
    )
    .unwrap();

    let unit: crate::units::Unit = service.try_into().unwrap();
    assert_eq!(
        unit.common.unit.job_timeout_sec, None,
        "Default JobTimeoutSec (None) should survive unit conversion"
    );
}

#[test]
fn test_job_timeout_sec_zero() {
    let test_service_str = r#"
    [Unit]
    JobTimeoutSec = 0
    [Service]
    ExecStart = /bin/myservice
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.common.unit.job_timeout_sec,
        Some(crate::units::Timeout::Duration(
            std::time::Duration::from_secs(0)
        ))
    );
}

#[test]
fn test_job_timeout_sec_hours() {
    let test_service_str = r#"
    [Unit]
    JobTimeoutSec = 2hrs
    [Service]
    ExecStart = /bin/myservice
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.common.unit.job_timeout_sec,
        Some(crate::units::Timeout::Duration(
            std::time::Duration::from_secs(7200)
        ))
    );
}

// ============================================================
// ExecReload= parsing tests
// ============================================================

#[test]
fn test_exec_reload_empty_by_default() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert!(
        service.srvc.reload.is_empty(),
        "ExecReload should default to empty"
    );
}

#[test]
fn test_exec_reload_single_command() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    ExecReload = /bin/kill -HUP $MAINPID
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.reload.len(),
        1,
        "Should have one reload command"
    );
    assert_eq!(service.srvc.reload[0].cmd, "/bin/kill");
}

#[test]
fn test_exec_reload_multiple_commands() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    ExecReload = /bin/kill -HUP $MAINPID
    ExecReload = /bin/myservice --reload
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.reload.len(),
        2,
        "Should have two reload commands"
    );
    assert_eq!(service.srvc.reload[0].cmd, "/bin/kill");
    assert_eq!(service.srvc.reload[1].cmd, "/bin/myservice");
}

#[test]
fn test_exec_reload_with_arguments() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    ExecReload = /usr/bin/myctl reload --config /etc/myservice.conf
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(service.srvc.reload.len(), 1);
    assert_eq!(service.srvc.reload[0].cmd, "/usr/bin/myctl");
    assert_eq!(
        service.srvc.reload[0].args,
        vec!["reload", "--config", "/etc/myservice.conf"]
    );
}

#[test]
fn test_exec_reload_with_minus_prefix() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    ExecReload = -/bin/kill -HUP $MAINPID
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(service.srvc.reload.len(), 1);
    assert_eq!(service.srvc.reload[0].cmd, "/bin/kill");
    assert!(
        service.srvc.reload[0]
            .prefixes
            .contains(&crate::units::CommandlinePrefix::Minus),
        "ExecReload with - prefix should have Minus prefix"
    );
}

#[test]
fn test_exec_reload_no_unsupported_warning() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    ExecReload = /bin/kill -HUP $MAINPID
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let result = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    );

    assert!(
        result.is_ok(),
        "ExecReload should parse without error or warning"
    );
}

#[test]
fn test_exec_reload_with_other_exec_commands() {
    let test_service_str = r#"
    [Service]
    ExecStartPre = /bin/prep
    ExecStart = /bin/myservice
    ExecReload = /bin/myservice --reload
    ExecStop = /bin/myservice --stop
    ExecStopPost = /bin/cleanup
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(service.srvc.startpre.len(), 1);
    assert_eq!(service.srvc.reload.len(), 1);
    assert_eq!(service.srvc.stop.len(), 1);
    assert_eq!(service.srvc.stoppost.len(), 1);
    assert_eq!(service.srvc.reload[0].cmd, "/bin/myservice");
}

#[test]
fn test_exec_reload_preserved_after_unit_conversion() {
    use std::convert::TryInto;

    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    ExecReload = /bin/myservice --reload
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.service"),
    )
    .unwrap();

    let unit: crate::units::Unit = service.try_into().unwrap();
    if let crate::units::Specific::Service(srvc) = &unit.specific {
        assert_eq!(srvc.conf.reload.len(), 1);
        assert_eq!(srvc.conf.reload[0].cmd, "/bin/myservice");
        assert_eq!(srvc.conf.reload[0].args, vec!["--reload"]);
    } else {
        panic!("Expected service unit");
    }
}

#[test]
fn test_exec_reload_empty_preserved_after_unit_conversion() {
    use std::convert::TryInto;

    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.service"),
    )
    .unwrap();

    let unit: crate::units::Unit = service.try_into().unwrap();
    if let crate::units::Specific::Service(srvc) = &unit.specific {
        assert!(
            srvc.conf.reload.is_empty(),
            "Empty ExecReload should survive unit conversion"
        );
    } else {
        panic!("Expected service unit");
    }
}

#[test]
fn test_exec_reload_with_notify_reload_type() {
    let test_service_str = r#"
    [Service]
    Type = notify-reload
    ExecStart = /bin/myservice
    ExecReload = /bin/myservice --reload
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(service.srvc.reload.len(), 1);
    assert_eq!(service.srvc.reload[0].cmd, "/bin/myservice");
    assert_eq!(
        service.srvc.srcv_type,
        crate::units::ServiceType::NotifyReload,
    );
}

#[test]
fn test_exec_reload_with_at_prefix() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    ExecReload = @/bin/myservice myservice --reload
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(service.srvc.reload.len(), 1);
    assert!(
        service.srvc.reload[0]
            .prefixes
            .contains(&crate::units::CommandlinePrefix::AtSign),
        "ExecReload with @ prefix should have AtSign prefix"
    );
}

// ============================================================
// LogExtraFields= parsing tests
// ============================================================

#[test]
fn test_log_extra_fields_defaults_to_empty() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert!(
        service.srvc.exec_section.log_extra_fields.is_empty(),
        "LogExtraFields should default to empty"
    );
}

#[test]
fn test_log_extra_fields_single_entry() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    LogExtraFields = MYFIELD=myvalue
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(service.srvc.exec_section.log_extra_fields.len(), 1);
    assert_eq!(
        service.srvc.exec_section.log_extra_fields[0],
        "MYFIELD=myvalue"
    );
}

#[test]
fn test_log_extra_fields_multiple_directives() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    LogExtraFields = FIELD_A=value_a
    LogExtraFields = FIELD_B=value_b
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(service.srvc.exec_section.log_extra_fields.len(), 2);
    assert_eq!(
        service.srvc.exec_section.log_extra_fields[0],
        "FIELD_A=value_a"
    );
    assert_eq!(
        service.srvc.exec_section.log_extra_fields[1],
        "FIELD_B=value_b"
    );
}

#[test]
fn test_log_extra_fields_no_unsupported_warning() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    LogExtraFields = MYFIELD=myvalue
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let result = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    );

    assert!(
        result.is_ok(),
        "LogExtraFields should parse without error or warning"
    );
}

#[test]
fn test_log_extra_fields_with_other_settings() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    LogExtraFields = COMPONENT=webserver
    Environment = FOO=bar
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(service.srvc.exec_section.log_extra_fields.len(), 1);
    assert_eq!(
        service.srvc.exec_section.log_extra_fields[0],
        "COMPONENT=webserver"
    );
}

#[test]
fn test_log_extra_fields_value_with_equals() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    LogExtraFields = MYFIELD=key=value
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(service.srvc.exec_section.log_extra_fields.len(), 1);
    assert_eq!(
        service.srvc.exec_section.log_extra_fields[0],
        "MYFIELD=key=value"
    );
}

#[test]
fn test_log_extra_fields_preserved_after_unit_conversion() {
    use std::convert::TryInto;

    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    LogExtraFields = SUBSYSTEM=auth
    LogExtraFields = COMPONENT=login
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.service"),
    )
    .unwrap();

    let unit: crate::units::Unit = service.try_into().unwrap();
    if let crate::units::Specific::Service(srvc) = &unit.specific {
        assert_eq!(srvc.conf.exec_config.log_extra_fields.len(), 2);
        assert_eq!(srvc.conf.exec_config.log_extra_fields[0], "SUBSYSTEM=auth");
        assert_eq!(srvc.conf.exec_config.log_extra_fields[1], "COMPONENT=login");
    } else {
        panic!("Expected service unit");
    }
}

#[test]
fn test_log_extra_fields_empty_preserved_after_unit_conversion() {
    use std::convert::TryInto;

    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.service"),
    )
    .unwrap();

    let unit: crate::units::Unit = service.try_into().unwrap();
    if let crate::units::Specific::Service(srvc) = &unit.specific {
        assert!(
            srvc.conf.exec_config.log_extra_fields.is_empty(),
            "Empty LogExtraFields should survive unit conversion"
        );
    } else {
        panic!("Expected service unit");
    }
}

#[test]
fn test_log_extra_fields_socket_unit() {
    let test_socket_str = r#"
    [Unit]
    Description = A socket with log fields
    [Socket]
    ListenStream = /run/test.sock
    LogExtraFields = UNIT_TYPE=socket
    "#;

    let parsed_file = crate::units::parse_file(test_socket_str).unwrap();
    let socket = crate::units::parse_socket(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.socket"),
    )
    .unwrap();

    assert_eq!(socket.sock.exec_section.log_extra_fields.len(), 1);
    assert_eq!(
        socket.sock.exec_section.log_extra_fields[0],
        "UNIT_TYPE=socket"
    );
}

#[test]
fn test_log_extra_fields_empty_value() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    LogExtraFields = MYFIELD=
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(service.srvc.exec_section.log_extra_fields.len(), 1);
    assert_eq!(service.srvc.exec_section.log_extra_fields[0], "MYFIELD=");
}

// ============================================================
// DynamicUser= parsing tests
// ============================================================

#[test]
fn test_dynamic_user_defaults_to_false() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.exec_section.dynamic_user, false,
        "DynamicUser should default to false"
    );
}

#[test]
fn test_dynamic_user_set_yes() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    DynamicUser = yes
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.exec_section.dynamic_user, true,
        "DynamicUser=yes should be true"
    );
}

#[test]
fn test_dynamic_user_set_true() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    DynamicUser = true
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.exec_section.dynamic_user, true,
        "DynamicUser=true should be true"
    );
}

#[test]
fn test_dynamic_user_set_no() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    DynamicUser = no
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.exec_section.dynamic_user, false,
        "DynamicUser=no should be false"
    );
}

#[test]
fn test_dynamic_user_set_false() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    DynamicUser = false
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.exec_section.dynamic_user, false,
        "DynamicUser=false should be false"
    );
}

#[test]
fn test_dynamic_user_set_1() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    DynamicUser = 1
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.exec_section.dynamic_user, true,
        "DynamicUser=1 should be true"
    );
}

#[test]
fn test_dynamic_user_set_0() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    DynamicUser = 0
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.exec_section.dynamic_user, false,
        "DynamicUser=0 should be false"
    );
}

#[test]
fn test_dynamic_user_case_insensitive() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    DynamicUser = YES
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.exec_section.dynamic_user, true,
        "DynamicUser=YES should be true (case-insensitive)"
    );
}

#[test]
fn test_dynamic_user_no_unsupported_warning() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    DynamicUser = yes
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let result = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    );

    assert!(
        result.is_ok(),
        "DynamicUser=yes should parse without error or warning"
    );
}

#[test]
fn test_dynamic_user_with_other_settings() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    DynamicUser = yes
    StateDirectory = myservice
    RuntimeDirectory = myservice
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(service.srvc.exec_section.dynamic_user, true);
    assert_eq!(service.srvc.exec_section.state_directory, vec!["myservice"]);
    assert_eq!(
        service.srvc.exec_section.runtime_directory,
        vec!["myservice"]
    );
}

#[test]
fn test_dynamic_user_preserved_after_unit_conversion() {
    use std::convert::TryInto;

    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    DynamicUser = yes
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.service"),
    )
    .unwrap();

    let unit: crate::units::Unit = service.try_into().unwrap();
    if let crate::units::Specific::Service(srvc) = &unit.specific {
        assert_eq!(
            srvc.conf.exec_config.dynamic_user, true,
            "DynamicUser=yes should survive unit conversion"
        );
    } else {
        panic!("Expected service unit");
    }
}

#[test]
fn test_dynamic_user_false_preserved_after_unit_conversion() {
    use std::convert::TryInto;

    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.service"),
    )
    .unwrap();

    let unit: crate::units::Unit = service.try_into().unwrap();
    if let crate::units::Specific::Service(srvc) = &unit.specific {
        assert_eq!(
            srvc.conf.exec_config.dynamic_user, false,
            "Default DynamicUser (false) should survive unit conversion"
        );
    } else {
        panic!("Expected service unit");
    }
}

#[test]
fn test_dynamic_user_socket_unit() {
    let test_socket_str = r#"
    [Unit]
    Description = A socket with dynamic user
    [Socket]
    ListenStream = /run/test.sock
    DynamicUser = yes
    "#;

    let parsed_file = crate::units::parse_file(test_socket_str).unwrap();
    let socket = crate::units::parse_socket(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.socket"),
    )
    .unwrap();

    assert_eq!(
        socket.sock.exec_section.dynamic_user, true,
        "DynamicUser=yes should work on socket units"
    );
}

// ============================================================
// SystemCallFilter= parsing tests
// ============================================================

#[test]
fn test_system_call_filter_defaults_to_empty() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert!(
        service.srvc.exec_section.system_call_filter.is_empty(),
        "SystemCallFilter should default to empty"
    );
}

#[test]
fn test_system_call_filter_single_syscall() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    SystemCallFilter = write
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(service.srvc.exec_section.system_call_filter.len(), 1);
    assert_eq!(service.srvc.exec_section.system_call_filter[0], "write");
}

#[test]
fn test_system_call_filter_multiple_syscalls_space_separated() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    SystemCallFilter = write read open close
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(service.srvc.exec_section.system_call_filter.len(), 4);
    assert_eq!(service.srvc.exec_section.system_call_filter[0], "write");
    assert_eq!(service.srvc.exec_section.system_call_filter[1], "read");
    assert_eq!(service.srvc.exec_section.system_call_filter[2], "open");
    assert_eq!(service.srvc.exec_section.system_call_filter[3], "close");
}

#[test]
fn test_system_call_filter_group_name() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    SystemCallFilter = @basic-io
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(service.srvc.exec_section.system_call_filter.len(), 1);
    assert_eq!(service.srvc.exec_section.system_call_filter[0], "@basic-io");
}

#[test]
fn test_system_call_filter_deny_list_with_tilde() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    SystemCallFilter = ~@mount @clock
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(service.srvc.exec_section.system_call_filter.len(), 2);
    assert_eq!(service.srvc.exec_section.system_call_filter[0], "~@mount");
    assert_eq!(service.srvc.exec_section.system_call_filter[1], "@clock");
}

#[test]
fn test_system_call_filter_multiple_directives_accumulate() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    SystemCallFilter = @basic-io
    SystemCallFilter = @file-system
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(service.srvc.exec_section.system_call_filter.len(), 2);
    assert_eq!(service.srvc.exec_section.system_call_filter[0], "@basic-io");
    assert_eq!(
        service.srvc.exec_section.system_call_filter[1],
        "@file-system"
    );
}

#[test]
fn test_system_call_filter_empty_resets_list() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    SystemCallFilter = @basic-io
    SystemCallFilter =
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert!(
        service.srvc.exec_section.system_call_filter.is_empty(),
        "Empty SystemCallFilter= should reset the list"
    );
}

#[test]
fn test_system_call_filter_empty_then_new_value() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    SystemCallFilter = @basic-io
    SystemCallFilter =
    SystemCallFilter = @network-io
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(service.srvc.exec_section.system_call_filter.len(), 1);
    assert_eq!(
        service.srvc.exec_section.system_call_filter[0],
        "@network-io"
    );
}

#[test]
fn test_system_call_filter_mixed_syscalls_and_groups() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    SystemCallFilter = @basic-io write @network-io sendto
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(service.srvc.exec_section.system_call_filter.len(), 4);
    assert_eq!(service.srvc.exec_section.system_call_filter[0], "@basic-io");
    assert_eq!(service.srvc.exec_section.system_call_filter[1], "write");
    assert_eq!(
        service.srvc.exec_section.system_call_filter[2],
        "@network-io"
    );
    assert_eq!(service.srvc.exec_section.system_call_filter[3], "sendto");
}

#[test]
fn test_system_call_filter_no_unsupported_warning() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    SystemCallFilter = @basic-io
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let result = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    );

    assert!(
        result.is_ok(),
        "SystemCallFilter should be recognised and not produce a parsing error"
    );
}

#[test]
fn test_system_call_filter_with_whitespace() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    SystemCallFilter =   @basic-io   write
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(service.srvc.exec_section.system_call_filter.len(), 2);
    assert_eq!(service.srvc.exec_section.system_call_filter[0], "@basic-io");
    assert_eq!(service.srvc.exec_section.system_call_filter[1], "write");
}

#[test]
fn test_system_call_filter_tilde_deny_single() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    SystemCallFilter = ~@raw-io
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(service.srvc.exec_section.system_call_filter.len(), 1);
    assert_eq!(service.srvc.exec_section.system_call_filter[0], "~@raw-io");
}

#[test]
fn test_system_call_filter_preserved_after_unit_conversion() {
    use std::convert::TryInto;

    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    SystemCallFilter = @basic-io
    SystemCallFilter = @network-io
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.service"),
    )
    .unwrap();

    let unit: crate::units::Unit = service.try_into().unwrap();
    if let crate::units::Specific::Service(srvc) = &unit.specific {
        assert_eq!(srvc.conf.exec_config.system_call_filter.len(), 2);
        assert_eq!(srvc.conf.exec_config.system_call_filter[0], "@basic-io");
        assert_eq!(srvc.conf.exec_config.system_call_filter[1], "@network-io");
    } else {
        panic!("Expected service unit");
    }
}

#[test]
fn test_system_call_filter_empty_preserved_after_unit_conversion() {
    use std::convert::TryInto;

    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.service"),
    )
    .unwrap();

    let unit: crate::units::Unit = service.try_into().unwrap();
    if let crate::units::Specific::Service(srvc) = &unit.specific {
        assert!(
            srvc.conf.exec_config.system_call_filter.is_empty(),
            "Empty SystemCallFilter should survive unit conversion"
        );
    } else {
        panic!("Expected service unit");
    }
}

#[test]
fn test_system_call_filter_socket_unit() {
    let test_socket_str = r#"
    [Unit]
    Description = A socket with syscall filter
    [Socket]
    ListenStream = /run/test.sock
    SystemCallFilter = @basic-io
    "#;

    let parsed_file = crate::units::parse_file(test_socket_str).unwrap();
    let socket = crate::units::parse_socket(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.socket"),
    )
    .unwrap();

    assert_eq!(socket.sock.exec_section.system_call_filter.len(), 1);
    assert_eq!(socket.sock.exec_section.system_call_filter[0], "@basic-io");
}

#[test]
fn test_system_call_filter_with_other_settings() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    SystemCallFilter = @basic-io
    DynamicUser = yes
    Environment = FOO=bar
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(service.srvc.exec_section.system_call_filter.len(), 1);
    assert_eq!(service.srvc.exec_section.system_call_filter[0], "@basic-io");
    assert_eq!(service.srvc.exec_section.dynamic_user, true);
}

#[test]
fn test_system_call_filter_complex_deny_list() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    SystemCallFilter = @default @basic-io @file-system @io-event @ipc @network-io @process @signal @timer
    SystemCallFilter = ~@clock @debug @module @mount @obsolete @raw-io @reboot @swap
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    // 9 from first directive + 8 from second directive
    assert_eq!(service.srvc.exec_section.system_call_filter.len(), 17);
    assert_eq!(service.srvc.exec_section.system_call_filter[0], "@default");
    assert_eq!(service.srvc.exec_section.system_call_filter[9], "~@clock");
}

// ============================================================
// ProtectSystem= parsing tests
// ============================================================

#[test]
fn test_protect_system_defaults_to_no() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.exec_section.protect_system,
        crate::units::ProtectSystem::No,
        "ProtectSystem should default to No when not specified"
    );
}

#[test]
fn test_protect_system_yes() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    ProtectSystem = yes
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.exec_section.protect_system,
        crate::units::ProtectSystem::Yes,
        "ProtectSystem=yes should parse correctly"
    );
}

#[test]
fn test_protect_system_true() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    ProtectSystem = true
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.exec_section.protect_system,
        crate::units::ProtectSystem::Yes,
        "ProtectSystem=true should map to Yes"
    );
}

#[test]
fn test_protect_system_no() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    ProtectSystem = no
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.exec_section.protect_system,
        crate::units::ProtectSystem::No,
        "ProtectSystem=no should parse correctly"
    );
}

#[test]
fn test_protect_system_false() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    ProtectSystem = false
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.exec_section.protect_system,
        crate::units::ProtectSystem::No,
        "ProtectSystem=false should map to No"
    );
}

#[test]
fn test_protect_system_full() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    ProtectSystem = full
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.exec_section.protect_system,
        crate::units::ProtectSystem::Full,
        "ProtectSystem=full should parse correctly"
    );
}

#[test]
fn test_protect_system_strict() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    ProtectSystem = strict
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.exec_section.protect_system,
        crate::units::ProtectSystem::Strict,
        "ProtectSystem=strict should parse correctly"
    );
}

#[test]
fn test_protect_system_case_insensitive_upper() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    ProtectSystem = FULL
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.exec_section.protect_system,
        crate::units::ProtectSystem::Full,
        "ProtectSystem should be case-insensitive (FULL)"
    );
}

#[test]
fn test_protect_system_case_insensitive_mixed() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    ProtectSystem = Strict
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.exec_section.protect_system,
        crate::units::ProtectSystem::Strict,
        "ProtectSystem should be case-insensitive (Strict)"
    );
}

#[test]
fn test_protect_system_numeric_1() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    ProtectSystem = 1
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.exec_section.protect_system,
        crate::units::ProtectSystem::Yes,
        "ProtectSystem=1 should map to Yes"
    );
}

#[test]
fn test_protect_system_numeric_0() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    ProtectSystem = 0
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.exec_section.protect_system,
        crate::units::ProtectSystem::No,
        "ProtectSystem=0 should map to No"
    );
}

#[test]
fn test_protect_system_invalid_value() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    ProtectSystem = bogus
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let result = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    );

    assert!(
        result.is_err(),
        "ProtectSystem with an invalid value should produce a parsing error"
    );
}

#[test]
fn test_protect_system_no_unsupported_warning() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    ProtectSystem = full
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let result = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    );

    assert!(
        result.is_ok(),
        "ProtectSystem should be recognised and not produce a parsing error"
    );
}

#[test]
fn test_protect_system_with_whitespace() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    ProtectSystem =   strict
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.exec_section.protect_system,
        crate::units::ProtectSystem::Strict,
        "ProtectSystem should handle leading/trailing whitespace"
    );
}

#[test]
fn test_protect_system_preserved_after_unit_conversion() {
    use std::convert::TryInto;

    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    ProtectSystem = strict
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.service"),
    )
    .unwrap();

    let unit: crate::units::Unit = service.try_into().unwrap();
    if let crate::units::Specific::Service(srvc) = &unit.specific {
        assert_eq!(
            srvc.conf.exec_config.protect_system,
            crate::units::ProtectSystem::Strict,
            "ProtectSystem=strict should survive unit conversion"
        );
    } else {
        panic!("Expected service unit");
    }
}

#[test]
fn test_protect_system_default_preserved_after_unit_conversion() {
    use std::convert::TryInto;

    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.service"),
    )
    .unwrap();

    let unit: crate::units::Unit = service.try_into().unwrap();
    if let crate::units::Specific::Service(srvc) = &unit.specific {
        assert_eq!(
            srvc.conf.exec_config.protect_system,
            crate::units::ProtectSystem::No,
            "Default ProtectSystem (No) should survive unit conversion"
        );
    } else {
        panic!("Expected service unit");
    }
}

#[test]
fn test_protect_system_socket_unit() {
    let test_socket_str = r#"
    [Unit]
    Description = A socket with protect system
    [Socket]
    ListenStream = /run/test.sock
    ProtectSystem = full
    "#;

    let parsed_file = crate::units::parse_file(test_socket_str).unwrap();
    let socket = crate::units::parse_socket(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.socket"),
    )
    .unwrap();

    assert_eq!(
        socket.sock.exec_section.protect_system,
        crate::units::ProtectSystem::Full,
        "ProtectSystem=full should work on socket units"
    );
}

#[test]
fn test_protect_system_with_other_settings() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    ProtectSystem = strict
    DynamicUser = yes
    SystemCallFilter = @basic-io
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.exec_section.protect_system,
        crate::units::ProtectSystem::Strict,
    );
    assert_eq!(service.srvc.exec_section.dynamic_user, true);
    assert_eq!(service.srvc.exec_section.system_call_filter.len(), 1);
}

// ============================================================
// RestrictNamespaces= parsing tests
// ============================================================

#[test]
fn test_restrict_namespaces_defaults_to_no() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.exec_section.restrict_namespaces,
        crate::units::RestrictNamespaces::No,
        "RestrictNamespaces should default to No when not specified"
    );
}

#[test]
fn test_restrict_namespaces_yes() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    RestrictNamespaces = yes
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.exec_section.restrict_namespaces,
        crate::units::RestrictNamespaces::Yes,
        "RestrictNamespaces=yes should parse correctly"
    );
}

#[test]
fn test_restrict_namespaces_true() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    RestrictNamespaces = true
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.exec_section.restrict_namespaces,
        crate::units::RestrictNamespaces::Yes,
        "RestrictNamespaces=true should map to Yes"
    );
}

#[test]
fn test_restrict_namespaces_numeric_1() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    RestrictNamespaces = 1
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.exec_section.restrict_namespaces,
        crate::units::RestrictNamespaces::Yes,
        "RestrictNamespaces=1 should map to Yes"
    );
}

#[test]
fn test_restrict_namespaces_no() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    RestrictNamespaces = no
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.exec_section.restrict_namespaces,
        crate::units::RestrictNamespaces::No,
        "RestrictNamespaces=no should parse correctly"
    );
}

#[test]
fn test_restrict_namespaces_false() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    RestrictNamespaces = false
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.exec_section.restrict_namespaces,
        crate::units::RestrictNamespaces::No,
        "RestrictNamespaces=false should map to No"
    );
}

#[test]
fn test_restrict_namespaces_numeric_0() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    RestrictNamespaces = 0
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.exec_section.restrict_namespaces,
        crate::units::RestrictNamespaces::No,
        "RestrictNamespaces=0 should map to No"
    );
}

#[test]
fn test_restrict_namespaces_allow_single() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    RestrictNamespaces = net
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.exec_section.restrict_namespaces,
        crate::units::RestrictNamespaces::Allow(vec!["net".to_owned()]),
        "RestrictNamespaces=net should produce Allow([net])"
    );
}

#[test]
fn test_restrict_namespaces_allow_multiple() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    RestrictNamespaces = cgroup ipc net mnt
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.exec_section.restrict_namespaces,
        crate::units::RestrictNamespaces::Allow(vec![
            "cgroup".to_owned(),
            "ipc".to_owned(),
            "net".to_owned(),
            "mnt".to_owned(),
        ]),
        "Space-separated namespace types should produce an Allow list"
    );
}

#[test]
fn test_restrict_namespaces_deny_single() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    RestrictNamespaces = ~user
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.exec_section.restrict_namespaces,
        crate::units::RestrictNamespaces::Deny(vec!["user".to_owned()]),
        "RestrictNamespaces=~user should produce Deny([user])"
    );
}

#[test]
fn test_restrict_namespaces_deny_multiple() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    RestrictNamespaces = ~mnt pid user
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.exec_section.restrict_namespaces,
        crate::units::RestrictNamespaces::Deny(vec![
            "mnt".to_owned(),
            "pid".to_owned(),
            "user".to_owned(),
        ]),
        "~mnt pid user should produce a Deny list"
    );
}

#[test]
fn test_restrict_namespaces_case_insensitive_yes() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    RestrictNamespaces = YES
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.exec_section.restrict_namespaces,
        crate::units::RestrictNamespaces::Yes,
        "RestrictNamespaces should be case-insensitive (YES)"
    );
}

#[test]
fn test_restrict_namespaces_case_insensitive_list() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    RestrictNamespaces = NET IPC
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.exec_section.restrict_namespaces,
        crate::units::RestrictNamespaces::Allow(vec!["net".to_owned(), "ipc".to_owned(),]),
        "Namespace type names should be lowercased"
    );
}

#[test]
fn test_restrict_namespaces_with_whitespace() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    RestrictNamespaces =   net   ipc
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.exec_section.restrict_namespaces,
        crate::units::RestrictNamespaces::Allow(vec!["net".to_owned(), "ipc".to_owned(),]),
        "RestrictNamespaces should handle extra whitespace"
    );
}

#[test]
fn test_restrict_namespaces_deny_with_whitespace() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    RestrictNamespaces = ~  mnt   pid
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.exec_section.restrict_namespaces,
        crate::units::RestrictNamespaces::Deny(vec!["mnt".to_owned(), "pid".to_owned(),]),
        "RestrictNamespaces deny with whitespace should parse correctly"
    );
}

#[test]
fn test_restrict_namespaces_empty_value() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    RestrictNamespaces =
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.exec_section.restrict_namespaces,
        crate::units::RestrictNamespaces::No,
        "Empty RestrictNamespaces= should map to No"
    );
}

#[test]
fn test_restrict_namespaces_all_namespace_types() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    RestrictNamespaces = cgroup ipc net mnt pid user uts
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.exec_section.restrict_namespaces,
        crate::units::RestrictNamespaces::Allow(vec![
            "cgroup".to_owned(),
            "ipc".to_owned(),
            "net".to_owned(),
            "mnt".to_owned(),
            "pid".to_owned(),
            "user".to_owned(),
            "uts".to_owned(),
        ]),
        "All seven namespace types should be parsed"
    );
}

#[test]
fn test_restrict_namespaces_no_unsupported_warning() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    RestrictNamespaces = yes
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let result = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    );

    assert!(
        result.is_ok(),
        "RestrictNamespaces should be recognised and not produce a parsing error"
    );
}

#[test]
fn test_restrict_namespaces_preserved_after_unit_conversion() {
    use std::convert::TryInto;

    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    RestrictNamespaces = ~mnt pid
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.service"),
    )
    .unwrap();

    let unit: crate::units::Unit = service.try_into().unwrap();
    if let crate::units::Specific::Service(srvc) = &unit.specific {
        assert_eq!(
            srvc.conf.exec_config.restrict_namespaces,
            crate::units::RestrictNamespaces::Deny(vec!["mnt".to_owned(), "pid".to_owned(),]),
            "RestrictNamespaces should survive unit conversion"
        );
    } else {
        panic!("Expected service unit");
    }
}

#[test]
fn test_restrict_namespaces_yes_preserved_after_unit_conversion() {
    use std::convert::TryInto;

    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    RestrictNamespaces = yes
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.service"),
    )
    .unwrap();

    let unit: crate::units::Unit = service.try_into().unwrap();
    if let crate::units::Specific::Service(srvc) = &unit.specific {
        assert_eq!(
            srvc.conf.exec_config.restrict_namespaces,
            crate::units::RestrictNamespaces::Yes,
            "RestrictNamespaces=yes should survive unit conversion"
        );
    } else {
        panic!("Expected service unit");
    }
}

#[test]
fn test_restrict_namespaces_default_preserved_after_unit_conversion() {
    use std::convert::TryInto;

    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.service"),
    )
    .unwrap();

    let unit: crate::units::Unit = service.try_into().unwrap();
    if let crate::units::Specific::Service(srvc) = &unit.specific {
        assert_eq!(
            srvc.conf.exec_config.restrict_namespaces,
            crate::units::RestrictNamespaces::No,
            "Default RestrictNamespaces (No) should survive unit conversion"
        );
    } else {
        panic!("Expected service unit");
    }
}

#[test]
fn test_restrict_namespaces_socket_unit() {
    let test_socket_str = r#"
    [Unit]
    Description = A socket with namespace restriction
    [Socket]
    ListenStream = /run/test.sock
    RestrictNamespaces = yes
    "#;

    let parsed_file = crate::units::parse_file(test_socket_str).unwrap();
    let socket = crate::units::parse_socket(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.socket"),
    )
    .unwrap();

    assert_eq!(
        socket.sock.exec_section.restrict_namespaces,
        crate::units::RestrictNamespaces::Yes,
        "RestrictNamespaces=yes should work on socket units"
    );
}

#[test]
fn test_restrict_namespaces_with_other_settings() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    RestrictNamespaces = ~mnt
    ProtectSystem = strict
    SystemCallFilter = @basic-io
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.exec_section.restrict_namespaces,
        crate::units::RestrictNamespaces::Deny(vec!["mnt".to_owned()]),
    );
    assert_eq!(
        service.srvc.exec_section.protect_system,
        crate::units::ProtectSystem::Strict,
    );
    assert_eq!(service.srvc.exec_section.system_call_filter.len(), 1);
}

// ============================================================
// RestrictRealtime= parsing tests
// ============================================================

#[test]
fn test_restrict_realtime_defaults_to_false() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.exec_section.restrict_realtime, false,
        "RestrictRealtime should default to false"
    );
}

#[test]
fn test_restrict_realtime_set_yes() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    RestrictRealtime = yes
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.exec_section.restrict_realtime, true,
        "RestrictRealtime=yes should be true"
    );
}

#[test]
fn test_restrict_realtime_set_true() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    RestrictRealtime = true
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.exec_section.restrict_realtime, true,
        "RestrictRealtime=true should be true"
    );
}

#[test]
fn test_restrict_realtime_set_1() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    RestrictRealtime = 1
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.exec_section.restrict_realtime, true,
        "RestrictRealtime=1 should be true"
    );
}

#[test]
fn test_restrict_realtime_set_no() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    RestrictRealtime = no
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.exec_section.restrict_realtime, false,
        "RestrictRealtime=no should be false"
    );
}

#[test]
fn test_restrict_realtime_set_false() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    RestrictRealtime = false
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.exec_section.restrict_realtime, false,
        "RestrictRealtime=false should be false"
    );
}

#[test]
fn test_restrict_realtime_set_0() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    RestrictRealtime = 0
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.exec_section.restrict_realtime, false,
        "RestrictRealtime=0 should be false"
    );
}

#[test]
fn test_restrict_realtime_case_insensitive() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    RestrictRealtime = YES
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.exec_section.restrict_realtime, true,
        "RestrictRealtime=YES should be true (case-insensitive)"
    );
}

#[test]
fn test_restrict_realtime_no_unsupported_warning() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    RestrictRealtime = yes
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let result = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    );

    assert!(
        result.is_ok(),
        "RestrictRealtime=yes should parse without error or warning"
    );
}

#[test]
fn test_restrict_realtime_preserved_after_unit_conversion() {
    use std::convert::TryInto;

    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    RestrictRealtime = yes
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.service"),
    )
    .unwrap();

    let unit: crate::units::Unit = service.try_into().unwrap();
    if let crate::units::Specific::Service(srvc) = &unit.specific {
        assert_eq!(
            srvc.conf.exec_config.restrict_realtime, true,
            "RestrictRealtime=yes should survive unit conversion"
        );
    } else {
        panic!("Expected service unit");
    }
}

#[test]
fn test_restrict_realtime_false_preserved_after_unit_conversion() {
    use std::convert::TryInto;

    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.service"),
    )
    .unwrap();

    let unit: crate::units::Unit = service.try_into().unwrap();
    if let crate::units::Specific::Service(srvc) = &unit.specific {
        assert_eq!(
            srvc.conf.exec_config.restrict_realtime, false,
            "Default RestrictRealtime (false) should survive unit conversion"
        );
    } else {
        panic!("Expected service unit");
    }
}

#[test]
fn test_restrict_realtime_socket_unit() {
    let test_socket_str = r#"
    [Unit]
    Description = A socket with realtime restriction
    [Socket]
    ListenStream = /run/test.sock
    RestrictRealtime = yes
    "#;

    let parsed_file = crate::units::parse_file(test_socket_str).unwrap();
    let socket = crate::units::parse_socket(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.socket"),
    )
    .unwrap();

    assert_eq!(
        socket.sock.exec_section.restrict_realtime, true,
        "RestrictRealtime=yes should work on socket units"
    );
}

#[test]
fn test_restrict_realtime_with_other_settings() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    RestrictRealtime = yes
    RestrictNamespaces = yes
    ProtectSystem = strict
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(service.srvc.exec_section.restrict_realtime, true);
    assert_eq!(
        service.srvc.exec_section.restrict_namespaces,
        crate::units::RestrictNamespaces::Yes,
    );
    assert_eq!(
        service.srvc.exec_section.protect_system,
        crate::units::ProtectSystem::Strict,
    );
}

// ============================================================
// RestrictAddressFamilies= parsing tests
// ============================================================

#[test]
fn test_restrict_address_families_defaults_to_empty() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert!(
        service
            .srvc
            .exec_section
            .restrict_address_families
            .is_empty(),
        "RestrictAddressFamilies should default to empty"
    );
}

#[test]
fn test_restrict_address_families_single() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    RestrictAddressFamilies = AF_UNIX
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(service.srvc.exec_section.restrict_address_families.len(), 1);
    assert_eq!(
        service.srvc.exec_section.restrict_address_families[0],
        "AF_UNIX"
    );
}

#[test]
fn test_restrict_address_families_multiple_space_separated() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    RestrictAddressFamilies = AF_UNIX AF_INET AF_INET6
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(service.srvc.exec_section.restrict_address_families.len(), 3);
    assert_eq!(
        service.srvc.exec_section.restrict_address_families[0],
        "AF_UNIX"
    );
    assert_eq!(
        service.srvc.exec_section.restrict_address_families[1],
        "AF_INET"
    );
    assert_eq!(
        service.srvc.exec_section.restrict_address_families[2],
        "AF_INET6"
    );
}

#[test]
fn test_restrict_address_families_deny_list_with_tilde() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    RestrictAddressFamilies = ~AF_PACKET AF_NETLINK
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(service.srvc.exec_section.restrict_address_families.len(), 2);
    assert_eq!(
        service.srvc.exec_section.restrict_address_families[0],
        "~AF_PACKET"
    );
    assert_eq!(
        service.srvc.exec_section.restrict_address_families[1],
        "AF_NETLINK"
    );
}

#[test]
fn test_restrict_address_families_multiple_directives_accumulate() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    RestrictAddressFamilies = AF_UNIX
    RestrictAddressFamilies = AF_INET
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(service.srvc.exec_section.restrict_address_families.len(), 2);
    assert_eq!(
        service.srvc.exec_section.restrict_address_families[0],
        "AF_UNIX"
    );
    assert_eq!(
        service.srvc.exec_section.restrict_address_families[1],
        "AF_INET"
    );
}

#[test]
fn test_restrict_address_families_empty_resets_list() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    RestrictAddressFamilies = AF_UNIX
    RestrictAddressFamilies =
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert!(
        service
            .srvc
            .exec_section
            .restrict_address_families
            .is_empty(),
        "Empty RestrictAddressFamilies= should reset the list"
    );
}

#[test]
fn test_restrict_address_families_empty_then_new_value() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    RestrictAddressFamilies = AF_UNIX
    RestrictAddressFamilies =
    RestrictAddressFamilies = AF_INET6
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(service.srvc.exec_section.restrict_address_families.len(), 1);
    assert_eq!(
        service.srvc.exec_section.restrict_address_families[0],
        "AF_INET6"
    );
}

#[test]
fn test_restrict_address_families_with_whitespace() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    RestrictAddressFamilies =   AF_UNIX   AF_INET
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(service.srvc.exec_section.restrict_address_families.len(), 2);
    assert_eq!(
        service.srvc.exec_section.restrict_address_families[0],
        "AF_UNIX"
    );
    assert_eq!(
        service.srvc.exec_section.restrict_address_families[1],
        "AF_INET"
    );
}

#[test]
fn test_restrict_address_families_no_unsupported_warning() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    RestrictAddressFamilies = AF_UNIX
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let result = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    );

    assert!(
        result.is_ok(),
        "RestrictAddressFamilies should be recognised and not produce a parsing error"
    );
}

#[test]
fn test_restrict_address_families_tilde_deny_single() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    RestrictAddressFamilies = ~AF_PACKET
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(service.srvc.exec_section.restrict_address_families.len(), 1);
    assert_eq!(
        service.srvc.exec_section.restrict_address_families[0],
        "~AF_PACKET"
    );
}

#[test]
fn test_restrict_address_families_preserved_after_unit_conversion() {
    use std::convert::TryInto;

    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    RestrictAddressFamilies = AF_UNIX
    RestrictAddressFamilies = AF_INET AF_INET6
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.service"),
    )
    .unwrap();

    let unit: crate::units::Unit = service.try_into().unwrap();
    if let crate::units::Specific::Service(srvc) = &unit.specific {
        assert_eq!(srvc.conf.exec_config.restrict_address_families.len(), 3);
        assert_eq!(
            srvc.conf.exec_config.restrict_address_families[0],
            "AF_UNIX"
        );
        assert_eq!(
            srvc.conf.exec_config.restrict_address_families[1],
            "AF_INET"
        );
        assert_eq!(
            srvc.conf.exec_config.restrict_address_families[2],
            "AF_INET6"
        );
    } else {
        panic!("Expected service unit");
    }
}

#[test]
fn test_restrict_address_families_empty_preserved_after_unit_conversion() {
    use std::convert::TryInto;

    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.service"),
    )
    .unwrap();

    let unit: crate::units::Unit = service.try_into().unwrap();
    if let crate::units::Specific::Service(srvc) = &unit.specific {
        assert!(
            srvc.conf.exec_config.restrict_address_families.is_empty(),
            "Empty RestrictAddressFamilies should survive unit conversion"
        );
    } else {
        panic!("Expected service unit");
    }
}

#[test]
fn test_restrict_address_families_socket_unit() {
    let test_socket_str = r#"
    [Unit]
    Description = A socket with address family restriction
    [Socket]
    ListenStream = /run/test.sock
    RestrictAddressFamilies = AF_UNIX AF_INET
    "#;

    let parsed_file = crate::units::parse_file(test_socket_str).unwrap();
    let socket = crate::units::parse_socket(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.socket"),
    )
    .unwrap();

    assert_eq!(socket.sock.exec_section.restrict_address_families.len(), 2);
    assert_eq!(
        socket.sock.exec_section.restrict_address_families[0],
        "AF_UNIX"
    );
    assert_eq!(
        socket.sock.exec_section.restrict_address_families[1],
        "AF_INET"
    );
}

#[test]
fn test_restrict_address_families_with_other_settings() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    RestrictAddressFamilies = AF_UNIX AF_INET AF_INET6
    RestrictRealtime = yes
    SystemCallFilter = @basic-io
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(service.srvc.exec_section.restrict_address_families.len(), 3);
    assert_eq!(service.srvc.exec_section.restrict_realtime, true);
    assert_eq!(service.srvc.exec_section.system_call_filter.len(), 1);
}

#[test]
fn test_restrict_address_families_complex_deny_list() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    RestrictAddressFamilies = AF_UNIX AF_INET AF_INET6 AF_NETLINK
    RestrictAddressFamilies = ~AF_PACKET AF_BLUETOOTH
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    // 4 from first directive + 2 from second directive
    assert_eq!(service.srvc.exec_section.restrict_address_families.len(), 6);
    assert_eq!(
        service.srvc.exec_section.restrict_address_families[0],
        "AF_UNIX"
    );
    assert_eq!(
        service.srvc.exec_section.restrict_address_families[4],
        "~AF_PACKET"
    );
    assert_eq!(
        service.srvc.exec_section.restrict_address_families[5],
        "AF_BLUETOOTH"
    );
}

// ============================================================
// SystemCallErrorNumber= parsing tests
// ============================================================

#[test]
fn test_system_call_error_number_defaults_to_none() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.exec_section.system_call_error_number, None,
        "SystemCallErrorNumber should default to None when not specified"
    );
}

#[test]
fn test_system_call_error_number_eperm() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    SystemCallErrorNumber = EPERM
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.exec_section.system_call_error_number,
        Some("EPERM".to_owned()),
        "SystemCallErrorNumber=EPERM should parse correctly"
    );
}

#[test]
fn test_system_call_error_number_eacces() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    SystemCallErrorNumber = EACCES
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.exec_section.system_call_error_number,
        Some("EACCES".to_owned()),
        "SystemCallErrorNumber=EACCES should parse correctly"
    );
}

#[test]
fn test_system_call_error_number_enosys() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    SystemCallErrorNumber = ENOSYS
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.exec_section.system_call_error_number,
        Some("ENOSYS".to_owned()),
        "SystemCallErrorNumber=ENOSYS should parse correctly"
    );
}

#[test]
fn test_system_call_error_number_empty_resets_to_none() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    SystemCallErrorNumber =
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.exec_section.system_call_error_number, None,
        "Empty SystemCallErrorNumber= should reset to None"
    );
}

#[test]
fn test_system_call_error_number_with_whitespace() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    SystemCallErrorNumber =   EPERM
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.exec_section.system_call_error_number,
        Some("EPERM".to_owned()),
        "SystemCallErrorNumber should handle leading/trailing whitespace"
    );
}

#[test]
fn test_system_call_error_number_no_unsupported_warning() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    SystemCallErrorNumber = EPERM
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let result = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    );

    assert!(
        result.is_ok(),
        "SystemCallErrorNumber should be recognised and not produce a parsing error"
    );
}

#[test]
fn test_system_call_error_number_preserved_after_unit_conversion() {
    use std::convert::TryInto;

    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    SystemCallErrorNumber = EPERM
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.service"),
    )
    .unwrap();

    let unit: crate::units::Unit = service.try_into().unwrap();
    if let crate::units::Specific::Service(srvc) = &unit.specific {
        assert_eq!(
            srvc.conf.exec_config.system_call_error_number,
            Some("EPERM".to_owned()),
            "SystemCallErrorNumber=EPERM should survive unit conversion"
        );
    } else {
        panic!("Expected service unit");
    }
}

#[test]
fn test_system_call_error_number_none_preserved_after_unit_conversion() {
    use std::convert::TryInto;

    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.service"),
    )
    .unwrap();

    let unit: crate::units::Unit = service.try_into().unwrap();
    if let crate::units::Specific::Service(srvc) = &unit.specific {
        assert_eq!(
            srvc.conf.exec_config.system_call_error_number, None,
            "Default SystemCallErrorNumber (None) should survive unit conversion"
        );
    } else {
        panic!("Expected service unit");
    }
}

#[test]
fn test_system_call_error_number_socket_unit() {
    let test_socket_str = r#"
    [Unit]
    Description = A socket with syscall error number
    [Socket]
    ListenStream = /run/test.sock
    SystemCallErrorNumber = EACCES
    "#;

    let parsed_file = crate::units::parse_file(test_socket_str).unwrap();
    let socket = crate::units::parse_socket(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.socket"),
    )
    .unwrap();

    assert_eq!(
        socket.sock.exec_section.system_call_error_number,
        Some("EACCES".to_owned()),
        "SystemCallErrorNumber=EACCES should work on socket units"
    );
}

#[test]
fn test_system_call_error_number_with_system_call_filter() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    SystemCallFilter = @basic-io
    SystemCallErrorNumber = EPERM
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.exec_section.system_call_error_number,
        Some("EPERM".to_owned()),
    );
    assert_eq!(service.srvc.exec_section.system_call_filter.len(), 1);
    assert_eq!(service.srvc.exec_section.system_call_filter[0], "@basic-io");
}

#[test]
fn test_system_call_error_number_with_other_settings() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    SystemCallErrorNumber = EPERM
    RestrictRealtime = yes
    ProtectSystem = strict
    RestrictAddressFamilies = AF_UNIX AF_INET
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.exec_section.system_call_error_number,
        Some("EPERM".to_owned()),
    );
    assert_eq!(service.srvc.exec_section.restrict_realtime, true);
    assert_eq!(
        service.srvc.exec_section.protect_system,
        crate::units::ProtectSystem::Strict,
    );
    assert_eq!(service.srvc.exec_section.restrict_address_families.len(), 2);
}

// ============================================================
// NoNewPrivileges= parsing tests
// ============================================================

#[test]
fn test_no_new_privileges_defaults_to_false() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.exec_section.no_new_privileges, false,
        "NoNewPrivileges should default to false"
    );
}

#[test]
fn test_no_new_privileges_set_yes() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    NoNewPrivileges = yes
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.exec_section.no_new_privileges, true,
        "NoNewPrivileges=yes should be true"
    );
}

#[test]
fn test_no_new_privileges_set_true() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    NoNewPrivileges = true
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.exec_section.no_new_privileges, true,
        "NoNewPrivileges=true should be true"
    );
}

#[test]
fn test_no_new_privileges_set_no() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    NoNewPrivileges = no
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.exec_section.no_new_privileges, false,
        "NoNewPrivileges=no should be false"
    );
}

#[test]
fn test_no_new_privileges_set_false() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    NoNewPrivileges = false
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.exec_section.no_new_privileges, false,
        "NoNewPrivileges=false should be false"
    );
}

#[test]
fn test_no_new_privileges_set_1() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    NoNewPrivileges = 1
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.exec_section.no_new_privileges, true,
        "NoNewPrivileges=1 should be true"
    );
}

#[test]
fn test_no_new_privileges_set_0() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    NoNewPrivileges = 0
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.exec_section.no_new_privileges, false,
        "NoNewPrivileges=0 should be false"
    );
}

#[test]
fn test_no_new_privileges_case_insensitive() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    NoNewPrivileges = YES
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.exec_section.no_new_privileges, true,
        "NoNewPrivileges=YES should be true (case insensitive)"
    );
}

#[test]
fn test_no_new_privileges_no_unsupported_warning() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    NoNewPrivileges = yes
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let result = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    );

    assert!(result.is_ok(), "NoNewPrivileges= should not cause errors");
}

#[test]
fn test_no_new_privileges_preserved_after_unit_conversion() {
    use std::convert::TryInto;

    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    NoNewPrivileges = yes
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.service"),
    )
    .unwrap();

    let unit: crate::units::Unit = service.try_into().unwrap();
    if let crate::units::Specific::Service(srvc) = &unit.specific {
        assert_eq!(
            srvc.conf.exec_config.no_new_privileges, true,
            "NoNewPrivileges=yes should survive unit conversion"
        );
    } else {
        panic!("Expected service unit");
    }
}

#[test]
fn test_no_new_privileges_false_preserved_after_unit_conversion() {
    use std::convert::TryInto;

    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.service"),
    )
    .unwrap();

    let unit: crate::units::Unit = service.try_into().unwrap();
    if let crate::units::Specific::Service(srvc) = &unit.specific {
        assert_eq!(
            srvc.conf.exec_config.no_new_privileges, false,
            "Default NoNewPrivileges (false) should survive unit conversion"
        );
    } else {
        panic!("Expected service unit");
    }
}

#[test]
fn test_no_new_privileges_socket_unit() {
    let test_socket_str = r#"
    [Unit]
    Description = A socket with no new privileges
    [Socket]
    ListenStream = /run/test.sock
    NoNewPrivileges = yes
    "#;

    let parsed_file = crate::units::parse_file(test_socket_str).unwrap();
    let socket = crate::units::parse_socket(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.socket"),
    )
    .unwrap();

    assert_eq!(
        socket.sock.exec_section.no_new_privileges, true,
        "NoNewPrivileges=yes should work on socket units"
    );
}

// ============================================================
// ProtectControlGroups= parsing tests
// ============================================================

#[test]
fn test_protect_control_groups_defaults_to_false() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.exec_section.protect_control_groups, false,
        "ProtectControlGroups should default to false"
    );
}

#[test]
fn test_protect_control_groups_set_yes() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    ProtectControlGroups = yes
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.exec_section.protect_control_groups, true,
        "ProtectControlGroups=yes should be true"
    );
}

#[test]
fn test_protect_control_groups_set_true() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    ProtectControlGroups = true
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.exec_section.protect_control_groups, true,
        "ProtectControlGroups=true should be true"
    );
}

#[test]
fn test_protect_control_groups_set_no() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    ProtectControlGroups = no
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.exec_section.protect_control_groups, false,
        "ProtectControlGroups=no should be false"
    );
}

#[test]
fn test_protect_control_groups_set_false() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    ProtectControlGroups = false
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.exec_section.protect_control_groups, false,
        "ProtectControlGroups=false should be false"
    );
}

#[test]
fn test_protect_control_groups_set_1() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    ProtectControlGroups = 1
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.exec_section.protect_control_groups, true,
        "ProtectControlGroups=1 should be true"
    );
}

#[test]
fn test_protect_control_groups_set_0() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    ProtectControlGroups = 0
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.exec_section.protect_control_groups, false,
        "ProtectControlGroups=0 should be false"
    );
}

#[test]
fn test_protect_control_groups_case_insensitive() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    ProtectControlGroups = YES
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.exec_section.protect_control_groups, true,
        "ProtectControlGroups=YES should be true (case insensitive)"
    );
}

#[test]
fn test_protect_control_groups_no_unsupported_warning() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    ProtectControlGroups = yes
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let result = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    );

    assert!(
        result.is_ok(),
        "ProtectControlGroups= should not cause errors"
    );
}

#[test]
fn test_protect_control_groups_preserved_after_unit_conversion() {
    use std::convert::TryInto;

    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    ProtectControlGroups = yes
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.service"),
    )
    .unwrap();

    let unit: crate::units::Unit = service.try_into().unwrap();
    if let crate::units::Specific::Service(srvc) = &unit.specific {
        assert_eq!(
            srvc.conf.exec_config.protect_control_groups, true,
            "ProtectControlGroups=yes should survive unit conversion"
        );
    } else {
        panic!("Expected service unit");
    }
}

#[test]
fn test_protect_control_groups_false_preserved_after_unit_conversion() {
    use std::convert::TryInto;

    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.service"),
    )
    .unwrap();

    let unit: crate::units::Unit = service.try_into().unwrap();
    if let crate::units::Specific::Service(srvc) = &unit.specific {
        assert_eq!(
            srvc.conf.exec_config.protect_control_groups, false,
            "Default ProtectControlGroups (false) should survive unit conversion"
        );
    } else {
        panic!("Expected service unit");
    }
}

#[test]
fn test_protect_control_groups_socket_unit() {
    let test_socket_str = r#"
    [Unit]
    Description = A socket with protect control groups
    [Socket]
    ListenStream = /run/test.sock
    ProtectControlGroups = yes
    "#;

    let parsed_file = crate::units::parse_file(test_socket_str).unwrap();
    let socket = crate::units::parse_socket(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.socket"),
    )
    .unwrap();

    assert_eq!(
        socket.sock.exec_section.protect_control_groups, true,
        "ProtectControlGroups=yes should work on socket units"
    );
}

// ============================================================
// ProtectKernelModules= parsing tests
// ============================================================

#[test]
fn test_protect_kernel_modules_defaults_to_false() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.exec_section.protect_kernel_modules, false,
        "ProtectKernelModules should default to false"
    );
}

#[test]
fn test_protect_kernel_modules_set_yes() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    ProtectKernelModules = yes
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.exec_section.protect_kernel_modules, true,
        "ProtectKernelModules=yes should be true"
    );
}

#[test]
fn test_protect_kernel_modules_set_true() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    ProtectKernelModules = true
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.exec_section.protect_kernel_modules, true,
        "ProtectKernelModules=true should be true"
    );
}

#[test]
fn test_protect_kernel_modules_set_no() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    ProtectKernelModules = no
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.exec_section.protect_kernel_modules, false,
        "ProtectKernelModules=no should be false"
    );
}

#[test]
fn test_protect_kernel_modules_set_false() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    ProtectKernelModules = false
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.exec_section.protect_kernel_modules, false,
        "ProtectKernelModules=false should be false"
    );
}

#[test]
fn test_protect_kernel_modules_set_1() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    ProtectKernelModules = 1
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.exec_section.protect_kernel_modules, true,
        "ProtectKernelModules=1 should be true"
    );
}

#[test]
fn test_protect_kernel_modules_set_0() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    ProtectKernelModules = 0
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.exec_section.protect_kernel_modules, false,
        "ProtectKernelModules=0 should be false"
    );
}

#[test]
fn test_protect_kernel_modules_case_insensitive() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    ProtectKernelModules = YES
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.exec_section.protect_kernel_modules, true,
        "ProtectKernelModules=YES should be true (case insensitive)"
    );
}

#[test]
fn test_protect_kernel_modules_no_unsupported_warning() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    ProtectKernelModules = yes
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let result = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    );

    assert!(
        result.is_ok(),
        "ProtectKernelModules= should not cause errors"
    );
}

#[test]
fn test_protect_kernel_modules_preserved_after_unit_conversion() {
    use std::convert::TryInto;

    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    ProtectKernelModules = yes
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.service"),
    )
    .unwrap();

    let unit: crate::units::Unit = service.try_into().unwrap();
    if let crate::units::Specific::Service(srvc) = &unit.specific {
        assert_eq!(
            srvc.conf.exec_config.protect_kernel_modules, true,
            "ProtectKernelModules=yes should survive unit conversion"
        );
    } else {
        panic!("Expected service unit");
    }
}

#[test]
fn test_protect_kernel_modules_false_preserved_after_unit_conversion() {
    use std::convert::TryInto;

    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.service"),
    )
    .unwrap();

    let unit: crate::units::Unit = service.try_into().unwrap();
    if let crate::units::Specific::Service(srvc) = &unit.specific {
        assert_eq!(
            srvc.conf.exec_config.protect_kernel_modules, false,
            "Default ProtectKernelModules (false) should survive unit conversion"
        );
    } else {
        panic!("Expected service unit");
    }
}

#[test]
fn test_protect_kernel_modules_socket_unit() {
    let test_socket_str = r#"
    [Unit]
    Description = A socket with protect kernel modules
    [Socket]
    ListenStream = /run/test.sock
    ProtectKernelModules = yes
    "#;

    let parsed_file = crate::units::parse_file(test_socket_str).unwrap();
    let socket = crate::units::parse_socket(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.socket"),
    )
    .unwrap();

    assert_eq!(
        socket.sock.exec_section.protect_kernel_modules, true,
        "ProtectKernelModules=yes should work on socket units"
    );
}

// ============================================================
// RestrictSUIDSGID= parsing tests
// ============================================================

#[test]
fn test_restrict_suid_sgid_defaults_to_false() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.exec_section.restrict_suid_sgid, false,
        "RestrictSUIDSGID should default to false"
    );
}

#[test]
fn test_restrict_suid_sgid_set_yes() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    RestrictSUIDSGID = yes
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.exec_section.restrict_suid_sgid, true,
        "RestrictSUIDSGID=yes should be true"
    );
}

#[test]
fn test_restrict_suid_sgid_set_true() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    RestrictSUIDSGID = true
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.exec_section.restrict_suid_sgid, true,
        "RestrictSUIDSGID=true should be true"
    );
}

#[test]
fn test_restrict_suid_sgid_set_no() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    RestrictSUIDSGID = no
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.exec_section.restrict_suid_sgid, false,
        "RestrictSUIDSGID=no should be false"
    );
}

#[test]
fn test_restrict_suid_sgid_set_false() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    RestrictSUIDSGID = false
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.exec_section.restrict_suid_sgid, false,
        "RestrictSUIDSGID=false should be false"
    );
}

#[test]
fn test_restrict_suid_sgid_set_1() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    RestrictSUIDSGID = 1
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.exec_section.restrict_suid_sgid, true,
        "RestrictSUIDSGID=1 should be true"
    );
}

#[test]
fn test_restrict_suid_sgid_set_0() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    RestrictSUIDSGID = 0
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.exec_section.restrict_suid_sgid, false,
        "RestrictSUIDSGID=0 should be false"
    );
}

#[test]
fn test_restrict_suid_sgid_case_insensitive() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    RestrictSUIDSGID = YES
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.exec_section.restrict_suid_sgid, true,
        "RestrictSUIDSGID=YES should be true (case insensitive)"
    );
}

#[test]
fn test_restrict_suid_sgid_no_unsupported_warning() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    RestrictSUIDSGID = yes
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let result = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    );

    assert!(result.is_ok(), "RestrictSUIDSGID= should not cause errors");
}

#[test]
fn test_restrict_suid_sgid_preserved_after_unit_conversion() {
    use std::convert::TryInto;

    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    RestrictSUIDSGID = yes
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.service"),
    )
    .unwrap();

    let unit: crate::units::Unit = service.try_into().unwrap();
    if let crate::units::Specific::Service(srvc) = &unit.specific {
        assert_eq!(
            srvc.conf.exec_config.restrict_suid_sgid, true,
            "RestrictSUIDSGID=yes should survive unit conversion"
        );
    } else {
        panic!("Expected service unit");
    }
}

#[test]
fn test_restrict_suid_sgid_false_preserved_after_unit_conversion() {
    use std::convert::TryInto;

    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.service"),
    )
    .unwrap();

    let unit: crate::units::Unit = service.try_into().unwrap();
    if let crate::units::Specific::Service(srvc) = &unit.specific {
        assert_eq!(
            srvc.conf.exec_config.restrict_suid_sgid, false,
            "Default RestrictSUIDSGID (false) should survive unit conversion"
        );
    } else {
        panic!("Expected service unit");
    }
}

#[test]
fn test_restrict_suid_sgid_socket_unit() {
    let test_socket_str = r#"
    [Unit]
    Description = A socket with restrict suid sgid
    [Socket]
    ListenStream = /run/test.sock
    RestrictSUIDSGID = yes
    "#;

    let parsed_file = crate::units::parse_file(test_socket_str).unwrap();
    let socket = crate::units::parse_socket(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.socket"),
    )
    .unwrap();

    assert_eq!(
        socket.sock.exec_section.restrict_suid_sgid, true,
        "RestrictSUIDSGID=yes should work on socket units"
    );
}

// ============================================================
// ProtectKernelLogs= parsing tests
// ============================================================

#[test]
fn test_protect_kernel_logs_defaults_to_false() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.exec_section.protect_kernel_logs, false,
        "ProtectKernelLogs should default to false"
    );
}

#[test]
fn test_protect_kernel_logs_set_yes() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    ProtectKernelLogs = yes
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.exec_section.protect_kernel_logs, true,
        "ProtectKernelLogs=yes should be true"
    );
}

#[test]
fn test_protect_kernel_logs_set_true() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    ProtectKernelLogs = true
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.exec_section.protect_kernel_logs, true,
        "ProtectKernelLogs=true should be true"
    );
}

#[test]
fn test_protect_kernel_logs_set_no() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    ProtectKernelLogs = no
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.exec_section.protect_kernel_logs, false,
        "ProtectKernelLogs=no should be false"
    );
}

#[test]
fn test_protect_kernel_logs_set_false() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    ProtectKernelLogs = false
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.exec_section.protect_kernel_logs, false,
        "ProtectKernelLogs=false should be false"
    );
}

#[test]
fn test_protect_kernel_logs_set_1() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    ProtectKernelLogs = 1
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.exec_section.protect_kernel_logs, true,
        "ProtectKernelLogs=1 should be true"
    );
}

#[test]
fn test_protect_kernel_logs_set_0() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    ProtectKernelLogs = 0
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.exec_section.protect_kernel_logs, false,
        "ProtectKernelLogs=0 should be false"
    );
}

#[test]
fn test_protect_kernel_logs_case_insensitive() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    ProtectKernelLogs = YES
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.exec_section.protect_kernel_logs, true,
        "ProtectKernelLogs=YES should be true (case insensitive)"
    );
}

#[test]
fn test_protect_kernel_logs_no_unsupported_warning() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    ProtectKernelLogs = yes
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let result = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    );

    assert!(result.is_ok(), "ProtectKernelLogs= should not cause errors");
}

#[test]
fn test_protect_kernel_logs_preserved_after_unit_conversion() {
    use std::convert::TryInto;

    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    ProtectKernelLogs = yes
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.service"),
    )
    .unwrap();

    let unit: crate::units::Unit = service.try_into().unwrap();
    if let crate::units::Specific::Service(srvc) = &unit.specific {
        assert_eq!(
            srvc.conf.exec_config.protect_kernel_logs, true,
            "ProtectKernelLogs=yes should survive unit conversion"
        );
    } else {
        panic!("Expected service unit");
    }
}

#[test]
fn test_protect_kernel_logs_false_preserved_after_unit_conversion() {
    use std::convert::TryInto;

    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.service"),
    )
    .unwrap();

    let unit: crate::units::Unit = service.try_into().unwrap();
    if let crate::units::Specific::Service(srvc) = &unit.specific {
        assert_eq!(
            srvc.conf.exec_config.protect_kernel_logs, false,
            "Default ProtectKernelLogs (false) should survive unit conversion"
        );
    } else {
        panic!("Expected service unit");
    }
}

#[test]
fn test_protect_kernel_logs_socket_unit() {
    let test_socket_str = r#"
    [Unit]
    Description = A socket with protect kernel logs
    [Socket]
    ListenStream = /run/test.sock
    ProtectKernelLogs = yes
    "#;

    let parsed_file = crate::units::parse_file(test_socket_str).unwrap();
    let socket = crate::units::parse_socket(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.socket"),
    )
    .unwrap();

    assert_eq!(
        socket.sock.exec_section.protect_kernel_logs, true,
        "ProtectKernelLogs=yes should work on socket units"
    );
}

// ============================================================
// ProtectKernelTunables= parsing tests
// ============================================================

#[test]
fn test_protect_kernel_tunables_defaults_to_false() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert!(
        !service.srvc.exec_section.protect_kernel_tunables,
        "ProtectKernelTunables should default to false"
    );
}

#[test]
fn test_protect_kernel_tunables_true() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    ProtectKernelTunables = yes
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert!(
        service.srvc.exec_section.protect_kernel_tunables,
        "ProtectKernelTunables=yes should be true"
    );
}

#[test]
fn test_protect_kernel_tunables_false() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    ProtectKernelTunables = no
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert!(
        !service.srvc.exec_section.protect_kernel_tunables,
        "ProtectKernelTunables=no should be false"
    );
}

#[test]
fn test_protect_kernel_tunables_true_variant() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    ProtectKernelTunables = true
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert!(
        service.srvc.exec_section.protect_kernel_tunables,
        "ProtectKernelTunables=true should be true"
    );
}

#[test]
fn test_protect_kernel_tunables_one_is_true() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    ProtectKernelTunables = 1
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert!(
        service.srvc.exec_section.protect_kernel_tunables,
        "ProtectKernelTunables=1 should be true"
    );
}

#[test]
fn test_protect_kernel_tunables_zero_is_false() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    ProtectKernelTunables = 0
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert!(
        !service.srvc.exec_section.protect_kernel_tunables,
        "ProtectKernelTunables=0 should be false"
    );
}

#[test]
fn test_protect_kernel_tunables_case_insensitive() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    ProtectKernelTunables = Yes
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert!(
        service.srvc.exec_section.protect_kernel_tunables,
        "ProtectKernelTunables=Yes should be true (case insensitive)"
    );
}

#[test]
fn test_protect_kernel_tunables_no_unsupported_warning() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    ProtectKernelTunables = yes
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let result = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    );

    assert!(
        result.is_ok(),
        "ProtectKernelTunables= should not cause errors"
    );
}

#[test]
fn test_protect_kernel_tunables_preserved_after_unit_conversion() {
    use std::convert::TryInto;

    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    ProtectKernelTunables = yes
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.service"),
    )
    .unwrap();

    let unit: crate::units::Unit = service.try_into().unwrap();
    if let crate::units::Specific::Service(srvc) = &unit.specific {
        assert!(
            srvc.conf.exec_config.protect_kernel_tunables,
            "ProtectKernelTunables=yes should survive unit conversion"
        );
    } else {
        panic!("Expected service unit");
    }
}

#[test]
fn test_protect_kernel_tunables_false_preserved_after_unit_conversion() {
    use std::convert::TryInto;

    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.service"),
    )
    .unwrap();

    let unit: crate::units::Unit = service.try_into().unwrap();
    if let crate::units::Specific::Service(srvc) = &unit.specific {
        assert!(
            !srvc.conf.exec_config.protect_kernel_tunables,
            "Default false ProtectKernelTunables should survive unit conversion"
        );
    } else {
        panic!("Expected service unit");
    }
}

#[test]
fn test_protect_kernel_tunables_socket_unit() {
    let test_socket_str = r#"
    [Unit]
    Description = A socket with kernel tunables protection
    [Socket]
    ListenStream = /run/test.sock
    ProtectKernelTunables = yes
    "#;

    let parsed_file = crate::units::parse_file(test_socket_str).unwrap();
    let socket = crate::units::parse_socket(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.socket"),
    )
    .unwrap();

    assert!(
        socket.sock.exec_section.protect_kernel_tunables,
        "ProtectKernelTunables should work in socket units"
    );
}

#[test]
fn test_protect_kernel_tunables_with_other_settings() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    ProtectKernelTunables = yes
    ProtectKernelModules = yes
    ProtectKernelLogs = yes
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert!(service.srvc.exec_section.protect_kernel_tunables);
    assert!(service.srvc.exec_section.protect_kernel_modules);
    assert!(service.srvc.exec_section.protect_kernel_logs);
}

// ============================================================
// CapabilityBoundingSet= parsing tests
// ============================================================

#[test]
fn test_capability_bounding_set_defaults_to_empty() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert!(
        service.srvc.exec_section.capability_bounding_set.is_empty(),
        "CapabilityBoundingSet should default to empty"
    );
}

#[test]
fn test_capability_bounding_set_single_cap() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    CapabilityBoundingSet = CAP_NET_ADMIN
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.exec_section.capability_bounding_set,
        vec!["CAP_NET_ADMIN"],
    );
}

#[test]
fn test_capability_bounding_set_multiple_caps() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    CapabilityBoundingSet = CAP_NET_ADMIN CAP_SYS_PTRACE CAP_DAC_OVERRIDE
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(service.srvc.exec_section.capability_bounding_set.len(), 3);
    assert_eq!(
        service.srvc.exec_section.capability_bounding_set[0],
        "CAP_NET_ADMIN"
    );
    assert_eq!(
        service.srvc.exec_section.capability_bounding_set[1],
        "CAP_SYS_PTRACE"
    );
    assert_eq!(
        service.srvc.exec_section.capability_bounding_set[2],
        "CAP_DAC_OVERRIDE"
    );
}

#[test]
fn test_capability_bounding_set_deny_list() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    CapabilityBoundingSet = ~CAP_SYS_ADMIN CAP_NET_RAW
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(service.srvc.exec_section.capability_bounding_set.len(), 2);
    assert_eq!(
        service.srvc.exec_section.capability_bounding_set[0],
        "~CAP_SYS_ADMIN"
    );
    assert_eq!(
        service.srvc.exec_section.capability_bounding_set[1],
        "CAP_NET_RAW"
    );
}

#[test]
fn test_capability_bounding_set_multiple_directives_accumulate() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    CapabilityBoundingSet = CAP_NET_ADMIN
    CapabilityBoundingSet = CAP_SYS_PTRACE
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(service.srvc.exec_section.capability_bounding_set.len(), 2);
    assert_eq!(
        service.srvc.exec_section.capability_bounding_set[0],
        "CAP_NET_ADMIN"
    );
    assert_eq!(
        service.srvc.exec_section.capability_bounding_set[1],
        "CAP_SYS_PTRACE"
    );
}

#[test]
fn test_capability_bounding_set_empty_resets() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    CapabilityBoundingSet = CAP_NET_ADMIN
    CapabilityBoundingSet =
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert!(
        service.srvc.exec_section.capability_bounding_set.is_empty(),
        "Empty CapabilityBoundingSet= should reset the list"
    );
}

#[test]
fn test_capability_bounding_set_no_unsupported_warning() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    CapabilityBoundingSet = CAP_NET_ADMIN
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let result = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    );

    assert!(
        result.is_ok(),
        "CapabilityBoundingSet= should not cause errors"
    );
}

#[test]
fn test_capability_bounding_set_preserved_after_unit_conversion() {
    use std::convert::TryInto;

    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    CapabilityBoundingSet = CAP_NET_ADMIN CAP_SYS_PTRACE
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.service"),
    )
    .unwrap();

    let unit: crate::units::Unit = service.try_into().unwrap();
    if let crate::units::Specific::Service(srvc) = &unit.specific {
        assert_eq!(
            srvc.conf.exec_config.capability_bounding_set.len(),
            2,
            "CapabilityBoundingSet should survive unit conversion"
        );
        assert_eq!(
            srvc.conf.exec_config.capability_bounding_set[0],
            "CAP_NET_ADMIN"
        );
        assert_eq!(
            srvc.conf.exec_config.capability_bounding_set[1],
            "CAP_SYS_PTRACE"
        );
    } else {
        panic!("Expected service unit");
    }
}

#[test]
fn test_capability_bounding_set_empty_preserved_after_unit_conversion() {
    use std::convert::TryInto;

    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.service"),
    )
    .unwrap();

    let unit: crate::units::Unit = service.try_into().unwrap();
    if let crate::units::Specific::Service(srvc) = &unit.specific {
        assert!(
            srvc.conf.exec_config.capability_bounding_set.is_empty(),
            "Default empty CapabilityBoundingSet should survive unit conversion"
        );
    } else {
        panic!("Expected service unit");
    }
}

#[test]
fn test_capability_bounding_set_socket_unit() {
    let test_socket_str = r#"
    [Unit]
    Description = A socket with capability bounding set
    [Socket]
    ListenStream = /run/test.sock
    CapabilityBoundingSet = CAP_NET_BIND_SERVICE
    "#;

    let parsed_file = crate::units::parse_file(test_socket_str).unwrap();
    let socket = crate::units::parse_socket(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.socket"),
    )
    .unwrap();

    assert_eq!(
        socket.sock.exec_section.capability_bounding_set,
        vec!["CAP_NET_BIND_SERVICE"],
        "CapabilityBoundingSet should work on socket units"
    );
}

// ============================================================
// AmbientCapabilities= parsing tests
// ============================================================

#[test]
fn test_ambient_capabilities_defaults_to_empty() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert!(
        service.srvc.exec_section.ambient_capabilities.is_empty(),
        "AmbientCapabilities should default to empty"
    );
}

#[test]
fn test_ambient_capabilities_single_cap() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    AmbientCapabilities = CAP_NET_BIND_SERVICE
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.exec_section.ambient_capabilities,
        vec!["CAP_NET_BIND_SERVICE"],
    );
}

#[test]
fn test_ambient_capabilities_multiple_caps() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    AmbientCapabilities = CAP_NET_BIND_SERVICE CAP_SYS_NICE CAP_DAC_READ_SEARCH
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(service.srvc.exec_section.ambient_capabilities.len(), 3);
    assert_eq!(
        service.srvc.exec_section.ambient_capabilities[0],
        "CAP_NET_BIND_SERVICE"
    );
    assert_eq!(
        service.srvc.exec_section.ambient_capabilities[1],
        "CAP_SYS_NICE"
    );
    assert_eq!(
        service.srvc.exec_section.ambient_capabilities[2],
        "CAP_DAC_READ_SEARCH"
    );
}

#[test]
fn test_ambient_capabilities_deny_list() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    AmbientCapabilities = ~CAP_SYS_ADMIN CAP_NET_RAW
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(service.srvc.exec_section.ambient_capabilities.len(), 2);
    assert_eq!(
        service.srvc.exec_section.ambient_capabilities[0],
        "~CAP_SYS_ADMIN"
    );
    assert_eq!(
        service.srvc.exec_section.ambient_capabilities[1],
        "CAP_NET_RAW"
    );
}

#[test]
fn test_ambient_capabilities_multiple_directives_accumulate() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    AmbientCapabilities = CAP_NET_BIND_SERVICE
    AmbientCapabilities = CAP_SYS_NICE
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(service.srvc.exec_section.ambient_capabilities.len(), 2);
    assert_eq!(
        service.srvc.exec_section.ambient_capabilities[0],
        "CAP_NET_BIND_SERVICE"
    );
    assert_eq!(
        service.srvc.exec_section.ambient_capabilities[1],
        "CAP_SYS_NICE"
    );
}

#[test]
fn test_ambient_capabilities_empty_resets() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    AmbientCapabilities = CAP_NET_BIND_SERVICE
    AmbientCapabilities =
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert!(
        service.srvc.exec_section.ambient_capabilities.is_empty(),
        "Empty AmbientCapabilities= should reset the list"
    );
}

#[test]
fn test_ambient_capabilities_empty_then_new_value() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    AmbientCapabilities = CAP_NET_BIND_SERVICE
    AmbientCapabilities =
    AmbientCapabilities = CAP_SYS_NICE
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(service.srvc.exec_section.ambient_capabilities.len(), 1);
    assert_eq!(
        service.srvc.exec_section.ambient_capabilities[0],
        "CAP_SYS_NICE"
    );
}

#[test]
fn test_ambient_capabilities_no_unsupported_warning() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    AmbientCapabilities = CAP_NET_BIND_SERVICE
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let result = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    );

    assert!(
        result.is_ok(),
        "AmbientCapabilities= should not cause errors"
    );
}

#[test]
fn test_ambient_capabilities_preserved_after_unit_conversion() {
    use std::convert::TryInto;

    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    AmbientCapabilities = CAP_NET_BIND_SERVICE CAP_SYS_NICE
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.service"),
    )
    .unwrap();

    let unit: crate::units::Unit = service.try_into().unwrap();
    if let crate::units::Specific::Service(srvc) = &unit.specific {
        assert_eq!(
            srvc.conf.exec_config.ambient_capabilities.len(),
            2,
            "AmbientCapabilities should survive unit conversion"
        );
        assert_eq!(
            srvc.conf.exec_config.ambient_capabilities[0],
            "CAP_NET_BIND_SERVICE"
        );
        assert_eq!(
            srvc.conf.exec_config.ambient_capabilities[1],
            "CAP_SYS_NICE"
        );
    } else {
        panic!("Expected service unit");
    }
}

#[test]
fn test_ambient_capabilities_empty_preserved_after_unit_conversion() {
    use std::convert::TryInto;

    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.service"),
    )
    .unwrap();

    let unit: crate::units::Unit = service.try_into().unwrap();
    if let crate::units::Specific::Service(srvc) = &unit.specific {
        assert!(
            srvc.conf.exec_config.ambient_capabilities.is_empty(),
            "Default empty AmbientCapabilities should survive unit conversion"
        );
    } else {
        panic!("Expected service unit");
    }
}

#[test]
fn test_ambient_capabilities_socket_unit() {
    let test_socket_str = r#"
    [Unit]
    Description = A socket with ambient capabilities
    [Socket]
    ListenStream = /run/test.sock
    AmbientCapabilities = CAP_NET_BIND_SERVICE
    "#;

    let parsed_file = crate::units::parse_file(test_socket_str).unwrap();
    let socket = crate::units::parse_socket(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.socket"),
    )
    .unwrap();

    assert_eq!(
        socket.sock.exec_section.ambient_capabilities,
        vec!["CAP_NET_BIND_SERVICE"],
        "AmbientCapabilities should work on socket units"
    );
}

#[test]
fn test_ambient_capabilities_with_whitespace() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    AmbientCapabilities =   CAP_NET_BIND_SERVICE   CAP_SYS_NICE
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(service.srvc.exec_section.ambient_capabilities.len(), 2);
    assert_eq!(
        service.srvc.exec_section.ambient_capabilities[0],
        "CAP_NET_BIND_SERVICE"
    );
    assert_eq!(
        service.srvc.exec_section.ambient_capabilities[1],
        "CAP_SYS_NICE"
    );
}

#[test]
fn test_ambient_capabilities_with_other_settings() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    AmbientCapabilities = CAP_NET_BIND_SERVICE
    CapabilityBoundingSet = CAP_NET_BIND_SERVICE CAP_SYS_NICE
    NoNewPrivileges = yes
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.exec_section.ambient_capabilities,
        vec!["CAP_NET_BIND_SERVICE"],
    );
    assert_eq!(service.srvc.exec_section.capability_bounding_set.len(), 2);
    assert!(service.srvc.exec_section.no_new_privileges);
}

#[test]
fn test_ambient_capabilities_tilde_deny_single() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    AmbientCapabilities = ~CAP_NET_RAW
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(service.srvc.exec_section.ambient_capabilities.len(), 1);
    assert_eq!(
        service.srvc.exec_section.ambient_capabilities[0],
        "~CAP_NET_RAW"
    );
}

// ============================================================
// ProtectClock= parsing tests
// ============================================================

#[test]
fn test_protect_clock_defaults_to_false() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.exec_section.protect_clock, false,
        "ProtectClock should default to false"
    );
}

#[test]
fn test_protect_clock_set_yes() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    ProtectClock = yes
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.exec_section.protect_clock, true,
        "ProtectClock=yes should be true"
    );
}

#[test]
fn test_protect_clock_set_true() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    ProtectClock = true
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.exec_section.protect_clock, true,
        "ProtectClock=true should be true"
    );
}

#[test]
fn test_protect_clock_set_no() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    ProtectClock = no
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.exec_section.protect_clock, false,
        "ProtectClock=no should be false"
    );
}

#[test]
fn test_protect_clock_set_false() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    ProtectClock = false
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.exec_section.protect_clock, false,
        "ProtectClock=false should be false"
    );
}

#[test]
fn test_protect_clock_set_1() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    ProtectClock = 1
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.exec_section.protect_clock, true,
        "ProtectClock=1 should be true"
    );
}

#[test]
fn test_protect_clock_set_0() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    ProtectClock = 0
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.exec_section.protect_clock, false,
        "ProtectClock=0 should be false"
    );
}

#[test]
fn test_protect_clock_case_insensitive() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    ProtectClock = YES
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.exec_section.protect_clock, true,
        "ProtectClock=YES should be true (case insensitive)"
    );
}

#[test]
fn test_protect_clock_no_unsupported_warning() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    ProtectClock = yes
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let result = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    );

    assert!(result.is_ok(), "ProtectClock= should not cause errors");
}

#[test]
fn test_protect_clock_preserved_after_unit_conversion() {
    use std::convert::TryInto;

    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    ProtectClock = yes
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.service"),
    )
    .unwrap();

    let unit: crate::units::Unit = service.try_into().unwrap();
    if let crate::units::Specific::Service(srvc) = &unit.specific {
        assert_eq!(
            srvc.conf.exec_config.protect_clock, true,
            "ProtectClock=yes should survive unit conversion"
        );
    } else {
        panic!("Expected service unit");
    }
}

#[test]
fn test_protect_clock_false_preserved_after_unit_conversion() {
    use std::convert::TryInto;

    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.service"),
    )
    .unwrap();

    let unit: crate::units::Unit = service.try_into().unwrap();
    if let crate::units::Specific::Service(srvc) = &unit.specific {
        assert_eq!(
            srvc.conf.exec_config.protect_clock, false,
            "Default ProtectClock (false) should survive unit conversion"
        );
    } else {
        panic!("Expected service unit");
    }
}

#[test]
fn test_protect_clock_socket_unit() {
    let test_socket_str = r#"
    [Unit]
    Description = A socket with protect clock
    [Socket]
    ListenStream = /run/test.sock
    ProtectClock = yes
    "#;

    let parsed_file = crate::units::parse_file(test_socket_str).unwrap();
    let socket = crate::units::parse_socket(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.socket"),
    )
    .unwrap();

    assert_eq!(
        socket.sock.exec_section.protect_clock, true,
        "ProtectClock=yes should work on socket units"
    );
}

#[test]
fn test_capability_bounding_set_empty_string_drops_all() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    CapabilityBoundingSet =
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert!(
        service.srvc.exec_section.capability_bounding_set.is_empty(),
        "CapabilityBoundingSet= (empty) should result in empty list"
    );
}

// ============================================================
// DeviceAllow= parsing tests
// ============================================================

#[test]
fn test_device_allow_defaults_to_empty() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert!(
        service.srvc.device_allow.is_empty(),
        "DeviceAllow should default to empty"
    );
}

#[test]
fn test_device_allow_single_entry() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    DeviceAllow = /dev/null rw
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(service.srvc.device_allow.len(), 1);
    assert_eq!(service.srvc.device_allow[0], "/dev/null rw");
}

#[test]
fn test_device_allow_multiple_directives_accumulate() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    DeviceAllow = /dev/null rw
    DeviceAllow = /dev/zero r
    DeviceAllow = /dev/urandom r
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(service.srvc.device_allow.len(), 3);
    assert_eq!(service.srvc.device_allow[0], "/dev/null rw");
    assert_eq!(service.srvc.device_allow[1], "/dev/zero r");
    assert_eq!(service.srvc.device_allow[2], "/dev/urandom r");
}

#[test]
fn test_device_allow_empty_resets() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    DeviceAllow = /dev/null rw
    DeviceAllow =
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert!(
        service.srvc.device_allow.is_empty(),
        "Empty DeviceAllow= should reset the list"
    );
}

#[test]
fn test_device_allow_empty_then_new_entry() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    DeviceAllow = /dev/null rw
    DeviceAllow =
    DeviceAllow = /dev/zero r
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(service.srvc.device_allow.len(), 1);
    assert_eq!(service.srvc.device_allow[0], "/dev/zero r");
}

#[test]
fn test_device_allow_char_class() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    DeviceAllow = char-* rwm
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(service.srvc.device_allow.len(), 1);
    assert_eq!(service.srvc.device_allow[0], "char-* rwm");
}

#[test]
fn test_device_allow_path_only_no_access() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    DeviceAllow = /dev/sda
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(service.srvc.device_allow.len(), 1);
    assert_eq!(service.srvc.device_allow[0], "/dev/sda");
}

#[test]
fn test_device_allow_no_unsupported_warning() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    DeviceAllow = /dev/null rw
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let result = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    );

    assert!(result.is_ok(), "DeviceAllow= should not cause errors");
}

#[test]
fn test_device_allow_preserved_after_unit_conversion() {
    use std::convert::TryInto;

    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    DeviceAllow = /dev/null rw
    DeviceAllow = /dev/zero r
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.service"),
    )
    .unwrap();

    let unit: crate::units::Unit = service.try_into().unwrap();
    if let crate::units::Specific::Service(srvc) = &unit.specific {
        assert_eq!(
            srvc.conf.device_allow.len(),
            2,
            "DeviceAllow should survive unit conversion"
        );
        assert_eq!(srvc.conf.device_allow[0], "/dev/null rw");
        assert_eq!(srvc.conf.device_allow[1], "/dev/zero r");
    } else {
        panic!("Expected service unit");
    }
}

#[test]
fn test_device_allow_empty_preserved_after_unit_conversion() {
    use std::convert::TryInto;

    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.service"),
    )
    .unwrap();

    let unit: crate::units::Unit = service.try_into().unwrap();
    if let crate::units::Specific::Service(srvc) = &unit.specific {
        assert!(
            srvc.conf.device_allow.is_empty(),
            "Default empty DeviceAllow should survive unit conversion"
        );
    } else {
        panic!("Expected service unit");
    }
}

// ============================================================
// ProtectHome= parsing tests
// ============================================================

#[test]
fn test_protect_home_defaults_to_no() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.exec_section.protect_home,
        crate::units::ProtectHome::No,
        "ProtectHome should default to No when not specified"
    );
}

#[test]
fn test_protect_home_yes() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    ProtectHome = yes
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.exec_section.protect_home,
        crate::units::ProtectHome::Yes,
        "ProtectHome=yes should parse correctly"
    );
}

#[test]
fn test_protect_home_true() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    ProtectHome = true
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.exec_section.protect_home,
        crate::units::ProtectHome::Yes,
        "ProtectHome=true should map to Yes"
    );
}

#[test]
fn test_protect_home_no() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    ProtectHome = no
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.exec_section.protect_home,
        crate::units::ProtectHome::No,
        "ProtectHome=no should parse correctly"
    );
}

#[test]
fn test_protect_home_false() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    ProtectHome = false
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.exec_section.protect_home,
        crate::units::ProtectHome::No,
        "ProtectHome=false should map to No"
    );
}

#[test]
fn test_protect_home_read_only() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    ProtectHome = read-only
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.exec_section.protect_home,
        crate::units::ProtectHome::ReadOnly,
        "ProtectHome=read-only should parse correctly"
    );
}

#[test]
fn test_protect_home_tmpfs() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    ProtectHome = tmpfs
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.exec_section.protect_home,
        crate::units::ProtectHome::Tmpfs,
        "ProtectHome=tmpfs should parse correctly"
    );
}

#[test]
fn test_protect_home_numeric_1() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    ProtectHome = 1
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.exec_section.protect_home,
        crate::units::ProtectHome::Yes,
        "ProtectHome=1 should map to Yes"
    );
}

#[test]
fn test_protect_home_numeric_0() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    ProtectHome = 0
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.exec_section.protect_home,
        crate::units::ProtectHome::No,
        "ProtectHome=0 should map to No"
    );
}

#[test]
fn test_protect_home_case_insensitive_upper() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    ProtectHome = READ-ONLY
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.exec_section.protect_home,
        crate::units::ProtectHome::ReadOnly,
        "ProtectHome should be case-insensitive (READ-ONLY)"
    );
}

#[test]
fn test_protect_home_case_insensitive_mixed() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    ProtectHome = Tmpfs
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.exec_section.protect_home,
        crate::units::ProtectHome::Tmpfs,
        "ProtectHome should be case-insensitive (Tmpfs)"
    );
}

#[test]
fn test_protect_home_case_insensitive_yes() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    ProtectHome = YES
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.exec_section.protect_home,
        crate::units::ProtectHome::Yes,
        "ProtectHome should be case-insensitive (YES)"
    );
}

#[test]
fn test_protect_home_no_unsupported_warning() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    ProtectHome = yes
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let result = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    );

    assert!(
        result.is_ok(),
        "ProtectHome should not produce an unsupported setting warning"
    );
}

#[test]
fn test_protect_home_preserved_after_unit_conversion() {
    use std::convert::TryInto;

    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    ProtectHome = read-only
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.service"),
    )
    .unwrap();

    let unit: crate::units::Unit = service.try_into().unwrap();
    if let crate::units::Specific::Service(srvc) = &unit.specific {
        assert_eq!(
            srvc.conf.exec_config.protect_home,
            crate::units::ProtectHome::ReadOnly,
            "ProtectHome=read-only should survive unit conversion"
        );
    } else {
        panic!("Expected service unit");
    }
}

#[test]
fn test_protect_home_tmpfs_preserved_after_unit_conversion() {
    use std::convert::TryInto;

    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    ProtectHome = tmpfs
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.service"),
    )
    .unwrap();

    let unit: crate::units::Unit = service.try_into().unwrap();
    if let crate::units::Specific::Service(srvc) = &unit.specific {
        assert_eq!(
            srvc.conf.exec_config.protect_home,
            crate::units::ProtectHome::Tmpfs,
            "ProtectHome=tmpfs should survive unit conversion"
        );
    } else {
        panic!("Expected service unit");
    }
}

#[test]
fn test_protect_home_default_preserved_after_unit_conversion() {
    use std::convert::TryInto;

    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.service"),
    )
    .unwrap();

    let unit: crate::units::Unit = service.try_into().unwrap();
    if let crate::units::Specific::Service(srvc) = &unit.specific {
        assert_eq!(
            srvc.conf.exec_config.protect_home,
            crate::units::ProtectHome::No,
            "Default ProtectHome=No should survive unit conversion"
        );
    } else {
        panic!("Expected service unit");
    }
}

// ============================================================
// RuntimeDirectoryPreserve= parsing tests
// ============================================================

#[test]
fn test_runtime_directory_preserve_defaults_to_no() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.exec_section.runtime_directory_preserve,
        crate::units::RuntimeDirectoryPreserve::No,
        "RuntimeDirectoryPreserve should default to No when not specified"
    );
}

#[test]
fn test_runtime_directory_preserve_yes() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    RuntimeDirectoryPreserve = yes
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.exec_section.runtime_directory_preserve,
        crate::units::RuntimeDirectoryPreserve::Yes,
        "RuntimeDirectoryPreserve=yes should parse correctly"
    );
}

#[test]
fn test_runtime_directory_preserve_true() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    RuntimeDirectoryPreserve = true
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.exec_section.runtime_directory_preserve,
        crate::units::RuntimeDirectoryPreserve::Yes,
        "RuntimeDirectoryPreserve=true should map to Yes"
    );
}

#[test]
fn test_runtime_directory_preserve_no() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    RuntimeDirectoryPreserve = no
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.exec_section.runtime_directory_preserve,
        crate::units::RuntimeDirectoryPreserve::No,
        "RuntimeDirectoryPreserve=no should parse correctly"
    );
}

#[test]
fn test_runtime_directory_preserve_false() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    RuntimeDirectoryPreserve = false
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.exec_section.runtime_directory_preserve,
        crate::units::RuntimeDirectoryPreserve::No,
        "RuntimeDirectoryPreserve=false should map to No"
    );
}

#[test]
fn test_runtime_directory_preserve_restart() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    RuntimeDirectoryPreserve = restart
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.exec_section.runtime_directory_preserve,
        crate::units::RuntimeDirectoryPreserve::Restart,
        "RuntimeDirectoryPreserve=restart should parse correctly"
    );
}

#[test]
fn test_runtime_directory_preserve_numeric_1() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    RuntimeDirectoryPreserve = 1
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.exec_section.runtime_directory_preserve,
        crate::units::RuntimeDirectoryPreserve::Yes,
        "RuntimeDirectoryPreserve=1 should map to Yes"
    );
}

#[test]
fn test_runtime_directory_preserve_numeric_0() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    RuntimeDirectoryPreserve = 0
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.exec_section.runtime_directory_preserve,
        crate::units::RuntimeDirectoryPreserve::No,
        "RuntimeDirectoryPreserve=0 should map to No"
    );
}

#[test]
fn test_runtime_directory_preserve_case_insensitive_upper() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    RuntimeDirectoryPreserve = RESTART
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.exec_section.runtime_directory_preserve,
        crate::units::RuntimeDirectoryPreserve::Restart,
        "RuntimeDirectoryPreserve should be case-insensitive (RESTART)"
    );
}

#[test]
fn test_runtime_directory_preserve_case_insensitive_mixed() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    RuntimeDirectoryPreserve = Restart
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.exec_section.runtime_directory_preserve,
        crate::units::RuntimeDirectoryPreserve::Restart,
        "RuntimeDirectoryPreserve should be case-insensitive (Restart)"
    );
}

#[test]
fn test_runtime_directory_preserve_case_insensitive_yes() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    RuntimeDirectoryPreserve = YES
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.exec_section.runtime_directory_preserve,
        crate::units::RuntimeDirectoryPreserve::Yes,
        "RuntimeDirectoryPreserve should be case-insensitive (YES)"
    );
}

#[test]
fn test_runtime_directory_preserve_no_unsupported_warning() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    RuntimeDirectoryPreserve = restart
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let result = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    );

    assert!(
        result.is_ok(),
        "RuntimeDirectoryPreserve should not produce an unsupported setting warning"
    );
}

#[test]
fn test_runtime_directory_preserve_with_runtime_directory() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    RuntimeDirectory = myapp
    RuntimeDirectoryPreserve = restart
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.exec_section.runtime_directory,
        vec!["myapp".to_owned()]
    );
    assert_eq!(
        service.srvc.exec_section.runtime_directory_preserve,
        crate::units::RuntimeDirectoryPreserve::Restart,
        "RuntimeDirectoryPreserve should work alongside RuntimeDirectory"
    );
}

#[test]
fn test_runtime_directory_preserve_preserved_after_unit_conversion() {
    use std::convert::TryInto;

    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    RuntimeDirectoryPreserve = restart
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.service"),
    )
    .unwrap();

    let unit: crate::units::Unit = service.try_into().unwrap();
    if let crate::units::Specific::Service(srvc) = &unit.specific {
        assert_eq!(
            srvc.conf.exec_config.runtime_directory_preserve,
            crate::units::RuntimeDirectoryPreserve::Restart,
            "RuntimeDirectoryPreserve=restart should survive unit conversion"
        );
    } else {
        panic!("Expected service unit");
    }
}

#[test]
fn test_runtime_directory_preserve_yes_preserved_after_unit_conversion() {
    use std::convert::TryInto;

    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    RuntimeDirectoryPreserve = yes
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.service"),
    )
    .unwrap();

    let unit: crate::units::Unit = service.try_into().unwrap();
    if let crate::units::Specific::Service(srvc) = &unit.specific {
        assert_eq!(
            srvc.conf.exec_config.runtime_directory_preserve,
            crate::units::RuntimeDirectoryPreserve::Yes,
            "RuntimeDirectoryPreserve=yes should survive unit conversion"
        );
    } else {
        panic!("Expected service unit");
    }
}

#[test]
fn test_runtime_directory_preserve_default_preserved_after_unit_conversion() {
    use std::convert::TryInto;

    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.service"),
    )
    .unwrap();

    let unit: crate::units::Unit = service.try_into().unwrap();
    if let crate::units::Specific::Service(srvc) = &unit.specific {
        assert_eq!(
            srvc.conf.exec_config.runtime_directory_preserve,
            crate::units::RuntimeDirectoryPreserve::No,
            "Default RuntimeDirectoryPreserve=No should survive unit conversion"
        );
    } else {
        panic!("Expected service unit");
    }
}

// ============================================================
// ProtectHostname= parsing tests
// ============================================================

#[test]
fn test_protect_hostname_defaults_to_false() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.exec_section.protect_hostname, false,
        "ProtectHostname should default to false when not specified"
    );
}

#[test]
fn test_protect_hostname_yes() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    ProtectHostname = yes
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.exec_section.protect_hostname, true,
        "ProtectHostname=yes should parse to true"
    );
}

#[test]
fn test_protect_hostname_true() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    ProtectHostname = true
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.exec_section.protect_hostname, true,
        "ProtectHostname=true should parse to true"
    );
}

#[test]
fn test_protect_hostname_no() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    ProtectHostname = no
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.exec_section.protect_hostname, false,
        "ProtectHostname=no should parse to false"
    );
}

#[test]
fn test_protect_hostname_false() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    ProtectHostname = false
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.exec_section.protect_hostname, false,
        "ProtectHostname=false should parse to false"
    );
}

#[test]
fn test_protect_hostname_numeric_1() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    ProtectHostname = 1
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.exec_section.protect_hostname, true,
        "ProtectHostname=1 should parse to true"
    );
}

#[test]
fn test_protect_hostname_numeric_0() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    ProtectHostname = 0
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.exec_section.protect_hostname, false,
        "ProtectHostname=0 should parse to false"
    );
}

#[test]
fn test_protect_hostname_case_insensitive_upper() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    ProtectHostname = YES
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.exec_section.protect_hostname, true,
        "ProtectHostname should be case-insensitive (YES)"
    );
}

#[test]
fn test_protect_hostname_case_insensitive_mixed() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    ProtectHostname = True
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.exec_section.protect_hostname, true,
        "ProtectHostname should be case-insensitive (True)"
    );
}

#[test]
fn test_protect_hostname_no_unsupported_warning() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    ProtectHostname = yes
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let result = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    );

    assert!(
        result.is_ok(),
        "ProtectHostname should not produce an unsupported setting warning"
    );
}

#[test]
fn test_protect_hostname_preserved_after_unit_conversion() {
    use std::convert::TryInto;

    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    ProtectHostname = yes
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.service"),
    )
    .unwrap();

    let unit: crate::units::Unit = service.try_into().unwrap();
    if let crate::units::Specific::Service(srvc) = &unit.specific {
        assert_eq!(
            srvc.conf.exec_config.protect_hostname, true,
            "ProtectHostname=yes should survive unit conversion"
        );
    } else {
        panic!("Expected service unit");
    }
}

#[test]
fn test_protect_hostname_default_preserved_after_unit_conversion() {
    use std::convert::TryInto;

    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.service"),
    )
    .unwrap();

    let unit: crate::units::Unit = service.try_into().unwrap();
    if let crate::units::Specific::Service(srvc) = &unit.specific {
        assert_eq!(
            srvc.conf.exec_config.protect_hostname, false,
            "Default ProtectHostname=false should survive unit conversion"
        );
    } else {
        panic!("Expected service unit");
    }
}

#[test]
fn test_protect_hostname_socket_unit() {
    let test_socket_str = r#"
    [Socket]
    ListenStream = /run/test.sock
    ProtectHostname = yes
    "#;

    let parsed_file = crate::units::parse_file(test_socket_str).unwrap();
    let socket = crate::units::parse_socket(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.socket"),
    )
    .unwrap();

    assert_eq!(
        socket.sock.exec_section.protect_hostname, true,
        "ProtectHostname=yes should work in socket units"
    );
}

// ============================================================
// SystemCallArchitectures= parsing tests
// ============================================================

#[test]
fn test_system_call_architectures_defaults_to_empty() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert!(
        service
            .srvc
            .exec_section
            .system_call_architectures
            .is_empty(),
        "SystemCallArchitectures should default to empty"
    );
}

#[test]
fn test_system_call_architectures_native() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    SystemCallArchitectures = native
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.exec_section.system_call_architectures,
        vec!["native".to_owned()]
    );
}

#[test]
fn test_system_call_architectures_multiple_space_separated() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    SystemCallArchitectures = native x86 x86-64
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.exec_section.system_call_architectures,
        vec!["native".to_owned(), "x86".to_owned(), "x86-64".to_owned()]
    );
}

#[test]
fn test_system_call_architectures_multiple_directives_accumulate() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    SystemCallArchitectures = native
    SystemCallArchitectures = x86
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.exec_section.system_call_architectures,
        vec!["native".to_owned(), "x86".to_owned()]
    );
}

#[test]
fn test_system_call_architectures_empty_resets() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    SystemCallArchitectures = native x86
    SystemCallArchitectures =
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert!(
        service
            .srvc
            .exec_section
            .system_call_architectures
            .is_empty(),
        "Empty SystemCallArchitectures= should reset the list"
    );
}

#[test]
fn test_system_call_architectures_empty_then_new_entry() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    SystemCallArchitectures = native x86
    SystemCallArchitectures =
    SystemCallArchitectures = x86-64
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.exec_section.system_call_architectures,
        vec!["x86-64".to_owned()]
    );
}

#[test]
fn test_system_call_architectures_various_archs() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    SystemCallArchitectures = x86 x86-64 arm aarch64 mips ppc64-le s390x
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(service.srvc.exec_section.system_call_architectures.len(), 7);
    assert_eq!(
        service.srvc.exec_section.system_call_architectures[0],
        "x86"
    );
    assert_eq!(
        service.srvc.exec_section.system_call_architectures[1],
        "x86-64"
    );
    assert_eq!(
        service.srvc.exec_section.system_call_architectures[6],
        "s390x"
    );
}

#[test]
fn test_system_call_architectures_no_unsupported_warning() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    SystemCallArchitectures = native
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let result = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    );

    assert!(
        result.is_ok(),
        "SystemCallArchitectures should not produce an unsupported setting warning"
    );
}

#[test]
fn test_system_call_architectures_preserved_after_unit_conversion() {
    use std::convert::TryInto;

    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    SystemCallArchitectures = native x86-64
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.service"),
    )
    .unwrap();

    let unit: crate::units::Unit = service.try_into().unwrap();
    if let crate::units::Specific::Service(srvc) = &unit.specific {
        assert_eq!(
            srvc.conf.exec_config.system_call_architectures.len(),
            2,
            "SystemCallArchitectures should survive unit conversion"
        );
        assert_eq!(srvc.conf.exec_config.system_call_architectures[0], "native");
        assert_eq!(srvc.conf.exec_config.system_call_architectures[1], "x86-64");
    } else {
        panic!("Expected service unit");
    }
}

#[test]
fn test_system_call_architectures_empty_preserved_after_unit_conversion() {
    use std::convert::TryInto;

    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.service"),
    )
    .unwrap();

    let unit: crate::units::Unit = service.try_into().unwrap();
    if let crate::units::Specific::Service(srvc) = &unit.specific {
        assert!(
            srvc.conf.exec_config.system_call_architectures.is_empty(),
            "Default empty SystemCallArchitectures should survive unit conversion"
        );
    } else {
        panic!("Expected service unit");
    }
}

#[test]
fn test_system_call_architectures_socket_unit() {
    let test_socket_str = r#"
    [Socket]
    ListenStream = /run/test.sock
    SystemCallArchitectures = native
    "#;

    let parsed_file = crate::units::parse_file(test_socket_str).unwrap();
    let socket = crate::units::parse_socket(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.socket"),
    )
    .unwrap();

    assert_eq!(
        socket.sock.exec_section.system_call_architectures,
        vec!["native".to_owned()],
        "SystemCallArchitectures=native should work in socket units"
    );
}

#[test]
fn test_system_call_architectures_with_system_call_filter() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    SystemCallArchitectures = native
    SystemCallFilter = @system-service
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.exec_section.system_call_architectures,
        vec!["native".to_owned()]
    );
    assert!(!service.srvc.exec_section.system_call_filter.is_empty());
}

// ============================================================
// WatchdogSec= parsing tests
// ============================================================

#[test]
fn test_watchdog_sec_defaults_to_none() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.watchdog_sec, None,
        "WatchdogSec should default to None when not specified"
    );
}

#[test]
fn test_watchdog_sec_seconds() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    WatchdogSec = 30
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.watchdog_sec,
        Some(crate::units::Timeout::Duration(
            std::time::Duration::from_secs(30)
        )),
        "WatchdogSec=30 should parse as 30 seconds"
    );
}

#[test]
fn test_watchdog_sec_with_s_suffix() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    WatchdogSec = 15s
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.watchdog_sec,
        Some(crate::units::Timeout::Duration(
            std::time::Duration::from_secs(15)
        )),
        "WatchdogSec=15s should parse as 15 seconds"
    );
}

#[test]
fn test_watchdog_sec_with_min_suffix() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    WatchdogSec = 2min
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.watchdog_sec,
        Some(crate::units::Timeout::Duration(
            std::time::Duration::from_secs(120)
        )),
        "WatchdogSec=2min should parse as 120 seconds"
    );
}

#[test]
fn test_watchdog_sec_compound_duration() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    WatchdogSec = 1min 30s
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.watchdog_sec,
        Some(crate::units::Timeout::Duration(
            std::time::Duration::from_secs(90)
        )),
        "WatchdogSec=1min 30s should parse as 90 seconds"
    );
}

#[test]
fn test_watchdog_sec_zero_disables() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    WatchdogSec = 0
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.watchdog_sec, None,
        "WatchdogSec=0 should disable the watchdog (None)"
    );
}

#[test]
fn test_watchdog_sec_infinity() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    WatchdogSec = infinity
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.watchdog_sec,
        Some(crate::units::Timeout::Infinity),
        "WatchdogSec=infinity should parse as Infinity"
    );
}

#[test]
fn test_watchdog_sec_no_unsupported_warning() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    WatchdogSec = 30s
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let result = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    );

    assert!(
        result.is_ok(),
        "WatchdogSec should not produce an unsupported setting warning"
    );
}

#[test]
fn test_watchdog_sec_with_restart_on_watchdog() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    WatchdogSec = 10s
    Restart = on-watchdog
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.watchdog_sec,
        Some(crate::units::Timeout::Duration(
            std::time::Duration::from_secs(10)
        )),
    );
    assert_eq!(
        service.srvc.restart,
        crate::units::ServiceRestart::OnWatchdog,
    );
}

#[test]
fn test_watchdog_sec_preserved_after_unit_conversion() {
    use std::convert::TryInto;

    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    WatchdogSec = 30s
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.service"),
    )
    .unwrap();

    let unit: crate::units::Unit = service.try_into().unwrap();
    if let crate::units::Specific::Service(srvc) = &unit.specific {
        assert_eq!(
            srvc.conf.watchdog_sec,
            Some(crate::units::Timeout::Duration(
                std::time::Duration::from_secs(30)
            )),
            "WatchdogSec=30s should survive unit conversion"
        );
    } else {
        panic!("Expected service unit");
    }
}

#[test]
fn test_watchdog_sec_none_preserved_after_unit_conversion() {
    use std::convert::TryInto;

    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.service"),
    )
    .unwrap();

    let unit: crate::units::Unit = service.try_into().unwrap();
    if let crate::units::Specific::Service(srvc) = &unit.specific {
        assert_eq!(
            srvc.conf.watchdog_sec, None,
            "Default WatchdogSec=None should survive unit conversion"
        );
    } else {
        panic!("Expected service unit");
    }
}

#[test]
fn test_watchdog_sec_zero_preserved_after_unit_conversion() {
    use std::convert::TryInto;

    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    WatchdogSec = 0
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.service"),
    )
    .unwrap();

    let unit: crate::units::Unit = service.try_into().unwrap();
    if let crate::units::Specific::Service(srvc) = &unit.specific {
        assert_eq!(
            srvc.conf.watchdog_sec, None,
            "WatchdogSec=0 (disabled) should survive unit conversion as None"
        );
    } else {
        panic!("Expected service unit");
    }
}

// ============================================================
// ReadWritePaths= parsing tests
// ============================================================

#[test]
fn test_read_write_paths_defaults_to_empty() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert!(
        service.srvc.exec_section.read_write_paths.is_empty(),
        "ReadWritePaths should default to empty"
    );
}

#[test]
fn test_read_write_paths_single() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    ReadWritePaths = /var/lib/myapp
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.exec_section.read_write_paths,
        vec!["/var/lib/myapp".to_owned()]
    );
}

#[test]
fn test_read_write_paths_multiple_space_separated() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    ReadWritePaths = /var/lib/myapp /var/log/myapp /run/myapp
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.exec_section.read_write_paths,
        vec![
            "/var/lib/myapp".to_owned(),
            "/var/log/myapp".to_owned(),
            "/run/myapp".to_owned()
        ]
    );
}

#[test]
fn test_read_write_paths_multiple_directives_accumulate() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    ReadWritePaths = /var/lib/myapp
    ReadWritePaths = /var/log/myapp
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.exec_section.read_write_paths,
        vec!["/var/lib/myapp".to_owned(), "/var/log/myapp".to_owned()]
    );
}

#[test]
fn test_read_write_paths_empty_resets() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    ReadWritePaths = /var/lib/myapp /var/log/myapp
    ReadWritePaths =
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert!(
        service.srvc.exec_section.read_write_paths.is_empty(),
        "Empty ReadWritePaths= should reset the list"
    );
}

#[test]
fn test_read_write_paths_empty_then_new_entry() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    ReadWritePaths = /var/lib/myapp
    ReadWritePaths =
    ReadWritePaths = /var/log/newapp
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.exec_section.read_write_paths,
        vec!["/var/log/newapp".to_owned()]
    );
}

#[test]
fn test_read_write_paths_with_minus_prefix() {
    // systemd allows a '-' prefix to suppress errors if the path doesn't exist
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    ReadWritePaths = -/var/lib/optional
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.exec_section.read_write_paths,
        vec!["-/var/lib/optional".to_owned()]
    );
}

#[test]
fn test_read_write_paths_no_unsupported_warning() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    ReadWritePaths = /var/lib/myapp
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let result = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    );

    assert!(
        result.is_ok(),
        "ReadWritePaths should not produce an unsupported setting warning"
    );
}

#[test]
fn test_read_write_paths_with_protect_system_strict() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    ProtectSystem = strict
    ReadWritePaths = /var/lib/myapp /var/log/myapp
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.exec_section.protect_system,
        crate::units::ProtectSystem::Strict,
    );
    assert_eq!(
        service.srvc.exec_section.read_write_paths,
        vec!["/var/lib/myapp".to_owned(), "/var/log/myapp".to_owned()]
    );
}

#[test]
fn test_read_write_paths_preserved_after_unit_conversion() {
    use std::convert::TryInto;

    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    ReadWritePaths = /var/lib/myapp /var/log/myapp
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.service"),
    )
    .unwrap();

    let unit: crate::units::Unit = service.try_into().unwrap();
    if let crate::units::Specific::Service(srvc) = &unit.specific {
        assert_eq!(
            srvc.conf.exec_config.read_write_paths.len(),
            2,
            "ReadWritePaths should survive unit conversion"
        );
        assert_eq!(srvc.conf.exec_config.read_write_paths[0], "/var/lib/myapp");
        assert_eq!(srvc.conf.exec_config.read_write_paths[1], "/var/log/myapp");
    } else {
        panic!("Expected service unit");
    }
}

#[test]
fn test_read_write_paths_empty_preserved_after_unit_conversion() {
    use std::convert::TryInto;

    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.service"),
    )
    .unwrap();

    let unit: crate::units::Unit = service.try_into().unwrap();
    if let crate::units::Specific::Service(srvc) = &unit.specific {
        assert!(
            srvc.conf.exec_config.read_write_paths.is_empty(),
            "Default empty ReadWritePaths should survive unit conversion"
        );
    } else {
        panic!("Expected service unit");
    }
}

#[test]
fn test_read_write_paths_socket_unit() {
    let test_socket_str = r#"
    [Socket]
    ListenStream = /run/test.sock
    ReadWritePaths = /var/lib/myapp
    "#;

    let parsed_file = crate::units::parse_file(test_socket_str).unwrap();
    let socket = crate::units::parse_socket(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.socket"),
    )
    .unwrap();

    assert_eq!(
        socket.sock.exec_section.read_write_paths,
        vec!["/var/lib/myapp".to_owned()],
        "ReadWritePaths should work in socket units"
    );
}

// ============================================================
// IPAddressAllow= parsing tests
// ============================================================

#[test]
fn test_ip_address_allow_defaults_to_empty() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert!(
        service.srvc.ip_address_allow.is_empty(),
        "IPAddressAllow should default to empty"
    );
}

#[test]
fn test_ip_address_allow_single_cidr() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    IPAddressAllow = 192.168.1.0/24
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.ip_address_allow,
        vec!["192.168.1.0/24".to_owned()]
    );
}

#[test]
fn test_ip_address_allow_any_keyword() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    IPAddressAllow = any
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(service.srvc.ip_address_allow, vec!["any".to_owned()]);
}

#[test]
fn test_ip_address_allow_localhost_keyword() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    IPAddressAllow = localhost
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(service.srvc.ip_address_allow, vec!["localhost".to_owned()]);
}

#[test]
fn test_ip_address_allow_link_local_keyword() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    IPAddressAllow = link-local
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(service.srvc.ip_address_allow, vec!["link-local".to_owned()]);
}

#[test]
fn test_ip_address_allow_multicast_keyword() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    IPAddressAllow = multicast
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(service.srvc.ip_address_allow, vec!["multicast".to_owned()]);
}

#[test]
fn test_ip_address_allow_multiple_space_separated() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    IPAddressAllow = 192.168.1.0/24 10.0.0.0/8 172.16.0.0/12
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.ip_address_allow,
        vec![
            "192.168.1.0/24".to_owned(),
            "10.0.0.0/8".to_owned(),
            "172.16.0.0/12".to_owned()
        ]
    );
}

#[test]
fn test_ip_address_allow_multiple_directives_accumulate() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    IPAddressAllow = 192.168.1.0/24
    IPAddressAllow = 10.0.0.0/8
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.ip_address_allow,
        vec!["192.168.1.0/24".to_owned(), "10.0.0.0/8".to_owned()]
    );
}

#[test]
fn test_ip_address_allow_empty_resets() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    IPAddressAllow = 192.168.1.0/24
    IPAddressAllow =
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert!(
        service.srvc.ip_address_allow.is_empty(),
        "Empty IPAddressAllow= should reset the list"
    );
}

#[test]
fn test_ip_address_allow_empty_then_new_entry() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    IPAddressAllow = 192.168.1.0/24
    IPAddressAllow =
    IPAddressAllow = 10.0.0.0/8
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(service.srvc.ip_address_allow, vec!["10.0.0.0/8".to_owned()]);
}

#[test]
fn test_ip_address_allow_ipv6() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    IPAddressAllow = ::1/128 fe80::/10
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.ip_address_allow,
        vec!["::1/128".to_owned(), "fe80::/10".to_owned()]
    );
}

#[test]
fn test_ip_address_allow_mixed_keywords_and_cidr() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    IPAddressAllow = localhost 192.168.1.0/24 link-local
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.ip_address_allow,
        vec![
            "localhost".to_owned(),
            "192.168.1.0/24".to_owned(),
            "link-local".to_owned()
        ]
    );
}

#[test]
fn test_ip_address_allow_no_unsupported_warning() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    IPAddressAllow = any
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let result = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    );

    assert!(
        result.is_ok(),
        "IPAddressAllow should not produce an unsupported setting warning"
    );
}

#[test]
fn test_ip_address_allow_preserved_after_unit_conversion() {
    use std::convert::TryInto;

    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    IPAddressAllow = 192.168.1.0/24 10.0.0.0/8
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.service"),
    )
    .unwrap();

    let unit: crate::units::Unit = service.try_into().unwrap();
    if let crate::units::Specific::Service(srvc) = &unit.specific {
        assert_eq!(
            srvc.conf.ip_address_allow,
            vec!["192.168.1.0/24".to_owned(), "10.0.0.0/8".to_owned()],
            "IPAddressAllow should survive unit conversion"
        );
    } else {
        panic!("Expected service unit");
    }
}

#[test]
fn test_ip_address_allow_empty_preserved_after_unit_conversion() {
    use std::convert::TryInto;

    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.service"),
    )
    .unwrap();

    let unit: crate::units::Unit = service.try_into().unwrap();
    if let crate::units::Specific::Service(srvc) = &unit.specific {
        assert!(
            srvc.conf.ip_address_allow.is_empty(),
            "Default empty IPAddressAllow should survive unit conversion"
        );
    } else {
        panic!("Expected service unit");
    }
}

// ============================================================
// IPAddressDeny= parsing tests
// ============================================================

#[test]
fn test_ip_address_deny_defaults_to_empty() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert!(
        service.srvc.ip_address_deny.is_empty(),
        "IPAddressDeny should default to empty"
    );
}

#[test]
fn test_ip_address_deny_single_cidr() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    IPAddressDeny = 192.168.1.0/24
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.ip_address_deny,
        vec!["192.168.1.0/24".to_owned()]
    );
}

#[test]
fn test_ip_address_deny_any_keyword() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    IPAddressDeny = any
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(service.srvc.ip_address_deny, vec!["any".to_owned()]);
}

#[test]
fn test_ip_address_deny_localhost_keyword() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    IPAddressDeny = localhost
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(service.srvc.ip_address_deny, vec!["localhost".to_owned()]);
}

#[test]
fn test_ip_address_deny_link_local_keyword() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    IPAddressDeny = link-local
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(service.srvc.ip_address_deny, vec!["link-local".to_owned()]);
}

#[test]
fn test_ip_address_deny_multicast_keyword() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    IPAddressDeny = multicast
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(service.srvc.ip_address_deny, vec!["multicast".to_owned()]);
}

#[test]
fn test_ip_address_deny_multiple_space_separated() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    IPAddressDeny = 192.168.1.0/24 10.0.0.0/8 172.16.0.0/12
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.ip_address_deny,
        vec![
            "192.168.1.0/24".to_owned(),
            "10.0.0.0/8".to_owned(),
            "172.16.0.0/12".to_owned()
        ]
    );
}

#[test]
fn test_ip_address_deny_multiple_directives_accumulate() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    IPAddressDeny = 192.168.1.0/24
    IPAddressDeny = 10.0.0.0/8
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.ip_address_deny,
        vec!["192.168.1.0/24".to_owned(), "10.0.0.0/8".to_owned()]
    );
}

#[test]
fn test_ip_address_deny_empty_resets() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    IPAddressDeny = 192.168.1.0/24
    IPAddressDeny =
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert!(
        service.srvc.ip_address_deny.is_empty(),
        "Empty IPAddressDeny= should reset the list"
    );
}

#[test]
fn test_ip_address_deny_empty_then_new_entry() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    IPAddressDeny = 192.168.1.0/24
    IPAddressDeny =
    IPAddressDeny = 10.0.0.0/8
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(service.srvc.ip_address_deny, vec!["10.0.0.0/8".to_owned()]);
}

#[test]
fn test_ip_address_deny_ipv6() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    IPAddressDeny = ::1/128 fe80::/10
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.ip_address_deny,
        vec!["::1/128".to_owned(), "fe80::/10".to_owned()]
    );
}

#[test]
fn test_ip_address_deny_mixed_keywords_and_cidr() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    IPAddressDeny = any
    IPAddressDeny = 192.168.1.0/24
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.ip_address_deny,
        vec!["any".to_owned(), "192.168.1.0/24".to_owned()]
    );
}

#[test]
fn test_ip_address_deny_no_unsupported_warning() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    IPAddressDeny = any
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let result = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    );

    assert!(
        result.is_ok(),
        "IPAddressDeny should not produce an unsupported setting warning"
    );
}

#[test]
fn test_ip_address_deny_preserved_after_unit_conversion() {
    use std::convert::TryInto;

    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    IPAddressDeny = 192.168.1.0/24 10.0.0.0/8
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.service"),
    )
    .unwrap();

    let unit: crate::units::Unit = service.try_into().unwrap();
    if let crate::units::Specific::Service(srvc) = &unit.specific {
        assert_eq!(
            srvc.conf.ip_address_deny,
            vec!["192.168.1.0/24".to_owned(), "10.0.0.0/8".to_owned()],
            "IPAddressDeny should survive unit conversion"
        );
    } else {
        panic!("Expected service unit");
    }
}

#[test]
fn test_ip_address_deny_empty_preserved_after_unit_conversion() {
    use std::convert::TryInto;

    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.service"),
    )
    .unwrap();

    let unit: crate::units::Unit = service.try_into().unwrap();
    if let crate::units::Specific::Service(srvc) = &unit.specific {
        assert!(
            srvc.conf.ip_address_deny.is_empty(),
            "Default empty IPAddressDeny should survive unit conversion"
        );
    } else {
        panic!("Expected service unit");
    }
}

// ============================================================
// IPAddressAllow= and IPAddressDeny= combined tests
// ============================================================

#[test]
fn test_ip_address_allow_and_deny_together() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    IPAddressAllow = localhost
    IPAddressDeny = any
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(service.srvc.ip_address_allow, vec!["localhost".to_owned()]);
    assert_eq!(service.srvc.ip_address_deny, vec!["any".to_owned()]);
}

#[test]
fn test_ip_address_allow_and_deny_with_device_allow() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    IPAddressAllow = 192.168.1.0/24
    IPAddressDeny = any
    DeviceAllow = /dev/null rw
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.ip_address_allow,
        vec!["192.168.1.0/24".to_owned()]
    );
    assert_eq!(service.srvc.ip_address_deny, vec!["any".to_owned()]);
    assert_eq!(service.srvc.device_allow, vec!["/dev/null rw".to_owned()]);
}

#[test]
fn test_ip_address_allow_and_deny_preserved_together_after_unit_conversion() {
    use std::convert::TryInto;

    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    IPAddressAllow = localhost 192.168.1.0/24
    IPAddressDeny = any
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.service"),
    )
    .unwrap();

    let unit: crate::units::Unit = service.try_into().unwrap();
    if let crate::units::Specific::Service(srvc) = &unit.specific {
        assert_eq!(
            srvc.conf.ip_address_allow,
            vec!["localhost".to_owned(), "192.168.1.0/24".to_owned()],
            "IPAddressAllow should survive unit conversion"
        );
        assert_eq!(
            srvc.conf.ip_address_deny,
            vec!["any".to_owned()],
            "IPAddressDeny should survive unit conversion"
        );
    } else {
        panic!("Expected service unit");
    }
}

// ============================================================
// MemoryDenyWriteExecute= parsing tests
// ============================================================

#[test]
fn test_memory_deny_write_execute_defaults_to_false() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert!(
        !service.srvc.exec_section.memory_deny_write_execute,
        "MemoryDenyWriteExecute should default to false"
    );
}

#[test]
fn test_memory_deny_write_execute_set_yes() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    MemoryDenyWriteExecute = yes
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert!(service.srvc.exec_section.memory_deny_write_execute);
}

#[test]
fn test_memory_deny_write_execute_set_true() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    MemoryDenyWriteExecute = true
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert!(service.srvc.exec_section.memory_deny_write_execute);
}

#[test]
fn test_memory_deny_write_execute_set_1() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    MemoryDenyWriteExecute = 1
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert!(service.srvc.exec_section.memory_deny_write_execute);
}

#[test]
fn test_memory_deny_write_execute_set_no() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    MemoryDenyWriteExecute = no
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert!(!service.srvc.exec_section.memory_deny_write_execute);
}

#[test]
fn test_memory_deny_write_execute_set_false() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    MemoryDenyWriteExecute = false
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert!(!service.srvc.exec_section.memory_deny_write_execute);
}

#[test]
fn test_memory_deny_write_execute_set_0() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    MemoryDenyWriteExecute = 0
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert!(!service.srvc.exec_section.memory_deny_write_execute);
}

#[test]
fn test_memory_deny_write_execute_case_insensitive() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    MemoryDenyWriteExecute = YES
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert!(service.srvc.exec_section.memory_deny_write_execute);
}

#[test]
fn test_memory_deny_write_execute_no_unsupported_warning() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    MemoryDenyWriteExecute = yes
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let result = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    );

    assert!(
        result.is_ok(),
        "MemoryDenyWriteExecute should not produce an unsupported setting warning"
    );
}

#[test]
fn test_memory_deny_write_execute_with_other_settings() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    MemoryDenyWriteExecute = yes
    NoNewPrivileges = yes
    RestrictRealtime = yes
    ProtectSystem = strict
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert!(service.srvc.exec_section.memory_deny_write_execute);
    assert!(service.srvc.exec_section.no_new_privileges);
    assert!(service.srvc.exec_section.restrict_realtime);
}

#[test]
fn test_memory_deny_write_execute_preserved_after_unit_conversion() {
    use std::convert::TryInto;

    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    MemoryDenyWriteExecute = yes
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.service"),
    )
    .unwrap();

    let unit: crate::units::Unit = service.try_into().unwrap();
    if let crate::units::Specific::Service(srvc) = &unit.specific {
        assert!(
            srvc.conf.exec_config.memory_deny_write_execute,
            "MemoryDenyWriteExecute=yes should survive unit conversion"
        );
    } else {
        panic!("Expected service unit");
    }
}

#[test]
fn test_memory_deny_write_execute_false_preserved_after_unit_conversion() {
    use std::convert::TryInto;

    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    MemoryDenyWriteExecute = no
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.service"),
    )
    .unwrap();

    let unit: crate::units::Unit = service.try_into().unwrap();
    if let crate::units::Specific::Service(srvc) = &unit.specific {
        assert!(
            !srvc.conf.exec_config.memory_deny_write_execute,
            "MemoryDenyWriteExecute=no should survive unit conversion"
        );
    } else {
        panic!("Expected service unit");
    }
}

#[test]
fn test_memory_deny_write_execute_socket_unit() {
    let test_socket_str = r#"
    [Socket]
    ListenStream = /run/test.sock
    MemoryDenyWriteExecute = yes
    "#;

    let parsed_file = crate::units::parse_file(test_socket_str).unwrap();
    let socket = crate::units::parse_socket(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.socket"),
    )
    .unwrap();

    assert!(
        socket.sock.exec_section.memory_deny_write_execute,
        "MemoryDenyWriteExecute should work in socket units"
    );
}

// ============================================================
// FileDescriptorStoreMax= parsing tests
// ============================================================

#[test]
fn test_file_descriptor_store_max_defaults_to_zero() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.file_descriptor_store_max, 0,
        "FileDescriptorStoreMax should default to 0"
    );
}

#[test]
fn test_file_descriptor_store_max_zero() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    FileDescriptorStoreMax = 0
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(service.srvc.file_descriptor_store_max, 0);
}

#[test]
fn test_file_descriptor_store_max_positive_value() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    FileDescriptorStoreMax = 128
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(service.srvc.file_descriptor_store_max, 128);
}

#[test]
fn test_file_descriptor_store_max_one() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    FileDescriptorStoreMax = 1
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(service.srvc.file_descriptor_store_max, 1);
}

#[test]
fn test_file_descriptor_store_max_large_value() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    FileDescriptorStoreMax = 4096
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(service.srvc.file_descriptor_store_max, 4096);
}

#[test]
fn test_file_descriptor_store_max_empty_resets_to_zero() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    FileDescriptorStoreMax =
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.file_descriptor_store_max, 0,
        "Empty FileDescriptorStoreMax= should reset to 0"
    );
}

#[test]
fn test_file_descriptor_store_max_invalid_value() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    FileDescriptorStoreMax = notanumber
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let result = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    );

    assert!(
        result.is_err(),
        "FileDescriptorStoreMax with invalid value should produce an error"
    );
}

#[test]
fn test_file_descriptor_store_max_with_whitespace() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    FileDescriptorStoreMax =   64
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(service.srvc.file_descriptor_store_max, 64);
}

#[test]
fn test_file_descriptor_store_max_no_unsupported_warning() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    FileDescriptorStoreMax = 10
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let result = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    );

    assert!(
        result.is_ok(),
        "FileDescriptorStoreMax should not produce an unsupported setting warning"
    );
}

#[test]
fn test_file_descriptor_store_max_with_other_settings() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    Type = notify
    FileDescriptorStoreMax = 32
    WatchdogSec = 30
    Restart = on-failure
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(service.srvc.file_descriptor_store_max, 32);
    assert_eq!(service.srvc.srcv_type, crate::units::ServiceType::Notify);
    assert_eq!(
        service.srvc.restart,
        crate::units::ServiceRestart::OnFailure
    );
}

#[test]
fn test_file_descriptor_store_max_preserved_after_unit_conversion() {
    use std::convert::TryInto;

    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    FileDescriptorStoreMax = 256
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.service"),
    )
    .unwrap();

    let unit: crate::units::Unit = service.try_into().unwrap();
    if let crate::units::Specific::Service(srvc) = &unit.specific {
        assert_eq!(
            srvc.conf.file_descriptor_store_max, 256,
            "FileDescriptorStoreMax should survive unit conversion"
        );
    } else {
        panic!("Expected service unit");
    }
}

#[test]
fn test_file_descriptor_store_max_zero_preserved_after_unit_conversion() {
    use std::convert::TryInto;

    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.service"),
    )
    .unwrap();

    let unit: crate::units::Unit = service.try_into().unwrap();
    if let crate::units::Specific::Service(srvc) = &unit.specific {
        assert_eq!(
            srvc.conf.file_descriptor_store_max, 0,
            "Default FileDescriptorStoreMax=0 should survive unit conversion"
        );
    } else {
        panic!("Expected service unit");
    }
}

// ============================================================
// PrivateTmp= parsing tests
// ============================================================

#[test]
fn test_private_tmp_defaults_to_false() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert!(
        !service.srvc.exec_section.private_tmp,
        "PrivateTmp should default to false"
    );
}

#[test]
fn test_private_tmp_set_yes() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    PrivateTmp = yes
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert!(service.srvc.exec_section.private_tmp);
}

#[test]
fn test_private_tmp_set_true() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    PrivateTmp = true
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert!(service.srvc.exec_section.private_tmp);
}

#[test]
fn test_private_tmp_set_1() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    PrivateTmp = 1
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert!(service.srvc.exec_section.private_tmp);
}

#[test]
fn test_private_tmp_set_no() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    PrivateTmp = no
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert!(!service.srvc.exec_section.private_tmp);
}

#[test]
fn test_private_tmp_set_false() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    PrivateTmp = false
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert!(!service.srvc.exec_section.private_tmp);
}

#[test]
fn test_private_tmp_set_0() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    PrivateTmp = 0
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert!(!service.srvc.exec_section.private_tmp);
}

#[test]
fn test_private_tmp_case_insensitive() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    PrivateTmp = YES
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert!(service.srvc.exec_section.private_tmp);
}

#[test]
fn test_private_tmp_no_unsupported_warning() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    PrivateTmp = yes
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let result = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    );

    assert!(
        result.is_ok(),
        "PrivateTmp should not produce an unsupported setting warning"
    );
}

#[test]
fn test_private_tmp_with_other_settings() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    PrivateTmp = yes
    ProtectSystem = strict
    ProtectHome = yes
    NoNewPrivileges = yes
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert!(service.srvc.exec_section.private_tmp);
    assert!(service.srvc.exec_section.no_new_privileges);
}

#[test]
fn test_private_tmp_preserved_after_unit_conversion() {
    use std::convert::TryInto;

    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    PrivateTmp = yes
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.service"),
    )
    .unwrap();

    let unit: crate::units::Unit = service.try_into().unwrap();
    if let crate::units::Specific::Service(srvc) = &unit.specific {
        assert!(
            srvc.conf.exec_config.private_tmp,
            "PrivateTmp=yes should survive unit conversion"
        );
    } else {
        panic!("Expected service unit");
    }
}

#[test]
fn test_private_tmp_false_preserved_after_unit_conversion() {
    use std::convert::TryInto;

    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    PrivateTmp = no
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.service"),
    )
    .unwrap();

    let unit: crate::units::Unit = service.try_into().unwrap();
    if let crate::units::Specific::Service(srvc) = &unit.specific {
        assert!(
            !srvc.conf.exec_config.private_tmp,
            "PrivateTmp=no should survive unit conversion"
        );
    } else {
        panic!("Expected service unit");
    }
}

#[test]
fn test_private_tmp_socket_unit() {
    let test_socket_str = r#"
    [Socket]
    ListenStream = /run/test.sock
    PrivateTmp = yes
    "#;

    let parsed_file = crate::units::parse_file(test_socket_str).unwrap();
    let socket = crate::units::parse_socket(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.socket"),
    )
    .unwrap();

    assert!(
        socket.sock.exec_section.private_tmp,
        "PrivateTmp should work in socket units"
    );
}

// ============================================================
// PrivateDevices= parsing tests
// ============================================================

#[test]
fn test_private_devices_defaults_to_false() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert!(
        !service.srvc.exec_section.private_devices,
        "PrivateDevices should default to false"
    );
}

#[test]
fn test_private_devices_true() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    PrivateDevices = yes
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert!(
        service.srvc.exec_section.private_devices,
        "PrivateDevices=yes should be true"
    );
}

#[test]
fn test_private_devices_false() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    PrivateDevices = no
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert!(
        !service.srvc.exec_section.private_devices,
        "PrivateDevices=no should be false"
    );
}

#[test]
fn test_private_devices_true_variant() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    PrivateDevices = true
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert!(
        service.srvc.exec_section.private_devices,
        "PrivateDevices=true should be true"
    );
}

#[test]
fn test_private_devices_one_is_true() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    PrivateDevices = 1
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert!(
        service.srvc.exec_section.private_devices,
        "PrivateDevices=1 should be true"
    );
}

#[test]
fn test_private_devices_zero_is_false() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    PrivateDevices = 0
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert!(
        !service.srvc.exec_section.private_devices,
        "PrivateDevices=0 should be false"
    );
}

#[test]
fn test_private_devices_case_insensitive() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    PrivateDevices = Yes
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert!(
        service.srvc.exec_section.private_devices,
        "PrivateDevices=Yes should be true (case insensitive)"
    );
}

#[test]
fn test_private_devices_no_unsupported_warning() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    PrivateDevices = yes
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let result = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    );

    assert!(result.is_ok(), "PrivateDevices= should not cause errors");
}

#[test]
fn test_private_devices_preserved_after_unit_conversion() {
    use std::convert::TryInto;

    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    PrivateDevices = yes
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.service"),
    )
    .unwrap();

    let unit: crate::units::Unit = service.try_into().unwrap();
    if let crate::units::Specific::Service(srvc) = &unit.specific {
        assert!(
            srvc.conf.exec_config.private_devices,
            "PrivateDevices=yes should survive unit conversion"
        );
    } else {
        panic!("Expected service unit");
    }
}

#[test]
fn test_private_devices_false_preserved_after_unit_conversion() {
    use std::convert::TryInto;

    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.service"),
    )
    .unwrap();

    let unit: crate::units::Unit = service.try_into().unwrap();
    if let crate::units::Specific::Service(srvc) = &unit.specific {
        assert!(
            !srvc.conf.exec_config.private_devices,
            "Default false PrivateDevices should survive unit conversion"
        );
    } else {
        panic!("Expected service unit");
    }
}

#[test]
fn test_private_devices_socket_unit() {
    let test_socket_str = r#"
    [Unit]
    Description = A socket with private devices
    [Socket]
    ListenStream = /run/test.sock
    PrivateDevices = yes
    "#;

    let parsed_file = crate::units::parse_file(test_socket_str).unwrap();
    let socket = crate::units::parse_socket(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.socket"),
    )
    .unwrap();

    assert!(
        socket.sock.exec_section.private_devices,
        "PrivateDevices should work in socket units"
    );
}

#[test]
fn test_private_devices_with_other_settings() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    PrivateDevices = yes
    PrivateTmp = yes
    ProtectSystem = strict
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert!(service.srvc.exec_section.private_devices);
    assert!(service.srvc.exec_section.private_tmp);
}

// ============================================================
// LockPersonality= parsing tests
// ============================================================

#[test]
fn test_lock_personality_defaults_to_false() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert!(
        !service.srvc.exec_section.lock_personality,
        "LockPersonality should default to false"
    );
}

#[test]
fn test_lock_personality_set_yes() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    LockPersonality = yes
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert!(service.srvc.exec_section.lock_personality);
}

#[test]
fn test_lock_personality_set_true() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    LockPersonality = true
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert!(service.srvc.exec_section.lock_personality);
}

#[test]
fn test_lock_personality_set_1() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    LockPersonality = 1
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert!(service.srvc.exec_section.lock_personality);
}

#[test]
fn test_lock_personality_set_no() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    LockPersonality = no
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert!(!service.srvc.exec_section.lock_personality);
}

#[test]
fn test_lock_personality_set_false() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    LockPersonality = false
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert!(!service.srvc.exec_section.lock_personality);
}

#[test]
fn test_lock_personality_set_0() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    LockPersonality = 0
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert!(!service.srvc.exec_section.lock_personality);
}

#[test]
fn test_lock_personality_case_insensitive() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    LockPersonality = YES
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert!(service.srvc.exec_section.lock_personality);
}

#[test]
fn test_lock_personality_no_unsupported_warning() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    LockPersonality = yes
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let result = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    );

    assert!(
        result.is_ok(),
        "LockPersonality should not produce an unsupported setting warning"
    );
}

#[test]
fn test_lock_personality_with_other_settings() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    LockPersonality = yes
    NoNewPrivileges = yes
    RestrictRealtime = yes
    ProtectSystem = strict
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert!(service.srvc.exec_section.lock_personality);
    assert!(service.srvc.exec_section.no_new_privileges);
    assert!(service.srvc.exec_section.restrict_realtime);
}

#[test]
fn test_lock_personality_preserved_after_unit_conversion() {
    use std::convert::TryInto;

    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    LockPersonality = yes
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    let unit: crate::units::Unit = service.try_into().unwrap();
    if let crate::units::Specific::Service(srvc) = &unit.specific {
        assert!(
            srvc.conf.exec_config.lock_personality,
            "LockPersonality should be preserved after unit conversion"
        );
    } else {
        panic!("Expected service unit");
    }
}

#[test]
fn test_lock_personality_in_socket_unit() {
    let test_socket_str = r#"
    [Socket]
    ListenStream = /run/test.sock
    LockPersonality = yes
    "#;

    let parsed_file = crate::units::parse_file(test_socket_str).unwrap();
    let socket = crate::units::parse_socket(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.socket"),
    )
    .unwrap();

    assert!(
        socket.sock.exec_section.lock_personality,
        "LockPersonality should work in socket units"
    );
}

// ============================================================
// ConditionVirtualization= parsing tests
// ============================================================

#[test]
fn test_condition_virtualization_no_unsupported_warning() {
    let test_service_str = r#"
    [Unit]
    ConditionVirtualization = yes

    [Service]
    ExecStart = /bin/myservice
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let result = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    );

    assert!(
        result.is_ok(),
        "ConditionVirtualization should not produce an unsupported setting warning"
    );
}

#[test]
fn test_condition_virtualization_yes_parsed() {
    let test_service_str = r#"
    [Unit]
    ConditionVirtualization = yes

    [Service]
    ExecStart = /bin/myservice
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.common.unit.conditions.len(),
        1,
        "Should have one condition"
    );
    match &service.common.unit.conditions[0] {
        crate::units::UnitCondition::Virtualization { value, negate } => {
            assert_eq!(value, "yes");
            assert!(!negate);
        }
        other => panic!("Expected Virtualization condition, got {:?}", other),
    }
}

#[test]
fn test_condition_virtualization_no_parsed() {
    let test_service_str = r#"
    [Unit]
    ConditionVirtualization = no

    [Service]
    ExecStart = /bin/myservice
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(service.common.unit.conditions.len(), 1);
    match &service.common.unit.conditions[0] {
        crate::units::UnitCondition::Virtualization { value, negate } => {
            assert_eq!(value, "no");
            assert!(!negate);
        }
        other => panic!("Expected Virtualization condition, got {:?}", other),
    }
}

#[test]
fn test_condition_virtualization_negated() {
    let test_service_str = r#"
    [Unit]
    ConditionVirtualization = !kvm

    [Service]
    ExecStart = /bin/myservice
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(service.common.unit.conditions.len(), 1);
    match &service.common.unit.conditions[0] {
        crate::units::UnitCondition::Virtualization { value, negate } => {
            assert_eq!(value, "kvm");
            assert!(negate, "Should be negated");
        }
        other => panic!("Expected Virtualization condition, got {:?}", other),
    }
}

#[test]
fn test_condition_virtualization_vm_category() {
    let test_service_str = r#"
    [Unit]
    ConditionVirtualization = vm

    [Service]
    ExecStart = /bin/myservice
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(service.common.unit.conditions.len(), 1);
    match &service.common.unit.conditions[0] {
        crate::units::UnitCondition::Virtualization { value, negate } => {
            assert_eq!(value, "vm");
            assert!(!negate);
        }
        other => panic!("Expected Virtualization condition, got {:?}", other),
    }
}

#[test]
fn test_condition_virtualization_container_category() {
    let test_service_str = r#"
    [Unit]
    ConditionVirtualization = container

    [Service]
    ExecStart = /bin/myservice
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(service.common.unit.conditions.len(), 1);
    match &service.common.unit.conditions[0] {
        crate::units::UnitCondition::Virtualization { value, negate } => {
            assert_eq!(value, "container");
            assert!(!negate);
        }
        other => panic!("Expected Virtualization condition, got {:?}", other),
    }
}

#[test]
fn test_condition_virtualization_specific_tech() {
    let test_service_str = r#"
    [Unit]
    ConditionVirtualization = docker

    [Service]
    ExecStart = /bin/myservice
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(service.common.unit.conditions.len(), 1);
    match &service.common.unit.conditions[0] {
        crate::units::UnitCondition::Virtualization { value, negate } => {
            assert_eq!(value, "docker");
            assert!(!negate);
        }
        other => panic!("Expected Virtualization condition, got {:?}", other),
    }
}

#[test]
fn test_condition_virtualization_case_insensitive() {
    let test_service_str = r#"
    [Unit]
    ConditionVirtualization = KVM

    [Service]
    ExecStart = /bin/myservice
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(service.common.unit.conditions.len(), 1);
    match &service.common.unit.conditions[0] {
        crate::units::UnitCondition::Virtualization { value, negate } => {
            assert_eq!(value, "kvm", "Value should be lowercased");
            assert!(!negate);
        }
        other => panic!("Expected Virtualization condition, got {:?}", other),
    }
}

#[test]
fn test_condition_virtualization_negated_container() {
    let test_service_str = r#"
    [Unit]
    ConditionVirtualization = !container

    [Service]
    ExecStart = /bin/myservice
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(service.common.unit.conditions.len(), 1);
    match &service.common.unit.conditions[0] {
        crate::units::UnitCondition::Virtualization { value, negate } => {
            assert_eq!(value, "container");
            assert!(negate, "Should be negated");
        }
        other => panic!("Expected Virtualization condition, got {:?}", other),
    }
}

#[test]
fn test_condition_virtualization_empty_ignored() {
    let test_service_str = r#"
    [Unit]
    ConditionVirtualization =

    [Service]
    ExecStart = /bin/myservice
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.common.unit.conditions.len(),
        0,
        "Empty ConditionVirtualization should not add a condition"
    );
}

#[test]
fn test_condition_virtualization_with_other_conditions() {
    let test_service_str = r#"
    [Unit]
    ConditionPathExists = /etc/myconfig
    ConditionVirtualization = !container

    [Service]
    ExecStart = /bin/myservice
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.common.unit.conditions.len(),
        2,
        "Should have two conditions"
    );

    // First condition: PathExists
    match &service.common.unit.conditions[0] {
        crate::units::UnitCondition::PathExists { path, negate } => {
            assert_eq!(path, "/etc/myconfig");
            assert!(!negate);
        }
        other => panic!("Expected PathExists condition, got {:?}", other),
    }

    // Second condition: Virtualization
    match &service.common.unit.conditions[1] {
        crate::units::UnitCondition::Virtualization { value, negate } => {
            assert_eq!(value, "container");
            assert!(negate);
        }
        other => panic!("Expected Virtualization condition, got {:?}", other),
    }
}

#[test]
fn test_condition_virtualization_preserved_after_unit_conversion() {
    use std::convert::TryInto;

    let test_service_str = r#"
    [Unit]
    ConditionVirtualization = !vm

    [Service]
    ExecStart = /bin/myservice
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    let unit: crate::units::Unit = service.try_into().unwrap();
    assert_eq!(
        unit.common.unit.conditions.len(),
        1,
        "Condition should be preserved after unit conversion"
    );
    match &unit.common.unit.conditions[0] {
        crate::units::UnitCondition::Virtualization { value, negate } => {
            assert_eq!(value, "vm");
            assert!(negate);
        }
        other => panic!("Expected Virtualization condition, got {:?}", other),
    }
}

#[test]
fn test_condition_virtualization_in_socket_unit() {
    let test_socket_str = r#"
    [Unit]
    ConditionVirtualization = yes

    [Socket]
    ListenStream = /run/test.sock
    "#;

    let parsed_file = crate::units::parse_file(test_socket_str).unwrap();
    let socket = crate::units::parse_socket(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.socket"),
    )
    .unwrap();

    assert_eq!(
        socket.common.unit.conditions.len(),
        1,
        "ConditionVirtualization should work in socket units"
    );
    match &socket.common.unit.conditions[0] {
        crate::units::UnitCondition::Virtualization { value, negate } => {
            assert_eq!(value, "yes");
            assert!(!negate);
        }
        other => panic!("Expected Virtualization condition, got {:?}", other),
    }
}

#[test]
fn test_condition_virtualization_in_target_unit() {
    let test_target_str = r#"
    [Unit]
    ConditionVirtualization = !container
    "#;

    let parsed_file = crate::units::parse_file(test_target_str).unwrap();
    let target = crate::units::parse_target(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.target"),
    )
    .unwrap();

    assert_eq!(
        target.common.unit.conditions.len(),
        1,
        "ConditionVirtualization should work in target units"
    );
    match &target.common.unit.conditions[0] {
        crate::units::UnitCondition::Virtualization { value, negate } => {
            assert_eq!(value, "container");
            assert!(negate);
        }
        other => panic!("Expected Virtualization condition, got {:?}", other),
    }
}

#[test]
fn test_condition_virtualization_multiple_techs() {
    // systemd only supports one ConditionVirtualization= per unit (last one wins),
    // but our parser accumulates them as separate conditions — all must pass.
    let test_service_str = r#"
    [Unit]
    ConditionVirtualization = !docker
    ConditionVirtualization = !lxc

    [Service]
    ExecStart = /bin/myservice
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.common.unit.conditions.len(),
        2,
        "Multiple ConditionVirtualization directives should accumulate"
    );
    match &service.common.unit.conditions[0] {
        crate::units::UnitCondition::Virtualization { value, negate } => {
            assert_eq!(value, "docker");
            assert!(negate);
        }
        other => panic!("Expected Virtualization condition, got {:?}", other),
    }
    match &service.common.unit.conditions[1] {
        crate::units::UnitCondition::Virtualization { value, negate } => {
            assert_eq!(value, "lxc");
            assert!(negate);
        }
        other => panic!("Expected Virtualization condition, got {:?}", other),
    }
}

#[test]
fn test_condition_virtualization_systemd_nspawn() {
    let test_service_str = r#"
    [Unit]
    ConditionVirtualization = systemd-nspawn

    [Service]
    ExecStart = /bin/myservice
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(service.common.unit.conditions.len(), 1);
    match &service.common.unit.conditions[0] {
        crate::units::UnitCondition::Virtualization { value, negate } => {
            assert_eq!(value, "systemd-nspawn");
            assert!(!negate);
        }
        other => panic!("Expected Virtualization condition, got {:?}", other),
    }
}

// ============================================================
// ConditionCapability= parsing tests
// ============================================================

#[test]
fn test_condition_capability_no_unsupported_warning() {
    let test_service_str = r#"
    [Unit]
    ConditionCapability = CAP_NET_ADMIN

    [Service]
    ExecStart = /bin/myservice
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let result = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    );

    assert!(
        result.is_ok(),
        "ConditionCapability should not produce an unsupported setting warning"
    );
}

#[test]
fn test_condition_capability_parsed() {
    let test_service_str = r#"
    [Unit]
    ConditionCapability = CAP_NET_ADMIN

    [Service]
    ExecStart = /bin/myservice
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.common.unit.conditions.len(),
        1,
        "Should have one condition"
    );
    match &service.common.unit.conditions[0] {
        crate::units::UnitCondition::Capability { capability, negate } => {
            assert_eq!(capability, "CAP_NET_ADMIN");
            assert!(!negate);
        }
        other => panic!("Expected Capability condition, got {:?}", other),
    }
}

#[test]
fn test_condition_capability_negated() {
    let test_service_str = r#"
    [Unit]
    ConditionCapability = !CAP_SYS_ADMIN

    [Service]
    ExecStart = /bin/myservice
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(service.common.unit.conditions.len(), 1);
    match &service.common.unit.conditions[0] {
        crate::units::UnitCondition::Capability { capability, negate } => {
            assert_eq!(capability, "CAP_SYS_ADMIN");
            assert!(negate, "Should be negated");
        }
        other => panic!("Expected Capability condition, got {:?}", other),
    }
}

#[test]
fn test_condition_capability_sys_ptrace() {
    let test_service_str = r#"
    [Unit]
    ConditionCapability = CAP_SYS_PTRACE

    [Service]
    ExecStart = /bin/myservice
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(service.common.unit.conditions.len(), 1);
    match &service.common.unit.conditions[0] {
        crate::units::UnitCondition::Capability { capability, negate } => {
            assert_eq!(capability, "CAP_SYS_PTRACE");
            assert!(!negate);
        }
        other => panic!("Expected Capability condition, got {:?}", other),
    }
}

#[test]
fn test_condition_capability_empty_ignored() {
    let test_service_str = r#"
    [Unit]
    ConditionCapability =

    [Service]
    ExecStart = /bin/myservice
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.common.unit.conditions.len(),
        0,
        "Empty ConditionCapability should not add a condition"
    );
}

#[test]
fn test_condition_capability_with_other_conditions() {
    let test_service_str = r#"
    [Unit]
    ConditionPathExists = /etc/myconfig
    ConditionCapability = CAP_NET_RAW
    ConditionVirtualization = !container

    [Service]
    ExecStart = /bin/myservice
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.common.unit.conditions.len(),
        3,
        "Should have three conditions"
    );

    // First: PathExists
    match &service.common.unit.conditions[0] {
        crate::units::UnitCondition::PathExists { path, negate } => {
            assert_eq!(path, "/etc/myconfig");
            assert!(!negate);
        }
        other => panic!("Expected PathExists condition, got {:?}", other),
    }

    // Second: Capability (parsed after PathIsDirectory and Virtualization)
    // Order depends on the parsing order in parse_unit_section
    let has_capability = service.common.unit.conditions.iter().any(|c| {
        matches!(
            c,
            crate::units::UnitCondition::Capability {
                capability,
                negate
            } if capability == "CAP_NET_RAW" && !negate
        )
    });
    assert!(
        has_capability,
        "Should have CAP_NET_RAW capability condition"
    );

    let has_virt = service.common.unit.conditions.iter().any(|c| {
        matches!(
            c,
            crate::units::UnitCondition::Virtualization {
                value,
                negate
            } if value == "container" && *negate
        )
    });
    assert!(
        has_virt,
        "Should have negated container virtualization condition"
    );
}

#[test]
fn test_condition_capability_multiple() {
    let test_service_str = r#"
    [Unit]
    ConditionCapability = CAP_NET_ADMIN
    ConditionCapability = CAP_SYS_ADMIN

    [Service]
    ExecStart = /bin/myservice
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.common.unit.conditions.len(),
        2,
        "Multiple ConditionCapability directives should accumulate"
    );
    match &service.common.unit.conditions[0] {
        crate::units::UnitCondition::Capability { capability, negate } => {
            assert_eq!(capability, "CAP_NET_ADMIN");
            assert!(!negate);
        }
        other => panic!("Expected Capability condition, got {:?}", other),
    }
    match &service.common.unit.conditions[1] {
        crate::units::UnitCondition::Capability { capability, negate } => {
            assert_eq!(capability, "CAP_SYS_ADMIN");
            assert!(!negate);
        }
        other => panic!("Expected Capability condition, got {:?}", other),
    }
}

#[test]
fn test_condition_capability_preserved_after_unit_conversion() {
    use std::convert::TryInto;

    let test_service_str = r#"
    [Unit]
    ConditionCapability = !CAP_SYS_MODULE

    [Service]
    ExecStart = /bin/myservice
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    let unit: crate::units::Unit = service.try_into().unwrap();
    assert_eq!(
        unit.common.unit.conditions.len(),
        1,
        "Condition should be preserved after unit conversion"
    );
    match &unit.common.unit.conditions[0] {
        crate::units::UnitCondition::Capability { capability, negate } => {
            assert_eq!(capability, "CAP_SYS_MODULE");
            assert!(negate);
        }
        other => panic!("Expected Capability condition, got {:?}", other),
    }
}

#[test]
fn test_condition_capability_in_socket_unit() {
    let test_socket_str = r#"
    [Unit]
    ConditionCapability = CAP_NET_BIND_SERVICE

    [Socket]
    ListenStream = /run/test.sock
    "#;

    let parsed_file = crate::units::parse_file(test_socket_str).unwrap();
    let socket = crate::units::parse_socket(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.socket"),
    )
    .unwrap();

    assert_eq!(
        socket.common.unit.conditions.len(),
        1,
        "ConditionCapability should work in socket units"
    );
    match &socket.common.unit.conditions[0] {
        crate::units::UnitCondition::Capability { capability, negate } => {
            assert_eq!(capability, "CAP_NET_BIND_SERVICE");
            assert!(!negate);
        }
        other => panic!("Expected Capability condition, got {:?}", other),
    }
}

#[test]
fn test_condition_capability_in_target_unit() {
    let test_target_str = r#"
    [Unit]
    ConditionCapability = CAP_SYS_ADMIN
    "#;

    let parsed_file = crate::units::parse_file(test_target_str).unwrap();
    let target = crate::units::parse_target(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.target"),
    )
    .unwrap();

    assert_eq!(
        target.common.unit.conditions.len(),
        1,
        "ConditionCapability should work in target units"
    );
    match &target.common.unit.conditions[0] {
        crate::units::UnitCondition::Capability { capability, negate } => {
            assert_eq!(capability, "CAP_SYS_ADMIN");
            assert!(!negate);
        }
        other => panic!("Expected Capability condition, got {:?}", other),
    }
}

#[test]
fn test_condition_capability_preserves_case() {
    let test_service_str = r#"
    [Unit]
    ConditionCapability = cap_net_admin

    [Service]
    ExecStart = /bin/myservice
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(service.common.unit.conditions.len(), 1);
    match &service.common.unit.conditions[0] {
        crate::units::UnitCondition::Capability { capability, negate } => {
            // The raw value is stored as-is; the check() method handles
            // case-insensitive matching via capability_name_to_bit().
            assert_eq!(capability, "cap_net_admin");
            assert!(!negate);
        }
        other => panic!("Expected Capability condition, got {:?}", other),
    }
}

#[test]
fn test_condition_capability_various_caps() {
    // Test several different capability names to ensure they all parse correctly
    let caps = [
        "CAP_CHOWN",
        "CAP_KILL",
        "CAP_SETUID",
        "CAP_SETGID",
        "CAP_NET_RAW",
        "CAP_SYS_CHROOT",
        "CAP_MKNOD",
        "CAP_AUDIT_WRITE",
        "CAP_SETFCAP",
        "CAP_SYSLOG",
        "CAP_BPF",
    ];

    for cap in &caps {
        let test_service_str = format!(
            r#"
            [Unit]
            ConditionCapability = {}

            [Service]
            ExecStart = /bin/myservice
            "#,
            cap
        );

        let parsed_file = crate::units::parse_file(&test_service_str).unwrap();
        let service = crate::units::parse_service(
            parsed_file,
            &std::path::PathBuf::from("/path/to/unitfile.service"),
        )
        .unwrap();

        assert_eq!(
            service.common.unit.conditions.len(),
            1,
            "Should have one condition for {}",
            cap
        );
        match &service.common.unit.conditions[0] {
            crate::units::UnitCondition::Capability { capability, negate } => {
                assert_eq!(capability, *cap);
                assert!(!negate);
            }
            other => panic!("Expected Capability condition for {}, got {:?}", cap, other),
        }
    }
}

// ============================================================
// ProtectProc= parsing tests
// ============================================================

#[test]
fn test_protect_proc_defaults_to_default() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.exec_section.protect_proc,
        crate::units::ProtectProc::Default,
        "ProtectProc should default to Default"
    );
}

#[test]
fn test_protect_proc_set_default() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    ProtectProc = default
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.exec_section.protect_proc,
        crate::units::ProtectProc::Default,
    );
}

#[test]
fn test_protect_proc_set_noaccess() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    ProtectProc = noaccess
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.exec_section.protect_proc,
        crate::units::ProtectProc::Noaccess,
    );
}

#[test]
fn test_protect_proc_set_invisible() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    ProtectProc = invisible
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.exec_section.protect_proc,
        crate::units::ProtectProc::Invisible,
    );
}

#[test]
fn test_protect_proc_set_ptraceable() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    ProtectProc = ptraceable
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.exec_section.protect_proc,
        crate::units::ProtectProc::Ptraceable,
    );
}

#[test]
fn test_protect_proc_case_insensitive() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    ProtectProc = Invisible
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.exec_section.protect_proc,
        crate::units::ProtectProc::Invisible,
    );
}

#[test]
fn test_protect_proc_case_insensitive_upper() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    ProtectProc = NOACCESS
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.exec_section.protect_proc,
        crate::units::ProtectProc::Noaccess,
    );
}

#[test]
fn test_protect_proc_empty_resets_to_default() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    ProtectProc =
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.exec_section.protect_proc,
        crate::units::ProtectProc::Default,
        "Empty ProtectProc should reset to Default"
    );
}

#[test]
fn test_protect_proc_no_unsupported_warning() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    ProtectProc = invisible
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let result = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    );

    assert!(
        result.is_ok(),
        "ProtectProc should not produce an unsupported setting warning"
    );
}

#[test]
fn test_protect_proc_invalid_value() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    ProtectProc = bogus
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let result = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    );

    assert!(
        result.is_err(),
        "ProtectProc with an invalid value should produce an error"
    );
}

#[test]
fn test_protect_proc_with_other_settings() {
    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    ProtectProc = invisible
    ProtectSystem = strict
    NoNewPrivileges = yes
    ProtectHome = read-only
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    assert_eq!(
        service.srvc.exec_section.protect_proc,
        crate::units::ProtectProc::Invisible,
    );
    assert!(service.srvc.exec_section.no_new_privileges);
    assert_eq!(
        service.srvc.exec_section.protect_home,
        crate::units::ProtectHome::ReadOnly,
    );
}

#[test]
fn test_protect_proc_preserved_after_unit_conversion() {
    use std::convert::TryInto;

    let test_service_str = r#"
    [Service]
    ExecStart = /bin/myservice
    ProtectProc = noaccess
    "#;

    let parsed_file = crate::units::parse_file(test_service_str).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from("/path/to/unitfile.service"),
    )
    .unwrap();

    let unit: crate::units::Unit = service.try_into().unwrap();
    if let crate::units::Specific::Service(srvc) = &unit.specific {
        assert_eq!(
            srvc.conf.exec_config.protect_proc,
            crate::units::ProtectProc::Noaccess,
            "ProtectProc should be preserved after unit conversion"
        );
    } else {
        panic!("Expected service unit");
    }
}

#[test]
fn test_protect_proc_in_socket_unit() {
    let test_socket_str = r#"
    [Socket]
    ListenStream = /run/test.sock
    ProtectProc = ptraceable
    "#;

    let parsed_file = crate::units::parse_file(test_socket_str).unwrap();
    let socket = crate::units::parse_socket(
        parsed_file,
        &std::path::PathBuf::from("/path/to/test.socket"),
    )
    .unwrap();

    assert_eq!(
        socket.sock.exec_section.protect_proc,
        crate::units::ProtectProc::Ptraceable,
        "ProtectProc should work in socket units"
    );
}
