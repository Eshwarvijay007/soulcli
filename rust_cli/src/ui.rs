use std::sync::mpsc::Receiver;
use std::time::Duration;
use crossterm::event::{self, Event, KeyCode};
use ratatui::{
    backend::CrosstermBackend, Terminal,
    layout::{Layout, Constraint, Direction},
    widgets::{Block, Borders, Paragraph, Wrap, Clear},
    style::{Style, Color, Modifier},
    text::{Span, Line},
};
use unicode_width::UnicodeWidthStr;
use tokio::sync::oneshot;

#[derive(Clone, Copy)]
pub enum Emotion { Neutral, Happy, Sad, Alert }

pub enum UiEvent {
    Llm { text: String, emotion: String },
    LlmChunk { id: u64, text: String },
    LlmDone { id: u64, emotion: String },
    Stdout(String),
    Stderr(String),
    Status(String),
    RegisterCancel(oneshot::Sender<()>),
    ClearCancel,
}

#[derive(Clone, Copy)]
pub enum MessageOrigin { UserCommand, Llm, Stdout, Stderr, Status }

pub struct Message {
    pub text: String,
    pub emotion: Emotion,
    pub origin: MessageOrigin,
    pub conversation_id: u64,
}

pub struct UiState {
    input: String,
    messages: Vec<Message>,
    typing: bool,
    pending_llm: u32,
    mood: Emotion,
    scroll: u16,
    cancel_sender: Option<oneshot::Sender<()>>, // active process cancel
}

impl UiState {
    fn new() -> Self {
        Self {
            input: String::new(),
            messages: vec![],
            typing: false,
            pending_llm: 0,
            mood: Emotion::Neutral,
            scroll: 0,
            cancel_sender: None,
        }
    }
}

fn emotion_color(emotion: Emotion) -> Color {
    match emotion {
        Emotion::Neutral => Color::Gray,
        Emotion::Happy => Color::Green,
        Emotion::Sad => Color::Blue,
        Emotion::Alert => Color::Red,
    }
}

fn lerp(a: u8, b: u8, t: f32) -> u8 {
    (a as f32 + (b as f32 - a as f32) * t).round() as u8
}

fn gradient_color(t: f32) -> Color {
    // Simple purple → cyan gradient
    let r = lerp(180, 0, t);
    let g = lerp(0, 255, t);
    let b = lerp(255, 255, t);
    Color::Rgb(r, g, b)
}

fn gradient_spans(text: &str, dim: bool) -> Vec<Span<'static>> {
    let mut spans: Vec<Span<'static>> = Vec::with_capacity(text.len().max(1));
    let chars: Vec<char> = text.chars().collect();
    let len = chars.len().max(1);
    for (i, ch) in chars.into_iter().enumerate() {
        let t = if len <= 1 { 0.0 } else { i as f32 / (len.saturating_sub(1) as f32) };
        let color = gradient_color(t);
        let mut style = Style::default().fg(color);
        if dim { style = style.add_modifier(Modifier::DIM); }
        spans.push(Span::styled(ch.to_string(), style));
    }
    spans
}

fn render_message_line(msg: &Message, dim: bool) -> Line<'static> {
    match msg.origin {
        MessageOrigin::Llm => Line::from(gradient_spans(&msg.text, dim)),
        MessageOrigin::UserCommand => {
            let mut style = Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD);
            if dim { style = style.add_modifier(Modifier::DIM); }
            Line::from(Span::styled(msg.text.clone(), style))
        }
        _ => {
            let mut style = Style::default().fg(emotion_color(msg.emotion));
            if dim { style = style.add_modifier(Modifier::DIM); }
            Line::from(Span::styled(msg.text.clone(), style))
        }
    }
}

fn line_display_rows(line: &Line<'_>, available_width: u16) -> u16 {
    let mut width = 0usize;
    for span in &line.spans {
        width += span.content.width();
    }
    let aw = available_width.max(1) as usize;
    let rows = if width == 0 { 1 } else { (width + aw - 1) / aw };
    rows as u16
}

/* -------------------- minimal LLM markdown cleaner -------------------- */

