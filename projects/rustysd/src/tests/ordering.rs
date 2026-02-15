use crate::units::Unit;
use std::convert::TryInto;

#[test]
fn test_unit_ordering() {
    let target1_str = format!(
        "
    [Unit]
    Description = {}
    Before = {}

    [Install]
    RequiredBy = {}
    ",
        "Target", "2.target", "2.target",
    );

    let parsed_file = crate::units::parse_file(&target1_str).unwrap();
    let target1_unit =
        crate::units::parse_target(parsed_file, &std::path::PathBuf::from("/path/to/1.target"))
            .unwrap();

    let target2_str = format!(
        "
    [Unit]
    Description = {}
    After = {}

    [Install]
    RequiredBy = {}
    ",
        "Target", "1.target", "3.target",
    );

    let parsed_file = crate::units::parse_file(&target2_str).unwrap();
    let target2_unit =
        crate::units::parse_target(parsed_file, &std::path::PathBuf::from("/path/to/2.target"))
            .unwrap();

    let target3_str = format!(
        "
    [Unit]
    Description = {}
    After = {}
    After = {}

    ",
        "Target", "1.target", "2.target"
    );

    let parsed_file = crate::units::parse_file(&target3_str).unwrap();
    let target3_unit =
        crate::units::parse_target(parsed_file, &std::path::PathBuf::from("/path/to/3.target"))
            .unwrap();

    let mut unit_table = std::collections::HashMap::new();

    let target1_unit: Unit = target1_unit.try_into().unwrap();
    let target2_unit: Unit = target2_unit.try_into().unwrap();
    let target3_unit: Unit = target3_unit.try_into().unwrap();
    let id1 = target1_unit.id.clone();
    let id2 = target2_unit.id.clone();
    let id3 = target3_unit.id.clone();

    unit_table.insert(target1_unit.id.clone(), target1_unit);
    unit_table.insert(target2_unit.id.clone(), target2_unit);
    unit_table.insert(target3_unit.id.clone(), target3_unit);

    crate::units::fill_dependencies(&mut unit_table).unwrap();
    unit_table
        .values_mut()
        .for_each(|unit| unit.dedup_dependencies());
    crate::units::sanity_check_dependencies(&unit_table).unwrap();

    unit_table
        .values()
        .for_each(|unit| println!("{} {:?}", unit.id, unit.common.dependencies));

    // before/after 1.target
    assert!(unit_table
        .get(&id1)
        .unwrap()
        .common
        .dependencies
        .after
        .is_empty());
    assert!(
        unit_table
            .get(&id1)
            .unwrap()
            .common
            .dependencies
            .before
            .len()
            == 2
    );
    assert!(unit_table
        .get(&id1)
        .unwrap()
        .common
        .dependencies
        .before
        .contains(&id2));
    assert!(unit_table
        .get(&id1)
        .unwrap()
        .common
        .dependencies
        .before
        .contains(&id3));

    // before/after 2.target
    assert_eq!(
        unit_table
            .get(&id2)
            .unwrap()
            .common
            .dependencies
            .before
            .len(),
        1
    );
    assert!(unit_table
        .get(&id2)
        .unwrap()
        .common
        .dependencies
        .before
        .contains(&id3));
    assert_eq!(
        unit_table
            .get(&id2)
            .unwrap()
            .common
            .dependencies
            .after
            .len(),
        1
    );
    assert!(unit_table
        .get(&id2)
        .unwrap()
        .common
        .dependencies
        .after
        .contains(&id1));

    // before/after 3.target
    assert!(unit_table
        .get(&id3)
        .unwrap()
        .common
        .dependencies
        .before
        .is_empty());
    assert!(
        unit_table
            .get(&id3)
            .unwrap()
            .common
            .dependencies
            .after
            .len()
            == 2
    );
    assert!(unit_table
        .get(&id3)
        .unwrap()
        .common
        .dependencies
        .after
        .contains(&id2));
    assert!(unit_table
        .get(&id3)
        .unwrap()
        .common
        .dependencies
        .after
        .contains(&id1));

    // Test the collection of start subgraphs
    // add a new unrelated unit, that should never occur in any of these operations for {1,2,3}.target
    let target4_str = format!(
        "
    [Unit]
    Description = {}

    ",
        "Target"
    );
    let parsed_file = crate::units::parse_file(&target4_str).unwrap();
    let target4_unit =
        crate::units::parse_target(parsed_file, &std::path::PathBuf::from("/path/to/4.target"))
            .unwrap();
    let target4_unit: Unit = target4_unit.try_into().unwrap();
    let id4 = target4_unit.id.clone();
    unit_table.insert(target4_unit.id.clone(), target4_unit);

    // 3.target needs all units
    let mut ids_to_start = vec![id3.clone()];
    crate::units::collect_unit_start_subgraph(&mut ids_to_start, &unit_table);
    ids_to_start.sort();
    assert_eq!(ids_to_start, vec![id1.clone(), id2.clone(), id3.clone()]);

    // 2.target needs 1 and 2
    let mut ids_to_start = vec![id2.clone()];
    crate::units::collect_unit_start_subgraph(&mut ids_to_start, &unit_table);
    ids_to_start.sort();
    assert_eq!(ids_to_start, vec![id1.clone(), id2.clone()]);

    // 1.target needs only 1
    let mut ids_to_start = vec![id1.clone()];
    crate::units::collect_unit_start_subgraph(&mut ids_to_start, &unit_table);
    ids_to_start.sort();
    assert_eq!(ids_to_start, vec![id1.clone()]);

    // 4.target needs only 4
    let mut ids_to_start = vec![id4.clone()];
    crate::units::collect_unit_start_subgraph(&mut ids_to_start, &unit_table);
    ids_to_start.sort();
    assert_eq!(ids_to_start, vec![id4.clone()]);
}

