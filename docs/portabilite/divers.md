# Chemins, commandes externes, Docker, Git, presse-papier, rendu, raccourcis

Etude de portabilite Linux -> macOS + Windows. Perimetre : tout `src-tauri/src/` et tout `src/`
SAUF les terminaux, l'audio des reunions, les metriques systeme et la chaine de livraison.
Lecture seule, rien modifie.

Classes :

- **A** — marche tel quel, rien a faire
- **B** — remplacement mecanique : un autre appel, meme comportement observable
- **C** — le comportement CHANGE, il y a une decision de conception a prendre
- **D** — perte de fonctionnalite, ou repense necessaire

---

## 1. Chemins et systeme de fichiers

### `HOME` en dur, sans equivalent Windows — **B**

`workspace/claude_sessions.rs:32`, `claude_auth/mod.rs:51` et `:93`, `agents/mod.rs:99` et `:104`,
`terminal/history.rs:99`.

Windows n'a pas `HOME` mais `USERPROFILE`. `agents/mod.rs` retombe meme sur `"/root"` en dur.
Un `home_dir()` unique (`app.path().home_dir()` ou la crate `dirs`) rend exactement le meme
comportement partout.

Ce qui est mauvais, c'est le **mode de panne** : chaque fonction rend `Ok(vec![])` ou
`logged_in: false`. Sessions Claude vides, marketplace d'agents introuvable, historique shell
vide — aucun message, aucune trace dans le journal.

### Le hook de panic reconstruit un chemin Linux — **B**

`lib.rs:1358-1364` rejoue `XDG_DATA_HOME` -> `~/.local/share/com.cockpit.dev`, alors que tout
le reste de l'app ecrit dans `app_data_dir()` (`~/Library/Application Support/...` sur mac,
`%APPDATA%\...` sur Windows).

Sur mac, les panics iraient dans un dossier fantome que personne ne relit ; sur Windows, nulle
part. Le commentaire sur place justifie de ne pas utiliser le handle Tauri a l'instant d'un
panic — c'est vrai, mais il suffit de memoriser le chemin dans un `OnceLock` au demarrage.
Meme comportement, autre source.

### `debug_log` ecrit dans `/tmp` — **B**

`lib.rs:922`. Remplacer par `std::env::temp_dir()`.

### `whoami_fallback` — **B**

`lib.rs:905` lit `USER` puis `LOGNAME`. Ajouter `USERNAME` (Windows).

### `PermissionsExt` non conditionne — **B**, mais bloque la compilation

`terminal/mod.rs:231` : `use std::os::unix::fs::PermissionsExt;` dans `copy_executable`, sans
`#[cfg(unix)]`.

Un `cfg` ne change rien au comportement Linux, donc c'est bien du B. Mais c'est le seul
bloqueur de compilation Windows de tout mon perimetre, et il empeche meme de *decouvrir* les
autres : tant qu'il est la, aucun `cargo check --target x86_64-pc-windows-msvc` ne va au bout.

### `secure_join` est solide ; la forme du chemin Windows est une decision — **C**

`workspace/mod.rs:48-58` : `canonicalize()` des deux cotes puis `starts_with`. Correct sur les
trois systemes, y compris avec les symlinks macOS (`/tmp` -> `/private/tmp`), puisque racine et
cible passent par le meme appel.

Le garde-fou de `stat_project_file:142` — refus des `ParentDir`/`RootDir`/`Prefix` AVANT
canonicalisation — est deja ecrit en pensant a macOS (son commentaire cite un echec constate en
CI sur macOS). Quelqu'un a deja fait tourner les tests dessus.

Ce qui demande un choix : sur Windows, `canonicalize()` rend `\\?\C:\proj`. Il faut trancher
quelle forme est la reference, car la meme valeur sert a trois usages differents :
- ce qui est stocke dans `projects.path` ;
- ce qui s'affiche dans les messages d'erreur (`\\?\C:\...` est illisible) ;
- ce qui est passe en `current_dir` a `docker` et `git` (docker n'aime pas les chemins
  verbatim).

### Le separateur dans les `rel_path` — **B** si le contrat est pose une fois, **C** si chaque appelant s'arrange

