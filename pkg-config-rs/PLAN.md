# Implementation Plan

This plan is organized into phases, each building on the previous one. Each phase produces a testable, working subset of functionality. The plan is designed so that you can iterate over it phase by phase until the project is complete.

## Phase 1: Foundation ✅

**Goal:** Core data types, `.pc` file parsing, version comparison, dependency parsing, and flag fragment management.

- [x] **1.1 Error types** (`error.rs`)
  - `Error` enum with all error variants (PackageNotFound, VersionMismatch, ParseError, etc.)
  - `ErrorFlags` bitfield compatible with pkgconf's `PKGCONF_PKG_ERRF_*`
  - `Result<T>` type alias
  - Unit tests

- [x] **1.2 Version comparison** (`version.rs`)
  - RPM-style `compare(a, b) -> i32` function (rpmvercmp algorithm)
  - `Comparator` enum (Any, Equal, NotEqual, LessThan, LessThanEqual, GreaterThan, GreaterThanEqual)
  - `Comparator::eval(actual, target) -> bool`
  - `is_operator_char()` and `is_module_separator()` helpers
  - Comprehensive tests including edge cases, tilde pre-release, real-world versions

- [x] **1.3 Parser** (`parser.rs`)
  - `PcFile` struct representing a parsed `.pc` file
  - `Keyword` enum for all known field names (Name, Description, Version, URL, Requires, Libs, Cflags, etc.)
  - `Directive` enum (Variable, Field, Comment, Blank)
  - Line-continuation handling (backslash at end of line)
  - Variable interpolation: `${varname}` expansion with recursion detection
  - `expand_variables()`, `resolve_variables()`, `resolve_field()`
  - `build_lookup()` function with proper resolution order (global > package > builtin)
  - `argv_split()` for shell-like argument splitting
  - Magic `pcfiledir` variable injection
  - Support for `pc_sysrootdir` built-in variable
  - Comprehensive tests

- [x] **1.4 Dependency parsing** (`dependency.rs`)
  - `Dependency` struct (package, comparator, version, flags)
  - `DependencyFlags` bitfield (NONE, INTERNAL, PRIVATE, QUERY)
  - `DependencyList` with `parse()` supporting comma and whitespace separation
  - `DependencyList::parse_with_flags()`
  - Display/formatting for round-trip fidelity
  - Real-world dependency string tests (GTK, Qt, etc.)

- [x] **1.5 Fragment management** (`fragment.rs`)
  - `Fragment` struct (type char, data, flags)
  - `FragmentList` with `parse()` for flags strings
  - Type classification: I (include), L (lib path), l (lib name), D (define), W (warning), U (undefine), untyped
  - Filtering: `filter_system_dirs()`, `filter_cflags_only_i()`, `filter_libs_only_ldpath()`, etc.
  - Deduplication with correct semantics (I/L/D keep first, l/untyped keep last)
  - Rendering with optional escaping and configurable delimiter
  - Quoting and backslash-escape handling in parsing

- [x] **1.6 CLI scaffolding** (`main.rs`)
  - clap-based argument parser matching all pkgconf options
  - `--version`, `--about`, `--help`
  - `--atleast-pkgconfig-version`
  - Basic package lookup from search paths
  - `--modversion`, `--cflags`, `--libs`, `--exists`, `--variable`, etc.
  - `PKG_CONFIG_PATH`, `PKG_CONFIG_LIBDIR` environment variable handling
  - System directory filtering with `--keep-system-cflags`, `--keep-system-libs`

---

## Phase 2: Client & Search Path Management ✅

**Goal:** Proper client state management and search path handling, including environment variables, sysroot, and build root.

- [x] **2.1 Search path module** (`path.rs`)
  - `SearchPath` struct wrapping a list of directories
  - `add()`, `prepend()`, `split()` from colon-delimited strings
  - `build_from_environ()` for `PKG_CONFIG_PATH`, `PKG_CONFIG_LIBDIR`
  - `match_list()` to check if a path is in the list
  - Path normalization and deduplication
  - `relocate()` for Windows-style path relocation
  - `copy_list()`, `prepend_list()` for list merging
  - Unit tests with various path formats

