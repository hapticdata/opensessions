# opensessions — AI Agent Instructions

You are working on **opensessions**, an agent-agnostic, mux-agnostic terminal session manager.

## North Star

**opensessions is becoming the parallel-agent operating system.** It should make many CLI agents, panes, mux sessions, and git worktrees feel like one coherent control plane instead of a pile of terminals.

The product direction is:

- **Observe everything**: track mux sessions, windows, panes, focused pane, layouts, worktrees, git state, agent status, approval state, unread/done state, and session activity as first-class runtime state.
- **One worktree === one session by default** for opensessions-created work: a new isolated task should get a worktree-backed mux session with predictable naming, pane layout, agent launch, and cleanup.
- **Existing work stays valid**: users and agents must also be able to launch agents inside existing worktrees, existing sessions, and existing panes when that is the right workflow.
- **Server as control plane**: the server should own durable state, synchronization, launch jobs, mux/worktree mappings, and agent-facing APIs. The sidebar is one UI over that control plane, not the control plane itself.
- **CLI/API for agents**: opensessions should expose commands and eventually an API/MCP surface that lets agents create sessions, launch sibling agents, inspect panes, read useful context, send prompts, wait for state changes, and report handoffs safely.
- **Mux-native, not mux-hostile**: tmux/zellij/etc. remain the substrate. opensessions should repair and manage only what it owns, preserve user layouts by default, and reserve fully managed layouts for explicit modes.
- **Review and merge are part of the OS**: parallel execution is only useful when the user can compare outputs, detect conflicts, review diffs, merge winning work, and clean up sessions/worktrees without ceremony.

## Project Structure

```
opensessions/
├── apps/
│   ├── server/        # @opensessions/server — bootstrap entrypoint for the Bun server
│   └── tui/           # @opensessions/tui — OpenTUI terminal sidebar (Solid)
│       ├── src/
│       │   └── index.tsx    # Main TUI app
│       ├── scripts/
│       │   └── start.sh     # Canonical sidebar launcher used by mux providers
│       ├── build.ts         # Bun build with Solid plugin
│       └── bunfig.toml      # Required: preload for Solid JSX transform
├── integrations/
│   └── tmux-plugin/  # tmux-facing scripts and host integration glue
├── packages/
│   ├── runtime/       # @opensessions/runtime — runtime, watcher logic, config, plugins, server internals
│   │   ├── src/
│   │   │   ├── contracts/   # AgentEvent, AgentStatus, AgentWatcher, MuxProvider, MuxSessionInfo
│   │   │   ├── agents/      # AgentTracker (state management for agent events)
│   │   │   │   └── watchers/  # Built-in agent watchers
│   │   │   │       ├── amp.ts
│   │   │   │       ├── claude-code.ts
│   │   │   │       ├── codex.ts
│   │   │   │       └── opencode.ts
│   │   │   ├── mux/         # Mux registry and detection helpers
│   │   │   ├── server/      # WebSocket server internals and launcher
│   │   │   ├── shared.ts    # Shared types, constants, palette
│   │   │   └── index.ts     # Barrel export
│   │   └── test/            # Tests (bun:test)
│   └── mux/
│       ├── contract/        # @opensessions/mux — mux contracts and capability guards
│       ├── providers/
│       │   ├── tmux/        # @opensessions/mux-tmux — tmux provider
│       │   └── zellij/      # @opensessions/mux-zellij — experimental zellij provider
│       └── tmux-sdk/        # @opensessions/tmux-sdk — lower-level tmux command wrapper
├── CONTRACTS.md       # Agent integration guide (Amp, Claude Code, OpenCode, Aider)
├── turbo.json         # Turborepo config
├── opensessions.tmux  # Root TPM entrypoint
└── package.json       # Bun workspace root
```

## Key Architecture Decisions

