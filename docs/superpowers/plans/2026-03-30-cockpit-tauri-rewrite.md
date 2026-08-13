# Cockpit Tauri Rewrite -- Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Construire Cockpit (orchestrateur Docker) en Tauri v2 + Rust + Svelte 5 + TypeScript

**Architecture:** App desktop Tauri v2 avec backend Rust (SQLite, Docker CLI, sysinfo) communiquant avec un frontend Svelte 5 via IPC (invoke/events). Pas de serveur HTTP ni WebSocket.

**Tech Stack:** Tauri v2, Rust, Svelte 5, TypeScript, Vite, rusqlite, sysinfo, tokio

**Spec:** `docs/superpowers/specs/2026-03-30-cockpit-tauri-rewrite-design.md`

---

## Phase 1 : Scaffold et fondations

### Task 1: Setup environnement et scaffold Tauri + Svelte

**Files:**
- Create: `ai-workforce/package.json`
- Create: `ai-workforce/src-tauri/Cargo.toml`
- Create: `ai-workforce/src-tauri/tauri.conf.json`
- Create: `ai-workforce/src-tauri/src/main.rs`
- Create: `ai-workforce/src-tauri/src/lib.rs`
- Create: `ai-workforce/src/App.svelte`
- Create: `ai-workforce/src/main.ts`
- Create: `ai-workforce/vite.config.ts`
- Create: `ai-workforce/tsconfig.json`

- [ ] **Step 1:** Installer Tauri CLI et créer le projet

```bash
cd ai-workforce
npm create tauri-app@latest . -- --template svelte-ts --manager npm
```

Si le scaffold interactif pose problème, créer manuellement :

```bash
cd ai-workforce
npm init -y
npm install -D @tauri-apps/cli@^2 @sveltejs/vite-plugin-svelte svelte typescript vite
npm install @tauri-apps/api@^2 @tauri-apps/plugin-shell@^2
cargo install tauri-cli --version "^2"
```

- [ ] **Step 2:** Configurer `tauri.conf.json` avec les bons paramètres

```json
{
  "productName": "Cockpit",
  "identifier": "com.cockpit.app",
  "version": "0.1.0",
  "build": {
    "frontendDist": "../dist",
    "devUrl": "http://localhost:5173",
    "beforeDevCommand": "npm run dev",
    "beforeBuildCommand": "npm run build"
  },
  "app": {
    "title": "Cockpit",
    "windows": [{ "title": "Cockpit", "width": 1200, "height": 800 }]
  },
  "plugins": {}
}
```

- [ ] **Step 3:** Ajouter les dépendances Rust dans `Cargo.toml`

```toml
[dependencies]
tauri = { version = "2", features = [] }
tauri-plugin-shell = "2"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
rusqlite = { version = "0.31", features = ["bundled"] }
tokio = { version = "1", features = ["full"] }
sysinfo = "0.30"
```

- [ ] **Step 4:** Créer `lib.rs` minimal avec un premier `invoke` de test

```rust
use tauri::Manager;

#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello {} from Rust!", name)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .invoke_handler(tauri::generate_handler![greet])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

- [ ] **Step 5:** Créer `App.svelte` minimal qui appelle le `greet`

```svelte
<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  let result = $state("");
  async function test() {
    result = await invoke<string>("greet", { name: "Cockpit" });
  }
</script>

<main>
  <h1>Cockpit</h1>
  <button onclick={test}>Test IPC</button>
  <p>{result}</p>
