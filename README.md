<h1 align="center">Cockpit</h1>

<p align="center">
  <strong>One place to run all your projects.</strong><br>
  Persistent terminals, files, Git, containers, notes, monitoring —<br>
  everything around a project, in a single native app.
</p>

<p align="center">
  <a href="https://github.com/jguevel-tech/cockpit/releases/latest"><img alt="Latest release" src="https://img.shields.io/github/v/release/jguevel-tech/cockpit?style=flat-square&color=2f81f7"></a>
  <a href="https://github.com/jguevel-tech/cockpit/actions/workflows/release.yml"><img alt="Build" src="https://img.shields.io/github/actions/workflow/status/jguevel-tech/cockpit/release.yml?style=flat-square"></a>
  <img alt="Platform" src="https://img.shields.io/badge/platform-Linux%20·%20macOS%20(beta)-informational?style=flat-square">
  <a href="LICENSE"><img alt="License" src="https://img.shields.io/badge/license-MIT-green?style=flat-square"></a>
</p>

---

## Install

### Linux

```sh
curl -fsSL https://raw.githubusercontent.com/jguevel-tech/cockpit/main/scripts/install.sh | sh
```

That's it. The script installs the latest release into `~/.local/bin`, adds a desktop entry, and
needs no root privileges. Then run `cockpit`.

### macOS (beta, Apple Silicon)

