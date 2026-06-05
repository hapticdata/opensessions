use std::fs::{self, File};
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, MutexGuard, OnceLock};
use std::thread::sleep;
use std::time::{Duration, Instant};

const W: &str = "36";
const SIDEBAR_SESSIONS: &[&str] = &[
    "opensessions",
    "effect-ts",
    "lazydiff",
    "os-demo-feat-agent-panel",
    "os-demo-preview",
];

#[test]
fn tmux_sidebar_keyboard_focus_and_worktree_flow() {
    let _guard = e2e_serial_guard();
    let lab = started_lab("opensessions-e2e-focus");

    lab.wait_for_text("opensessions", "os-demo-worktrees");
    lab.wait_for_text("effect-ts", "effect-ts");

    let source = lab.sidebar_pane("opensessions");
    let tab_destination = lab.sidebar_pane("os-demo-feat-agent-panel");
    lab.tmux_ok(["select-pane", "-t", source.as_str()]);
    lab.tmux_ok(["switch-client", "-t", "opensessions"]);
    sleep(Duration::from_millis(250));

    lab.tmux_ok(["send-keys", "-t", source.as_str(), "Tab"]);
    lab.wait_for_client_session("os-demo-feat-agent-panel");
    lab.wait_for_capture("os-demo-feat-agent-panel", |text| {
        row_with(text, "os-demo-feat-agent-panel").is_some_and(|row| row.contains("▌"))
    });

    let source_after_tab = lab.capture_pane(&source);
    assert!(
        row_with(&source_after_tab, "opensessions")
            .is_some_and(|row| row.trim_start().starts_with("▌")),
        "old source sidebar should rehome to its own confirmed active session after settled state; got:\n{source_after_tab}",
    );
    let effect_after_tab = lab.capture_pane(&tab_destination);
    assert!(
        row_with(&effect_after_tab, "os-demo-feat-agent-panel")
            .is_some_and(|row| row.contains("▌")),
        "destination sidebar should focus the destination concrete session; got:\n{effect_after_tab}",
    );

    let worktree_source = lab.sidebar_pane("os-demo-feat-agent-panel");
    let worktree_dest = lab.sidebar_pane("os-demo-preview");
    lab.tmux_ok(["switch-client", "-t", "os-demo-feat-agent-panel"]);
    lab.tmux_ok(["select-pane", "-t", worktree_source.as_str()]);
    lab.wait_for_client_session("os-demo-feat-agent-panel");
    sleep(Duration::from_millis(250));

    lab.tmux_ok(["send-keys", "-t", worktree_source.as_str(), "Up"]);
    lab.wait_for_capture_pane(&worktree_source, |text| {
        row_with(text, "os-demo-worktrees").is_some_and(|row| row.trim_start().starts_with("›"))
    });
    lab.tmux_ok(["send-keys", "-t", worktree_source.as_str(), "Enter"]);
    lab.wait_for_capture_pane(&worktree_source, |text| {
        text.contains("▸ os-demo-worktrees")
    });
    lab.tmux_ok(["send-keys", "-t", worktree_source.as_str(), "Enter"]);
    lab.wait_for_capture_pane(&worktree_source, |text| {
        text.contains("▾ os-demo-worktrees")
    });

    lab.tmux_ok(["send-keys", "-t", worktree_source.as_str(), "Down"]);
    lab.tmux_ok(["send-keys", "-t", worktree_source.as_str(), "Down"]);
    lab.wait_for_capture_pane(&worktree_source, |text| {
        row_with(text, "os-demo-preview").is_some_and(|row| row.contains("›"))
    });
    lab.tmux_ok(["send-keys", "-t", worktree_source.as_str(), "Enter"]);
    lab.wait_for_client_session("os-demo-preview");
    lab.wait_for_capture_pane(&worktree_dest, |text| {
        row_with(text, "os-demo-preview").is_some_and(|row| row.contains("▌"))
            && !row_with(text, "os-demo-worktrees")
                .is_some_and(|row| row.trim_start().starts_with("›"))
    });

    let destination = lab.capture_pane(&worktree_dest);
    assert!(
        row_with(&destination, "os-demo-preview").is_some_and(|row| row.contains("▌")),
        "destination worktree child should own active/focused row; got:\n{destination}",
    );
    assert!(
        !row_with(&destination, "os-demo-worktrees")
            .is_some_and(|row| row.trim_start().starts_with("›")),
        "worktree group header must not remain focused after switching to concrete child; got:\n{destination}",
    );
}

#[test]
fn tmux_sidebar_width_is_fixed_and_rejects_manual_sidebar_resize() {
    let _guard = e2e_serial_guard();
    let lab = started_lab("opensessions-e2e-width");
    let source = lab.sidebar_pane("opensessions");
    lab.tmux_ok(["switch-client", "-t", "opensessions"]);
    lab.tmux_ok(["select-pane", "-t", source.as_str()]);
    lab.wait_for_all_sidebar_widths(36);
    sleep(Duration::from_millis(1500));

    lab.tmux_ok(["resize-pane", "-t", source.as_str(), "-x", "40"]);
    sleep(Duration::from_millis(250));
    lab.tmux_ok(["resize-pane", "-t", source.as_str(), "-x", "42"]);
    sleep(Duration::from_millis(250));
    lab.tmux_ok(["resize-pane", "-t", source.as_str(), "-x", "1"]);

    lab.wait_for_all_sidebar_widths(36);
}

