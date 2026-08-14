# Cockpit (ai-workforce)

Application desktop qui regroupe tout ce qui tourne autour d'un projet : terminaux persistants,
notes, fichiers, Git, conteneurs, monitoring. Construite en Tauri v2 + Rust + Svelte 5 + TypeScript.

**Positionnement** : Docker n'est qu'UN onglet parmi huit, et le fichier compose est optionnel
(`compose_file TEXT NOT NULL DEFAULT ''`). Ne pas remettre Docker en avant dans le README ni dans
la description du repo — un projet Cockpit, c'est un nom et un dossier.

Repo : `github.com/jguevel-tech/cockpit` (public, MIT). Compte `jguevel-tech`, **distinct** du
compte `jguevel` utilise chez CCM — ne jamais melanger les deux.

## Workflow IA — a lire en premier a chaque session

**Ce repo est pilote a 100 % par l'IA.** Jimmy demande des fonctionnalites, l'IA s'occupe de tout
le reste : code, tests, changelog, numero de version, commit, push, release. Il ne doit avoir a
rappeler aucune de ces etapes.

### A chaque fonctionnalite

1. **Coder**, en respectant les regles non negociables ci-dessous.
2. **Verifier** — les 4 points de la definition de "fini". Aucun n'est optionnel.
3. **Consigner** dans `CHANGELOG.md` sous `## [Unreleased]`, section Added / Changed / Fixed /
   Removed. Uniquement si l'utilisateur peut le constater ; une refonte interne n'y a pas sa place.
4. **Commiter et pousser sur `main`** — libre, aucune confirmation a demander. Un push de branche
   ne declenche aucun deploiement (le workflow ne reagit qu'aux tags `v*`).
5. **Releaser** : `npm run release -- <patch|minor|major>` puis pousser le tag.

### Politique de release

**Une fonctionnalite = une release.** C'est la regle par defaut : ce qui est fini part, on
n'accumule pas dans `[Unreleased]` en attendant un lot. Jimmy demande des fonctionnalites, elles
arrivent chez les utilisateurs.

**Plusieurs fonctionnalites dans une meme release, c'est bon** — si elles sont terminees ensemble
ou dependent l'une de l'autre, une seule version les embarque. Ce qu'il faut eviter, c'est le
contraire : une fonctionnalite finie qui dort des jours dans `[Unreleased]`.

**Choix du niveau — c'est a l'IA de trancher, pas de demander.** La regle est deterministe et se
lit dans le contenu de `[Unreleased]` :

| Contenu de `[Unreleased]` | Niveau |
|---|---|
| Seulement `### Fixed` | `patch` |
| Au moins un `### Added` ou `### Changed` visible | `minor` |
| Un `### Removed`, ou un `Changed` qui casse un usage existant | `minor` en 0.x, `major` a partir de 1.0.0 |

**Pourquoi une rupture n'est pas un `major` en 0.x** : SemVer est explicite sur la 0.y.z
(« Anything MAY change at any time. The public API SHOULD NOT be considered stable. »). Publier
une 1.0.0 pour une suppression de fonctionnalite signalerait une stabilite que le projet n'a pas
encore atteinte. Le script applique cette regle et redeviendra strict des la 1.0.0.

`scripts/release.mjs` refuse les incoherences (un `Added` avec un bump `patch`, un `Removed` sans
`major`), donc une erreur de jugement est rattrapee avant le tag. En cas de doute entre deux
niveaux, prendre le plus eleve : une version de trop ne coute rien, une rupture annoncee comme un
patch trompe les utilisateurs.

**Ne jamais laisser un numero de version au hasard** : `package.json` est la source unique, et
seul le script y touche.

### Messages de commit

**JAMAIS de `Co-Authored-By: Claude` ni aucune mention d'IA.** Claude Code l'ajoute par defaut, il
faut activement l'omettre. Le message decrit le changement, pas l'outil.

Style attendu : une ligne de titre a l'imperatif, puis un corps qui explique **pourquoi**, pas quoi
(le diff dit deja quoi). Mentionner ce qui a ete verifie.

### Outils disponibles

`gh` est installe et authentifie sur `jguevel-tech`. L'IA peut donc lire les logs de CI, diagnostiquer
un build rate, gerer les secrets et les releases seule — sans jamais demander a Jimmy de copier des
logs. En cas d'echec de CI : `gh run view <id> --log-failed`.

### Pieges d'environnement

- **Registre npm** : la config npm globale de la machine pointe sur le registre prive CCM
  (`npm.ccmbg.com`). Le `.npmrc` du projet la surcharge vers le registre public — **ne pas le
  retirer**, sinon `npm ci` echoue en E401 sur le runner et un hostname interne fuite dans un repo
  public. Si le `package-lock.json` doit etre regenere : supprimer `node_modules` AVANT, sinon
  npm reutilise les metadonnees de l'arbre existant et conserve les anciennes URLs.
- **Codes de sortie** : ne jamais lire `$?` derriere un pipe (`cmd | tail`) — c'est celui du dernier
  maillon. Rediriger vers un fichier puis tester, sinon on annonce des succes inexistants.
- **Sorties de `grep`** : le proxy `rtk` les reformate et fausse les `grep -c`. Passer par
  `rtk proxy grep ...` quand le comptage compte.

## Regles non negociables (a lire AVANT de coder)

**Definition de "fini"** — une modification n'est livrable que si ces 4 points passent :
1. `npm run check` -> 0 erreur, 0 warning (c'est l'etat actuel, le maintenir)
2. `cd src-tauri && cargo test` -> tous verts
3. `npx tauri build --no-bundle` si on livre un binaire (JAMAIS `cargo build --release` seul :
   sans les env vars Tauri le binaire sort en mode dev et cherche Vite sur localhost:5173)
4. **Toute modification visible par l'utilisateur est consignee dans `CHANGELOG.md` sous
   `## [Unreleased]`**, dans la bonne section (Added / Changed / Fixed / Removed). Ce texte
   n'est pas de la doc interne : il est affiche dans le logiciel ET sert de notes de version
   dans le modal de mise a jour. Une refonte interne sans effet visible n'a rien a y faire.

**Interdits absolus** :
- Retirer ou "simplifier" du code marque `NE PAS RETIRER` (fixes accents/IME de TerminalTab.svelte
  et `GTK_IM_MODULE` dans lib.rs — bug diagnostique en 8 iterations douloureuses, voir Pieges connus)
- Ajouter une surcouche sur le chemin de frappe xterm (`onData` -> PTY doit rester direct)
- Couleur/taille en dur dans un composant : uniquement les tokens de `styles/theme.css`
- `catch {}` muet ou `catch (e: any)` : toujours `catch (e) { notify(String(e)); }`
- SQL : valeurs toujours en parametres `?`, jamais interpolees (les noms de tables/colonnes
  en `format!()` doivent etre des constantes hardcodees)
- **Un controle cliquable ecrit autrement qu'avec un vrai `<button>`** (pas de `<div onclick>`,
  pas de `<span role="button">`). Voir la regle « Tout controle doit rester visible » ci-dessous :
  c'est le selecteur `button` qui garantit sa visibilite sur une image de fond. Un `div` y echappe
  silencieusement.
- Retirer le `!important` de la couche `html.has-wallpaper` de `components.css` : il est
  delibere et documente sur place.

**Tout controle doit rester visible, y compris sur une image de fond** :
- Le mode image de fond rend les surfaces translucides. Un bouton sans fond propre — et c'est le
  cas de la majorite dans ce projet (58 `background: none` dans 25 composants) — devient alors
  du texte flottant sur une photo, illisible.
- Une couche d'override dans `components.css` (`html.has-wallpaper:root button...`) donne
  automatiquement un fond a **tout** `<button>`, `<select>` et `<summary>`. Rien a faire en
  ecrivant un nouveau composant, A CONDITION d'utiliser un vrai element interactif.
- Sont exclus de l'override, volontairement : `.primary`, `.danger`, `.active` (ils portent deja
  une couleur porteuse de sens), `.logo-btn`, les `input` de type checkbox/radio/range/color, et
  tout le `.term-container`.