</main>
```

- [ ] **Step 6:** Vérifier que l'app compile et s'ouvre

```bash
cd ai-workforce
cargo tauri dev
```

Expected: Une fenêtre native s'ouvre avec "Cockpit" et le bouton Test IPC fonctionne.

- [ ] **Step 7:** Commit

```bash
git add ai-workforce/
git commit -m "feat: scaffold Tauri v2 + Svelte 5 + TypeScript"
```

---

### Task 2: Module storage -- SQLite + migrations

**Files:**
- Create: `src-tauri/src/storage/mod.rs`
- Create: `src-tauri/src/storage/db.rs`
- Modify: `src-tauri/src/lib.rs`

Port de : `internal/storage/storage.go`

- [ ] **Step 1:** Créer `storage/db.rs` avec init SQLite, WAL mode, foreign keys, et le schéma complet de migration (6 tables : projects, notes, note_folders, note_files, todos, urls + indexes). Reprendre exactement le schéma SQL du Go source (`internal/storage/storage.go:41-97`).

- [ ] **Step 2:** Créer `storage/mod.rs` qui exporte le module et le type `Database` wrappé dans `Arc<Mutex<Connection>>`.

- [ ] **Step 3:** Écrire un test unitaire qui ouvre une DB en mémoire, vérifie que les 6 tables existent, et que la migration `position` column fonctionne.

```bash
cd ai-workforce/src-tauri
cargo test storage -- --nocapture
```

- [ ] **Step 4:** Commit

---

### Task 3: Storage CRUD -- Projects

**Files:**
- Create: `src-tauri/src/storage/projects.rs`
- Modify: `src-tauri/src/storage/mod.rs`

Port de : `internal/storage/projects.go`

- [ ] **Step 1:** Implémenter les structs `Project` avec `Serialize`/`Deserialize` et les méthodes : `get_projects()`, `create_project()`, `update_project()`, `delete_project()`, `get_project_by_name()`, `update_project_by_name()`, `reorder_projects()`. Le champ `depends_on` est stocké comme JSON string en DB, avec une méthode `depends_on_list() -> Vec<String>`.

- [ ] **Step 2:** Tests unitaires pour chaque opération CRUD (create, read, update, delete, reorder).

- [ ] **Step 3:** Commit

---

### Task 4: Storage CRUD -- Notes, Todos, URLs

**Files:**
- Create: `src-tauri/src/storage/notes.rs`
- Create: `src-tauri/src/storage/todos.rs`
- Create: `src-tauri/src/storage/urls.rs`
- Modify: `src-tauri/src/storage/mod.rs`

Port de : `internal/storage/notes.go`, `note_tree.go`, `todos.go`, `urls.go`

- [ ] **Step 1:** Implémenter `notes.rs` : structs `Note`, `NoteFolder`, `NoteFile`, `NoteTree`. Méthodes : `get_note`, `save_note`, `get_note_tree`, `create_note_folder`, `rename_note_folder`, `delete_note_folder`, `create_note_file`, `get_note_file`, `save_note_file`, `rename_note_file`, `delete_note_file`. Le pattern `maxPos+1` pour les positions auto-incrémentées doit être reproduit.

- [ ] **Step 2:** Implémenter `todos.rs` : struct `Todo`. Méthodes : `get_todos`, `create_todo`, `update_todo`, `delete_todo`, `reorder_todos`, `get_pending_todos` (avec JOIN sur projects pour le tri par position projet).

- [ ] **Step 3:** Implémenter `urls.rs` : struct `Url`. Méthodes : `get_urls`, `create_url`, `update_url`, `delete_url`.

- [ ] **Step 4:** Tests unitaires pour notes (create folder, create file, tree structure), todos (CRUD + reorder + pending), urls (CRUD).

- [ ] **Step 5:** Commit

---

## Phase 2 : Docker orchestration

### Task 5: Module docker/compose -- wrapper Docker CLI

**Files:**
- Create: `src-tauri/src/docker/mod.rs`
- Create: `src-tauri/src/docker/compose.rs`

Port de : `internal/compose/compose.go`

- [ ] **Step 1:** Implémenter `compose.rs` : struct `Compose { project_dir, compose_file }`, struct `ContainerStatus`. Méthodes :
  - `has_compose_file()` : vérifie l'existence de docker-compose.yml/yaml/compose.yml/yaml
  - `up()` : exécute `docker compose up -d` via `tokio::process::Command` avec timeout 2min
  - `down()` : exécute `docker compose down` avec timeout 2min
  - `ps()` : exécute `docker compose ps --format json`, parse JSON array ou NDJSON, retourne `Vec<ContainerStatus>`

- [ ] **Step 2:** `parse_container()` : parser les clés case-insensitive (Name/name, State/state, etc.) comme dans le Go (`compose.go:137-194`).

- [ ] **Step 3:** Test unitaire pour `parse_container` avec des payloads JSON connus (format array et NDJSON).

- [ ] **Step 4:** Commit

---

### Task 6: Module docker/graph -- dépendances

**Files:**
- Create: `src-tauri/src/docker/graph.rs`

Port de : `internal/orchestrator/graph.go` + `graph_test.go`

- [ ] **Step 1:** Implémenter `build_graph()`, `detect_cycles()` (DFS tricolore), `topological_sort()` (algorithme de Kahn), `format_cycle()`. Types : `Graph` = `HashMap<String, Vec<String>>`. Reprendre la logique exacte du Go.

- [ ] **Step 2:** Porter les 6 tests Go existants : `test_build_graph`, `test_detect_cycles_no_cycle`, `test_detect_cycles_with_cycle`, `test_detect_cycles_self_loop`, `test_topological_sort_simple`, `test_topological_sort_single_node`, `test_topological_sort_multiple_targets`.

- [ ] **Step 3:** Commit

---

### Task 7: Module docker/orchestrator -- machine à états

**Files:**
- Create: `src-tauri/src/docker/orchestrator.rs`

Port de : `internal/orchestrator/orchestrator.go`

- [ ] **Step 1:** Implémenter les types :
  - `ProjectState` enum : `Stopped, Starting, Running, Stopping, Error`
  - `Project` struct avec `name, path, description, depends_on, depended_by, state, containers, error`
  - `Orchestrator` struct avec `projects: Arc<RwLock<HashMap>>`, `graph`, `reverse`, `composers`

- [ ] **Step 2:** Implémenter `new()`, `get_projects()`, `get_project()`.

- [ ] **Step 3:** Implémenter `start_project()` : résolution topologique, démarrage ordonné, gestion des états. **Correction du bug Go** : les commandes docker (up) doivent être exécutées en dehors du lock, ne verrouiller que pour les changements d'état.

- [ ] **Step 4:** Implémenter `stop_project()` : vérification des dépendants actifs, arrêt, cleanup des dépendances orphelines récursif. Même correction de lock.

- [ ] **Step 5:** Implémenter `restart_project()`, `add_project()`, `update_project()`, `remove_project()`.

- [ ] **Step 6:** Commit

---

### Task 8: Module docker/monitor + scanner

**Files:**
- Create: `src-tauri/src/docker/monitor.rs`
- Create: `src-tauri/src/scanner/mod.rs`

Port de : `internal/status/status.go`, `internal/scanner/scanner.go`

- [ ] **Step 1:** Implémenter `monitor.rs` : `refresh_statuses()` avec le pattern 3 phases (read lock → exec sans lock → write lock). **Correction du bug Go** : accéder à `composers` sous read lock en phase 2, pas sans lock. Boucle ticker async avec `tokio::time::interval`.

- [ ] **Step 2:** Implémenter `scanner/mod.rs` : `scan()` et `scan_subdirs()`. Structs `ScanResult { path, name, compose_files, has_dockerfile }`. Reconnaître les patterns : docker-compose.yml/yaml, compose.yml/yaml, docker-compose.*.yml/yaml, Dockerfile*.

- [ ] **Step 3:** Tests unitaires pour le scanner (créer un tmpdir avec des fichiers compose et vérifier la détection).

- [ ] **Step 4:** Commit

---

## Phase 3 : Commandes Tauri (bridge backend ↔ frontend)

### Task 9: Enregistrer toutes les commandes Tauri

**Files:**
- Create: `src-tauri/src/commands/mod.rs`
- Create: `src-tauri/src/commands/docker.rs`
- Create: `src-tauri/src/commands/storage.rs`
- Create: `src-tauri/src/commands/system.rs`
- Create: `src-tauri/src/commands/scanner.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1:** Créer `commands/docker.rs` : fonctions `#[tauri::command]` pour `list_projects`, `start_project`, `stop_project`, `restart_project`. Chaque commande accède à l'état Tauri via `tauri::State<AppState>`.