#[test]
fn tmux_sidebar_width_slider_is_the_only_width_author() {
    let _guard = e2e_serial_guard();
    let mut lab = started_lab("opensessions-e2e-width-slider");
    let source = lab.sidebar_pane("opensessions");
    lab.tmux_ok(["switch-client", "-t", "opensessions"]);
    lab.tmux_ok(["select-pane", "-t", source.as_str()]);
    lab.wait_for_all_sidebar_widths(36);

    lab.tmux_ok(["send-keys", "-t", source.as_str(), "w"]);
    lab.wait_for_capture_pane(&source, |text| text.contains("Sidebar width"));
    lab.tmux_ok(["send-keys", "-t", source.as_str(), "Right"]);
    lab.tmux_ok(["send-keys", "-t", source.as_str(), "Right"]);
    lab.wait_for_all_sidebar_widths(38);
    assert_eq!(
        lab.tmux(["show-option", "-gqv", "@opensessions_width"]),
        "38"
    );
    lab.tmux_ok(["send-keys", "-t", source.as_str(), "Enter"]);

    lab.tmux_ok(["resize-pane", "-t", source.as_str(), "-x", "50"]);
    lab.wait_for_all_sidebar_widths(38);

    lab.tmux_ok(["send-keys", "-t", source.as_str(), "w"]);
    lab.wait_for_capture_pane(&source, |text| text.contains("Sidebar width"));
    for _ in 0..4 {
        lab.tmux_ok(["send-keys", "-t", source.as_str(), "H"]);
    }
    lab.wait_for_all_sidebar_widths(20);
    assert_eq!(
        lab.tmux(["show-option", "-gqv", "@opensessions_width"]),
        "20"
    );
    lab.tmux_ok(["send-keys", "-t", source.as_str(), "Enter"]);

    lab.tmux_ok(["send-keys", "-t", source.as_str(), "w"]);
    lab.wait_for_capture_pane(&source, |text| {
        text.contains("Sidebar width") && text.contains("20 columns")
    });
    lab.tmux_ok(["send-keys", "-t", source.as_str(), "Right"]);
    lab.wait_for_all_sidebar_widths(21);
    lab.tmux_ok(["send-keys", "-t", source.as_str(), "Esc"]);

    lab.wait_for_config_sidebar_width(21);

    lab.restart_server();
    lab.wait_for_all_sidebar_widths(21);
    assert_eq!(
        lab.tmux(["show-option", "-gqv", "@opensessions_width"]),
        "21"
    );
}

#[test]
fn tmux_sidebar_quit_closes_the_server_and_every_sidebar_client() {
    let _guard = e2e_serial_guard();
    let mut lab = started_lab("opensessions-e2e-quit");
    let source = lab.sidebar_pane("opensessions");
    lab.tmux_ok(["switch-client", "-t", "opensessions"]);
    lab.tmux_ok(["select-pane", "-t", source.as_str()]);

    lab.tmux_ok(["send-keys", "-t", source.as_str(), "q"]);

    lab.wait_for_server_exit();
    lab.wait_for_no_sidebar_processes();
}

#[test]
fn tmux_sidebar_stays_closed_across_session_and_window_switch_hooks() {
    let _guard = e2e_serial_guard();
    let lab = started_lab("opensessions-e2e-open-close");
    lab.wait_for_all_sidebar_widths(36);

    post_hook(lab.port, "/toggle");
    lab.wait_for_no_sidebar_processes();

    lab.tmux_ok(["switch-client", "-t", "effect-ts"]);
    sleep(Duration::from_millis(800));
    lab.assert_no_sidebar_panes("closed sidebar must not respawn after session switch");

    lab.tmux_ok(["new-window", "-d", "-t", "effect-ts", "sh"]);
    lab.tmux_ok(["select-window", "-t", "effect-ts:1"]);
    sleep(Duration::from_millis(800));
    lab.assert_no_sidebar_panes("closed sidebar must not respawn after window switch");
}

#[test]
fn tmux_sidebar_multiple_clients_keep_independent_active_rows() {
    let _guard = e2e_serial_guard();
    let mut lab = started_lab("opensessions-e2e-multiclient");
    lab.spawn_attached_client_for("effect-ts");
    lab.wait_for_client_sessions(["opensessions", "effect-ts"]);

    let opensessions = lab.capture_pane(&lab.sidebar_pane("opensessions"));
    let effect = lab.capture_pane(&lab.sidebar_pane("effect-ts"));

    assert_active_row(&opensessions, "opensessions");
    assert_active_row(&effect, "effect-ts");
}

#[test]
fn tmux_sidebar_state_is_isolated_per_tmux_socket() {
    let _guard = e2e_serial_guard();
    let lab_a = started_lab("opensessions-e2e-socket-a");
    let lab_b = started_lab("opensessions-e2e-socket-b");

    let source = lab_a.sidebar_pane("opensessions");
    lab_a.tmux_ok(["switch-client", "-t", "opensessions"]);
    lab_a.tmux_ok(["select-pane", "-t", source.as_str()]);
    lab_a.wait_for_all_sidebar_widths(36);
    lab_b.wait_for_all_sidebar_widths(36);
    sleep(Duration::from_millis(1500));

    lab_a.tmux_ok(["resize-pane", "-t", source.as_str(), "-x", "40"]);
    sleep(Duration::from_millis(250));
    lab_a.tmux_ok(["resize-pane", "-t", source.as_str(), "-x", "42"]);

    lab_a.wait_for_all_sidebar_widths(36);
    lab_b.wait_for_all_sidebar_widths(36);
    assert_ne!(
        lab_a.port, lab_b.port,
        "isolated servers must use distinct ports"
    );
}

