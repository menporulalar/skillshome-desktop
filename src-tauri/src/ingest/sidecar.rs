//! Module 4 task 4.12 — the first real Rust→sidecar process spawn, deferred three
//! times already (tasks 4.9, 4.10, 4.11) waiting for a genuine UI trigger.
//!
//! **Dev-mode only** (explicit, approved scope): spawns `npm run <script>` against
//! the sidecar's checked-out source directory via `CARGO_MANIFEST_DIR` — this
//! assumes Node/npm installed and the full repo checked out, true for a developer
//! running `cargo tauri dev`, not for a hypothetical installed end-user bundle.
//! Real production packaging (bundling the sidecar as a standalone binary via
//! Tauri's `externalBin`) is a separate, larger follow-up, not attempted here.
//!
//! The sidecar's own stdout is NOT clean single-blob JSON — `@menporulalar/agents-core`'s
//! logger writes its own info-level JSON lines to stdout (a separate published
//! dependency, not something to patch), interleaved with each script's own
//! progress messages. Both `sidecar/src/run-local-extraction-and-stage.ts` and
//! `sidecar/src/confirm-staged-ingestion.ts` route their own messages to stderr and
//! end with exactly one `__SIDECAR_RESULT__:{...json...}` line on stdout — this
//! module scans for that line and ignores everything else, a standard pattern for
//! mixed human+machine CLI output (same idea as `npm --json`/`terraform -json`).

use std::path::PathBuf;
use std::process::Stdio;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::process::Command as TokioCommand;

const RESULT_MARKER: &str = "__SIDECAR_RESULT__:";

/// `CARGO_MANIFEST_DIR` is a compile-time constant Cargo always sets to
/// `src-tauri`'s own directory — `sidecar/` is a real sibling directory in this
/// same repo, so this needs zero configuration. Canonicalized purely for a
/// clearer error message if it's ever missing (e.g. a shallow checkout), not
/// required for correctness.
fn sidecar_dir() -> Result<PathBuf, String> {
    let raw = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../sidecar"));
    std::fs::canonicalize(&raw).map_err(|e| format!("sidecar directory not found at {}: {e}", raw.display()))
}

/// Parses the last `__SIDECAR_RESULT__:` line out of the sidecar's captured
/// stdout. Pure/no I/O — the genuinely unit-testable piece of this module: real
/// process-spawning behavior is covered by live verification instead (mirroring
/// tasks 4.10/4.11's precedent — mocking a child process's I/O is low-value and
/// brittle compared to running the real thing once).
fn parse_marker_line(stdout: &[u8]) -> Result<serde_json::Value, String> {
    let text = String::from_utf8_lossy(stdout);
    let marker_line = text
        .lines()
        .rev()
        .find(|line| line.starts_with(RESULT_MARKER))
        .ok_or_else(|| "sidecar produced no result line — see terminal output for details".to_string())?;

    let json_str = &marker_line[RESULT_MARKER.len()..];
    let parsed: serde_json::Value =
        serde_json::from_str(json_str).map_err(|e| format!("failed to parse sidecar result: {e}"))?;

    let ok = parsed.get("ok").and_then(|v| v.as_bool()).unwrap_or(false);
    if ok {
        Ok(parsed)
    } else {
        Err(parsed
            .get("error")
            .and_then(|v| v.as_str())
            .unwrap_or("sidecar reported failure")
            .to_string())
    }
}

