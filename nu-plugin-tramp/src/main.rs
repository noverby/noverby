//! `nu_plugin_tramp` — A TRAMP-inspired remote filesystem plugin for Nushell.
//!
//! This is the plugin entry point.  It registers the following commands:
//!
//! - `tramp open <path>`        — read a remote file and return it as a Nushell value
//! - `tramp ls <path>`          — list a remote directory as a Nushell table
//! - `tramp save <path>`        — write piped data to a remote file
//! - `tramp rm <path>`          — delete a remote file
//! - `tramp cp <src> <dst>`     — copy files between local/remote locations
//! - `tramp cd <path>`          — set the remote working directory
//! - `tramp ping <path>`        — test connectivity to a remote host
//! - `tramp connections`        — list active connections
//! - `tramp disconnect [host]`  — close connections
//!
//! Paths use the TRAMP URI format: `/ssh:user@host#port:/remote/path`

mod backend;
mod errors;
mod protocol;
mod vfs;

use std::sync::{Arc, Mutex};

use nu_plugin::{
    EngineInterface, EvaluatedCall, MsgPackSerializer, Plugin, PluginCommand, serve_plugin,
};
use nu_protocol::{
    Category, Example, LabeledError, PipelineData, Record, Signature, Span, SyntaxShape, Type,
    Value,
};

use errors::TrampError;
use protocol::TrampPath;
use vfs::Vfs;

// ---------------------------------------------------------------------------
// Plugin struct
// ---------------------------------------------------------------------------

/// The main plugin object.  Holds a shared [`Vfs`] instance whose connection
/// pool is reused across all command invocations for the lifetime of the
/// plugin process.
struct TrampPlugin {
    vfs: Arc<Vfs>,
    /// The current remote working directory, set by `tramp cd`.
    /// When set, relative paths passed to tramp commands are resolved
    /// against this URI.
    remote_cwd: Arc<Mutex<Option<TrampPath>>>,
}

impl TrampPlugin {
    fn new() -> Self {
        Self {
            vfs: Arc::new(Vfs::new().expect("failed to initialise tramp VFS")),
            remote_cwd: Arc::new(Mutex::new(None)),
        }
    }
}

impl Plugin for TrampPlugin {
    fn version(&self) -> String {
        env!("CARGO_PKG_VERSION").into()
    }

    fn commands(&self) -> Vec<Box<dyn PluginCommand<Plugin = Self>>> {
        vec![
            Box::new(TrampMain),
            Box::new(TrampOpen),
            Box::new(TrampLs),
            Box::new(TrampSave),
            Box::new(TrampRm),
            Box::new(TrampCp),
            Box::new(TrampCd),
            Box::new(TrampPwd),
            Box::new(TrampPing),
            Box::new(TrampConnections),
            Box::new(TrampDisconnect),
        ]
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Resolve a raw string path to a [`TrampPath`].
///
/// If the string is already a full TRAMP URI it is parsed directly.
/// If it is a relative (or absolute-but-not-TRAMP) path **and** the plugin
/// has a remote CWD set via `tramp cd`, the path is resolved against
/// that CWD.  Otherwise an error is returned explaining the expected format.
fn resolve_tramp_path(
    raw: &str,
    remote_cwd: &Mutex<Option<TrampPath>>,
    span: Span,
) -> Result<TrampPath, LabeledError> {
    // Try parsing as a full TRAMP URI first.
    match protocol::parse(raw) {
        Ok(Some(path)) => return Ok(path),
        Ok(None) => {} // Not a TRAMP URI — try relative resolution below.
        Err(e) => {
            return Err(LabeledError::new(e.to_string()).with_label("parse error", span));
        }
    }

    // Attempt relative resolution against the current remote CWD.
    let cwd_guard = remote_cwd
        .lock()
        .map_err(|e| LabeledError::new(format!("internal lock error: {e}")))?;

    if let Some(ref cwd) = *cwd_guard {
        let resolved = resolve_relative(&cwd.remote_path, raw);
        let mut new_path = cwd.clone();
        new_path.remote_path = resolved;
        Ok(new_path)
    } else {
        Err(LabeledError::new(
            "not a tramp path — expected format: /ssh:host:/remote/path\n\
             Hint: set a remote working directory with `tramp cd /ssh:host:/dir` \
             to use relative paths.",
        )
        .with_label("this is not a TRAMP URI", span))
    }
}

/// Parse a positional string argument at index 0 as a TRAMP path, with
/// relative path resolution support.
fn parse_tramp_arg(
    call: &EvaluatedCall,
    remote_cwd: &Mutex<Option<TrampPath>>,
) -> Result<TrampPath, LabeledError> {
    let raw: String = call.req(0)?;
    resolve_tramp_path(&raw, remote_cwd, call.head)
}

/// Resolve a relative path against a base directory.
///
/// - Absolute paths (starting with `/`) are returned unchanged.
/// - `..` and `.` components are handled.
/// - The result is always a clean absolute path.
fn resolve_relative(base: &str, relative: &str) -> String {
    if relative.starts_with('/') {
        return normalize_path(relative);
    }

    // Start with the base directory's components.
    let base_trimmed = base.trim_end_matches('/');
    let mut components: Vec<&str> = base_trimmed.split('/').filter(|c| !c.is_empty()).collect();

    for part in relative.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                components.pop();
            }
            other => {
                components.push(other);
            }
        }
    }

    if components.is_empty() {
        "/".to_string()
    } else {
        format!("/{}", components.join("/"))
    }
}