1. **Monorepo**: Turborepo + Bun workspaces, with `apps/` for runnable entrypoints and `packages/` for reusable libraries.
2. **Built-in agent watchers**: Core ships with `AmpAgentWatcher`, `ClaudeCodeAgentWatcher`, `CodexAgentWatcher`, and `OpenCodeAgentWatcher` that watch agent data directories directly. External agents integrate via the `AgentWatcher` plugin interface.
3. **Mux-agnostic**: `MuxProvider` interface abstracts all mux operations. `TmuxProvider` is the reference implementation.
4. **MuxProvider is SYNC**: All methods use `Bun.spawnSync` — matches the existing pattern and keeps the server simple.
5. **Auto-detect mux**: `detectMux()` checks `$TMUX`, `$ZELLIJ_SESSION_NAME` env vars. Config file override planned.
6. **TDD**: All contracts and tracker logic have tests. Use `bun test` in `packages/runtime/`.

## Contracts

### AgentEvent
```typescript
{ agent: string, session: string, status: AgentStatus, ts: number, threadId?: string, threadName?: string, unseen?: number }
```
`AgentStatus = "running" | "idle" | "done" | "error" | "waiting" | "interrupted"`

### MuxProvider Interface
```typescript
interface MuxProvider {
  name: string;
  listSessions(): MuxSessionInfo[];        // {name, createdAt, dir, windows}[]
  switchSession(name, clientTty?): void;
  getCurrentSession(): string | null;
  getSessionDir(name): string;
  getPaneCount(name): number;
  getClientTty(): string;
  setupHooks(host, port): void;
  cleanupHooks(): void;
}
```

### AgentWatcher Interface
```typescript
interface AgentWatcher {
  name: string;
  watch(callback: (event: AgentEvent) => void): void;
  stop(): void;
}
```

## Stack

- **Runtime**: Bun (not Node)
- **Language**: TypeScript (strict)
- **TUI**: OpenTUI with Solid reconciler (`@opentui/solid`, `@opentui/core`, `solid-js`)
- **Tests**: `bun:test` — run with `bun test` in `packages/runtime/`
- **Build**: `@opentui/solid/bun-plugin` for TUI builds

## Development Guidelines

- **TDD**: Red-green-refactor, vertical slices, one test at a time. Tests verify behavior through public interfaces.
- **Sync mux calls**: MuxProvider methods are synchronous. Don't make them async.
- **Preserve optimizations**: Batched tmux calls, 5s git cache with HEAD watchers, lightweight focus-only broadcasts.
- **Sidebar resize work**: Before changing sidebar spawning, width sync, tmux resize handling, or `sidebar-coordinator`, read `docs/explanation/sidebar-behavior.md` and preserve those invariants unless you update the doc in the same change.
- **Built-in watchers in runtime**: Amp, Claude Code, Codex, and OpenCode have built-in watchers in `packages/runtime/src/agents/watchers/`. Community agents use the `AgentWatcher` plugin interface.
- **OpenTUI Solid**: JSX needs `bunfig.toml` preload and `jsxImportSource: "@opentui/solid"` in tsconfig. Build needs `solidPlugin`.
- **Never call `process.exit()` directly in TUI**: Use `renderer.destroy()`.

## Common Commands

```bash
bun install                          # Install all workspace deps
bun test                             # Run all tests (from root via turbo)
cd packages/runtime && bun test      # Run runtime tests directly
cd apps/tui && bun run start         # Start TUI (requires tmux)
cd apps/tui && bun run build         # Build TUI for distribution
cd apps/server && bun run start      # Start the server bootstrap directly
```

## Adding a New Mux Provider

1. Create a new package under `packages/mux/providers/<your-mux>/`
2. Implement the `MuxProvider` interface
3. Register it from the server bootstrap in `apps/server/src/main.ts` if it should be built in
4. Add tests in the provider package or `packages/runtime/test/` at the highest useful layer
5. Export the provider from its package entrypoint

## Adding Agent Support

1. Create `packages/runtime/src/agents/watchers/your-agent.ts`
2. Implement the `AgentWatcher` interface
3. Register via `PluginAPI.registerWatcher()` in your plugin
4. Add tests in `packages/runtime/test/`
5. See `CONTRACTS.md` for integration examples