#[test]
fn test_circle() {
    let target1_str = format!(
        "
    [Unit]
    Description = {}
    After = {}
    ",
        "Target", "3.target"
    );

    let parsed_file = crate::units::parse_file(&target1_str).unwrap();
    let target1_unit =
        crate::units::parse_target(parsed_file, &std::path::PathBuf::from("/path/to/1.target"))
            .unwrap();

    let target2_str = format!(
        "
    [Unit]
    Description = {}
    After = {}
    ",
        "Target", "1.target"
    );

    let parsed_file = crate::units::parse_file(&target2_str).unwrap();
    let target2_unit =
        crate::units::parse_target(parsed_file, &std::path::PathBuf::from("/path/to/2.target"))
            .unwrap();

    let target3_str = format!(
        "
    [Unit]
    Description = {}
    After = {}
    ",
        "Target", "2.target"
    );

    let parsed_file = crate::units::parse_file(&target3_str).unwrap();
    let target3_unit =
        crate::units::parse_target(parsed_file, &std::path::PathBuf::from("/path/to/3.target"))
            .unwrap();

    let mut unit_table = std::collections::HashMap::new();
    let target1_unit: Unit = target1_unit.try_into().unwrap();
    let target2_unit: Unit = target2_unit.try_into().unwrap();
    let target3_unit: Unit = target3_unit.try_into().unwrap();
    let target1_id = target1_unit.id.clone();
    let target2_id = target2_unit.id.clone();
    let target3_id = target3_unit.id.clone();
    unit_table.insert(target1_unit.id.clone(), target1_unit);
    unit_table.insert(target2_unit.id.clone(), target2_unit);
    unit_table.insert(target3_unit.id.clone(), target3_unit);

    crate::units::fill_dependencies(&mut unit_table).unwrap();
    unit_table
        .values_mut()
        .for_each(|unit| unit.dedup_dependencies());

    if let Err(crate::units::SanityCheckError::CirclesFound(circles)) =
        crate::units::sanity_check_dependencies(&unit_table)
    {
        if circles.len() == 1 {
            let circle = &circles[0];
            assert_eq!(circle.len(), 3);
            assert!(circle.contains(&target1_id));
            assert!(circle.contains(&target2_id));
            assert!(circle.contains(&target3_id));
        } else {
            panic!("more than one circle found but there is only one");
        }
    } else {
        panic!("No circle found but there is one");
    }
}

/// Helper to create a minimal target unit string
fn target_unit_str(description: &str) -> String {
    format!(
        r#"
    [Unit]
    Description = {}
    "#,
        description
    )
}

/// Helper to create a target unit string with DefaultDependencies setting
fn target_unit_str_with_default_deps(description: &str, default_deps: &str) -> String {
    format!(
        r#"
    [Unit]
    Description = {}
    DefaultDependencies = {}
    "#,
        description, default_deps
    )
}

/// Helper to parse and convert a target unit
fn make_target(name: &str, content: &str) -> Unit {
    let parsed_file = crate::units::parse_file(content).unwrap();
    let target = crate::units::parse_target(
        parsed_file,
        &std::path::PathBuf::from(format!("/path/to/{}", name)),
    )
    .unwrap();
    target.try_into().unwrap()
}

/// Helper to parse and convert a service unit
fn make_service(name: &str, content: &str) -> Unit {
    let parsed_file = crate::units::parse_file(content).unwrap();
    let service = crate::units::parse_service(
        parsed_file,
        &std::path::PathBuf::from(format!("/path/to/{}", name)),
    )
    .unwrap();
    service.try_into().unwrap()
}

/// Helper to parse and convert a socket unit
fn make_socket(name: &str, content: &str) -> Unit {
    let parsed_file = crate::units::parse_file(content).unwrap();
    let socket = crate::units::parse_socket(
        parsed_file,
        &std::path::PathBuf::from(format!("/path/to/{}", name)),
    )
    .unwrap();
    socket.try_into().unwrap()
}

#[test]
fn test_default_deps_service_gets_shutdown_conflict_and_before() {
    let shutdown = make_target("shutdown.target", &target_unit_str("Shutdown"));
    let service = make_service(
        "myapp.service",
        r#"
        [Service]
        ExecStart = /bin/true
        "#,
    );

    let shutdown_id = shutdown.id.clone();
    let service_id = service.id.clone();

    let mut unit_table = std::collections::HashMap::new();
    unit_table.insert(shutdown.id.clone(), shutdown);
    unit_table.insert(service.id.clone(), service);

    crate::units::fill_dependencies(&mut unit_table).unwrap();
    unit_table
        .values_mut()
        .for_each(|unit| unit.dedup_dependencies());

    let service = unit_table.get(&service_id).unwrap();
    assert!(
        service.common.dependencies.conflicts.contains(&shutdown_id),
        "Service should have Conflicts=shutdown.target"
    );
    assert!(
        service.common.dependencies.before.contains(&shutdown_id),
        "Service should have Before=shutdown.target"
    );
}

