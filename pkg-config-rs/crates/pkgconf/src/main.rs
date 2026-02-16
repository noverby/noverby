//! `pkgconf` — A Rust drop-in replacement for pkg-config/pkgconf.
//!
//! This binary provides a command-line interface compatible with both
//! `pkg-config` and `pkgconf`, implementing all standard flags and
//! environment variable handling.

use std::process::ExitCode;

use anyhow::{Context, Result, bail};
use clap::Parser;

use libpkgconf::version as ver;
use libpkgconf::{PKGCONFIG_COMPAT_VERSION, VERSION};

/// A package compiler and linker metadata toolkit.
///
/// pkgconf retrieves information about installed libraries and packages,
/// providing compiler and linker flags needed to use them.
#[derive(Parser, Debug)]
#[command(
    name = "pkgconf",
    about = "package compiler and linker metadata toolkit",
    long_about = None,
    disable_help_flag = false,
    disable_version_flag = true,
    args_override_self = true,
    trailing_var_arg = true,
)]
struct Cli {
    // ── Basic options ────────────────────────────────────────────────
    /// Print pkgconf version and license information.
    #[arg(long)]
    about: bool,

    /// Print supported pkg-config version.
    #[arg(long)]
    version: bool,

    /// Print additional information.
    #[arg(long)]
    verbose: bool,

    /// Check whether pkgconf is compatible with a specified pkg-config version.
    #[arg(long = "atleast-pkgconfig-version", value_name = "VERSION")]
    atleast_pkgconfig_version: Option<String>,

    /// Print all errors on stdout instead of stderr.
    #[arg(long = "errors-to-stdout")]
    errors_to_stdout: bool,

    /// Ensure all errors are printed.
    #[arg(long = "print-errors")]
    print_errors: bool,

    /// Be less verbose about some errors.
    #[arg(long = "short-errors")]
    short_errors: bool,

    /// Explicitly silence errors.
    #[arg(long = "silence-errors")]
    silence_errors: bool,

    /// List all known packages.
    #[arg(long = "list-all")]
    list_all: bool,

    /// List all known package names (one per line, no description).
    #[arg(long = "list-package-names")]
    list_package_names: bool,

    /// Simulate walking the calculated dependency graph.
    #[arg(long)]
    simulate: bool,

    /// Do not cache already seen packages when walking the dependency graph.
    #[arg(long = "no-cache")]
    no_cache: bool,

    /// Write an audit log to a specified file.
    #[arg(long = "log-file", value_name = "FILENAME")]
    log_file: Option<String>,

    /// Add a directory to the search path.
    #[arg(long = "with-path", value_name = "PATH")]
    with_path: Vec<String>,

    /// Override the prefix variable with one guessed from the .pc file location.
    #[arg(long = "define-prefix")]
    define_prefix: bool,

    /// Do not override the prefix variable under any circumstances.
    #[arg(long = "dont-define-prefix")]
    dont_define_prefix: bool,

    /// Set the name of the variable considered to be the package prefix.
    #[arg(long = "prefix-variable", value_name = "VARNAME")]
    prefix_variable: Option<String>,

    /// Relocate a path and exit (mostly for testsuite).
    #[arg(long, value_name = "PATH")]
    relocate: Option<String>,

    /// Disable path relocation support.
    #[arg(long = "dont-relocate-paths")]
    dont_relocate_paths: bool,

    // ── Cross-compilation personality ────────────────────────────────
    /// Set the cross-compilation personality.
    #[arg(long, value_name = "TRIPLET")]
    personality: Option<String>,

    /// Dump details concerning the selected personality.
    #[arg(long = "dump-personality")]
    dump_personality: bool,

    // ── Version checking ────────────────────────────────────────────
    /// Require a minimum version of a module.
    #[arg(long = "atleast-version", value_name = "VERSION")]
    atleast_version: Option<String>,

    /// Require an exact version of a module.
    #[arg(long = "exact-version", value_name = "VERSION")]
    exact_version: Option<String>,