/// Normalize an absolute path by resolving `.` and `..` components.
fn normalize_path(path: &str) -> String {
    let mut components: Vec<&str> = Vec::new();
    for part in path.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                components.pop();
            }
            other => {
                components.push(other);
            }
        }
    }
    if components.is_empty() {
        "/".to_string()
    } else {
        format!("/{}", components.join("/"))
    }
}

/// Try to auto-detect the file format from the extension and parse the bytes
/// into a structured Nushell [`Value`].  Falls back to a plain string (if
/// valid UTF-8) or binary.
fn bytes_to_value(data: &[u8], remote_path: &str, span: Span) -> Value {
    let ext = std::path::Path::new(remote_path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("");

    // If the file is a known structured format, try to parse it via the raw
    // text so the user gets a rich Value.  For Phase 1 we only handle JSON
    // and TOML — extending this is trivial.
    if let Ok(text) = std::str::from_utf8(data) {
        match ext {
            "json" => {
                if let Ok(val) = serde_like_json(text, span) {
                    return val;
                }
            }
            "toml" => {
                // Return as string — let the user pipe through `from toml`
            }
            _ => {}
        }
        // Return as string for any valid UTF-8 content
        return Value::string(text, span);
    }

    // Binary fallback
    Value::binary(data, span)
}

/// Minimal JSON → Nushell Value conversion using nushell's own JSON support.
///
/// We do a very thin parse here: the nushell `from json` command is richer,
/// but having *some* auto-detection makes `tramp open` immediately useful.
fn serde_like_json(text: &str, span: Span) -> Result<Value, ()> {
    // Try to parse as a JSON value using a simple recursive descent.
    // For Phase 1, just return the raw string — the user can pipe through
    // `from json`.  A future version could use serde_json.
    let trimmed = text.trim();
    if (trimmed.starts_with('{') && trimmed.ends_with('}'))
        || (trimmed.starts_with('[') && trimmed.ends_with(']'))
    {
        // Looks like JSON — return as string so the user can `from json`
        // This is better than returning binary.
        return Ok(Value::string(text, span));
    }
    Err(())
}

/// Convert a `TrampError` into a `LabeledError` with the call's span.
fn tramp_err(e: TrampError, span: Span) -> LabeledError {
    LabeledError::new(e.to_string()).with_label("tramp error", span)
}

// ---------------------------------------------------------------------------
// `tramp` (main / help)
// ---------------------------------------------------------------------------

struct TrampMain;

impl PluginCommand for TrampMain {
    type Plugin = TrampPlugin;

    fn name(&self) -> &str {
        "tramp"
    }

    fn description(&self) -> &str {
        "TRAMP-style remote filesystem access for Nushell"
    }

    fn extra_description(&self) -> &str {
        r#"
nu-plugin-tramp lets you transparently access remote files using TRAMP-style URIs:

    tramp open /ssh:myvm:/etc/hostname
    tramp ls   /ssh:myvm:/var/log
    tramp save /ssh:myvm:/app/config.toml
    tramp rm   /ssh:myvm:/tmp/stale.lock
    tramp cp   /ssh:myvm:/etc/config ./local-copy

Set a remote working directory to use relative paths:

    tramp cd /ssh:myvm:/app
    tramp open config.toml     # resolves to /ssh:myvm:/app/config.toml
    tramp ls                   # lists /app on myvm

Connection management:

    tramp ping /ssh:myvm:/
    tramp connections
    tramp disconnect myvm

Path format: /<backend>:<user>@<host>#<port>:<remote-path>

Only the SSH backend is supported in Phase 1.
"#
        .trim()
    }

    fn signature(&self) -> Signature {
        Signature::build(self.name()).category(Category::FileSystem)
    }

    fn search_terms(&self) -> Vec<&str> {
        vec!["tramp", "remote", "ssh", "sftp"]
    }

    fn run(
        &self,
        _plugin: &TrampPlugin,
        engine: &EngineInterface,
        _call: &EvaluatedCall,
        _input: PipelineData,
    ) -> Result<PipelineData, LabeledError> {
        Ok(PipelineData::Value(
            Value::string(engine.get_help()?, Span::unknown()),
            None,
        ))
    }
}

// ---------------------------------------------------------------------------
// `tramp open`
// ---------------------------------------------------------------------------

struct TrampOpen;

impl PluginCommand for TrampOpen {
    type Plugin = TrampPlugin;

    fn name(&self) -> &str {
        "tramp open"
    }

    fn description(&self) -> &str {
        "Read a remote file via a TRAMP URI"
    }

    fn extra_description(&self) -> &str {
        "Reads the remote file and returns it as a Nushell value. \
         Text files are returned as strings; binary files as binary data. \
         Pipe through `from json`, `from toml`, etc. for structured parsing."
    }

    fn signature(&self) -> Signature {
        Signature::build(self.name())
            .required("path", SyntaxShape::String, "TRAMP URI of the remote file")
            .input_output_types(vec![(Type::Nothing, Type::Any)])
            .category(Category::FileSystem)
    }

    fn search_terms(&self) -> Vec<&str> {
        vec!["tramp", "open", "read", "remote", "ssh", "sftp"]
    }

    fn examples(&self) -> Vec<Example<'_>> {
        vec![
            Example {
                example: "tramp open /ssh:myvm:/etc/hostname",
                description: "Read a remote text file",
                result: None,
            },
            Example {
                example: "tramp open /ssh:admin@myvm:/app/config.json | from json",
                description: "Read and parse a remote JSON file",
                result: None,
            },
            Example {
                example: "tramp cd /ssh:myvm:/app; tramp open config.toml",
                description: "Read a file relative to the remote CWD",
                result: None,
            },
        ]
    }

    fn run(
        &self,
        plugin: &TrampPlugin,
        _engine: &EngineInterface,
        call: &EvaluatedCall,
        _input: PipelineData,
    ) -> Result<PipelineData, LabeledError> {
        let path = parse_tramp_arg(call, &plugin.remote_cwd)?;
        let data = plugin
            .vfs
            .read(&path)
            .map_err(|e| tramp_err(e, call.head))?;

        let value = bytes_to_value(&data, &path.remote_path, call.head);
        Ok(PipelineData::Value(value, None))
    }
}

