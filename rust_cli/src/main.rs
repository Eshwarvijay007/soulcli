mod ui;
mod api_client;
mod autocorrect;
mod history;
mod shell;
mod router;

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::mpsc;
use tokio::runtime::Runtime;

use autocorrect::AutoCorrect;
use crate::shell::run_shell_and_stream;
use history::History;
use ui::{run_loop, Emotion, UiEvent};
use router::{route_prompt as route_prompt_local, mode_label};

fn map_emotion(s: &str) -> Emotion {
    match s {
        "happy" => Emotion::Happy,
        "sad" => Emotion::Sad,
        "alert" | "warning" => Emotion::Alert,
        _ => Emotion::Neutral,
    }
}

fn main() -> anyhow::Result<()> {
    // Print big gradient banner + tips, Rust-style
    print_welcome_banner();
    // Keep banner visible before switching to alternate screen with a small indicator
    show_startup_indicator(5);

    let api_url = std::env::var("SOULSHELL_API_URL")
        .unwrap_or_else(|_| "http://127.0.0.1:8000".into());

    // Single Tokio runtime shared by all async work
    let rt = Arc::new(Runtime::new()?);

    // Fan-in channel from workers → UI
    let (tx, rx) = mpsc::channel::<UiEvent>();

    // Autocorrect + in-memory history
    let mut ac = AutoCorrect::load();
    let mut hist = History::new(PathBuf::from("./history.txt"), 200);

    // TUI loop: consumes `rx` and renders; the closure dispatches work per submitted line
    run_loop(
        rx,
        move |mut line: String| {
            // 1) Autocorrect first token
            let corrected = ac.correct_line(&line);
            if corrected != line {
                ac.learn(
                    line.split_whitespace().next().unwrap_or(""),
                    corrected.split_whitespace().next().unwrap_or(""),
                );
                line = corrected;
            }

            // 2) Save history
            hist.push(line.clone());

            // 3) Route prompt (LLM router with fallback), announce mode, and spawn LLM request (non-blocking)
            {
                let api_url = api_url.clone();
                let hist_vec = hist.items.clone();
                let rt_llm = rt.clone();
                // Try server-side LLM router first inside async task; fall back to local heuristic
                let api_url_clone = api_url.clone();
                let hist_for_router = hist_vec.clone();
                let tx_router = tx.clone();
                let line_raw_for_router = line.clone();
                rt_llm.spawn(async move {
                    let routed = api_client::route_prompt(&api_url_clone, &line_raw_for_router, hist_for_router.clone()).await;
                    let (line_for_llm, mode_label_str, router_note) = match routed {
                        Ok(r) => (r.framed, r.mode, r.note.unwrap_or_default()),
                        Err(_) => {
                            let (fallback, mode) = route_prompt_local(&line_raw_for_router);
                            (fallback, mode_label(mode).to_string(), String::new())
                        }
                    };
                    let _ = tx_router.send(UiEvent::Status(format!("router: {}", mode_label_str)));
                    if !router_note.is_empty() {
                        let _ = tx_router.send(UiEvent::Status(router_note));
                    }

                    // Now launch the actual LLM query stream
                    let tx_llm_inner = tx_router.clone();
                    let api_url_q = api_url_clone.clone();
                    let hist_for_llm = hist_for_router.clone();
                    let line_for_q = line_for_llm.clone();
                    let conv_id: u64 = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .as_nanos() as u64;
                    tokio::spawn(async move {
                        match api_client::send_query(&api_url_q, &line_for_q, hist_for_llm).await {
                            Ok(resp) => {
                                let text = resp.text;
                                let emo = resp.emotion.unwrap_or_else(|| "neutral".into());
                                let chunk_size = 48usize;
                                let mut i = 0usize;
                                while i < text.len() {
                                    let end = (i + chunk_size).min(text.len());
                                    let part = text[i..end].to_string();
                                    let _ = tx_llm_inner.send(UiEvent::LlmChunk { id: conv_id, text: part });
                                    i = end;
                                    tokio::time::sleep(std::time::Duration::from_millis(20)).await;
                                }
                                let _ = tx_llm_inner.send(UiEvent::LlmDone { id: conv_id, emotion: emo });
                            }
                            Err(e) => {
                                let _ = tx_llm_inner.send(UiEvent::LlmChunk { id: conv_id, text: format!("LLM error: {}", e) });
                                let _ = tx_llm_inner.send(UiEvent::LlmDone { id: conv_id, emotion: "alert".into() });
                            }
                        }
                    });
                });
                // removed duplicate immediate query; we now run it after routing completes
            }

            // 4) Spawn shell execution (streams stdout/stderr, non-blocking)
            {
                let tx_shell = tx.clone(); // clone sender for shell task
                let rt_sh = rt.clone();
                let cmd = line.clone();

                rt_sh.spawn(async move {
                    // pass a dedicated clone into the streaming function
                    let tx_for_run = tx_shell.clone();
                    if let Err(e) = run_shell_and_stream(&cmd, tx_for_run).await {
                        // use the original clone for error reporting
                        let _ = tx_shell.send(UiEvent::Stderr(format!("shell error: {}", e)));
                    }
                });
            }
        },
        map_emotion,
    )
}

/* ----------------------- Welcome Banner ----------------------- */