/// Very small markdown cleaner for LLM text.
/// - strips **bold**, *italics*, __bold__, _italics_, `inline code`
/// - flattens headings like "# Title" -> "Title"
/// - converts [text](url) -> "text (url)"
/// - preserves fenced code blocks ``` ... ``` by indenting them
/// - inserts a newline after "Next steps:" / "Summary:" / "Tips:"
fn clean_llm_text(input: &str) -> String {
    // normalize newlines
    let mut s = input.replace("\r\n", "\n");

    // 1) preserve fenced code blocks by indenting and removing the fences
    let mut out = String::with_capacity(s.len());
    let mut i = 0usize;
    let bytes = s.as_bytes();
    let mut in_fence = false;

    while i < bytes.len() {
        if !in_fence && i + 3 <= bytes.len() && &s[i..i + 3] == "```" {
            in_fence = true;
            i += 3;
            while i < bytes.len() && bytes[i] != b'\n' { i += 1; }
            if i < bytes.len() && bytes[i] == b'\n' { i += 1; }
            out.push('\n');
            continue;
        }
        if in_fence && i + 3 <= bytes.len() && &s[i..i + 3] == "```" {
            in_fence = false;
            i += 3;
            if i < bytes.len() && bytes[i] == b'\n' { i += 1; }
            out.push('\n');
            continue;
        }
        if in_fence {
            let start = i;
            while i < bytes.len() && bytes[i] != b'\n' { i += 1; }
            out.push_str("    ");
            out.push_str(&s[start..i]);
            if i < bytes.len() && bytes[i] == b'\n' { out.push('\n'); i += 1; }
            continue;
        }
        out.push(bytes[i] as char);
        i += 1;
    }

    s = out;

    // 2) headings: strip leading '#' and spaces on each line
    let mut cleaned = String::with_capacity(s.len());
    for line in s.lines() {
        let mut l = line;
        let mut hashes = 0;
        for ch in l.chars() {
            if ch == '#' && hashes < 6 { hashes += 1; } else { break; }
        }
        if hashes > 0 {
            l = l.trim_start_matches('#').trim_start();
        }
        cleaned.push_str(l);
        cleaned.push('\n');
    }
    s = cleaned;

    // 3) inline code: remove backticks
    s = s.replace('`', "");

    // 4) bold/italics markers
    s = s.replace("**", "");
    s = s.replace("__", "");
    s = s.replace('*', "");
    s = s.replace('_', "");

    // 5) links: [text](url) -> text (url)  (minimal, non-regex)
    let mut out2 = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '[' {
            let mut text = String::new();
            while let Some(&nc) = chars.peek() {
                chars.next();
                if nc == ']' { break; }
                text.push(nc);
            }
            if let Some(&'(') = chars.peek() {
                chars.next();
                let mut url = String::new();
                while let Some(&nc) = chars.peek() {
                    chars.next();
                    if nc == ')' { break; }
                    url.push(nc);
                }
                out2.push_str(&text);
                if !url.is_empty() {
                    out2.push_str(" (");
                    out2.push_str(&url);
                    out2.push(')');
                }
            } else {
                out2.push('[');
                out2.push_str(&text);
                out2.push(']');
            }
        } else {
            out2.push(c);
        }
    }
    s = out2;

    // 6) newline after common labels
    for label in ["Next steps:", "NEXT STEPS:", "Summary:", "SUMMARY:", "Tips:", "TIPS:"] {
        s = s.replace(label, &format!("{label}\n"));
    }

    // 7) split glued ordered lists like "1. foo 2. bar"
    let mut last_was_digit_dot = false;
    let mut out3 = String::with_capacity(s.len());
    let mut iter = s.chars().peekable();
    while let Some(ch) = iter.next() {
        if ch.is_ascii_digit() && matches!(iter.peek(), Some('.')) {
            if let Some(prev) = out3.chars().last() {
                if prev != '\n' && prev != ' ' { out3.push('\n'); }
            }
            out3.push(ch);
            last_was_digit_dot = true;
        } else {
            out3.push(ch);
            if last_was_digit_dot && ch == '.' { last_was_digit_dot = false; }
        }
    }

    // 8) collapse double spaces (keep newlines)
    let mut final_s = String::with_capacity(out3.len());
    let mut prev_space = false;
    for ch in out3.chars() {
        if ch == ' ' {
            if !prev_space { final_s.push(ch); }
            prev_space = true;
        } else {
            final_s.push(ch);
            prev_space = false;
        }
    }

    final_s
}

/* -------------------- UI loop -------------------- */