// ---------------------------------------------------------------------------
// `tramp ls`
// ---------------------------------------------------------------------------

struct TrampLs;

impl PluginCommand for TrampLs {
    type Plugin = TrampPlugin;

    fn name(&self) -> &str {
        "tramp ls"
    }

    fn description(&self) -> &str {
        "List a remote directory via a TRAMP URI"
    }

    fn signature(&self) -> Signature {
        Signature::build(self.name())
            .optional(
                "path",
                SyntaxShape::String,
                "TRAMP URI of the remote directory (defaults to remote CWD if set)",
            )
            .input_output_types(vec![(Type::Nothing, Type::table())])
            .category(Category::FileSystem)
    }

    fn search_terms(&self) -> Vec<&str> {
        vec!["tramp", "ls", "list", "dir", "remote", "ssh"]
    }

    fn examples(&self) -> Vec<Example<'_>> {
        vec![
            Example {
                example: "tramp ls /ssh:myvm:/var/log",
                description: "List files in a remote directory",
                result: None,
            },
            Example {
                example: "tramp ls /ssh:myvm:/ | where type == dir",
                description: "List only directories at the remote root",
                result: None,
            },
            Example {
                example: "tramp cd /ssh:myvm:/var/log; tramp ls",
                description: "List the current remote working directory",
                result: None,
            },
        ]
    }

    fn run(
        &self,
        plugin: &TrampPlugin,
        _engine: &EngineInterface,
        call: &EvaluatedCall,
        _input: PipelineData,
    ) -> Result<PipelineData, LabeledError> {
        let path = match call.opt::<String>(0)? {
            Some(raw) => resolve_tramp_path(&raw, &plugin.remote_cwd, call.head)?,
            None => {
                // No argument — use the remote CWD if set.
                let cwd_guard = plugin
                    .remote_cwd
                    .lock()
                    .map_err(|e| LabeledError::new(format!("internal lock error: {e}")))?;
                match &*cwd_guard {
                    Some(cwd) => cwd.clone(),
                    None => {
                        return Err(LabeledError::new(
                            "no path provided and no remote working directory set.\n\
                             Use `tramp cd /ssh:host:/dir` first, or pass a TRAMP URI.",
                        )
                        .with_label("missing path", call.head));
                    }
                }
            }
        };

        let entries = plugin
            .vfs
            .list(&path)
            .map_err(|e| tramp_err(e, call.head))?;

        let span = call.head;
        let rows: Vec<Value> = entries
            .into_iter()
            .map(|entry| {
                let kind_str = match entry.kind {
                    backend::EntryKind::File => "file",
                    backend::EntryKind::Dir => "dir",
                    backend::EntryKind::Symlink => "symlink",
                };

                let mut record = Record::new();
                record.push("name", Value::string(&entry.name, span));
                record.push("type", Value::string(kind_str, span));

                if let Some(size) = entry.size {
                    record.push("size", Value::filesize(size as i64, span));
                } else {
                    record.push("size", Value::nothing(span));
                }

                if let Some(modified) = entry.modified {
                    if let Ok(duration) = modified.duration_since(std::time::UNIX_EPOCH) {
                        let nanos = duration.as_nanos() as i64;
                        record.push("modified", Value::date(chrono_from_nanos(nanos), span));
                    } else {
                        record.push("modified", Value::nothing(span));
                    }
                } else {
                    record.push("modified", Value::nothing(span));
                }

                if let Some(perms) = entry.permissions {
                    record.push("mode", Value::string(format!("{perms:o}"), span));
                } else {
                    record.push("mode", Value::nothing(span));
                }

                Value::record(record, span)
            })
            .collect();

        Ok(PipelineData::Value(Value::list(rows, span), None))
    }
}