- [ ] **Step 2:** Créer `commands/storage.rs` : commandes pour tous les CRUD (notes, todos, urls, projects settings). ~25 commandes.

- [ ] **Step 3:** Créer `commands/system.rs` : commandes `get_system_metrics`, `kill_process`.

- [ ] **Step 4:** Créer `commands/scanner.rs` : commandes `scan_dir`, `scan_subdirs`.

- [ ] **Step 5:** Créer un `AppState` partagé dans `lib.rs` contenant `Database`, `Orchestrator`, `SystemCollector`. Enregistrer toutes les commandes dans `tauri::Builder`.

- [ ] **Step 6:** Configurer les events Tauri pour le monitor (status_update) et les métriques (system_metrics). Lancer les boucles async au setup de l'app.

- [ ] **Step 7:** Vérifier la compilation

```bash
cd ai-workforce && cargo tauri build --debug 2>&1 | tail -5
```

- [ ] **Step 8:** Commit

---

## Phase 4 : Frontend Svelte

### Task 10: Types TypeScript + couche API

**Files:**
- Create: `src/lib/types/index.ts`
- Create: `src/lib/api/docker.ts`
- Create: `src/lib/api/storage.ts`
- Create: `src/lib/api/system.ts`
- Create: `src/lib/api/scanner.ts`

