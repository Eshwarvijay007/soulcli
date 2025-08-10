// src/shell.rs
use std::process::Stdio;
use std::sync::{mpsc::Sender, Arc, Mutex};
use tokio::{
    io::{AsyncBufReadExt, BufReader},
    process::Command,
};
use crate::ui::UiEvent;
use tokio::sync::oneshot;
use crate::api_client;

pub async fn run_shell_and_stream(
    cmdline: &str,
    tx: Sender<UiEvent>,
    api_url: String,
    history: Vec<String>,
) -> anyhow::Result<()> {
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

    let stdout_acc = Arc::new(Mutex::new(String::new()));
    let stderr_acc = Arc::new(Mutex::new(String::new()));

    // stdout
    if let Some(out) = child.stdout.take() {
        let tx_out = tx.clone();
        let stdout_acc_clone = stdout_acc.clone();
        tokio::spawn(async move {
            let mut reader = BufReader::new(out).lines();
            while let Ok(Some(line)) = reader.next_line().await {
                let _ = tx_out.send(UiEvent::Stdout(line.clone()));
                let mut acc = stdout_acc_clone.lock().unwrap();
                acc.push_str(&line);
                acc.push('\n');
            }
        });
    }

    // stderr
    if let Some(err) = child.stderr.take() {
        let tx_err = tx.clone();
        let stderr_acc_clone = stderr_acc.clone();
        tokio::spawn(async move {
            let mut reader = BufReader::new(err).lines();
            while let Ok(Some(line)) = reader.next_line().await {
                let _ = tx_err.send(UiEvent::Stderr(line.clone()));
                let mut acc = stderr_acc_clone.lock().unwrap();
                acc.push_str(&line);
                acc.push('\n');
            }
        });
    }

    // wait for completion OR cancel
    let mut killed = false;
    let status = tokio::select! {
        status = child.wait() => {
            status
        }
        _ = &mut rx_cancel => {
            // kill process tree best-effort
            let _ = child.kill().await; // requires tokio 1.20+, sends SIGKILL/Terminate
            killed = true;
            let _ = tx.send(UiEvent::Status("↯ process killed".into()));
            let _ = tx.send(UiEvent::ClearCancel);
            return Ok(())
        }
    }?;

    let code = status.code().unwrap_or(-1);
    let _ = tx.send(UiEvent::Status(format!("← exit: {}", code)));

    // clear cancel button in UI when done
    let _ = tx.send(UiEvent::ClearCancel);

    let stdout_output = stdout_acc.lock().unwrap().clone();
    let stderr_output = stderr_acc.lock().unwrap().clone();

    // please also feed the llm returned response from command but be cautious because if its simple git status its fine but if its a ping command which prints the
    // dynamic values will call the llm unlimited times and eatup all my credits so be cautious about this similar types of commands
    if !killed && code == 0 && (!stdout_output.is_empty() || !stderr_output.is_empty()) {
        let output = format!("STDOUT:\n{}
STDERR:{}
", stdout_output, stderr_output);
        let llm_input = format!("The command `{}` was executed with exit code {}. It produced the following output. Please analyze it and provide a summary or suggest a next step:\n\n{}", cmdline, code, output);

        let tx_llm = tx.clone();
        tokio::spawn(async move {
            match api_client::send_query(&api_url, &llm_input, history).await {
                Ok(resp) => {
                    let conv_id: u64 = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .as_nanos() as u64;
                    let _ = tx_llm.send(UiEvent::LlmChunk { id: conv_id, text: resp.text });
                    let _ = tx_llm.send(UiEvent::LlmDone { id: conv_id, emotion: resp.emotion.unwrap_or("neutral".to_string()) });
                }
                Err(e) => {
                    let _ = tx_llm.send(UiEvent::Stderr(format!("LLM error after shell command: {}", e)));
                }
            }
        });
    }

    Ok(())
}

