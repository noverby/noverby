//! systemd-escape — Escape strings for use in systemd unit names.
//!
//! A drop-in replacement for `systemd-escape(1)` supporting:
//!
//! - Escaping arbitrary strings for use in unit names
//! - Unescaping unit name components back to the original string
//! - Path escaping/unescaping (for .mount / .device style names)
//! - Mangling strings into complete unit names
//! - Template instantiation (`--template`)
//! - Instance extraction (`--instance`)
//! - Appending a unit type suffix (`--suffix`)

use clap::Parser;
use libsystemd::unit_name;
use std::process;

#[derive(Parser, Debug)]
#[command(
    name = "systemd-escape",
    about = "Escape strings for use as systemd unit names",
    version
)]
struct Cli {
    /// Unescape the given strings instead of escaping them.
    #[arg(short, long)]
    unescape: bool,

    /// Mangle the given strings into unit names (appending .service if needed).
    #[arg(short, long)]
    mangle: bool,

    /// Treat the input/output as a filesystem path (strip leading/trailing
    /// slashes, collapse consecutive slashes).
    #[arg(short, long)]
    path: bool,

    /// Append the specified unit type suffix (e.g. "mount", "service") to
    /// the escaped string. The leading dot is added automatically if not
    /// present.
    #[arg(long, value_name = "SUFFIX")]
    suffix: Option<String>,

    /// Use the specified unit template and insert the escaped string as
    /// the instance. TEMPLATE should be a template unit name like
    /// "foo@.service".
    #[arg(long, value_name = "TEMPLATE")]
    template: Option<String>,

    /// When used with --unescape, extract and print only the instance part
    /// of a template unit name.
    #[arg(long)]
    instance: bool,

    /// The strings to escape or unescape. If none are given, reads from
    /// stdin (one per line).
    strings: Vec<String>,
}

fn read_stdin_lines() -> Vec<String> {
    use std::io::BufRead;
    let stdin = std::io::stdin();
    let mut lines = Vec::new();
    for line in stdin.lock().lines() {
        match line {
            Ok(l) => lines.push(l),
            Err(e) => {
                eprintln!("Error reading stdin: {e}");
                process::exit(1);
            }
        }
    }
    lines
}

fn normalize_suffix(suffix: &str) -> String {
    if suffix.starts_with('.') {
        suffix.to_string()
    } else {
        format!(".{suffix}")
    }
}

fn do_escape(input: &str, cli: &Cli) -> Result<String, String> {
    if cli.unescape {
        // Unescape mode
        if cli.instance {
            // Extract instance from a template instance name
            let name = input;
            match unit_name::unit_name_template_split(name) {
                Some((_prefix, instance, _suffix)) => {
                    if cli.path {
                        unit_name::unit_name_path_unescape(instance)
                            .ok_or_else(|| format!("Failed to path-unescape instance: {instance}"))
                    } else {
                        unit_name::unit_name_unescape(instance)
                            .ok_or_else(|| format!("Failed to unescape instance: {instance}"))
                    }
                }
                None => Err(format!("Not a template instance unit name: {name}")),
            }
        } else {
            // Strip known suffix before unescaping if present
            let name = strip_unit_suffix(input);

            if cli.path {
                unit_name::unit_name_path_unescape(name)
                    .ok_or_else(|| format!("Failed to path-unescape: {name}"))
            } else {
                unit_name::unit_name_unescape(name)
                    .ok_or_else(|| format!("Failed to unescape: {name}"))
            }
        }
    } else if cli.mangle {
        // Mangle mode
        Ok(unit_name::unit_name_mangle(input))
    } else {
        // Escape mode (default)
        let escaped = if cli.path {
            unit_name::unit_name_path_escape(input)
        } else {
            unit_name::unit_name_escape(input)
        };

        // Apply --template if given
        if let Some(template) = &cli.template {
            if !unit_name::is_template(template) {
                return Err(format!("Not a valid template unit name: {template}"));
            }
            unit_name::template_instantiate(template, &escaped)
                .ok_or_else(|| format!("Failed to instantiate template {template} with {escaped}"))
        } else if let Some(suffix) = &cli.suffix {
            // Apply --suffix
            let norm = normalize_suffix(suffix);
            Ok(format!("{escaped}{norm}"))
        } else {
            Ok(escaped)
        }
    }
}