- [x] **2.2 Client module** (`client.rs`)
  - `Client` struct holding all state:
    - Search path list (`dir_list`)
    - Filter lists for system lib/include dirs
    - Global variable overrides
    - Sysroot and buildroot directories
    - Client flags (bitfield matching `PKGCONF_PKG_PKGF_*`)
    - Prefix variable name (default: `prefix`)
    - Serial and identifier counters for graph traversal
  - `Client::new()` / `Client::builder()` with configuration
  - `Client::set_sysroot_dir()`, `Client::set_buildroot_dir()`
  - `Client::set_flags()` / `Client::get_flags()`
  - `Client::dir_list_build()` — construct search path from personality + env
  - `Client::getenv()` — lookup handler (allows mocking in tests)
  - Error, warning, and trace handler callbacks
  - Comprehensive environment variable handling:
    - `PKG_CONFIG_PATH`, `PKG_CONFIG_LIBDIR`
    - `PKG_CONFIG_SYSROOT_DIR`, `PKG_CONFIG_TOP_BUILD_DIR`
    - `PKG_CONFIG_ALLOW_SYSTEM_CFLAGS`, `PKG_CONFIG_ALLOW_SYSTEM_LIBS`
    - `PKG_CONFIG_DISABLE_UNINSTALLED`
    - `PKG_CONFIG_MAXIMUM_TRAVERSE_DEPTH`
    - `PKG_CONFIG_IGNORE_CONFLICTS`
    - `PKG_CONFIG_PURE_DEPGRAPH`
    - `PKG_CONFIG_RELOCATE_PATHS`
    - `PKG_CONFIG_DONT_DEFINE_PREFIX`
    - `PKG_CONFIG_DONT_RELOCATE_PATHS`
    - `PKG_CONFIG_FDO_SYSROOT_RULES`
    - `PKG_CONFIG_LOG`
    - `PKG_CONFIG_MSVC_SYNTAX`
    - `PKG_CONFIG_DEBUG_SPEW`
  - Integration tests

- [x] **2.3 Integrate client into CLI**
  - Refactor `main.rs` to use `Client` instead of ad-hoc state
  - Move search path construction into `Client`
  - Move system directory filtering into `Client`
  - Wire up `--define-variable` through `Client::global_vars`
  - Wire up `--with-path` through `Client::dir_list`
  - Wire up `--static`, `--shared`, `--pure` through client flags

---

## Phase 3: Package Model & Loading

**Goal:** Full package representation with `.pc` file loading, prefix redefinition, provides, conflicts, and uninstalled package support.

- [ ] **3.1 Package module** (`pkg.rs`)
  - `Package` struct mirroring pkgconf's `pkgconf_pkg_t`:
    - id, filename, realname, version, description, url
    - pc_filedir, license, maintainer, copyright, source, license_file
    - Parsed fragment lists: libs, libs_private, cflags, cflags_private
    - Parsed dependency lists: required, requires_private, conflicts, provides
    - Variable map (resolved)
    - Flags (STATIC, CACHED, UNINSTALLED, VIRTUAL, ANCESTOR, etc.)
    - Reference counting (or `Arc`-based ownership)
  - `Package::from_pc_file()` — convert a parsed `PcFile` into a fully resolved `Package`
  - `Package::find()` — search for a package by name in the client's search paths
  - `Package::verify_dependency()` — check if a dependency is satisfied by this package
  - Virtual package support (packages without a .pc file)
  - Uninstalled package detection (`.pc` files with `-uninstalled` suffix)
  - Prefix redefinition logic (`--define-prefix`):
    - Calculate the effective prefix from the `.pc` file location
    - Override the `prefix` variable (or custom `--prefix-variable`)
  - `Provides` resolution: a package can satisfy dependencies for other names
  - `scan_all()` — iterate over all `.pc` files in search paths

- [ ] **3.2 Cache module** (`cache.rs`)
  - `Cache` backed by a `HashMap<String, Package>`
  - `lookup()`, `add()`, `remove()`
  - Respect `PKGCONF_PKG_PKGF_NO_CACHE` flag
  - Preloaded / virtual package registration
  - Cache invalidation on search path changes

