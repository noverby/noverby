use std::path::{Path, PathBuf};

use crate::units::{PlatformSpecificServiceFields, RLimitValue, ResourceLimit};

#[derive(serde::Serialize, serde::Deserialize, Debug)]
pub struct ExecHelperConfig {
    pub name: String,

    pub cmd: PathBuf,
    pub args: Vec<String>,

    pub env: Vec<(String, String)>,

    pub group: libc::gid_t,
    pub supplementary_groups: Vec<libc::gid_t>,
    pub user: libc::uid_t,

    pub working_directory: Option<PathBuf>,
    pub state_directory: Vec<String>,

    pub platform_specific: PlatformSpecificServiceFields,

    pub limit_nofile: Option<ResourceLimit>,
}

fn prepare_exec_args(
    cmd_str: &Path,
    args_str: &[String],
) -> (std::ffi::CString, Vec<std::ffi::CString>) {
    let cmd = std::ffi::CString::new(cmd_str.to_string_lossy().as_bytes()).unwrap();

    let exec_name = std::path::PathBuf::from(cmd_str);
    let exec_name = exec_name.file_name().unwrap();
    let exec_name: Vec<u8> = exec_name.to_str().unwrap().bytes().collect();
    let exec_name = std::ffi::CString::new(exec_name).unwrap();

    let mut args = Vec::new();
    args.push(exec_name);

    for word in args_str {
        args.push(std::ffi::CString::new(word.as_str()).unwrap());
    }

    (cmd, args)
}

pub fn run_exec_helper() {
    println!("Exec helper trying to read config from stdin");
    let config: ExecHelperConfig = serde_json::from_reader(std::io::stdin()).unwrap();
    println!("Apply config: {config:?}");

    nix::unistd::close(libc::STDIN_FILENO).expect("I want to be able to close this fd!");

    // Apply LimitNOFILE resource limit before anything else
    if let Some(ref limit) = config.limit_nofile {
        let soft = match limit.soft {
            RLimitValue::Value(v) => v as libc::rlim_t,
            RLimitValue::Infinity => libc::RLIM_INFINITY,
        };
        let hard = match limit.hard {
            RLimitValue::Value(v) => v as libc::rlim_t,
            RLimitValue::Infinity => libc::RLIM_INFINITY,
        };
        let rlim = libc::rlimit {
            rlim_cur: soft,
            rlim_max: hard,
        };
        let ret = unsafe { libc::setrlimit(libc::RLIMIT_NOFILE, &rlim) };
        if ret != 0 {
            eprintln!(
                "[EXEC_HELPER {}] Failed to set RLIMIT_NOFILE (soft={}, hard={}): {}",
                config.name,
                soft,
                hard,
                std::io::Error::last_os_error()
            );
            std::process::exit(1);
        }
    }

    if let Err(e) =
        crate::services::fork_os_specific::post_fork_os_specific(&config.platform_specific)
    {
        eprintln!("[FORK_CHILD {}] postfork error: {}", config.name, e);
        std::process::exit(1);
    }

    if nix::unistd::getuid().is_root() {
        let supp_gids: Vec<nix::unistd::Gid> = config
            .supplementary_groups
            .iter()
            .map(|gid| nix::unistd::Gid::from_raw(*gid))
            .collect();
        match crate::platform::drop_privileges(
            nix::unistd::Gid::from_raw(config.group),
            &supp_gids,
            nix::unistd::Uid::from_raw(config.user),
        ) {
            Ok(()) => { /* Happy */ }
            Err(e) => {
                eprintln!(
                    "[EXEC_HELPER {}] could not drop privileges because: {}",
                    config.name, e
                );
                std::process::exit(1);
            }
        }
    }

    let (cmd, args) = prepare_exec_args(&config.cmd, &config.args);

    // create state directories under /var/lib/ and set STATE_DIRECTORY env var
    if !config.state_directory.is_empty() {
        let base = Path::new("/var/lib");
        let mut full_paths = Vec::new();
        for dir_name in &config.state_directory {
            let full_path = base.join(dir_name);
            if let Err(e) = std::fs::create_dir_all(&full_path) {
                eprintln!(
                    "[EXEC_HELPER {}] Failed to create state directory {:?}: {}",
                    config.name, full_path, e
                );
                std::process::exit(1);
            }
            // Set ownership to the service user/group
            let uid = nix::unistd::Uid::from_raw(config.user);
            let gid = nix::unistd::Gid::from_raw(config.group);
            if let Err(e) = nix::unistd::chown(&full_path, Some(uid), Some(gid)) {
                eprintln!(
                    "[EXEC_HELPER {}] Failed to chown state directory {:?}: {}",
                    config.name, full_path, e
                );
                std::process::exit(1);
            }
            full_paths.push(full_path.to_string_lossy().into_owned());
        }
        std::env::set_var("STATE_DIRECTORY", full_paths.join(":"));
    }

    // change working directory if configured
    if let Some(ref dir) = config.working_directory {
        let dir = if dir == Path::new("~") {
            // Resolve ~ to the home directory of the current user
            match std::env::var("HOME") {
                Ok(home) => PathBuf::from(home),
                Err(_) => {
                    eprintln!(
                        "[EXEC_HELPER {}] WorkingDirectory=~ but $HOME is not set",
                        config.name
                    );
                    std::process::exit(1);
                }
            }
        } else {
            dir.clone()
        };
        if let Err(e) = std::env::set_current_dir(&dir) {
            eprintln!(
                "[EXEC_HELPER {}] Failed to set working directory to {:?}: {}",
                config.name, dir, e
            );
            std::process::exit(1);
        }
    }

    // setup environment vars
    for (k, v) in &config.env {
        std::env::set_var(k, v);
    }

    std::env::set_var("LISTEN_PID", format!("{}", nix::unistd::getpid()));

    eprintln!("EXECV: {:?} {:?}", &cmd, &args);

    nix::unistd::execv(&cmd, &args).unwrap();
}