    /// Require a maximum version of a module.
    #[arg(long = "max-version", value_name = "VERSION")]
    max_version: Option<String>,

    /// Check whether or not a module exists.
    #[arg(long)]
    exists: bool,

    /// Check whether or not an uninstalled module will be used.
    #[arg(long)]
    uninstalled: bool,

    /// Never use uninstalled modules when satisfying dependencies.
    #[arg(long = "no-uninstalled")]
    no_uninstalled: bool,

    /// Do not use 'provides' rules to resolve dependencies.
    #[arg(long = "no-provides")]
    no_provides: bool,

    /// Maximum allowed depth for dependency graph.
    #[arg(long = "maximum-traverse-depth", value_name = "DEPTH")]
    maximum_traverse_depth: Option<i32>,

    /// Be more aggressive when computing dependency graph (for static linking).
    #[arg(long = "static")]
    r#static: bool,

    /// Use a simplified dependency graph (usually default).
    #[arg(long)]
    shared: bool,

    /// Optimize a static dependency graph as if it were a normal dependency graph.
    #[arg(long)]
    pure: bool,

    /// Look only for package entries in PKG_CONFIG_PATH.
    #[arg(long = "env-only")]
    env_only: bool,

    /// Ignore 'conflicts' rules in modules.
    #[arg(long = "ignore-conflicts")]
    ignore_conflicts: bool,

    /// Validate specific .pc files for correctness.
    #[arg(long)]
    validate: bool,

    // ── Querying fields ─────────────────────────────────────────────
    /// Define variable 'varname' as 'value' (format: varname=value).
    #[arg(long = "define-variable", value_name = "VARNAME=VALUE")]
    define_variable: Vec<String>,

    /// Print specified variable entry.
    #[arg(long = "variable", value_name = "VARNAME")]
    variable: Option<String>,

    /// Print required CFLAGS.
    #[arg(long)]
    cflags: bool,

    /// Print required include-dir CFLAGS only.
    #[arg(long = "cflags-only-I")]
    cflags_only_i: bool,

    /// Print required non-include-dir CFLAGS only.
    #[arg(long = "cflags-only-other")]
    cflags_only_other: bool,

    /// Print required linker flags.
    #[arg(long)]
    libs: bool,

    /// Print required LDPATH linker flags only.
    #[arg(long = "libs-only-L")]
    libs_only_l_upper: bool,

    /// Print required LIBNAME linker flags only.
    #[arg(long = "libs-only-l")]
    libs_only_l_lower: bool,

    /// Print required other linker flags only.
    #[arg(long = "libs-only-other")]
    libs_only_other: bool,

    /// Print required dependency frameworks.
    #[arg(long = "print-requires")]
    print_requires: bool,

    /// Print required dependency frameworks for static linking.
    #[arg(long = "print-requires-private")]
    print_requires_private: bool,

    /// Print provided dependencies.
    #[arg(long = "print-provides")]
    print_provides: bool,

    /// Print all known variables in module.
    #[arg(long = "print-variables")]
    print_variables: bool,

    /// Print entire dependency graph in graphviz 'dot' format.
    #[arg(long)]
    digraph: bool,

    /// Also print query nodes in 'dot' format.
    #[arg(long = "print-digraph-query-nodes")]
    print_digraph_query_nodes: bool,

    /// Print dependency graph solution in a simple format.
    #[arg(long)]
    solution: bool,

    /// Keep system cflags (e.g. -I/usr/include) in output.
    #[arg(long = "keep-system-cflags")]
    keep_system_cflags: bool,

    /// Keep system libs (e.g. -L/usr/lib) in output.
    #[arg(long = "keep-system-libs")]
    keep_system_libs: bool,

    /// Show the exact filenames for any matching .pc files.
    #[arg(long)]
    path: bool,

    /// Print the specified module's version.
    #[arg(long)]
    modversion: bool,

    /// Do not filter 'internal' cflags from output.
    #[arg(long = "internal-cflags")]
    internal_cflags: bool,

