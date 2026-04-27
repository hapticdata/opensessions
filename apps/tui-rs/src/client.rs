use anyhow::Result;
use http::Uri;
use tokio::net::TcpStream;
use tokio_websockets::{ClientBuilder, MaybeTlsStream, WebSocketStream};

use crate::generated::protocol::{ClientCommand, ServerMessage};

pub const EXPECTED_PROTOCOL_VERSION: u16 = 1;

pub fn validate_hello(msg: &ServerMessage) -> std::result::Result<(), String> {
    let ServerMessage::Hello(hello) = msg else {
        return Err("expected hello as first server message".to_string());
    };

    if hello.protocol != EXPECTED_PROTOCOL_VERSION {
        return Err(format!(
            "unsupported protocol {}, expected {}",
            hello.protocol, EXPECTED_PROTOCOL_VERSION
        ));
    }

    Ok(())
}

pub fn decode_server_message(payload: &[u8]) -> serde_json::Result<ServerMessage> {
    serde_json::from_slice(payload)
}

pub fn encode_client_command(command: &ClientCommand) -> serde_json::Result<String> {
    serde_json::to_string(command)
}

pub async fn connect_ws(
    host: &str,
    port: u16,
) -> Result<WebSocketStream<MaybeTlsStream<TcpStream>>> {
    let uri: Uri = format!("ws://{host}:{port}/").parse()?;
    let (ws, _) = ClientBuilder::from_uri(uri).connect().await?;
    Ok(ws)
}