- **Un nouveau CONTENEUR structurel** (barre d'onglets, panneau lateral, en-tete de section) doit
  etre ajoute a la liste des conteneurs floutes de `components.css`. C'est l'oubli qui a rendu la
  sidebar illisible en v0.5.0 : le flou n'etait pose que sur les cartes.
- Reflexe de verification : activer une image de fond chargee et parcourir l'ecran ajoute. Un
  contraste correct en theme sombre uni ne prouve rien.

**Reflexes obligatoires** :
- **Tout overlay `position: fixed` (modal, menu contextuel, panneau, toast) doit porter
  `use:portal`** (actions/portal.ts, le deplace dans `<body>`). Raison : en mode image de fond,
  les conteneurs structurels portent `isolation: isolate` (components.css) — chacun est un
  contexte d'empilement, et un overlay reste enfant d'un de ces conteneurs est peint SOUS les
  conteneurs suivants du DOM, quel que soit son z-index. Constate le 2026-08-14 : le modal de
  creation de projet, enfant de la sidebar, etait invisible des qu'un wallpaper etait actif.
- Nouvelle table referencant un projet -> l'ajouter a `PROJECT_SCOPED_TABLES` (storage/projects.rs),
  sinon delete/rename laisseront des donnees orphelines
- Modal, rename inline, menu contextuel, toast, DnD de liste -> utiliser `components/ui/`,
  `actions/reorderable.ts`, `stores/toast.ts` AVANT d'ecrire du neuf
- Nouvelle vue top-niveau -> etendre `activeView` (stores/ui.ts) + un case dans MainPanel ;
  nouvel onglet projet -> 1 entree dans la map `tabs` de ProjectDetail.svelte
- Nouvelle commande Tauri -> wrapper type dans `src/lib/api/`, types partages dans
  `src/lib/types/index.ts` en snake_case (aligne sur les structs Rust Serialize)
- Svelte 5 runes uniquement : `$state`/`$derived`/`$props` + callback props
  (pas de createEventDispatcher, pas de stores locaux inutiles)
- Commandes externes (git, docker, tmux...) : args en tableau via Command, jamais `sh -c` interpole
- Bug a corriger -> reproduire et instrumenter AVANT de patcher (lecon du bug accents) ;
  ne jamais enchainer des correctifs hypothetiques

## Stack technique

| Couche | Technologie | Version |
|--------|-------------|---------|
| Desktop framework | Tauri | v2 (plugins shell, store, opener) |
| Backend | Rust | edition 2021 |
| Frontend | Svelte | v5 (runes mode) |
| Langage frontend | TypeScript | v6 |
| Build frontend | Vite | v8 |
| Base de donnees | SQLite | rusqlite 0.31 (bundled) |
| Metriques systeme | sysinfo | 0.30 |
| Async runtime | tokio | 1 (via Tauri) |
| HTTP client | reqwest 0.12 | rustls, json, multipart (APIs OpenAI) |
| PTY | portable-pty 0.9 | terminaux integres + flow claude setup-token |
| Persistance terminaux | tmux >= 3 | socket dedie `-L cockpit` ; statique 3.5a EMBARQUE dans l'AppImage |
| Scan fichiers | ignore 0.4 | walker gitignore-aware (celui de ripgrep) |
| Dates | chrono 0.4 | titres de notes reunion |
| Terminal frontend | @xterm/xterm | + addon-fit + addon-webgl + addon-web-links (Ctrl+clic) |
| Presse-papier | arboard 3 | copie OSC 52 des terminaux -> systeme |
| Go-to-definition | LSP (intelephense, rust-analyzer...) | client stdio maison (`src-tauri/src/lsp/`) |
| Coloration code | shiki | bundle fin ~30 langages (`src/lib/shiki.ts`) |
| Markdown rendu | marked | (frontend) |
| HTML -> Markdown | turndown | (frontend, pour editeur WYSIWYG) |

Dependances systeme runtime : `pw-record`/PipeWire (enregistrement reunions), `git` (onglet Git),
CLI `claude` (connexion abonnement + sessions). `tmux` n'en est PLUS une : un tmux statique
(musl, construit par `scripts/build-tmux-static.sh` dans un conteneur Alpine, checksum epingle)
est embarque comme ressource de l'AppImage. Resolution au demarrage (terminal/setup_bundled_tmux) :
1) binaire deja deploye dans `<app_data>/bin/tmux` (les sessions vivantes tournent dessus, on s'y
tient), 2) tmux systeme, 3) deploiement du binaire embarque — COPIE hors du montage AppImage,
car le montage disparait a la fermeture alors que le serveur tmux doit survivre. Le remplacement
du binaire deploye n'a lieu que si AUCUN serveur ne tourne (protocol version mismatch sinon).

## Commandes

```bash
# Dev avec hot-reload
npx tauri dev

# Build production (binaire dans src-tauri/target/release/cockpit)
# --no-bundle saute le packaging AppImage/deb (plus rapide)
npx tauri build --no-bundle

# ATTENTION : ne JAMAIS builder le binaire final avec `cargo build --release` seul —
# sans les env vars de la CLI Tauri il sort en mode dev et cherche Vite sur
# localhost:5173 (ecran "Could not connect to localhost").

# Build frontend seul
npm run build

# Tests Rust (85 tests)
cd src-tauri && cargo test

# Verification types frontend (0 erreur attendu)
npm run check

# Check compilation Rust sans build
cd src-tauri && cargo check

# Lancer le binaire release directement
./src-tauri/target/release/cockpit

# Pointer vers une DB specifique
COCKPIT_DB=/chemin/vers/data.db ./src-tauri/target/release/cockpit
```

## Dependances systeme (Linux)

```bash
sudo apt-get install -y libgtk-3-dev libwebkit2gtk-4.1-dev librsvg2-dev patchelf
```

## Architecture

```
┌───────────────────────────────────────────────────────────┐
│                     Tauri App (Rust)                       │
│                                                           │
│  ┌──────────────┐    IPC (invoke/events)   ┌────────────┐│
│  │   Frontend    │◄──────────────────────► │   Backend   ││
│  │   Svelte 5    │                         │    Rust     ││
│  │  TypeScript   │                         │             ││
│  └──────────────┘                         │  Modules:   ││
│       WebView                              │  docker/    ││
│       native                               │  storage/   ││
│                                            │  terminal/  ││
│                                            │  workspace/ ││
│                                            │  gitdiff/   ││
│                                            │  lsp/       ││
│                                            │  recorder/  ││
│                                            │  claude_auth││
│                                            │  system/    ││
│                                            │  scanner/   ││
│                                            │  agents/    ││
│                                            │  plugin/    ││
│                                            └────────────┘│
└───────────────────────────────────────────────────────────┘
```

Communication frontend <-> backend via IPC Tauri :
- **invoke** : le frontend appelle des fonctions Rust typees
- **events** : le backend push des mises a jour en temps reel (status_update, system_metrics_tick)

Pas de serveur HTTP ni de WebSocket.

## Arborescence du projet

