# Cockpit -- Tauri + Rust + Svelte Rewrite

**Date** : 2026-03-30
**Statut** : Draft
**Projet cible** : `ai-workforce/` (Tauri v2 + Rust + Svelte 5 + TypeScript)

---

## 1. Contexte

Cockpit est un dashboard web de gestion de projets Docker Compose. Il tourne actuellement comme un serveur Go + frontend vanilla JS ouvert dans un navigateur. L'objectif est de le transformer en application desktop native cross-platform (Linux, macOS, Windows) avec auto-update intégrée.

A terme, l'application deviendra un pilote IA complet : intégration Claude Code, gestion de tickets, connecteurs, etc. L'architecture doit être pensée pour cette extensibilité, même si la v1 reprend uniquement les fonctionnalités existantes.

## 2. Décisions techniques

| Choix | Décision | Raison |
|-------|----------|--------|
| Framework desktop | **Tauri v2** | Léger (~15-20 Mo), cross-platform, auto-update natif |
| Backend | **Rust** | Langage natif de Tauri, intégration directe |
| Frontend | **Svelte 5 + TypeScript** | Léger, réactif, recommandé par Tauri |
| Base de données | **SQLite via rusqlite** | Même schéma que l'existant, embarqué |
| Métriques système | **sysinfo** (crate Rust) | Cross-platform sans code conditionnel |
| Éditeur Markdown | **milkdown** | Intégré au bundle, remplace Toast UI CDN |
| Auto-update | **tauri-plugin-updater** | Natif Tauri, supporte GitHub Releases + custom |
| Build frontend | **Vite** | Standard Tauri + Svelte |
| CI/CD | **GitHub Actions + tauri-action** | Build 3 plateformes, release automatique |

## 3. Architecture

```
┌─────────────────────────────────────────────────────────┐
│                    Tauri App (Rust)                      │
│                                                         │
│  ┌──────────────┐    IPC (invoke/events)   ┌──────────┐│
│  │   Frontend    │◄──────────────────────► │  Backend  ││
│  │   Svelte 5 + │                         │   Rust    ││
│  │  TypeScript   │                         │           ││
│  │               │                         │ Modules:  ││
│  │  Pages:       │                         │ • docker  ││
│  │  • Dashboard  │                         │ • storage ││
│  │  • Projet     │                         │ • system  ││
│  │  • Settings   │                         │ • scanner ││
│  │  • System     │                         │ • plugin  ││
│  └──────────────┘                         │           ││
│       WebView                              │ SQLite    ││
│       native                               │(rusqlite) ││
│                                            └──────────┘│
│  Plugins Tauri:                                         │
│  • updater, notification, shell, store                  │
└─────────────────────────────────────────────────────────┘
```

Communication frontend ↔ backend via IPC Tauri :
- **Commandes** (`invoke`) : le frontend appelle des fonctions Rust typées
- **Events** (`emit`/`listen`) : le backend push les mises à jour en temps réel (statuts, métriques)

Plus de serveur HTTP ni de WebSocket.

## 4. Backend Rust -- Structure des modules

```
src-tauri/src/
├── main.rs                 # Point d'entrée Tauri
├── lib.rs                  # Enregistrement commandes et plugins
│
├── docker/
│   ├── mod.rs
│   ├── compose.rs          # Exec docker compose (up/down/ps) via tauri-plugin-shell
│   ├── orchestrator.rs     # États projets, démarrage ordonné par dépendances
│   ├── graph.rs            # Tri topologique, détection de cycles
│   └── monitor.rs          # Boucle de refresh des statuts → emit events
│
├── storage/
│   ├── mod.rs
│   ├── db.rs               # Init SQLite, migrations, pool
│   ├── projects.rs         # CRUD projets
│   ├── notes.rs            # Notes simples + arborescence dossiers/fichiers
│   ├── todos.rs            # CRUD todos + réordonnancement
│   └── urls.rs             # CRUD URLs
│
├── system/
│   ├── mod.rs
│   ├── metrics.rs          # CPU, RAM, disques, GPU via sysinfo
│   └── process.rs          # Liste processus, kill
│
├── scanner/
│   └── mod.rs              # Scan filesystem pour docker-compose.yml
│
└── plugin/
    └── mod.rs              # Trait Plugin (préparation extensibilité future)
```