#[test]
fn test_default_deps_service_gets_sysinit_requires_and_after() {
    let sysinit = make_target("sysinit.target", &target_unit_str("System Initialization"));
    let service = make_service(
        "myapp.service",
        r#"
        [Service]
        ExecStart = /bin/true
        "#,
    );

    let sysinit_id = sysinit.id.clone();
    let service_id = service.id.clone();

    let mut unit_table = std::collections::HashMap::new();
    unit_table.insert(sysinit.id.clone(), sysinit);
    unit_table.insert(service.id.clone(), service);

    crate::units::fill_dependencies(&mut unit_table).unwrap();
    unit_table
        .values_mut()
        .for_each(|unit| unit.dedup_dependencies());

    let service = unit_table.get(&service_id).unwrap();
    assert!(
        service.common.dependencies.requires.contains(&sysinit_id),
        "Service should have Requires=sysinit.target"
    );
    assert!(
        service.common.dependencies.after.contains(&sysinit_id),
        "Service should have After=sysinit.target"
    );
}

#[test]
fn test_default_deps_service_gets_after_basic_target() {
    let basic = make_target("basic.target", &target_unit_str("Basic System"));
    let service = make_service(
        "myapp.service",
        r#"
        [Service]
        ExecStart = /bin/true
        "#,
    );

    let basic_id = basic.id.clone();
    let service_id = service.id.clone();

    let mut unit_table = std::collections::HashMap::new();
    unit_table.insert(basic.id.clone(), basic);
    unit_table.insert(service.id.clone(), service);

    crate::units::fill_dependencies(&mut unit_table).unwrap();
    unit_table
        .values_mut()
        .for_each(|unit| unit.dedup_dependencies());

    let service = unit_table.get(&service_id).unwrap();
    assert!(
        service.common.dependencies.after.contains(&basic_id),
        "Service should have After=basic.target"
    );
}

#[test]
fn test_default_deps_disabled_no_implicit_deps() {
    let shutdown = make_target("shutdown.target", &target_unit_str("Shutdown"));
    let sysinit = make_target("sysinit.target", &target_unit_str("System Initialization"));
    let basic = make_target("basic.target", &target_unit_str("Basic System"));
    let service = make_service(
        "myapp.service",
        r#"
        [Unit]
        DefaultDependencies = no
        [Service]
        ExecStart = /bin/true
        "#,
    );

    let shutdown_id = shutdown.id.clone();
    let sysinit_id = sysinit.id.clone();
    let basic_id = basic.id.clone();
    let service_id = service.id.clone();

    let mut unit_table = std::collections::HashMap::new();
    unit_table.insert(shutdown.id.clone(), shutdown);
    unit_table.insert(sysinit.id.clone(), sysinit);
    unit_table.insert(basic.id.clone(), basic);
    unit_table.insert(service.id.clone(), service);

    crate::units::fill_dependencies(&mut unit_table).unwrap();
    unit_table
        .values_mut()
        .for_each(|unit| unit.dedup_dependencies());

    let service = unit_table.get(&service_id).unwrap();
    assert!(
        !service.common.dependencies.conflicts.contains(&shutdown_id),
        "Service with DefaultDependencies=no should NOT have Conflicts=shutdown.target"
    );
    assert!(
        !service.common.dependencies.before.contains(&shutdown_id),
        "Service with DefaultDependencies=no should NOT have Before=shutdown.target"
    );
    assert!(
        !service.common.dependencies.requires.contains(&sysinit_id),
        "Service with DefaultDependencies=no should NOT have Requires=sysinit.target"
    );
    assert!(
        !service.common.dependencies.after.contains(&sysinit_id),
        "Service with DefaultDependencies=no should NOT have After=sysinit.target"
    );
    assert!(
        !service.common.dependencies.after.contains(&basic_id),
        "Service with DefaultDependencies=no should NOT have After=basic.target"
    );
}

#[test]
fn test_default_deps_target_only_gets_shutdown_not_sysinit() {
    let shutdown = make_target("shutdown.target", &target_unit_str("Shutdown"));
    let sysinit = make_target("sysinit.target", &target_unit_str("System Initialization"));
    let basic = make_target("basic.target", &target_unit_str("Basic System"));
    let custom_target = make_target("custom.target", &target_unit_str("Custom Target"));

    let shutdown_id = shutdown.id.clone();
    let sysinit_id = sysinit.id.clone();
    let basic_id = basic.id.clone();
    let custom_id = custom_target.id.clone();

    let mut unit_table = std::collections::HashMap::new();
    unit_table.insert(shutdown.id.clone(), shutdown);
    unit_table.insert(sysinit.id.clone(), sysinit);
    unit_table.insert(basic.id.clone(), basic);
    unit_table.insert(custom_target.id.clone(), custom_target);

    crate::units::fill_dependencies(&mut unit_table).unwrap();
    unit_table
        .values_mut()
        .for_each(|unit| unit.dedup_dependencies());

    let custom = unit_table.get(&custom_id).unwrap();
    // Targets get Conflicts and Before on shutdown.target
    assert!(
        custom.common.dependencies.conflicts.contains(&shutdown_id),
        "Target should have Conflicts=shutdown.target"
    );
    assert!(
        custom.common.dependencies.before.contains(&shutdown_id),
        "Target should have Before=shutdown.target"
    );
    // Targets should NOT get Requires/After on sysinit.target or After on basic.target
    assert!(
        !custom.common.dependencies.requires.contains(&sysinit_id),
        "Target should NOT have Requires=sysinit.target"
    );
    assert!(
        !custom.common.dependencies.after.contains(&sysinit_id),
        "Target should NOT have After=sysinit.target"
    );
    assert!(
        !custom.common.dependencies.after.contains(&basic_id),
        "Target should NOT have After=basic.target"
    );
}

