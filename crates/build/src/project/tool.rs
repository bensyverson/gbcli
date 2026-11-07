//! One run of a build tool under a time limit. Each tool runs in a process
//! group of its own, so the limit stops whatever the tool started with it:
//! a wrapper script (a version manager's shim, `sh` without `exec`) runs
//! the real tool as its child, and that child holds stderr open after the
//! wrapper is killed. Reading stderr never blocks past a short grace once
//! the tool has exited or been stopped, whatever still holds it.

use std::io::Read;
use std::path::Path;
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

/// How long stderr may stay open after the tool exits or is stopped. A tool
/// that exits closes it at once; past this, what holds it is a process the
/// tool left behind, which is stopped, and then given this long again.
const GRACE: Duration = Duration::from_millis(500);

/// What one run did: its exit status, or `None` when the time limit
/// stopped it, and what it printed to stderr.
pub(crate) struct Ran {
    pub status: Option<ExitStatus>,
    pub stderr: String,
}

/// Runs `tool` with `args` in `dir`, stopping it, and everything in its
/// process group, past `timeout`.
pub(crate) fn run(
    tool: &Path,
    args: &[String],
    dir: &Path,
    timeout: Duration,
) -> std::io::Result<Ran> {
    let mut command = Command::new(tool);
    command
        .args(args)
        .current_dir(dir)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped());
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        // Its own group (pgid = its pid), so the group is the tool and
        // everything it started, and nothing of ours.
        command.process_group(0);
    }
    let deadline = Instant::now() + timeout;
    let mut child = command.spawn()?;
    let mut pipe = child.stderr.take().expect("stderr is piped");
    let (send, receive) = mpsc::channel();
    // A thread that blocks on the pipe until every holder closes it; the
    // channel lets this side stop waiting while it still blocks.
    std::thread::spawn(move || {
        let mut bytes = Vec::new();
        let _ = pipe.read_to_end(&mut bytes);
        let _ = send.send(bytes);
    });
    let status = wait(&mut child, deadline)?;
    if status.is_none() {
        // Killed before it is reaped, so its pid still names its group.
        stop_group(&mut child);
        let _ = child.wait();
    }
    let bytes = receive.recv_timeout(GRACE).unwrap_or_else(|_| {
        stop_group(&mut child);
        receive.recv_timeout(GRACE).unwrap_or_default()
    });
    Ok(Ran {
        status,
        stderr: String::from_utf8_lossy(&bytes).into_owned(),
    })
}

/// Waits for `child` until `deadline`; `None` when it is still running then.
fn wait(child: &mut Child, deadline: Instant) -> std::io::Result<Option<ExitStatus>> {
    loop {
        if let Some(status) = child.try_wait()? {
            return Ok(Some(status));
        }
        if Instant::now() >= deadline {
            return Ok(None);
        }
        std::thread::sleep(Duration::from_millis(2));
    }
}

/// Kills `child`'s process group. std has no call for signaling a group and
/// the workspace forbids `unsafe`, so this runs the POSIX `kill` utility;
/// it only runs when a tool is being stopped. The child itself is killed
/// directly too, in case `kill` can't be run.
#[cfg(unix)]
fn stop_group(child: &mut Child) {
    let group = format!("-{}", child.id());
    for kill in ["/bin/kill", "kill"] {
        let ran = Command::new(kill)
            .args(["-KILL", "--", &group])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
        if ran.is_ok() {
            break;
        }
    }
    let _ = child.kill();
}

#[cfg(not(unix))]
fn stop_group(child: &mut Child) {
    let _ = child.kill();
}