Rust produit les `rel_path` par `strip_prefix(&root).to_string_lossy()`. Trois sites :
`workspace/mod.rs:76` (`list_dir`), `:233` (`find_symbol`), `:407` (`search_project`). Sur
Windows, ce sont des `\`.

Le front les decoupe partout avec `/` :
- `FilesTab.svelte:105` — `path.split("/").pop()` pour deviner le langage ;
- `FilesTab.svelte:453` — `parentOf`, `rel.lastIndexOf("/")` ;
- `FilesTab.svelte:461` — `findNode`, `rel.startsWith(n.rel_path + "/")` ;
- `FilesTab.svelte:511` — concatenation apres renommage ;
- `FilesTab.svelte:603` — `revealDir`, depliage segment par segment.

La lecture continue de marcher : `Path::join` accepte `/` sous Windows. Donc rien a changer cote
front — il suffit de normaliser aux trois sites de production.

Le point de conception : ecrire noir sur blanc que **`rel_path` est toujours en `/`**, comme
contrat de l'API Tauri. Sans cette phrase quelque part, le prochain `strip_prefix` ajoute dans
six mois reintroduit le bug.

### `CreateProjectModal.svelte:16` — **B**

`selected.split("/").filter(Boolean).pop()` sur le retour du selecteur de dossier : sur Windows,
le nom du projet devient le chemin entier.

### Renommage de casse impossible sur mac et Windows — **C**

`rename_project_entry` (`workspace/mod.rs:326`) refuse si `target.exists()`. Sur un FS
insensible a la casse, renommer `Foo.ts` en `foo.ts` declenche « existe deja ».

Autoriser ce cas est un changement de comportement : il faut decider comment on distingue « la
cible est un autre fichier » de « la cible est le meme fichier ecrit autrement » (comparaison de
chemin canonique, ou passage par un nom temporaire).

### Normalisation Unicode macOS — **C**

Le Finder ecrit les noms accentues en NFD ; une requete tapee dans l'app arrive en NFC.
`search_project:390` fait `contains(&q)` sur des chaines brutes, `find_symbol` aussi.

Decider de normaliser change le comportement de la recherche **sur les trois plateformes** (deux
fichiers aujourd'hui distincts pourraient matcher la meme requete) : c'est une decision, pas un
remplacement. Invisible en test ASCII, systematique sur un projet francais.

### Noms interdits Windows — **B** si conditionne, **C** si global

`validate_leaf_name` (`workspace/mod.rs:288`) refuse `.`, `..`, `/`, `\`, `\0`. Laisse passer
`CON`, `NUL`, `AUX`, `COM1`, `:`, `*`, `?`, `"`, `<`, `>`, `|`, et les noms a point ou espace
final.

Sous `#[cfg(windows)]`, rien ne change sur Linux : B. Applique partout, on refuse des noms
aujourd'hui valides sous Linux : C. Note : creer `nul` sur Windows ne cree pas de fichier mais
ecrit dans le peripherique nul.

### Base SQLite — **A**

`Connection::open` + WAL + foreign keys (`storage/db.rs:10-13`), chemin issu de
`app_data_dir()`, `backup_database` par l'API backup SQLite : corrects sur les trois. Rusqlite
`bundled` compile son propre SQLite, donc pas de dependance systeme. La seule limite (WAL
inutilisable sur un partage reseau) existe deja sous Linux.

### Corbeille systeme (`trash = "5"`) — **C**

La crate couvre bien les trois OS. Mais sur Windows elle passe par `IFileOperation` (COM) et son
initialiseur **panique** au lieu de rendre une erreur :

```
trash-5.2.6/src/windows.rs:299
panic!("Call to CoInitializeEx failed. HRESULT: {:?}. Consider using `trash` with the feature `coinit_multithreaded`", hr);
```

Ca arrive quand le thread appelant est deja initialise en MTA (`RPC_E_CHANGED_MODE`).
`trash_project_entry` est une commande Tauri synchrone : on ne choisit pas son thread.

Il faut decider ou la mise a la corbeille s'execute (thread dedie dont on maitrise l'apartment
COM). Ne PAS basculer sur `coinit_multithreaded` pour faire taire le panic : `IFileOperation`
exige STA.

### Presse-papier (`arboard`) — **A**

`Cargo.toml` : `arboard = { version = "3", default-features = false, features =
["wayland-data-control"] }`. Les backends mac et Windows ne sont pas derriere un feature, donc
ca compile. Verifie dans la source :
- `arboard-3.6.1/src/platform/osx.rs:98-99` declare `unsafe impl Send`/`Sync` a la main ;
- la version Windows est un `Clipboard(())`, donc auto-`Send`.

Le `static Mutex<Option<Clipboard>>` de `lib.rs:761` compile donc sur les trois.

La contrainte « garder l'instance en vie » est **propre a X11** : sur mac et Windows le contenu
est copie dans le presse-papier de l'OS et survit a la mort du processus. Garder l'instance ne
nuit pas. Seul le commentaire de `lib.rs:759` est a nuancer, pas la mecanique a changer.

---

## 2. Commandes externes

Tout passe par `Command::new(prog).args([...])`, jamais par `sh -c`. **Aucune injection shell a
corriger** — la regle du projet a tenu.

### Le PATH d'une app GUI macOS — **D**

Une `.app` lancee depuis le Finder ou le Dock herite du PATH de `launchd` : a peu pres
`/usr/bin:/bin:/usr/sbin:/sbin`. Ni `/usr/local/bin`, ni `/opt/homebrew/bin`, ni
`~/.local/bin`, ni les shims `nvm`/`mise`.

