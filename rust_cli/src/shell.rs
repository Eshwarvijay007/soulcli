// src/shell.rs
use std::process::Stdio;
use std::sync::mpsc::Sender;
use tokio::{
    io::{AsyncBufReadExt, BufReader},
    process::Command,
};
use crate::ui::UiEvent;

pub async fn run_shell_and_stream(cmdline: &str, tx: Sender<UiEvent>) -> anyhow::Result<()> {
    // announce start
    let _ = tx.send(UiEvent::Status(format!("→ running: {}", cmdline)));

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

    // wait for completion
    let status = child.wait().await?;
    let code = status.code().unwrap_or(-1);
    let _ = tx.send(UiEvent::Status(format!("← exit: {}", code)));
    Ok(())
}
