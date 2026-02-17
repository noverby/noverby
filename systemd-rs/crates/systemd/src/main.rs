fn main() {
    let exec_name = std::env::args()
        .next()
        .expect("could not get executable name from args");
    if exec_name.ends_with("exec_helper") {
        libsystemd::entrypoints::run_exec_helper();
    } else if exec_name.ends_with("systemd-rs")
        || exec_name.ends_with("systemd_rs")
        || exec_name.ends_with("systemd")
    {
        libsystemd::entrypoints::run_service_manager();
    } else {
        eprintln!("Can only start as systemd, systemd-rs, or exec_helper. Was: {exec_name}");
    }
}
