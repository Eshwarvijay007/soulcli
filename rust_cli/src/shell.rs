// src/shell.rs
use std::process::Stdio;
use std::sync::mpsc::Sender;
use tokio::{
    io::{AsyncBufReadExt, BufReader},
    process::Command,
};
use crate::ui::UiEvent;
use tokio::sync::oneshot;

pub async fn run_shell_and_stream(cmdline: &str, tx: Sender<UiEvent>) -> anyhow::Result<()> {
    // announce start
    let _ = tx.send(UiEvent::Status(format!("→ running: {}", cmdline)));

    // make a cancel channel for this process and register with UI
    let (tx_cancel, mut rx_cancel) = oneshot::channel::<()>();
    let _ = tx.send(UiEvent::RegisterCancel(tx_cancel));

    // spawn /bin/sh -c "<cmd>"
    let mut child = Command::new("sh")
        .arg("-c")
        .arg(cmdline)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    // stdout
    if let Some(out) = child.stdout.take() {
        let tx_out = tx.clone();
        tokio::spawn(async move {
            let mut reader = BufReader::new(out).lines();
            while let Ok(Some(line)) = reader.next_line().await {
                let _ = tx_out.send(UiEvent::Stdout(line));
            }
        });
    }

    // stderr
    if let Some(err) = child.stderr.take() {
        let tx_err = tx.clone();
        tokio::spawn(async move {
            let mut reader = BufReader::new(err).lines();
            while let Ok(Some(line)) = reader.next_line().await {
                let _ = tx_err.send(UiEvent::Stderr(line));
            }
        });
    }

    // wait for completion OR cancel
    let mut killed = false;
    tokio::select! {
        status = child.wait() => {
            let status = status?;
            let code = status.code().unwrap_or(-1);
            let _ = tx.send(UiEvent::Status(format!("← exit: {}", code)));
        }
        _ = &mut rx_cancel => {
            // kill process tree best-effort
            let _ = child.kill().await; // requires tokio 1.20+, sends SIGKILL/Terminate
            killed = true;
            let _ = tx.send(UiEvent::Status("↯ process killed".into()));
        }
    }

    // clear cancel button in UI when done
    let _ = tx.send(UiEvent::ClearCancel);
    Ok(())
}
