<h1 align="center">Cockpit</h1>

<p align="center">
  <strong>Le poste de pilotage de vos projets Docker Compose.</strong><br>
  Orchestration, terminaux persistants, notes, Git et monitoring — dans une seule application native.
</p>

<p align="center">
  <a href="https://github.com/jguevel-tech/cockpit/releases/latest"><img alt="Derniere version" src="https://img.shields.io/github/v/release/jguevel-tech/cockpit?style=flat-square&color=2f81f7"></a>
  <a href="https://github.com/jguevel-tech/cockpit/actions/workflows/release.yml"><img alt="Build" src="https://img.shields.io/github/actions/workflow/status/jguevel-tech/cockpit/release.yml?style=flat-square"></a>
  <img alt="Plateforme" src="https://img.shields.io/badge/plateforme-Linux%20x86__64-informational?style=flat-square">
  <a href="LICENSE"><img alt="Licence" src="https://img.shields.io/badge/licence-MIT-green?style=flat-square"></a>
</p>

---

## Installation

```sh
curl -fsSL https://raw.githubusercontent.com/jguevel-tech/cockpit/main/scripts/install.sh | sh
```

C'est tout. Le script installe la derniere version dans `~/.local/bin`, ajoute une entree au menu
des applications, et n'a besoin d'aucun privilege root.

Ensuite, lancez :

```sh
cockpit
```

> **Les mises a jour se font depuis l'application.** Quand une nouvelle version parait, une cloche
> apparait dans l'en-tete : un clic affiche les nouveautes et installe la mise a jour. Vous n'aurez
> plus a relancer ce script.

<details>
<summary><strong>Autres methodes d'installation</strong></summary>

<br>

**AppImage manuelle** — telechargez la derniere depuis la [page des releases](https://github.com/jguevel-tech/cockpit/releases/latest) :

```sh
chmod +x Cockpit_*_amd64.AppImage
./Cockpit_*_amd64.AppImage
```

**Depuis les sources** — voir [Developpement](#developpement).

</details>

### Prerequis

Cockpit fonctionne sans rien d'autre, mais certaines fonctions s'appuient sur des outils du systeme :

| Outil | Necessaire pour |
|---|---|
| `docker` + `docker compose` | l'orchestration des projets |
| `git` | l'onglet Git |
| `tmux` (>= 3) | la persistance des terminaux |
| `pw-record` (PipeWire) | l'enregistrement de reunions |

```sh
sudo apt install docker.io docker-compose-plugin git tmux pipewire-audio-client-libraries
```

---

## Fonctionnalites

### Orchestration Docker

Demarrez et arretez vos projets Compose dans le bon ordre. Cockpit resout les dependances par tri
topologique, detecte les cycles avant de lancer quoi que ce soit, et arrete recursivement les
dependances devenues orphelines. Une vue globale liste tous les conteneurs, volumes et images de la
machine, avec les actions de nettoyage.

### Terminaux persistants

Chaque terminal est une session `tmux` sur un socket dedie : fermez l'application, vos processus
continuent de tourner et vous les retrouvez au redemarrage. Un point vert signale les sessions ou un
agent IA travaille. Les conversations Claude Code du projet sont listees et reprenables en un clic.

### Espace de travail par projet

Notes Markdown arborescentes avec editeur WYSIWYG et sauvegarde automatique, todos reordonnables au
glisser-deposer, liens rapides. Tout est rattache au projet et suit ses renommages.

### Fichiers et Git

Un explorateur qui respecte votre `.gitignore`, une coloration syntaxique sur une trentaine de
langages, l'edition en place, et un « aller a la definition » branche sur les serveurs LSP presents
sur votre machine (`rust-analyzer`, `intelephense`, `typescript-language-server`...).

L'onglet Git couvre le quotidien : status, diff colore, stage par fichier, commit, push, gestion des
branches.

### Enregistrement de reunions

Capture simultanee du micro et du son systeme, transcription, puis resume automatique depose en note
dans le projet. Le prompt de resume est configurable globalement et surchargeable par projet.

### Monitoring

CPU global et par coeur, memoire detaillee (cache, buffers, ARC ZFS), disques, et les vingt processus
les plus gourmands — avec l'historique sur une minute.

---

## Utilisation

### Ajouter un projet

Depuis la barre laterale, `+` puis indiquez le chemin du dossier contenant votre `docker-compose.yml`.
Cockpit sait aussi scanner un repertoire parent pour detecter tous les projets Compose d'un coup.

### Declarer des dependances

Dans l'onglet **Parametres** d'un projet, listez les projets dont il depend. Au demarrage, Cockpit
lancera automatiquement la chaine complete dans l'ordre, et vous previendra si vous avez cree un
cycle.

### Zoom

`Ctrl` + molette n'importe ou dans l'application, ou les boutons `− +` de l'en-tete. Le reglage est
conserve entre les sessions.

### Raccourcis utiles

| Action | Raccourci |
|---|---|
| Zoomer / dezoomer | `Ctrl` + molette |
| Sauvegarder un fichier ouvert | `Ctrl` + `S` |
| Valider un commit | `Ctrl` + `Entree` |
| Copier depuis un terminal | selection souris, puis `Ctrl` + `C` |
| Ouvrir un lien du terminal | `Ctrl` + clic |

---

## Developpement

```sh
git clone https://github.com/jguevel-tech/cockpit.git
cd cockpit
npm install
npx tauri dev
```

### Dependances de compilation

```sh
sudo apt install libgtk-3-dev libwebkit2gtk-4.1-dev librsvg2-dev patchelf
```

### Commandes

| Commande | Effet |
|---|---|
| `npx tauri dev` | developpement avec rechargement a chaud |
| `npm run check` | verification des types frontend |
| `cargo test --manifest-path src-tauri/Cargo.toml` | tests Rust |
| `npx tauri build --no-bundle` | binaire de developpement |

> Pour produire un binaire, utilisez toujours `npx tauri build`, jamais `cargo build --release`
> seul : sans les variables d'environnement de la CLI Tauri, le binaire sort en mode developpement
> et cherche un serveur Vite sur `localhost:5173`.

### Architecture

```
src/                  Frontend Svelte 5 (runes) + TypeScript
  lib/api/            Wrappers types autour des commandes Tauri
  lib/components/     Composants, groupes par domaine
  lib/stores/         Etat reactif partage
  styles/             Tokens de theme et classes partagees

src-tauri/src/        Backend Rust
  docker/             Orchestrateur, graphe de dependances, conteneurs
  storage/            SQLite : projets, notes, todos, parametres
  terminal/           Sessions tmux et historique de commandes
  workspace/          Explorateur de fichiers, sessions Claude
  gitdiff/  lsp/  recorder/  sitemap/  system/  agents/
```

La communication frontend/backend passe exclusivement par l'IPC de Tauri : `invoke` pour les appels,
des evenements pour les mises a jour temps reel. Ni serveur HTTP, ni WebSocket.

Les conventions du projet — regles non negociables, pieges connus, processus de release — sont
detaillees dans [CLAUDE.md](CLAUDE.md).

---

## Contribuer

Les issues et les pull requests sont bienvenues.

Une contribution est prete quand ces trois commandes passent :

```sh
npm run check                                   # 0 erreur, 0 warning
cargo test --manifest-path src-tauri/Cargo.toml # tous verts
npx tauri build --no-bundle                     # compile
```

Et quand la modification est consignee dans [CHANGELOG.md](CHANGELOG.md), sous `## [Unreleased]`,
si elle est visible par l'utilisateur.

## Licence

[MIT](LICENSE)
