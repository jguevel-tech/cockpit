<h1 align="center">Cockpit</h1>

<p align="center">
  <strong>One place to run all your projects.</strong><br>
  Persistent terminals, notes, files, Git, containers and system monitoring —<br>
  everything around a project, in a single native app.
</p>

<p align="center">
  <a href="https://github.com/jguevel-tech/cockpit/releases/latest"><img alt="Latest release" src="https://img.shields.io/github/v/release/jguevel-tech/cockpit?style=flat-square&color=2f81f7"></a>
  <a href="https://github.com/jguevel-tech/cockpit/actions/workflows/release.yml"><img alt="Build" src="https://img.shields.io/github/actions/workflow/status/jguevel-tech/cockpit/release.yml?style=flat-square"></a>
  <img alt="Platform" src="https://img.shields.io/badge/platform-Linux%20x86__64-informational?style=flat-square">
  <a href="LICENSE"><img alt="License" src="https://img.shields.io/badge/license-MIT-green?style=flat-square"></a>
</p>

---

## Install

```sh
curl -fsSL https://raw.githubusercontent.com/jguevel-tech/cockpit/main/scripts/install.sh | sh
```

That's it. The script installs the latest release into `~/.local/bin`, adds a desktop entry, and
needs no root privileges.

Then run:

```sh
cockpit
```

> **Updates happen inside the app.** When a new version ships, a bell appears in the header: one
> click shows what changed and installs it. You won't need this script again.

<details>
<summary><strong>Other install methods</strong></summary>

<br>

**Manual AppImage** — grab the latest from the [releases page](https://github.com/jguevel-tech/cockpit/releases/latest):

```sh
chmod +x Cockpit_*_amd64.AppImage
./Cockpit_*_amd64.AppImage
```

**From source** — see [Development](#development).

</details>

### Requirements

None. Cockpit runs as-is — a project is just a name and a folder.

Individual features lean on the tool they concern, and stay quietly inactive when it is missing:

| Tool | Unlocks |
|---|---|
| `tmux` (>= 3) | persistent terminals |
| `git` | Git tab |
| `docker` + `docker compose` | containers tab |
| `pw-record` (PipeWire) | meeting recording |
| LSP servers (`rust-analyzer`, `intelephense`…) | go-to-definition |

```sh
sudo apt install tmux git docker.io docker-compose-plugin pipewire-audio-client-libraries
```

---

## Features

### Persistent terminals

Every terminal is a `tmux` session on a dedicated socket: close the app and your processes keep
running — you pick them back up on the next launch. A green dot marks sessions where an AI agent is
working, and your project's Claude Code conversations are listed and resumable in one click.

Mouse selection, `Ctrl`+`C` to copy, `Ctrl`+click to open links. Nothing sits between your keystrokes
and the shell.

### Per-project workspace

Tree-organised Markdown notes with a WYSIWYG editor and autosave, drag-and-drop todos, quick links.
Everything is scoped to the project and follows it when you rename it.

### Files and code

A file browser that respects your `.gitignore`, syntax highlighting for about thirty languages, and
in-place editing. `Ctrl`+click on a symbol jumps to its definition, using whichever LSP servers are
installed on your machine — with a regex fallback when none is.

### Git

The daily loop, without leaving the app: status, coloured diff, per-file staging, commit, push and
branch management.

### Containers

Start and stop Docker Compose projects in the right order — Cockpit resolves dependencies by
topological sort, detects cycles before launching anything, and recursively stops dependencies that
became orphaned. A global view lists every container, volume and image on the machine, with cleanup
actions.

Entirely optional: projects without a compose file simply don't show this tab.

### Meeting recording

Records your microphone and system audio at once, transcribes both, then drops an automatic summary
as a note in the project. The summary prompt is configurable globally and overridable per project.

### Sitemap diff

Compare two sitemaps and get a unified HTML diff, URL by URL — useful for spotting what a deployment
actually changed.

### System monitoring

Global and per-core CPU, detailed memory (cache, buffers, ZFS ARC), disks, and the twenty hungriest
processes, with one minute of history.

---

## Usage

### Add a project

From the sidebar, hit `+` and give it a name and a folder. That's the whole requirement — everything
else is optional and can be filled in later from the project's **Settings** tab.

Cockpit can also scan a parent directory to detect several projects at once.

### Declare dependencies

In a project's **Settings** tab, list the projects it depends on. Starting it will then bring up the
whole chain in order, and Cockpit will warn you if you have created a cycle.

### Zoom

`Ctrl`+scroll anywhere in the app, or the `−` `+` buttons in the header. The level persists across
restarts.

### Handy shortcuts

| Action | Shortcut |
|---|---|
| Zoom in / out | `Ctrl`+scroll |
| Save the open file | `Ctrl`+`S` |
| Commit | `Ctrl`+`Enter` |
| Copy from a terminal | select with the mouse, then `Ctrl`+`C` |
| Open a link from a terminal | `Ctrl`+click |
| Go to definition | `Ctrl`+click on a symbol |

---

## Development

```sh
git clone https://github.com/jguevel-tech/cockpit.git
cd cockpit
npm install
npx tauri dev
```

### Build dependencies

```sh
sudo apt install libgtk-3-dev libwebkit2gtk-4.1-dev librsvg2-dev patchelf
```

### Commands

| Command | What it does |
|---|---|
| `npx tauri dev` | development with hot reload |
| `npm run check` | frontend type checking |
| `cargo test --manifest-path src-tauri/Cargo.toml` | Rust tests |
| `npx tauri build --no-bundle` | development binary |

> Always build with `npx tauri build`, never `cargo build --release` alone: without the Tauri CLI's
> environment variables the binary comes out in development mode and looks for a Vite server on
> `localhost:5173`.

### Architecture

```
src/                  Svelte 5 (runes) + TypeScript frontend
  lib/api/            Typed wrappers around Tauri commands
  lib/components/     Components, grouped by domain
  lib/stores/         Shared reactive state
  styles/             Theme tokens and shared classes

src-tauri/src/        Rust backend
  terminal/           tmux sessions and command history
  workspace/          File browser, Claude sessions
  storage/            SQLite: projects, notes, todos, settings
  gitdiff/            Git status and diff parsing
  docker/             Compose orchestration, dependency graph, containers
  lsp/  recorder/  sitemap/  system/  agents/
```

Frontend and backend talk exclusively over Tauri's IPC: `invoke` for calls, events for real-time
updates. No HTTP server, no WebSocket.

Project conventions — non-negotiable rules, known pitfalls, release process — live in
[CLAUDE.md](CLAUDE.md).

---

## Contributing

Issues and pull requests are welcome.

A change is ready when these three commands pass:

```sh
npm run check                                   # 0 errors, 0 warnings
cargo test --manifest-path src-tauri/Cargo.toml # all green
npx tauri build --no-bundle                     # compiles
```

And when it is recorded in [CHANGELOG.md](CHANGELOG.md) under `## [Unreleased]`, if a user can
notice it.

## License

[MIT](LICENSE)