```
ai-workforce/
├── src-tauri/                      # Backend Rust
│   ├── Cargo.toml                  # Dependances Rust
│   ├── tauri.conf.json             # Config Tauri (fenetre, plugins, build)
│   ├── capabilities/default.json   # Permissions Tauri v2
│   ├── build.rs
│   ├── icons/
│   └── src/
│       ├── main.rs                 # Point d'entree
│       ├── lib.rs                  # AppState, commandes Tauri, setup, import DB
│       ├── docker/
│       │   ├── compose.rs          # Wrapper docker compose (up/down/ps) async
│       │   ├── graph.rs            # Tri topologique, detection de cycles (7 tests)
│       │   ├── orchestrator.rs     # Machine a etats (stopped/starting/running/stopping/error)
│       │   └── monitor.rs          # Boucle refresh statuts toutes les 5s
│       ├── lsp/
│       │   └── mod.rs              # Client LSP stdio minimal (goto definition), 1 serveur/projet+langage
│       ├── storage/
│       │   ├── db.rs               # Init SQLite, WAL mode, migrations
│       │   ├── import.rs           # Import ancienne DB Go (transactionnel)
│       │   ├── projects.rs         # CRUD projets + PROJECT_SCOPED_TABLES + rename auto-reparant
│       │   ├── project_folders.rs  # Dossiers hierarchiques de projets
│       │   ├── notes.rs            # Notes simples + arborescence dossiers/fichiers
│       │   ├── todos.rs            # CRUD todos + reorder + pending cross-projet
│       │   ├── urls.rs             # CRUD URLs
│       │   ├── settings.rs         # Cle/valeur globales (upsert)
│       │   ├── recordings.rs       # Suivi pipeline reunions + summary_prompt par projet
│       │   ├── terminals.rs        # Metadonnees terminaux persistants (nom, session tmux)
│       ├── recorder/
│       │   ├── mod.rs              # Pipeline reunion (recording -> transcribing -> summarizing -> done/error)
│       │   ├── capture.rs          # 2x pw-record (micro + monitor sink), PCM brut s16 mono 16 kHz
│       │   ├── wav.rs              # WAV en memoire par chunk, detection silence (2 tests)
│       │   ├── transcribe.rs       # OpenAI whisper-1 (chunks 10 min), fusion dialogue Moi/Eux (3 tests)
│       │   └── summarize.rs        # OpenAI chat completions, prompt systeme editable
│       ├── terminal/
│       │   ├── mod.rs              # Terminaux persistants : sessions tmux, client attach frais en PTY,
│       │   │                       #   detection agents LLM (flag llm), copie OSC 52
│       │   └── history.rs          # Historique commandes (DB + zsh/bash history fusionnes, recherche)
│       ├── workspace/
│       │   ├── mod.rs              # Explorateur fichiers : listing gitignore-aware, lecture/ECRITURE, find_symbol
│       │   └── claude_sessions.rs  # Sessions Claude Code du projet (~/.claude/projects/*.jsonl) + renommage
│       ├── claude_auth/
│       │   └── mod.rs              # Statut connexion abonnement + flow `claude setup-token` en PTY
│       ├── gitdiff/
│       │   └── mod.rs              # git status/diff par shell-out, parser unified diff
│       ├── system/
│       │   ├── metrics.rs          # CPU, RAM (detail: cached/buffers/shmem/zfs_arc), disques
│       │   └── process.rs          # Liste processus groupes, kill via SIGTERM
│       ├── scanner/
│       │   └── mod.rs              # Scan filesystem pour docker-compose.yml (2 tests)
│       └── plugin/
│           └── mod.rs              # Trait Plugin (preparation future)
│
├── src/                            # Frontend Svelte 5 + TypeScript
│   ├── App.svelte                  # Layout principal (Header + Sidebar + MainPanel)
│   ├── main.ts                     # Point d'entree, mount Svelte
│   ├── lib/
│   │   ├── api/                    # Wrappers invoke() vers le backend Rust
│   │   │   ├── docker.ts           # listProjects, startProject, stopProject, restartProject
│   │   │   ├── storage.ts          # CRUD todos, notes, urls (~25 fonctions)
│   │   │   ├── workspace.ts        # Terminaux, fichiers, git, sessions Claude, historique, auth Claude
│   │   │   ├── recorder.ts         # Enregistrement reunions + app settings
│   │   │   ├── system.ts           # getSystemMetrics, killProcess
│   │   │   └── scanner.ts          # scanDir, scanSubdirs, gestion projets DB
│   │   ├── shiki.ts                # Highlighter code (bundle fin, themes github dark/light)
│   │   ├── actions/
│   │   │   └── reorderable.ts      # Action Svelte DnD de reordonnancement (classes globales components.css)
│   │   ├── utils/
│   │   │   ├── reorder.ts          # reorder(list, from, to, pos) + groupBy(list, keyFn)
│   │   │   └── format.ts           # formatBytes
│   │   ├── stores/                 # Stores Svelte reactifs
│   │   │   ├── projects.ts         # Liste projets, alimente par event status_update
│   │   │   ├── recording.ts        # Statut pipeline reunion (event recording_status)
│   │   │   ├── system.ts           # Metriques systeme + historique CPU/mem (60 pts FIFO)
│   │   │   ├── toast.ts            # notify(message, kind) — feedback non bloquant (erreurs/succes)
│   │   │   └── ui.ts               # Navigation (activeView enum, selectedProject, activeTab, dashboardView, pendingTerminalId, theme)
│   │   ├── components/
│   │   │   ├── ui/                 # Composants partages (a utiliser AVANT de recoder)
│   │   │   │   ├── Modal.svelte        # Backdrop + Escape + clic exterieur
│   │   │   │   ├── InlineEdit.svelte   # Rename inline (Enter/Escape/blur, autofocus)
│   │   │   │   ├── ContextMenu.svelte  # Menu clic droit (items label/action/danger)
│   │   │   │   └── Toast.svelte        # Rendu des notify() (monte dans App.svelte)
│   │   │   ├── layout/
│   │   │   │   ├── Header.svelte       # Barre superieure (logo, cloche notifs, zoom, parametres, theme)
│   │   │   │   ├── Sidebar.svelte      # Terminaux + projets (DnD local : reorder + deplacement inter-dossiers)
│   │   │   │   └── MainPanel.svelte    # Routeur sur activeView ({#key} pour remount au switch projet)
│   │   │   ├── dashboard/
│   │   │   │   ├── Dashboard.svelte    # Menu + routage vers les 4 vues (67 lignes)
│   │   │   │   ├── TasksView.svelte    # Todos par projet (DnD todos local : move inter-projet)
│   │   │   │   ├── MonitoringView.svelte # Donuts CPU/mem, historique, top processus
│   │   │   │   ├── TerminalsView.svelte  # Terminaux par projet, clic = navigation
│   │   │   │   └── ContainersView.svelte # Conteneurs/Volumes/Images + df + prune
│   │   │   ├── project/
│   │   │   │   ├── ProjectDetail.svelte  # Barre unique (titre + onglets + actions ⏺/URLs), map tabs
│   │   │   │   ├── DockerTab.svelte      # Start/stop/restart, dependances, conteneurs
│   │   │   │   ├── WorkspaceTab.svelte   # Notes (gauche, flex:2) + Todos (droite, flex:1)
│   │   │   │   ├── TerminalTab.svelte    # Multi-terminaux tmux, sessions Claude (fixes accents NE PAS RETIRER)
│   │   │   │   ├── FilesTab.svelte       # Arbre lazy gitignore-aware + viewer Shiki
│   │   │   │   ├── GitTab.svelte         # Status + diff viewer (hunks colores)
│   │   │   │   ├── PluginsTab.svelte     # Marketplace agents par projet
│   │   │   │   └── SettingsTab.svelte    # Parametres projet + URLs + override prompt resume
│   │   │   ├── todos/
│   │   │   │   └── TodoList.svelte       # CRUD + checkbox (use:reorderable + InlineEdit)
│   │   │   ├── urls/
│   │   │   │   └── UrlList.svelte        # CRUD liens rapides
│   │   │   ├── notes/
│   │   │   │   ├── NoteTree.svelte       # Arborescence (DnD local : move inter-dossiers)
│   │   │   │   └── NoteEditor.svelte     # Editeur WYSIWYG (contenteditable + toolbar + autosave 1s)
│   │   │   ├── system/
│   │   │   │   ├── SystemMonitor.svelte  # Vue systeme complete (barres + processus)
│   │   │   │   └── ProcessList.svelte    # Top CPU / Top memoire
│   │   │   └── settings/
│   │   │       └── GlobalSettings.svelte # Page a menu lateral 4 vues (cartes) : General / Claude & IA / Reunions / Projets
│   │   └── types/
│   │       └── index.ts            # Tous les types TypeScript partages
│   └── styles/
│       ├── global.css              # Reset CSS
│       ├── theme.css               # Tokens design (couleurs, radius, ombres) dark/light
│       └── components.css          # Classes partagees : .btn, .icon-btn, .card, .input, .badge, .empty, DnD, scrollbars
│
├── index.html                      # Entry HTML (charge main.ts)
├── package.json                    # Dependances npm
├── vite.config.ts                  # Config Vite
├── tsconfig.json                   # Config TypeScript
├── svelte.config.js                # Config Svelte
└── docs/superpowers/
    ├── specs/                      # Spec de design
    └── plans/                      # Plan d'implementation
```

## Fonctionnalites

### Dashboard (page d'accueil)