### Commandes Tauri exposées au frontend

**Docker :**
- `list_projects() -> Vec<Project>`
- `start_project(name: String) -> Result<()>`
- `stop_project(name: String) -> Result<()>`
- `restart_project(name: String) -> Result<()>`

**Storage -- Todos :**
- `get_todos(project: String) -> Vec<Todo>`
- `create_todo(project: String, text: String) -> Todo`
- `update_todo(id: i64, text: String, done: bool) -> Result<()>`
- `delete_todo(id: i64) -> Result<()>`
- `reorder_todos(project: String, ids: Vec<i64>) -> Result<()>`
- `get_pending_todos() -> Vec<TodoWithProject>`

**Storage -- Notes :**
- `get_note(project: String) -> Option<Note>`
- `save_note(project: String, content: String) -> Result<()>`
- `get_note_tree(project: String) -> NoteTree`
- `create_note_folder(project: String, parent_id: Option<i64>, name: String) -> NoteFolder`
- `rename_note_folder(id: i64, name: String) -> Result<()>`
- `delete_note_folder(id: i64) -> Result<()>`
- `create_note_file(project: String, folder_id: i64, name: String) -> NoteFile`
- `get_note_file(id: i64) -> NoteFile`
- `save_note_file(id: i64, content: String) -> Result<()>`
- `rename_note_file(id: i64, name: String) -> Result<()>`
- `delete_note_file(id: i64) -> Result<()>`

**Storage -- URLs :**
- `get_urls(project: String) -> Vec<Url>`
- `create_url(project: String, label: String, url: String) -> Url`
- `update_url(id: i64, label: String, url: String) -> Result<()>`
- `delete_url(id: i64) -> Result<()>`

**Settings :**
- `scan_dir(path: String) -> Vec<ScanResult>`
- `scan_subdirs(path: String) -> Vec<ScanResult>`
- `get_db_projects() -> Vec<DbProject>`
- `add_project(name: String, path: String, compose_file: String) -> DbProject`
- `update_project(id: i64, ...) -> Result<()>`
- `delete_project(id: i64) -> Result<()>`
- `get_project_settings(name: String) -> ProjectSettings`
- `update_project_settings(name: String, ...) -> Result<()>`
- `reorder_projects(ids: Vec<i64>) -> Result<()>`

**System :**
- `get_system_metrics() -> SystemMetrics`
- `kill_process(pid: u32) -> Result<()>`

### Events Tauri (backend → frontend)

- `status_update` : émis toutes les 5s par le monitor, payload `Vec<Project>`
- `system_metrics` : émis toutes les 3s, payload `SystemMetrics`

## 5. Frontend Svelte -- Structure

```
src/
├── App.svelte                  # Layout principal (header + sidebar + main)
├── main.ts                     # Point d'entrée, init listeners events
│
├── lib/
│   ├── api/
│   │   ├── docker.ts           # Wrappers invoke pour docker
│   │   ├── storage.ts          # Wrappers invoke pour notes/todos/urls
│   │   ├── system.ts           # Wrappers invoke pour métriques
│   │   └── scanner.ts          # Wrappers invoke pour scan/settings
│   │
│   ├── stores/
│   │   ├── projects.ts         # Store réactif : projets + états (alimenté par event)
│   │   ├── system.ts           # Store réactif : métriques système (alimenté par event)
│   │   └── ui.ts               # Store : thème, projet sélectionné, onglet actif
│   │
│   ├── components/
│   │   ├── layout/
│   │   │   ├── Header.svelte
│   │   │   ├── Sidebar.svelte
│   │   │   └── MainPanel.svelte
│   │   │
│   │   ├── dashboard/
│   │   │   ├── Dashboard.svelte
│   │   │   └── ProjectCard.svelte
│   │   │
│   │   ├── project/
│   │   │   ├── ProjectDetail.svelte
│   │   │   ├── DockerTab.svelte
│   │   │   ├── WorkspaceTab.svelte
│   │   │   └── SettingsTab.svelte
│   │   │
│   │   ├── notes/
│   │   │   ├── NoteTree.svelte
│   │   │   └── NoteEditor.svelte
│   │   │
│   │   ├── todos/
│   │   │   └── TodoList.svelte
│   │   │
│   │   ├── urls/
│   │   │   └── UrlList.svelte
│   │   │
│   │   ├── system/
│   │   │   ├── SystemMonitor.svelte
│   │   │   └── ProcessList.svelte
│   │   │
│   │   └── settings/
│   │       └── GlobalSettings.svelte
│   │
│   └── types/
│       └── index.ts            # Types partagés
│
└── styles/
    ├── theme.css               # Variables CSS dark/light
    └── global.css              # Reset + styles de base
```