- [ ] **3.3 Built-in virtual packages**
  - Register `pkg-config` and `pkgconf` virtual packages
  - Populate with `pc_path`, `pc_system_libdirs`, `pc_system_includedirs` variables
  - Set version to compatibility version

---

## Phase 4: Dependency Graph Solver

**Goal:** Full dependency graph resolution matching pkgconf's flattened directed graph algorithm.

- [ ] **4.1 Queue module** (`queue.rs`)
  - `Queue` struct — ordered list of package queries
  - `push()`, `push_dependency()`
  - `compile()` — parse queue entries into the world package's dependency list
  - `solve()` — the main entry point:
    1. Compile the queue into a virtual "world" package
    2. For each dependency, find and load the package
    3. Recursively resolve transitive dependencies
    4. Build the flattened dependency graph
    5. Check version constraints
    6. Detect and report conflicts
    7. Return the resolved world package
  - `validate()` — check `.pc` files for correctness without resolving
  - `apply()` — generic traversal with callback
  - Depth limiting (`maximum_traverse_depth`)

- [ ] **4.2 Graph traversal** (in `pkg.rs`)
  - `Package::traverse()` — walk the dependency graph depth-first
  - Visited tracking via serial numbers (avoiding re-traversal)
  - Private dependency handling:
    - `PKGCONF_PKG_PKGF_SEARCH_PRIVATE` — include private deps in traversal
    - `PKGCONF_PKG_PKGF_MERGE_PRIVATE_FRAGMENTS` — merge private fragments
  - Skip flags: `SKIP_ROOT_VIRTUAL`, `SKIP_CONFLICTS`, `SKIP_PROVIDES`, `SKIP_ERRORS`
  - Conflict detection: `walk_conflicts_list()`
  - Graph verification: `verify_graph()`

- [ ] **4.3 Fragment collection** (in `pkg.rs`)
  - `Package::cflags()` — collect cflags from the dependency graph
  - `Package::libs()` — collect libs from the dependency graph
  - Proper ordering: cflags are depth-first, libs respect link order
  - Fragment merging from private dependencies when `--static`

- [ ] **4.4 Integrate solver into CLI**
  - Replace the per-package loading in `main.rs` with queue-based solving
  - Implement `--exists` using the solver
  - Handle multi-package queries correctly
  - Handle recursive dependencies
  - Report proper error messages matching pkgconf output format

---

## Phase 5: Advanced Features

**Goal:** Cross-compilation personalities, MSVC syntax, sysroot handling, and remaining CLI features.

- [ ] **5.1 Cross-compilation personalities** (`personality.rs`)
  - `CrossPersonality` struct:
    - name (triplet)
    - dir_list (search paths for this personality)
    - filter_libdirs, filter_includedirs
    - sysroot_dir
    - want_default_static, want_default_pure
  - `CrossPersonality::default()` — system default personality
  - `CrossPersonality::find(triplet)` — load from personality file or system config
  - Personality file parsing (INI-like format)
  - Personality deduction from `argv[0]` (e.g. `x86_64-linux-gnu-pkg-config`)
  - `--personality` and `--dump-personality` CLI support

- [ ] **5.2 Sysroot handling**
  - FDO sysroot rules (`PKG_CONFIG_FDO_SYSROOT_RULES`)
  - pkgconf1 sysroot rules (`PKG_CONFIG_PKGCONF1_SYSROOT_RULES`)
  - Sysroot prepending to `-I` and `-L` paths
  - DESTDIR interaction with sysroot
  - Buildroot directory support

- [ ] **5.3 Path relocation**
  - `--define-prefix` auto-detection of prefix from `.pc` file location
  - `--dont-define-prefix` to disable
  - `--prefix-variable` to customize which variable is overridden
  - `--relocate` path transformation
  - `--dont-relocate-paths` to disable relocation
  - Windows-style path relocation (drive letter handling)

- [ ] **5.4 MSVC syntax renderer**
  - Fragment renderer that translates GCC-style flags to MSVC:
    - `-I` → `/I`
    - `-L` → `/LIBPATH:`
    - `-l` → `<name>.lib`
    - `-D` → `/D`
  - `--msvc-syntax` CLI flag
  - `PKG_CONFIG_MSVC_SYNTAX` environment variable