/// Convert nanoseconds since UNIX epoch to a chrono DateTime suitable for
/// Nushell's `Value::date`.
fn chrono_from_nanos(nanos: i64) -> chrono::DateTime<chrono::FixedOffset> {
    let secs = nanos / 1_000_000_000;
    let nsecs = (nanos % 1_000_000_000) as u32;
    let dt = chrono::DateTime::from_timestamp(secs, nsecs)
        .unwrap_or_else(|| chrono::DateTime::from_timestamp(0, 0).unwrap());
    dt.with_timezone(&chrono::FixedOffset::east_opt(0).unwrap())
}

// ---------------------------------------------------------------------------
// `tramp save`
// ---------------------------------------------------------------------------

struct TrampSave;

impl PluginCommand for TrampSave {
    type Plugin = TrampPlugin;

    fn name(&self) -> &str {
        "tramp save"
    }

    fn description(&self) -> &str {
        "Write piped data to a remote file via a TRAMP URI"
    }

    fn extra_description(&self) -> &str {
        "Accepts string or binary pipeline input and writes it to the remote path. \
         To save structured data, first convert it with `to json`, `to toml`, etc."
    }

    fn signature(&self) -> Signature {
        Signature::build(self.name())
            .required(
                "path",
                SyntaxShape::String,
                "TRAMP URI of the remote file to write",
            )
            .input_output_types(vec![
                (Type::String, Type::Nothing),
                (Type::Binary, Type::Nothing),
                (Type::Any, Type::Nothing),
            ])
            .category(Category::FileSystem)
    }

    fn search_terms(&self) -> Vec<&str> {
        vec!["tramp", "save", "write", "remote", "ssh", "sftp"]
    }

    fn examples(&self) -> Vec<Example<'_>> {
        vec![
            Example {
                example: r#""hello world" | tramp save /ssh:myvm:/tmp/hello.txt"#,
                description: "Write a string to a remote file",
                result: None,
            },
            Example {
                example: "open config.toml | to toml | tramp save /ssh:myvm:/app/config.toml",
                description: "Save a local config file to a remote host",
                result: None,
            },
        ]
    }

    fn run(
        &self,
        plugin: &TrampPlugin,
        _engine: &EngineInterface,
        call: &EvaluatedCall,
        input: PipelineData,
    ) -> Result<PipelineData, LabeledError> {
        let path = parse_tramp_arg(call, &plugin.remote_cwd)?;

        // Collect the pipeline input into bytes.
        let data = pipeline_to_bytes(input)?;

        plugin
            .vfs
            .write(&path, data)
            .map_err(|e| tramp_err(e, call.head))?;

        Ok(PipelineData::Empty)
    }
}

/// Collect pipeline data into a `Bytes` value.
fn pipeline_to_bytes(input: PipelineData) -> Result<bytes::Bytes, LabeledError> {
    match input {
        PipelineData::Value(value, _) => match value {
            Value::String { val, .. } => Ok(bytes::Bytes::from(val.into_bytes())),
            Value::Binary { val, .. } => Ok(bytes::Bytes::from(val)),
            Value::Nothing { .. } => Ok(bytes::Bytes::new()),
            other => {
                // Convert arbitrary values to their display string.
                let s = other.to_expanded_string(", ", &nu_protocol::Config::default());
                Ok(bytes::Bytes::from(s.into_bytes()))
            }
        },
        PipelineData::ListStream(stream, _) => {
            let mut buf = Vec::new();
            for value in stream {
                match value {
                    Value::String { val, .. } => buf.extend_from_slice(val.as_bytes()),
                    Value::Binary { val, .. } => buf.extend_from_slice(&val),
                    other => {
                        let s = other.to_expanded_string(", ", &nu_protocol::Config::default());
                        buf.extend_from_slice(s.as_bytes());
                    }
                }
            }
            Ok(bytes::Bytes::from(buf))
        }
        PipelineData::ByteStream(stream, _) => {
            let collected = stream
                .into_bytes()
                .map_err(|e| LabeledError::new(e.to_string()))?;
            Ok(bytes::Bytes::from(collected))
        }
        PipelineData::Empty => Ok(bytes::Bytes::new()),
    }
}

// ---------------------------------------------------------------------------
// `tramp rm`
// ---------------------------------------------------------------------------

struct TrampRm;

impl PluginCommand for TrampRm {
    type Plugin = TrampPlugin;

