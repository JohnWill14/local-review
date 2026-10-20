# AI Agent Operating Guide

## Safety boundary

Local Review is a read-only Git review tool. Do not use it, its build scripts, or helper commands to modify the repository being reviewed. In particular, do not run `git commit`, `git add`, `git push`, `git reset`, `git checkout`, destructive cleanup commands, or delete `~/.config/local-review/notes.sqlite` unless the user explicitly requests that action.

The only intentional mutable application data is the local SQLite notes database and `~/.config/local-review/config.toml`. Creating, editing, exporting, and clearing notes is performed through the TUI and requires user-directed intent.

## Build and validate

Run these commands from the Local Review project root:

```sh
cargo fmt --check
cargo test
./scripts/build-container.sh
```

The Podman build writes the executable to `release/local-review` and does not install Rust dependencies on the host.

## GitHub release automation

`.github/workflows/release.yml` builds native Linux x86_64 and Windows x86_64 binaries for every pushed `v*` tag. It runs tests, generates `SHA256SUMS`, and attaches assets to a pre-release. For an already-created tag, use the **Run workflow** action with the tag name; do not move or recreate a published tag to retrigger a build.

## Run the executable

```sh
# Use the current directory as the Git repository.
./release/local-review

# Review a specific repository.
./release/local-review --path /absolute/path/to/repository

# Compare explicit refs, optionally including working-tree changes.
./release/local-review --path /absolute/path/to/repository --base main --head feature/name --working-tree

# Inspect supported flags without opening the TUI.
./release/local-review --help
```

The executable requires an interactive TTY for the review interface. Noninteractive agents may run `--help`, tests, and builds, but must not claim to have navigated the TUI without a real terminal session.

## Interactive operation

- Use `b` and `l` to choose base and head; `Enter` confirms ref choices.
- Use `Tab` and `Shift+Tab` to change focused panel; `j`/`k` or arrows navigate.
- Use `u` for split/unified diff, `f` to search, and `n`/`N` to navigate diff matches.
- Use `v` or `V` to start visual selection and `c` to add a one-line comment. `Enter` saves comments.
- In Notes, use `Enter` to edit the selected note. Use `X` only when the user requested clipboard export and note removal; it asks for confirmation.
- Use `q` to exit. `h`, `H`, and `?` open help.

## Change guidelines

- Keep all user-facing strings in English.
- Keep Git interaction read-only.
- Sanitize text originating in Git before terminal rendering.
- Run tests and the Podman build after code changes.
