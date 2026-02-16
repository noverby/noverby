fn main() {
    let exec_name = std::env::args()
        .next()
        .expect("could not get executable name from args");
    if exec_name.ends_with("exec_helper") {
        systemd_rs::entrypoints::run_exec_helper();
    } else if exec_name.ends_with("systemd-rs") || exec_name.ends_with("systemd_rs") {
        systemd_rs::entrypoints::run_service_manager();
    } else {
        eprintln!("Can only start as systemd-rs or exec_helper. Was: {exec_name}");
    }
}