fn print_welcome_banner() {
    // Print SOULCLI banner with beautiful Rust gradient using proper Unicode box characters
    println!("\x1b[38;5;208m███████\x1b[38;5;196m╗\x1b[0m \x1b[38;5;196m██████\x1b[38;5;130m╗\x1b[0m \x1b[38;5;130m██\x1b[38;5;220m╗\x1b[0m   \x1b[38;5;220m██\x1b[38;5;184m╗\x1b[0m \x1b[38;5;184m██\x1b[38;5;172m╗\x1b[0m      \x1b[38;5;172m██████\x1b[38;5;214m╗\x1b[0m \x1b[38;5;214m██\x1b[38;5;208m╗\x1b[0m     \x1b[38;5;208m██\x1b[38;5;196m╗\x1b[0m");
    println!("\x1b[38;5;208m██\x1b[38;5;196m╔════╝\x1b[0m \x1b[38;5;196m██\x1b[38;5;130m╔═══██\x1b[38;5;130m╗\x1b[0m \x1b[38;5;130m██\x1b[38;5;220m║\x1b[0m   \x1b[38;5;220m██\x1b[38;5;184m║\x1b[0m \x1b[38;5;184m██\x1b[38;5;172m║\x1b[0m     \x1b[38;5;172m██\x1b[38;5;214m╔════╝\x1b[0m \x1b[38;5;214m██\x1b[38;5;208m║\x1b[0m     \x1b[38;5;208m██\x1b[38;5;196m║\x1b[0m");
    println!("\x1b[38;5;208m███████\x1b[38;5;196m╗\x1b[0m \x1b[38;5;196m██\x1b[38;5;130m║\x1b[0m   \x1b[38;5;130m██\x1b[38;5;220m║\x1b[0m \x1b[38;5;220m██\x1b[38;5;184m║\x1b[0m   \x1b[38;5;184m██\x1b[38;5;172m║\x1b[0m \x1b[38;5;172m██\x1b[38;5;214m║\x1b[0m     \x1b[38;5;214m██\x1b[38;5;208m║\x1b[0m      \x1b[38;5;208m██\x1b[38;5;196m║\x1b[0m     \x1b[38;5;196m██\x1b[38;5;130m║\x1b[0m");
    println!("\x1b[38;5;208m╚════██\x1b[38;5;196m║\x1b[0m \x1b[38;5;196m██\x1b[38;5;130m║\x1b[0m   \x1b[38;5;130m██\x1b[38;5;220m║\x1b[0m \x1b[38;5;220m██\x1b[38;5;184m║\x1b[0m   \x1b[38;5;184m██\x1b[38;5;172m║\x1b[0m \x1b[38;5;172m██\x1b[38;5;214m║\x1b[0m     \x1b[38;5;214m██\x1b[38;5;208m║\x1b[0m      \x1b[38;5;208m██\x1b[38;5;196m║\x1b[0m     \x1b[38;5;196m██\x1b[38;5;130m║\x1b[0m");
    println!("\x1b[38;5;208m███████\x1b[38;5;196m║\x1b[0m \x1b[38;5;196m╚██████\x1b[38;5;130m╔╝\x1b[0m \x1b[38;5;130m╚██████\x1b[38;5;220m╔╝\x1b[0m \x1b[38;5;220m███████\x1b[38;5;184m╗\x1b[0m \x1b[38;5;184m╚██████\x1b[38;5;172m╗\x1b[0m \x1b[38;5;172m███████\x1b[38;5;214m╗\x1b[0m \x1b[38;5;214m██\x1b[38;5;208m║\x1b[0m");
    println!("\x1b[38;5;208m╚══════╝\x1b[0m \x1b[38;5;196m╚═════╝\x1b[0m  \x1b[38;5;130m╚═════╝\x1b[0m  \x1b[38;5;220m╚══════╝\x1b[0m \x1b[38;5;184m╚═════╝\x1b[0m \x1b[38;5;172m╚══════╝\x1b[0m \x1b[38;5;214m╚═╝\x1b[0m");

    let version = env!("CARGO_PKG_VERSION");
    let bold = "\x1b[1m";
    let dim = "\x1b[2m";
    let orange = "\x1b[38;5;208m"; // Rust orange for accents
    let reset = "\x1b[0m";

    println!();
    println!("{bold}Tips for getting started:{reset}");
    println!("1. Ask questions, run shell commands, or chat with the AI.");
    println!("2. Be specific for the best results.");
    println!("3. Use {orange}{bold}:help{reset} for commands, or {orange}{bold}:clear{reset} to reset the view.");
    println!("4. History is saved to {orange}{bold}history.txt{reset} (recent items only).");
    println!();
    println!("{dim}SoulCLI v{version} - Terminal with a Soul{reset}");
    println!();
}

fn show_startup_indicator(seconds: u64) {
    use std::io::Write;
    use std::time::{Duration, Instant};

    let green = "\x1b[32m";      // green
    let white = "\x1b[97m";      // bright white
    let reset = "\x1b[0m";       // reset
    let msg = "Building your terminal - Great things take time";

    let start = Instant::now();
    let mut tick: u32 = 0;
    while start.elapsed() < Duration::from_secs(seconds) {
        let active = (tick % 3) as usize; // which circle glows
        let mut circles = String::new();
        for i in 0..3 {
            if i == active {
                circles.push_str(&format!("{green}●{reset}"));
            } else {
                circles.push_str(&format!("{white}○{reset}"));
            }
            if i < 2 { circles.push(' '); }
        }
        // Clear line, carriage return; circles at far left, then message
        print!("\r\x1b[2K{}  {}…", circles, msg);
        let _ = std::io::stdout().flush();
        std::thread::sleep(Duration::from_millis(250));
        tick = tick.wrapping_add(1);
    }
    println!("");
}