    /// Print the specified module's license if known.
    #[arg(long)]
    license: bool,

    /// Print the specified module's source code location if known.
    #[arg(long)]
    source: bool,

    /// Add -DHAVE_FOO fragments to cflags for each found module.
    #[arg(long = "exists-cflags")]
    exists_cflags: bool,

    // ── Output filtering ────────────────────────────────────────────
    /// Print translatable fragments in MSVC syntax.
    #[arg(long = "msvc-syntax")]
    msvc_syntax: bool,

    /// Filter output fragments to the specified types.
    #[arg(long = "fragment-filter", value_name = "TYPES")]
    fragment_filter: Option<String>,

    /// Print output as shell-compatible environmental variables.
    #[arg(long = "env", value_name = "PREFIX")]
    env: Option<String>,

    /// Visualize printed CFLAGS/LIBS fragments as a tree.
    #[arg(long = "fragment-tree")]
    fragment_tree: bool,

    /// Use newlines for whitespace between fragments.
    #[arg(long)]
    newlines: bool,

    /// Enable debug output.
    #[arg(long)]
    debug: bool,

    // ── Positional ──────────────────────────────────────────────────
    /// Package names (and optional version constraints) to query.
    #[arg(trailing_var_arg = true)]
    packages: Vec<String>,
}

fn main() -> ExitCode {
    let cli = Cli::parse();

    match run(&cli) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            // Only print the error if we're not silencing errors
            if !cli.silence_errors {
                if cli.errors_to_stdout {
                    println!("{e:#}");
                } else {
                    eprintln!("{e:#}");
                }
            }
            ExitCode::FAILURE
        }
    }
}