#[test]
fn tmux_sidebar_q_in_main_pane_does_not_quit_opensessions() {
    let _guard = e2e_serial_guard();
    let mut lab = started_lab("opensessions-e2e-q-main-pane");
    let main = lab.main_pane("opensessions");
    lab.tmux_ok(["switch-client", "-t", "opensessions"]);
    lab.tmux_ok(["select-pane", "-t", main.as_str()]);
    lab.tmux_ok(["send-keys", "-t", main.as_str(), "q"]);
    sleep(Duration::from_millis(700));

    assert!(
        lab.server_is_running(),
        "server exited after q in main pane"
    );
    assert_eq!(lab.sidebar_panes().len(), SIDEBAR_SESSIONS.len());
}

#[test]
fn tmux_sidebar_pane_exit_does_not_steal_sidebar_width() {
    let _guard = e2e_serial_guard();
    let lab = started_lab("opensessions-e2e-pane-exit");
    let sidebar = lab.sidebar_pane("opensessions");
    let main = lab.main_pane("opensessions");
    lab.tmux_ok(["switch-client", "-t", "opensessions"]);
    lab.tmux_ok(["select-pane", "-t", sidebar.as_str()]);
    lab.wait_for_all_sidebar_widths(36);

    lab.tmux_ok(["split-window", "-h", "-t", main.as_str(), "sh"]);
    lab.wait_for_non_sidebar_pane_count("opensessions", 2);
    lab.tmux_ok(["kill-pane", "-t", main.as_str()]);

    lab.wait_for_non_sidebar_pane_count("opensessions", 1);
    lab.wait_for_all_sidebar_widths(36);
}

#[test]
fn tmux_sidebar_width_survives_flat_three_pane_layout_churn() {
    let _guard = e2e_serial_guard();
    let lab = started_lab("opensessions-e2e-flat-pane-churn");
    let sidebar = lab.sidebar_pane("opensessions");
    let main = lab.main_pane("opensessions");
    lab.tmux_ok(["switch-client", "-t", "opensessions"]);
    lab.tmux_ok(["select-pane", "-t", sidebar.as_str()]);
    lab.wait_for_all_sidebar_widths(36);

    lab.tmux_ok(["split-window", "-h", "-t", main.as_str(), "sh"]);
    lab.wait_for_non_sidebar_pane_count("opensessions", 2);
    lab.tmux_ok(["select-layout", "-t", "opensessions", "even-horizontal"]);
    lab.wait_for_sidebar_width("opensessions", 36);

    let content_panes = lab.non_sidebar_panes("opensessions");
    assert_eq!(
        content_panes.len(),
        2,
        "expected sidebar | pane1 | pane2 before churn; got {content_panes:?}"
    );
    lab.tmux_ok(["kill-pane", "-t", content_panes[0].as_str()]);
    let immediate_width = lab.sidebar_width("opensessions");
    assert_eq!(
        immediate_width,
        Some(36),
        "sidebar width changed immediately after killing pane1 in `sidebar | pane1 | pane2`; panes={:?}\nwidth_option={}\nhooks:\n{}\nlogs:\n{}",
        lab.sidebar_panes(),
        lab.tmux(["show-option", "-gqv", "@opensessions_width"]),
        lab.tmux(["show-hooks", "-g"]),
        lab.logs(),
    );

    lab.wait_for_non_sidebar_pane_count("opensessions", 1);
    lab.wait_for_sidebar_width("opensessions", 36);
    lab.wait_for_all_sidebar_widths(36);
}

#[test]
fn tmux_sidebar_client_resize_never_persists_a_smaller_sidebar_width() {
    let _guard = e2e_serial_guard();
    let lab = started_lab("opensessions-e2e-client-resize-fixed-width");
    let sidebar = lab.sidebar_pane("opensessions");
    lab.tmux_ok(["switch-client", "-t", "opensessions"]);
    lab.tmux_ok(["select-pane", "-t", sidebar.as_str()]);
    lab.wait_for_all_sidebar_widths(36);

    lab.tmux_ok([
        "resize-window",
        "-t",
        "opensessions",
        "-x",
        "80",
        "-y",
        "40",
    ]);
    sleep(Duration::from_millis(600));
    lab.tmux_ok([
        "resize-window",
        "-t",
        "opensessions",
        "-x",
        "160",
        "-y",
        "40",
    ]);
    lab.tmux_ok([
        "set-window-option",
        "-t",
        "opensessions",
        "window-size",
        "latest",
    ]);

    lab.wait_for_sidebar_width("opensessions", 36);
    lab.wait_for_all_sidebar_widths(36);
}

#[test]
fn tmux_sidebar_switch_stays_responsive_with_100_connected_clients() {
    let _guard = e2e_serial_guard();
    let lab = started_lab("opensessions-e2e-100-clients");
    let source = lab.sidebar_pane("opensessions");
    lab.tmux_ok(["switch-client", "-t", "opensessions"]);
    lab.tmux_ok(["select-pane", "-t", source.as_str()]);
    lab.wait_for_client_session("opensessions");

    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_io()
        .enable_time()
        .build()
        .expect("build e2e tokio runtime");

    runtime.block_on(async {
        let mut clients = Vec::new();
        for index in 0..100 {
            let ws = opensessions_sidebar::client::connect_ws("127.0.0.1", lab.port)
                .await
                .unwrap_or_else(|err| panic!("connect passive ws client {index}: {err}"));
            clients.push(ws);
        }

        for _ in 0..25 {
            post_refresh(lab.port);
        }

        let started = Instant::now();
        lab.tmux_ok(["send-keys", "-t", source.as_str(), "Tab"]);
        lab.wait_for_client_session("os-demo-feat-agent-panel");
        let elapsed = started.elapsed();
        assert!(
            elapsed < Duration::from_secs(2),
            "switch took {elapsed:?} with 100 connected sidebar clients"
        );

        drop(clients);
    });
}

