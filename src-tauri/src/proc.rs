//! Process-ownership helpers for the `llama-server` lifecycle.
//!
//! The runtime diagnostic found orphaned `llama-server` processes surviving across
//! Voxa sessions and holding wired (`--mlock`) memory hostage. To reap them safely we
//! must distinguish *Voxa-owned* servers from any `llama-server` the user runs
//! independently.
//!
//! The critical guardrail (Requirement 1.3): a process is only ever eligible for
//! termination if its command-line args reference the Voxa model path. We never do a
//! blanket `killall llama-server`.
//!
//! Enumeration is strictly read-only (`ps`), and any failure fails safe — we log and
//! return no PIDs so startup is never blocked and no unrelated process is touched.

// These helpers are the foundation for the startup-reconciliation, shutdown, and respawn
// tasks (see tasks 3, 5, 6 of the llama-server-lifecycle spec). They are intentionally
// not yet wired into the app, so allow dead_code until those tasks consume them.
#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

use crate::models::ModelManager;

/// How long to wait for a process to exit after `SIGTERM` before escalating to `SIGKILL`.
const TERMINATE_GRACE: Duration = Duration::from_millis(500);

/// File name (under the app data dir) where the live `llama-server` PID is persisted.
const PID_FILE_NAME: &str = "llama-server.pid";

/// Absolute path to the `llama-server` PID file for a given app data dir.
///
/// The PID file lives directly under the app data dir (a sibling of `models/`), matching
/// the design: `<app_data_dir>/llama-server.pid`. Callers derive `app_data_dir` from the
/// existing `ModelManager` (its `base_path` is `<app_data_dir>/models`) rather than
/// hardcoding a path.
pub fn llama_pid_file(app_data_dir: &Path) -> PathBuf {
    app_data_dir.join(PID_FILE_NAME)
}

/// Persist `pid` to the PID file at `path` as plain text (a single integer).
///
/// Written on a successful `LlamaEngine::new` so a future startup can reap this server
/// even after an unclean Voxa exit (Requirement 2.3).
pub fn write_pid_file(path: &Path, pid: u32) -> std::io::Result<()> {
    std::fs::write(path, pid.to_string())
}

/// Read a PID previously written by [`write_pid_file`].
///
/// Returns `None` when the file is missing, unreadable, or does not contain a valid
/// integer — a corrupt PID file is ignored (fail safe), and callers fall back to process
/// enumeration.
pub fn read_pid_file(path: &Path) -> Option<u32> {
    let contents = std::fs::read_to_string(path).ok()?;
    contents.trim().parse::<u32>().ok()
}

/// Remove the PID file at `path`, treating an already-absent file as success.
///
/// Called on clean termination (Drop / explicit terminate / shutdown) so the file never
/// names a dead PID once the server is gone.
pub fn remove_pid_file(path: &Path) {
    match std::fs::remove_file(path) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => log::warn!("failed to remove PID file {:?}: {}", path, e),
    }
}

/// The absolute Voxa model path used to identify Voxa-owned `llama-server` processes.
///
/// Any `llama-server` whose command line contains this marker was spawned by Voxa (the
/// model lives under the app data dir), so it is ours to reap.
pub fn voxa_model_path_marker(manager: &ModelManager) -> String {
    manager.get_llama_path().to_string_lossy().to_string()
}

/// Pure ownership matcher: returns `true` only when `command_line` belongs to a
/// Voxa-owned `llama-server`.
///
/// A line is Voxa-owned only if it references **both** the `llama-server` binary and the
/// Voxa model path (`model_path_marker`). This is the ownership guardrail of
/// Requirement 1.3 — a decoy command line that runs `llama-server` against a *different*
/// model must never match.
///
/// An empty marker never matches (fail safe: we will not claim ownership of anything when
/// the model path is unknown).
fn is_voxa_llama_server(command_line: &str, model_path_marker: &str) -> bool {
    if model_path_marker.is_empty() {
        return false;
    }
    command_line.contains("llama-server") && command_line.contains(model_path_marker)
}