pub fn run_loop<F, MapEmo>(
    rx: Receiver<UiEvent>,
    mut on_submit: F,
    mut map_emotion: MapEmo,
) -> anyhow::Result<()>
where
    F: FnMut(String) + Send + 'static,
    MapEmo: FnMut(&str) -> Emotion + Send + 'static,
{
    crossterm::terminal::enable_raw_mode()?;
    let mut stdout = std::io::stdout();
    crossterm::execute!(stdout, crossterm::terminal::EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    terminal.clear()?;

    let mut state = UiState::new();
    let mut frame = 0u64;

    loop {
        // 1) Pull any backend replies (non-blocking) and update state
        while let Ok(ev) = rx.try_recv() {
            match ev {
                UiEvent::Llm { text, emotion } => {
                    state.pending_llm = state.pending_llm.saturating_sub(1);
                    state.typing = state.pending_llm > 0;
                    state.mood = map_emotion(&emotion);
                    let clean = clean_llm_text(&text);
                    state.messages.push(Message {
                        text: clean,
                        emotion: state.mood,
                        origin: MessageOrigin::Llm,
                        conversation_id: 0,
                    });
                }
                
                // 1) streaming chunks: append RAW (no cleaning yet)
                UiEvent::LlmChunk { id, text } => {
                    if let Some(pos) = state.messages.iter().rposition(|m|
                        matches!(m.origin, MessageOrigin::Llm) && m.conversation_id == id
                    ) {
                        state.messages[pos].text.push_str(&text);
                    } else {
                        state.messages.push(Message {
                            text,
                            emotion: Emotion::Neutral,
                            origin: MessageOrigin::Llm,
                            conversation_id: id,
                        });
                    }
                }

                // 2) stream finished: CLEAN the whole aggregated text once
                UiEvent::LlmDone { id, emotion } => {
                    state.pending_llm = state.pending_llm.saturating_sub(1);
                    state.typing = state.pending_llm > 0;
                    state.mood = map_emotion(&emotion);

                    if let Some(pos) = state.messages.iter().rposition(|m|
                        matches!(m.origin, MessageOrigin::Llm) && m.conversation_id == id
                    ) {
                        let raw = std::mem::take(&mut state.messages[pos].text);
                        state.messages[pos].text = clean_llm_text(&raw);
                        state.messages[pos].emotion = state.mood;
                    }
                }

                UiEvent::Stdout(line) => {
                    state.messages.push(Message { text: line, emotion: Emotion::Neutral, origin: MessageOrigin::Stdout, conversation_id: 0 });
                }
                UiEvent::Stderr(line) => {
                    state.messages.push(Message { text: line, emotion: Emotion::Alert, origin: MessageOrigin::Stderr, conversation_id: 0 });
                }
                UiEvent::Status(line) => {
                    state.messages.push(Message { text: line, emotion: Emotion::Neutral, origin: MessageOrigin::Status, conversation_id: 0 });
                }
                UiEvent::RegisterCancel(tx_cancel) => {
                    state.cancel_sender = Some(tx_cancel);
                }
                UiEvent::ClearCancel => {
                    state.cancel_sender = None;
                }
            }
        }
        

        // 2) Draw UI
        terminal.draw(|f| {
            let size = f.size();
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Length(3), Constraint::Min(3), Constraint::Length(3)])
                .split(size);

            // Header
            let mut header_spans = vec![
                Span::styled(" 🧠 SoulShell ", Style::default().fg(Color::Cyan)),
                Span::raw("— a terminal with feelings "),
            ];
            if state.cancel_sender.is_some() {
                header_spans.push(Span::styled("[", Style::default().fg(Color::DarkGray)));
                header_spans.push(Span::styled("X", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)));
                header_spans.push(Span::styled("] press x to cancel", Style::default().fg(Color::DarkGray)));
            }
            let header = Paragraph::new(Line::from(header_spans))
                .block(Block::default().borders(Borders::ALL));
            f.render_widget(header, chunks[0]);

            // Messages: latest conversation first (top), older history below
            let mut lines: Vec<Line> = Vec::with_capacity(state.messages.len() + 2);

            // Identify the start of the most recent command group by origin
            let latest_cmd_start = state
                .messages
                .iter()
                .rposition(|m| matches!(m.origin, MessageOrigin::UserCommand));

            // Render older history first (top), then a separator, then latest group (bottom)
            if let Some(idx) = latest_cmd_start {
                let has_prev_command = idx > 0 && state.messages[..idx]
                    .iter()
                    .any(|m| matches!(m.origin, MessageOrigin::UserCommand));
                if has_prev_command {
                    for m in &state.messages[..idx] {
                        lines.push(render_message_line(m, true));
                    }
                    lines.push(Line::from(Span::styled("──────── latest ────────", Style::default().fg(Color::DarkGray).add_modifier(Modifier::BOLD))));
                }

                // Latest group (chronological, not dimmed)
                for m in &state.messages[idx..] {
                    lines.push(render_message_line(m, false));
                }
                if state.typing {
                    let dots = ["·  ", "·· ", "···"][(frame as usize / 10) % 3];
                    lines.push(Line::from(Span::styled(format!("thinking {}", dots), Style::default().fg(Color::DarkGray))));
                }
            } else {
                // No commands yet: default to newest-first view
                for m in state.messages.iter() { lines.push(render_message_line(m, false)); }
                if state.typing {
                    let dots = ["·  ", "·· ", "···"][(frame as usize / 10) % 3];
                    lines.push(Line::from(Span::styled(format!("thinking {}", dots), Style::default().fg(Color::DarkGray))));
                }
            }
            // Bottom-anchored scrolling across entire buffer based on wrapped rows
            let available_width = chunks[1].width.saturating_sub(2); // minus borders
            let mut total_rows: u16 = 0;
            for line in &lines { total_rows = total_rows.saturating_add(line_display_rows(line, available_width)); }
            let content_height = chunks[1].height.saturating_sub(2); // minus borders
            let base_from_top = total_rows.saturating_sub(content_height);
            let clamped_scroll = state.scroll.min(base_from_top);
            let effective_from_top = base_from_top.saturating_sub(clamped_scroll);

            let dialog = Paragraph::new(lines)
                .wrap(Wrap { trim: true })
                .scroll((effective_from_top, 0))
                .block(Block::default().borders(Borders::ALL).title("dialog"));
            f.render_widget(dialog, chunks[1]);

            // Input
            let prompt = "> ";
            let input = Paragraph::new(format!("{prompt}{}", state.input))
                .block(Block::default().borders(Borders::ALL).title("input"));
            f.render_widget(Clear, chunks[2]);
            f.render_widget(input, chunks[2]);

            // Cursor in input
            let x = chunks[2].x + (prompt.len() as u16) + (state.input.chars().count() as u16);
            let y = chunks[2].y + 1;
            f.set_cursor(x, y);

            // Removed top loading/mood gauge bar
        })?;

        frame += 1;

        // 3) Handle keys
        if crossterm::event::poll(Duration::from_millis(33))? {
            match event::read()? {
                Event::Key(key) => match key.code {
                    KeyCode::Char(c) => state.input.push(c),
                    KeyCode::Backspace => { state.input.pop(); },
                    KeyCode::Enter => {
                        let line = std::mem::take(&mut state.input);
                        // Echo user command and show spinner
                        state.messages.push(Message { text: format!("$ {}", line), emotion: Emotion::Neutral, origin: MessageOrigin::UserCommand, conversation_id: 0 });
                        state.typing = true;
                        state.pending_llm = state.pending_llm.saturating_add(1);
                        state.scroll = 0; // anchor to latest group bottom
                        on_submit(line); // no borrowing of state inside the callback
                    }
                    KeyCode::Esc => break,
                    KeyCode::Char('x') => {
                        if let Some(tx) = state.cancel_sender.take() {
                            let _ = tx.send(());
                            state.messages.push(Message { text: "↯ canceled current process".into(), emotion: Emotion::Alert, origin: MessageOrigin::Status, conversation_id: 0 });
                        }
                    }
                    KeyCode::Up => state.scroll = state.scroll.saturating_add(1),
                    KeyCode::Down => state.scroll = state.scroll.saturating_sub(1),
                    KeyCode::PageUp => state.scroll = state.scroll.saturating_add(5),
                    KeyCode::PageDown => state.scroll = state.scroll.saturating_sub(5),
                    _ => {}
                },
                _ => {}
            }
        }
    }

    crossterm::terminal::disable_raw_mode()?;
    crossterm::execute!(terminal.backend_mut(), crossterm::terminal::LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    Ok(())
}