#[test]
fn test_default_deps_shutdown_target_excluded() {
    let shutdown = make_target("shutdown.target", &target_unit_str("Shutdown"));
    let sysinit = make_target("sysinit.target", &target_unit_str("System Initialization"));

    let shutdown_id = shutdown.id.clone();
    let sysinit_id = sysinit.id.clone();

    let mut unit_table = std::collections::HashMap::new();
    unit_table.insert(shutdown.id.clone(), shutdown);
    unit_table.insert(sysinit.id.clone(), sysinit);

    crate::units::fill_dependencies(&mut unit_table).unwrap();
    unit_table
        .values_mut()
        .for_each(|unit| unit.dedup_dependencies());

    let shutdown = unit_table.get(&shutdown_id).unwrap();
    // shutdown.target should NOT have Conflicts/Before on itself
    assert!(
        !shutdown
            .common
            .dependencies
            .conflicts
            .contains(&shutdown_id),
        "shutdown.target should NOT conflict with itself"
    );
    assert!(
        !shutdown.common.dependencies.before.contains(&shutdown_id),
        "shutdown.target should NOT be Before itself"
    );
    // shutdown.target should NOT get Requires on sysinit (it's a target, not service)
    // but also it's excluded from default deps entirely
    assert!(
        !shutdown.common.dependencies.requires.contains(&sysinit_id),
        "shutdown.target should NOT have Requires=sysinit.target"
    );
}

#[test]
fn test_default_deps_reverse_relations_on_shutdown() {
    let shutdown = make_target("shutdown.target", &target_unit_str("Shutdown"));
    let service = make_service(
        "myapp.service",
        r#"
        [Service]
        ExecStart = /bin/true
        "#,
    );

    let shutdown_id = shutdown.id.clone();
    let service_id = service.id.clone();

    let mut unit_table = std::collections::HashMap::new();
    unit_table.insert(shutdown.id.clone(), shutdown);
    unit_table.insert(service.id.clone(), service);

    crate::units::fill_dependencies(&mut unit_table).unwrap();
    unit_table
        .values_mut()
        .for_each(|unit| unit.dedup_dependencies());

    let shutdown = unit_table.get(&shutdown_id).unwrap();
    assert!(
        shutdown
            .common
            .dependencies
            .conflicted_by
            .contains(&service_id),
        "shutdown.target should have ConflictedBy=myapp.service (reverse of service's Conflicts)"
    );
    assert!(
        shutdown.common.dependencies.after.contains(&service_id),
        "shutdown.target should have After=myapp.service (reverse of service's Before)"
    );
}

#[test]
fn test_default_deps_reverse_relations_on_sysinit() {
    let sysinit = make_target("sysinit.target", &target_unit_str("System Initialization"));
    let service = make_service(
        "myapp.service",
        r#"
        [Service]
        ExecStart = /bin/true
        "#,
    );

    let sysinit_id = sysinit.id.clone();
    let service_id = service.id.clone();

    let mut unit_table = std::collections::HashMap::new();
    unit_table.insert(sysinit.id.clone(), sysinit);
    unit_table.insert(service.id.clone(), service);

    crate::units::fill_dependencies(&mut unit_table).unwrap();
    unit_table
        .values_mut()
        .for_each(|unit| unit.dedup_dependencies());

    let sysinit = unit_table.get(&sysinit_id).unwrap();
    assert!(
        sysinit
            .common
            .dependencies
            .required_by
            .contains(&service_id),
        "sysinit.target should have RequiredBy=myapp.service (reverse of service's Requires)"
    );
    assert!(
        sysinit.common.dependencies.before.contains(&service_id),
        "sysinit.target should have Before=myapp.service (reverse of service's After)"
    );
}

#[test]
fn test_default_deps_reverse_relations_on_basic() {
    let basic = make_target("basic.target", &target_unit_str("Basic System"));
    let service = make_service(
        "myapp.service",
        r#"
        [Service]
        ExecStart = /bin/true
        "#,
    );

    let basic_id = basic.id.clone();
    let service_id = service.id.clone();

    let mut unit_table = std::collections::HashMap::new();
    unit_table.insert(basic.id.clone(), basic);
    unit_table.insert(service.id.clone(), service);

    crate::units::fill_dependencies(&mut unit_table).unwrap();
    unit_table
        .values_mut()
        .for_each(|unit| unit.dedup_dependencies());

    let basic = unit_table.get(&basic_id).unwrap();
    assert!(
        basic.common.dependencies.before.contains(&service_id),
        "basic.target should have Before=myapp.service (reverse of service's After)"
    );
}

#[test]
fn test_default_deps_socket_gets_shutdown_and_sysinit_but_not_basic() {
    let shutdown = make_target("shutdown.target", &target_unit_str("Shutdown"));
    let sysinit = make_target("sysinit.target", &target_unit_str("System Initialization"));
    let basic = make_target("basic.target", &target_unit_str("Basic System"));
    let socket = make_socket(
        "myapp.socket",
        r#"
        [Socket]
        ListenStream = /run/myapp.sock
        "#,
    );

    let shutdown_id = shutdown.id.clone();
    let sysinit_id = sysinit.id.clone();
    let basic_id = basic.id.clone();
    let socket_id = socket.id.clone();

    let mut unit_table = std::collections::HashMap::new();
    unit_table.insert(shutdown.id.clone(), shutdown);
    unit_table.insert(sysinit.id.clone(), sysinit);
    unit_table.insert(basic.id.clone(), basic);
    unit_table.insert(socket.id.clone(), socket);

    crate::units::fill_dependencies(&mut unit_table).unwrap();
    unit_table
        .values_mut()
        .for_each(|unit| unit.dedup_dependencies());

    let socket = unit_table.get(&socket_id).unwrap();
    // Sockets get shutdown conflict/before
    assert!(
        socket.common.dependencies.conflicts.contains(&shutdown_id),
        "Socket should have Conflicts=shutdown.target"
    );
    assert!(
        socket.common.dependencies.before.contains(&shutdown_id),
        "Socket should have Before=shutdown.target"
    );
    // Sockets get sysinit requires/after
    assert!(
        socket.common.dependencies.requires.contains(&sysinit_id),
        "Socket should have Requires=sysinit.target"
    );
    assert!(
        socket.common.dependencies.after.contains(&sysinit_id),
        "Socket should have After=sysinit.target"
    );
    // Sockets should NOT get basic.target (only services do)
    assert!(
        !socket.common.dependencies.after.contains(&basic_id),
        "Socket should NOT have After=basic.target"
    );
}

