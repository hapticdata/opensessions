# Ratatui TUI Agent Discipline

Strict red/green TDD is mandatory for this crate.

Every function, every keybind, every render path, and every protocol behavior must start with a failing `cargo test` that proves the intended behavior is absent. Implementation may begin only after the failure is observed, and the change is complete only when that same test turns green without weakening the assertion.

Snapshot tests against `docs/ratatui-migration/reference-snapshots/*.ansi` are the ultimate gate for render fidelity. Do not mark read-only rendering complete unless the Ratatui output matches the documented reference snapshots.

Follow all architectural decisions in `docs/ratatui-migration/00-index.md` through `17-feasibility-matrix.md`. Do not substitute crates, async runtime features, distribution strategy, layout behavior, protocol shape, or performance targets unless the migration docs are updated by a human reviewer first.
