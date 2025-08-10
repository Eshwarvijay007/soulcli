use std::sync::mpsc::Receiver;
use std::time::Duration;
use crossterm::event::{self, Event, KeyCode};
use ratatui::{
    backend::CrosstermBackend, Terminal,
    layout::{Layout, Constraint, Direction},
    widgets::{Block, Borders, Paragraph, Wrap, Clear},
    style::{Style, Color},
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
    mood: Emotion,
    scroll: u16,
}

impl UiState {
    fn new() -> Self {
        Self { input: String::new(), messages: vec![], typing: false, mood: Emotion::Neutral, scroll: 0 }
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
                    state.typing = false;                 // stop LLM spinner only
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

            // Messages
            let mut lines: Vec<Line> = Vec::with_capacity(state.messages.len() + 1);
            for m in &state.messages {
                let color = match m.emotion {
                    Emotion::Neutral => Color::Gray,
                    Emotion::Happy => Color::Green,
                    Emotion::Sad    => Color::Blue,
                    Emotion::Alert  => Color::Red,
                };
                lines.push(Line::from(Span::styled(m.text.clone(), Style::default().fg(color))));
            }
            if state.typing {
                let dots = ["·  ", "·· ", "···"][(frame as usize / 10) % 3];
                lines.push(Line::from(Span::styled(format!("thinking {}", dots), Style::default().fg(Color::DarkGray))));
            }
            let dialog = Paragraph::new(lines)
                .wrap(Wrap { trim: true })
                .scroll((state.scroll, 0))
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
                        on_submit(line); // no borrowing of state inside the callback
                    }
                    KeyCode::Esc => break,
                    KeyCode::Up => state.scroll = state.scroll.saturating_sub(1),
                    KeyCode::Down => state.scroll = state.scroll.saturating_add(1),
                    KeyCode::PageUp => state.scroll = state.scroll.saturating_sub(5),
                    KeyCode::PageDown => state.scroll = state.scroll.saturating_add(5),
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