#[test]
fn tmux_sidebar_switch_latency_during_width_repair_probe() {
    let _guard = e2e_serial_guard();
    let lab = started_lab("os-e2e-latency");
    for session in SIDEBAR_SESSIONS {
        for index in 0..9 {
            lab.spawn_window_with_sidebar(session, &format!("extra-{index}"));
        }
    }
    lab.wait_for_sidebar_pane_count(SIDEBAR_SESSIONS.len() * 10);
    lab.wait_for_all_sidebar_widths(36);
    sleep(Duration::from_millis(1500));

    let baseline_source = lab.sidebar_pane("opensessions");
    lab.tmux_ok(["switch-client", "-t", "opensessions"]);
    lab.tmux_ok(["select-pane", "-t", baseline_source.as_str()]);
    let baseline = lab.measure_tab_switch("opensessions", &baseline_source);

    let resize_source = lab.sidebar_pane("opensessions");
    lab.tmux_ok(["switch-client", "-t", "opensessions"]);
    lab.wait_for_client_session("opensessions");
    lab.tmux_ok(["select-pane", "-t", resize_source.as_str()]);
    sleep(Duration::from_millis(300));
    lab.tmux_ok(["resize-pane", "-t", resize_source.as_str(), "-x", "42"]);
    lab.wait_for_all_sidebar_widths(36);
    let during_resize = lab.measure_tab_switch("opensessions", &resize_source);

    eprintln!(
        "switch latency probe: sidebars={} baseline_ms={} during_resize_ms={}",
        lab.sidebar_panes().len(),
        baseline.as_millis(),
        during_resize.as_millis(),
    );

    assert!(
        during_resize <= baseline + Duration::from_millis(250),
        "switch during width repair should stay close to baseline; baseline={baseline:?} during_resize={during_resize:?} panes={:?}\nlogs:\n{}",
        lab.sidebar_panes(),
        lab.logs(),
    );
}

fn started_lab(prefix: &str) -> Lab {
    Command::new("tmux")
        .arg("-V")
        .output()
        .expect("tmux is required for product E2E tests");
    Command::new("python3")
        .arg("--version")
        .output()
        .expect("python3 is required for product E2E tests");
    Command::new("git")
        .arg("--version")
        .output()
        .expect("git is required for product E2E tests");

    let mut lab = Lab::new(prefix);
    lab.setup_repos();
    lab.setup_tmux();
    lab.start_server();
    lab.spawn_sidebars();
    lab
}

