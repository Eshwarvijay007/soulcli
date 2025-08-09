mod ui; mod api_client; mod autocorrect; mod history; mod shell;

use std::sync::Arc;
use std::sync::mpsc;
use tokio::runtime::Runtime;
use ui::{run_loop, Emotion, UiEvent};
use crate::shell::run_shell_and_stream;
use autocorrect::AutoCorrect;
use history::History;
use std::path::PathBuf;


fn map_emotion(s: &str) -> Emotion {
    match s {
        "happy" => Emotion::Happy,
        "sad" => Emotion::Sad,
        "alert" | "warning" => Emotion::Alert,
        _ => Emotion::Neutral,
    }
}

fn main() -> anyhow::Result<()> {
    let api_url = std::env::var("SOULSHELL_API_URL").unwrap_or("http://127.0.0.1:8000".into());
    let rt = Arc::new(Runtime::new()?);
    //let (tx, rx) = mpsc::channel::<(String, String)>(); // (text, emotion)
    let (tx, rx) = mpsc::channel::<UiEvent>(); 

    let mut ac = AutoCorrect::load();
    let mut hist = History::new(PathBuf::from("./history.txt"), 200);

    // Run TUI; UI will pull from rx and render messages
    run_loop(rx, move |mut line: String| {
        // 1) Autocorrect first token
        let corrected = ac.correct_line(&line);
        if corrected != line {
            ac.learn(
                &line.split_whitespace().next().unwrap_or(""),
                &corrected.split_whitespace().next().unwrap_or(""),
            );
            line = corrected;
        }
    
        // 2) Save history
        hist.push(line.clone());
    
        // 3) Spawn LLM request (does NOT block shell output)
        {
            let api_url = api_url.clone();
            let hist_vec = hist.items.clone();
            let tx_llm = tx.clone();
            let rt_llm = rt.clone();
            let line_for_llm = line.clone();
            rt_llm.spawn(async move {
                match api_client::send_query(&api_url, &line_for_llm, hist_vec).await {
                    Ok(resp) => {
                        let emo = resp.emotion.unwrap_or_else(|| "neutral".into());
                        let _ = tx_llm.send(UiEvent::Llm { text: resp.text, emotion: emo });
                    }
                    Err(e) => {
                        let _ = tx_llm.send(UiEvent::Llm { text: format!("LLM error: {}", e), emotion: "alert".into() });
                    }
                }
            });
        }
    
        // 4) Spawn shell execution (streams stdout/stderr)
        {
            let tx_shell = tx.clone();
            let rt_sh = rt.clone();
            let cmd = line.clone();
            rt_sh.spawn(async move {
                // give run_shell_and_stream its own sender instance
                let tx_for_run = tx_shell.clone();
                if let Err(e) = run_shell_and_stream(&cmd, tx_for_run).await {
                    // use the original cloned sender here
                    let _ = tx_shell.send(UiEvent::Stderr(format!("shell error: {}", e)));
                }
            });
        }
    }, map_emotion)
}