#[test]
fn test_default_deps_no_targets_present_is_noop() {
    // When none of shutdown/sysinit/basic targets exist, no implicit deps are added
    let service = make_service(
        "myapp.service",
        r#"
        [Service]
        ExecStart = /bin/true
        "#,
    );

    let service_id = service.id.clone();

    let mut unit_table = std::collections::HashMap::new();
    unit_table.insert(service.id.clone(), service);

    crate::units::fill_dependencies(&mut unit_table).unwrap();
    unit_table
        .values_mut()
        .for_each(|unit| unit.dedup_dependencies());

    let service = unit_table.get(&service_id).unwrap();
    assert!(
        service.common.dependencies.conflicts.is_empty(),
        "Service should have no conflicts when no targets are present"
    );
    assert!(
        service.common.dependencies.requires.is_empty(),
        "Service should have no requires when no targets are present"
    );
    assert!(
        service.common.dependencies.before.is_empty(),
        "Service should have no before when no targets are present"
    );
    assert!(
        service.common.dependencies.after.is_empty(),
        "Service should have no after when no targets are present"
    );
}

#[test]
fn test_default_deps_mixed_enabled_and_disabled() {
    // One service with default deps, one without; only the one with should get implicit deps
    let shutdown = make_target("shutdown.target", &target_unit_str("Shutdown"));
    let service_with = make_service(
        "with.service",
        r#"
        [Service]
        ExecStart = /bin/true
        "#,
    );
    let service_without = make_service(
        "without.service",
        r#"
        [Unit]
        DefaultDependencies = no
        [Service]
        ExecStart = /bin/true
        "#,
    );

    let shutdown_id = shutdown.id.clone();
    let with_id = service_with.id.clone();
    let without_id = service_without.id.clone();

    let mut unit_table = std::collections::HashMap::new();
    unit_table.insert(shutdown.id.clone(), shutdown);
    unit_table.insert(service_with.id.clone(), service_with);
    unit_table.insert(service_without.id.clone(), service_without);

    crate::units::fill_dependencies(&mut unit_table).unwrap();
    unit_table
        .values_mut()
        .for_each(|unit| unit.dedup_dependencies());

    // Service with default deps should have shutdown conflict
    let with = unit_table.get(&with_id).unwrap();
    assert!(
        with.common.dependencies.conflicts.contains(&shutdown_id),
        "Service with DefaultDependencies=yes should have Conflicts=shutdown.target"
    );
    assert!(
        with.common.dependencies.before.contains(&shutdown_id),
        "Service with DefaultDependencies=yes should have Before=shutdown.target"
    );

    // Service without default deps should NOT have shutdown conflict
    let without = unit_table.get(&without_id).unwrap();
    assert!(
        !without.common.dependencies.conflicts.contains(&shutdown_id),
        "Service with DefaultDependencies=no should NOT have Conflicts=shutdown.target"
    );
    assert!(
        !without.common.dependencies.before.contains(&shutdown_id),
        "Service with DefaultDependencies=no should NOT have Before=shutdown.target"
    );

    // shutdown.target reverse relations should only reference the service with default deps
    let shutdown = unit_table.get(&shutdown_id).unwrap();
    assert!(
        shutdown
            .common
            .dependencies
            .conflicted_by
            .contains(&with_id),
        "shutdown.target should have ConflictedBy for service with default deps"
    );
    assert!(
        !shutdown
            .common
            .dependencies
            .conflicted_by
            .contains(&without_id),
        "shutdown.target should NOT have ConflictedBy for service without default deps"
    );
}

#[test]
fn test_default_deps_full_service_with_all_targets() {
    // Integration test: service with all three special targets present
    let shutdown = make_target(
        "shutdown.target",
        &target_unit_str_with_default_deps("Shutdown", "no"),
    );
    let sysinit = make_target(
        "sysinit.target",
        &target_unit_str_with_default_deps("System Initialization", "no"),
    );
    let basic = make_target(
        "basic.target",
        &target_unit_str_with_default_deps("Basic System", "no"),
    );
    let service = make_service(
        "myapp.service",
        r#"
        [Service]
        ExecStart = /bin/true
        "#,
    );

    let shutdown_id = shutdown.id.clone();
    let sysinit_id = sysinit.id.clone();
    let basic_id = basic.id.clone();
    let service_id = service.id.clone();

    let mut unit_table = std::collections::HashMap::new();
    unit_table.insert(shutdown.id.clone(), shutdown);
    unit_table.insert(sysinit.id.clone(), sysinit);
    unit_table.insert(basic.id.clone(), basic);
    unit_table.insert(service.id.clone(), service);

    crate::units::fill_dependencies(&mut unit_table).unwrap();
    unit_table
        .values_mut()
        .for_each(|unit| unit.dedup_dependencies());

    let service = unit_table.get(&service_id).unwrap();

    // shutdown.target relations
    assert!(service.common.dependencies.conflicts.contains(&shutdown_id));
    assert!(service.common.dependencies.before.contains(&shutdown_id));

    // sysinit.target relations
    assert!(service.common.dependencies.requires.contains(&sysinit_id));
    assert!(service.common.dependencies.after.contains(&sysinit_id));

    // basic.target relations
    assert!(service.common.dependencies.after.contains(&basic_id));

    // Verify reverse relations
    let shutdown = unit_table.get(&shutdown_id).unwrap();
    assert!(shutdown
        .common
        .dependencies
        .conflicted_by
        .contains(&service_id));
    assert!(shutdown.common.dependencies.after.contains(&service_id));

    let sysinit = unit_table.get(&sysinit_id).unwrap();
    assert!(sysinit
        .common
        .dependencies
        .required_by
        .contains(&service_id));
    assert!(sysinit.common.dependencies.before.contains(&service_id));

    let basic = unit_table.get(&basic_id).unwrap();
    assert!(basic.common.dependencies.before.contains(&service_id));
}

