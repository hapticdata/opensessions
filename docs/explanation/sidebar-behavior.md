# Sidebar Behavior Invariants

This document captures the sidebar behaviors that were learned the hard way while making tmux sidebar resizing feel native, stable, and predictable.

If you change sidebar spawning, width sync, tmux hook handling, focus/session switching, or the `sidebar-coordinator` state machine, read this first.

## The Product Contract

The sidebar should behave like a real sidebar, not like an ordinary tmux pane.

That means:

- the sidebar width stays fixed until the user explicitly drags it
- the width propagates globally across every sidebar pane in every managed session/window
- full terminal resizes do not redefine the saved width
- session switching does not cause the sidebar to jump, breathe, or re-proportion itself
- background windows should already be correct before the user lands in them
- the UI should clearly show `warming up…` while sidebars are still spawning, `adjusting…` while width normalization is still in flight, and `closing…` while the server is draining clients before exit

## Product Behaviors

These are user-visible behaviors. Preserve them even if the implementation details change.

### The sidebar is global within one tmux server

Each tmux server/socket gets its own opensessions server and its own sidebar state. A sidebar in one tmux server must not talk to another tmux server's opensessions server.

The practical product behavior is:

- every sidebar pane inside the same tmux server shows the same session list, filters, lifecycle state, collapsed groups, and width
- each attached tmux client owns its own confirmed active row and temporary keyboard focus
- changing width in one window updates the sidebars in sibling windows/sessions inside that tmux server
- a different tmux server may have a different opensessions width/state/server without conflict
- stale hooks or sidebars talking to another port are considered broken configuration, not valid mixed-server behavior

### Sidebar width is owned by explicit sidebar resizing

The sidebar width should feel like a user-owned setting. It changes when the user deliberately resizes the sidebar divider, then fans out everywhere in the same tmux server.

Important details from the user's point of view:

- dragging the divider should work even if tmux focus visually remains in the main pane
- a normal tmux pane, background sidebar, or whole-terminal resize must not redefine the sidebar width
- after a valid resize, background sidebars should converge to the new width without flashes or snap-back
- pressing `q` quits opensessions only when the key is delivered to a connected sidebar client; pressing `q` in a normal tmux pane is just a normal shell/app keypress

### Server shutdown must not leave stale sidebar clients

When an opensessions server exits, every connected sidebar client in that tmux-server namespace should exit too. A dead server with old sidebar panes still rendering stale state is broken.

Expected shutdown behavior:

- quit can be requested by a connected sidebar keypress, websocket command, `/quit`, or process shutdown
- the server marks the sidebar lifecycle as `closing…`
- the server broadcasts `quit` to websocket and shim clients
- the server waits briefly for clients to receive the quit frame, then removes hooks, pid file, and shim socket
- restarting the same tmux server should create a fresh server/client generation, not reuse stale sidebars from a previous generation

### Session switching keeps the sidebar in control

Clicking a tmux session in the sidebar should switch to that session and leave focus in that session's sidebar pane. This keeps the global sidebar interaction continuous while navigating.

Agent-row activation is different: it may switch sessions first, then intentionally focus the target agent pane.

### Pane exits must not let tmux steal sidebar space

tmux's native layout repair can give freed pane width to the left neighbor. The sidebar must not permanently keep space just because tmux temporarily assigned it there.

The canonical case is:

```text
before:  sidebar(40) │ pane1(29) │ pane2(29)
action:  pane1 exits or is killed
after:   sidebar(40) │ pane2(59)
```

The broken behavior is:

```text
before:  sidebar(40) │ pane1(29) │ pane2(29)
action:  pane1 exits or is killed
wrong:   sidebar(70) │ pane2(29)
```

This applies to both explicit `kill-pane` and normal shell/process exit from inside `pane1`.

## Width Authority

Only true user intent should persist a new width.

The accepted rule set is:

- only the foreground sidebar in the active session can author a new width
- a sidebar pane in the active session/window can author width even when tmux focus remains on the adjacent main pane during a border drag
- background sidebars never get to redefine global width
- a user drag may continue for a short tail window even if focus moves immediately after the drag starts
- programmatic tmux resizes, session switches, and terminal resizes must be treated as echoes unless we have evidence of real user drag intent
- when a switch happens too quickly for the TUI to emit `report-width`, the server must opportunistically adopt the source window's actual sidebar pane width before switching

