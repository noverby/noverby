//! `nu_plugin_tramp` — A TRAMP-inspired remote filesystem plugin for Nushell.
//!
//! This is the plugin entry point.  It registers the following commands:
//!
//! - `tramp open <path>`  — read a remote file and return it as a Nushell value
//! - `tramp ls <path>`    — list a remote directory as a Nushell table
//! - `tramp save <path>`  — write piped data to a remote file
//! - `tramp rm <path>`    — delete a remote file
//!
//! Paths use the TRAMP URI format: `/ssh:user@host#port:/remote/path`

mod backend;
mod errors;
mod protocol;
mod vfs;

use std::sync::Arc;

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
}

impl TrampPlugin {
    fn new() -> Self {
        Self {
            vfs: Arc::new(Vfs::new().expect("failed to initialise tramp VFS")),
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
        ]
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Parse a string argument as a TRAMP path, returning a nice LabeledError on
/// failure.
fn parse_tramp_arg(call: &EvaluatedCall) -> Result<TrampPath, LabeledError> {
    let raw: String = call.req(0)?;
    match protocol::parse(&raw) {
        Ok(Some(path)) => Ok(path),
        Ok(None) => Err(LabeledError::new(
            "not a tramp path — expected format: /ssh:host:/remote/path",
        )
        .with_label("this is not a TRAMP URI", call.head)),
        Err(e) => Err(LabeledError::new(e.to_string()).with_label("parse error", call.head)),
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
tramp-rs lets you transparently access remote files using TRAMP-style URIs:

    tramp open /ssh:myvm:/etc/hostname
    tramp ls   /ssh:myvm:/var/log
    tramp save /ssh:myvm:/app/config.toml
    tramp rm   /ssh:myvm:/tmp/stale.lock

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
        ]
    }

    fn run(
        &self,
        plugin: &TrampPlugin,
        _engine: &EngineInterface,
        call: &EvaluatedCall,
        _input: PipelineData,
    ) -> Result<PipelineData, LabeledError> {
        let path = parse_tramp_arg(call)?;
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
            .required(
                "path",
                SyntaxShape::String,
                "TRAMP URI of the remote directory",
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
        ]
    }

    fn run(
        &self,
        plugin: &TrampPlugin,
        _engine: &EngineInterface,
        call: &EvaluatedCall,
        _input: PipelineData,
    ) -> Result<PipelineData, LabeledError> {
        let path = parse_tramp_arg(call)?;
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
        let path = parse_tramp_arg(call)?;

        // Collect the pipeline input into bytes.
        let data = match input {
            PipelineData::Value(value, _) => match value {
                Value::String { val, .. } => bytes::Bytes::from(val.into_bytes()),
                Value::Binary { val, .. } => bytes::Bytes::from(val),
                Value::Nothing { .. } => bytes::Bytes::new(),
                other => {
                    // Convert arbitrary values to their display string.
                    let s = other.to_expanded_string(", ", &nu_protocol::Config::default());
                    bytes::Bytes::from(s.into_bytes())
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
                bytes::Bytes::from(buf)
            }
            PipelineData::ByteStream(stream, _) => {
                let collected = stream
                    .into_bytes()
                    .map_err(|e| LabeledError::new(e.to_string()))?;
                bytes::Bytes::from(collected)
            }
            PipelineData::Empty => bytes::Bytes::new(),
        };

        plugin
            .vfs
            .write(&path, data)
            .map_err(|e| tramp_err(e, call.head))?;

        Ok(PipelineData::Empty)
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
        let path = parse_tramp_arg(call)?;

        plugin
            .vfs
            .delete(&path)
            .map_err(|e| tramp_err(e, call.head))?;

        Ok(PipelineData::Empty)
    }
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

fn main() {
    serve_plugin(&TrampPlugin::new(), MsgPackSerializer {})
}
