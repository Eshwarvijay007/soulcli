mod ui; mod api_client; mod autocorrect; mod history;

use std::sync::Arc;
use std::sync::mpsc;
use tokio::runtime::Runtime;
use ui::{run_loop, Emotion};
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
    let (tx, rx) = mpsc::channel::<(String, String)>(); // (text, emotion)

    let mut ac = AutoCorrect::load();
    let mut hist = History::new(PathBuf::from("./history.txt"), 200);

    // Run TUI; UI will pull from rx and render messages
    run_loop(rx, move |mut line: String| {
        // autocorrect only the first token
        let corrected = ac.correct_line(&line);
        if corrected != line {
            ac.learn(
                &line.split_whitespace().next().unwrap_or(""),
                &corrected.split_whitespace().next().unwrap_or(""),
            );
            line = corrected;
        }
    
        // add to history
        hist.push(line.clone());
    
        // async call to backend
        let api_url = api_url.clone();
        let hist_vec = hist.items.clone();
        let tx = tx.clone();
        let rt = rt.clone();
    
        rt.spawn(async move {
            match api_client::send_query(&api_url, &line, hist_vec).await {
                Ok(resp) => {
                    let emo = resp.emotion.unwrap_or_else(|| "neutral".into());
                    let _ = tx.send((resp.text, emo));
                }
                Err(e) => {
                    let _ = tx.send((format!("error: {}", e), "alert".into()));
                }
            }
        });
    }, map_emotion)    
}    
