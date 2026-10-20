# Local Review

Local Review is a local-first terminal UI for reviewing Git branches. It reads Git history and working-tree changes, but never commits, stages, pushes, or modifies files in the repository under review.

## Build without installing Rust on the host

The release build runs in Podman. The host receives only `release/local-review`.

```sh
./scripts/build-container.sh
./release/local-review --path /path/to/repository
```

The generated binary targets Linux on the container/host architecture. Building native macOS or Windows binaries requires runners and SDKs for those platforms.

## Usage

```sh
# Compare the repository default base with its current branch.
local-review --path /path/to/repository

# Select both references explicitly.
local-review --path . --base main --head feature/my-change

# Include staged and unstaged working-tree changes.
local-review --path . --working-tree
```

Run `local-review --help` for flags and a concise shortcut reference. When `--path` is omitted, the current directory is used just like Git.

## Interactive shortcuts

| Key | Action |
| --- | --- |
| `Tab` / `Shift+Tab` | Move to the panel on the right / left |
| `j` / `k` or arrows | Navigate the active panel |
| `b` / `l` | Choose base / head reference |
| `g`, then `y` | Open commits and copy the selected full hash |
| `u` | Toggle split and unified diff |
| `w` | Include or exclude working-tree changes |
| `v` / `V`, `c` | Select diff lines and add a local comment |
| `f`, then `n` / `N` | Search files or diff text; move between diff matches |
| `Enter` in Notes | Edit the selected comment |
| `X` | Copy notes and code snippets as Markdown, then confirm removal |
| `n` | Show or hide the notes panel when focus is not on Diff |
| `t` | Switch theme |
| `h` / `H` / `?` | Contextual / full help |
| `q` | Quit |

## Local data and themes

Notes are stored separately from the reviewed repository in SQLite at `~/.config/local-review/notes.sqlite`. They are scoped to the absolute repository path. Exporting notes copies Markdown, including the selected diff snippet, to the clipboard; removal requires confirmation.

The default `Terminal` theme uses the terminal's own foreground, background, and ANSI colors. Press `t` to cycle through Terminal, One Dark, One Light, and High Contrast. The selected theme is saved in `~/.config/local-review/config.toml`.

## Project documentation

- [Architecture and implementation notes](docs/ARCHITECTURE.md)
- [AI agent operating guide](AGENTS.md)
- [MIT License](LICENSE)