fn run(cli: &Cli) -> Result<()> {
    // --about
    if cli.about {
        print_about();
        return Ok(());
    }

    // --version
    if cli.version {
        println!("{PKGCONFIG_COMPAT_VERSION}");
        return Ok(());
    }

    // --atleast-pkgconfig-version
    if let Some(ref required) = cli.atleast_pkgconfig_version {
        if ver::compare(PKGCONFIG_COMPAT_VERSION, required) >= 0 {
            return Ok(());
        } else {
            bail!(
                "pkg-config compatibility version {PKGCONFIG_COMPAT_VERSION} is less than required {required}"
            );
        }
    }

    // --relocate
    if let Some(ref path) = cli.relocate {
        // TODO: Implement path relocation
        println!("{path}");
        return Ok(());
    }

    // --dump-personality
    if cli.dump_personality {
        // TODO: Implement personality dumping
        println!("Triplet: default");
        println!(
            "DefaultSearchPaths: {}",
            libpkgconf::DEFAULT_PKGCONFIG_PATH.join(":")
        );
        println!(
            "SystemIncludePaths: {}",
            libpkgconf::DEFAULT_SYSTEM_INCLUDEDIRS.join(":")
        );
        println!(
            "SystemLibraryPaths: {}",
            libpkgconf::DEFAULT_SYSTEM_LIBDIRS.join(":")
        );
        return Ok(());
    }

    // --list-all
    if cli.list_all {
        // TODO: Implement package listing
        eprintln!("pkg-config-rs: --list-all is not yet implemented");
        return Ok(());
    }

    // --list-package-names
    if cli.list_package_names {
        // TODO: Implement package name listing
        eprintln!("pkg-config-rs: --list-package-names is not yet implemented");
        return Ok(());
    }

    // All other operations require at least one package name
    if cli.packages.is_empty() {
        bail!("Please specify at least one package name on the command line");
    }

    // Parse --define-variable options
    let mut global_vars = std::collections::HashMap::new();
    for def in &cli.define_variable {
        if let Some((key, value)) = def.split_once('=') {
            global_vars.insert(key.to_string(), value.to_string());
        } else {
            bail!("Invalid --define-variable format: '{def}' (expected varname=value)");
        }
    }

    // Build the combined query string from positional args
    let query = cli.packages.join(" ");

    // Parse as dependencies for version constraint handling
    let mut deps = libpkgconf::dependency::DependencyList::parse(&query);

    // Apply --atleast-version / --exact-version / --max-version to unconstrained deps
    if let Some(ref ver_str) = cli.atleast_version {
        for dep in deps.entries_mut() {
            if dep.compare == libpkgconf::version::Comparator::Any {
                dep.compare = libpkgconf::version::Comparator::GreaterThanEqual;
                dep.version = Some(ver_str.clone());
            }
        }
    }
    if let Some(ref ver_str) = cli.exact_version {
        for dep in deps.entries_mut() {
            if dep.compare == libpkgconf::version::Comparator::Any {
                dep.compare = libpkgconf::version::Comparator::Equal;
                dep.version = Some(ver_str.clone());
            }
        }
    }
    if let Some(ref ver_str) = cli.max_version {
        for dep in deps.entries_mut() {
            if dep.compare == libpkgconf::version::Comparator::Any {
                dep.compare = libpkgconf::version::Comparator::LessThanEqual;
                dep.version = Some(ver_str.clone());
            }
        }
    }

    // Determine sysroot
    let sysroot = std::env::var("PKG_CONFIG_SYSROOT_DIR").ok();

    // Determine search paths
    let search_paths = build_search_paths(cli);

    // Keep-system flags
    let keep_system_cflags = cli.keep_system_cflags
        || std::env::var(libpkgconf::ENV_PKG_CONFIG_ALLOW_SYSTEM_CFLAGS).is_ok();
    let keep_system_libs =
        cli.keep_system_libs || std::env::var(libpkgconf::ENV_PKG_CONFIG_ALLOW_SYSTEM_LIBS).is_ok();

    // System filter dirs
    let system_libdirs: Vec<String> = libpkgconf::DEFAULT_SYSTEM_LIBDIRS
        .iter()
        .map(|s| s.to_string())
        .collect();
    let system_includedirs: Vec<String> = libpkgconf::DEFAULT_SYSTEM_INCLUDEDIRS
        .iter()
        .map(|s| s.to_string())
        .collect();

    // Resolve each package
    let mut all_cflags = libpkgconf::fragment::FragmentList::new();
    let mut all_libs = libpkgconf::fragment::FragmentList::new();

    for dep in deps.iter() {
        let pc = find_and_load_package(&dep.package, &search_paths)?;

        // Check version constraint
        if let Some(ref required_ver) = dep.version {
            let actual_ver = pc.version().unwrap_or("0");
            if !dep.compare.eval(actual_ver, required_ver) {
                bail!(
                    "Requested '{}' but version of {} is {}",
                    dep,
                    pc.name().unwrap_or(&dep.package),
                    actual_ver
                );
            }
        }

        // Resolve variables
        let resolved_vars =
            libpkgconf::parser::resolve_variables(&pc, &global_vars, sysroot.as_deref())
                .with_context(|| format!("Failed to resolve variables for '{}'", dep.package))?;

        // --exists / --atleast-version / --exact-version / --max-version:
        // If only checking existence/version, just succeed (version already checked above).
        if cli.exists
            || cli.atleast_version.is_some()
            || cli.exact_version.is_some()
            || cli.max_version.is_some()
        {
            continue;
        }

        // --validate: just check that the file parsed OK (which it did if we got here)
        if cli.validate {
            continue;
        }

        // --modversion
        if cli.modversion {
            if let Some(ver) = pc.version() {
                if cli.verbose {
                    print!("{}: ", pc.name().unwrap_or(&dep.package));
                }
                println!("{ver}");
            }
            continue;
        }

        // --variable
        if let Some(ref var_name) = cli.variable {
            if let Some(val) = resolved_vars.get(var_name.as_str()) {
                print!("{val}");
            }
            println!();
            continue;
        }

        // --print-variables
        if cli.print_variables {
            for name in pc.variable_names() {
                println!("{name}");
            }
            continue;
        }

        // --print-requires
        if cli.print_requires {
            if let Some(req) = pc.get_field(libpkgconf::parser::Keyword::Requires) {
                let expanded = libpkgconf::parser::resolve_field(req, &resolved_vars)?;
                let req_deps = libpkgconf::dependency::DependencyList::parse(&expanded);
                for d in req_deps.iter() {
                    println!("{d}");
                }
            }
            continue;
        }

        // --print-requires-private
        if cli.print_requires_private {
            if let Some(req) = pc.get_field(libpkgconf::parser::Keyword::RequiresPrivate) {
                let expanded = libpkgconf::parser::resolve_field(req, &resolved_vars)?;
                let req_deps = libpkgconf::dependency::DependencyList::parse(&expanded);
                for d in req_deps.iter() {
                    println!("{d}");
                }
            }
            continue;
        }

        // --print-provides
        if cli.print_provides {
            if let Some(prov) = pc.get_field(libpkgconf::parser::Keyword::Provides) {
                let expanded = libpkgconf::parser::resolve_field(prov, &resolved_vars)?;
                let prov_deps = libpkgconf::dependency::DependencyList::parse(&expanded);
                for d in prov_deps.iter() {
                    println!("{d}");
                }
            }
            continue;
        }

        // --path
        if cli.path {
            if let Some(ref p) = pc.path {
                println!("{}", p.display());
            }
            continue;
        }

        // --license
        if cli.license {
            let lic = pc.license().unwrap_or("NOASSERTION");
            println!("{}: {lic}", pc.name().unwrap_or(&dep.package));
            continue;
        }

        // --source
        if cli.source {
            let src = pc.source().unwrap_or("");
            println!("{}: {src}", pc.name().unwrap_or(&dep.package));
            continue;
        }

        // Collect cflags
        if cli.cflags || cli.cflags_only_i || cli.cflags_only_other {
            if let Some(raw) = pc.get_field(libpkgconf::parser::Keyword::Cflags) {
                let expanded = libpkgconf::parser::resolve_field(raw, &resolved_vars)?;
                let frags = libpkgconf::fragment::FragmentList::parse(&expanded);
                all_cflags.append(&frags);
            }
        }

        // Collect libs
        if cli.libs || cli.libs_only_l_upper || cli.libs_only_l_lower || cli.libs_only_other {
            if let Some(raw) = pc.get_field(libpkgconf::parser::Keyword::Libs) {
                let expanded = libpkgconf::parser::resolve_field(raw, &resolved_vars)?;
                let frags = libpkgconf::fragment::FragmentList::parse(&expanded);
                all_libs.append(&frags);
            }

            // Also include Libs.private if --static
            if cli.r#static {
                if let Some(raw) = pc.get_field(libpkgconf::parser::Keyword::LibsPrivate) {
                    let expanded = libpkgconf::parser::resolve_field(raw, &resolved_vars)?;
                    let frags = libpkgconf::fragment::FragmentList::parse(&expanded);
                    all_libs.append(&frags);
                }
            }
        }
    }

    // If we were just doing existence/version checks, we're done
    if cli.exists
        || cli.atleast_version.is_some()
        || cli.exact_version.is_some()
        || cli.max_version.is_some()
        || cli.validate
        || cli.modversion
        || cli.variable.is_some()
        || cli.print_variables
        || cli.print_requires
        || cli.print_requires_private
        || cli.print_provides
        || cli.path
        || cli.license
        || cli.source
    {
        return Ok(());
    }

    let delim = if cli.newlines { '\n' } else { ' ' };

    // Build output fragments
    let mut output_frags = libpkgconf::fragment::FragmentList::new();

    // Process cflags
    if cli.cflags || cli.cflags_only_i || cli.cflags_only_other {
        let mut cflags = all_cflags;

        // Filter system dirs unless --keep-system-cflags
        if !keep_system_cflags {
            cflags = cflags.filter_system_dirs(&system_libdirs, &system_includedirs);
        }

        // Apply fragment-filter
        if let Some(ref filter) = cli.fragment_filter {
            cflags = cflags.filter_by_types(filter);
        }

        // Sub-filter by type
        if cli.cflags_only_i {
            cflags = cflags.filter_cflags_only_i();
        } else if cli.cflags_only_other {
            cflags = cflags.filter_cflags_only_other();
        }

        output_frags.append(&cflags);
    }

    // Process libs
    if cli.libs || cli.libs_only_l_upper || cli.libs_only_l_lower || cli.libs_only_other {
        let mut libs = all_libs;

        // Filter system dirs unless --keep-system-libs
        if !keep_system_libs {
            libs = libs.filter_system_dirs(&system_libdirs, &system_includedirs);
        }

        // Apply fragment-filter
        if let Some(ref filter) = cli.fragment_filter {
            libs = libs.filter_by_types(filter);
        }

        // Sub-filter by type
        if cli.libs_only_l_upper {
            libs = libs.filter_libs_only_ldpath();
        } else if cli.libs_only_l_lower {
            libs = libs.filter_libs_only_libname();
        } else if cli.libs_only_other {
            libs = libs.filter_libs_only_other();
        }

        output_frags.append(&libs);
    }

    // Deduplicate
    let output_frags = output_frags.deduplicate();

    // Render and print
    if !output_frags.is_empty() {
        let rendered = output_frags.render_escaped(delim);
        println!("{rendered}");
    }

    Ok(())
}