In practice, width authority is split into these cases:

- `user-drag`: a real user-driven sidebar resize
- `client-resize-sync`: the server correcting widths after a whole terminal/client resize
- `programmatic-adjust`: the server normalizing widths during ensure/switch/fan-out paths
- `none`: no resize authority is active

## Global Propagation Rules

When a width change is accepted:

- the persisted width changes once
- the server fans that width out to every other sidebar pane
- the source pane/window should not be fought by the fan-out pass
- rapid switching must not cut propagation short

This was a real bug: a drag could be accepted in one pane, then a fast switch would happen before later reports arrived, and the destination session would snap everything back to the old stored width. The current rule is to capture the source sidebar pane width during switch handoff so explicit user resizing is not lost.

## Terminal Resize Rules

External terminal resizing is not the same thing as sidebar resizing.

Expected behavior:

- moving between monitor sizes or resizing Ghostty/iTerm should not change the saved sidebar width
- the foreground window should be corrected quickly so the sidebar does not visually breathe
- background windows can catch up with a staggered sync pass after a short settle delay
- transient half-window widths reported during client resizes must never become the persisted width

The server therefore needs both:

- a suppression window to ignore server-induced resize echoes
- a client-resize guard window so transient widths during full terminal resize do not get mistaken for user drag

## Session Switching Rules

Session switching should feel boring.

That means:

- the destination session/window should already have a sidebar at the current global width
- switching must not trigger visible layout jumps
- switching must not reset the width to an older value
- transient sidebar widths produced while tmux settles after a session/window switch must not redefine the global width
- if the user resized immediately before switching, the just-resized width must survive the switch
- switching from a sidebar session row should leave focus on the destination sidebar pane, not the destination main pane

The sidebar session list has one durable local active row: this tmux client's confirmed active session. The keyboard-focused row may temporarily diverge while the user browses with `j`/`k`/arrow keys, but that temporary selection is local-only and must not be server-synced. `Enter` switches to the temporary selection and keeps that row visible as the pending switch target until `YourSession`/pane identity confirms the new context; it must not snap back to the old active row for an intermediate frame. `Tab`/`Shift-Tab` are the only keys that immediately switch to the next/previous visible session without first moving temporary focus. Mouse clicks on sessions also make the clicked concrete session the pending focus target. In all cases, the durable active row stays on the confirmed active session until confirmation.

Worktree group headers are normal temporary focus targets. `j`/`k`/arrow navigation can land on them, and `Enter` toggles collapse/expand. Once the user chooses a concrete child session inside an expanded worktree group, pending focus moves from the group header to that child session row; the group header must not remain focused in the destination session.

Temporary focus must not look like active focus. The confirmed active row owns the strong green active marker; a temporary keyboard selection uses a weaker cursor marker. Otherwise a normal `j`/`k` browse shows two active-looking rows and feels like focus randomly split.

When `Enter` or `Tab` switches from session A to session B, the old A sidebar remains alive in the background. If A keeps `pending=B` forever, returning to A later replays stale focus and feels random. A server `Focus` broadcast is only a fast intent echo and can arrive before tmux visibly switches the attached client, so it must not clear pending state. A settled `State` snapshot for B is allowed to clean up this one local case: if a sidebar has `pending=B` but its own local session is still A, it clears pending and rehomes focus to A. This does not make server focus authoritative; it only cleans up the source pane after its switch request moved the attached tmux client elsewhere.

This keeps per-window state simple: every attached tmux client can show a different active session row if that client is in a different tmux session, while shared server state still provides the common session list, width, filters, collapsed groups, and lifecycle labels. Server focus broadcasts are compatibility hints only; they must not move a client's local active/focused session row.

One specific regression we already paid for: forcing `resize-window` during the session-switch path caused visible layout jumps. The fix was to stop doing that in the switch path and instead use targeted width enforcement plus background pre-layout where appropriate.