Menu vertical a gauche, 4 vues — un composant par vue dans `dashboard/` (voir section "Tableau de bord" plus bas) :
- **Taches** : todos en attente groupes par projet avec compteur, clic sur un projet pour y naviguer
- **Terminaux** : tous les terminaux ouverts, raccourci direct vers chaque session
- **Conteneurs** : tous les conteneurs Docker de la machine + volumes/images + prune
- **Monitoring** : monitoring systeme avec :
  - Badge hostname, version kernel, uptime
  - Jauges circulaires SVG (donuts) CPU + Memoire avec pourcentage, nombre de coeurs, modele CPU
  - Detail memoire (Processus, ZFS ARC, Cache, Partage, Buffers) lu depuis `/proc/meminfo` et `/proc/spl/kstat/zfs/arcstats`
  - Graphiques d'historique CPU et memoire (SVG polyline, 60 points FIFO a 3s d'intervalle)
  - Top 20 processus CPU et Top 20 processus memoire (tableau avec toggle)

### Sidebar

- Section **Terminaux** en haut (repliable, masquee si vide) : raccourcis vers toutes les sessions
  tmux vivantes (nom + projet), clic = navigation directe vers la session (pendingTerminalId),
  clic droit = Renommer/Fermer. Point VERT = un agent IA (claude, codex...) tourne dans la session,
  gris = terminal normal. Alimentee par le store `terminals` (stores/terminals.ts) : recharge sur
  terminal_exit, apres creation/fermeture/renommage, et toutes les 5 s (suivi du flag llm).
- Liste de tous les projets avec :
  - Dot de couleur selon l'etat (running/starting/stopping/error/stopped)
  - Nom du projet
  - Description (si presente)
  - Nombre de containers (si > 0)
  - Etat textuel
- Clic pour naviguer vers le projet

### Vue projet (8 onglets)

En-tete : nom renommable (double-clic), description, bouton ⏺ Enregistrer (reunions), liens rapides.

- **Workspace** : Notes a gauche (flex: 2, arborescence + editeur WYSIWYG) + Todos a droite (flex: 1)
- **Docker** : start/stop/restart, dependances ("depend de" / "requis par"), tableau des conteneurs
- **Terminal** : multi-terminaux persistants (voir section dediee plus bas)
- **Fichiers** : arbre lazy gitignore-aware + viewer Shiki + Ctrl+clic "aller a la definition" (LSP)
  + edition avec coloration (✎ / Ctrl+S)
- **Git** : gestion complete (stage/unstage, commit, push, branches) + diff colore
- **Plugins** : marketplace d'agents par projet
- **Parametres** : formulaire projet (chemin, compose, description, dependances), URLs, override du prompt de resume

### Editeur de notes (WYSIWYG)

- Arborescence de dossiers et fichiers Markdown a gauche
- Editeur contenteditable a droite : affiche le Markdown rendu (via `marked`) et permet l'edition directe
- Toolbar : Gras, Italique, Barre, H1/H2/H3, Listes, Citation, Code, Lien
- Conversion HTML -> Markdown via `turndown` a la sauvegarde
- Auto-save avec debounce 1s

### Monitoring systeme

Accessible depuis le dashboard (integre) et comme page separee :
- CPU global + par coeur
- Memoire (total, used, available, swap) + detail (cached, buffers, shmem, s_reclaimable, zfs_arc)
- Disques (mount, device, total, used, free, percent)
- Top 20 processus par CPU et par memoire
- Kill processus via SIGTERM

### Parametres globaux

Page a menu lateral (6 vues, etat local `view` dans GlobalSettings.svelte, sections en cartes) :
- **General** : chemin DB, version, build time, verification de mise a jour, changelog embarque,
  import depuis ancienne DB Go
- **Apparence** : palettes, accent, image de fond (`AppearanceSettings.svelte`)
- **Agents** : marketplace d'agents Claude Code, `AgentsView.svelte` ENCASTREE ici — ce n'est plus
  une vue top-niveau et il n'y a plus de bouton dans le Header. Sa grille est fluide
  (`minmax`) pour tenir dans la colonne des parametres, qui passe a 1500 px sur cette vue
  (`.settings.wide`) ; `.embedded-view` lui donne une hauteur, sinon `height: 100%` s'ecrase.
- **Claude & IA** : connexion abonnement (badge statut + flow setup-token)
- **Reunions** : cle OpenAI, modele et prompt systeme du resume
- **Projets** : liste des projets enregistres (suppression)

L'ancienne commande `restart_app` a ete SUPPRIMEE avec le bouton ↻ : l'updater relance
l'application lui-meme via `@tauri-apps/plugin-process`.

### Centre de notifications