Deviennent introuvables : `docker` (Docker Desktop l'installe dans `/usr/local/bin`), `claude`,
`intelephense`, `rust-analyzer`, `typescript-language-server`, `svelteserver`, `pylsp`, `gopls`,
`pactl`. `git` survit (`/usr/bin/git`).

Ce n'est pas un chemin a corriger : il faut decider **comment Cockpit resout un binaire** (lire
le PATH du shell de connexion, chercher dans des prefixes connus, laisser l'utilisateur pointer
le binaire dans les reglages) et l'appliquer a tous les appelants. La moitie des fonctionnalites
de l'app depend de cette decision.

Sur Windows le probleme n'existe pas : le PATH vient du registre, une app GUI l'a.

### Windows : `.exe` contre `.cmd` — **C**

`CreateProcess` ne lance que des executables. `docker.exe`, `git.exe` : d'accord. Mais `claude`,
`intelephense`, `typescript-language-server`, `svelteserver` sont des **shims npm `.cmd`** : ils
ne se lancent pas.

`binary_exists` (`lsp/mod.rs:41-50`) ne les voit meme pas — il fait `dir.join(bin).is_file()`
sans consulter `PATHEXT`.

Decision a prendre : lancer un `.cmd` implique `cmd /C`, donc l'echappement des arguments
repasse par l'interpreteur de commandes — exactement la surface de la CVE-2024-24576. On
abandonne la garantie « tableau d'arguments, jamais de shell » qui tient partout ailleurs dans
le crate. C'est un choix, pas un remplacement.

### `gnome-terminal` / `x-terminal-emulator` — **D, perte assumee**

`lib.rs:585-611`, commande `open_terminal`, marquee « legacy, plus de bouton UI » dans le
CLAUDE.md. Elle rend deja une erreur propre ailleurs. A supprimer plutot qu'a porter.

### Fiche machine muette hors Linux — **D**

`report/mod.rs:186-199` lit `/etc/os-release` et lance `pactl info`, `pw-record --version`,
`tmux -V`. Sur mac/Windows : pas de crash (`unwrap_or_default`), mais `distro` vide et
`audio_server: "aucun"`.

Le CLAUDE.md dit que c'est cette fiche « qui a manque pendant plusieurs corrections ». Sur les
deux nouvelles plateformes, elle n'apporterait rien : il faut lui trouver un equivalent, sinon
on porte a l'aveugle exactement la ou on connait le moins le terrain.

### Fenetres console qui clignotent sur Windows — **B**

Aucun `creation_flags(CREATE_NO_WINDOW)` nulle part : zero occurrence de `CommandExt` dans le
crate. Chaque `Command` lancee depuis une app GUI Windows ouvre une console noire le temps de
son execution.

Le facteur d'amplification est le monitor Docker : **toutes les 5 secondes**, un `compose ps`
par projet, en permanence. Ajoute `git status`, `docker ps -a`, la verification d'URL.

Le drapeau est un remplacement strictement mecanique (meme comportement observable, moins une
fenetre). Le risque, c'est de ne pas y penser — voir la section « sous-estimes ».

### Statut de connexion Claude sur macOS — **D**

`claude_auth/mod.rs:51-56` lit `~/.claude/.credentials.json`. Sur macOS, Claude Code stocke ses
jetons dans le **Keychain** : le fichier n'existe pas.

On affichera « deconnecte » a un utilisateur connecte, et on lui proposera un flow
`claude setup-token` qui ne changera rien a l'affichage. Boucle sans issue. Il faut une autre
source de verite, pas un chemin corrige.

### URI `file://` du LSP — **C**

`lsp/mod.rs:132` (`initialize`), `:254` (`textDocument/definition`), `:283` (`parse_locations`) :
`format!("file://{}", root)`.

Rien n'est encode — un chemin avec espace ou `#` est **deja casse sur Linux aujourd'hui**.

Sur Windows on produit `file://C:\proj`, le serveur repond `file:///c%3A/proj/src/a.rs`, et
`parse_locations:303` fait `uri.strip_prefix(&root_uri)?` -> `None` -> zero resultat, avale par
le `filter_map`. Le « aller a la definition » ne dira jamais qu'il ne comprend pas la reponse.

C'est un C et non un B parce que corriger fait **changer le comportement Linux** : des projets
dont le chemin contient un espace se mettront a fonctionner. Le test
`parse_location_array_and_links` verrouille l'ancienne representation, il faudra le reecrire.

---

## 3. Docker

### Le socket — **A**

Aucune reference a `/var/run/docker.sock`, `DOCKER_HOST` ou un named pipe dans tout le crate
(verifie par grep). On passe uniquement par le CLI `docker`, qui gere lui-meme le transport
(socket Unix sur Linux/mac, `npipe:////./pipe/dockerDesktopLinuxEngine` sur Windows). Docker
Desktop configure ca a l'installation. Rien a porter.

### `compose ps --format json`, `ps -a --format {{json .}}`, les labels — **A**

Meme binaire Go sur les trois plateformes, format identique. Le double parsing tableau/NDJSON de
`compose.rs:151-168` et le repli sur le champ `Labels` brut de `containers.rs:85` couvrent deja
la seule variabilite reelle, celle des versions de docker.

