use anyhow::{bail, Context, Result};
use clap::Parser;
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use futures_util::StreamExt;
use opensessions_sidebar::app::App;
use opensessions_sidebar::cli::Args;
use opensessions_sidebar::client::{connect_ws, decode_server_message, validate_hello};
use opensessions_sidebar::generated::protocol::ServerMessage;
use opensessions_sidebar::snapshot::{buffer_to_ansi, render_to_buffer};
use std::time::Duration;

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    let args = Args::parse();
    let mut ws = connect_ws(&args.server_host, args.server_port).await
        .with_context(|| format!("connect ws://{}:{}/", args.server_host, args.server_port))?;

    let first = ws.next().await.context("read protocol hello")??;
    if !first.is_text() {
        bail!("expected text hello frame");
    }
    let hello = decode_server_message(first.as_payload())?;
    validate_hello(&hello).map_err(anyhow::Error::msg)?;

    loop {
        let msg = ws.next().await.context("read server state")??;
        if msg.is_close() {
            return Ok(());
        }
        if !msg.is_text() {
            continue;
        }

        match decode_server_message(msg.as_payload())? {
            ServerMessage::State(state) => {
                let mut app = App::from_state(state);
                let rendered = render_to_buffer(&mut app, 35, 56);
                print!("{}", buffer_to_ansi(&rendered));
                wait_for_q()?;
                return Ok(());
            }
            ServerMessage::Quit => return Ok(()),
            _ => {}
        }
    }
}

fn wait_for_q() -> Result<()> {
    loop {
        if !event::poll(Duration::from_millis(100))? {
            continue;
        }

        if let Event::Key(key) = event::read()? {
            if key.kind == KeyEventKind::Press && key.code == KeyCode::Char('q') {
                return Ok(());
            }
        }
    }
}