/// Strip a recognized unit type suffix from a name, returning the name
/// part without the suffix. If no recognized suffix is found, returns
/// the whole input.
fn strip_unit_suffix(name: &str) -> &str {
    const SUFFIXES: &[&str] = &[
        ".service",
        ".socket",
        ".target",
        ".device",
        ".mount",
        ".automount",
        ".swap",
        ".timer",
        ".path",
        ".slice",
        ".scope",
    ];

    for &suffix in SUFFIXES {
        if let Some(stripped) = name.strip_suffix(suffix) {
            return stripped;
        }
    }
    name
}

fn main() {
    let cli = Cli::parse();

    // Validate conflicting options
    if cli.unescape && cli.mangle {
        eprintln!("Error: --unescape and --mangle cannot be used together.");
        process::exit(1);
    }
    if cli.mangle && cli.template.is_some() {
        eprintln!("Error: --mangle and --template cannot be used together.");
        process::exit(1);
    }
    if cli.instance && !cli.unescape {
        eprintln!("Error: --instance can only be used with --unescape.");
        process::exit(1);
    }

    let inputs = if cli.strings.is_empty() {
        read_stdin_lines()
    } else {
        cli.strings.clone()
    };

    if inputs.is_empty() {
        eprintln!("Error: no input strings provided.");
        process::exit(1);
    }

    let mut exit_code = 0;

    for input in &inputs {
        match do_escape(input, &cli) {
            Ok(result) => println!("{result}"),
            Err(e) => {
                eprintln!("Error: {e}");
                exit_code = 1;
            }
        }
    }

    process::exit(exit_code);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_cli(
        unescape: bool,
        mangle: bool,
        path: bool,
        suffix: Option<&str>,
        template: Option<&str>,
        instance: bool,
    ) -> Cli {
        Cli {
            unescape,
            mangle,
            path,
            suffix: suffix.map(String::from),
            template: template.map(String::from),
            instance,
            strings: vec![],
        }
    }

    #[test]
    fn test_basic_escape() {
        let cli = make_cli(false, false, false, None, None, false);
        assert_eq!(do_escape("foo bar", &cli).unwrap(), r"foo\x20bar");
    }

    #[test]
    fn test_basic_unescape() {
        let cli = make_cli(true, false, false, None, None, false);
        assert_eq!(do_escape(r"foo\x20bar", &cli).unwrap(), "foo bar");
    }

    #[test]
    fn test_path_escape() {
        let cli = make_cli(false, false, true, None, None, false);
        assert_eq!(do_escape("/foo/bar", &cli).unwrap(), "foo-bar");
    }

    #[test]
    fn test_path_unescape() {
        let cli = make_cli(true, false, true, None, None, false);
        assert_eq!(do_escape("foo-bar", &cli).unwrap(), "/foo/bar");
    }

    #[test]
    fn test_escape_with_suffix() {
        let cli = make_cli(false, false, true, Some("mount"), None, false);
        assert_eq!(do_escape("/foo/bar", &cli).unwrap(), "foo-bar.mount");
    }

    #[test]
    fn test_escape_with_template() {
        let cli = make_cli(false, false, false, None, Some("foo@.service"), false);
        assert_eq!(do_escape("bar", &cli).unwrap(), "foo@bar.service");
    }

    #[test]
    fn test_unescape_instance() {
        let cli = make_cli(true, false, false, None, None, true);
        assert_eq!(do_escape("foo@bar.service", &cli).unwrap(), "bar");
    }

    #[test]
    fn test_mangle() {
        let cli = make_cli(false, true, false, None, None, false);
        assert_eq!(do_escape("foo", &cli).unwrap(), "foo.service");
        assert_eq!(do_escape("foo.service", &cli).unwrap(), "foo.service");
    }

    #[test]
    fn test_unescape_strips_suffix() {
        let cli = make_cli(true, false, false, None, None, false);
        assert_eq!(do_escape("foo-bar.mount", &cli).unwrap(), "foo/bar");
    }

    #[test]
    fn test_escape_root_path() {
        let cli = make_cli(false, false, true, Some("mount"), None, false);
        assert_eq!(do_escape("/", &cli).unwrap(), "-.mount");
    }
}