### Navigation

Pas de routeur. Navigation par état dans le store `ui` :
- `selectedProject === null` → Dashboard
- `selectedProject !== null` → ProjectDetail avec onglets (workspace, docker, settings)
- Vue settings globale via un flag `showSettings`
- Vue system via un flag `showSystem`

### Éditeur Markdown

milkdown intégré dans `NoteEditor.svelte`. Remplace Toast UI Editor (CDN).

### Drag-and-drop

`svelte-dnd-action` pour :
- Réordonnancement des todos
- Réordonnancement des projets dans la sidebar/dashboard

### Thème

Variables CSS dans `theme.css`. Toggle dark/light persisté via `tauri-plugin-store`.

## 6. Base de données

Schéma SQLite identique à l'existant :

```sql
CREATE TABLE projects (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL UNIQUE,
    path TEXT NOT NULL,
    compose_file TEXT NOT NULL DEFAULT 'docker-compose.yml',
    description TEXT NOT NULL DEFAULT '',
    depends_on TEXT NOT NULL DEFAULT '[]',
    position INTEGER NOT NULL DEFAULT 0,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE notes (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    project TEXT NOT NULL,
    content TEXT NOT NULL DEFAULT '',
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE note_folders (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    project TEXT NOT NULL,
    parent_id INTEGER,
    name TEXT NOT NULL,
    position INTEGER NOT NULL DEFAULT 0,
    FOREIGN KEY (parent_id) REFERENCES note_folders(id) ON DELETE CASCADE
);

CREATE TABLE note_files (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    project TEXT NOT NULL,
    folder_id INTEGER NOT NULL,
    name TEXT NOT NULL,
    content TEXT NOT NULL DEFAULT '',
    position INTEGER NOT NULL DEFAULT 0,
    updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (folder_id) REFERENCES note_folders(id) ON DELETE CASCADE
);

CREATE TABLE todos (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    project TEXT NOT NULL,
    text TEXT NOT NULL,
    done BOOLEAN NOT NULL DEFAULT 0,
    position INTEGER NOT NULL DEFAULT 0,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE urls (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    project TEXT NOT NULL,
    label TEXT NOT NULL,
    url TEXT NOT NULL,
    position INTEGER NOT NULL DEFAULT 0
);

CREATE INDEX idx_notes_project ON notes(project);
CREATE INDEX idx_note_folders_project ON note_folders(project);
CREATE INDEX idx_note_files_folder ON note_files(folder_id);
CREATE INDEX idx_todos_project ON todos(project);
CREATE INDEX idx_urls_project ON urls(project);
```

La DB existante `data.db` est directement compatible.

## 7. Auto-update et distribution

### Plugins Tauri

- `tauri-plugin-updater` : vérifie et applique les mises à jour
- Source configurable : GitHub Releases (défaut) ou serveur custom

### Flux de mise à jour

1. Au démarrage, l'app vérifie silencieusement si une nouvelle version existe
2. Si oui, notification dans l'UI : "Nouvelle version X.Y.Z disponible"
3. L'utilisateur clique "Installer"
4. Téléchargement du binaire signé → vérification signature → remplacement → redémarrage