- [ ] **Step 1:** Définir tous les types TypeScript dans `types/index.ts` : `Project`, `ProjectState`, `ContainerStatus`, `Todo`, `Note`, `NoteFolder`, `NoteFile`, `NoteTree`, `Url`, `ScanResult`, `SystemMetrics`, `CPUMetrics`, `MemoryMetrics`, `DiskMetrics`, `GPUMetrics`, `ProcessMetrics`.

- [ ] **Step 2:** Créer les 4 fichiers API. Chaque fichier wrappe `invoke` avec les bons types. Exemple :

```typescript
// api/docker.ts
import { invoke } from "@tauri-apps/api/core";
import type { Project } from "../types";

export const listProjects = () => invoke<Project[]>("list_projects");
export const startProject = (name: string) => invoke("start_project", { name });
export const stopProject = (name: string) => invoke("stop_project", { name });
export const restartProject = (name: string) => invoke("restart_project", { name });
```

- [ ] **Step 3:** Commit

---

### Task 11: Stores Svelte + layout principal

**Files:**
- Create: `src/lib/stores/projects.ts`
- Create: `src/lib/stores/system.ts`
- Create: `src/lib/stores/ui.ts`
- Create: `src/styles/theme.css`
- Create: `src/styles/global.css`
- Create: `src/lib/components/layout/Header.svelte`
- Create: `src/lib/components/layout/Sidebar.svelte`
- Create: `src/lib/components/layout/MainPanel.svelte`
- Modify: `src/App.svelte`

- [ ] **Step 1:** Créer les stores Svelte 5 (`$state` runes) : `projects` (alimenté par event `status_update`), `system` (alimenté par event `system_metrics`), `ui` (selectedProject, activeTab, showSettings, showSystem, theme).

- [ ] **Step 2:** Créer `theme.css` avec les variables CSS dark/light (reprendre les variables de `web/css/base.css` du projet Go). Créer `global.css` avec le reset de base.

- [ ] **Step 3:** Créer `Header.svelte` (titre Cockpit, boutons settings/theme/statut connexion), `Sidebar.svelte` (liste projets cliquable avec indicateur d'état), `MainPanel.svelte` (conteneur conditionnel selon l'état du store ui).