/// Pure parser: given the output of `ps` (one process per line, `<pid> <command…>`) and
/// the Voxa model path marker, return the PIDs of Voxa-owned `llama-server` processes.
///
/// Factored out from [`find_voxa_llama_servers`] so the matching logic is unit-testable
/// without spawning real processes.
fn parse_voxa_pids(ps_output: &str, model_path_marker: &str) -> Vec<u32> {
    ps_output
        .lines()
        .filter_map(|line| {
            let line = line.trim_start();
            // Each line is "<pid> <command with args>". Split off the PID; the remainder
            // is the full command line we match against.
            let (pid_str, command_line) = line.split_once(char::is_whitespace)?;
            let pid: u32 = pid_str.trim().parse().ok()?;
            if is_voxa_llama_server(command_line, model_path_marker) {
                Some(pid)
            } else {
                None
            }
        })
        .collect()
}

/// Read-only enumeration of all running processes with their full command lines.
///
/// `-ww` disables `ps` column-width truncation — essential because the Voxa model path is
/// long and would otherwise be cut off, breaking ownership matching. `-o pid=,command=`
/// emits "<pid> <full command>" with no header row.
#[cfg(unix)]
fn enumerate_processes() -> Result<String, String> {
    let output = Command::new("ps")
        .args(["-axww", "-o", "pid=,command="])
        .output()
        .map_err(|e| format!("failed to invoke ps: {}", e))?;

    if !output.status.success() {
        return Err(format!("ps exited with status {}", output.status));
    }

    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

#[cfg(not(unix))]
fn enumerate_processes() -> Result<String, String> {
    // The orphaned-server problem this guards against is observed on macOS (launchd
    // reparenting + --mlock wired memory). On non-unix platforms we fail safe rather
    // than risk touching unrelated processes.
    Err("process enumeration is only implemented on unix".to_string())
}

/// Enumerate running `llama-server` processes that are Voxa-owned (their command line
/// references `model_path_marker`) and return their PIDs.
///
/// Fails safe: if enumeration fails for any reason we log a warning and return an empty
/// list, so startup is never blocked and no process is ever terminated on bad data
/// (Requirement 1.4).
pub fn find_voxa_llama_servers(model_path_marker: &str) -> Vec<u32> {
    match enumerate_processes() {
        Ok(output) => parse_voxa_pids(&output, model_path_marker),
        Err(e) => {
            log::warn!("llama-server enumeration failed, skipping reap: {}", e);
            Vec::new()
        }
    }
}

/// Returns `true` if a process with `pid` currently exists.
///
/// Uses `kill -0`, which sends no signal but succeeds only when the process is alive and
/// signalable.
#[cfg(unix)]
fn pid_is_alive(pid: u32) -> bool {
    Command::new("kill")
        .arg("-0")
        .arg(pid.to_string())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

#[cfg(unix)]
fn send_signal(pid: u32, signal: &str) {
    let _ = Command::new("kill")
        .arg(format!("-{}", signal))
        .arg(pid.to_string())
        .status();
}

/// Terminate a process by PID with `SIGTERM` → grace period → `SIGKILL` escalation.
///
/// The caller is responsible for ensuring `pid` is Voxa-owned (e.g. via
/// [`find_voxa_llama_servers`]) before calling this — this function does not re-check
/// ownership.
#[cfg(unix)]
pub fn terminate_pid(pid: u32) {
    // Ask politely first so the server can release resources cleanly.
    send_signal(pid, "TERM");

    // Wait out the grace period, polling so we return as soon as it exits.
    let start = Instant::now();
    while start.elapsed() < TERMINATE_GRACE {
        if !pid_is_alive(pid) {
            log::info!("llama-server pid {} exited after SIGTERM", pid);
            return;
        }
        std::thread::sleep(Duration::from_millis(50));
    }

    // Still alive — escalate.
    if pid_is_alive(pid) {
        log::warn!(
            "llama-server pid {} did not exit within {}ms of SIGTERM, escalating to SIGKILL",
            pid,
            TERMINATE_GRACE.as_millis()
        );
        send_signal(pid, "KILL");
    }
}

/// Non-unix fallback: forcibly terminate the process tree via `taskkill`.
#[cfg(not(unix))]
pub fn terminate_pid(pid: u32) {
    let _ = Command::new("taskkill")
        .args(["/PID", &pid.to_string(), "/T", "/F"])
        .status();
}

#[cfg(test)]
mod tests {
    use super::*;

    // A realistic absolute Voxa model path under the app data dir.
    const VOXA_MARKER: &str =
        "/Users/alice/Library/Application Support/com.lufermalgo.voxa/models/qwen2.5-1.5b-instruct-q4_k_m.gguf";

    fn voxa_command_line() -> String {
        format!(
            "/opt/homebrew/bin/llama-server --model {} --port 51234 --host 127.0.0.1 -ngl 99 --ctx-size 4096 --mlock",
            VOXA_MARKER
        )
    }

    fn decoy_command_line() -> String {
        // Same binary, but a DIFFERENT model path — a user's own llama.cpp server.
        "/opt/homebrew/bin/llama-server --model /Users/alice/models/llama-3-8b.gguf --port 8080".to_string()
    }

    #[test]
    fn real_voxa_command_line_is_matched() {
        assert!(
            is_voxa_llama_server(&voxa_command_line(), VOXA_MARKER),
            "a llama-server running the Voxa model path must be recognized as Voxa-owned"
        );
    }

    #[test]
    fn decoy_with_different_model_is_not_matched() {
        assert!(
            !is_voxa_llama_server(&decoy_command_line(), VOXA_MARKER),
            "a llama-server running a DIFFERENT model path must never be matched (ownership guardrail, Req 1.3)"
        );
    }

    #[test]
    fn empty_marker_never_matches() {
        // Fail safe: with no known model path we must not claim ownership of anything.
        assert!(!is_voxa_llama_server(&voxa_command_line(), ""));
    }

    #[test]
    fn parse_picks_only_voxa_pids() {
        // Mixed `ps` output: a Voxa-owned server, a decoy server, and an unrelated process.
        let ps_output = format!(
            "  101 {voxa}\n  202 {decoy}\n  303 /usr/sbin/cfprefsd agent\n",
            voxa = voxa_command_line(),
            decoy = decoy_command_line(),
        );

        let pids = parse_voxa_pids(&ps_output, VOXA_MARKER);

        assert_eq!(pids, vec![101], "only the Voxa-owned server PID should be returned");
    }

    #[test]
    fn pid_file_write_read_remove_round_trip() {
        // Use a unique path under the temp dir so the test never touches the real app
        // data dir and parallel test runs don't collide.
        let dir = std::env::temp_dir();
        let path = dir.join(format!(
            "voxa-llama-server-test-{}-{}.pid",
            std::process::id(),
            Instant::now().elapsed().as_nanos()
        ));
        // Guard against a stale file from a previous aborted run.
        remove_pid_file(&path);

        let pid: u32 = 424242;

        // Write, then read back the same PID.
        write_pid_file(&path, pid).expect("write_pid_file should succeed in temp dir");
        assert_eq!(
            read_pid_file(&path),
            Some(pid),
            "read_pid_file should return the PID that was written"
        );

        // Remove, then confirm it is gone (read yields None).
        remove_pid_file(&path);
        assert!(!path.exists(), "PID file should be deleted after remove_pid_file");
        assert_eq!(
            read_pid_file(&path),
            None,
            "read_pid_file should return None once the file is removed"
        );

        // Removing an absent file is a no-op (does not panic / error).
        remove_pid_file(&path);
    }

    #[test]
    fn read_pid_file_ignores_corrupt_contents() {
        let dir = std::env::temp_dir();
        let path = dir.join(format!("voxa-llama-server-corrupt-{}.pid", std::process::id()));
        std::fs::write(&path, "not-a-pid").unwrap();

        assert_eq!(
            read_pid_file(&path),
            None,
            "a corrupt PID file must be ignored rather than parsed"
        );

        remove_pid_file(&path);
    }

    #[test]
    fn llama_pid_file_sits_under_app_data_dir() {
        let app_data_dir = Path::new("/Users/alice/Library/Application Support/com.lufermalgo.voxa");
        let pid_file = llama_pid_file(app_data_dir);
        assert_eq!(pid_file, app_data_dir.join("llama-server.pid"));
    }
}
