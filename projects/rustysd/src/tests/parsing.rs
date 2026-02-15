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
