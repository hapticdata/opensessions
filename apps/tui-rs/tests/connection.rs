use opensessions_sidebar::client::{
    EXPECTED_PROTOCOL_VERSION, decode_server_message, validate_hello,
};
use opensessions_sidebar::generated::protocol::{ProtocolHello, ServerMessage};

#[test]
fn accepts_matching_protocol_hello() {
    let hello = ServerMessage::Hello(ProtocolHello {
        protocol: EXPECTED_PROTOCOL_VERSION,
        server_version: "0.2.0-alpha.5".into(),
    });
    assert!(validate_hello(&hello).is_ok());
}

#[test]
fn rejects_mismatched_protocol_hello() {
    let hello = ServerMessage::Hello(ProtocolHello {
        protocol: EXPECTED_PROTOCOL_VERSION + 1,
        server_version: "future".into(),
    });
    let err = validate_hello(&hello).unwrap_err();
    assert!(err.contains("unsupported protocol"));
}

#[test]
fn rejects_non_hello_first_message() {
    let err = validate_hello(&ServerMessage::Quit).unwrap_err();
    assert!(err.contains("expected hello"));
}

#[test]
fn decodes_text_server_message_payload() {
    let msg =
        decode_server_message(br#"{"type":"hello","protocol":1,"serverVersion":"0.2.0-alpha.5"}"#)
            .unwrap();
    assert!(matches!(msg, ServerMessage::Hello(_)));
}