Another regression: switching into a session can briefly resize the destination sidebar through impossible widths such as `1 → 20 → 58 → 20` while tmux restores layout. Those reports are layout-settle echoes, not user drags, even when they come from the active session/window/sidebar pane. Session handoff paths therefore arm a short width-report guard before accepting new user-authored sidebar width.

## Warmup And Adjusting Semantics

There are three user-visible initializing states and they mean different things.

`warming up…` means:

- sidebars are being spawned/restored across windows
- the system is still converging on presence, not width

`adjusting…` means:

- width normalization is still in flight across windows/sessions
- this includes whole-client resize sync, accepted drag propagation, and server-driven cross-window enforcement

`closing…` means:

- the server has accepted shutdown and is draining clients
- sidebars should receive `quit` and exit rather than staying around as stale clients

Important nuance:

- if warmup and a global adjustment overlap, the UI should prefer `adjusting…`
- warmup must not get stranded forever because a resize sync canceled the only completion timer

## tmux-Specific Invariants

tmux has several behaviors that look reasonable until they break the sidebar.

These are non-negotiable:

- ignore control-mode clients with empty `client_tty` when inferring current session or foreground client
- infer current session/window/pane from the active tmux command context where possible; do not let an unrelated attached client make width or focus decisions for the current sidebar
- keep tmux windows in `window-size latest`; do not leave them in manual mode after `resize-window`
- install both `pane-exited` and `after-kill-pane`; normal shell/process exit is not covered by `after-kill-pane` alone
- treat `pane-exited` and `after-kill-pane` as topology-change signals only; they must never adopt tmux's redistributed sidebar width as user intent
- do not use `after-resize-pane` as primary width authority; it may adopt active-window sidebar width when that pane is the sidebar being resized, or else act as a delayed drift signal that re-enforces the coordinator-owned width after tmux layout churn, and it must no-op while `user-drag`, client-resize, or programmatic-adjust authority is active
- do not refocus the main pane immediately after sidebar spawn/restore; let the TUI refocus after capability detection settles so escape sequences do not leak into the main pane
- invalidate cached sidebar pane listings before logic that depends on just-spawned or just-hidden panes

## Per-tmux-server Technical Contract

The tmux socket is the namespace boundary.

Derived configuration must be stable for a given tmux socket and distinct across tmux sockets:

- `server_key` is derived from the tmux socket path unless explicitly overridden in tmux-scoped environment
- server port, pid file, log file, and shim socket path derive from that key
- tmux hooks for that tmux server point only to that tmux server's derived server port
- sidebar clients derive the same endpoint as the server launcher
- explicit overrides such as `OPENSESSIONS_PORT`, `OPENSESSIONS_HOST`, and `OPENSESSIONS_PID_FILE` should be tmux-scoped, not ambient shell leftovers

Symptoms of broken per-server wiring:

- multiple `opensessions-server` processes for the same tmux socket
- hooks in one tmux server point to another tmux server's port
- sidebars inside the same tmux server have mixed widths after settle
- pressing `q` in a connected sidebar does not stop the expected derived server
- sidebars keep rendering after their server pid file/socket has been removed

The recovery path is: clear stale tmux-scoped overrides, refresh hooks from the current plugin, restart the derived server for that tmux socket, and respawn visible sidebar panes.

## Regressions We Already Paid For

These are the big historical failure modes worth remembering.

### 1. Jamming tmux's resizer

What happened:

- the server tried to do too much synchronous resize work during tmux resize storms
- or it got stuck in echo/enforcement loops where programmatic resizes caused more programmatic resizes

What fixed it:

- deferred client-resize sync after a short settle window
- fast staggered fan-out for background windows
- ignore-only suppression rather than recursive re-enforcement loops
- explicit re-entrancy guards around enforcement passes

### 2. Treating external width changes as user intent

What happened:

- full terminal resizes and other layout churn produced `report-width` values that looked like drags
- the saved width changed even though the user never dragged the divider

What fixed it:

- only the foreground active sidebar can author width
- client-resize guard windows reject transient reports during full terminal resize
- the state machine models causality so programmatic adjustments and user drags are not conflated

### 3. Switching quickly stopped propagation

What happened:

- the initial width report could be accepted
- later reports in the same drag were suppressed or never arrived before a switch
- the destination session then re-enforced the older persisted width

What fixed it:

- longer drag settle windows so drag authority survives realistic report timing
- drag-tail acceptance for the originating pane even after focus changes
- source-window width adoption during switch handoff so the latest real pane width is not lost if the TUI report races with the switch

### 4. Background windows were stale

What happened:

- only the active session/window got corrected promptly
- switching into a background window revealed a stale sidebar width flash

What fixed it:

- pre-layout plus staggered background correction
- global fan-out that includes sibling windows, not just other sessions

### 5. Manual window-size mode poisoned layouts

What happened:

- `resize-window` left tmux windows in `window-size manual`
- later terminal behavior looked padded or broken

What fixed it:

- always restore `window-size latest` after forced window resizes

### 6. Pane exit gave width to the sidebar

What happened:

- with `sidebar | pane1 | pane2`, tmux gave `pane1`'s freed width to the left sidebar when `pane1` exited
- if the sidebar accepted that observed width, it permanently grew and pane2 did not absorb the space

What fixed it:

- `pane-exited` covers normal shell/process exit and `after-kill-pane` covers explicit kill-pane
- pane-exit handlers re-enforce the coordinator-owned width instead of adopting observed tmux width
- the remaining non-sidebar pane absorbs the freed space

### 7. Active pane was the wrong resize authority

What happened:

- a border drag can resize the sidebar while tmux focus remains in the main pane
- requiring `#{pane_active}` to be the sidebar made legitimate user resizing fail

What fixed it:

- accept width reports from the sidebar pane in the active session/window
- reject reports from background sidebars or non-sidebar panes
- keep client-resize and programmatic-adjust guards so whole-terminal churn cannot become user intent

## Rejected Approaches

These are not theoretical. They were tried and caused problems.

- using `after-resize-pane` as the main width-authority mechanism
- forcing `resize-window` directly in the normal session-switch path
- suppressing width reports so broadly that legitimate drag events got blocked
- setting drag suppression in a way that made the server fight the user's live drag
- requiring tmux's active pane to be the sidebar pane before accepting a sidebar width report
- treating every TUI as authoritative instead of only the current foreground one

## Performance Constraints

The sidebar should not make tmux feel heavy.

Keep these constraints in mind:

- stagger expensive cross-window work instead of doing it all inline in hooks
- avoid repeated full `list-panes -a` scans inside the same resize cycle
- batch where possible, cache briefly, and invalidate on real topology changes
- prioritize the active window first, then let the rest catch up quickly in the background
- polling may correct a detected sidebar-width drift after a short settle window, but it must not redefine saved width and must not run while user-drag or client-resize authority is active

## Change Checklist

Before shipping any sidebar behavior change, verify all of these.

- dragging the active sidebar changes width smoothly
- dragging the sidebar divider while focus remains in the main pane still changes width smoothly
- switching sessions immediately after a drag preserves the new width
- clicking a session row leaves focus in the destination sidebar pane
- resizing the whole terminal does not redefine the persisted width
- in `sidebar | pane1 | pane2`, killing or exiting `pane1` leaves `sidebar` fixed-width and lets `pane2` absorb the freed width
- background windows land at the current width without visible proportional flash
- `warming up…` clears once spawn/restore is complete
- `adjusting…` appears reliably while global width correction is still happening
- server shutdown broadcasts `quit` to websocket and shim sidebar clients before cleanup
- control-mode clients cannot steal foreground/current-session authority
- hooks and sidebar clients point to the derived server for the current tmux socket
- tmux windows remain in `window-size latest`
- no resize or enforcement loop appears in `/tmp/opensessions-debug.log`

## Files To Read Before Changing This Area

- `packages/runtime/src/server/sidebar-coordinator.ts`
- `packages/runtime/src/server/index.ts`
- `packages/mux/providers/tmux/src/provider.ts`
- `packages/mux/tmux-sdk/src/index.ts`
- `apps/tui/src/index.tsx`
- `packages/runtime/test/sidebar-coordinator.test.ts`

If a future change violates this doc but seems necessary, update the doc in the same change and explain the new invariant explicitly.