### Le filtre par `working_dir` — **C**

`docker/compose.rs:172-178` :

```
label=com.docker.compose.project.working_dir={project_dir.display()}
```

C'est une **egalite de chaine** entre un chemin de notre base et un chemin qu'un autre programme
a ecrit dans un label. Ca tient sur Linux. Ailleurs, chaque ecart rend `Ok(vec![])`, donc
« projet arrete » alors qu'il tourne :
- Windows : `C:\Users\x\proj` contre `C:/Users/x/proj`, et la casse de la lettre de lecteur ;
- macOS : un chemin resolu par compose la ou le notre passe par un lien (`/tmp`, un dossier
  synchronise) ;
- les deux : la casse du chemin, puisque le FS est insensible mais la comparaison non.

Comparer des chemins canonises et insensibles a la casse change le comportement **sur Linux
aussi** (deux projets aujourd'hui distincts deviendraient le meme) : c'est une decision.

Et ce repli sert precisement les projets sans fichier compose standard, donc c'est le cas deja
degrade qui se degrade encore.

### Nom de projet compose contre `name TEXT UNIQUE` — **C**

Compose met le nom de projet en minuscules et remplace les caracteres non alphanumeriques.
SQLite compare `UNIQUE` avec la casse : « MonProjet » et « monprojet » sont deux projets Cockpit
qui pointent le meme projet Docker.

Deja vrai sous Linux, mais un FS insensible a la casse rend la collision facile a creer par
accident (deux projets Cockpit sur le meme dossier, ecrit differemment). Passer la colonne en
`COLLATE NOCASE` est une migration de donnees, pas un remplacement d'appel.

Meme sujet cote `resolve_db_project_name` (`lib.rs:451-469`) : il retrouve un projet par son
CHEMIN via `get_project_name_by_path(&p)`, en comparaison de chaine exacte. Sur mac/Windows,
deux orthographes du meme chemin cassent la reparation automatique.

### Chemins dans le fichier compose et montages de volumes — hors code

Un `- /home/x/data:/data` ecrit sur Linux ne se monte pas sur mac/Windows, et sur Windows les
montages passent par la traduction Docker Desktop. C'est le fichier de l'utilisateur : on n'a
rien a porter, seulement a ne pas laisser croire qu'un projet devient portable parce que Cockpit
l'est.

### Timeouts — **A**, a surveiller

`containers.rs:8-11` : `TIMEOUT` 15 s, `TIMEOUT_LONG` 300 s. Docker Desktop passe par une VM :
les premiers appels apres un reveil sont nettement plus lents qu'un daemon natif, et `system df`
peut depasser 300 s sur une grosse installation.

---

## 4. Git

### Disponibilite — **A**

L'erreur remonte deja proprement (`.map_err(|e| format!("git: {}", e))` dans `run_git` et
`run_git_strict`). Deux nuances :
- Git for Windows n'est pas preinstalle : il faudra le dire clairement, mais le code le dit deja ;
- sur macOS `/usr/bin/git` est un *shim* qui declenche l'installation des Command Line Tools au
  premier appel. Depuis une app GUI, l'invite peut passer inapercue et l'appel rester en attente.

### `--porcelain -z` et son decoupage — **A**

