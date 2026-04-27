use anyhow::{Context, Result, bail};
use clap::Parser;
use crossterm::cursor::{Hide, Show};
use crossterm::event::{Event, EventStream, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use crossterm::execute;
use crossterm::terminal::{self, EnterAlternateScreen, LeaveAlternateScreen};
use futures_util::{SinkExt, StreamExt};
use opensessions_sidebar::app::{App, PanelFocus};
use opensessions_sidebar::cli::Args;
use opensessions_sidebar::client::{
    connect_ws, connect_ws_path, decode_server_message, encode_client_command, validate_hello,
};
use opensessions_sidebar::generated::protocol::ServerMessage;
use opensessions_sidebar::renderer::render_app;
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use std::io::{self, Write};
use tokio_websockets::Message;

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    let args = Args::parse();
    if try_rendered_sidebar(&args).await? {
        return Ok(());
    }

    let mut ws = connect_ws(&args.server_host, args.server_port)
        .await
        .with_context(|| format!("connect ws://{}:{}/", args.server_host, args.server_port))?;

    let first = ws.next().await.context("read protocol hello")??;
    if !first.is_text() {
        bail!("expected text hello frame");
    }
    let hello = decode_server_message(first.as_payload())?;
    validate_hello(&hello).map_err(anyhow::Error::msg)?;

    let mut terminal = TerminalGuard::enter()?;
    let mut events = EventStream::new();
    let mut app: Option<App> = None;

    loop {
        tokio::select! {
            biased;

            event = events.next() => {
                let Some(event) = event else {
                    return Ok(());
                };
                let event = event?;
                if let Event::Key(key) = event {
                    if key.kind != KeyEventKind::Press {
                        continue;
                    }
                    if key.modifiers.contains(KeyModifiers::CONTROL)
                        && matches!(key.code, KeyCode::Char('c'))
                    {
                        return Ok(());
                    }
                    let Some(app) = &mut app else {
                        continue;
                    };

                    handle_key(app, key);
                    for command in app.drain_commands() {
                        let should_exit = matches!(
                            command,
                            opensessions_sidebar::generated::protocol::ClientCommand::Quit
                        );
                        ws.send(Message::text(encode_client_command(&command)?)).await?;
                        if should_exit {
                            return Ok(());
                        }
                    }
                    terminal.draw(app)?;
                } else if let Event::Resize(_, _) = event {
                    if let Some(app) = &mut app {
                        terminal.draw(app)?;
                    }
                }
            }

            message = ws.next() => {
                let Some(message) = message else {
                    return Ok(());
                };
                let message = message?;
                if message.is_close() {
                    return Ok(());
                }
                if message.is_text() {
                    let decoded = decode_server_message(message.as_payload())?;
                    if matches!(decoded, ServerMessage::Quit) {
                        return Ok(());
                    }
                    match (&mut app, decoded) {
                        (slot @ None, ServerMessage::State(state)) => {
                            *slot = Some(App::from_state(state))
                        }
                        (Some(app), message) => app.apply_server_message(message),
                        (None, _) => {}
                    }
                    if let Some(app) = &mut app {
                        terminal.draw(app)?;
                    }
                }
            }
        }
    }
}

async fn try_rendered_sidebar(args: &Args) -> Result<bool> {
    let (width, height) = terminal::size().unwrap_or((35, 56));
    let path = format!("/rendered-sidebar?width={width}&height={height}");
    let Ok(mut ws) = connect_ws_path(&args.server_host, args.server_port, &path).await else {
        return Ok(false);
    };

    let Some(first) = ws.next().await else {
        return Ok(false);
    };
    let first = first?;
    if !first.is_text() {
        return Ok(false);
    }
    let first_payload = first.as_text().unwrap_or_default();
    if first_payload.trim_start().starts_with('{') {
        return Ok(false);
    }

    let mut terminal = RawTerminalGuard::enter()?;
    terminal.write_frame(first_payload)?;
    let mut events = EventStream::new();

    loop {
        tokio::select! {
            biased;

            event = events.next() => {
                let Some(event) = event else {
                    return Ok(true);
                };
                match event? {
                    Event::Key(key) if key.kind == KeyEventKind::Press => {
                        if key.modifiers.contains(KeyModifiers::CONTROL)
                            && matches!(key.code, KeyCode::Char('c'))
                        {
                            return Ok(true);
                        }
                        if let Some(command) = render_key_command(key) {
                            ws.send(Message::text(command)).await?;
                        }
                    }
                    Event::Resize(width, height) => {
                        ws.send(Message::text(format!(
                            r#"{{"type":"render-resize","width":{width},"height":{height}}}"#
                        ))).await?;
                    }
                    _ => {}
                }
            }

            message = ws.next() => {
                let Some(message) = message else {
                    return Ok(true);
                };
                let message = message?;
                if message.is_close() {
                    return Ok(true);
                }
                if message.is_text() {
                    let payload = message.as_text().unwrap_or_default();
                    if matches!(serde_json::from_str::<ServerMessage>(payload), Ok(ServerMessage::Quit)) {
                        return Ok(true);
                    }
                    terminal.write_frame(payload)?;
                }
            }
        }
    }
}