- [ ] **Step 4:** Assembler dans `App.svelte` : layout header + sidebar + main. Initialiser les listeners d'events Tauri au mount.

- [ ] **Step 5:** Tester : `cargo tauri dev` — la fenêtre affiche le layout, la sidebar montre les projets depuis la DB.

- [ ] **Step 6:** Commit

---

### Task 12: Dashboard + ProjectDetail + DockerTab

**Files:**
- Create: `src/lib/components/dashboard/Dashboard.svelte`
- Create: `src/lib/components/dashboard/ProjectCard.svelte`
- Create: `src/lib/components/project/ProjectDetail.svelte`
- Create: `src/lib/components/project/DockerTab.svelte`

- [ ] **Step 1:** `Dashboard.svelte` : grille de cartes projet avec état, todos en attente globaux. `ProjectCard.svelte` : carte avec nom, description, badge d'état, boutons start/stop.

- [ ] **Step 2:** `ProjectDetail.svelte` : conteneur avec onglets (Workspace, Docker, Settings). Switch conditionnel sur l'onglet actif.

- [ ] **Step 3:** `DockerTab.svelte` : affichage des dépendances (depends_on, depended_by), liste des conteneurs avec statut/health/ports, boutons start/stop/restart.

- [ ] **Step 4:** Tester le flux complet : sélectionner un projet dans la sidebar → voir le détail → onglet Docker → start/stop un projet.

- [ ] **Step 5:** Commit

---

### Task 13: WorkspaceTab -- Todos + URLs

**Files:**
- Create: `src/lib/components/project/WorkspaceTab.svelte`
- Create: `src/lib/components/todos/TodoList.svelte`
- Create: `src/lib/components/urls/UrlList.svelte`

- [ ] **Step 1:** `TodoList.svelte` : liste des todos avec checkbox (toggle done), input pour créer, bouton supprimer, drag-and-drop pour réordonner via `svelte-dnd-action`.

```bash
cd ai-workforce && npm install svelte-dnd-action
```

- [ ] **Step 2:** `UrlList.svelte` : liste des URLs avec label cliquable, formulaire ajout/édition, bouton supprimer.

- [ ] **Step 3:** `WorkspaceTab.svelte` : layout avec TodoList + UrlList + zone notes (placeholder pour Task 14).

- [ ] **Step 4:** Commit

---

### Task 14: Notes -- arborescence + éditeur Markdown

**Files:**
- Create: `src/lib/components/notes/NoteTree.svelte`
- Create: `src/lib/components/notes/NoteEditor.svelte`
- Modify: `src/lib/components/project/WorkspaceTab.svelte`

- [ ] **Step 1:** Installer milkdown

```bash
cd ai-workforce && npm install @milkdown/core @milkdown/preset-commonmark @milkdown/theme-nord @milkdown/ctx
```

- [ ] **Step 2:** `NoteTree.svelte` : arborescence de dossiers/fichiers, boutons créer dossier/fichier, renommer, supprimer. Sélection d'un fichier ouvre l'éditeur.

- [ ] **Step 3:** `NoteEditor.svelte` : éditeur Markdown milkdown. Sauvegarde auto avec debounce (timer 1s après la dernière frappe, comme dans le Go : `state.noteSaveTimer`).

- [ ] **Step 4:** Intégrer dans `WorkspaceTab.svelte`.

- [ ] **Step 5:** Commit

---

### Task 15: System monitor

**Files:**
- Create: `src-tauri/src/system/mod.rs`
- Create: `src-tauri/src/system/metrics.rs`
- Create: `src-tauri/src/system/process.rs`
- Create: `src/lib/components/system/SystemMonitor.svelte`
- Create: `src/lib/components/system/ProcessList.svelte`

Port de : `internal/system/system.go`