- [ ] **5.5 Audit logging** (`audit.rs`)
  - `--log-file` support
  - `PKG_CONFIG_LOG` environment variable
  - Log dependency resolution decisions
  - `audit_log_dependency()` for each resolved dep

- [ ] **5.6 Remaining CLI features**
  - `--simulate` — print dependency graph without resolving flags
  - `--digraph` — output in graphviz dot format
  - `--print-digraph-query-nodes` — include query nodes in dot output
  - `--solution` — print the dependency solution
  - `--fragment-tree` — visualize CFLAGS/LIBS as a tree
  - `--env` — output as shell-compatible environment variables
  - `--exists-cflags` — add `-DHAVE_FOO` for each found module
  - `--internal-cflags` — don't filter internal cflags
  - `--uninstalled` / `--no-uninstalled` — uninstalled package handling
  - `--validate` — validate `.pc` files for correctness

- [ ] **5.7 Preloaded packages**
  - `PKG_CONFIG_PRELOADED_FILES` support
  - `Client::preload_path()` and `Client::preload_from_environ()`

---

## Phase 6: Compatibility & Testing

**Goal:** Ensure output-level compatibility with pkgconf by running against real-world `.pc` files and the pkgconf test suite.

- [ ] **6.1 Port pkgconf test suite**
  - Adapt tests from pkgconf's `tests/` directory
  - Create test `.pc` files in `tests/data/`
  - Test categories:
    - Basic queries (cflags, libs, modversion, variable)
    - Version comparison and constraints
    - Dependency resolution (simple, diamond, circular detection)
    - Fragment deduplication and ordering
    - System directory filtering
    - Sysroot and prefix redefinition
    - Static linking
    - Conflicts detection
    - Provides resolution
    - Uninstalled packages
    - Cross-compilation personalities
    - Error messages
    - Environment variable handling

- [ ] **6.2 Differential testing**
  - Build a test harness that runs both `pkgconf` (C) and our binary on the same inputs
  - Compare outputs for:
    - `--cflags`
    - `--libs`
    - `--modversion`
    - `--variable`
    - `--exists` (exit code)
    - `--print-requires`
    - `--print-provides`
    - `--list-all`
  - Run against all `.pc` files in `/usr/lib/pkgconfig/` and `/usr/share/pkgconfig/`

- [ ] **6.3 Edge case testing**
  - Empty `.pc` files
  - `.pc` files with only variables, no fields
  - Very deep dependency chains
  - Very wide dependency fan-out
  - Circular dependencies (should be detected and reported)
  - Invalid `.pc` files (malformed lines, missing fields)
  - Unicode in paths and values
  - Windows path separators (`;` instead of `:`)
  - Very long flag strings

- [ ] **6.4 Performance benchmarking**
  - Benchmark against pkgconf on large dependency graphs (e.g. GTK, Qt, Abseil)
  - Measure startup time (should be competitive since Rust has no runtime init)
  - Profile memory usage
  - Optimize hot paths if needed

---

## Phase 7: Platform Support & Distribution

**Goal:** Cross-platform support, packaging, and release.

- [ ] **7.1 Platform support**
  - Linux (primary target) ✅
  - macOS
    - Framework handling (`-framework`)
    - Default paths (`/usr/local/lib/pkgconfig`, Homebrew paths)
  - Windows
    - Registry-based search paths
    - MSVC syntax by default
    - Path separator handling (`;`)
    - UNC path support
  - FreeBSD / OpenBSD
    - `pledge()` / `unveil()` support (or graceful no-op)

- [ ] **7.2 Packaging**
  - Nix flake integration (add to workspace `flake.nix`)
  - Create `pkg-config` symlink in packaging
  - Cargo install support
  - Binary releases for major platforms

- [ ] **7.3 Documentation**
  - Comprehensive rustdoc on all public API
  - Man page generation
  - CHANGELOG.md
  - CONTRIBUTING.md
  - License file (ISC, matching pkgconf)

---