Cloche **toujours visible** dans le Header, badge du nombre de non-lues, clic ->
`notifications/NotificationPanel.svelte`. C'est le point d'entree UNIQUE : ne jamais remettre une
information de ce type derriere les parametres (l'utilisateur ne doit pas aller la chercher).

**Architecture — les notices ne sont JAMAIS persistees.** Elles sont recreees a chaque lancement
par leur producteur ; seul l'etat utilisateur (lu / ecarte) va en localStorage, indexe par l'`id`.
Deux consequences : une notice peut porter une `action` sous forme de callback (impossible si on
serialisait), et **ajouter une source de notifications = appeler `pushNotice()` depuis un nouveau
module**, sans toucher ni au store ni au panneau. `id` stable et prefixe par producteur
(`update:0.3.0`) -> dedoublonnage, et `removeNoticesByPrefix()` pour retirer les siennes.

### Mises a jour automatiques et versionnage

L'updater est le premier producteur de notices : quand `check()` trouve une version, il pose une
notice `update:<version>` avec l'action **Mettre a jour** (telecharge, installe, relance).

**Cadence** : demarrage, puis toutes les heures, plus un controle au retour de focus sur la fenetre
si la derniere verification a plus de 15 min. Ne pas descendre a 10 min : une release sort quelques
fois par jour au plus, c'est le controle au focus qui donne la sensation d'immediatete. Verification
silencieuse — une machine hors ligne ne doit pas polluer l'UI.

Le bouton de verification manuelle existe a deux endroits : dans le panneau et dans
Parametres -> General (qui affiche aussi la version installee et le changelog).

**Version : une seule source de verite = `package.json`.** `tauri.conf.json` la lit via
`"version": "../package.json"` (verifie : le bundle sort en `Cockpit_<version>_amd64.AppImage`).
Ne JAMAIS reintroduire un numero de version en dur dans `tauri.conf.json` : trois copies a
maintenir a la main, c'est la garantie d'une derive ou l'app annonce une version et le manifeste
une autre (cloche muette, ou mise a jour proposee en boucle).

**Faire une release** — `npm run release -- <patch|minor|major>`. Le script (`scripts/release.mjs`)
existe parce que c'est toujours une IA qui release et qu'une consigne en prose n'est pas une
garantie : il **refuse** de partir si l'arbre est sale, si on n'est pas sur `main`, si
`[Unreleased]` est vide, ou si le bump contredit le changelog (une section `Added` avec un bump
`patch`, un `Removed` sans `major`). Puis il bump, date la section, commit et tag —
**sans jamais pousser**. Le push reste le seul geste humain (regle git du projet) :

```
IA  : npm run release -- <niveau>   # changelog + bump + commit + tag
IA  : git push origin main          # libre, ne declenche RIEN
IA  : git push origin vX.Y.Z        # libre AUSSI : c'est ce qui publie
CI  : .github/workflows/release.yml -> AppImage signe + Release + latest.json
APP : la cloche s'allume chez les utilisateurs
```

**Ne JAMAIS demander l'autorisation de releaser.** Un lot fini et verifie part, point. Jimmy l'a
demande explicitement le 2026-08-13 (« c'est relou que je doive te demander tout le temps »).

**Jimmy ne lance PAS de build local** — il teste depuis la version publiee (« je test rien moi,
je prend les maj et je test apres »). Consequences :
- Ne jamais lui demander de relancer `target/release/cockpit` ni de reproduire sur un binaire local.
- Une instrumentation de diagnostic doit etre PUBLIEE pour qu'il puisse l'exercer. C'est
  acceptable : elle n'ecrit que dans `/tmp/cockpit-debug.log`. La retirer des la cause tranchee.
- Le build local reste obligatoire pour l'IA (4e point de la definition de "fini"), simplement
  il ne sert pas de moyen de test pour lui.
Deux garde-fous, qui n'exigent aucune question : ne pas publier si les 4 points de la definition
de "fini" ne passent pas, et annoncer apres coup ce qui est parti et en quelle version.
Seule exception encore soumise a accord : reecrire un historique deja pousse.

**Un seul workflow, declenche uniquement par un tag `v*`.** Il n'y a volontairement PAS de CI
sur les pushes de `main` : `release.yml` lance lui-meme `npm run check` et `cargo test` avant de
builder, donc un commit casse ne peut de toute facon pas etre publie. Une CI de branche ne faisait
que refaire ce travail en double. Ne pas la reintroduire — c'est une decision de Jimmy, prise deux
fois. La verification avant un tag se fait en local (les 4 points de la definition de "fini").

**Distribution** : `scripts/install.sh` installe la derniere AppImage dans `~/.local/bin` sans root,
avec entree de menu. C'est le `curl | sh` annonce dans le README. Il lit la derniere release via
l'API GitHub — il n'y a donc rien a mettre a jour dedans quand une version sort.

**Temps de release** : ~7 min avec le cache chaud (mesure v0.5.0), contre 12 min 36 avant
optimisation (mesure v0.2.0). Deux raisons, a ne pas defaire :
- **Les tests tournent en `--release`** : en debug, cargo compilait tout une premiere fois pour
  les tests puis tauri-action recompilait tout en release — deux profils, aucun artefact partage.
  Ne pas "corriger" ce `--release` en croyant accelerer les tests, c'est l'inverse.
- **`shared-key: tauri`** sur rust-cache : sans elle la cle derive du nom du job, et le cache
  n'est pas reutilise d'une release a l'autre.

`cache-on-failure` est actif pour qu'un echec ne reparte pas d'une compilation complete.

**Pieges** :
- **`Error updating policy` en fin de release** : incident transitoire de l'API GitHub, pas une
  erreur du workflow. Il survient APRES le build et laisse une release incomplete (AppImage
  uploade, mais ni `.sig` ni `latest.json`) — donc `latest.json` en 404 et plus aucune mise a
  jour detectee chez les utilisateurs. Remede : `gh run rerun <id> --failed`. tauri-action
  retrouve la release existante et y ajoute les fichiers manquants. Constate sur la v0.6.2.
  **Toujours verifier apres publication** : `curl -sL -o /dev/null -w "%{http_code}"
  https://github.com/jguevel-tech/cockpit/releases/latest/download/latest.json` doit rendre 200.
  Nuance : un 404 dans les ~2 premieres minutes apres publication est la propagation CDN de
  GitHub, pas l incident — re-tester avant de relancer quoi que ce soit (constate sur la 0.8.0).
- Sous Linux l'updater ne remplace qu'un **AppImage**. Un binaire brut (`--no-bundle`) ne peut
  pas se mettre a jour : pour tester le flow reel, lancer l'AppImage, pas `target/release/cockpit`.
- En local `npx tauri build` (avec bundle) **echoue** faute de cle privee : c'est voulu, la
  signature n'a lieu qu'en CI. Pour un binaire de dev, garder `--no-bundle`.
- Secrets GitHub requis : `TAURI_SIGNING_PRIVATE_KEY` et `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`.
  La cle publique est dans `tauri.conf.json`. Perdre la cle privee = plus aucune mise a jour
  possible pour les utilisateurs deja installes (reinstallation manuelle obligatoire).
- Le `CHANGELOG.md` est embarque au build (`?raw` + `marked`) et affiche dans
  Parametres -> General. Il est donc toujours celui de la version installee.

### Zoom global

Controle `− 100% +` dans le Header (clic sur la valeur = retour a 100 %) + **Ctrl+molette** partout,
terminaux compris (listener capture `passive:false` dans App.svelte, garde 120 ms contre les rafales
de trackpad ; molette nue laissee au copy-mode tmux).

Implemente par le **zoom natif du webview** (`set_webview_zoom` -> `WebviewWindow::set_zoom`), donc
tout est mis a l'echelle : typo, paddings, bordures ET le rendu xterm. Un `html { font-size }` variable
a ete ecarte : ~423 tailles en px (paddings, `--header-height`, boutons 32x32) ne suivraient pas les
809 `rem` et le texte finirait par deborder de ses boites.

Rien a faire cote terminaux : zoomer change les dimensions en px CSS du conteneur, donc le
`ResizeObserver` de TerminalTab (debounce 80 ms) refit et renvoie la nouvelle taille a tmux.

**Paliers derives de la police terminal (NE PAS remettre des paliers ronds)** : `ZOOM_LEVELS`
vaut `TERMINAL_FONT_STEPS.map(px => px / TERMINAL_FONT_SIZE)` — les facteurs visent des tailles
de police ENTIERES (14/13 = 108 %, 15/13 = 115 %...). Des paliers ronds (1.1, 1.25, 1.4) donnent
une police fractionnaire (13 x 1.1 = 14.3 px) que le rasteriseur lisse -> texte visiblement mou.
Diagnostique en comparant 110 % (mou), 150 % et 200 % (nets, 13 x 2 = 26 px pile) : c'est ce qui a
elimine les deux autres hypotheses (agrandissement bitmap de la couche webview, canvas WebGL xterm
non realloue) — les deux auraient rendu 200 % flou, pas net. Changer `TERMINAL_FONT_SIZE` suffit,
les paliers suivent. L'UI (racine 14 px) ne peut pas etre exacte simultanement : 13z et 14z entiers
implique z entier, donc seuls 100 % et 200 % ; l'ecart residuel est de 0,08 a 0,46 px.

### Apparence : palettes, accent, image de fond

`stores/appearance.ts` + `settings/AppearanceSettings.svelte`. **Le theme n'est plus un booleen
sombre/clair** : c'est une palette parmi plusieurs. L'ancien store `theme` de `stores/ui.ts` a
demenage.

**DEUX mecanismes CSS complementaires — ne pas les confondre** :
- la classe `html.dark` porte la **base** (sombre ou claire). C'est elle que lisent le theme xterm
  (`XTERM_THEMES`), Shiki (`FilesTab`) et le selecteur `html:not(.dark) .term-container`. Toute
  palette sombre doit donc aussi porter cette classe.
- l'attribut `html[data-theme]` porte la **palette** et surcharge les tokens.

Consommer `themeBase` (derive, `"dark" | "light"`) partout ou le choix est binaire, jamais `theme` :
sinon chaque nouvelle palette casse un `Record` a deux entrees.

**Ajouter une palette = 3 endroits** : un bloc `html[data-theme="x"]` dans theme.css, sa ligne de
couleurs OPAQUES (`--surface-canvas/base/raised`, indispensable au verre depoli), et une entree
dans `THEMES` (appearance.ts).

**Image de fond** : stockee en FICHIER dans `<app_data>/wallpaper.<ext>` (module `appearance/`),
pas dans la table `settings` — `get_app_settings()` renvoie toutes les cles d'un coup et y glisser
des centaines de Ko de base64 alourdirait chaque lecture. Le frontend redimensionne (canvas,
2560 px max, WebP 0.85) et extrait la couleur dominante ; Rust ne fait que valider et ecrire.
Lecture du fichier source par `read_image_as_data_url` cote Rust, PAS par `@tauri-apps/plugin-fs`
(non installe cote JS, et il faudrait des permissions de lecture bien trop larges).

**Lisibilite** : quand `html.has-wallpaper` est pose, les tokens `--bg-*` deviennent translucides
via `color-mix` et les surfaces recoivent un `backdrop-filter: blur()` (components.css). Le
**TERMINAL reste opaque et sans flou** : xterm dessine dans un canvas WebGL, le rendre translucide
est un terrain a regressions (voir Pieges connus), et un terminal doit rester lisible avant d'etre
joli. Ne pas "harmoniser" ce cas particulier.

Le bouton ◑ du Header (`toggleBase`) bascule sombre <-> clair ; les palettes de couleur se
choisissent dans Parametres -> Apparence. Reglages persistes en localStorage sous la cle
`cockpit-appearance` (migration automatique depuis l'ancienne cle `cockpit-theme`).

## Base de donnees

SQLite stockee dans `~/.local/share/com.cockpit.dev/data.db` (ou via `COCKPIT_DB` env var).

13 tables :

| Table | Contenu |
|-------|---------|
| `projects` | Projets Docker (name, path, compose_file, description, depends_on JSON, position, folder_id, summary_prompt) |
| `project_folders` | Dossiers hierarchiques de projets |
| `notes` | Note simple par projet (une seule par projet) |
| `note_folders` | Dossiers de notes hierarchiques (parent_id nullable, cascade delete) |
| `note_files` | Fichiers de notes dans les dossiers (content Markdown, cascade delete) |
| `todos` | Taches par projet (text, done, position) |
| `urls` | Liens rapides par projet (label, url, position) |
| `settings` | Cle/valeur globales (openai_api_key, summary_prompt, summary_model) |
| `recordings` | Enregistrements de reunions (project, started_at, duration_secs, state, error, dir) |
| `terminals` | Terminaux persistants (project, name, tmux_name) |
| `command_history` | Historique de commandes (command PRIMARY KEY, project, ts — upsert) |
| `claude_session_names` | Noms personnalises des sessions Claude Code (session_id, name) |

La colonne `summary_prompt` (nullable) sur `projects` surcharge le prompt global de resume par projet.

Le champ `depends_on` dans `projects` est un JSON array stocke comme TEXT (ex: `["docker-devbox"]`).

Index : idx_notes_project, idx_note_folders_project, idx_note_files_project, idx_note_files_folder,
idx_todos_project, idx_urls_project, idx_projects_folder,
idx_recordings_project, idx_terminals_project, idx_command_history_ts.

Migrations automatiques au demarrage via `storage/db.rs`. Mode WAL + foreign keys actives.

## Commandes Tauri

### Docker
- `list_projects`, `start_project`, `stop_project`, `restart_project`

### Todos
- `get_todos`, `create_todo`, `update_todo`, `delete_todo`, `reorder_todos`, `move_todo`, `get_pending_todos`

### Notes
- `get_note`, `save_note`, `get_note_tree`
- `create_note_folder`, `rename_note_folder`, `delete_note_folder`
- `create_note_file`, `get_note_file`, `save_note_file`, `rename_note_file`, `delete_note_file`
- `reorder_note_folders`, `reorder_note_files`, `move_note_file`

### URLs
- `get_urls`, `create_url`, `update_url`, `delete_url`


### Project Folders
- `get_project_folders`, `create_project_folder`, `rename_project_folder`, `delete_project_folder`, `reorder_project_folders`, `move_project_to_folder`

### Scanner/Settings
- `scan_dir`, `scan_subdirs`
- `get_db_projects`, `add_project`, `update_db_project`, `delete_db_project`, `reorder_projects`
- `get_project_settings`, `update_project_settings`, `rename_project` — tous auto-reparants :
  si le nom affiche (orchestrateur) a derive du nom stocke, resolution par le CHEMIN du projet
  (resolve_db_project_name dans lib.rs) ; idem get/set_project_summary_prompt

### System
- `get_system_metrics`, `kill_process`, `open_terminal` (legacy : terminal externe gnome-terminal, plus de bouton UI)
- `set_webview_zoom(factor)` : zoom natif du webview (`WebviewWindow::set_zoom`), bornes ZOOM_MIN/MAX

### Enregistrement de reunions
- `start_recording`, `stop_recording`, `get_active_recording`
- `get_failed_recordings`, `retry_recording`, `delete_recording`
- `get_app_settings`, `set_app_setting`, `get_project_summary_prompt`, `set_project_summary_prompt`

### Connexion Claude Code (abonnement)
- `claude_auth_status` (lit ~/.claude/.credentials.json : logged_in, subscription_type, expires_at)
- `start_claude_login` / `claude_login_input` / `cancel_claude_login` (pilote `claude setup-token`
  dans un PTY, events `claude_login_output` / `claude_login_done`), `open_url` (navigateur systeme)

### Terminaux integres
- `create_terminal` (init_command via tmux send-keys), `write_terminal`, `resize_terminal`, `close_terminal`
- `attach_terminal` (tue l'ancien client tmux et en respawn un frais, events des le 1er octet),
  `detach_terminal`, `rename_terminal`
- `list_terminals`, `list_all_terminals` (avec flag `llm` : un agent IA tourne dans la session), `terminal_alt_screen`
- `set_clipboard` / `get_clipboard` (presse-papier systeme via arboard, instance gardee en vie),
  `terminal_copy_selection` (copie la selection copy-mode tmux — clic droit > Copier)
- `list_claude_sessions`, `rename_claude_session`
- `record_command`, `search_command_history` (historique fusionne DB Cockpit + ~/.zsh_history + ~/.bash_history)
- `debug_log` (diagnostic : append dans /tmp/cockpit-debug.log)

### Explorateur de fichiers / Git
- `list_project_dir`, `read_project_file`, `write_project_file` (fichiers existants, racine verrouillee)
- `goto_definition` (LSP si serveur dispo pour le langage, sinon repli `find_symbol`)
- `git_status` (staged/unstaged par fichier, +/- via --numstat, ahead/behind), `git_diff_file`
- `git_stage`, `git_unstage`, `git_stage_all`, `git_unstage_all` (add / reset)
- `git_commit`, `git_push` (set_upstream auto si pas d'upstream)
- `git_branches`, `git_checkout_branch`, `git_create_branch`, `git_delete_branch` (force en fallback)
- run_git_strict pour les operations (code != 0 = erreur remontee) vs run_git (tolere code 1 pour diff)

### Migration
- `import_database`, `get_db_path`

## Events Tauri (backend -> frontend)

- `status_update` : emis toutes les 5s par le monitor apres refresh des statuts Docker
- `recording_status` : emis a chaque changement d'etat du pipeline reunion (recording_id, project, state, error, started_at)
- `terminal_output` : octets du PTY encodes base64 ({id, data}), consommes par xterm.js
- `terminal_exit` : id de la session dont le shell s'est termine
- `claude_login_output` / `claude_login_done` : sortie et fin du flow `claude setup-token`

## Onglets Terminal / Fichiers / Git (vue projet)

- **Terminal** : multi-terminaux par projet, renommables (double-clic sur l'onglet, clic droit dans
  la sidebar), PERSISTANTS : chaque terminal est une session tmux `ckpt_<id>` sur un socket dedie
  (`tmux -L cockpit`, isole du tmux perso). Conf geree par Cockpit (`<app_data>/tmux.conf`, reecrite
  au demarrage + options appliquees au serveur vivant via apply_server_options) : status off, mouse on,
  history 10000, set-clipboard on (OSC 52), mode-style bleu accent, selection qui RESTE au relachement
  (stop-selection), Ctrl+C copie en copy-mode, pas de menus tmux au clic droit. Metadonnees en DB
  (table `terminals`), le serveur tmux survit a la fermeture de l'app : au redemarrage on se rattache
  (purge au boot des sessions disparues, suppression de la ligne quand le shell se termine).
  Les events IPC ne sont emis que si une UI est attachee. Ecritures/resizes serialises cote frontend
  (file par terminal), PTY cree a la taille mesuree. Theme suit dark/light. RENDU : addon WebGL +
  police mono explicite (DejaVu Sans Mono...) — le renderer DOM d'xterm avec "monospace" generique
  derive visuellement sur les glyphes accentues ; le modele terminal etait sain (verifie via
  tmux capture-pane), seul l'affichage etait corrompu.
- **Copier/Coller** : selection souris (tmux copy-mode, surlignage bleu) qui reste affichee au
  relachement -> Ctrl+C copie (sinon SIGINT normal), ou clic droit -> menu Cockpit Copier/Coller.
  Chaine de copie : copy-pipe -> `tmux load-buffer -w -` (PAS set-buffer : il ne lit pas stdin !)
  -> OSC 52 -> handler xterm (parser, chemin de sortie) -> commande set_clipboard (arboard, instance
  gardee en vie sinon le presse-papier X11 meurt avec elle). Shift+glisser = selection xterm locale
  dans les TUI qui capturent la souris (claude, vim).
- **Liens** : addon web-links, Ctrl+clic ouvre l'URL (http/https) dans le navigateur via open_url.
- **Detection agents IA** : point VERT dans la sidebar/dashboard quand un CLI LLM tourne dans la
  session (claude, codex, gemini, aider... — constante LLM_COMMANDS dans terminal/mod.rs, detection
  pane_current_command + arbre de process pour les CLIs node), point gris sinon. Store terminals
  rafraichi toutes les 5 s.
- **Frappe = xterm brut** : `onData` -> `queueWrite` (PTY) directement, AUCUNE surcouche sur le chemin
  de frappe. L'autosuggestion, le Ctrl+R overlay et le bandeau ⚡ ont ete RETIRES le 10/07/2026.
- **BUG ACCENTS (fix racine, NE PAS RETIRER)** : ibus (module de saisie GTK d'Ubuntu) route les touches
  accentuees DIRECTES de l'AZERTY (e accent, c cedille...) par le pipeline de composition IME du WebView,
  en emettant des `compositionend` SANS `compositionstart` — cas mal gere par xterm.js (accumulation du
  textarea, prefixes espace+insecable U+00A0, doublons apres un espace : symptomes multiples d'une meme
  cause). FIX : `GTK_IM_MODULE=gtk-im-context-simple` pose dans `run()` AVANT l'init GTK (lib.rs) ->
  plus aucune composition pour ces touches, frappes normales. Les touches mortes (^+e -> e circonflexe)
  restent gerees par le contexte simple. Deux filets JS conserves dans TerminalTab (vide-textarea sur
  compositionend, strip espace+U+00A0 dans sendInput) — inertes si la composition n'existe plus.
  Diagnostique par instrumentation keydown/compo/input/onData apres de multiples patchs symptomatiques
  (police, WebGL, locale, handler clavier, regex) qui corrigeaient chacun UN symptome.
- **Sessions Claude Code** : bouton "Claude" dans l'onglet Terminal — liste les conversations du projet
  lues depuis `~/.claude/projects/<chemin-encode>/*.jsonl` (label = premier message user, tri par mtime),
  clic = nouveau terminal avec `claude --resume <session-id>` injecte via le PTY. Renommables (crayon ✎
  au survol, table `claude_session_names`, nom vide = retour au label auto), commande
  `rename_claude_session`.

- **Fichiers** : arbre lazy respectant .gitignore (crate `ignore`), viewer code colore via Shiki
  (bundle fin ~30 langages, `src/lib/shiki.ts`), limite 2 Mo, detection binaire, chemins verrouilles a la racine projet.
  - **Aller a la definition** : Ctrl+clic sur un symbole -> module `lsp/` (client JSON-RPC stdio
    minimal, un serveur par projet+langage garde vivant, textDocument/definition uniquement).
    Serveurs reconnus s'ils sont dans le PATH : intelephense (php), rust-analyzer,
    typescript-language-server, svelteserver, pylsp, gopls — ajouter un langage = 1 ligne dans
    `server_for()`. Repli sans serveur : `workspace::find_symbol` (regex de declarations,
    gitignore-aware). Multi-resultats -> ContextMenu ; saut = ouverture + scroll + flash `.line`.
    Le 1er appel paie l'indexation du serveur (jusqu'a ~25 s sur un gros projet) ; serveurs
    stoppes a la fermeture (RunEvent::Exit -> lsp.shutdown_all).
  - **Edition** : bouton ✎ (ou Ctrl+S pour sauver) -> `ui/CodeEditor.svelte` (textarea transparent
    superpose au rendu Shiki, memes metriques de police obligatoires), `write_project_file`
    (fichiers existants uniquement, chemins verrouilles). Fichiers tronques : lecture seule.
- **Git** : gestion complete. Colonne gauche = barre branche (switch/creer/supprimer via menu),
  totaux +/- globaux, bouton Push (avec ahead, set-upstream auto), groupes Indexe / Modifications
  (stage/unstage par fichier + tout), +/- par fichier, zone de commit (Ctrl+Enter). Colonne droite =
  diff colore. Backend : `git status --porcelain -z` + `--numstat` pour les compteurs, diff unified
  parse en FileDiff/DiffHunk/DiffLine (doubles numeros old/new), untracked via `git diff --no-index`.
  Shell-out git, pas de libgit2.

## Tableau de bord

Menu a gauche, 4 vues (store `dashboardView`), un composant par vue dans `dashboard/` :
- **Taches** : todos en attente groupes par projet (drag & drop, edition inline)
- **Monitoring** : jauges CPU/memoire, historique, top processus (Snapshot / Live)
- **Terminaux** : tous les terminaux ouverts groupes par projet, clic = navigation directe
  vers la session (store `pendingTerminalId` lu par TerminalTab au mount)
- **Conteneurs** : TOUS les conteneurs Docker de la machine (`docker ps -a`, pas seulement les
  projets Compose Cockpit), groupes par projet compose (label com.docker.compose.project), avec
  actions start/stop/restart/remove + bulk par groupe, sous-onglets Volumes/Images, bandeau
  `docker system df` + boutons prune. ATTENTION perfs : `system df` mesure chaque volume (10 s+)
  -> charge en ARRIERE-PLAN non bloquant, chaque sous-onglet a son propre etat de chargement ;
  timeouts docker : 15 s listings rapides, 300 s (TIMEOUT_LONG) pour df/prune/actions en masse.
  Commandes `list_all_containers` / `container_action(_bulk)` / `docker_disk_usage` /
  `list_docker_volumes` / `list_docker_images` / `docker_prune` (module `docker/containers.rs`).

## Enregistrement de reunions

Bouton ⏺ dans l'en-tete de la vue projet. Pipeline :
1. Capture 2 pistes via `pw-record` (PipeWire) : micro + monitor du sink par defaut (son systeme). PCM brut s16 mono 16 kHz dans `<app_data>/recordings/rec_<id>/`
2. Transcription OpenAI `whisper-1` par piste (chunks de 10 min < 25 Mo, `verbose_json`, langue fr, filtre silence + no_speech_prob)
3. Fusion chronologique en dialogue "Moi" (micro) / "Eux" (son systeme)
4. Resume via chat completions (modele et prompt systeme configurables dans Parametres globaux, override par projet)
5. Note auto-creee dans le dossier "Réunions" du projet : `Réunion du JJ/MM/AAAA à HHhMM`

Audio supprime apres succes, conserve en cas d'echec (bouton retry dans l'en-tete projet).
Un seul enregistrement a la fois. Cle API importee au premier lancement depuis `<app_data>/secrets.json` si presente.

## Navigation frontend

Pas de routeur. Un seul enum dans le store `ui.ts` :
- `activeView: "dashboard" | "project" | "settings" | "system"` — MainPanel switch dessus
- `selectProject(name)` pose `activeView = "project"` (+ reset onglet), `openView(v)` pour le reste
- Ajouter une vue top-niveau = etendre le type + un case dans MainPanel (rien d'autre)
- Onglets projet : map `tabs` dans ProjectDetail.svelte (id, label, component) — ajouter un onglet
  = 1 entree dans la map + le type `activeTab` dans ui.ts

Le `{#key $selectedProject}` dans MainPanel force le remount de ProjectDetail quand on change de projet.

## Stores reactifs

| Store | Fichier | Contenu |
|-------|---------|---------|
| `projects` | `stores/projects.ts` | Liste projets, reload sur event `status_update` |
| `systemMetrics` | `stores/system.ts` | Metriques systeme courantes |
| `cpuHistory` | `stores/system.ts` | Historique CPU (60 points FIFO) |
| `memoryHistory` | `stores/system.ts` | Historique memoire (60 points FIFO) |
| `activeView` | `stores/ui.ts` | Vue top-niveau (dashboard/project/settings/system/agents) |
| `selectedProject` | `stores/ui.ts` | Projet selectionne (utilise quand activeView === "project") |
| `activeTab` | `stores/ui.ts` | Onglet actif (workspace/docker/terminal/files/git/plugins/settings) |
| `dashboardView` | `stores/ui.ts` | Sous-vue du tableau de bord (tasks/monitoring/terminals/containers) |
| `pendingTerminalId` | `stores/ui.ts` | Session terminal a activer a l'arrivee sur l'onglet Terminal |
| `theme` | `stores/appearance.ts` | Palette active (identifiant), persiste localStorage |
| `themeBase` | `stores/appearance.ts` | Base derivee "dark" ou "light" — a consommer pour xterm et Shiki |
| `wallpaper` | `stores/appearance.ts` | Data URL de l image de fond, ou null (fichier cote Rust) |
| `notices` | `stores/notifications.ts` | Notifications visibles, non-lues comptees par `unreadCount` |
| `zoom` | `stores/ui.ts` | Zoom global (paliers ZOOM_LEVELS 0.7->2), persiste localStorage `cockpit-zoom` |
| `toasts` | `stores/toast.ts` | Notifications non bloquantes — emettre via `notify(msg, kind?)` |
| `recordingStatus` | `stores/recording.ts` | Pipeline reunion en cours (null sinon) |
| `lastRecordingEvent` | `stores/recording.ts` | Dernier event recording_status (y compris done/error) |

## Orchestration Docker

L'orchestrateur (`docker/orchestrator.rs`) gere :
- **Etats** : stopped, starting, running, stopping, error
- **Dependances** : tri topologique pour le demarrage ordonne, detection de cycles au demarrage
- **Cleanup** : arret recursif des dependances orphelines quand on stoppe un projet
- **Concurrence** : `Arc<RwLock<>>` avec locks limites, les commandes docker (up/down) s'executent hors du lock

Le monitor (`docker/monitor.rs`) rafraichit les statuts en 3 phases :
1. Read lock : collecte des projets a verifier
2. Sans lock : execute `docker compose ps` pour chaque projet
3. Write lock : applique les resultats

## Metriques systeme detaillees

Le backend (`system/metrics.rs`) collecte :
- **CPU** : usage global, par coeur, modele, nombre de coeurs (via sysinfo)
- **Memoire** : total, used, available, swap + detail via `/proc/meminfo` :
  - `cached` : pages cache disque
  - `buffers` : buffers kernel
  - `shmem` : memoire partagee
  - `s_reclaimable` : memoire reclamable (slab)
  - `zfs_arc` : cache ZFS (lu depuis `/proc/spl/kstat/zfs/arcstats`, 0 si absent)
- **Disques** : partitions filtrees (/, /home, /boot, /var, /tmp, /opt)
- **Processus** : top 20 CPU + top 20 memoire groupes par nom

## Vision future

- Connecteurs externes
- Le trait `Plugin` dans `plugin/mod.rs` prepare cette extensibilite

## Pieges connus (lecons apprises)

- **Build** : jamais `cargo build --release` seul pour le binaire final (mode dev -> cherche Vite
  sur localhost:5173). Toujours `npx tauri build --no-bundle`. Un rebuild du frontend seul ne
  ré-embarque pas toujours les assets : c'est la recompilation de la crate qui les fige.
- **Ordre des invoke Tauri** : des `invoke` rapproches peuvent s'executer dans le desordre ->
  toute ecriture PTY passe par une file par terminal cote frontend (ioQueues).
- **Ctrl+lettre sous WebKitGTK** : emet aussi un keypress ; n'intercepter que le keydown laisse
  xterm envoyer le caractere de controle au shell. Bloquer tous les types d'events + listener en
  phase capture sur le conteneur.
- **tmux et ecran alternatif** : le client tmux met TOUJOURS le terminal hote en ecran alternatif ->
  `term.buffer.active.type` est inutilisable pour detecter une TUI ; demander a tmux
  (`#{alternate_on}`).
- **Reponses du terminal dans onData** : focus in/out, DA, CPR, reponses DCS/OSC arrivent par le
  meme canal que les frappes -> a filtrer (regex TERMINAL_REPLY) sinon toute heuristique de suivi
  de frappe se fait polluer.
- **POOL PERSISTANT : ni detach ni re-attach au switch (doctrine du 2026-08-13, remplace
  « client frais obligatoire »).** tmux SYNTHETISE des evenements focus vers l'application du
  pane a CHAQUE attache/detache de client — meme avec `focus-events off`, qui ne gouverne que
  le focus du terminal exterieur. Prouve en isolation : un cycle attache/tue/rattache sans
  AUCUNE entree fait reagir claude (re-render), et ce re-render laissait un saut de ligne a
  chaque changement de terminal. Trois correctifs (0.6.5, 0.6.7) ont vise d'autres maillons
  sans eteindre le symptome. Donc : les xterm vivent dans un POOL au niveau module
  (TerminalTab, `<script module>`), gares dans un div invisible au demontage et re-adoptes au
  retour ; les clients tmux restent attaches en permanence ; attach_terminal REUTILISE un
  client vivant (no-op) et ne respawn que s'il est mort. Les listeners terminal_output/exit
  sont GLOBAUX pour alimenter les xterm meme demontes. Benefice : switch instantane.
- **Un xterm NEUF, lui, exige toujours un client frais** (sequence d'init complete : ecran
  alternatif, modes souris, redraw) — c'est le cas au premier attach d'une session, et c'est
  pourquoi le pool conserve le xterm d'origine : il a deja recu l'init de son client. Le
  retour "replay" d'attach_terminal reste ignore (course replay/live -> ecran dechire).
  init_command passe par `tmux send-keys` vers la SESSION. Historique molette = copy-mode
  tmux (history-limit 10000).
- **Rendu xterm** : le renderer DOM + `monospace` generique derive visuellement sur les glyphes
  accentues. Le modele est sain (verifiable par `tmux -L cockpit capture-pane -p`) : addon WebGL
  + police explicite.
- **Saisie accents (dead-key) sous WebKitGTK** : le textarea cache d'xterm ne se vide PAS apres
  une composition -> il accumule "è","èè","èèè"... et xterm renvoie tout le buffer a chaque frappe
  (caracteres/espaces en trop). Fix REEL : vider le textarea sur `compositionend` (setTimeout 0).
  Diagnostique par instrumentation keydown/composition/input/onData (pas par supposition) apres
  plusieurs faux diagnostics (police, WebGL, locale, strip regex — tous inefficaces).
- **`claude -p` en interactif** : ~5-10 s de latence via l'abonnement, et sans
  `CLAUDE_CONFIG_DIR=<app_data>/claude-fast` (credentials symlinkees) la CLI charge le CLAUDE.md
  global + tous les MCP (~20 s). Une suggestion IA de commande (bouton 💡) a ete implementee puis
  SUPPRIMEE le 09/07/2026 (latence/qualite) — reserver `claude -p` aux taches de fond, preferer
  l'extraction locale (bandeau ⚡) pour l'interactif.
- **Profil release** : `lto = "thin"` + `codegen-units = 16` (le fat LTO doublait+ le temps de build
  pour ~2-5 % de perf).

## Conventions

- **Nouvelle table liee a un projet** : l'ajouter a `PROJECT_SCOPED_TABLES` (storage/projects.rs) —
  la constante alimente delete_project (cascade) ET rename_project. L'oublier = donnees orphelines.
- **Avant de coder un modal / rename inline / menu contextuel / DnD de liste / toast** : utiliser
  `components/ui/` (Modal, InlineEdit, ContextMenu), `actions/reorderable.ts` + `utils/reorder.ts`,
  `stores/toast.ts` (notify). Exceptions connues gardees en local : DnD Sidebar/NoteTree/TasksView-todos
  (deplacement inter-groupe, hors modele de l'action).
- **Erreurs UI** : jamais de `catch {}` muet — `catch (e) { notify(String(e)); }`. `confirm()` natif OK
  pour les actions destructives.
- **Styles** : tokens de theme.css uniquement (jamais de couleur en dur) ; classes partagees dans
  components.css (.btn, .icon-btn, .card, .input, .badge, .empty, etats DnD).
- Backend Rust : modules separes par responsabilite, erreurs retournees comme `Result<T, String>`
- Frontend Svelte 5 : runes mode (`$state`, `$derived`, `$props`, `$effect`), stores avec `writable`
- Pas de framework CSS, variables CSS pures pour le theming
- Les commandes Tauri sync utilisent `fn`, les async (docker) utilisent `async fn`
- Editeur notes : contenteditable + marked (render) + turndown (HTML -> Markdown)
- Navigation inter-projet : `{#key}` pour forcer le remount des composants
- Auto-save notes : debounce 1s via setTimeout