- [ ] **Step 1:** `metrics.rs` : utiliser la crate `sysinfo` pour collecter CPU (global + per-core), mémoire (total, used, available, swap), disques. Structs `SystemMetrics`, `CpuMetrics`, `MemoryMetrics`, `DiskMetrics`. Pour GPU, tenter `nvidia-smi` via `Command`.

- [ ] **Step 2:** `process.rs` : utiliser `sysinfo` pour lister les processus (PID, name, CPU, memory, user, command). Groupement par nom comme dans le Go (`psGroupedByMem`). `kill_process(pid)` via `sysinfo` ou signal SIGTERM.

- [ ] **Step 3:** `SystemMonitor.svelte` : affichage des métriques CPU (barres par core), mémoire (barre + détail), disques (barres), GPU si disponible. Alimenté par le store `system` (events Tauri).

- [ ] **Step 4:** `ProcessList.svelte` : tableau des processus triés par CPU ou mémoire, groupés par nom avec enfants dépliables, bouton kill.

- [ ] **Step 5:** Commit

---

### Task 16: Settings -- scanner et gestion projets

**Files:**
- Create: `src/lib/components/settings/GlobalSettings.svelte`
- Create: `src/lib/components/project/SettingsTab.svelte`

- [ ] **Step 1:** `GlobalSettings.svelte` : input chemin + bouton "Scanner" (appelle `scan_dir`), bouton "Scanner sous-dossiers" (appelle `scan_subdirs`). Affichage des résultats avec bouton "Ajouter" par projet trouvé. Liste des projets en DB avec édition/suppression.

- [ ] **Step 2:** `SettingsTab.svelte` : formulaire édition du projet sélectionné (nom, chemin, fichier compose, description, dépendances). Sauvegarde via `update_project_settings`.

- [ ] **Step 3:** Commit

---

## Phase 5 : Polish et distribution

### Task 17: Thème dark/light + persistence

**Files:**
- Modify: `src/lib/stores/ui.ts`
- Modify: `src/lib/components/layout/Header.svelte`

- [ ] **Step 1:** Installer `@tauri-apps/plugin-store`. Persister le thème choisi. Au mount, lire la préférence et appliquer la classe CSS `dark` ou `light` sur `<html>`.

- [ ] **Step 2:** Bouton toggle dans le Header.

- [ ] **Step 3:** Commit

---

### Task 18: Plugin trait (préparation future)

**Files:**
- Create: `src-tauri/src/plugin/mod.rs`

- [ ] **Step 1:** Créer le trait `Plugin` minimal :

```rust
use tauri::AppHandle;
use anyhow::Result;

pub trait Plugin: Send + Sync {
    fn name(&self) -> &str;
    fn init(&self, app: &AppHandle) -> Result<()>;
}
```

- [ ] **Step 2:** Commit

---

### Task 19: CI/CD GitHub Actions

**Files:**
- Create: `.github/workflows/release.yml`

- [ ] **Step 1:** Créer le workflow release déclenché sur tag `v*`. 3 jobs (ubuntu, macos, windows) utilisant `tauri-apps/tauri-action@v0`. Génère les binaires et crée une GitHub Release.

- [ ] **Step 2:** Générer les clés de signature : `cargo tauri signer generate`. Stocker la clé publique dans `tauri.conf.json`, la clé privée comme secret GitHub.

- [ ] **Step 3:** Configurer `tauri-plugin-updater` dans `tauri.conf.json` pour vérifier les updates via GitHub Releases.

- [ ] **Step 4:** Commit

---

### Task 20: Icônes et packaging final

- [ ] **Step 1:** Générer les icônes depuis `cockpit.svg` :

```bash
cargo tauri icon ../cockpit.svg
```

- [ ] **Step 2:** Build de production et test :

```bash
cargo tauri build
```

Vérifier que le .deb/.AppImage est généré et fonctionne.

- [ ] **Step 3:** Commit final

```bash
git add -A
git commit -m "feat: Cockpit v0.1.0 - Tauri desktop app"
git tag v0.1.0
```