#[test]
fn test_conflicts_bidirectional() {
    // When unit A declares Conflicts=B, fill_dependencies should make it bidirectional:
    // A.conflicts contains B, B.conflicted_by contains A
    let target_a = make_target(
        "a.target",
        r#"
        [Unit]
        Description = Target A
        Conflicts = b.target
        DefaultDependencies = no
        "#,
    );
    let target_b = make_target(
        "b.target",
        &target_unit_str_with_default_deps("Target B", "no"),
    );

    let id_a = target_a.id.clone();
    let id_b = target_b.id.clone();

    let mut unit_table = std::collections::HashMap::new();
    unit_table.insert(target_a.id.clone(), target_a);
    unit_table.insert(target_b.id.clone(), target_b);

    crate::units::fill_dependencies(&mut unit_table).unwrap();
    unit_table
        .values_mut()
        .for_each(|unit| unit.dedup_dependencies());

    let a = unit_table.get(&id_a).unwrap();
    assert!(
        a.common.dependencies.conflicts.contains(&id_b),
        "A should have Conflicts=B"
    );

    let b = unit_table.get(&id_b).unwrap();
    assert!(
        b.common.dependencies.conflicted_by.contains(&id_a),
        "B should have ConflictedBy=A"
    );
}

#[test]
fn test_conflicts_one_way_does_not_reverse_conflicts_field() {
    // When only A declares Conflicts=B, the relationship is one-directional:
    // A.conflicts=[B], B.conflicted_by=[A]
    // But B does NOT get conflicts=[A], and A does NOT get conflicted_by=[B]
    let target_a = make_target(
        "a.target",
        r#"
        [Unit]
        Description = Target A
        Conflicts = b.target
        DefaultDependencies = no
        "#,
    );
    let target_b = make_target(
        "b.target",
        &target_unit_str_with_default_deps("Target B", "no"),
    );

    let id_a = target_a.id.clone();
    let id_b = target_b.id.clone();

    let mut unit_table = std::collections::HashMap::new();
    unit_table.insert(target_a.id.clone(), target_a);
    unit_table.insert(target_b.id.clone(), target_b);

    crate::units::fill_dependencies(&mut unit_table).unwrap();
    unit_table
        .values_mut()
        .for_each(|unit| unit.dedup_dependencies());

    // A declared the conflict, so A.conflicts has B
    let a = unit_table.get(&id_a).unwrap();
    assert!(
        a.common.dependencies.conflicts.contains(&id_b),
        "A should have Conflicts=B"
    );

    // B gets the reverse: conflicted_by=[A]
    let b = unit_table.get(&id_b).unwrap();
    assert!(
        b.common.dependencies.conflicted_by.contains(&id_a),
        "B should have ConflictedBy=A"
    );

    // B should NOT get conflicts=[A] — only A declared the conflict
    assert!(
        !b.common.dependencies.conflicts.contains(&id_a),
        "B should NOT have Conflicts=A (only A declared the conflict)"
    );

    // A should NOT get conflicted_by=[B] — B didn't declare a conflict on A
    let a = unit_table.get(&id_a).unwrap();
    assert!(
        !a.common.dependencies.conflicted_by.contains(&id_b),
        "A should NOT have ConflictedBy=B (B did not declare a conflict)"
    );
}

#[test]
fn test_conflicts_mutual() {
    // Both A and B declare Conflicts on each other
    let target_a = make_target(
        "a.target",
        r#"
        [Unit]
        Description = Target A
        Conflicts = b.target
        DefaultDependencies = no
        "#,
    );
    let target_b = make_target(
        "b.target",
        r#"
        [Unit]
        Description = Target B
        Conflicts = a.target
        DefaultDependencies = no
        "#,
    );

    let id_a = target_a.id.clone();
    let id_b = target_b.id.clone();

    let mut unit_table = std::collections::HashMap::new();
    unit_table.insert(target_a.id.clone(), target_a);
    unit_table.insert(target_b.id.clone(), target_b);

    crate::units::fill_dependencies(&mut unit_table).unwrap();
    unit_table
        .values_mut()
        .for_each(|unit| unit.dedup_dependencies());

    let a = unit_table.get(&id_a).unwrap();
    assert!(
        a.common.dependencies.conflicts.contains(&id_b),
        "A should have Conflicts=B"
    );
    assert!(
        a.common.dependencies.conflicted_by.contains(&id_b),
        "A should have ConflictedBy=B"
    );

    let b = unit_table.get(&id_b).unwrap();
    assert!(
        b.common.dependencies.conflicts.contains(&id_a),
        "B should have Conflicts=A"
    );
    assert!(
        b.common.dependencies.conflicted_by.contains(&id_a),
        "B should have ConflictedBy=A"
    );
}

