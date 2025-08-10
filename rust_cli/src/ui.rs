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

#[derive(Clone, Copy)]
pub enum Emotion { Neutral, Happy, Sad, Alert }

pub enum UiEvent {
    Llm { text: String, emotion: String },
    Stdout(String),
    Stderr(String),
    Status(String),
}

pub struct Message { pub text: String, pub emotion: Emotion }

pub struct UiState {
    input: String,
    messages: Vec<Message>,
    typing: bool,
    pending_llm: u32,
    mood: Emotion,
    scroll: u16,
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
        }
    }
}

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
                    state.typing = state.pending_llm > 0; // keep spinner if other LLMs pending
                    state.mood = map_emotion(&emotion);
                    state.messages.push(Message { text, emotion: state.mood });
                }
                UiEvent::Stdout(line) => {
                    state.messages.push(Message { text: line, emotion: Emotion::Neutral });
                }
                UiEvent::Stderr(line) => {
                    state.messages.push(Message { text: line, emotion: Emotion::Alert });
                }
                UiEvent::Status(line) => {
                    state.messages.push(Message { text: line, emotion: Emotion::Neutral });
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
            let header = Paragraph::new(Line::from(vec![
                Span::styled(" 🧠 SoulShell ", Style::default().fg(Color::Cyan)),
                Span::raw("— a terminal with feelings"),
            ])).block(Block::default().borders(Borders::ALL));
            f.render_widget(header, chunks[0]);

            // Messages: latest conversation first (top), older history below
            let mut lines: Vec<Line> = Vec::with_capacity(state.messages.len() + 2);

            // Identify the start of the most recent command group by the echoed "$ " prefix
            let latest_cmd_start = state
                .messages
                .iter()
                .rposition(|m| m.text.starts_with("$ "));

            // Render current group first (top) so newest is visible immediately
            let mut latest_len: usize = 0;
            if let Some(idx) = latest_cmd_start {
                // Latest group (chronological, not dimmed)
                for m in &state.messages[idx..] {
                    let mut style = match m.emotion {
                        Emotion::Neutral => Style::default().fg(Color::Gray),
                        Emotion::Happy => Style::default().fg(Color::Green),
                        Emotion::Sad    => Style::default().fg(Color::Blue),
                        Emotion::Alert  => Style::default().fg(Color::Red),
                    };
                    if m.text.starts_with("$ ") {
                        style = Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD);
                    }
                    lines.push(Line::from(Span::styled(m.text.clone(), style)));
                    latest_len += 1;
                }
                if state.typing {
                    let dots = ["·  ", "·· ", "···"][(frame as usize / 10) % 3];
                    lines.push(Line::from(Span::styled(format!("thinking {}", dots), Style::default().fg(Color::DarkGray))));
                    latest_len += 1;
                }

                // Separator + older history only if there is a previous command
                let has_prev_command = idx > 0 && state.messages[..idx]
                    .iter()
                    .any(|m| m.text.starts_with("$ "));
                if has_prev_command {
                    // Separator before older history
                    lines.push(Line::from(Span::styled("──────── previous ────────", Style::default().fg(Color::DarkGray))));

                    // Older history (newest-first), dimmed
                    for m in state.messages[..idx].iter().rev() {
                        let mut style = match m.emotion {
                            Emotion::Neutral => Style::default().fg(Color::Gray),
                            Emotion::Happy => Style::default().fg(Color::Green),
                            Emotion::Sad    => Style::default().fg(Color::Blue),
                            Emotion::Alert  => Style::default().fg(Color::Red),
                        }.add_modifier(Modifier::DIM);
                        if m.text.starts_with("$ ") {
                            style = Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD | Modifier::DIM);
                        }
                        lines.push(Line::from(Span::styled(m.text.clone(), style)));
                    }
                }
            } else {
                // No commands yet: default to newest-first view
                for m in state.messages.iter() {
                    let mut style = match m.emotion {
                        Emotion::Neutral => Style::default().fg(Color::Gray),
                        Emotion::Happy => Style::default().fg(Color::Green),
                        Emotion::Sad    => Style::default().fg(Color::Blue),
                        Emotion::Alert  => Style::default().fg(Color::Red),
                    };
                    if m.text.starts_with("$ ") {
                        style = Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD);
                    }
                    lines.push(Line::from(Span::styled(m.text.clone(), style)));
                }
                if state.typing {
                    let dots = ["·  ", "·· ", "···"][(frame as usize / 10) % 3];
                    lines.push(Line::from(Span::styled(format!("thinking {}", dots), Style::default().fg(Color::DarkGray))));
                }
            }
            // Auto-anchor to the bottom of latest group; Up/PageUp scroll upwards into history
            let total_lines = lines.len() as u16;
            let content_height = chunks[1].height.saturating_sub(2); // minus borders
            let latest_len_u16 = (latest_len as u16).min(total_lines);
            let base_from_top = latest_len_u16.saturating_sub(content_height);
            let effective_from_top = base_from_top.saturating_sub(state.scroll);

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
                        state.messages.push(Message { text: format!("$ {}", line), emotion: Emotion::Neutral });
                        state.typing = true;
                        state.pending_llm = state.pending_llm.saturating_add(1);
                        state.scroll = 0; // anchor to latest group bottom
                        on_submit(line); // no borrowing of state inside the callback
                    }
                    KeyCode::Esc => break,
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