    fn name(&self) -> &str {
        "tramp rm"
    }

    fn description(&self) -> &str {
        "Delete a remote file via a TRAMP URI"
    }

    fn signature(&self) -> Signature {
        Signature::build(self.name())
            .required(
                "path",
                SyntaxShape::String,
                "TRAMP URI of the remote file to delete",
            )
            .input_output_types(vec![(Type::Nothing, Type::Nothing)])
            .category(Category::FileSystem)
    }

    fn search_terms(&self) -> Vec<&str> {
        vec!["tramp", "rm", "remove", "delete", "remote", "ssh"]
    }

    fn examples(&self) -> Vec<Example<'_>> {
        vec![Example {
            example: "tramp rm /ssh:myvm:/tmp/stale.lock",
            description: "Delete a remote file",
            result: None,
        }]
    }

    fn run(
        &self,
        plugin: &TrampPlugin,
        _engine: &EngineInterface,
        call: &EvaluatedCall,
        _input: PipelineData,
    ) -> Result<PipelineData, LabeledError> {
        let path = parse_tramp_arg(call, &plugin.remote_cwd)?;

        plugin
            .vfs
            .delete(&path)
            .map_err(|e| tramp_err(e, call.head))?;

        Ok(PipelineData::Empty)
    }
}

// ---------------------------------------------------------------------------
// `tramp cp`
// ---------------------------------------------------------------------------

struct TrampCp;

impl PluginCommand for TrampCp {
    type Plugin = TrampPlugin;

    fn name(&self) -> &str {
        "tramp cp"
    }

    fn description(&self) -> &str {
        "Copy a file between remote hosts or between local and remote"
    }

    fn extra_description(&self) -> &str {
        "Copies a file from source to destination. Both, one, or neither \
         may be TRAMP URIs. When a path is not a TRAMP URI, it is treated \
         as a local file path.\n\n\
         Examples:\n\
         - Remote → Remote: tramp cp /ssh:vm1:/file /ssh:vm2:/file\n\
         - Remote → Local:  tramp cp /ssh:vm:/file ./local-copy\n\
         - Local → Remote:  tramp cp ./local-file /ssh:vm:/file"
    }

    fn signature(&self) -> Signature {
        Signature::build(self.name())
            .required(
                "source",
                SyntaxShape::String,
                "Source path (local or TRAMP URI)",
            )
            .required(
                "destination",
                SyntaxShape::String,
                "Destination path (local or TRAMP URI)",
            )
            .input_output_types(vec![(Type::Nothing, Type::Nothing)])
            .category(Category::FileSystem)
    }

    fn search_terms(&self) -> Vec<&str> {
        vec!["tramp", "cp", "copy", "remote", "ssh", "transfer"]
    }

    fn examples(&self) -> Vec<Example<'_>> {
        vec![
            Example {
                example: "tramp cp /ssh:vm1:/etc/config /ssh:vm2:/etc/config",
                description: "Copy a file between two remote hosts",
                result: None,
            },
            Example {
                example: "tramp cp /ssh:myvm:/etc/hostname ./hostname",
                description: "Copy a remote file to local",
                result: None,
            },
            Example {
                example: "tramp cp ./config.toml /ssh:myvm:/app/config.toml",
                description: "Copy a local file to remote",
                result: None,
            },
        ]
    }

    fn run(
        &self,
        plugin: &TrampPlugin,
        _engine: &EngineInterface,
        call: &EvaluatedCall,
        _input: PipelineData,
    ) -> Result<PipelineData, LabeledError> {
        let src_raw: String = call.req(0)?;
        let dst_raw: String = call.req(1)?;

        let src_tramp = protocol::parse(&src_raw).map_err(|e| {
            LabeledError::new(e.to_string()).with_label("source parse error", call.head)
        })?;
        let dst_tramp = protocol::parse(&dst_raw).map_err(|e| {
            LabeledError::new(e.to_string()).with_label("destination parse error", call.head)
        })?;

        // Also try relative resolution for source and destination.
        let src_tramp = match src_tramp {
            Some(p) => Some(p),
            None => resolve_tramp_path(&src_raw, &plugin.remote_cwd, call.head).ok(),
        };
        let dst_tramp = match dst_tramp {
            Some(p) => Some(p),
            None => resolve_tramp_path(&dst_raw, &plugin.remote_cwd, call.head).ok(),
        };

        // Read the source data.
        let data: bytes::Bytes = match src_tramp {
            Some(ref src_path) => plugin
                .vfs
                .read(src_path)
                .map_err(|e| tramp_err(e, call.head))?,
            None => {
                // Local file read.
                let contents = std::fs::read(&src_raw).map_err(|e| {
                    LabeledError::new(format!("failed to read local file '{}': {}", src_raw, e))
                        .with_label("local read error", call.head)
                })?;
                bytes::Bytes::from(contents)
            }
        };

        // Write the destination data.
        match dst_tramp {
            Some(ref dst_path) => {
                plugin
                    .vfs
                    .write(dst_path, data)
                    .map_err(|e| tramp_err(e, call.head))?;
            }
            None => {
                // Local file write.
                std::fs::write(&dst_raw, &data).map_err(|e| {
                    LabeledError::new(format!("failed to write local file '{}': {}", dst_raw, e))
                        .with_label("local write error", call.head)
                })?;
            }
        }

        Ok(PipelineData::Empty)
    }
}