#[test]
fn test_conflicts_dedup_after_fill() {
    // Mutual conflicts should be deduped so each ID appears only once
    let target_a = make_target(
        "a.target",
        r#"
        [Unit]
        Description = Target A
        Conflicts = b.target
        DefaultDependencies = no
        "#,
    );
    let target_b = make_target(
        "b.target",
        r#"
        [Unit]
        Description = Target B
        Conflicts = a.target
        DefaultDependencies = no
        "#,
    );

    let id_a = target_a.id.clone();
    let id_b = target_b.id.clone();

    let mut unit_table = std::collections::HashMap::new();
    unit_table.insert(target_a.id.clone(), target_a);
    unit_table.insert(target_b.id.clone(), target_b);

    crate::units::fill_dependencies(&mut unit_table).unwrap();
    unit_table
        .values_mut()
        .for_each(|unit| unit.dedup_dependencies());

    let a = unit_table.get(&id_a).unwrap();
    let count_b_in_conflicts = a
        .common
        .dependencies
        .conflicts
        .iter()
        .filter(|id| **id == id_b)
        .count();
    assert_eq!(
        count_b_in_conflicts, 1,
        "B should appear exactly once in A.conflicts after dedup"
    );

    let count_b_in_conflicted_by = a
        .common
        .dependencies
        .conflicted_by
        .iter()
        .filter(|id| **id == id_b)
        .count();
    assert_eq!(
        count_b_in_conflicted_by, 1,
        "B should appear exactly once in A.conflicted_by after dedup"
    );
}

#[test]
fn test_conflicts_multiple_targets() {
    // A conflicts with both B and C
    let target_a = make_target(
        "a.target",
        r#"
        [Unit]
        Description = Target A
        Conflicts = b.target,c.target
        DefaultDependencies = no
        "#,
    );
    let target_b = make_target(
        "b.target",
        &target_unit_str_with_default_deps("Target B", "no"),
    );
    let target_c = make_target(
        "c.target",
        &target_unit_str_with_default_deps("Target C", "no"),
    );

    let id_a = target_a.id.clone();
    let id_b = target_b.id.clone();
    let id_c = target_c.id.clone();

    let mut unit_table = std::collections::HashMap::new();
    unit_table.insert(target_a.id.clone(), target_a);
    unit_table.insert(target_b.id.clone(), target_b);
    unit_table.insert(target_c.id.clone(), target_c);

    crate::units::fill_dependencies(&mut unit_table).unwrap();
    unit_table
        .values_mut()
        .for_each(|unit| unit.dedup_dependencies());

    let a = unit_table.get(&id_a).unwrap();
    assert!(
        a.common.dependencies.conflicts.contains(&id_b),
        "A should have Conflicts=B"
    );
    assert!(
        a.common.dependencies.conflicts.contains(&id_c),
        "A should have Conflicts=C"
    );

    let b = unit_table.get(&id_b).unwrap();
    assert!(
        b.common.dependencies.conflicted_by.contains(&id_a),
        "B should have ConflictedBy=A"
    );

    let c = unit_table.get(&id_c).unwrap();
    assert!(
        c.common.dependencies.conflicted_by.contains(&id_a),
        "C should have ConflictedBy=A"
    );
}

#[test]
fn test_conflicts_service_with_service() {
    // Service-to-service conflict
    let svc_a = make_service(
        "a.service",
        r#"
        [Unit]
        Conflicts = b.service
        DefaultDependencies = no
        [Service]
        ExecStart = /bin/true
        "#,
    );
    let svc_b = make_service(
        "b.service",
        r#"
        [Unit]
        DefaultDependencies = no
        [Service]
        ExecStart = /bin/true
        "#,
    );

    let id_a = svc_a.id.clone();
    let id_b = svc_b.id.clone();

    let mut unit_table = std::collections::HashMap::new();
    unit_table.insert(svc_a.id.clone(), svc_a);
    unit_table.insert(svc_b.id.clone(), svc_b);

    crate::units::fill_dependencies(&mut unit_table).unwrap();
    unit_table
        .values_mut()
        .for_each(|unit| unit.dedup_dependencies());

    let a = unit_table.get(&id_a).unwrap();
    assert!(
        a.common.dependencies.conflicts.contains(&id_b),
        "Service A should have Conflicts=B"
    );

    let b = unit_table.get(&id_b).unwrap();
    assert!(
        b.common.dependencies.conflicted_by.contains(&id_a),
        "Service B should have ConflictedBy=A"
    );
}

#[test]
fn test_conflicts_cross_unit_types() {
    // Service conflicts with a target
    let service = make_service(
        "myapp.service",
        r#"
        [Unit]
        Conflicts = rescue.target
        DefaultDependencies = no
        [Service]
        ExecStart = /bin/true
        "#,
    );
    let target = make_target(
        "rescue.target",
        &target_unit_str_with_default_deps("Rescue", "no"),
    );

    let svc_id = service.id.clone();
    let tgt_id = target.id.clone();

    let mut unit_table = std::collections::HashMap::new();
    unit_table.insert(service.id.clone(), service);
    unit_table.insert(target.id.clone(), target);

    crate::units::fill_dependencies(&mut unit_table).unwrap();
    unit_table
        .values_mut()
        .for_each(|unit| unit.dedup_dependencies());

    let svc = unit_table.get(&svc_id).unwrap();
    assert!(
        svc.common.dependencies.conflicts.contains(&tgt_id),
        "Service should have Conflicts=rescue.target"
    );

    let tgt = unit_table.get(&tgt_id).unwrap();
    assert!(
        tgt.common.dependencies.conflicted_by.contains(&svc_id),
        "rescue.target should have ConflictedBy=myapp.service"
    );
}