### CI/CD -- GitHub Actions

Workflow `release.yml` déclenché sur push d'un tag `v*` :
1. Build sur 3 runners : `ubuntu-latest`, `macos-latest`, `windows-latest`
2. `tauri-action` compile, signe et package
3. Crée une GitHub Release avec les artifacts (.deb, .AppImage, .dmg, .msi)
4. Publie le manifest d'update (`latest.json`)

### Signature

Paire de clés générée par `tauri signer generate`. Clé publique embarquée dans `tauri.conf.json`, clé privée stockée comme secret GitHub Actions.

## 8. Corrections par rapport à l'existant

Problèmes identifiés dans l'analyse du projet Go qui seront corrigés dans la réécriture :

| Problème Go | Correction Rust |
|-------------|-----------------|
| Deadlock dans WSHub.Broadcast | Plus de WebSocket, IPC Tauri thread-safe |
| Race condition sur composers map | `Arc<RwLock<>>` avec scopes limités |
| Mutex bloquant pendant docker up/down | Commandes async via tokio, lock uniquement pour les changements d'état |
| Pas d'auth (0.0.0.0) | Plus de serveur HTTP, app locale uniquement |
| Path traversal dans scanner | Validation du chemin côté Rust + permissions Tauri |
| Kill processus arbitraire | Vérification que le PID appartient à un conteneur Docker |
| os.Exit empêche defer | Shutdown propre via Tauri lifecycle |
| Pas de tests | Tests unitaires pour chaque module Rust |
| CDN Toast UI non sécurisé | milkdown embarqué dans le bundle |

## 9. Module plugin (préparation future)

Trait simple posant les bases pour l'extensibilité :

```rust
pub trait Plugin: Send + Sync {
    fn name(&self) -> &str;
    fn init(&self, app: &AppHandle) -> Result<()>;
}
```

Non implémenté en v1. Prévu pour les futures intégrations :
- Connecteurs externes
- Intégration Claude Code
- Gestion de tickets
- Terminal intégré

## 10. Ordre d'implémentation

| Étape | Module | Contenu |
|-------|--------|---------|
| 1 | Scaffold | `tauri init`, Svelte, structure de base, config Tauri |
| 2 | Storage | rusqlite, migrations, CRUD projets/notes/todos/urls + tests |
| 3 | Docker | compose.rs (exec), orchestrator.rs (états), graph.rs (dépendances) + tests |
| 4 | Frontend core | Layout (Header, Sidebar, MainPanel), stores, thème |
| 5 | Frontend détail | ProjectDetail, DockerTab, WorkspaceTab, SettingsTab |
| 6 | System | Métriques via sysinfo, ProcessList + tests |
| 7 | Notes/Todos/URLs | UI complète avec éditeur Markdown et drag-and-drop |
| 8 | Settings | Scanner, gestion projets, GlobalSettings |
| 9 | Monitor | Boucle refresh statuts + métriques → events Tauri |
| 10 | Auto-update | Plugin updater, CI/CD GitHub Actions, signature |

## 11. Arborescence finale du projet

```
ai-workforce/
├── src-tauri/
│   ├── Cargo.toml
│   ├── tauri.conf.json
│   ├── capabilities/
│   │   └── default.json
│   ├── icons/
│   └── src/
│       ├── main.rs
│       ├── lib.rs
│       ├── docker/
│       ├── storage/
│       ├── system/
│       ├── scanner/
│       └── plugin/
│
├── src/
│   ├── App.svelte
│   ├── main.ts
│   ├── lib/
│   │   ├── api/
│   │   ├── stores/
│   │   ├── components/
│   │   └── types/
│   └── styles/
│
├── package.json
├── vite.config.ts
├── tsconfig.json
├── .github/
│   └── workflows/
│       └── release.yml
├── docs/
│   └── superpowers/
│       └── specs/
│           └── 2026-03-30-cockpit-tauri-rewrite-design.md
└── README.md
```
