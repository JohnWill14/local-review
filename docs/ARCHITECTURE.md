# Architecture

## Overview

Local Review is a single-process Rust terminal application. It keeps Git inspection, application state, rendering, and note persistence separate so the reviewed repository is never written by the application.

## Components

- `src/cli.rs` defines command-line flags with Clap.
- `src/git/mod.rs` opens repositories through `git2`, resolves refs, computes comparisons, and lists commits.
- `src/app.rs` owns navigation state, selected refs/files/lines, search state, visual selection, themes, and note actions.
- `src/ui.rs` runs the Crossterm/Ratatui event loop and renders the tree, diff, notes, dialogs, and help.
- `src/notes/mod.rs` persists repository-scoped notes and captured diff snippets in SQLite.
- `src/config.rs` persists the selected theme under the user's configuration directory.

## Review data flow

1. The CLI selects a repository path, base, head, and optional working-tree mode.
2. `RepositoryModel` resolves commits with `git2`, finds a merge base, and produces file-level diffs.
3. Patch lines become `DiffLine` records with old/new line numbers and safe display content.
4. `App` selects one file and tracks the cursor or visual line range.
5. The UI renders either a unified diff or independent before/after panes.
6. A comment stores its file path, line range, captured snippet, and body in SQLite.

## Terminal rendering

Ratatui renders into an in-memory cell buffer and emits only changed cells. Local Review clears the physical terminal before the first frame and after resize events, then fills the canvas and every panel explicitly before rendering content. This prevents stale terminal cells from appearing when files, modes, or modals change.

Diff content is sanitized at the Git boundary: tabs expand to four spaces, line endings are removed, and control characters become printable symbols. This prevents terminal control bytes or literal tab stops from moving the terminal cursor outside a panel.

## Build and verification

```sh
cargo fmt --check
cargo test
./scripts/build-container.sh
./release/local-review --help
```

`scripts/build-container.sh` uses Podman with `container/Containerfile`, mounts source read-only, caches Cargo dependencies in named volumes, and writes the final Linux binary to `release/local-review`.

## Extension boundaries

- Add CLI flags in `src/cli.rs` and resolve them in `src/main.rs`.
- Add read-only Git data in `src/git/mod.rs`; sanitize any text that will reach the terminal.
- Keep UI state transitions in `src/app.rs` and rendering in `src/ui.rs`.
- Add persistent review metadata through migrations in `src/notes/mod.rs`.
- Cover data and persistence changes with integration tests under `tests/`.