fn render_key_command(key: KeyEvent) -> Option<String> {
    let key_name = match key.code {
        KeyCode::Char(ch) => ch.to_string(),
        KeyCode::Up => "up".to_string(),
        KeyCode::Down => "down".to_string(),
        KeyCode::Tab => "tab".to_string(),
        KeyCode::BackTab => "tab".to_string(),
        KeyCode::Enter => "enter".to_string(),
        KeyCode::Esc => "esc".to_string(),
        _ => return None,
    };
    let shift = key.modifiers.contains(KeyModifiers::SHIFT) || matches!(key.code, KeyCode::BackTab);
    let command = serde_json::json!({
        "type": "render-key",
        "key": key_name,
        "alt": key.modifiers.contains(KeyModifiers::ALT),
        "ctrl": key.modifiers.contains(KeyModifiers::CONTROL),
        "shift": shift,
    });
    Some(command.to_string())
}

fn handle_key(app: &mut App, key: KeyEvent) {
    if key.modifiers.contains(KeyModifiers::ALT) {
        match key.code {
            KeyCode::Up => app.reorder_focused_session(-1),
            KeyCode::Down => app.reorder_focused_session(1),
            _ => {}
        }
    } else if key.modifiers.contains(KeyModifiers::CONTROL) {
        match key.code {
            KeyCode::Char('j') => app.focus_agents_panel(),
            KeyCode::Char('k') => app.focus_sessions_panel(),
            _ => {}
        }
    } else {
        match key.code {
            KeyCode::Char('j') | KeyCode::Down => {
                if app.panel_focus == PanelFocus::Agents {
                    app.move_agent_focus(1);
                } else {
                    app.move_focus(1);
                }
            }
            KeyCode::Char('k') | KeyCode::Up => {
                if app.panel_focus == PanelFocus::Agents {
                    app.move_agent_focus(-1);
                } else {
                    app.move_focus(-1);
                }
            }
            KeyCode::Char(ch) => app.handle_key_char(ch),
            KeyCode::Tab => app.handle_tab(false),
            KeyCode::BackTab => app.handle_tab(true),
            KeyCode::Enter => app.activate_focused_item(),
            KeyCode::Esc => app.focus_sessions_panel(),
            _ => {}
        }
    }
}

struct TerminalGuard {
    terminal: Terminal<CrosstermBackend<io::Stdout>>,
}

struct RawTerminalGuard {
    stdout: io::Stdout,
}

impl RawTerminalGuard {
    fn enter() -> Result<Self> {
        terminal::enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen, Hide)?;
        Ok(Self { stdout })
    }

    fn write_frame(&mut self, frame: &str) -> Result<()> {
        self.stdout.write_all(b"\x1b[H")?;
        for line in frame.split_inclusive('\n') {
            let line = line.strip_suffix('\n').unwrap_or(line);
            self.stdout.write_all(line.as_bytes())?;
            self.stdout.write_all(b"\x1b[K\r\n")?;
        }
        self.stdout.write_all(b"\x1b[J")?;
        self.stdout.flush()?;
        Ok(())
    }
}

impl Drop for RawTerminalGuard {
    fn drop(&mut self) {
        let _ = execute!(self.stdout, Show, LeaveAlternateScreen);
        let _ = terminal::disable_raw_mode();
    }
}

impl TerminalGuard {
    fn enter() -> Result<Self> {
        terminal::enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen)?;
        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend)?;
        terminal.hide_cursor()?;
        Ok(Self { terminal })
    }

    fn draw(&mut self, app: &App) -> Result<()> {
        self.terminal.draw(|frame| render_app(frame, app))?;
        Ok(())
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = self.terminal.show_cursor();
        let _ = execute!(self.terminal.backend_mut(), Show, LeaveAlternateScreen);
        let _ = terminal::disable_raw_mode();
    }
}