#[test]
fn test_conflicts_refs_by_name_includes_conflicts() {
    // Conflicts should be included in refs_by_name so pruning works correctly
    let service = make_service(
        "myapp.service",
        r#"
        [Unit]
        Conflicts = other.service
        DefaultDependencies = no
        [Service]
        ExecStart = /bin/true
        "#,
    );

    let other_id: crate::units::UnitId = "other.service".try_into().unwrap();

    assert!(
        service.common.unit.refs_by_name.contains(&other_id),
        "refs_by_name should include conflicting unit IDs"
    );
}

#[test]
fn test_conflicts_no_conflict_when_not_specified() {
    // Two units with no conflict relationship
    let target_a = make_target(
        "a.target",
        &target_unit_str_with_default_deps("Target A", "no"),
    );
    let target_b = make_target(
        "b.target",
        &target_unit_str_with_default_deps("Target B", "no"),
    );

    let id_a = target_a.id.clone();
    let id_b = target_b.id.clone();

    let mut unit_table = std::collections::HashMap::new();
    unit_table.insert(target_a.id.clone(), target_a);
    unit_table.insert(target_b.id.clone(), target_b);

    crate::units::fill_dependencies(&mut unit_table).unwrap();
    unit_table
        .values_mut()
        .for_each(|unit| unit.dedup_dependencies());

    let a = unit_table.get(&id_a).unwrap();
    assert!(
        !a.common.dependencies.conflicts.contains(&id_b),
        "A should NOT have Conflicts=B when not declared"
    );
    assert!(
        !a.common.dependencies.conflicted_by.contains(&id_b),
        "A should NOT have ConflictedBy=B when not declared"
    );

    let b = unit_table.get(&id_b).unwrap();
    assert!(
        !b.common.dependencies.conflicts.contains(&id_a),
        "B should NOT have Conflicts=A when not declared"
    );
    assert!(
        !b.common.dependencies.conflicted_by.contains(&id_a),
        "B should NOT have ConflictedBy=A when not declared"
    );
}

#[test]
fn test_conflicts_with_before_after_ordering() {
    // Conflicts can coexist with ordering relations
    let target_a = make_target(
        "a.target",
        r#"
        [Unit]
        Description = Target A
        Conflicts = b.target
        Before = b.target
        DefaultDependencies = no
        "#,
    );
    let target_b = make_target(
        "b.target",
        &target_unit_str_with_default_deps("Target B", "no"),
    );

    let id_a = target_a.id.clone();
    let id_b = target_b.id.clone();

    let mut unit_table = std::collections::HashMap::new();
    unit_table.insert(target_a.id.clone(), target_a);
    unit_table.insert(target_b.id.clone(), target_b);

    crate::units::fill_dependencies(&mut unit_table).unwrap();
    unit_table
        .values_mut()
        .for_each(|unit| unit.dedup_dependencies());

    let a = unit_table.get(&id_a).unwrap();
    assert!(
        a.common.dependencies.conflicts.contains(&id_b),
        "A should have Conflicts=B"
    );
    assert!(
        a.common.dependencies.before.contains(&id_b),
        "A should have Before=B"
    );

    let b = unit_table.get(&id_b).unwrap();
    assert!(
        b.common.dependencies.conflicted_by.contains(&id_a),
        "B should have ConflictedBy=A"
    );
    assert!(
        b.common.dependencies.after.contains(&id_a),
        "B should have After=A (reverse of A's Before)"
    );
}

#[test]
fn test_conflicts_chain_three_units() {
    // A conflicts with B, B conflicts with C — conflicts are NOT transitive
    let target_a = make_target(
        "a.target",
        r#"
        [Unit]
        Description = Target A
        Conflicts = b.target
        DefaultDependencies = no
        "#,
    );
    let target_b = make_target(
        "b.target",
        r#"
        [Unit]
        Description = Target B
        Conflicts = c.target
        DefaultDependencies = no
        "#,
    );
    let target_c = make_target(
        "c.target",
        &target_unit_str_with_default_deps("Target C", "no"),
    );

    let id_a = target_a.id.clone();
    let id_b = target_b.id.clone();
    let id_c = target_c.id.clone();

    let mut unit_table = std::collections::HashMap::new();
    unit_table.insert(target_a.id.clone(), target_a);
    unit_table.insert(target_b.id.clone(), target_b);
    unit_table.insert(target_c.id.clone(), target_c);

    crate::units::fill_dependencies(&mut unit_table).unwrap();
    unit_table
        .values_mut()
        .for_each(|unit| unit.dedup_dependencies());

    // A <-> B conflict
    let a = unit_table.get(&id_a).unwrap();
    assert!(a.common.dependencies.conflicts.contains(&id_b));

    // B <-> C conflict
    let b = unit_table.get(&id_b).unwrap();
    assert!(b.common.dependencies.conflicts.contains(&id_c));
    assert!(b.common.dependencies.conflicted_by.contains(&id_a));

    // A should NOT conflict with C (not transitive)
    let a = unit_table.get(&id_a).unwrap();
    assert!(
        !a.common.dependencies.conflicts.contains(&id_c),
        "Conflicts should NOT be transitive: A should not conflict with C"
    );
    assert!(
        !a.common.dependencies.conflicted_by.contains(&id_c),
        "Conflicts should NOT be transitive: A should not be conflicted_by C"
    );

    let c = unit_table.get(&id_c).unwrap();
    assert!(
        !c.common.dependencies.conflicts.contains(&id_a),
        "Conflicts should NOT be transitive: C should not conflict with A"
    );
    assert!(
        !c.common.dependencies.conflicted_by.contains(&id_a),
        "Conflicts should NOT be transitive: C should not be conflicted_by A"
    );
}