// ---------------------------------------------------------------------------
// `tramp cd`
// ---------------------------------------------------------------------------

struct TrampCd;

impl PluginCommand for TrampCd {
    type Plugin = TrampPlugin;

    fn name(&self) -> &str {
        "tramp cd"
    }

    fn description(&self) -> &str {
        "Set the remote working directory for subsequent tramp commands"
    }

    fn extra_description(&self) -> &str {
        "Sets a remote CWD so that subsequent tramp commands can accept \
         relative paths. The path must be a TRAMP URI pointing to a \
         directory (or use a relative path if a remote CWD is already set).\n\n\
         Use `tramp cd --reset` to clear the remote CWD.\n\
         Use `tramp pwd` to see the current remote CWD."
    }

    fn signature(&self) -> Signature {
        Signature::build(self.name())
            .optional(
                "path",
                SyntaxShape::String,
                "TRAMP URI or relative path to set as the remote working directory",
            )
            .switch("reset", "Clear the remote working directory", Some('r'))
            .input_output_types(vec![(Type::Nothing, Type::String)])
            .category(Category::FileSystem)
    }

    fn search_terms(&self) -> Vec<&str> {
        vec!["tramp", "cd", "directory", "remote", "ssh"]
    }

    fn examples(&self) -> Vec<Example<'_>> {
        vec![
            Example {
                example: "tramp cd /ssh:myvm:/app",
                description: "Set the remote CWD to /app on myvm",
                result: None,
            },
            Example {
                example: "tramp cd subdir",
                description: "Navigate to a subdirectory relative to the current remote CWD",
                result: None,
            },
            Example {
                example: "tramp cd ..",
                description: "Go up one directory",
                result: None,
            },
            Example {
                example: "tramp cd --reset",
                description: "Clear the remote working directory",
                result: None,
            },
        ]
    }

    fn run(
        &self,
        plugin: &TrampPlugin,
        _engine: &EngineInterface,
        call: &EvaluatedCall,
        _input: PipelineData,
    ) -> Result<PipelineData, LabeledError> {
        // Handle --reset flag.
        if call.has_flag("reset")? {
            let mut cwd_guard = plugin
                .remote_cwd
                .lock()
                .map_err(|e| LabeledError::new(format!("internal lock error: {e}")))?;
            *cwd_guard = None;
            return Ok(PipelineData::Value(
                Value::string("remote CWD cleared", call.head),
                None,
            ));
        }

        let raw: Option<String> = call.opt(0)?;
        let raw = match raw {
            Some(r) => r,
            None => {
                // No argument, no reset — show the current CWD.
                let cwd_guard = plugin
                    .remote_cwd
                    .lock()
                    .map_err(|e| LabeledError::new(format!("internal lock error: {e}")))?;
                return match &*cwd_guard {
                    Some(cwd) => Ok(PipelineData::Value(
                        Value::string(cwd.to_string(), call.head),
                        None,
                    )),
                    None => Ok(PipelineData::Value(
                        Value::string("(no remote CWD set)", call.head),
                        None,
                    )),
                };
            }
        };

        // Resolve the target path (may be absolute TRAMP URI or relative).
        let target = resolve_tramp_path(&raw, &plugin.remote_cwd, call.head)?;

        // Verify the path exists and is a directory.
        let meta = plugin
            .vfs
            .stat(&target)
            .map_err(|e| tramp_err(e, call.head))?;

        if meta.kind != backend::EntryKind::Dir {
            return Err(
                LabeledError::new(format!("not a directory: {}", target.remote_path))
                    .with_label("expected a directory", call.head),
            );
        }

        let display = target.to_string();

        // Store the new CWD.
        {
            let mut cwd_guard = plugin
                .remote_cwd
                .lock()
                .map_err(|e| LabeledError::new(format!("internal lock error: {e}")))?;
            *cwd_guard = Some(target);
        }

        Ok(PipelineData::Value(Value::string(display, call.head), None))
    }
}

// ---------------------------------------------------------------------------
// `tramp pwd`
// ---------------------------------------------------------------------------

struct TrampPwd;

impl PluginCommand for TrampPwd {
    type Plugin = TrampPlugin;

    fn name(&self) -> &str {
        "tramp pwd"
    }

    fn description(&self) -> &str {
        "Show the current remote working directory"
    }

    fn signature(&self) -> Signature {
        Signature::build(self.name())
            .input_output_types(vec![(Type::Nothing, Type::String)])
            .category(Category::FileSystem)
    }

    fn search_terms(&self) -> Vec<&str> {
        vec!["tramp", "pwd", "directory", "remote"]
    }