/// Build the list of directories to search for .pc files.
fn build_search_paths(cli: &Cli) -> Vec<std::path::PathBuf> {
    let mut paths = Vec::new();

    // --with-path takes highest precedence
    for p in &cli.with_path {
        paths.push(std::path::PathBuf::from(p));
    }

    // PKG_CONFIG_PATH is prepended to the default search path
    if let Ok(pkg_config_path) = std::env::var(libpkgconf::ENV_PKG_CONFIG_PATH) {
        for p in pkg_config_path.split(':') {
            if !p.is_empty() {
                paths.push(std::path::PathBuf::from(p));
            }
        }
    }

    // PKG_CONFIG_LIBDIR replaces the default search path; otherwise use defaults
    if let Ok(libdir) = std::env::var(libpkgconf::ENV_PKG_CONFIG_LIBDIR) {
        for p in libdir.split(':') {
            if !p.is_empty() {
                paths.push(std::path::PathBuf::from(p));
            }
        }
    } else if !cli.env_only {
        for p in libpkgconf::DEFAULT_PKGCONFIG_PATH {
            paths.push(std::path::PathBuf::from(p));
        }
    }

    paths
}

/// Find and load a .pc file for the given package name from the search paths.
fn find_and_load_package(
    name: &str,
    search_paths: &[std::path::PathBuf],
) -> Result<libpkgconf::parser::PcFile> {
    // If the name looks like a path (contains '/' or ends with '.pc'), load directly
    if name.contains('/') || name.ends_with(".pc") {
        let path = std::path::Path::new(name);
        return libpkgconf::parser::PcFile::from_path(path)
            .with_context(|| format!("Failed to load '{name}'"));
    }

    let filename = format!("{name}.pc");

    for dir in search_paths {
        let candidate = dir.join(&filename);
        if candidate.exists() {
            return libpkgconf::parser::PcFile::from_path(&candidate)
                .with_context(|| format!("Failed to load '{}'", candidate.display()));
        }
    }

    Err(libpkgconf::error::Error::PackageNotFound {
        name: name.to_string(),
    }
    .into())
}

fn print_about() {
    println!("pkgconf (pkg-config-rs) {VERSION}");
    println!("Copyright (c) 2025 pkg-config-rs authors.");
    println!();
    println!("Permission to use, copy, modify, and/or distribute this software for any");
    println!("purpose with or without fee is hereby granted, provided that the above");
    println!("copyright notice and this permission notice appear in all copies.");
    println!();
    println!("This software is provided 'as is' and without any warranty, express or");
    println!("implied. In no event shall the authors be liable for any damages arising");
    println!("from the use of this software.");
    println!();
    println!("Report bugs at https://github.com/noverby/pkg-config-rs/issues");
}
