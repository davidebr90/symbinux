//! Cancellable subprocess execution for the Linux host-tool backends.

use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use crate::WirelessError;

pub(crate) fn require_command(program: &str, message: &str) -> Result<(), WirelessError> {
    if command_exists(program) {
        Ok(())
    } else {
        Err(WirelessError::Unavailable(message.to_string()))
    }
}

fn command_exists(program: &str) -> bool {
    let Some(paths) = std::env::var_os("PATH") else {
        return false;
    };
    std::env::split_paths(&paths)
        .any(|dir| dir.join(program).is_file() || dir.join(format!("{program}.exe")).is_file())
}

pub(crate) fn run_command(
    program: &str,
    args: &[&str],
    timeout: Duration,
    cancel: &AtomicBool,
) -> Result<String, WirelessError> {
    let mut child = Command::new(program)
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|err| WirelessError::Failed(format!("Could not start {program}: {err}")))?;
    let deadline = Instant::now() + timeout;

    loop {
        if cancel.load(Ordering::SeqCst) {
            let _ = child.kill();
            return Err(WirelessError::Cancelled);
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            return Err(WirelessError::Failed(format!("{program} timed out.")));
        }
        match child.try_wait() {
            Ok(Some(_)) => {
                let output = child.wait_with_output().map_err(|err| {
                    WirelessError::Failed(format!("Could not read {program} output: {err}"))
                })?;
                if output.status.success() {
                    return Ok(String::from_utf8_lossy(&output.stdout).into_owned());
                }

                let stderr = String::from_utf8_lossy(&output.stderr);
                let stdout = String::from_utf8_lossy(&output.stdout);
                let detail = if stderr.trim().is_empty() {
                    stdout.trim()
                } else {
                    stderr.trim()
                };
                return Err(WirelessError::Failed(if detail.is_empty() {
                    format!("{program} failed.")
                } else {
                    format!("{program} failed: {detail}")
                }));
            }
            Ok(None) => thread::sleep(Duration::from_millis(50)),
            Err(err) => {
                let _ = child.kill();
                return Err(WirelessError::Failed(format!(
                    "Could not wait for {program}: {err}"
                )));
            }
        }
    }
}