/// Spawns `npm run <script> -- <args...>` in the sidecar directory, with
/// `SKILLSHOME_ACCESS_TOKEN`/`SKILLSHOME_BACKEND_URL` plus `extra_env` (e.g.
/// `BYOK_API_KEY`) set — child-process env vars, never sent over a network,
/// matching the precedent already established for `BYOK_API_KEY` in task 4.9.
/// If `stdin_payload` is present, it's written to the child's stdin concurrently
/// with draining stdout (`tokio::join!`, not sequential awaits) — avoids a
/// pipe-buffer deadlock risk on a large review package that could otherwise fill
/// the OS pipe buffer before either side finishes.
pub async fn run_sidecar_command(
    script: &str,
    args: &[&str],
    access_token: &str,
    backend_url: &str,
    stdin_payload: Option<&str>,
    extra_env: &[(&str, &str)],
) -> Result<serde_json::Value, String> {
    let dir = sidecar_dir()?;

    let mut cmd = TokioCommand::new("npm");
    cmd.current_dir(&dir)
        .arg("run")
        .arg(script)
        .arg("--")
        .args(args)
        .env("SKILLSHOME_ACCESS_TOKEN", access_token)
        .env("SKILLSHOME_BACKEND_URL", backend_url)
        .stdin(if stdin_payload.is_some() { Stdio::piped() } else { Stdio::null() })
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit()); // dev-mode: progress messages surface in the terminal running `cargo tauri dev`

    for (key, value) in extra_env {
        cmd.env(key, value);
    }

    let mut child = cmd.spawn().map_err(|e| format!("failed to spawn sidecar: {e}"))?;
    let mut stdin_handle = child.stdin.take();
    let mut stdout_handle = child.stdout.take().ok_or_else(|| "sidecar stdout was not piped".to_string())?;

    let write_stdin = async move {
        if let (Some(payload), Some(stdin)) = (stdin_payload, stdin_handle.as_mut()) {
            stdin
                .write_all(payload.as_bytes())
                .await
                .map_err(|e| format!("failed to write to sidecar stdin: {e}"))?;
        }
        drop(stdin_handle); // close stdin explicitly so the child sees EOF
        Ok::<(), String>(())
    };

    let read_stdout = async move {
        let mut output = Vec::new();
        stdout_handle
            .read_to_end(&mut output)
            .await
            .map_err(|e| format!("failed to read sidecar stdout: {e}"))?;
        Ok::<Vec<u8>, String>(output)
    };

    let (stdin_result, stdout_result) = tokio::join!(write_stdin, read_stdout);
    stdin_result?;
    let output = stdout_result?;

    child.wait().await.map_err(|e| format!("failed to wait for sidecar: {e}"))?;

    parse_marker_line(&output)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_marker_line_extracts_success_result() {
        let stdout = b"some info log\nanother log line\n__SIDECAR_RESULT__:{\"ok\":true,\"jobId\":\"job-1\"}\n";
        let result = parse_marker_line(stdout).expect("should succeed");
        assert_eq!(result["jobId"], "job-1");
    }

    #[test]
    fn parse_marker_line_uses_the_last_marker_line_if_multiple_appear() {
        // Defensive: agents-core's own logger could in principle emit a line that
        // happens to start with the same prefix by coincidence — always trust the
        // LAST one, since that's what each script's own code always emits last.
        let stdout = b"__SIDECAR_RESULT__:{\"ok\":false,\"error\":\"stale\"}\n__SIDECAR_RESULT__:{\"ok\":true,\"jobId\":\"job-2\"}\n";
        let result = parse_marker_line(stdout).expect("should succeed");
        assert_eq!(result["jobId"], "job-2");
    }

    #[test]
    fn parse_marker_line_maps_ok_false_to_err_with_the_error_message() {
        let stdout = b"__SIDECAR_RESULT__:{\"ok\":false,\"error\":\"model not found\"}\n";
        let err = parse_marker_line(stdout).expect_err("should fail");
        assert_eq!(err, "model not found");
    }

    #[test]
    fn parse_marker_line_errors_clearly_when_no_marker_line_is_present() {
        let stdout = b"some crash before any output\n";
        let err = parse_marker_line(stdout).expect_err("should fail");
        assert!(err.contains("no result line"));
    }

    #[test]
    fn parse_marker_line_errors_clearly_on_malformed_json() {
        let stdout = b"__SIDECAR_RESULT__:{not valid json\n";
        let err = parse_marker_line(stdout).expect_err("should fail");
        assert!(err.contains("failed to parse sidecar result"));
    }

    #[test]
    fn sidecar_dir_resolves_to_a_real_sibling_directory() {
        let dir = sidecar_dir().expect("sidecar dir should resolve in this checkout");
        assert!(dir.join("package.json").is_file());
    }
}