`gitdiff/mod.rs:147-171`. `-z` supprime guillemets et echappements, neutralise `core.quotepath`,
et git rend **toujours** des `/`, meme sur Windows. Le `entry[3..]` est sur (les trois premiers
octets sont ASCII, donc c'est une frontiere de caractere). Le saut du champ « ancien chemin » sur
`R`/`C` est correct. Rien a toucher.

### `/dev/null` dans `--no-index` — **A**

`gitdiff/mod.rs:174` (compteurs des untracked) et `:393` (`git_diff_file`). Git traite lui-meme
la chaine `/dev/null` comme « fichier vide » dans son code de diff, independamment du shell : ca
marche avec `git.exe` appele directement. A confirmer en une commande au premier build, mais je
n'attends pas de probleme.

### Fins de ligne : le sujet est cote EDITEUR, pas cote git — **C**

`read_project_file` rend le contenu brut, CRLF compris, et il alimente le `<textarea>` de
`ui/CodeEditor.svelte`. Le DOM **normalise** la valeur d'un textarea en LF. `write_project_file`
reecrit donc le fichier entier en LF.

Sur un depot Windows avec `core.autocrlf=true` : on ouvre un fichier, on change un caractere, on
sauve, et l'onglet Git affiche **toutes les lignes modifiees**.

Detecter la fin de ligne dominante a la lecture et la restaurer a l'ecriture change ce que la
commande ecrit sur le disque : c'est une decision de conception, avec un test a ecrire.

### Parsers de diff et de log — **A**

`parse_unified_diff`, `split_multi_file_diff`, `parse_git_log` passent tous par `str::lines()`,
qui retire le `\r` final : les diffs restent lisibles sur un depot CRLF (au prix d'un `\r`
masque, ce qui est le comportement souhaitable pour de l'affichage).

`split_multi_file_diff:377` extrait le chemin par `line.rfind(" b/")` : casserait sur un fichier
contenant litteralement ` b/`, mais c'est vrai sur les trois OS, donc pas un sujet de portage.

Aucun `split('\n')` nulle part dans le crate (verifie) : rien qui laisserait passer un `\r`.

---

## 5. Rendu, polices, CSS

### Les piles monospace ne contiennent aucune police mac ni Windows — **B**

- `TerminalTab.svelte:425` : `'DejaVu Sans Mono', 'Liberation Mono', 'Noto Sans Mono', monospace`
- `theme.css:40` : `"DejaVu Sans Mono", "Liberation Mono", "Ubuntu Mono", monospace`

Aucune de ces cinq polices n'existe par defaut sur macOS ni Windows. On retombe sur le
`monospace` **generique** — exactement ce que le CLAUDE.md decrit comme la cause de la derive
visuelle sur les glyphes accentues. Sur Windows, le `monospace` generique de Chromium est
**Courier New** : pour un terminal, c'est indefendable.

Ajouter `Menlo`, `SF Mono`, `Cascadia Mono`, `Consolas` est strictement mecanique. Garder les
deux couches de `CodeEditor.svelte` sur la MEME valeur — elles utilisent `var(--font-mono)` et
`inherit`, c'est deja juste, ne pas les desynchroniser.

La pile de l'interface (`global.css:11` : `system-ui, -apple-system, "Segoe UI", Roboto`) est
deja correcte partout : **A**.

### `document.execCommand` dans l'editeur de notes — **C**

`NoteEditor.svelte:413` (`format`), `:420` (`insertHeading`), `:436` (`basculerBlocDeCode`).

`execCommand` n'est plus specifie : chaque moteur produit un HTML different, et `turndown` le
convertit ensuite. Deux cas connus :
- `formatBlock` que WebKit a longtemps voulu avec le nom de balise entre chevrons (`"<h1>"`) et
  pas toujours nu ;
- un « gras » qui sort en `<span style="font-weight:bold">` selon l'etat de `styleWithCSS` — que
  turndown **jette**, donc le gras disparait a la sauvegarde sans erreur.

`marked` et `turndown` sont du JS pur, identiques partout. Le risque est entierement dans ces
trois lignes et dans les fonctions maison qui inspectent le DOM produit (`blocDeCodeCourant`,
`ramenerEnParagraphes`, `unitesDeLaSelection`).

Remplacer `execCommand` par de la manipulation de Range explicite — ce qui a deja ete fait pour
un cas, cf. le commentaire de `:500` — change le HTML produit, donc le Markdown produit :
decision.

### `confirm()` natif, 22 appels sur des actions destructives — **C**

`AgentsView` (5), `FilesTab` (4), `ContainersView` (4), `GitTab` (3), `NoteTree` (2), plus
`GlobalSettings`, `ProcessList`, `SettingsTab`, `ProjectDetail`.

WKWebView sans delegue d'interface utilisateur rend `false` immediatement, sans rien afficher.
Le sens de la panne est heureusement le bon — on ne supprime pas par accident, on n'arrive plus
a supprimer.

Le remplacement n'est pas mecanique : `tauri-plugin-dialog` est **deja** en dependance, mais son
`ask()` est **asynchrone**, donc les 22 `if (!confirm(x)) return;` deviennent des `await`, et
`capabilities/default.json` n'autorise aujourd'hui que `dialog:allow-open`.

### Pas de menu applicatif — **B**, a verifier en premier

Aucun `menu`/`Menu` dans `lib.rs`. Tauri v2 pose normalement un menu par defaut sur macOS ; s'il
ne le fait pas dans cette configuration, **Cmd+C et Cmd+V ne fonctionnent nulle part dans
l'interface**, l'edition macOS passant par le menu Edition.

A constater au premier lancement, avant tout autre test : c'est le genre de chose qui fait
conclure a tort que « le presse-papier est casse » et lance une chasse dans `arboard`.

### Barre de titre — **A**, cosmetique

Pas de `decorations: false` dans `tauri.conf.json` : titre natif partout. Sur macOS, une barre
systeme au-dessus du `Header` maison, donc deux bandeaux empiles.

### `::-webkit-scrollbar` — **A**

`components.css:91-97`. Marche sur WebKit et sur WebView2 (Chromium). Une barre de 10 px
toujours visible jure avec les barres flottantes de macOS : cosmetique.

---

## 6. REPONSE FERME : les contournements WebKitGTK sous WebView2 (Chromium)

Question posee : ces contournements sont-ils seulement **inutiles** (benins) ou peuvent-ils
**CASSER** quelque chose ?

Reponse : **trois sur quatre sont benins et il n'y a rien a conditionner. Le quatrieme n'est pas
inutile sur Chromium — il y porte exactement le meme piege, specifie, et donc respecte a
l'identique. Aucun des quatre ne casse quoi que ce soit sur Chromium.**

### 1. « Aucun `backdrop-filter` sous du contenu » — **benin, A**

C'est une regle **soustractive** : on n'ecrit pas la propriete. Un moteur qui n'a pas le bug de
halo ne souffre pas de son absence — il affiche des panneaux translucides sans verre depoli.

Les trois `backdrop-filter: none` de `components.css:198`, `:205`, `:247` sont des annulations
defensives : s'il n'y a rien a annuler, ce sont des no-ops sur tous les moteurs.

Le seul effet sur mac et Windows est **esthetique** : ils pourraient avoir le flou et ne
l'auront pas. Rien a corriger, rien a conditionner.

### 2. Le voile plein ecran qui porte son propre `backdrop-filter: blur(12px)` — **ni inutile ni inerte sur Chromium. A garder, et a documenter autrement.**

Trois voiles le portent : `ui/Modal.svelte:47`, `CreateProjectModal.svelte:161`,
`CommandPalette.svelte:242`.

Cette declaration porte **deux justifications distinctes**, melees dans un seul commentaire :
- *(a)* le flou uniforme voulu, qui masque le halo — valable et joli sur les trois moteurs ;
- *(b)* « sans lui, WebKitGTK tue les backdrop-filter de toute la page situee dessous » —
  contournement d'un bug moteur, qui n'a AUCUN sens sur Chromium.

Et surtout, le contournement traine un effet de bord **specifie** : un `backdrop-filter` autre
que `none` fait de l'element un **bloc conteneur pour ses descendants `position: fixed`**, et
cree un contexte d'empilement. Ce n'est pas une bizarrerie WebKitGTK, c'est la spec Filter
Effects, honoree par Chromium comme par WebKit. Le CLAUDE.md a deja paye ce bug une fois
(« menu contextuel decale, 2026-08-13 »).

Autrement dit : **le piege existe a l'identique sous WebView2**. Tout nouvel overlay `fixed`
place a l'interieur d'un modal sans `use:portal` sera mal positionne sur les trois plateformes,
pas seulement sous Linux. Ce n'est donc pas un contournement Linux qu'on pourrait retirer sur
Windows : c'est une contrainte partagee.

Deuxieme effet, degradant sans casser : un blur qui couvre tout l'ecran force la composition du
contenu situe dessous a chaque image. Sous rendu logiciel — WebView2 en session Bureau a
distance, VM sans acceleration, GPU sur liste noire — l'ouverture d'un modal peut devenir
visiblement saccadee. Degradation, pas bug d'affichage.

**Conclusion operationnelle** : on garde le blur, il est legitime partout. Mais il faut
**separer les deux justifications dans le commentaire**, sinon quelqu'un qui optimisera un jour
les perfs Windows retirera le blur en croyant ne toucher qu'a de l'esthetique, et fera
reapparaitre le bug Linux du 2026-08-14.

### 3. `use:portal` — **benin, et toujours necessaire partout**

Deplacer un noeud dans `<body>` est neutre sur tous les moteurs. Deux precisions :
- la raison ecrite dans `actions/portal.ts:5` (« les conteneurs structurels portent
  `isolation: isolate` ») est **perimee** : j'ai cherche dans tout `src/`, il n'y a plus aucune
  regle `isolation` dans le CSS, seulement des commentaires historiques ;
- la raison qui **reste vraie** est celle du point 2 — le bloc conteneur cree par le
  `backdrop-filter` des voiles — et elle vaut sur les trois moteurs.

Le portal n'est donc pas un contournement Linux a retirer : c'est la regle qui protege du seul
piege cross-moteur du lot. A garder, et a re-justifier.

### 4. Tokens opaques (`--surface-base` / `--surface-raised`) pour les surfaces flottantes — **benin, ce n'est meme pas un contournement**

C'est une regle de conception : une surface flottante ne doit pas etre translucide, puisque rien
ne peint de fond derriere elle. Vrai sur tous les moteurs, independamment de tout bug. A garder
telle quelle.

### Le `!important` de la couche `html.has-wallpaper` — **benin, A**

`!important` est de la cascade, pas du rendu : comportement identique dans les trois moteurs.
L'interdiction de le retirer (CLAUDE.md) reste valable partout.

### Ce qui, lui, casserait en silence sous WebView2 — **C**

Ce n'est aucun des quatre contournements, c'est une dependance CSS moderne.

Le CSS utilise **10 `color-mix()`** (7 dans `components.css`, 3 dans `theme.css`), et toute la
lisibilite sur image de fond en depend : c'est `color-mix` qui rend les `--bg-*` translucides et
qui donne un fond aux boutons.

`color-mix()` est arrive dans Chromium 111. Le runtime WebView2 « evergreen » se met a jour tout
seul, donc en pratique ca passe. Mais sur un poste en runtime **a version figee**, ou
fraichement installe sans mise a jour, chaque `color-mix()` est une valeur invalide -> le token
n'est pas defini -> fonds transparents, bordures invisibles, boutons illisibles sur l'image.
Pas d'erreur, pas de message : une interface qui a l'air a moitie peinte.

Aucun `@supports` ne protege aujourd'hui. En revanche, aucun `:has()`, `@container` ni `@layer`
dans le projet (verifie) : `color-mix` est la seule dependance CSS moderne a surveiller.

### Le risque de fond sur cette zone — **C, decision a acter**

Aujourd'hui, **zero branchement par moteur dans tout le CSS**. Pas un `@supports`, pas une
classe moteur sur `<html>`. C'est ce qui fait que la zone rendu est massivement A dans cette
etude, alors qu'elle contient plus de contournements documentes que n'importe quelle autre
partie du projet.

Cette gratuite tient a une seule decision : « aucun `backdrop-filter` sous du contenu, jamais ».
Elle est soustractive, donc portable. La premiere fois qu'on voudra rendre le verre depoli a
Chromium sans le rendre a WebKitGTK, il faudra inventer le branchement, et on passe de un a deux
rendus a valider — donc deux bancs de test, dont un (le harnais python3+gi+Xvfb du CLAUDE.md)
n'existe que pour Linux.

Le gain est esthetique, sur deux plateformes sur trois ; le cout est permanent, sur les trois.
A ecrire comme une decision prise, pas comme un etat de fait a ameliorer.

---

## 7. Raccourcis clavier

### Le code est deja pret — **A**

Onze sites gerent `e.ctrlKey || e.metaKey` : `NoteEditor:110` et `:373`, `CodeEditor:63`,
`CommandPalette:171`, `GitTab:358`, `TerminalTab:449` et `:632`, `FilesTab:401`, `:437`, `:727`.
C'est ecrit correctement du premier coup.

Seule exception : `App.svelte:26`, le zoom Ctrl+molette, ne teste que `ctrlKey`. En pratique le
pinch-to-zoom du trackpad macOS emet un `wheel` avec `ctrlKey: true`, donc ca marchera ;
`Cmd+molette` non. Une ligne — **B**.

### Les LIBELLES, en revanche — **C**

« Ctrl » est ecrit en dur dans **~19 cles de `fr.ts` et autant dans `en.ts`** :
`header.zoom`, `note.openHint`, `files.searchHint`, `files.findInFile`, `term.searchHint`, et une
douzaine de cles `docs.*` ou le mot est enrobe dans du HTML `<span class="kbd">`.
Plus **4 occurrences en dur dans `DocsView.svelte:426-429`**.

Ce n'est pas un `replace` : il faut un mecanisme (un jeton `{mod}` substitue a l'affichage, ou
une cle `key.mod` interpolee) et repasser sur ~40 chaines dans deux langues, dont certaines
contiennent du balisage. Compter aussi « Maj » -> « ⇧ » et « Entree » -> « ⏎ » si on veut etre
coherent avec les conventions mac.

Ca reste sans risque : rien ne casse, c'est juste faux a l'ecran. Mais le mecanisme
d'interpolation est une decision, d'ou le C.

---

## 8. Les points les plus SOUS-ESTIMES

Formules en risque : ce qui casserait sans qu'on le voie, ce qui obligerait a maintenir deux
comportements, ce qui trompe le diagnostic.

### 1. Le PATH d'une app GUI macOS : la panne est invisible au developpement, et le diagnostic est trompeur

Ce qui casse sans qu'on le voie : **rien ne casse en developpement**. `npx tauri dev` est lance
depuis un shell, donc herite du bon PATH ; tous les tests passent, toutes les fonctionnalites
marchent. La panne n'apparait qu'apres packaging, chez l'utilisateur.

Et elle porte un message **activement trompeur** : « docker introuvable », « Claude non
installe », « aucun serveur LSP » — sur une machine ou l'utilisateur voit tres bien ces outils
fonctionner dans son Terminal. Il conclura que Cockpit est mal fait, pas que le PATH d'une
`.app` est ampute.

Le risque secondaire est pire que le premier : c'est une panne **transversale** qui se presente
comme cinq pannes independantes. On corrigerait Docker, puis Claude, puis le LSP, chacun avec un
contournement local — et chaque contournement local est un comportement de plus a maintenir,
alors qu'il n'y avait qu'une cause.

### 2. Les fenetres console Windows : aucun test ne les voit, parce que tout fonctionne

Ce qui casse sans qu'on le voie : **rien, au sens fonctionnel**. `docker compose ps` rend le bon
JSON, `git status` rend le bon statut, les tests sont verts, une revue de code ne montre rien
d'anormal. C'est purement visuel — et c'est precisement pour ca que ca passerait la porte : il
n'existe aucun automate qui constate qu'une fenetre noire clignote.

Le facteur d'amplification est le monitor Docker : toutes les 5 secondes, un `compose ps` par
projet, en permanence, tant que l'application est ouverte. Avec cinq projets, cinq flashs toutes
les cinq secondes, pendant toute la journee de travail.

Ce n'est pas un defaut cosmetique : c'est une application inutilisable, et le premier
utilisateur Windows le dira dans ces termes-la.

### 3. La corbeille qui fait tomber l'application, de facon intermittente

Ce qui casse sans qu'on le voie : `trash` appelle `panic!`, pas `Err`. Un panic dans une
commande Tauri n'est pas une erreur remontee dans un toast et ecrite dans `cockpit.log` — c'est
le processus qui s'arrete.

Toute la mecanique de remontee sur laquelle repose le diagnostic a distance (journal local,
fiche machine, envoi au serveur de suivi) est **contournee** : il ne reste rien a lire. Le hook
de panic de `lib.rs` ecrirait bien une ligne — mais dans un chemin Linux, donc nulle part sur
Windows (voir zone 1). Les deux defauts se composent : le crash le plus opaque possible, dans le
seul cas ou le journal ne fonctionne pas.

Et le declenchement depend de l'etat COM du thread qui execute la commande, que nous ne
choisissons pas. Ca peut marcher cinquante fois puis tuer l'application la cinquante-et-unieme,
sur le meme clic, sur le meme fichier. Non reproductible, sans trace, sur une action
destructive : l'utilisateur ne saura meme pas si le fichier est parti.

Facteur aggravant : ca vient d'une dependance choisie **parce qu'elle est multiplateforme**
(le commentaire du `Cargo.toml` le dit explicitement). C'est exactement le genre de confiance
qu'on ne reexamine pas.

### 4. Le separateur de chemin : une cause unique deguisee en cinq bugs distincts

Ce qui casse sans qu'on le voie : chaque source de `rel_path` est **coherente avec elle-meme**.
`git status -z` rend toujours des `/`, `strip_prefix` rend des `\` sous Windows. Teste onglet par
onglet, tout parait correct : l'arbre de fichiers marche, l'onglet Git marche.

Ca ne casse qu'aux **croisements** — la palette Ctrl+K qui ouvre un fichier, la recherche
globale qui saute a une ligne, le renommage d'un dossier qui doit faire suivre le fichier
ouvert, le depliage de l'arbre jusqu'a un resultat. Cinq symptomes differents, cinq endroits
differents dans le code, aucun lien apparent entre eux.

On corrigerait quatre bugs separement — avec quatre rustines locales, donc quatre comportements
divergents a maintenir — avant de voir qu'il n'y en avait qu'un et qu'il se reglait en trois
lignes.

Corollaire durable : sans un contrat ecrit (« `rel_path` est toujours en `/` »), le prochain
`strip_prefix` ajoute dans six mois reintroduit le bug et la chasse recommence.

### 5. La conversion CRLF -> LF par l'editeur : le code Rust est irreprochable, la perte vient du DOM

Ce qui casse sans qu'on le voie : aucun test Rust ne peut l'attraper. `write_project_file` ecrit
fidelement ce qu'on lui donne ; `read_project_file` lit fidelement le disque. La perte se
produit **entre les deux**, dans la normalisation silencieuse de la valeur d'un `<textarea>` par
le moteur de rendu. Un test d'integration cote Rust passerait, un test unitaire cote front
aussi.

Le symptome observe est pire que la cause : l'utilisateur change une virgule et voit 800 lignes
modifiees dans l'onglet Git. Il ne dira pas « il y a un probleme de fins de ligne », il dira
« ton editeur a reecrit mon fichier ».

Et sur un depot partage, il aura raison de le dire : le commit qui en sort est illisible pour
ses collegues. C'est le seul dommage de cette liste qui **sort de l'application et atterrit chez
des tiers**.

### 6. Le statut Claude sur macOS : une hypothese fausse, pas un bug

Ce qui casse sans qu'on le voie : rien ne plante, rien ne remonte d'erreur. On lit un fichier
qui n'existe pas, on repond « deconnecte », et on propose un flow de connexion. L'utilisateur
lance le flow, se connecte, revient — et l'ecran dit toujours « deconnecte », parce que la CLI a
ecrit dans le Keychain. Boucle fermee, sans message pour en sortir.

Le risque de fond est plus large que ce seul ecran : c'est le seul endroit du code qui affirme
quelque chose sur l'**etat d'un autre logiciel** en lisant son stockage interne. Cette hypothese
n'est vraie que sur Linux, et rien dans le code ne le dit.

Le meme raisonnement s'applique a trois autres fonctionnalites adossees a la disposition sur
disque d'un programme tiers, dont on n'a verifie la disposition que sur une plateforme :
`~/.claude/projects/` (sessions Claude Code), `~/.claude/settings.json` et
`~/.claude/plugins/cache/` (marketplace d'agents).

### 7. La regle CSS qui rend le portage gratuit — et le jour ou on la sacrifiera

Ce qui obligerait a maintenir deux comportements : voir la fin de la zone 6. Aujourd'hui zero
branchement par moteur, donc portabilite par construction. Le jour ou on veut le verre depoli
sur Chromium seulement, on cree deux rendus a valider, avec un banc de test qui n'existe que
pour Linux. Gain esthetique sur deux plateformes, cout permanent sur les trois.

C'est le seul point de cette liste qui n'est pas un bug present mais une porte a fermer
explicitement.