Download the `.dmg` from the [releases page](https://github.com/jguevel-tech/cockpit/releases/latest),
drag Cockpit to Applications.

The app is not notarized by Apple yet, so the first launch is refused. **Right-click → Open no
longer works** — Apple removed that shortcut in macOS 15. Two cases:

- macOS says it *"cannot verify this app is free of malware"* → click **Cancel** (not *Move to
  Trash*), then open **System Settings → Privacy & Security**, scroll to the message about
  Cockpit and click **Open Anyway**.
- macOS says the app *"is damaged and cannot be opened"* → no button will help. Run
  `xattr -d com.apple.quarantine /Applications/Cockpit.app` in a terminal, then open it normally.

Updates afterwards need none of this: the in-app updater downloads them itself, and macOS only
quarantines what comes from a browser.

> **Updates happen inside the app** on both platforms. When a new version ships, a bell appears in
> the header: one click shows what changed and installs it. You won't need to download anything again.

### Windows (not yet)

No installer yet. The whole codebase compiles for Windows with zero warnings, and the CI knows how
to build an NSIS installer — but the terminal service does not work there yet: writing to the
pseudo-console fails, so terminals would be dead on arrival. Shipping that would be worse than
shipping nothing.

Progress happens in the `Windows (atelier)` workflow, which builds and tests on demand.

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

None. Cockpit runs as-is — a project is just a name and a folder, and **persistent terminals are
built in**: Cockpit runs its own terminal service, with nothing to install.

Individual features lean on the tool they concern, and stay quietly inactive (or tell you exactly
what's missing) when it is absent:

| Tool | Unlocks |
|---|---|
| `git` | Git tab |
| `docker` + `docker compose` | containers tab |
| LSP servers (`rust-analyzer`, `intelephense`…) | go-to-definition |
| `tmux` | Claude Code teammates in split panes — that display mode is Claude Code's own, not Cockpit's terminals |

---

## Features

Press the **`i` button** in the header: the built-in, illustrated guide covers everything below
with visual examples rather than prose.

### Persistent terminals

Cockpit runs its own terminal service, in a process that outlives the window: close the app and
your shells keep running — screen and scrollback included — and you pick them back up on the next
launch. Search the scrollback (`Ctrl`+`Shift`+`F`) with highlighting and a match counter. The Claude logo marks sessions
where an AI agent is working, and your project's Claude Code conversations are listed and resumable
in one click.

Mouse selection, `Ctrl`+`C` to copy, `Ctrl`+click to open links. Nothing sits between your
keystrokes and the shell — not even our own shortcuts.

### Files and code

A file browser that respects your `.gitignore`, syntax highlighting for about thirty languages
with line numbers, image preview, and in-place editing. Create, rename and delete files from the
tree — deletion goes to the **system trash**, never `rm`.

Find in file (`Ctrl`+`F`) with highlighted matches, and project-wide search (`Ctrl`+`Shift`+`F`)
across folder names, file names and file contents. `Ctrl`+click on a symbol jumps to its
definition, using whichever LSP servers are installed — with a regex fallback when none is.

### Git

The daily loop without leaving the app: status, coloured diff, per-file staging, commit, push,
pull (fast-forward only — a button never merges behind your back), branch management, and a
commit history with the full diff of any commit.

### Containers

Start and stop Docker Compose projects in the right order — Cockpit resolves dependencies by
topological sort and detects cycles before launching anything. Per container: **live logs** and a
**shell inside it**, one click each. A global view lists every container, volume and image on the
machine, with cleanup actions. Entirely optional: a project without Docker is still a project.

### Command palette & quick commands

`Ctrl`+`K` jumps anywhere: projects, open terminals, tabs, files by name, dashboard views. Declare
your usual commands per project (`make up`, `npm run dev`…) and run them from the **▶ Cmd** button —
each in a fresh terminal.

### Tasks, notes, meetings

Per-project todos with **due dates** — the notification bell warns you when something is due, and
the alert clears itself once the task is done. Tree-organised Markdown notes with a WYSIWYG editor
and autosave. Meeting recording (Linux) captures mic + system audio, transcribes, and drops a
summary note in the project automatically.

### Monitoring & alerts

CPU, detailed memory, disks, top processes — and the bell warns you when a disk is almost full or
CPU/memory stay saturated for minutes (never on a passing spike). Project quick links carry a
live up/down dot, so you see the moment staging goes down.

### Make it yours

Colour palettes, custom accent, and a wallpaper mode that turns the UI into legible frosted glass —
with dim, blur, opacity and shine all adjustable. Native zoom (`Ctrl`+scroll) that keeps terminals
pixel-sharp. One-click **database backup** from the settings.

---

## Handy shortcuts

| Action | Shortcut |
|---|---|
| Command palette | `Ctrl`+`K` |
| Find in file | `Ctrl`+`F` |
| Search project / terminal history | `Ctrl`+`Shift`+`F` |
| Go to definition | `Ctrl`+click on a symbol |
| Save the open file | `Ctrl`+`S` |
| Commit | `Ctrl`+`Enter` |
| Copy from a terminal | select with the mouse, then `Ctrl`+`C` |
| Zoom | `Ctrl`+scroll |

On macOS, `Cmd` works everywhere `Ctrl` does.

---

## Development

```sh
git clone https://github.com/jguevel-tech/cockpit.git
cd cockpit
npm install
npx tauri dev
```

### Build dependencies (Linux)

```sh
sudo apt install libgtk-3-dev libwebkit2gtk-4.1-dev librsvg2-dev patchelf libasound2-dev
```

On macOS, Xcode Command Line Tools are enough.

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
  lib/components/     Components, grouped by domain (docs/ = built-in guide)
  lib/stores/         Shared reactive state, notification producers
  styles/             Theme tokens and shared classes

src-tauri/src/        Rust backend
  terminal/           Terminal service (shells, screen, search), command history
  workspace/          File browser, project search, file management, Claude sessions
  storage/            SQLite: projects, notes, todos, commands, settings, backup
  gitdiff/            Git status, diff and log parsing
  docker/             Compose orchestration, dependency graph, containers, logs
  urlhealth.rs        Quick-link up/down checks
  lsp/  recorder/  system/  agents/  appearance/
```

Frontend and backend talk exclusively over Tauri's IPC: `invoke` for calls, events for real-time
updates. No HTTP server, no WebSocket. Releases ship from a Linux + macOS CI matrix that runs the
full test suite before bundling anything.

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