    fn examples(&self) -> Vec<Example<'_>> {
        vec![Example {
            example: "tramp pwd",
            description: "Show the current remote working directory",
            result: None,
        }]
    }

    fn run(
        &self,
        plugin: &TrampPlugin,
        _engine: &EngineInterface,
        call: &EvaluatedCall,
        _input: PipelineData,
    ) -> Result<PipelineData, LabeledError> {
        let cwd_guard = plugin
            .remote_cwd
            .lock()
            .map_err(|e| LabeledError::new(format!("internal lock error: {e}")))?;

        match &*cwd_guard {
            Some(cwd) => Ok(PipelineData::Value(
                Value::string(cwd.to_string(), call.head),
                None,
            )),
            None => Ok(PipelineData::Value(
                Value::string("(no remote CWD set)", call.head),
                None,
            )),
        }
    }
}

// ---------------------------------------------------------------------------
// `tramp ping`
// ---------------------------------------------------------------------------

struct TrampPing;

impl PluginCommand for TrampPing {
    type Plugin = TrampPlugin;

    fn name(&self) -> &str {
        "tramp ping"
    }

    fn description(&self) -> &str {
        "Test connectivity to a remote host via a TRAMP URI"
    }

    fn extra_description(&self) -> &str {
        "Opens an SSH connection (or reuses a pooled one) and runs a \
         health-check command (`true`) on the remote. Reports success \
         or failure with timing information."
    }

    fn signature(&self) -> Signature {
        Signature::build(self.name())
            .required(
                "path",
                SyntaxShape::String,
                "TRAMP URI to test (e.g. /ssh:myvm:/)",
            )
            .input_output_types(vec![(Type::Nothing, Type::record())])
            .category(Category::FileSystem)
    }

    fn search_terms(&self) -> Vec<&str> {
        vec!["tramp", "ping", "test", "connection", "ssh"]
    }

    fn examples(&self) -> Vec<Example<'_>> {
        vec![Example {
            example: "tramp ping /ssh:myvm:/",
            description: "Test SSH connectivity to myvm",
            result: None,
        }]
    }

    fn run(
        &self,
        plugin: &TrampPlugin,
        _engine: &EngineInterface,
        call: &EvaluatedCall,
        _input: PipelineData,
    ) -> Result<PipelineData, LabeledError> {
        let path = parse_tramp_arg(call, &plugin.remote_cwd)?;
        let span = call.head;

        let start = std::time::Instant::now();
        let result = plugin.vfs.ping(&path);
        let elapsed = start.elapsed();

        let mut record = Record::new();
        record.push("host", Value::string(&path.hops[0].host, span));
        record.push(
            "backend",
            Value::string(path.hops[0].backend.to_string(), span),
        );

        match result {
            Ok(()) => {
                record.push("status", Value::string("ok", span));
                record.push(
                    "time_ms",
                    Value::float(elapsed.as_secs_f64() * 1000.0, span),
                );
                record.push("error", Value::nothing(span));
            }
            Err(ref e) => {
                record.push("status", Value::string("error", span));
                record.push(
                    "time_ms",
                    Value::float(elapsed.as_secs_f64() * 1000.0, span),
                );
                record.push("error", Value::string(e.to_string(), span));
            }
        }

        Ok(PipelineData::Value(Value::record(record, span), None))
    }
}

// ---------------------------------------------------------------------------
// `tramp connections`
// ---------------------------------------------------------------------------

struct TrampConnections;

impl PluginCommand for TrampConnections {
    type Plugin = TrampPlugin;

    fn name(&self) -> &str {
        "tramp connections"
    }

    fn description(&self) -> &str {
        "List active remote connections"
    }

    fn signature(&self) -> Signature {
        Signature::build(self.name())
            .input_output_types(vec![(Type::Nothing, Type::table())])
            .category(Category::FileSystem)
    }

    fn search_terms(&self) -> Vec<&str> {
        vec!["tramp", "connections", "pool", "ssh", "status"]
    }

    fn examples(&self) -> Vec<Example<'_>> {
        vec![Example {
            example: "tramp connections",
            description: "Show all active remote connections",
            result: None,
        }]
    }

    fn run(
        &self,
        plugin: &TrampPlugin,
        _engine: &EngineInterface,
        call: &EvaluatedCall,
        _input: PipelineData,
    ) -> Result<PipelineData, LabeledError> {
        let span = call.head;
        let connections = plugin.vfs.active_connections_detailed();

        if connections.is_empty() {
            return Ok(PipelineData::Value(Value::list(vec![], span), None));
        }

        let rows: Vec<Value> = connections
            .into_iter()
            .map(|info| {
                let mut record = Record::new();
                record.push("backend", Value::string(&info.backend, span));
                record.push(
                    "user",
                    info.user
                        .as_deref()
                        .map(|u| Value::string(u, span))
                        .unwrap_or_else(|| Value::nothing(span)),
                );
                record.push("host", Value::string(&info.host, span));
                record.push(
                    "port",
                    info.port
                        .map(|p| Value::int(p as i64, span))
                        .unwrap_or_else(|| Value::nothing(span)),
                );
                Value::record(record, span)
            })
            .collect();

        Ok(PipelineData::Value(Value::list(rows, span), None))
    }
}

