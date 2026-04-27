use opensessions_sidebar::cli::Args;

#[test]
fn cli_defaults_match_typescript_runtime() {
    let args = Args::try_parse_from(["opensessions-sidebar"]).unwrap();
    assert_eq!(args.server_host, "127.0.0.1");
    assert_eq!(args.server_port, 7_391);
}

#[test]
fn cli_accepts_explicit_server_endpoint() {
    let args = Args::try_parse_from([
        "opensessions-sidebar",
        "--server-host",
        "0.0.0.0",
        "--server-port",
        "8123",
    ])
    .unwrap();
    assert_eq!(args.server_host, "0.0.0.0");
    assert_eq!(args.server_port, 8_123);
}
