//! SSH backend implementation.
//!
//! Uses the [`openssh`] crate (which shells out to the system's OpenSSH
//! binary) for session management.  All file operations are implemented
//! via remote command execution (`cat`, `stat`, `ls`, `rm`, etc.).
//!
//! This gives us:
//!
//! - Full `~/.ssh/config` support
//! - SSH agent forwarding
//! - `ControlMaster` multiplexing (fast subsequent operations)
//! - Key management delegated entirely to the user's existing setup
//! - Works on any remote with a POSIX shell (no SFTP subsystem required)
//!
//! Phase 2 will add an optional SFTP fast-path for large/binary file
//! transfers via `openssh-sftp-client`.

use async_trait::async_trait;
use bytes::Bytes;
use openssh::{KnownHosts, Session, SessionBuilder};
use std::path::Path;
use std::time::{Duration, UNIX_EPOCH};

use super::{Backend, DirEntry, EntryKind, ExecResult, Metadata};
use crate::errors::{TrampError, TrampResult};

// ---------------------------------------------------------------------------
// SSH backend
// ---------------------------------------------------------------------------

/// An SSH backend backed by a live [`openssh::Session`].
///
/// All file-system operations are implemented by executing remote commands
/// over the SSH session.
pub struct SshBackend {
    session: Session,
    host: String,
}

impl SshBackend {
    /// Open a new SSH connection to `host`, optionally as `user` and/or on a
    /// non-default `port`.
    pub async fn connect(host: &str, user: Option<&str>, port: Option<u16>) -> TrampResult<Self> {
        let mut builder = SessionBuilder::default();
        builder.known_hosts_check(KnownHosts::Accept);

        if let Some(user) = user {
            builder.user(user.to_string());
        }
        if let Some(port) = port {
            builder.port(port);
        }

        let session = builder
            .connect(host)
            .await
            .map_err(|e| TrampError::ConnectionFailed {
                host: host.to_string(),
                reason: e.to_string(),
            })?;

        Ok(Self {
            session,
            host: host.to_string(),
        })
    }

    /// Run a command via the SSH session and return its collected output.
    async fn run(&self, program: &str, args: &[&str]) -> TrampResult<ExecResult> {
        let mut cmd = self.session.command(program);
        for arg in args {
            cmd.arg(arg);
        }
        let output = cmd
            .output()
            .await
            .map_err(|e| TrampError::from_ssh(&self.host, e))?;

        Ok(ExecResult {
            stdout: Bytes::from(output.stdout),
            stderr: Bytes::from(output.stderr),
            exit_code: output.status.code().unwrap_or(-1),
        })
    }

    /// Run a shell snippet (`sh -c '<script>'`) and return its output.
    async fn run_sh(&self, script: &str) -> TrampResult<ExecResult> {
        self.run("sh", &["-c", script]).await
    }

    /// Check the exit code / stderr of a command result and turn failures
    /// into an appropriate `TrampError`.
    fn check_result(result: &ExecResult, path: &str, host: &str) -> TrampResult<()> {
        if result.exit_code == 0 {
            return Ok(());
        }
        let stderr = String::from_utf8_lossy(&result.stderr);
        let msg = stderr.trim();

        if msg.contains("No such file")
            || msg.contains("cannot access")
            || msg.contains("not found")
        {
            Err(TrampError::NotFound(path.to_string()))
        } else if msg.contains("Permission denied") || msg.contains("permission denied") {
            Err(TrampError::PermissionDenied(path.to_string()))
        } else if msg.is_empty() {
            Err(TrampError::RemoteError(format!(
                "command failed with exit code {} for path: {path}",
                result.exit_code
            )))
        } else {
            Err(TrampError::from_ssh(host, msg))
        }
    }
}

// ---------------------------------------------------------------------------
// Backend trait implementation
// ---------------------------------------------------------------------------

#[async_trait]
impl Backend for SshBackend {
    async fn read(&self, path: &str) -> TrampResult<Bytes> {
        let escaped = shell_escape(path);
        let result = self.run_sh(&format!("cat {escaped}")).await?;
        Self::check_result(&result, path, &self.host)?;
        Ok(result.stdout)
    }

    async fn write(&self, path: &str, data: Bytes) -> TrampResult<()> {
        // We use `base64` encoding to safely transport arbitrary binary data
        // through the shell without corruption from special characters / NUL
        // bytes.  The remote side decodes and writes the file.
        //
        // For small-to-medium files (< ~10 MB) this is perfectly fine.
        // Phase 2 can add SFTP streaming for large files.
        use base64::Engine;
        let encoded = base64::engine::general_purpose::STANDARD.encode(&data);
        let escaped = shell_escape(path);

        // First check if the remote has base64(1).  GNU coreutils and busybox
        // both ship it.  If not, fall back to a simpler (but text-only) path.
        let check = self
            .run_sh("command -v base64 >/dev/null 2>&1 && echo yes || echo no")
            .await?;
        let has_base64 = String::from_utf8_lossy(&check.stdout)
            .trim()
            .eq_ignore_ascii_case("yes");

        if has_base64 {
            // Pipe the base64 blob via stdin → base64 -d → file.
            // We split into a heredoc to avoid argument-length limits.
            let script =
                format!("base64 -d > {escaped} <<'__TRAMP_EOF__'\n{encoded}\n__TRAMP_EOF__");
            let result = self.run_sh(&script).await?;
            Self::check_result(&result, path, &self.host)?;
        } else {
            // Fallback: plain printf.  This only works for text content
            // (no NUL bytes) but is better than nothing.
            let text = String::from_utf8(data.to_vec()).map_err(|_| {
                TrampError::RemoteError(
                    "remote host lacks base64(1) and file contains binary data".to_string(),
                )
            })?;
            let escaped_content = text.replace('\\', "\\\\").replace('\'', "'\\''");
            let script = format!("printf '%s' '{escaped_content}' > {escaped}");
            let result = self.run_sh(&script).await?;
            Self::check_result(&result, path, &self.host)?;
        }

        Ok(())
    }