// ---------------------------------------------------------------------------
// `tramp disconnect`
// ---------------------------------------------------------------------------

struct TrampDisconnect;

impl PluginCommand for TrampDisconnect {
    type Plugin = TrampPlugin;

    fn name(&self) -> &str {
        "tramp disconnect"
    }

    fn description(&self) -> &str {
        "Close remote connections"
    }

    fn extra_description(&self) -> &str {
        "With a host argument, disconnects only sessions to that host. \
         With --all, disconnects every active connection. \
         Also clears any cached metadata for the affected hosts."
    }

    fn signature(&self) -> Signature {
        Signature::build(self.name())
            .optional(
                "host",
                SyntaxShape::String,
                "Hostname to disconnect (e.g. 'myvm')",
            )
            .switch("all", "Disconnect all active connections", Some('a'))
            .input_output_types(vec![(Type::Nothing, Type::String)])
            .category(Category::FileSystem)
    }

    fn search_terms(&self) -> Vec<&str> {
        vec!["tramp", "disconnect", "close", "ssh"]
    }

    fn examples(&self) -> Vec<Example<'_>> {
        vec![
            Example {
                example: "tramp disconnect myvm",
                description: "Close all connections to myvm",
                result: None,
            },
            Example {
                example: "tramp disconnect --all",
                description: "Close all remote connections",
                result: None,
            },
        ]
    }

    fn run(
        &self,
        plugin: &TrampPlugin,
        _engine: &EngineInterface,
        call: &EvaluatedCall,
        _input: PipelineData,
    ) -> Result<PipelineData, LabeledError> {
        let span = call.head;

        if call.has_flag("all")? {
            let count = plugin.vfs.connection_count();
            plugin.vfs.disconnect_all();

            // Also clear the remote CWD since all connections are gone.
            if let Ok(mut cwd) = plugin.remote_cwd.lock() {
                *cwd = None;
            }

            return Ok(PipelineData::Value(
                Value::string(format!("disconnected {} connection(s)", count), span),
                None,
            ));
        }

        let host: Option<String> = call.opt(0)?;
        match host {
            Some(host) => {
                plugin.vfs.disconnect_host(&host);

                // If the remote CWD was on this host, clear it.
                if let Ok(mut cwd_guard) = plugin.remote_cwd.lock() {
                    let should_clear = cwd_guard
                        .as_ref()
                        .is_some_and(|cwd| cwd.hops.iter().any(|h| h.host == host));
                    if should_clear {
                        *cwd_guard = None;
                    }
                }

                Ok(PipelineData::Value(
                    Value::string(format!("disconnected from {host}"), span),
                    None,
                ))
            }
            None => Err(LabeledError::new(
                "provide a hostname to disconnect, or use --all to disconnect everything",
            )
            .with_label("missing argument", span)),
        }
    }
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

fn main() {
    serve_plugin(&TrampPlugin::new(), MsgPackSerializer {})
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_relative_simple() {
        assert_eq!(resolve_relative("/app", "config.toml"), "/app/config.toml");
    }

    #[test]
    fn resolve_relative_subdir() {
        assert_eq!(
            resolve_relative("/app", "sub/file.txt"),
            "/app/sub/file.txt"
        );
    }

    #[test]
    fn resolve_relative_dotdot() {
        assert_eq!(resolve_relative("/app/sub", ".."), "/app");
    }

    #[test]
    fn resolve_relative_dotdot_and_file() {
        assert_eq!(
            resolve_relative("/app/sub", "../other/file.txt"),
            "/app/other/file.txt"
        );
    }

    #[test]
    fn resolve_relative_dot() {
        assert_eq!(
            resolve_relative("/app", "./config.toml"),
            "/app/config.toml"
        );
    }

    #[test]
    fn resolve_relative_absolute_overrides() {
        assert_eq!(resolve_relative("/app", "/etc/config"), "/etc/config");
    }

    #[test]
    fn resolve_relative_to_root() {
        assert_eq!(resolve_relative("/app", ".."), "/");
    }

    #[test]
    fn resolve_relative_past_root() {
        assert_eq!(resolve_relative("/", ".."), "/");
    }

    #[test]
    fn resolve_relative_trailing_slash_base() {
        assert_eq!(resolve_relative("/app/", "config.toml"), "/app/config.toml");
    }

    #[test]
    fn normalize_path_dots() {
        assert_eq!(normalize_path("/app/./sub/../file"), "/app/file");
    }

    #[test]
    fn normalize_path_root() {
        assert_eq!(normalize_path("/"), "/");
    }

    #[test]
    fn normalize_path_clean() {
        assert_eq!(normalize_path("/a/b/c"), "/a/b/c");
    }
}
