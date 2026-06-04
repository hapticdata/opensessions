use std::fs::{self, File};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
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
    let mut lab = started_lab("opensessions-e2e-focus");
    if lab.skipped {
        return;
    }

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
fn tmux_sidebar_width_resize_fans_out_to_every_session_sidebar() {
    let lab = started_lab("opensessions-e2e-width");
    if lab.skipped {
        return;
    }
    let source = lab.sidebar_pane("opensessions");
    lab.tmux_ok(["switch-client", "-t", "opensessions"]);
    lab.tmux_ok(["select-pane", "-t", source.as_str()]);
    lab.wait_for_all_sidebar_widths(36);
    sleep(Duration::from_millis(900));

    // First resize initializes the TUI's last-observed terminal width. The
    // second resize is the product behavior: the active sidebar reports the
    // user-owned width and the server fans it out to every other session.
    lab.tmux_ok(["resize-pane", "-t", source.as_str(), "-x", "40"]);
    sleep(Duration::from_millis(250));
    lab.tmux_ok(["resize-pane", "-t", source.as_str(), "-x", "42"]);

    lab.wait_for_all_sidebar_widths(42);
}

#[test]
fn tmux_sidebar_quit_closes_the_server_and_every_sidebar_client() {
    let mut lab = started_lab("opensessions-e2e-quit");
    if lab.skipped {
        return;
    }
    let source = lab.sidebar_pane("opensessions");
    lab.tmux_ok(["switch-client", "-t", "opensessions"]);
    lab.tmux_ok(["select-pane", "-t", source.as_str()]);

    lab.tmux_ok(["send-keys", "-t", source.as_str(), "q"]);

    lab.wait_for_server_exit();
    lab.wait_for_no_sidebar_processes();
}

fn started_lab(prefix: &str) -> Lab {
    if std::env::var("OPENSESSIONS_TMUX_E2E").ok().as_deref() != Some("1") {
        eprintln!("skipping tmux e2e; set OPENSESSIONS_TMUX_E2E=1 to run");
        return Lab::skipped();
    }
    if Command::new("tmux").arg("-V").output().is_err() {
        eprintln!("skipping tmux e2e; tmux is unavailable");
        return Lab::skipped();
    }

    let mut lab = Lab::new(prefix);
    lab.setup_repos();
    lab.setup_tmux();
    lab.start_server();
    lab.spawn_sidebars();
    lab
}

fn row_with<'a>(text: &'a str, needle: &str) -> Option<&'a str> {
    text.lines().find(|line| line.contains(needle))
}

fn free_port() -> u16 {
    TcpListener::bind(("127.0.0.1", 0))
        .expect("bind ephemeral e2e port")
        .local_addr()
        .expect("read ephemeral e2e port")
        .port()
}

struct Lab {
    skipped: bool,
    socket: String,
    root: PathBuf,
    port: u16,
    server: Option<Child>,
    client: Option<Child>,
}

impl Lab {
    fn skipped() -> Self {
        Self {
            skipped: true,
            socket: String::new(),
            root: std::env::temp_dir(),
            port: 0,
            server: None,
            client: None,
        }
    }

    fn new(prefix: &str) -> Self {
        let unique = format!(
            "{}-{}-{}",
            prefix,
            std::process::id(),
            Instant::now().elapsed().as_nanos()
        );
        let root = std::env::temp_dir().join(&unique);
        fs::create_dir_all(&root).expect("create e2e root");
        Self {
            skipped: false,
            socket: unique,
            root,
            port: free_port(),
            server: None,
            client: None,
        }
    }

    fn setup_repos(&self) {
        if self.skipped {
            return;
        }
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
        if self.skipped {
            return;
        }
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

        self.client = Some(self.spawn_attached_client());
        self.wait_for_client_session("opensessions");
    }

    fn spawn_attached_client(&self) -> Child {
        let script = r#"
import fcntl, os, pty, struct, sys, termios, time

socket = sys.argv[1]
pid, fd = pty.fork()
if pid == 0:
    os.environ["TERM"] = "xterm-256color"
    os.execvp("tmux", ["tmux", "-L", socket, "attach-session", "-t", "opensessions"])

fcntl.ioctl(fd, termios.TIOCSWINSZ, struct.pack("HHHH", 40, 160, 0, 0))
time.sleep(300)
"#;
        Command::new("python3")
            .arg("-c")
            .arg(script)
            .arg(&self.socket)
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
        if self.skipped {
            return;
        }
        let server = self.server_bin();
        let tmux_env = self.tmux_socket_env();
        let child = Command::new(server)
            .env("TMUX", tmux_env)
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
            .env("OPENSESSIONS_WIDTH", W)
            .stdout(File::create(self.root.join("server.stdout.log")).expect("server stdout log"))
            .stderr(File::create(self.root.join("server.stderr.log")).expect("server stderr log"))
            .spawn()
            .expect("start opensessions server");
        self.server = Some(child);
        self.wait_for_server();
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
        if self.skipped {
            return;
        }
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

    fn wait_for_text(&self, session: &str, text: &str) {
        if self.skipped {
            return;
        }
        self.wait_for_capture(session, |capture| capture.contains(text));
    }

    fn wait_for_capture<F>(&self, session: &str, predicate: F)
    where
        F: Fn(&str) -> bool,
    {
        if self.skipped {
            return;
        }
        let pane = self.sidebar_pane(session);
        self.wait_for_capture_pane(&pane, predicate);
    }

    fn wait_for_capture_pane<F>(&self, pane: &str, predicate: F)
    where
        F: Fn(&str) -> bool,
    {
        if self.skipped {
            return;
        }
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
        if self.skipped {
            return;
        }
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

    fn wait_for_all_sidebar_widths(&self, expected: u16) {
        if self.skipped {
            return;
        }
        let deadline = Instant::now() + Duration::from_secs(8);
        while Instant::now() < deadline {
            let panes = self.sidebar_panes();
            if panes.len() == SIDEBAR_SESSIONS.len()
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

    fn wait_for_no_sidebar_processes(&self) {
        if self.skipped {
            return;
        }
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

    fn wait_for_server_exit(&mut self) {
        if self.skipped {
            return;
        }
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
            "tmux failed: {}",
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
        if self.skipped {
            return;
        }
        if let Some(mut server) = self.server.take() {
            let _ = server.kill();
            let _ = server.wait();
        }
        if let Some(mut client) = self.client.take() {
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