fn e2e_serial_guard() -> MutexGuard<'static, ()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn row_with<'a>(text: &'a str, needle: &str) -> Option<&'a str> {
    text.lines().find(|line| line.contains(needle))
}

fn post_refresh(port: u16) {
    post_hook(port, "/refresh");
}

fn post_hook(port: u16, path: &str) {
    let mut stream = TcpStream::connect(("127.0.0.1", port)).expect("connect /refresh");
    let request = format!(
        "POST {path} HTTP/1.1\r\nHost: 127.0.0.1\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
    );
    stream
        .write_all(request.as_bytes())
        .expect("write /refresh");
    let mut response = [0; 128];
    let _ = stream.read(&mut response);
}

fn assert_active_row(capture: &str, session: &str) {
    assert!(
        row_with(capture, session).is_some_and(|row| row.contains("▌")),
        "expected {session} to be the active row; got:\n{capture}",
    );
}

fn free_port() -> u16 {
    TcpListener::bind(("127.0.0.1", 0))
        .expect("bind ephemeral e2e port")
        .local_addr()
        .expect("read ephemeral e2e port")
        .port()
}

struct Lab {
    socket: String,
    root: PathBuf,
    port: u16,
    server: Option<Child>,
    clients: Vec<Child>,
}

impl Lab {
    fn new(prefix: &str) -> Self {
        static LAB_ID: AtomicU64 = AtomicU64::new(1);
        let id = LAB_ID.fetch_add(1, Ordering::Relaxed);
        let unique = format!("{}-{}-{}", prefix, std::process::id(), id,);
        let root = std::env::temp_dir().join(&unique);
        fs::create_dir_all(&root).expect("create e2e root");
        let config_dir = root.join("home/.config/opensessions");
        fs::create_dir_all(&config_dir).expect("create e2e config dir");
        fs::write(
            config_dir.join("config.json"),
            format!("{{\"plugins\":[],\"sidebarWidth\":{W}}}\n"),
        )
        .expect("write e2e config");
        Self {
            socket: unique,
            root,
            port: free_port(),
            server: None,
            clients: Vec::new(),
        }
    }

    fn setup_repos(&self) {
        for name in ["opensessions", "effect-ts", "lazydiff"] {
            let dir = self.root.join(name);
            fs::create_dir_all(&dir).expect("create fake repo dir");
            self.git(&dir, ["init", "-q"]);
            self.git(&dir, ["config", "user.email", "e2e@example.com"]);
            self.git(&dir, ["config", "user.name", "OpenSessions E2E"]);
            fs::write(dir.join("README.md"), format!("{name}\n")).expect("write readme");
            self.git(&dir, ["add", "README.md"]);
            self.git(&dir, ["commit", "-q", "-m", "init"]);
        }

        let base = self.root.join("os-demo-base");
        fs::create_dir_all(&base).expect("create worktree base");
        self.git(&base, ["init", "-q"]);
        self.git(&base, ["config", "user.email", "e2e@example.com"]);
        self.git(&base, ["config", "user.name", "OpenSessions E2E"]);
        fs::write(base.join("README.md"), "os-demo\n").expect("write worktree readme");
        self.git(&base, ["add", "README.md"]);
        self.git(&base, ["commit", "-q", "-m", "init"]);
        self.git(&base, ["branch", "feat-agent-panel"]);
        self.git(&base, ["branch", "preview"]);
        fs::create_dir_all(self.root.join("os-demo-worktrees")).expect("create worktrees dir");
        self.git(
            &base,
            [
                "worktree",
                "add",
                "-q",
                self.root
                    .join("os-demo-worktrees/feat-agent-panel")
                    .to_str()
                    .unwrap(),
                "feat-agent-panel",
            ],
        );
        self.git(
            &base,
            [
                "worktree",
                "add",
                "-q",
                self.root
                    .join("os-demo-worktrees/preview")
                    .to_str()
                    .unwrap(),
                "preview",
            ],
        );
    }

    fn setup_tmux(&mut self) {
        let _ = Command::new("tmux")
            .args(["-L", &self.socket, "kill-server"])
            .output();
        for (session, dir) in [
            ("opensessions", self.root.join("opensessions")),
            ("effect-ts", self.root.join("effect-ts")),
            ("lazydiff", self.root.join("lazydiff")),
            (
                "os-demo-feat-agent-panel",
                self.root.join("os-demo-worktrees/feat-agent-panel"),
            ),
            (
                "os-demo-preview",
                self.root.join("os-demo-worktrees/preview"),
            ),
        ] {
            self.tmux_ok([
                "new-session",
                "-d",
                "-x",
                "160",
                "-y",
                "40",
                "-s",
                session,
                "-c",
                dir.to_str().unwrap(),
                "sh",
            ]);
        }

        self.spawn_attached_client_for("opensessions");
        self.wait_for_client_session("opensessions");
    }

    fn spawn_attached_client_for(&mut self, session: &str) {
        let child = self.spawn_attached_client(session);
        self.clients.push(child);
    }

    fn spawn_attached_client(&self, session: &str) -> Child {
        let script = r#"
import fcntl, os, pty, struct, sys, termios, time

socket = sys.argv[1]
session = sys.argv[2]
pid, fd = pty.fork()
if pid == 0:
    os.environ["TERM"] = "xterm-256color"
    os.execvp("tmux", ["tmux", "-L", socket, "attach-session", "-t", session])

fcntl.ioctl(fd, termios.TIOCSWINSZ, struct.pack("HHHH", 40, 160, 0, 0))
time.sleep(300)
"#;
        Command::new("python3")
            .arg("-c")
            .arg(script)
            .arg(&self.socket)
            .arg(session)
            .env("TERM", "xterm-256color")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(
                File::create(self.root.join("tmux-client.stderr.log")).expect("client stderr log"),
            )
            .spawn()
            .expect("spawn attached tmux client through script")
    }

    fn start_server(&mut self) {
        let server = self.server_bin();
        let tmux_env = self.tmux_socket_env();
        let child = Command::new(server)
            .env("TMUX", tmux_env)
            .env("HOME", self.home_dir())
            .env("OPENSESSIONS_WIDTH", "35")
            .env("OPENSESSIONS_HOST", "127.0.0.1")
            .env("OPENSESSIONS_PORT", self.port.to_string())
            .env(
                "OPENSESSIONS_DEBUG_LOG",
                self.root.join("debug.log").to_str().unwrap(),
            )
            .env(
                "OPENSESSIONS_PID_FILE",
                self.root.join("server.pid").to_str().unwrap(),
            )
            .stdout(File::create(self.root.join("server.stdout.log")).expect("server stdout log"))
            .stderr(File::create(self.root.join("server.stderr.log")).expect("server stderr log"))
            .spawn()
            .expect("start opensessions server");
        self.server = Some(child);
        self.wait_for_server();
    }

    fn restart_server(&mut self) {
        if let Some(mut server) = self.server.take() {
            let _ = server.kill();
            let _ = server.wait();
        }
        self.wait_for_no_sidebar_processes();
        self.start_server();
        self.spawn_sidebars();
    }

    fn wait_for_server(&self) {
        let deadline = Instant::now() + Duration::from_secs(10);
        while Instant::now() < deadline {
            if TcpStream::connect(("127.0.0.1", self.port)).is_ok() {
                return;
            }
            sleep(Duration::from_millis(100));
        }
        panic!("server did not become ready; logs:\n{}", self.logs());
    }

    fn spawn_sidebars(&self) {
        let sidebar = self.sidebar_bin();
        for session in SIDEBAR_SESSIONS {
            let command = format!(
                "env OPENSESSIONS_HOST=127.0.0.1 OPENSESSIONS_PORT={} OPENSESSIONS_DEBUG_LOG={} {} 2>{}",
                self.port,
                shell_quote(&self.root.join("debug.log").to_string_lossy()),
                sidebar.display(),
                shell_quote(
                    &self
                        .root
                        .join(format!("sidebar-{session}.stderr.log"))
                        .to_string_lossy()
                ),
            );
            let pane = self.tmux([
                "split-window",
                "-h",
                "-b",
                "-l",
                W,
                "-P",
                "-F",
                "#{pane_id}",
                "-t",
                *session,
                &command,
            ]);
            self.tmux_ok([
                "select-pane",
                "-t",
                pane.as_str(),
                "-T",
                "opensessions-sidebar",
            ]);
        }
        sleep(Duration::from_millis(1200));
    }

    fn spawn_window_with_sidebar(&self, session: &str, window_name: &str) -> String {
        let next_index = self.next_window_index(session);
        self.tmux_ok([
            "new-window",
            "-d",
            "-t",
            &format!("{session}:{next_index}"),
            "-n",
            window_name,
            "sh",
        ]);
        let window_index = self.tmux([
            "display-message",
            "-p",
            "-t",
            &format!("{session}:{window_name}"),
            "#{window_index}",
        ]);
        let sidebar = self.sidebar_bin();
        let command = format!(
            "env OPENSESSIONS_HOST=127.0.0.1 OPENSESSIONS_PORT={} OPENSESSIONS_DEBUG_LOG={} {} 2>{}",
            self.port,
            shell_quote(&self.root.join("debug.log").to_string_lossy()),
            sidebar.display(),
            shell_quote(
                &self
                    .root
                    .join(format!("sidebar-{session}-{window_name}.stderr.log"))
                    .to_string_lossy()
            ),
        );
        let pane = self.tmux([
            "split-window",
            "-h",
            "-b",
            "-l",
            W,
            "-P",
            "-F",
            "#{pane_id}",
            "-t",
            &format!("{session}:{window_index}"),
            &command,
        ]);
        self.tmux_ok([
            "select-pane",
            "-t",
            pane.as_str(),
            "-T",
            "opensessions-sidebar",
        ]);
        self.wait_for_capture_pane(&pane, |text| text.contains("opensessions"));
        window_index
    }

    fn wait_for_text(&self, session: &str, text: &str) {
        self.wait_for_capture(session, |capture| capture.contains(text));
    }

    fn wait_for_capture<F>(&self, session: &str, predicate: F)
    where
        F: Fn(&str) -> bool,
    {
        let pane = self.sidebar_pane(session);
        self.wait_for_capture_pane(&pane, predicate);
    }

    fn wait_for_capture_pane<F>(&self, pane: &str, predicate: F)
    where
        F: Fn(&str) -> bool,
    {
        let deadline = Instant::now() + Duration::from_secs(5);
        while Instant::now() < deadline {
            let capture = self.capture_pane(pane);
            if predicate(&capture) {
                return;
            }
            sleep(Duration::from_millis(100));
        }
        panic!(
            "timed out waiting for pane {pane}; last capture:\n{}\n\npanes:\n{}\n\nlogs:\n{}",
            self.capture_pane(pane),
            self.tmux(["list-panes", "-a", "-F", "#{session_name} #{pane_id} #{pane_width}x#{pane_height} command=#{pane_current_command} dead=#{pane_dead} status=#{pane_dead_status}"]),
            self.logs(),
        );
    }

    fn wait_for_client_session(&self, expected: &str) {
        let deadline = Instant::now() + Duration::from_secs(5);
        while Instant::now() < deadline {
            let output = self.tmux(["list-clients", "-F", "#{client_session}"]);
            if output.lines().any(|line| line.trim() == expected) {
                return;
            }
            sleep(Duration::from_millis(100));
        }
        panic!(
            "timed out waiting for client session {expected}; clients:\n{}\n\nlogs:\n{}",
            self.tmux([
                "list-clients",
                "-F",
                "#{client_name} #{client_tty} #{client_session}"
            ]),
            self.logs(),
        );
    }

    fn wait_for_client_sessions<const N: usize>(&self, expected: [&str; N]) {
        let deadline = Instant::now() + Duration::from_secs(5);
        while Instant::now() < deadline {
            let output = self.tmux(["list-clients", "-F", "#{client_session}"]);
            if expected
                .iter()
                .all(|expected| output.lines().any(|line| line.trim() == *expected))
            {
                return;
            }
            sleep(Duration::from_millis(100));
        }
        panic!(
            "timed out waiting for client sessions {expected:?}; clients:\n{}\n\nlogs:\n{}",
            self.tmux([
                "list-clients",
                "-F",
                "#{client_name} #{client_tty} #{client_session}"
            ]),
            self.logs(),
        );
    }

    fn wait_for_client_to_leave_session(&self, previous: &str) {
        let deadline = Instant::now() + Duration::from_secs(5);
        while Instant::now() < deadline {
            let output = self.tmux(["list-clients", "-F", "#{client_session}"]);
            if output
                .lines()
                .any(|line| !line.trim().is_empty() && line.trim() != previous)
            {
                return;
            }
            sleep(Duration::from_millis(100));
        }
        panic!(
            "timed out waiting for client to leave session {previous}; clients:\n{}\n\nlogs:\n{}",
            self.tmux([
                "list-clients",
                "-F",
                "#{client_name} #{client_tty} #{client_session}"
            ]),
            self.logs(),
        );
    }

    fn current_client_session(&self) -> String {
        self.tmux(["list-clients", "-F", "#{client_session}"])
            .lines()
            .find_map(|line| {
                let session = line.trim();
                (!session.is_empty()).then(|| session.to_string())
            })
            .unwrap_or_else(|| {
                panic!(
                    "no attached client session found; clients:\n{}\n\nlogs:\n{}",
                    self.tmux([
                        "list-clients",
                        "-F",
                        "#{client_name} #{client_tty} #{client_session}"
                    ]),
                    self.logs(),
                )
            })
    }

    fn wait_for_all_sidebar_widths(&self, expected: u16) {
        let deadline = Instant::now() + Duration::from_secs(8);
        while Instant::now() < deadline {
            let panes = self.sidebar_panes();
            if panes.len() >= SIDEBAR_SESSIONS.len()
                && panes.iter().all(|pane| pane.width == expected)
            {
                return;
            }
            sleep(Duration::from_millis(100));
        }
        panic!(
            "timed out waiting for all sidebar widths to be {expected}; panes={:?}\nlogs:\n{}",
            self.sidebar_panes(),
            self.logs(),
        );
    }

    fn wait_for_config_sidebar_width(&self, expected: u16) {
        let deadline = Instant::now() + Duration::from_secs(5);
        while Instant::now() < deadline {
            let config = fs::read_to_string(self.config_path()).unwrap_or_default();
            if config.contains(&format!("\"sidebarWidth\": {expected}"))
                || config.contains(&format!("\"sidebarWidth\":{expected}"))
            {
                return;
            }
            sleep(Duration::from_millis(50));
        }
        let config = fs::read_to_string(self.config_path()).unwrap_or_default();
        panic!("timed out waiting for sidebarWidth={expected}; config={config}");
    }

    fn wait_for_sidebar_width(&self, session: &str, expected: u16) {
        let deadline = Instant::now() + Duration::from_secs(8);
        while Instant::now() < deadline {
            if self.sidebar_width(session) == Some(expected) {
                return;
            }
            sleep(Duration::from_millis(100));
        }
        panic!(
            "timed out waiting for {session} sidebar width to be {expected}; panes={:?}\nlogs:\n{}",
            self.sidebar_panes(),
            self.logs(),
        );
    }

    fn wait_for_sidebar_widths_except_pane(&self, expected: u16, excluded_pane: &str) {
        let deadline = Instant::now() + Duration::from_secs(8);
        while Instant::now() < deadline {
            let panes = self.sidebar_panes();
            let checked = panes
                .iter()
                .filter(|pane| pane.pane != excluded_pane)
                .collect::<Vec<_>>();
            if checked.len() >= SIDEBAR_SESSIONS.len().saturating_sub(1)
                && checked.iter().all(|pane| pane.width == expected)
            {
                return;
            }
            sleep(Duration::from_millis(100));
        }
        panic!(
            "timed out waiting for sidebar widths except {excluded_pane} to be {expected}; panes={:?}\nlogs:\n{}",
            self.sidebar_panes(),
            self.logs(),
        );
    }

    fn wait_for_sidebar_pane_count(&self, expected: usize) {
        let deadline = Instant::now() + Duration::from_secs(10);
        while Instant::now() < deadline {
            if self.sidebar_panes().len() >= expected {
                return;
            }
            sleep(Duration::from_millis(100));
        }
        panic!(
            "timed out waiting for at least {expected} sidebar panes; panes={:?}\nlogs:\n{}",
            self.sidebar_panes(),
            self.logs(),
        );
    }

    fn measure_tab_switch(&self, from_session: &str, source_pane: &str) -> Duration {
        let started = Instant::now();
        self.tmux_ok(["send-keys", "-t", source_pane, "Tab"]);
        let deadline = Instant::now() + Duration::from_secs(5);
        while Instant::now() < deadline {
            let output = self.tmux(["list-clients", "-F", "#{client_session}"]);
            if output
                .lines()
                .any(|line| !line.trim().is_empty() && line.trim() != from_session)
            {
                return started.elapsed();
            }
            sleep(Duration::from_millis(10));
        }
        panic!(
            "timed out waiting for client to leave session {from_session}; clients:\n{}\n\nlogs:\n{}",
            self.tmux([
                "list-clients",
                "-F",
                "#{client_name} #{client_tty} #{client_session}"
            ]),
            self.logs(),
        );
    }

    fn wait_for_no_sidebar_processes(&self) {
        let deadline = Instant::now() + Duration::from_secs(5);
        while Instant::now() < deadline {
            if self.sidebar_panes().is_empty() {
                return;
            }
            sleep(Duration::from_millis(100));
        }
        panic!(
            "timed out waiting for all sidebar panes to exit; panes={:?}\nlogs:\n{}",
            self.sidebar_panes(),
            self.logs(),
        );
    }

    fn assert_no_sidebar_panes(&self, reason: &str) {
        let panes = self.sidebar_panes();
        assert!(
            panes.is_empty(),
            "{reason}; panes={panes:?}\nlogs:\n{}",
            self.logs(),
        );
    }

    fn wait_for_server_exit(&mut self) {
        let deadline = Instant::now() + Duration::from_secs(5);
        while Instant::now() < deadline {
            if let Some(server) = &mut self.server
                && server.try_wait().expect("poll server process").is_some()
            {
                self.server = None;
                return;
            }
            sleep(Duration::from_millis(100));
        }
        panic!("server did not exit after q; logs:\n{}", self.logs());
    }

    fn server_is_running(&mut self) -> bool {
        self.server
            .as_mut()
            .and_then(|server| server.try_wait().expect("poll server process"))
            .is_none()
    }

    fn wait_for_non_sidebar_pane_count(&self, session: &str, expected: usize) {
        let deadline = Instant::now() + Duration::from_secs(5);
        while Instant::now() < deadline {
            if self.non_sidebar_panes(session).len() == expected {
                return;
            }
            sleep(Duration::from_millis(100));
        }
        panic!(
            "timed out waiting for {expected} non-sidebar panes in {session}; panes={:?}\nlogs:\n{}",
            self.non_sidebar_panes(session),
            self.logs(),
        );
    }

    fn sidebar_pane(&self, session: &str) -> String {
        let output = self.tmux([
            "list-panes",
            "-t",
            session,
            "-F",
            "#{pane_id} #{pane_current_command}",
        ]);
        output
            .lines()
            .find_map(|line| {
                let (pane, command) = line.split_once(' ')?;
                command
                    .starts_with("opensessions")
                    .then(|| pane.to_string())
            })
            .unwrap_or_else(|| panic!("no sidebar pane found for {session}; panes:\n{output}"))
    }

    fn sidebar_pane_in_window(&self, session: &str, window: &str) -> String {
        let output = self.tmux([
            "list-panes",
            "-t",
            &format!("{session}:{window}"),
            "-F",
            "#{pane_id} #{pane_current_command}",
        ]);
        output
            .lines()
            .find_map(|line| {
                let (pane, command) = line.split_once(' ')?;
                command
                    .starts_with("opensessions")
                    .then(|| pane.to_string())
            })
            .unwrap_or_else(|| {
                panic!("no sidebar pane found for {session}:{window}; panes:\n{output}")
            })
    }

    fn current_window_index(&self, session: &str) -> String {
        self.tmux(["display-message", "-p", "-t", session, "#{window_index}"])
    }

    fn pane_window_id(&self, pane: &str) -> String {
        self.tmux(["display-message", "-p", "-t", pane, "#{window_id}"])
    }

    fn client_tty(&self) -> String {
        self.tmux(["list-clients", "-F", "#{client_tty}"])
            .lines()
            .find_map(|line| {
                let tty = line.trim();
                (!tty.is_empty()).then(|| tty.to_string())
            })
            .expect("attached tmux client tty")
    }

    fn next_window_index(&self, session: &str) -> u32 {
        self.tmux(["list-windows", "-t", session, "-F", "#{window_index}"])
            .lines()
            .filter_map(|line| line.trim().parse::<u32>().ok())
            .max()
            .unwrap_or(0)
            .saturating_add(1)
    }

    fn main_pane(&self, session: &str) -> String {
        self.non_sidebar_panes(session)
            .into_iter()
            .next()
            .unwrap_or_else(|| panic!("no main pane found for {session}"))
    }

    fn sidebar_width(&self, session: &str) -> Option<u16> {
        self.sidebar_panes()
            .into_iter()
            .find(|pane| pane.session == session)
            .map(|pane| pane.width)
    }

    fn non_sidebar_panes(&self, session: &str) -> Vec<String> {
        self.tmux([
            "list-panes",
            "-t",
            session,
            "-F",
            "#{pane_id}\t#{pane_title}",
        ])
        .lines()
        .filter_map(|line| {
            let (pane, title) = line.split_once('\t')?;
            (title != "opensessions-sidebar").then(|| pane.to_string())
        })
        .collect()
    }

    fn sidebar_panes(&self) -> Vec<SidebarPane> {
        self.tmux([
            "list-panes",
            "-a",
            "-F",
            "#{session_name}\t#{pane_id}\t#{pane_width}\t#{pane_current_command}\t#{pane_title}",
        ])
        .lines()
        .filter_map(|line| {
            let mut parts = line.split('\t');
            let session = parts.next()?;
            let pane = parts.next()?;
            let width = parts.next()?.parse::<u16>().ok()?;
            let command = parts.next()?;
            let title = parts.next()?;
            (title == "opensessions-sidebar" || command.starts_with("opensessions")).then(|| {
                SidebarPane {
                    session: session.to_string(),
                    pane: pane.to_string(),
                    width,
                }
            })
        })
        .collect()
    }

    fn capture_pane(&self, pane: &str) -> String {
        self.tmux(["capture-pane", "-p", "-t", pane])
    }

    fn tmux_socket_env(&self) -> String {
        format!(
            "{},0,0",
            self.tmux(["display-message", "-p", "#{socket_path}"])
        )
    }

    fn tmux_ok<const N: usize>(&self, args: [&str; N]) {
        let output = Command::new("tmux")
            .arg("-L")
            .arg(&self.socket)
            .args(args)
            .output()
            .expect("run tmux");
        assert!(
            output.status.success(),
            "tmux failed: args={args:?}\nstdout={}\nstderr={}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }

    fn tmux<const N: usize>(&self, args: [&str; N]) -> String {
        let output = Command::new("tmux")
            .arg("-L")
            .arg(&self.socket)
            .args(args)
            .output()
            .expect("run tmux");
        assert!(
            output.status.success(),
            "tmux failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8_lossy(&output.stdout).trim().to_string()
    }

    fn git<const N: usize>(&self, dir: &Path, args: [&str; N]) {
        let output = Command::new("git")
            .arg("-C")
            .arg(dir)
            .args(args)
            .output()
            .expect("run git");
        assert!(
            output.status.success(),
            "git failed in {}: {}",
            dir.display(),
            String::from_utf8_lossy(&output.stderr)
        );
    }

    fn logs(&self) -> String {
        let mut logs = String::new();
        for entry in fs::read_dir(&self.root).expect("read e2e root") {
            let entry = entry.expect("read e2e log entry");
            let path = entry.path();
            if path.extension().is_some_and(|extension| extension == "log") {
                logs.push_str(&format!("\n--- {} ---\n", path.display()));
                logs.push_str(&fs::read_to_string(&path).unwrap_or_else(|err| err.to_string()));
            }
        }
        logs
    }

    fn sidebar_bin(&self) -> PathBuf {
        std::env::var_os("CARGO_BIN_EXE_opensessions-sidebar")
            .map(PathBuf::from)
            .unwrap_or_else(|| self.target_debug_bin("opensessions-sidebar"))
    }

    fn server_bin(&self) -> PathBuf {
        std::env::var_os("OPENSESSIONS_E2E_SERVER_BIN")
            .map(PathBuf::from)
            .unwrap_or_else(|| self.target_debug_bin("opensessions-server"))
    }

    fn home_dir(&self) -> PathBuf {
        self.root.join("home")
    }

    fn config_path(&self) -> PathBuf {
        self.home_dir().join(".config/opensessions/config.json")
    }

    fn target_debug_bin(&self, name: &str) -> PathBuf {
        let current = std::env::current_exe().expect("current exe");
        let deps = current.parent().expect("deps dir");
        let debug = deps.parent().expect("target debug dir");
        debug.join(name)
    }
}

fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

impl Drop for Lab {
    fn drop(&mut self) {
        if let Some(mut server) = self.server.take() {
            let _ = server.kill();
            let _ = server.wait();
        }
        for mut client in self.clients.drain(..) {
            let _ = client.kill();
            let _ = client.wait();
        }
        let _ = Command::new("tmux")
            .args(["-L", &self.socket, "kill-server"])
            .output();
        let _ = fs::remove_dir_all(&self.root);
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SidebarPane {
    session: String,
    pane: String,
    width: u16,
}