    async fn list(&self, path: &str) -> TrampResult<Vec<DirEntry>> {
        let escaped = shell_escape(path.trim_end_matches('/'));

        // Use GNU stat to get structured output for each entry.
        // Format: %n\t%F\t%s\t%Y\t%a
        //   %n = filename, %F = file type string, %s = size,
        //   %Y = mtime (epoch seconds), %a = octal permissions
        let script = format!(
            r#"for f in {escaped}/* {escaped}/.*; do
  case "$(basename "$f")" in .|..) continue;; esac
  [ -e "$f" ] || [ -L "$f" ] || continue
  stat --format='%n\t%F\t%s\t%Y\t%a' "$f" 2>/dev/null
done"#
        );

        let result = self.run_sh(&script).await?;

        // A non-zero exit code with empty stdout usually means the directory
        // doesn't exist or is empty.
        if result.exit_code != 0 && result.stdout.is_empty() {
            let stderr = String::from_utf8_lossy(&result.stderr);
            if stderr.contains("No such file")
                || stderr.contains("cannot access")
                || stderr.contains("not a directory")
            {
                return Err(TrampError::NotFound(path.to_string()));
            }
            // Empty directory — return empty vec.
        }

        let stdout = String::from_utf8_lossy(&result.stdout);
        let mut entries = Vec::new();

        for line in stdout.lines() {
            if line.is_empty() {
                continue;
            }
            let parts: Vec<&str> = line.splitn(5, '\t').collect();
            if parts.len() < 5 {
                continue;
            }

            let full_name = parts[0];
            let name = Path::new(full_name)
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| full_name.to_string());

            let kind = parse_file_type(parts[1]);
            let size = parts[2].parse::<u64>().ok();
            let modified = parts[3]
                .parse::<u64>()
                .ok()
                .map(|secs| UNIX_EPOCH + Duration::from_secs(secs));
            let permissions = parts[4].parse::<u32>().ok();

            entries.push(DirEntry {
                name,
                kind,
                size,
                modified,
                permissions,
            });
        }

        Ok(entries)
    }

    async fn stat(&self, path: &str) -> TrampResult<Metadata> {
        let escaped = shell_escape(path);
        let script = format!("stat --format='%F\\t%s\\t%Y\\t%a' {escaped}");
        let result = self.run_sh(&script).await?;
        Self::check_result(&result, path, &self.host)?;

        let stdout = String::from_utf8_lossy(&result.stdout);
        let line = stdout.trim();
        let parts: Vec<&str> = line.splitn(4, '\t').collect();
        if parts.len() < 4 {
            return Err(TrampError::RemoteError(format!(
                "unexpected stat output: {line}"
            )));
        }

        let kind = parse_file_type(parts[0]);
        let size = parts[1].parse::<u64>().unwrap_or(0);
        let modified = parts[2]
            .parse::<u64>()
            .ok()
            .map(|secs| UNIX_EPOCH + Duration::from_secs(secs));
        let permissions = parts[3].parse::<u32>().ok();

        Ok(Metadata {
            kind,
            size,
            modified,
            permissions,
        })
    }

    async fn exec(&self, cmd: &str, args: &[&str]) -> TrampResult<ExecResult> {
        self.run(cmd, args).await
    }

    async fn delete(&self, path: &str) -> TrampResult<()> {
        let escaped = shell_escape(path);
        let result = self.run_sh(&format!("rm -f {escaped}")).await?;
        Self::check_result(&result, path, &self.host)?;
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Parse GNU stat's `%F` output into an [`EntryKind`].
fn parse_file_type(type_str: &str) -> EntryKind {
    let s = type_str.to_ascii_lowercase();
    if s.contains("directory") {
        EntryKind::Dir
    } else if s.contains("symbolic link") || s.contains("symlink") {
        EntryKind::Symlink
    } else {
        EntryKind::File
    }
}

/// Shell-escape a string for safe embedding in `sh -c '…'` commands.
fn shell_escape(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

impl Drop for SshBackend {
    fn drop(&mut self) {
        // `Session::close` is async; we can't await it here.
        // The `openssh` crate cleans up the ControlMaster socket when the
        // `Session` is dropped, so this is safe to leave as a no-op.
    }
}
