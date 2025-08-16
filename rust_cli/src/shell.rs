use anyhow::{anyhow, Context, Result};
use std::process::Stdio;
use std::sync::mpsc::Sender as StdSender;
use tokio::{
    io::{AsyncBufReadExt, BufReader},
    process::Command,
    select,
    sync::oneshot,
    time::{timeout, Duration},
};

use crate::ui::UiEvent;

/// Run a shell command, streaming stdout/stderr to the UI, supporting cancel.
/// Matches main.rs call: (&str, std::sync::mpsc::Sender<UiEvent>, String, Vec<String>)
/// The last two args are accepted and ignored (keeps your current call site unchanged).
pub async fn run_shell_and_stream(
    cmdline: &str,
    tx: StdSender<UiEvent>,
    _api_url: String,
    _history: Vec<String>,
) -> Result<i32> {
    // --- Build a concrete Command WITHOUT keeping a temporary borrow alive
    #[cfg(target_os = "windows")]
    let mut cmd = {
        let mut c = Command::new("cmd");
        c.arg("/C").arg(cmdline);
        c
    };

    #[cfg(not(target_os = "windows"))]
    let mut cmd = {
        let mut c = Command::new("bash");
        c.arg("-lc").arg(cmdline);
        c
    };

    // Configure stdio & spawn
    cmd.stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let mut child = cmd
        .spawn()
        .with_context(|| format!("failed to spawn shell for: {cmdline}"))?;

    // Prepare readers
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| anyhow!("missing stdout pipe"))?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| anyhow!("missing stderr pipe"))?;

    let mut stdout = BufReader::new(stdout).lines();
    let mut stderr = BufReader::new(stderr).lines();

    // Cancel channel (UI should hold the sender and trigger it on Esc/Ctrl-C)
    let (tx_cancel, mut rx_cancel) = oneshot::channel::<()>();
    let _ = tx.send(UiEvent::RegisterCancel(tx_cancel));
    let _ = tx.send(UiEvent::Status(format!("→ running: {cmdline}")));

    // Read/forward concurrently with cancel
    loop {
        select! {
            // cancellation requested
            _ = &mut rx_cancel => {
                let _ = tx.send(UiEvent::Status("⛔ cancelling…".into()));
                // Try graceful kill; if the child already exited this is a no-op
                let _ = child.kill().await;
                let _ = timeout(Duration::from_millis(400), child.wait()).await;
                let _ = tx.send(UiEvent::Status("⛔ cancelled".into()));
                let _ = tx.send(UiEvent::ClearCancel);
                return Ok(-1);
            }

            // stdout line
            line = stdout.next_line() => {
                match line {
                    Ok(Some(l)) => { let _ = tx.send(UiEvent::Stdout(l)); }
                    Ok(None) => { /* stdout EOF; keep looping to flush stderr / await exit */ }
                    Err(e) => {
                        let _ = tx.send(UiEvent::Status(format!("stdout error: {e}")));
                        break;
                    }
                }
            }

            // stderr line
            line = stderr.next_line() => {
                match line {
                    Ok(Some(l)) => { let _ = tx.send(UiEvent::Stderr(l)); }
                    Ok(None) => { /* stderr EOF */ }
                    Err(e) => {
                        let _ = tx.send(UiEvent::Status(format!("stderr error: {e}")));
                        break;
                    }
                }
            }
        }

        // Exit if the process ended
        if let Some(status) = child.try_wait()? {
            let code = status.code().unwrap_or(-1);
            let _ = tx.send(UiEvent::Status(format!("← exit: {code}")));
            let _ = tx.send(UiEvent::ClearCancel);
            return Ok(code);
        }
    }

    // Fallback (should rarely hit): ensure we reap the child
    let status = child.wait().await?;
    let code = status.code().unwrap_or(-1);
    let _ = tx.send(UiEvent::Status(format!("← exit: {code}")));
    let _ = tx.send(UiEvent::ClearCancel);
    Ok(code)
}
