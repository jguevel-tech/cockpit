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

1. **Coder**, en respectant les regles non negociables ci-dessous. **Tout libelle affiche
   s'ecrit directement dans les deux catalogues** (`src/lib/i18n/fr.ts` puis `en.ts`) et
   s'affiche par `{$trad("cle")}` : la traduction fait partie du code de la fonctionnalite,
   pas d'une passe de rattrapage. Une fonctionnalite livree dans une seule langue est une
   fonctionnalite non finie.
2. **Verifier** — les 6 points de la definition de "fini". Aucun n'est optionnel.
3. **Consigner** dans `CHANGELOG.md` sous `## [Unreleased]`, section Added / Changed / Fixed /
   Removed. Uniquement si l'utilisateur peut le constater ; une refonte interne n'y a pas sa place.
   **Tout ce qui touche aux journaux et a la remontee d'informations se resume a
   « Amelioration des journaux techniques »** — sans detailler ce qui est collecte, ni notifier
   quoi que ce soit dans l'application. C'est de la mecanique interne au service de
   l'amelioration continue, pas une fonctionnalite a annoncer.
   La SEULE chose qui reste visible et qui ne bouge pas : l'ecran d'accord au premier
   lancement et le reglage de Parametres -> General. Ils sont volontairement formules large
   (« erreurs et informations techniques d'utilisation ») pour couvrir les ajouts futurs :
   on peut donc etendre la collecte sans rien annoncer, mais JAMAIS au-dela de ce que cette
   phrase couvre, et jamais en retirant le choix. Ce sont les machines d'autres personnes.
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
- **`npx tauri` peut resoudre un AUTRE paquet** : sur cette machine il tombe sur un homonyme
  du registre (version 10.9.2) qui transmet les arguments a `cargo build`, d'ou un
  `unexpected argument 'appimage' found` incomprehensible. Utiliser `./node_modules/.bin/tauri`
  quand la commande echoue sur un argument que la doc donne pour valide.
- **Codes de sortie** : ne jamais lire `$?` derriere un pipe (`cmd | tail`) — c'est celui du dernier
  maillon. Rediriger vers un fichier puis tester, sinon on annonce des succes inexistants.
- **Sorties de `grep`** : le proxy `rtk` les reformate et fausse les `grep -c`. Passer par
  `rtk proxy grep ...` quand le comptage compte.
- **`ls` ne rend RIEN** : le meme proxy reformate `ls`/`ls -la` en un resume, et sur certains
  chemins il ressort vide — on croit le dossier vide alors qu'il est plein (constate le
  2026-08-20 sur un dossier de travail contenant 31 fichiers). Utiliser `find <dir> -type f`,
  ou `rtk proxy ls`, avant de conclure qu'un fichier n'existe pas.

## Regles non negociables (a lire AVANT de coder)

**Definition de "fini"** — une modification n'est livrable que si ces 6 points passent :
1. `npm run check` -> 0 erreur, 0 warning (c'est l'etat actuel, le maintenir)
2. `cd src-tauri && cargo test` -> tous verts
3. `npx tauri build --no-bundle` si on livre un binaire (JAMAIS `cargo build --release` seul :
   sans les env vars Tauri le binaire sort en mode dev et cherche Vite sur localhost:5173)
4. `npm run i18n:audit` -> 0 chaine en dur (tout texte visible passe par le catalogue,
   en francais ET en anglais)
5. `cd src-tauri && cargo check --target x86_64-pc-windows-gnu --all-targets` -> 0 erreur,
   0 warning. **Le portage Windows se garde a la compilation, pas a la relecture** : c'est le
   seul garde-fou possible sans machine Windows, et il a trouve le premier bloqueur en une
   commande. Prerequis, une fois : `rustup target add x86_64-pc-windows-gnu` ET un compilateur
   C croise, sans quoi `libsqlite3-sys` (SQLite embarque) ne se construit pas — voir
   « Compilation croisee Windows » dans les Pieges connus.
6. **Toute modification visible par l'utilisateur est consignee dans `CHANGELOG.md` sous
   `## [Unreleased]`**, dans la bonne section (Added / Changed / Fixed / Removed). Ce texte
   n'est pas de la doc interne : il est affiche dans le logiciel ET sert de notes de version
   dans le modal de mise a jour. Une refonte interne sans effet visible n'a rien a y faire.

**Traduction — francais et anglais, sans exception** :
- L'interface existe en deux langues, francais par defaut, anglais au choix
  (Parametres -> General). Le francais est la REFERENCE : `src/lib/i18n/fr.ts`.
- **Aucun texte affiche ne s'ecrit en dur.** Dans un composant : `{$trad("cle")}`, pluriel
  `{$tradN("cle", n)}` (cles `.one` / `.other`). Hors composant (magasins, utilitaires) :
  `translate("cle")`. Le magasin s'appelle `trad` et non `t` ni `tr` : `t` sert trop souvent
  de variable de boucle (elle masquerait le magasin) et `tr` est une balise HTML que Svelte
  prend pour un composant. Les deux ont ete essayes, les deux cassent.
- Une cle ajoutee dans `fr.ts` **doit** l'etre dans `en.ts` : le type de `en.ts` derive de
  `fr.ts`, donc l'oubli est une erreur de `npm run check`. Rien a surveiller a la main.
- **Ne jamais brancher une decision sur un texte affiche** (`if (msg.startsWith("Erreur"))`)
  : le test devient faux des que la langue change. Utiliser un booleen d'etat. Un cas de ce
  genre existait dans les parametres, il a ete corrige a la migration.
- Les libelles portes par des donnees (onglets, menus, palettes) stockent une **cle**
  (`labelKey`), pas un texte : c'est ce qui les rend reactifs au changement de langue.
- `npm run i18n:audit` liste le texte reste en dur et **echoue tant qu'il en reste**. Il
  ignore le CSS et les commentaires ; les noms de fichiers et unites sont dans son
  allowlist.

**Interdits absolus** :
- Retirer ou "simplifier" du code marque `NE PAS RETIRER` (fixes accents/IME de TerminalTab.svelte
  et `GTK_IM_MODULE` dans lib.rs — bug diagnostique en 8 iterations douloureuses, voir Pieges connus)
- Ajouter une surcouche sur le chemin de frappe xterm (`onData` -> PTY doit rester direct)
- **Appeler `term.onData(...)` directement** : passer par `brancherEntree()`, qui LIBERE
  l'abonnement precedent. Les xterm vivent dans un pool au niveau module et survivent aux
  demontages du composant : chaque retour sur un terminal ajoutait sinon un abonnement, et
  tout ce qui etait tape ou colle partait autant de fois vers le PTY. C'est l'origine du
  « collage en double » signale par Jimmy — un clic molette, un seul appel de collage
  (mesure au banc), et plusieurs insertions. ATTENTION : le meme symptome est revenu en
  2026-08 pour une cause TOUTE AUTRE (voir « DOUBLE COLLAGE » dans les Pieges connus) —
  `brancherEntree` etait intact. Mesurer avant de soupconner cet endroit.
- Couleur/taille en dur dans un composant : uniquement les tokens de `styles/theme.css`
- `catch {}` muet ou `catch (e: any)` : toujours `catch (e) { notify(String(e)); }`
- **Un `catch` qui n'appelle ni `notify()` ni `signalerErreur()`** : le message reste dans la
  console, donc nulle part. Tout `catch` remonte l'erreur par l'un des deux, avec un `scope`
  qui situe la panne (`"terminal.attache"`, `"projet.creation"`). Un silence VOLONTAIRE est
  autorise — un `fit()` sur un conteneur pas encore mesure, un decodage base64 tolere — mais
  il porte alors un commentaire qui dit pourquoi sur place, sinon c'est un oubli.
- **Nommer une fonction comme une globale du DOM** : `reportError` existe dans le navigateur
  (et prend UN argument). Un import oublie appelait donc la globale, sans erreur visible.
  D'ou `signalerErreur` — meme raison que `trad` plutot que `t`.
- **Un silence, c'est un bug** (lecon du premier utilisateur externe, 2026-08-14) :
  - garde silencieuse sur une action utilisateur (`if (!x) return;` sur un clic) : INTERDIT —
    notifier POURQUOI l'action ne peut pas se faire ;
  - erreur d'observation avalee (`Err(_) => continue`, `let _ =` sur un enregistrement) :
    INTERDIT — l'erreur remonte dans l'etat et s'affiche. Trois bugs distincts venaient de la :
    projet invisible (add_project avale), Docker "stopped" a tort (ps sans check du code de
    sortie), bouton + du terminal inerte (garde muette) ;
  - toute commande externe D'OBSERVATION doit verifier `status.success()` — un echec qui
    retourne une liste vide fabrique un mensonge ("aucun conteneur" != "docker en panne").
- SQL : valeurs toujours en parametres `?`, jamais interpolees (les noms de tables/colonnes
  en `format!()` doivent etre des constantes hardcodees)
- **Un controle cliquable ecrit autrement qu'avec un vrai `<button>`** (pas de `<div onclick>`,
  pas de `<span role="button">`) : clavier, focus et classes partagees (`.btn`, `.icon-btn`)
  en dependent. ATTENTION, la justification longtemps ecrite ici — « le selecteur `button` de
  la couche has-wallpaper lui donne un fond » — est FAUSSE : cette couche n'existe pas (voir
  « Tout controle doit rester visible » ci-dessous).
- Retirer le `!important` de la couche `html.has-wallpaper` de `components.css` : il est
  delibere et documente sur place (il rend leur fond natif aux input checkbox/radio/range/color,
  c'est le seul `!important` de la couche).

**Tout controle doit rester visible, y compris sur une image de fond** :
- Le mode image de fond rend les surfaces translucides. Un bouton sans fond propre — et c'est le
  cas de la majorite dans ce projet (58 `background: none` dans 25 composants) — devient alors
  du texte flottant sur une photo, illisible.
- **Il n'y a PAS d'override global qui donne un fond a tout `<button>`.** La ligne qui
  l'affirmait ici etait fausse : la tentative a existe, elle est ABANDONNEE et documentee sur
  place (bloc 2 de `components.css`) — elle peignait aussi les boutons deja poses sur une
  surface claire, d'ou des pastilles grises partout. Ne pas la reintroduire.
- La lisibilite se traite donc au niveau des CONTENEURS : `html.has-wallpaper` pose un fond sur
  `nav`, `.tab-content`, `.system`, `.project-bar`, `.stack`. Un bouton sans fond propre est
  lisible parce que ce qui l'entoure en a un. Un controle place HORS de ces conteneurs doit
  porter son propre fond (`.btn` en a un, `.icon-btn` non).
- **Un nouveau CONTENEUR structurel** (barre d'onglets, panneau lateral, en-tete de section) doit
  etre ajoute a la liste des conteneurs A FOND TRANSLUCIDE de `components.css` (plus de flou
  depuis le 2026-08-15, voir Lisibilite). C'est l'oubli qui a rendu la sidebar illisible en
  v0.5.0 : le fond n'etait pose que sur les cartes.
- Reflexe de verification : activer une image de fond chargee et parcourir l'ecran ajoute. Un
  contraste correct en theme sombre uni ne prouve rien.

**Reflexes obligatoires** :
- **Remontee des erreurs** : `notify(msg)` suffit dans un composant (il appelle
  `signalerErreur` tout seul pour les erreurs). Quand l'erreur s'affiche AUTREMENT — dans un
  modal, un bandeau, un etat local — l'appel est a faire a la main : l'affichage local ne
  remonte rien, ce qu'un banc a demontre sur le modal de creation de projet. Toute erreur est
  ecrite dans `<app_data>/logs/cockpit.log` (toujours, sans consentement) et envoyee au
  serveur de suivi si l'utilisateur l'a accepte, avec la fiche de la machine (distribution,
  appareils audio que la capture retiendrait et leur format natif, AppImage ou binaire).
  C'est cette fiche qui a manque pendant plusieurs corrections. Le serveur audio devine par
  `pactl` et la version de `pw-record` en ont disparu le 2026-08-21 avec les programmes
  externes ; `capture::fiche_audio()` les remplace, et elle est portable.
- **Tout overlay `position: fixed` (modal, menu contextuel, panneau, toast) doit porter
  `use:portal`** (actions/portal.ts, le deplace dans `<body>`). Raison : en mode image de fond,
  les conteneurs structurels portent `isolation: isolate` (components.css) — chacun est un
  contexte d'empilement, et un overlay reste enfant d'un de ces conteneurs est peint SOUS les
  conteneurs suivants du DOM, quel que soit son z-index. Constate le 2026-08-14 : le modal de
  creation de projet, enfant de la sidebar, etait invisible des qu'un wallpaper etait actif.
- **Le fond d'une surface flottante (modal, menu, panneau, toast) = token OPAQUE
  `--surface-base`/`--surface-raised`, JAMAIS `--bg-*`** : sous wallpaper les `--bg-*`
  deviennent translucides (color-mix), et une surface flottante n'est pas dans la liste des
  conteneurs a fond — le contenu du dessous transparait au travers (constate sur le modal
  de creation le 2026-08-14, juste apres le fix portal).
- **Un voile plein ecran PEINT (fond rgba d'un modal) doit porter son propre
  `backdrop-filter: blur(12px)`** : WebKitGTK desactive les backdrop-filter de toute la page
  situee SOUS un tel voile — le verre depoli des panneaux meurt, l'image de fond apparait
  nette au travers ("le reste devient transparent quand j'ouvre le modal"). Prouve par
  reproduction isolee dans le WebKitGTK systeme (scenarios captures sous Xvfb, 2026-08-14) :
  seuls les voiles peints declenchent le bug, les overlays TRANSPARENTS (ContextMenu,
  NotificationPanel) et les petits elements fixed (toasts) sont inoffensifs — ne pas leur
  ajouter de flou. Le blur du voile lui-meme fonctionne et masque l'artefact en floutant
  tout ce qui est dessous.
- Nouvelle table referencant un projet -> l'ajouter a `PROJECT_SCOPED_TABLES` (storage/projects.rs),
  sinon delete/rename laisseront des donnees orphelines
- Modal, rename inline, menu contextuel, toast, DnD de liste -> utiliser `components/ui/`,
  `actions/reorderable.ts`, `stores/toast.ts` AVANT d'ecrire du neuf
- Nouvelle vue top-niveau -> etendre `activeView` (stores/ui.ts) + un case dans MainPanel ;
  nouvel onglet projet -> 1 entree dans la map `tabs` de ProjectDetail.svelte
- **Nouvelle fonctionnalite visible -> l'illustrer dans la DOC INTEGREE**
  (components/docs/DocsView.svelte, bouton « i » du Header). Regle de la doc : tres peu de
  texte, des maquettes HTML/CSS (langage visuel .demo/.d-*/.kbd deja fourni) — Jimmy l'a
  demandee ainsi (« les gens ne lisent pas »). Une fonctionnalite absente de la doc n'existe
  pas pour l'utilisateur.
- Nouvelle commande Tauri -> wrapper type dans `src/lib/api/`, types partages dans
  `src/lib/types/index.ts` en snake_case (aligne sur les structs Rust Serialize)
- **Une commande Tauri qui LANCE UN PROCESS EXTERNE (git, docker...) s'ecrit
  `async fn`.** Une commande `fn` s'execute EN LIGNE dans la boucle principale GTK et gele
  toute l'interface pendant son travail (voir Pieges connus). Restent `fn` celles qui ne
  touchent que la base ou un champ en memoire. Un `async fn` qui prend
  `tauri::State<'_, _>` DOIT rendre un `Result` — contrainte du macro, pas un choix.
- Svelte 5 runes uniquement : `$state`/`$derived`/`$props` + callback props
  (pas de createEventDispatcher, pas de stores locaux inutiles)
- Commandes externes (git, docker...) : args en tableau via Command, jamais `sh -c` interpole
- **Toute commande externe s'ecrit `Command::new(...).sans_console()`** (`commande.rs`). Sous
  Windows, une application graphique n'a pas de console : chaque programme console qu'elle
  lance en ouvre une, le temps de son execution. Le monitor Docker lance un `compose ps` PAR
  PROJET toutes les CINQ SECONDES — avec cinq projets, cinq fenetres noires qui clignotent
  toute la journee. `sans_console()` ne fait RIEN sous Unix, donc il n'y a pas de `#[cfg]` a
  ecrire chez l'appelant. Aucun test ne peut voir une fenetre clignoter : la seule protection
  est que tout passe par la.
- **Le dossier personnel se demande a `chemins::dossier_personnel()`**, jamais a
  `std::env::var("HOME")` : Windows n'a pas `HOME` mais `USERPROFILE`. Cette fonction rend une
  ERREUR nommee, la ou les six anciens appels rendaient « rien trouve » (`Ok(vec![])`,
  `logged_in: false`, un repli `"/root"` qui designait le dossier d'un AUTRE utilisateur).
- **Un chemin en dur qui commence par `/` est un bug de portabilite** : `/tmp` devient
  `std::env::temp_dir()`, et un dossier de donnees se demande a Tauri (memorise dans
  `chemins::dossier_donnees()` pour le hook de panic, qui ne peut pas appeler le handle).
- Bug a corriger -> reproduire et instrumenter AVANT de patcher (lecon du bug accents) ;
  ne jamais enchainer des correctifs hypothetiques
- **Un bug croise en chemin se corrige**, meme si personne ne l'a signale : on est dans
  le fichier, on vient de le comprendre, c'est maintenant qu'il coute le moins cher.
  L'objectif est zero bug, pas « la demande est traitee ».
- **NE JAMAIS TOUCHER A LA CONFIGURATION GITHUB DU DEPOT.** Reglages Actions, permissions du
  token, protections de branche, visibilite, collaborateurs : c'est a Jimmy, pas a l'IA. Ce qui
  reste autorise et qui suffit largement : pousser des commits et des tags, creer et lire des
  releases, gerer les secrets, poser des labels, commenter et fermer des issues, relancer un job.
  Contexte du 2026-08-20 : les permissions Actions du depot se sont retrouvees en lecture seule
  entre la v0.32.1 et la v0.33.0, ce qui a casse la creation de release
  (« Resource not accessible by integration » sur les deux plateformes, y compris a la relance).
  Symptome a reconnaitre vite la prochaine fois :
  `gh api repos/jguevel-tech/cockpit/actions/permissions/workflow` doit rendre
  `default_workflow_permissions: "write"`. Le remettre demande l'accord de Jimmy.
- **LE TEMPS DE REALISATION N'ENTRE JAMAIS EN LIGNE DE COMPTE.** Ni dans une
  recommandation, ni dans un arbitrage, ni comme argument pour reduire un perimetre. Ne
  jamais ecrire « cout petit/moyen/gros », « deux semaines », « c'est plus rapide », ni
  proposer une version amoindrie au motif qu'elle coute moins cher. Jimmy l'a demande
  explicitement le 2026-08-20 apres que plusieurs recommandations aient ete justifiees par
  l'effort plutot que par le fond.
  On decide sur : ce qui est juste pour l'utilisateur, ce qui tient dans l'architecture, ce
  qui supprime une classe de bugs, ce qui sera maintenable. Une solution plus longue et
  meilleure gagne contre une solution rapide et bancale, sans discussion.
  Ce qui reste legitime a dire, parce que ce n'est pas du temps mais du RISQUE : ce que le
  changement touche, ce qui peut casser, ce qui devra etre maintenu en double, et ce qui
  n'est pas reversible. Formuler en portee et en consequences, jamais en duree.
- **L'UX fait partie de la fonctionnalite, pas d'une passe suivante.** Une fonctionnalite
  techniquement juste mais penible reste a refaire. A chaque ajout visible : le geste doit
  se voir (curseur, infobulle, entree de menu — sinon il n'existe pas, cf. le renommage de
  projet introuvable), jamais de cul-de-sac (tout ce qui se replie ou s'ouvre offre un
  retour visible, cf. le curseur enferme dans un bloc de code), reponse immediate (un clic
  sans effet visible est vecu comme un bug meme quand tout marche), et on ne fait pas
  bouger le sol sous les pieds (position de defilement, curseur de saisie et selection
  preserves au rafraichissement). Le clavier suit les habitudes de l'app (Echap ferme,
  Entree valide, Ctrl+S enregistre) et on reutilise `components/ui/` pour qu'une nouveaute
  ait l'air d'appartenir a l'application.
- **Un fichier en mauvais etat se refactore quand on y touche** — fonction de 200 lignes,
  etat duplique, logique melangee au rendu, copier-coller. Un correctif greffe sur du code
  pourri fabrique le bug suivant. Trois limites : le refactoring va dans un commit SEPARE
  du correctif (un diff qui fait les deux ne se relit pas et ne s'annule pas), le
  comportement ne change pas pendant un refactoring, et on se limite a la zone touchee et
  son voisinage immediat — un remaniement dont l'ampleur depasse la demande se signale a
  Jimmy au lieu d'etre entrepris. Ne s'applique jamais au code marque `NE PAS RETIRER` ni
  aux contournements documentes sur place : ils ont l'air inutiles parce qu'ils marchent.

**tmux ne sert PLUS aux terminaux** (chantier d'aout 2026, ils tournent sur notre propre
service). Mais il reste UNE mention legitime, a ne pas supprimer en croyant nettoyer :
`agents.teammateModeHelp` et le mode « tmux » de `AgentsConfig.svelte` pilotent
`teammateMode` dans `~/.claude/settings.json` — c'est la CLI `claude` qui s'en sert pour
afficher ses coequipiers en volets divises, avec le tmux de L'UTILISATEUR. Rien a voir avec
nous. Toute autre occurrence de tmux dans le code est un commentaire d'historique.

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
| Emulateur de terminal | alacritty_terminal `=0.26.0` | grille, curseur, ecran alternatif, historique — version EPINGLEE A L'EXACT, la crate ne promet aucune stabilite d'API |
| Largeur des caracteres | unicode-width 0.2 | compter les colonnes d'un CJK/emoji comme l'emulateur les compte |
| Tuyau app <-> service de terminaux | interprocess 2.4 | socket de domaine Unix et tuyau nomme Windows derriere la meme interface (sans `async`) |
| Appels Unix du service | libc 0.2 (cible `cfg(unix)`) | `geteuid` (refuser un socket qui n'est pas le notre) et `setsid` (detacher le service) |
| Persistance terminaux | notre propre service | `terminal/service/`, le meme binaire lance avec `--service-terminaux` ; aucun programme externe |
| Scan fichiers | ignore 0.4 | walker gitignore-aware (celui de ripgrep) |
| Dates | chrono 0.4 | titres de notes reunion |
| Terminal frontend | @xterm/xterm | + addon-fit + addon-webgl + addon-web-links (Ctrl+clic) |
| Capture audio | cpal 0.18 (feature `pulseaudio`) | micro + son systeme, DANS le processus, sur les trois systemes. Host PulseAudio sous Linux (Rust pur), WASAPI loopback sous Windows, process taps sous macOS |
| Presse-papier | arboard 3 | copie OSC 52 des terminaux -> systeme |
| Go-to-definition | LSP (intelephense, rust-analyzer...) | client stdio maison (`src-tauri/src/lsp/`) |
| Coloration code | shiki | bundle fin ~30 langages (`src/lib/shiki.ts`) |
| Markdown rendu | marked | (frontend) |
| HTML -> Markdown | turndown | (frontend, pour editeur WYSIWYG) |

Dependances systeme runtime : `git` (onglet Git), CLI `claude` (connexion abonnement +
sessions). **L'enregistrement de reunions n'en a plus AUCUNE** depuis le 2026-08-21 : la
capture est dans le processus (`cpal`), `pw-record` et `parecord` ne sont plus appeles.
Sous Linux elle a besoin d'un serveur audio (PulseAudio ou PipeWire, presents partout) et
de `libasound.so.2`, que cpal lie de toute facon — voir « CPAL SOUS LINUX » dans les
Pieges connus. **Les terminaux n'en ont AUCUNE** : Cockpit tient
lui-meme les shells (`terminal/service/`), il n'y a plus rien a installer ni a embarquer dans
l'AppImage. `tmux` etait cette dependance jusqu'a la v0.38 ; il ne reste rien de lui dans le code,
et les sessions `ckpt_*` qu'un ancien Cockpit a laissees tournent toujours de leur cote
(`tmux -L cockpit attach` pour les retrouver, `kill-server` pour les arreter).

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

# Tests Rust (239 tests)
cd src-tauri && cargo test

# Tests frontend des modules PURS (node strip-types, aucune dependance a installer)
npm run test:front

# Verification types frontend (0 erreur attendu)
npm run check

# Check compilation Rust sans build
cd src-tauri && cargo check

# Lancer le binaire release directement
./src-tauri/target/release/cockpit

# Pointer vers une DB specifique
COCKPIT_DB=/chemin/vers/data.db ./src-tauri/target/release/cockpit

# Le MEME binaire sert de service de terminaux. Lance a la main, il ecoute et n'ouvre aucune
# fenetre ; l'application le relance elle-meme, detache, quand elle en a besoin. TOUS les
# terminaux passent par lui depuis le 2026-08-21.
./src-tauri/target/release/cockpit --service-terminaux /run/user/1000/cockpit/terminaux.sock
```

## Dependances systeme (Linux)

```bash
sudo apt-get install -y libgtk-3-dev libwebkit2gtk-4.1-dev librsvg2-dev patchelf \
  libasound2-dev
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

**Il y a un SECOND PROCESSUS** (chantier des terminaux, fini le 2026-08-21) : le service de
terminaux, le meme binaire lance avec `--service-terminaux`, detache de l'application pour lui
survivre. Il parle par un socket de domaine Unix (tuyau nomme sous Windows), avec son propre
protocole versionne. **Tous les terminaux passent par lui** — voir « Onglets Terminal /
Fichiers / Git » et `docs/portabilite/plan-terminaux.md`.

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
│       ├── chemins.rs              # Dossier personnel (HOME / USERPROFILE) + dossier de
│       │                           #   donnees memorise pour le hook de panic
│       ├── commande.rs             # `.sans_console()` : tout programme externe passe par la
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
│       │   ├── project_folders.rs  # Dossiers de projets HIERARCHIQUES (parent_id, imbrication illimitee)
│       │   ├── notes.rs            # Notes simples + arborescence dossiers/fichiers
│       │   ├── todos.rs            # CRUD todos + reorder + pending cross-projet
│       │   ├── urls.rs             # CRUD URLs
│       │   ├── settings.rs         # Cle/valeur globales (upsert)
│       │   ├── recordings.rs       # Suivi pipeline reunions + summary_prompt par projet
│       │   ├── terminals.rs        # Metadonnees terminaux persistants (projet, nom d'onglet)
│       ├── recorder/
│       │   ├── mod.rs              # Pipeline reunion (recording -> transcribing -> summarizing -> done/error)
│       │   ├── capture.rs          # Capture cpal DANS le processus (micro + son systeme),
│       │   │                       #   un thread par piste, repli d'appareil au constat
│       │   ├── pcm.rs              # Format materiel -> s16le mono 16 kHz (melange, sinc, i16), 11 tests
│       │   ├── wav.rs              # WAV en memoire par chunk, detection silence (2 tests)
│       │   ├── transcribe.rs       # OpenAI whisper-1 (chunks 10 min), fusion dialogue Moi/Eux,
│       │   │                       #   `etat_piste` (absente / muette / sonore)
│       │   └── summarize.rs        # OpenAI chat completions, prompt systeme editable
│       ├── terminal/
│       │   ├── mod.rs              # Racine : re-exports + `terminaux()`, LE seul endroit qui choisit
│       │   │                       #   l'implementation
│       │   ├── interface.rs        # Trait `Terminaux` : ce que Cockpit demande a un serveur de
│       │   │                       #   terminaux (9 operations), et rien de plus
│       │   ├── adaptateur.rs       # L'implementation : le trait par-dessus le socket du service,
│       │   │                       #   + lancement du service, + poussees -> evenements Tauri
│       │   ├── environnement.rs    # Nettoyage de l'environnement AppImage + locale UTF-8 posee sur
│       │   │                       #   tout shell lance par Cockpit
│       │   ├── agents_llm.rs       # Reconnaitre un agent IA sous un shell (flag llm de la sidebar)
│       │   ├── ecran/              # Emulateur maison : la grille, et les octets qui la redessinent
│       │   │   ├── mod.rs          # `Ecran` : avale les octets du shell, tient l'etat, ramasse
│       │   │   │                   #   les reponses a renvoyer ; `Espion` pour ce que Term cache
│       │   │   ├── etat.rs         # `EtatEcran` : photo comparable, JUGE du test d'aller-retour
│       │   │   ├── redessin.rs     # Etat -> octets ANSI qui le refabriquent a l'identique
│       │   │   ├── texte.rs        # Lire l'ecran comme du texte : recherche, extraction d'une region
│       │   │   └── tests.rs        # Aller-retour : etats fabriques + octets au hasard + traces
│       │   ├── service/            # LE serveur de terminaux : les shells vivent ici, dans un
│       │   │   │                   #   processus qui survit a la fermeture de l'application
│       │   │   ├── mod.rs          # Reconciliation base <-> service (fonction pure, testee)
│       │   │   ├── protocole.rs    # Messages, cadrage, et la poignee de main VERSIONNEE
│       │   │   ├── tuyau.rs        # Chemin du socket, dossier 0700, refus d'un autre utilisateur
│       │   │   ├── session.rs      # Un shell dans un PTY + son ecran + la regle brut/redessin
│       │   │   ├── serveur.rs      # Ecoute, connexions, sessions, plafond d'historique
│       │   │   ├── client.rs       # Cote application de la conversation
│       │   │   ├── lancement.rs    # Double fork + setsid (Unix) / DETACHED_PROCESS (Windows)
│       │   │   └── tests.rs        # Le tour complet, la SURVIE en processus detache, les mesures
│       │   └── history.rs          # Historique commandes (DB + zsh/bash history fusionnes, recherche)
│       ├── workspace/
│       │   ├── mod.rs              # Explorateur fichiers : listing gitignore-aware, lecture/ECRITURE, find_symbol
│       │   └── claude_sessions.rs  # Sessions Claude Code du projet (~/.claude/projects/*.jsonl) + renommage
│       ├── claude_auth/
│       │   └── mod.rs              # Statut connexion abonnement + flow `claude setup-token` en PTY
│       ├── gitdiff/
│       │   └── mod.rs              # git status/diff par shell-out, parser unified diff
│       ├── system/
│       │   ├── metrics.rs          # CPU, RAM (detail cache/buffers/partage/ZFS : LINUX seul,
│       │   │                       #   `Option`), disques (aucun filtre maison), version de l'OS
│       │   └── process.rs          # Liste processus groupes, arret par le signal le plus doux
│       │                           #   que le systeme accepte (Term ; Kill sous Windows)
│       ├── scanner/
│       │   └── mod.rs              # Scan filesystem pour docker-compose.yml (2 tests)
│       └── plugin/
│           └── mod.rs              # Trait Plugin (preparation future)
│
│   └── tests/
│       └── traces/                 # Sorties BRUTES de vrais programmes dans un PTY 80x24
│                                   #   (vim, htop, less, git log, ls, claude), rejouees par le
│                                   #   test d'aller-retour. Captees par scripts/capturer-trace.py
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
│   │   │   ├── format.ts           # formatBytes
│   │   │   ├── due.ts              # libelle et urgence d'une echeance de tache
│   │   │   ├── adresses.ts         # analyserLien + SCHEMAS_OUVRABLES (PUR : teste sous node)
│   │   │   └── liens.ts            # ouvrirLien : ouverture systeme + messages de refus
│   │   ├── stores/                 # Stores Svelte reactifs
│   │   │   ├── projects.ts         # Liste projets, alimente par event status_update
│   │   │   ├── recording.ts        # Statut pipeline reunion (event recording_status)
│   │   │   ├── system.ts           # Metriques systeme + historique CPU/mem (60 pts FIFO)
│   │   │   ├── toast.ts            # notify(message, kind) — feedback non bloquant (erreurs/succes)
│   │   │   └── ui.ts               # Navigation (activeView enum, selectedProject, activeTab, dashboardView, pendingTerminalId, pendingTerminalCommand)
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
│   │   │   │   ├── TerminalTab.svelte    # Multi-terminaux, sessions Claude (fixes accents NE PAS RETIRER)
│   │   │   │   ├── FilesTab.svelte       # Arbre lazy gitignore-aware + viewer Shiki
│   │   │   │   ├── GitTab.svelte         # Status + diff viewer (hunks colores)
│   │   │   │   ├── PluginsTab.svelte     # Marketplace agents par projet
│   │   │   │   └── SettingsTab.svelte    # Parametres projet + URLs + override prompt resume
│   │   │   ├── todos/
│   │   │   │   ├── TodoList.svelte       # CRUD + checkbox (use:reorderable + InlineEdit)
│   │   │   │   └── TodoText.svelte       # Texte d'une tache : PARTAGE par TodoList et TasksView
│   │   │   ├── urls/
│   │   │   │   └── UrlList.svelte        # CRUD liens rapides
│   │   │   ├── notes/
│   │   │   │   ├── NoteTree.svelte       # Arborescence (DnD local : move inter-dossiers)
│   │   │   │   ├── NoteEditor.svelte     # Editeur WYSIWYG (contenteditable + toolbar + autosave 1s)
│   │   │   │   └── ReadingToggle.svelte  # Bouton du mode lecture (en-tete de note + etat vide)
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
  - Badge hostname, systeme et version (`long_os_version`, pas le noyau), uptime
  - Jauges circulaires SVG (donuts) CPU + Memoire avec pourcentage, nombre de coeurs, modele CPU
  - Detail memoire (Processus, ZFS ARC, Cache, Partage, Buffers) lu depuis `/proc/meminfo` et
    `/proc/spl/kstat/zfs/arcstats` — **LINUX uniquement**, le panneau se masque ailleurs
  - Graphiques d'historique CPU et memoire (SVG polyline, 60 points FIFO a 3s d'intervalle)
  - Top 20 processus CPU et Top 20 processus memoire (tableau avec toggle)

### Sidebar

- Section **Terminaux** en haut (repliable, masquee si vide) : raccourcis vers toutes les sessions
  vivantes (nom + projet), clic = navigation directe vers la session (pendingTerminalId),
  clic droit = Renommer/Fermer. Logo Claude = un agent IA (claude, codex...) tourne dans la
  session (detection par le BINAIRE REEL du process, insensible a l'usurpation d'argv :
  `/proc/<pid>/exe` sous Linux, `Process::exe()` de sysinfo ailleurs), point gris =
  terminal normal. Alimentee par le store `terminals` (stores/terminals.ts) : recharge sur
  terminal_exit, apres creation/fermeture/renommage, et toutes les 5 s (suivi du flag llm).
- Boutons **« + Projet »** et **« + Dossier »** en toutes lettres (une icone seule n'etait pas
  comprise — retour utilisateur 2026-08-14, ne pas revenir aux icones).
- Dossiers de projets **imbriques sans limite de profondeur** (issue #2, 2026-08-20) : rendu
  recursif, repliables (clic n'importe ou sur la ligne), renommables (double-clic sur le nom ou
  clic droit), supprimables par la **corbeille au survol** de l'en-tete — UNIQUEMENT s'ils sont
  VIDES, sous-dossiers compris, sinon un message dit ce qui reste a deplacer (pas de detachement
  silencieux vers la racine, et surtout pas d'une branche repliee donc invisible).
  - Creer un sous-dossier : bouton `+▸` au survol de l'en-tete, ou clic droit -> Nouveau
    sous-dossier. Le parent est DEPLIE automatiquement, sinon le champ de saisie apparaitrait
    dans une branche fermee.
  - Glisser un dossier : l'en-tete visee a TROIS zones — quart haut / quart bas = reordonner
    dans la fratrie (trait accent), moitie centrale = ranger DEDANS (cadre accent + teinte).
    Une cible impossible (soi-meme ou un de ses descendants) s'affiche en pointilles rouges
    pendant le survol et explique le refus au lacher. La zone du bas (racine) sort un dossier
    de son parent, avec un libelle qui l'annonce pendant le glisser.
  - Le compteur d'un dossier compte les projets de TOUTE sa branche : un compte direct
    afficherait « 0 » sur un dossier replie qui contient des projets deux niveaux plus bas.
  - Un dossier vide affiche quoi en faire au lieu de rester un trou.
- Liste de tous les projets avec :
  - Dot de couleur selon l'etat (running/starting/stopping/error/stopped)
  - Nom du projet
  - Description (si presente)
  - Nombre de containers (si > 0)
  - Etat textuel
- Clic pour naviguer vers le projet, **double-clic ou clic droit -> Renommer** (meme paire de
  gestes que les dossiers et les terminaux) et infobulle qui l'annonce. Le menu contextuel d'un
  projet ne contient QUE Renommer : la suppression cascade sur les notes, taches, URLs et
  enregistrements du projet, son chemin confirme vit dans Parametres -> Projets, et on ne la met
  pas a un clic droit de la ligne qu'on vient de renommer.
- La ligne projet est rendue par UN SEUL snippet (`ligneProjet`) utilise par la racine et par
  les dossiers : le balisage etait recopie deux fois et toute retouche devait etre faite deux
  fois.

### Vue projet (7 onglets)

En-tete : nom renommable (double-clic sur le titre, ou le **crayon ✎** qui apparait au survol —
c'est lui le vrai `<button>`, le titre n'est que la cible confortable), description, bouton ⏺
Enregistrer (reunions), liens rapides.

**Le renommage d'un projet passe par `renommerProjet` (stores/projects.ts), pas par
`renameProject` en direct** : le nom est UNIQUE en base, il faut donc controler la collision
AVANT (message traduit) sinon SQLite remonte « UNIQUE constraint failed: projects.name » jusqu'au
toast ; et la memoire d'onglet est indexee par nom, elle doit suivre le renommage. La fonction ne
reselectionne le projet que s'il etait DEJA affiche — renommer depuis la barre laterale ne doit
pas emmener ailleurs.

- **Workspace** : Notes a gauche (flex: 2, arborescence + editeur WYSIWYG) + Todos a droite
  (flex: 1). Le TEXTE d'une tache est rendu par `todos/TodoText.svelte`, partage avec le
  tableau de bord : clic simple = edition inline, Ctrl+clic sur une adresse = ouverture
  (navigateur ou client mail). Ne pas remettre un balisage de texte de tache ailleurs. **Mode lecture** (bouton ▸◂ Lecture dans l'en-tete de la note, store `readingMode`) :
  les DEUX colonnes se replient d'un coup et le compte rendu prend toute la zone, borne a 70rem
  et centre. Echap en sort aussi
- **Docker** : start/stop/restart, dependances ("depend de" / "requis par"), tableau des conteneurs
- **Terminal** : multi-terminaux persistants (voir section dediee plus bas)
- **Fichiers** : arbre lazy gitignore-aware + viewer Shiki (numeros de ligne, stats, copie du
  chemin, wrap) + Ctrl+clic "aller a la definition" (LSP) + edition avec coloration (✎ / Ctrl+S)
  + recherche dans le fichier (Ctrl+F, occurrences surlignees par <mark> sur les noeuds texte)
  + recherche globale projet (Ctrl+Maj+F, noms + contenu, commande search_project)
- **Git** : gestion complete (stage/unstage, commit, push, branches) + diff colore
- **Plugins** : marketplace d'agents par projet
- **Parametres** : formulaire projet (chemin, compose, description, dependances), URLs, override du prompt de resume

### Editeur de notes (WYSIWYG)

- Arborescence de dossiers et fichiers Markdown a gauche
- Editeur contenteditable a droite : affiche le Markdown rendu (via `marked`) et permet l'edition directe
- Toolbar : Gras, Italique, Barre, H1/H2/H3, Listes, Citation, Code, Lien
- Conversion HTML -> Markdown via `turndown` a la sauvegarde
- Auto-save avec debounce 1s
- **Mode lecture** (`notes/ReadingToggle.svelte`, store `readingMode` de `stores/ui.ts`) : replie
  l'arborescence ET la colonne des taches, borne le texte a 70rem et le centre. Les deux colonnes
  sont MASQUEES (`display: none`), pas demontees : elles gardent leur defilement, une tache a
  moitie saisie survit a l'aller-retour et il n'y a aucun rechargement. Le bouton est
  dans l'en-tete de la note — le seul endroit toujours visible dans les deux etats — et se
  REPETE dans l'etat vide « aucune note ouverte », sinon un mode lecture active sans note serait
  un cul-de-sac. La bascule conserve le paragraphe en haut de la vue et le curseur de saisie
  (`basculerLecture` dans NoteEditor)

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

**UNE EXCEPTION, et une seule : un changement qui refait le coeur de l'outil.** Quand une
release remplace un mecanisme central — le remplacement de tmux par notre service de
terminaux, en aout 2026 — Jimmy veut l'essayer en local AVANT publication. Il l'a demande
explicitement le 2026-08-20. Raison : ces releases partent sur les machines d'autres
personnes, et un terminal casse rend l'application inutilisable, pas juste genante.
Concretement : construire le binaire (`npx tauri build --no-bundle`), lui donner le chemin,
et attendre son accord. Ca ne s'applique PAS aux corrections ordinaires, meme nombreuses :
onze versions sont parties le meme jour sans qu'il ait rien a valider, et c'est bien ainsi.

**Ne JAMAIS demander l'autorisation de releaser.** Un lot fini et verifie part, point. Jimmy l'a
demande explicitement le 2026-08-13 (« c'est relou que je doive te demander tout le temps »).

**Jimmy ne lance PAS de build local** — il teste depuis la version publiee (« je test rien moi,
je prend les maj et je test apres »). Consequences :
- Ne jamais lui demander de relancer `target/release/cockpit` ni de reproduire sur un binaire local.
- Une instrumentation de diagnostic doit etre PUBLIEE pour qu'il puisse l'exercer. C'est
  acceptable : elle n'ecrit que dans `/tmp/cockpit-debug.log`. La retirer des la cause tranchee.
- Le build local reste obligatoire pour l'IA (4e point de la definition de "fini"), simplement
  il ne sert pas de moyen de test pour lui.
Deux garde-fous, qui n'exigent aucune question : ne pas publier si les 6 points de la definition
de "fini" ne passent pas, et annoncer apres coup ce qui est parti et en quelle version.
Seule exception encore soumise a accord : reecrire un historique deja pousse.

**Un seul workflow, declenche uniquement par un tag `v*`.** Il n'y a volontairement PAS de CI
sur les pushes de `main` : `release.yml` lance lui-meme `npm run check` et `cargo test` avant de
builder, donc un commit casse ne peut de toute facon pas etre publie. Une CI de branche ne faisait
que refaire ce travail en double. Ne pas la reintroduire — c'est une decision de Jimmy, prise deux
fois. La verification avant un tag se fait en local (les 6 points de la definition de "fini").

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
- **La release est publiee en BROUILLON, un job `publier` la rend visible a la fin.** Les deux
  jobs de la matrice publient sur la MEME release et tauri-action fusionne `latest.json`
  plateforme par plateforme : le premier a finir exposait donc un `latest.json` sans l'autre
  plateforme, et l'updater de cet OS affichait « None of the fallback platforms [...] were
  found ». Constate le 2026-08-19 sur la v0.31.0. Le brouillon n'est pas servi par
  `releases/latest`, donc plus personne ne voit un fichier incomplet. Le job `publier`
  (`if: always()`, donc il tourne meme si macOS a echoue) verifie la presence de l'AppImage ET
  d'une entree `linux-*` dans `latest.json` avant de lever le brouillon — sinon il echoue et la
  release reste invisible. Il passe par l'API et non par `gh release view` : **un brouillon
  n'est pas accessible par son tag** (l'API rend 404), il faut le chercher dans la liste
  (tauri-action fait pareil pour retrouver le brouillon de l'autre job).
- **URGENCE : une release deja publiee mais incomplete se repare sans rien republier** —
  `gh release edit vX.Y.Z --prerelease --latest=false`. `releases/latest` exclut les
  preversions, donc l'endpoint retombe aussitot sur la derniere version COMPLETE et les
  utilisateurs cessent de voir l'erreur (~1 min de propagation CDN). C'est le premier geste a
  faire, avant meme de diagnostiquer.
- **Le job Linux peut se figer sur `apt-get`** : `unattended-upgrades` tient le verrou dpkg au
  demarrage du runner et apt attend indefiniment. Sans plafond, GitHub laisse courir **six
  heures** avant de tuer le job (v0.31.0 : fige de 15h19 a 21h20, aucune AppImage publiee,
  macOS termine seul -> tous les utilisateurs Linux casses). D'ou `timeout-minutes` sur le job
  (120) et sur l'etape apt (15), l'arret du service, `-o DPkg::Lock::Timeout=120` et trois
  essais. Ne pas retirer ces plafonds : c'est ce qui transforme un blocage en echec rapide.
- **`Resource not accessible by integration` a la CREATION de la release** : le jeton n'a pas
  le droit d'ecrire. **Verifier d'abord le reglage du depot** :
  `gh api repos/jguevel-tech/cockpit/actions/permissions/workflow` doit rendre
  `default_workflow_permissions: "write"`. Constate le 2026-08-20 sur la v0.33.0 : il etait
  passe en `read`, les builds reussissaient (AppImage et app.tar.gz signes) et seule la
  creation de release echouait.
  **UNE RELANCE NE SERT A RIEN dans ce cas** : les permissions du jeton sont fixees a la
  CREATION du run, donc `gh run rerun` rejoue avec l'ancien jeton, meme apres avoir corrige le
  reglage. Il faut un run NEUF, c'est-a-dire un nouveau tag. Deux relances ont ete perdues a
  croire au diagnostic « incident transitoire » ci-dessous.
  Ce diagnostic-la existe aussi (constate sur la v0.24.0 ; la v0.25.0 a reussi avec le meme
  token juste apres) mais il ne doit venir qu'en SECOND, apres avoir verifie le reglage.
  Quand le tag est perdu : laisser le tag orphelin, et **remettre ses notes sous
  `[Unreleased]`** avant de tagger la version suivante — sinon le correctif est livre sans
  figurer dans les notes que le logiciel affiche.
  REGLE : si une version PLUS RECENTE est deja publiee, NE JAMAIS rerun
  le vieux tag — sa release deviendrait "latest" (GitHub classe par date de creation) et
  servirait un latest.json plus vieux aux utilisateurs. Le contenu du tag rate part de toute
  facon dans la version suivante (le code est cumulatif) ; on laisse le tag orphelin.
- **Windows : NSIS et pas MSI** (ajoute a la matrice le 2026-08-21). MSI passe par WiX,
  s'installe PAR MACHINE donc reclame l'elevation a chaque mise a jour, et double les fichiers
  a signer ; NSIS s'installe par utilisateur et c'est le format que l'updater Tauri v2 sait
  remplacer seul. `plugins.updater.windows.installMode = "passive"` est ecrit explicitement
  dans `tauri.conf.json` — c'est deja la valeur par defaut du plugin, mais « quiet » ne relance
  pas l'application proprement et on ne veut pas dependre d'un defaut qui peut changer.
  Rien a installer sur le runner : WebView2 est deja present sur les images `windows-*`.
  **Le shell par defaut du runner Windows est PowerShell** : l'etape qui extrait les notes du
  CHANGELOG porte donc un `shell: bash` explicite (`${VAR#prefixe}` et les heredocs n'existent
  pas en PowerShell ; Git Bash est installe sur ces images).
- **`PLATEFORMES_ATTENDUES` du job `publier` doit lister TOUTES les plateformes de la
  matrice** (`linux- darwin- windows-`). C'est ce qui fait echouer le run quand une plateforme
  manque dans `latest.json` : sans ca, son absence redevient silencieuse et ses utilisateurs
  sont coupes des mises a jour sans que personne ne soit averti (bug de la v0.32.0).
- **macOS : les bundles du job mac doivent etre `app,dmg`, pas `dmg` seul** — l'artefact de
  mise a jour (.app.tar.gz + .sig) est genere depuis le bundle `app` ; sans lui, latest.json
  n'a pas d'entree darwin et l'updater mac est muet (constate sur la v0.25.0).
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
de trackpad ; molette nue laissee au defilement d'xterm).

Implemente par le **zoom natif du webview** (`set_webview_zoom` -> `WebviewWindow::set_zoom`), donc
tout est mis a l'echelle : typo, paddings, bordures ET le rendu xterm. Un `html { font-size }` variable
a ete ecarte : ~423 tailles en px (paddings, `--header-height`, boutons 32x32) ne suivraient pas les
809 `rem` et le texte finirait par deborder de ses boites.

Rien a faire cote terminaux : zoomer change les dimensions en px CSS du conteneur, donc le
`ResizeObserver` de TerminalTab (debounce 80 ms) refit et renvoie la nouvelle taille au service.

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
via `color-mix` (components.css). **AUCUN `backdrop-filter` sous du contenu** : le WebKitGTK de
Tauri inclut le contenu de l'element dans le backdrop qu'il floute (violation de spec, prouvee au
banc le 2026-08-15 sur 4 variantes) — chaque panneau affichait une copie floutee de son propre
texte, un halo epousant chaque lettre. La lisibilite repose sur : opacite des surfaces
(--surface-alpha), voile (--wallpaper-dim), et flou de L'IMAGE elle-meme (--wallpaper-blur,
filter sur .wallpaper, sans halo). Seuls les VOILES de modals gardent un backdrop-filter (flou
uniforme voulu + il empeche WebKitGTK de tuer les filtres, bug du 2026-08-14). Le **TERMINAL
reste opaque** : xterm dessine dans un canvas WebGL, le rendre translucide est un terrain a
regressions, et un terminal doit rester lisible avant d'etre joli.

Le bouton ◑ du Header (`toggleBase`) bascule sombre <-> clair ; les palettes de couleur se
choisissent dans Parametres -> Apparence. Reglages persistes en localStorage sous la cle
`cockpit-appearance` (migration automatique depuis l'ancienne cle `cockpit-theme`).

## Base de donnees

SQLite stockee dans `~/.local/share/com.cockpit.dev/data.db` (ou via `COCKPIT_DB` env var).

13 tables :

| Table | Contenu |
|-------|---------|
| `projects` | Projets Docker (name, path, compose_file, description, depends_on JSON, position, folder_id, summary_prompt) |
| `project_folders` | Dossiers de projets hierarchiques (id, name, position, parent_id nullable — imbrication sans limite depuis le 2026-08-20). `position` est LOCALE A LA FRATRIE |
| `notes` | Note simple par projet (une seule par projet) |
| `note_folders` | Dossiers de notes hierarchiques (parent_id nullable, cascade delete) |
| `note_files` | Fichiers de notes dans les dossiers (content Markdown, cascade delete) |
| `todos` | Taches par projet (text, done, position) |
| `urls` | Liens rapides par projet (label, url, position) |
| `settings` | Cle/valeur globales (openai_api_key, summary_prompt, summary_model) |
| `recordings` | Enregistrements de reunions (project, started_at, duration_secs, state, error, dir) |
| `terminals` | Terminaux persistants (project, name) — l'etat vivant, lui, appartient au service |
| `command_history` | Historique de commandes (command PRIMARY KEY, project, ts — upsert) |
| `claude_session_names` | Noms personnalises des sessions Claude Code (session_id, name) |
| `project_commands` | Commandes rapides par projet (label, command, position) |

La colonne `summary_prompt` (nullable) sur `projects` surcharge le prompt global de resume par projet.

Le champ `depends_on` dans `projects` est un JSON array stocke comme TEXT (ex: `["docker-devbox"]`).

Index : idx_notes_project, idx_note_folders_project, idx_note_files_project, idx_note_files_folder,
idx_todos_project, idx_urls_project, idx_projects_folder, idx_project_folders_parent,
idx_recordings_project, idx_terminals_project, idx_command_history_ts.

Migrations automatiques au demarrage via `storage/db.rs`. Mode WAL + foreign keys actives.

## Commandes Tauri

### Docker
- `list_projects`, `start_project`, `stop_project`, `restart_project`
- `container_logs` (500 dernieres lignes : docker logs ecrit sur DEUX flux, fusion par
  timestamps RFC3339 puis prefixe retire — ContainerLogsModal, suivi 2 s)

### Todos
- `get_todos`, `create_todo`, `update_todo`, `delete_todo`, `reorder_todos`, `move_todo`, `get_pending_todos`
- `set_todo_due` (echeance ISO nullable ; alertes via stores/todoAlerts.ts -> cloche)

### Notes
- `get_note`, `save_note`, `get_note_tree`
- `create_note_folder`, `rename_note_folder`, `delete_note_folder`
- `create_note_file`, `get_note_file`, `save_note_file`, `rename_note_file`, `delete_note_file`
- `reorder_note_folders`, `reorder_note_files`, `move_note_file`

### URLs
- `get_urls`, `create_url`, `update_url`, `delete_url`
- `check_urls` (statut up/down : HEAD avec repli GET, module urlhealth.rs — pastilles dans
  ProjectDetail et UrlList, re-verif 60 s)

### Commandes rapides par projet
- `get_project_commands`, `create_project_command`, `update_project_command`,
  `delete_project_command`, `reorder_project_commands` (table project_commands, DANS
  PROJECT_SCOPED_TABLES avec test qui verrouille la regle ; bouton ▶ Cmd de ProjectDetail
  + entrees de la palette Ctrl+K. Execution = depot dans le magasin `pendingTerminalCommand`,
  l'onglet Terminal cree la session par `addTerminal` a la taille mesuree — l'appelant
  n'appelle PAS `create_terminal` lui-meme)


### Project Folders
- `get_project_folders` (tous les dossiers a plat, `ORDER BY position, name` — l'arbre est
  reconstruit cote frontend depuis `parent_id`)
- `create_project_folder(name, parent_id)` — `parent_id` a None = premier niveau
- `rename_project_folder`, `move_project_folder(id, parent_id)` (refuse les BOUCLES : un dossier
  ne peut pas devenir son propre descendant, message remonte),
  `delete_project_folder` (refuse un dossier NON VIDE : sous-dossiers ou projets),
  `reorder_project_folders(ids)` — les ids d'UNE SEULE fratrie, les positions sont locales au
  parent, `move_project_to_folder` (un projet)

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
- `create_terminal` (init_command tapee par le service des que le shell repond), `write_terminal`,
  `resize_terminal`, `close_terminal`
- `attach_terminal` (SANS EFFET si le terminal est deja branche — voir la doctrine du pool dans
  Pieges connus ; ne rend rien, l'etat retrouve arrive par `terminal_output`), `rename_terminal`
- `list_terminals`, `list_all_terminals` (avec flag `llm` : un agent IA tourne dans la session)
- `set_clipboard` / `get_clipboard` (presse-papier systeme via arboard, instance gardee en vie ;
  `poser_presse_papier` est la meme chose appelable depuis le backend, pour l'OSC 52 d'un
  programme)
- `list_claude_sessions`, `rename_claude_session`
- `record_command`, `search_command_history` (historique fusionne DB Cockpit + ~/.zsh_history + ~/.bash_history)
- `terminal_search` (start/next/prev/cancel ; sous-chaine LITTERALE, casse ignoree, lignes
  enroulees recollees). Rend `{total, index, ligne, colonne}` : le serveur n'a pas d'ecran a
  peindre, c'est le frontend qui defile et surligne (`registerDecoration`)
- `debug_log` (diagnostic : append dans /tmp/cockpit-debug.log)

### Explorateur de fichiers / Git
- `list_project_dir`, `read_project_file` (rend aussi la mtime), `write_project_file` (fichiers
  existants, racine verrouillee)
- `stat_project_file` : mtime + taille seules, sans lire le contenu. Rend `None` — pas une erreur —
  quand le fichier a disparu. Sert au suivi du fichier ouvert dans l'onglet Fichiers (stat toutes
  les 2 s + controle au retour de focus ; relecture UNIQUEMENT si mtime ou taille ont bouge)
- `search_project` (recherche globale : noms de dossiers/fichiers + contenu, gitignore-aware,
  insensible a la casse, bornee a 100 noms / 400 occurrences avec flag `truncated` ;
  `spawn_blocking` pour ne pas bloquer le runtime)
- `goto_definition` (LSP si serveur dispo pour le langage, sinon repli `find_symbol`)
- `git_status` (staged/unstaged par fichier, +/- via --numstat, ahead/behind), `git_diff_file`
- `git_stage`, `git_unstage`, `git_stage_all`, `git_unstage_all` (add / reset)
- `git_commit`, `git_push` (set_upstream auto si pas d'upstream)
- `git_pull` (--ff-only : JAMAIS de merge surprise depuis un bouton), `git_log` (format
  %x1f + epoch), `git_commit_diff` (git show decoupe multi-fichiers, hash valide hexadecimal)
- `git_branches`, `git_checkout_branch`, `git_create_branch`, `git_delete_branch` (force en fallback)
- run_git_strict pour les operations (code != 0 = erreur remontee) vs run_git (tolere code 1 pour diff)

### Fichiers (gestion et apercu)
- `create_project_file`, `create_project_dir`, `rename_project_entry` (feuille validee,
  refus d'ecraser), `trash_project_entry` (CORBEILLE systeme via crate trash, jamais rm)
- `read_project_image` (data URL, 10 Mo max, apercu damier dans FilesTab)

### Migration / sauvegarde
- `import_database`, `get_db_path`
- `backup_database` (API backup SQLite — coherente en WAL, bouton Parametres -> General)

## Events Tauri (backend -> frontend)

- `status_update` : emis toutes les 5s par le monitor apres refresh des statuts Docker
- `recording_status` : emis a chaque changement d'etat du pipeline reunion (recording_id,
  project, state, error, started_at, lost_track, mute_track — les deux derniers sont des
  CODES, traduits par l'interface)
- `terminal_output` : octets du PTY encodes base64 ({id, data}), consommes par xterm.js
- `terminal_exit` : id de la session dont le shell s'est termine
- `claude_login_output` / `claude_login_done` : sortie et fin du flow `claude setup-token`

## Onglets Terminal / Fichiers / Git (vue projet)

**LES TERMINAUX SONT A NOUS, plus de tmux** (chantier fini le 2026-08-21,
`docs/portabilite/plan-terminaux.md`). Trois etages, et chacun ignore les autres :

- `terminal/interface.rs` : le trait `Terminaux`, ce que Cockpit demande a un serveur de
  terminaux (preparer, creer, ecrire, redimensionner, fermer, attacher, renommer, lister,
  chercher). `AppState.terminals` est un `Box<dyn Terminaux>`, donc **aucune commande Tauri ne
  connait le serveur**.
- `terminal/service/` : LE serveur. Le meme binaire lance avec `--service-terminaux <socket>`
  tient les shells dans ses propres PTY et leur ecran dans `terminal/ecran/`. Il survit a la
  fermeture de l'application (double fork + setsid), et n'ecrit RIEN sur disque : il meurt avec
  la machine.
- `terminal/adaptateur.rs` : l'implementation du trait par-dessus le socket. C'est elle que
  `terminal/mod.rs` choisit en une ligne (`terminaux()`), elle qui lance le service au demarrage
  s'il ne tourne pas deja, et elle qui traduit ses poussees en evenements Tauri.

Ce qu'il faut en savoir avant d'y toucher :

- **Qui detient quoi** : le service tient l'etat VIVANT (sessions, taille, ecran, agent qui
  tourne), SQLite garde le NOM d'onglet et le PROJET — ils doivent survivre a un redemarrage de
  la machine, ce que le service ne fait pas. D'ou deux consequences : **l'identifiant d'un
  terminal est fourni par l'application** a la creation (le rowid SQLite), et `renommer` ne
  traverse PAS le socket (deux verites pour une meme chaine, c'est la garantie qu'elles
  divergent).
- **Le service ne redessine JAMAIS un terminal deja branche.** `attacher` est un no-op quand le
  terminal l'est deja (`TerminauxService.attaches`) : le frontend l'appelle a chaque retour sur
  un onglet, et re-brancher demanderait un redessin complet — clignotement, et retour en bas de
  l'historique a chaque changement d'onglet.
- **Le flux brut EST transmis, mais en gros lots.** Ce qui est petit part tel quel et tout de
  suite (l'echo d'une touche) ; le reste attend `FENETRE_RAFALE` (8 ms) pour partir groupe.
  C'est ce qui remplit le tampon de defilement d'xterm, donc ce qui fait marcher la molette sans
  rien demander au service. Un lot de plus de 256 Ko (32 Mo/s, que personne ne lit) est REMPLACE
  par un redessin, et l'historique est renvoye entier des que le calme revient.
- **Un redessin porte l'ecran ET les 10 000 lignes d'historique.** Il commence par une remise a
  plat (RIS) qui VIDE le tampon de defilement du terminal d'arrivee : sans l'historique, chaque
  attache ferait perdre ce que la molette remontait. C'est aussi ce qui garde xterm en miroir
  exact de la grille du service — la recherche rend des positions dans cette grille.
- **Poignee de main** : le SERVICE parle en premier, dix octets de forme figee (`CKPTERM\0` +
  version sur 2 octets). Le client sait donc dire « ce service est plus ancien que moi » avec les
  deux numeros, au lieu d'echouer sur un message incomprehensible. Tout changement de forme d'un
  message = incrementer `protocole::VERSION`.
- **Socket** : `$XDG_RUNTIME_DIR/cockpit/terminaux.sock` (repli `<temp>/cockpit-<uid>/`), dans un
  dossier cree en 0700, et le service comme le client verifient l'euid du pair. Surchargeable par
  `COCKPIT_TERMINAUX_SOCKET`. **Une base choisie a la main (`COCKPIT_DB`) obtient AUTOMATIQUEMENT
  son propre socket** (`terminaux-<empreinte>.sock`, `adaptateur::chemin_socket`) : sans ca, la
  reconciliation du demarrage verrait les terminaux de l'installation normale comme des sessions
  orphelines et les tuerait.
- **Historique borne en CELLULES, pas en lignes** (`serveur::CELLULES_D_HISTORIQUE`) : 10 000
  lignes a 80 colonnes, moins au-dela. Mesures en release : 19,5 Mo par session pleine a 80
  colonnes, 23,1 Mo a 240 — contre 57,1 Mo a 240 sans ce plafond.
- **Sous AppImage, le service est relance depuis `$APPIMAGE`, pas depuis `current_exe()`**
  (`lancement::binaire_a_relancer`) : le montage `/tmp/.mount_*` disparait a la fermeture de
  l'application, et le service doit lui survivre.

- **Terminal** : multi-terminaux par projet, renommables (double-clic sur l'onglet, clic droit dans
  la sidebar), PERSISTANTS : chaque terminal est une session du service (`terminal/service/`), qui
  survit a la fermeture de l'application. Metadonnees en DB (table `terminals`) ; au demarrage,
  `preparer` RECONCILIE la base et le service (ligne sans session -> supprimee, session sans ligne
  -> fermee, elle ne tournait pour personne). Ecritures/resizes serialises cote frontend
  (file par terminal), PTY cree a la taille mesuree. **`addTerminal` (TerminalTab) est le SEUL
  endroit qui cree une session** : une commande venue d'ailleurs (▶ Cmd, shell de conteneur,
  palette Ctrl+K) arrive par le magasin `pendingTerminalCommand` et c'est l'onglet qui la lance,
  parce que lui seul mesure son conteneur (voir « TUI lancee a la creation » dans Pieges connus).
  Theme suit dark/light. RENDU : addon WebGL + police mono explicite (DejaVu Sans Mono...) — le
  renderer DOM d'xterm avec "monospace" generique derive visuellement sur les glyphes accentues.
  `allowProposedApi: true` est OBLIGATOIRE : le surlignage de la recherche passe par
  `registerMarker`/`registerDecoration`, refuses par une exception sans ce drapeau.
- **Copier/Coller** : la selection appartient a XTERM (glisser la souris), elle reste affichee au
  relachement -> **Ctrl+C copie quand il y a une selection, interrompt sinon** (handler de touche,
  `attachCustomKeyEventHandler` — pas une surcouche sur `onData`), ou clic droit -> Copier. Un
  « Copier » sans selection le DIT. La copie va au presse-papier systeme par `set_clipboard`
  (arboard, instance gardee en vie sinon le presse-papier X11 meurt avec elle). Un programme qui
  demande la copie par OSC 52 est servi deux fois sans dommage : l'emulateur du service la remonte
  (`Pousse::PressePapier`) et le handler OSC d'xterm la voit passer dans le flux.
- **Collage : UN SEUL chemin, `pasteClipboard()`.** Clic droit -> « Coller » et clic molette
  appellent la meme fonction, donc collent la meme chose (le presse-papier systeme). Les deux
  autre candidat est eteint expres : le collage natif du WebView (annule dans `createXterm`).
  Voir « DOUBLE
  COLLAGE » dans les Pieges connus avant d'y toucher : la facon d'annuler le collage natif n'est
  pas celle qu'on croit.
- **Liens** : addon web-links, Ctrl+clic ouvre l'URL (http/https) dans le navigateur via open_url.
- **Detection agents IA** : logo Claude dans la sidebar/dashboard quand un CLI LLM tourne dans la
  session (claude, codex, gemini, aider... — constante COMMANDES_LLM dans terminal/agents_llm.rs).
  La racine est le PID du shell, que le service connait puisqu'il l'a lance : on descend son arbre
  de process (les CLIs node se cachent sous un enfant), point gris sinon.
  Store terminals rafraichi toutes les 5 s. La descente se fait par
  `/proc/<pid>/task/<tid>/children`, PAS par un `ps -e` global (voir Pieges connus) : toutes
  les taches, pas seulement le thread principal, parce qu'un node fork depuis un thread de
  travail.
- **Sortie du PTY = 2 threads par session, lecteur puis emetteur**
  (terminal/service/session.rs, `lire_pty` et `emettre`) : le lecteur vide le PTY dans l'ecran et
  dans une file, l'emetteur decide si le lot part tel quel ou en redessin. Un lot part TOUT DE
  SUITE quand il a fallu l'attendre ET qu'il est petit (l'echo d'une touche) ; sinon il attend
  `FENETRE_RAFALE` (8 ms) pour partir groupe. NE PAS monter cette fenetre « pour mieux
  regrouper » : ce serait payer en latence percue. Voir « REGROUPEMENT DE LA SORTIE » dans les
  Pieges connus : la premiere version se declenchait sur le VOLUME en attente, et ne se
  declenchait donc jamais.
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
- **Taches** : todos en attente groupes par projet (drag & drop, edition inline, adresses
  ouvrables au Ctrl+clic — meme composant `todos/TodoText.svelte` que la colonne Todos).
  Echeance modifiable ICI AUSSI : badge cliquable, 📅 au survol pour en poser une. Le badge
  y etait un `<span>` inerte jusqu'au 2026-08-20 — on lisait l'echeance sur l'ecran de tri
  sans pouvoir la deplacer
- **Monitoring** : jauges CPU/memoire, historique, top processus (Snapshot / Live)
- **Terminaux** : tous les terminaux ouverts groupes par projet, clic = navigation directe
  vers la session (store `pendingTerminalId`, consomme par TerminalTab au montage ET a chaud)
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
1. Capture 2 pistes par `cpal`, DANS le processus (`recorder/capture.rs`, un thread par
   piste) : micro + son systeme. PCM brut s16 mono 16 kHz dans
   `<app_data>/recordings/rec_<id>/` — **c'est la frontiere du module**, `recorder/pcm.rs`
   ramene le format natif du materiel (48 kHz, 2 canaux, I32 sur une machine ordinaire) a
   ce format-la, et tout l'aval en depend a l'octet pres.
   - Le son systeme se capte sur un appareil de SORTIE : source `<sink>.monitor` sous
     Linux (host PulseAudio), drapeau de loopback WASAPI sous Windows, « process tap »
     sous macOS. Meme expression dans notre code, trois mecanismes systeme.
   - Repli AU CONSTAT, piste par piste : on essaie un appareil, on attend son premier lot
     (1 s), et on passe au suivant s'il ne vient pas. Une seule piste vivante suffit pour
     enregistrer (`lost_track`, code traduit par l'interface).
   - Une piste qui a tourne sans recevoir un seul echantillon NON NUL est signalee a
     l'arret (`mute_track` : "mic" / "system" / "both") et arrete le pipeline avant
     l'appel a Whisper. C'est le symptome d'un tap macOS sans autorisation, et il ne doit
     pas ressortir en « aucune parole detectee ».
2. Transcription OpenAI `whisper-1` par piste (chunks de 10 min < 25 Mo, `verbose_json`, langue fr, filtre silence + no_speech_prob)
3. Fusion chronologique en dialogue "Moi" (micro) / "Eux" (son systeme)
4. Resume via chat completions (modele et prompt systeme configurables dans Parametres globaux, override par projet)
5. Note auto-creee dans le dossier "Réunions" du projet : `Réunion du JJ/MM/AAAA à HHhMM`

Audio supprime apres succes, conserve en cas d'echec (bouton retry dans l'en-tete projet).
Un seul enregistrement a la fois. Cle API importee au premier lancement depuis `<app_data>/secrets.json` si presente.

## Navigation frontend

Pas de routeur. Un seul enum dans le store `ui.ts` :
- `activeView: "dashboard" | "project" | "settings" | "system" | "docs"` — MainPanel switch dessus
- `selectProject(name)` pose `activeView = "project"` (+ reset onglet), `openView(v)` pour le reste
- Ajouter une vue top-niveau = etendre le type + un case dans MainPanel (rien d'autre)
- Onglets projet : map `tabs` dans ProjectDetail.svelte (id, label, component) — ajouter un onglet
  = 1 entree dans la map + le type `activeTab` dans ui.ts

Le `{#key $selectedProject}` dans MainPanel force le remount de ProjectDetail quand on change de projet.

## Stores reactifs

| Store | Fichier | Contenu |
|-------|---------|---------|
| `projects` | `stores/projects.ts` | Liste projets, reload sur event `status_update`. Porte aussi `renommerProjet()` : SEUL chemin de renommage (controle du nom deja pris, memoire d'onglet, reselection) |
| `systemMetrics` | `stores/system.ts` | Metriques systeme courantes |
| `cpuHistory` | `stores/system.ts` | Historique CPU (60 points FIFO) |
| `memoryHistory` | `stores/system.ts` | Historique memoire (60 points FIFO) |
| `activeView` | `stores/ui.ts` | Vue top-niveau (dashboard/project/settings/system/docs) — plus de vue `agents`, elle est encastree dans les parametres |
| `selectedProject` | `stores/ui.ts` | Projet selectionne (utilise quand activeView === "project") |
| `activeTab` | `stores/ui.ts` | Onglet actif (workspace/docker/terminal/files/git/plugins/settings). MEMORISE PAR PROJET (map en memoire) : revenir sur un projet retrouve son onglet. Une navigation intentionnelle vers un onglet precis reste prioritaire et devient la nouvelle memoire |
| `dashboardView` | `stores/ui.ts` | Sous-vue du tableau de bord (tasks/monitoring/terminals/containers) |
| `pendingTerminalId` | `stores/ui.ts` | Session EXISTANTE a ouvrir. Consomme par `honorerDemande` (TerminalTab) au montage ET a chaud, quel que soit le producteur (barre laterale, tableau de bord, palette). Il RECHARGE la liste des sessions avant de conclure, parce que la cible peut venir d'etre creee. Toujours remis a null : ouverte, ou signalee disparue |
| `pendingTerminalCommand` | `stores/ui.ts` | Commande a lancer dans un NOUVEAU terminal du projet (`{ project, command }`) : bouton ▶ Cmd, shell d'un conteneur, commande rapide de la palette. Consomme par `honorerCommande` (TerminalTab) au montage ET a chaud, qui appelle `addTerminal(commande)` — c'est LUI qui cree la session, a la taille MESUREE du conteneur. Toujours remis a null, y compris quand la commande visait un autre projet (message a l'appui). NE PAS remettre un `create_terminal` chez l'appelant : voir « TUI lancee a la creation » dans les Pieges connus |
| `readingMode` | `stores/ui.ts` | Mode lecture de l'onglet Workspace (replie notes + taches), persiste localStorage `cockpit-notes-reading` |
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
- **Memoire** : total, used, available, swap + un detail **Linux uniquement** (`detail:
  Option<MemoryDetail>`, `None` ailleurs et `None` si `/proc/meminfo` est illisible), via
  `/proc/meminfo` :
  - `cached` : pages cache disque
  - `buffers` : buffers kernel
  - `shmem` : memoire partagee
  - `s_reclaimable` : memoire reclamable (slab)
  - `zfs_arc` : cache ZFS (lu depuis `/proc/spl/kstat/zfs/arcstats`, 0 si absent)
- **Disques** : ce que `sysinfo` juge local et reel. **Plus aucun filtre maison** : les six
  points de montage Unix qui etaient ecrits en dur ne matchaient RIEN sous Windows (`C:\`) et
  laissaient tomber `/System/Volumes/Data` sous macOS. sysinfo ecarte deja les pseudo-systemes
  de fichiers et les montages snap sous Linux, les instantanes APFS et les volumes reseau sous
  macOS, et ne garde que `DRIVE_FIXED`/`DRIVE_REMOVABLE` sous Windows. Consequence voulue : un
  disque monte sur `/mnt/data` apparait enfin, et la liste peut etre plus longue qu'avant.
  **Ne pas dedupliquer** deux volumes qui se ressemblent (APFS, ZFS) : c'est de l'heuristique
  qui se trompera, et l'ecran affiche une carte par disque sans jamais additionner.
- **Processus** : top 20 CPU + top 20 memoire groupes par nom
- **Arret d'un processus** : le signal le plus doux que le systeme accepte. `Signal::Term`
  sous Unix, `Signal::Kill` sous Windows — `SystemMetrics.kill_is_forced` le dit au frontend,
  qui affiche « Forcer l'arret » au lieu de « Arreter ». Voir « Un signal POSIX compile sous
  Windows » dans les Pieges connus.

## Vision future

- Connecteurs externes
- Le trait `Plugin` dans `plugin/mod.rs` prepare cette extensibilite

## Pieges connus (lecons apprises)

- **CPAL SOUS LINUX TIRE `alsa-sys` SANS CONDITION**, meme quand on ne se sert que du host
  PulseAudio. Le host ALSA n'est pas derriere une feature (`[target.'cfg(linux)']
  dependencies alsa, alsa-sys` dans le Cargo.toml de cpal 0.18.2). Consequences, decouvertes
  le 2026-08-21 en construisant la capture audio :
  - au BUILD, il faut `libasound2-dev` (sinon `Package alsa was not found in the pkg-config
    search path`). Ajoute a `release.yml` et aux dependances systeme de ce fichier. Sans les
  droits root : `apt-get download libasound2-dev`, `dpkg-deb -x` dans un prefixe, y copier
    `libasound.so.2*` du systeme (le `.so` du paquet dev est un lien qui pointe dessus), puis
    `PKG_CONFIG_PATH=<prefixe>/usr/lib/x86_64-linux-gnu/pkgconfig
    PKG_CONFIG_SYSROOT_DIR=<prefixe>` — c'est `SYSROOT_DIR` qui redirige le `-L` absolu du
    fichier `.pc` vers le prefixe ;
  - a l'EXECUTION, `libasound.so.2` est liee au binaire, donc EMBARQUEE dans l'AppImage par
    linuxdeploy. C'est la famille de pannes de la libwayland, en plus benin : rien
    d'exterieur ne se lie a NOTRE copie, et libasound ne charge ses greffons qu'a
    l'ouverture d'un peripherique ALSA — ce que le chemin PulseAudio ne fait jamais. Le
    repli ALSA du micro, lui, le fait : c'est la seule voie par laquelle ce risque peut se
    reveiller. **Ne pas ajouter la feature `pipewire` de cpal** : elle ajouterait
    libpipewire EN PLUS de libasound, sans rien apporter (le host PulseAudio couvre les
    machines PipeWire par `pipewire-pulse`).
- **`Device::id()` et `Display` ne rendent PAS la meme chose dans cpal**, et c'est
  l'identifiant qui porte la convention `.monitor`. `Display` (donc `to_string()`) rend la
  DESCRIPTION lisible du host PulseAudio (« Monitor of Built-in Audio Stéréo analogique »),
  `id().id()` rend le nom du serveur (`alsa_output.pci-0000_00_1f.3.analog-stereo.monitor`).
  Chercher le suffixe dans le mauvais des deux ne trouve jamais le monitor. Mesure du
  2026-08-21 : `default_output_device()` + `.monitor` trouve la source, un ton de 440 Hz a
  8 000 d'amplitude joue sur la sortie ressort dans `system.raw` a 440,0 Hz et 7 999.
- **UN APPAREIL DE SORTIE REFUSE `default_input_config()`.** C'est pourtant sur lui qu'on
  construit le flux d'entree pour capter le son systeme (loopback WASAPI, tap Core Audio) :
  WASAPI rend `UnsupportedOperation / Device does not support input`, et la forme du flux
  se demande alors par `default_output_config()` (c'est le format du MELANGE qui sort).
  D'ou `config_capture()` dans capture.rs, qui essaie les deux dans cet ordre. Le monitor
  PulseAudio, lui, est une SOURCE : c'est le premier appel qui repond.
- **Le materiel ne livre pas du `f32`.** Sur la machine du banc, micro et monitor livrent
  **48 000 Hz, 2 canaux, I32**. Un code qui ferait `data.as_slice::<f32>().unwrap()`
  paniquerait dans le rappel audio, et `unwrap_or_default()` rendrait une piste muette sans
  rien dire. `en_flottants` couvre les douze formats de `SampleFormat` (qui est
  `#[non_exhaustive]`), et un format inconnu est refuse a l'OUVERTURE — un rappel audio ne
  peut remonter aucune erreur.
- **`cpal::Stream` n'est pas `Send`** sur la plupart des systemes, alors que les handles de
  capture vivent dans l'etat partage de Tauri. D'ou un thread par piste, qui construit le
  flux, l'ecoute et le relache sans jamais le faire sortir de la (`recorder/capture.rs`).
  Le rappel audio ne fait que convertir en flottants et deposer dans une file : le melange
  des canaux, le reechantillonnage et l'ecriture disque sont de l'autre cote.
- **Mesurer un repliement sur TOUT le signal mesure les bords, pas le repliement.** Un ton
  de 10 kHz reechantillonne a 16 kHz ressortait a 0,127 d'amplitude alors que le filtre est
  parfait : les 400 premiers echantillons portent la reponse transitoire de l'attaque du
  signal de test (et autant a la coupure), et le regime etabli est a ZERO. Un test qui
  prend la crete globale conclut a un filtre defaillant et fait chercher un bug qui n'existe
  pas (2026-08-21, `pcm.rs`).
- **Verifier l'audio sans lancer l'application** : `cargo test --lib capture_reelle --
  --ignored --nocapture` enregistre 2 s par les vrais appareils et affiche, piste par
  piste, l'appareil retenu, le format natif, la taille obtenue, la crete et la frequence
  dominante. Un son connu qui joue a cote rend le resultat concluant : « des octets sont
  arrives » et « le son est juste » ne sont pas la meme chose. Le test est `#[ignore]`
  parce qu'il demande une carte son, qu'un runner n'a pas.
- **NE JAMAIS FAIRE SORTIR DE SON DES ENCEINTES POUR TESTER.** C'est la machine de
  quelqu'un, il travaille dessus, et un ton pur de dix secondes est insupportable. Le
  2026-08-21, un banc audio en a joue plusieurs fois pendant que Jimmy travaillait. La
  capture du son systeme se verifie SANS RIEN FAIRE ENTENDRE, par un sink nul :
  ```bash
  pactl load-module module-null-sink sink_name=banc \
    sink_properties=device.description=Banc   # rend aussi banc.monitor
  pactl set-default-sink banc                 # ce qui joue ne sort plus des enceintes
  pw-play /tmp/ton.wav &                      # inaudible : le sink n'a pas de materiel
  # ... capter ici, le monitor du sink par defaut est banc.monitor
  pactl set-default-sink <sink d'origine>     # A REMETTRE, sinon plus aucun son
  pactl unload-module module-null-sink
  ```
  Si un vrai signal audible est indispensable, c'est l'UTILISATEUR qui lance ce qu'il veut
  entendre — on ne le decide pas pour lui.
- **COMPILATION CROISEE WINDOWS : `rustup target add` ne suffit pas, il faut un compilateur C
  croise.** `cargo check --target x86_64-pc-windows-gnu` echoue d'abord sur
  `failed to find tool "x86_64-w64-mingw32-gcc"` — ce n'est pas notre code, c'est
  `libsqlite3-sys` (feature `bundled`) qui compile `sqlite3.c` POUR LA CIBLE, et `cargo check`
  execute les scripts de construction. Le message ne dit pas quelle crate le demande.
  - Avec les droits : `sudo apt-get install gcc-mingw-w64-x86-64-win32`.
  - Sans les droits, et ca marche : `apt-get download` des paquets mingw puis `dpkg-deb -x`
    dans un dossier a soi. Le pilote gcc de Debian est RELOCATABLE (il retrouve son `libexec`
    a partir de `argv[0]`), donc `<prefixe>/usr/bin/x86_64-w64-mingw32-gcc-13-win32` compile
    tel quel depuis n'importe ou. Poser `CC_x86_64_pc_windows_gnu` et
    `AR_x86_64_pc_windows_gnu` dessus, et ajouter son `bin` au `PATH` (l'assembleur et le
    lieur y sont cherches par nom court).
  - `gnu` et non `msvc` : la cible MSVC voudrait `cl.exe`, introuvable sur une machine Linux.
    Le binaire publie, lui, est construit par le runner `windows-latest` en MSVC — la cible
    croisee ne sert qu'a GARDER LE CODE COMPILABLE, pas a produire un binaire.
  - **Recette complete, verifiee le 2026-08-21 (13 s de check, 0 erreur)** :
    `apt-get download gcc-mingw-w64-x86-64-win32 gcc-mingw-w64-base binutils-mingw-w64-x86-64
    mingw-w64-x86-64-dev mingw-w64-common` (51 Mo), `dpkg-deb -x` chacun dans un prefixe a soi,
    puis `PATH=<prefixe>/usr/bin:$PATH`,
    `CC_x86_64_pc_windows_gnu=<prefixe>/usr/bin/x86_64-w64-mingw32-gcc-13-win32` et
    `AR_x86_64_pc_windows_gnu=<prefixe>/usr/bin/x86_64-w64-mingw32-ar`. **Ajouter aussi un
    lien `x86_64-w64-mingw32-gcc` (nom court) vers le binaire `-13-win32`** : `windres`
    l'appelle sous ce nom-la, par `sh`, pour pre-traiter le `resource.rc` de `tauri-winres`.
    Sans le lien, le script de construction de NOTRE crate panique sur
    « x86_64-w64-mingw32-windres: echec du pre-traitement » — et l'etape ne se declenche que
    quand `tauri.conf.json` a change (donc apres un bump de version), ce qui la fait passer
    pour une regression du portage. Sans ces variables,
    `ring` et `libsqlite3-sys` echouent AVANT que notre crate soit analysee — le check rend
    alors 2 erreurs qui n'ont rien a voir avec notre code, et on croit a tort que le portage
    est casse.
  - Verifier `--all-targets` : sans lui, le code des tests (`#[cfg(test)]`) n'est pas compile
    pour la cible et ses `use std::os::unix::...` passent inapercus.
  - **`--all-targets` prouve que les essais COMPILENT, jamais qu'ils PASSENT.** Cinq essais
    de `terminal/service/tests.rs` tapent dans le shell de la machine et se reperent dans sa
    sortie (`printf`, `cat`, `for i in $(seq 1 400)`). Sous Windows le shell est `%COMSPEC%`,
    soit `cmd.exe`, qui ne connait aucune des trois : ils auraient echoue sur le runner, donc
    aucun bundle Windows n'aurait ete produit — et le job `publier` aurait signale une
    plateforme manquante APRES avoir publie Linux et macOS. Ils portent donc `#[cfg(unix)]`,
    avec sur place ce qui reste couvert et ce qui ne l'est pas. Deux consequences a ne pas
    oublier :
    - **garder un essai laisse ses outils inutilises** (imports, structs de banc, `impl
      Drop`, fonctions d'aide) : chacun devient un avertissement sur la cible ou l'essai
      n'existe plus, et le projet exige 0 avertissement. Il faut garder l'outillage AVEC
      l'essai — sept avertissements et deux erreurs sont sortis de cet oubli, dont un `impl
      Drop for` orphelin que le compilateur signale comme un type introuvable.
    - **ne PAS inventer d'equivalent `cmd.exe` sans machine pour l'essayer.** Un essai vert
      qu'on n'a jamais vu tourner ne prouve rien, et le marqueur guette ne doit surtout pas
      figurer dans la ligne TAPEE (le PTY en renvoie l'echo avant execution). Ce que le
      terminal fait vraiment dans un ConPTY se saura en lancant l'installeur Windows, pas
      en ecrivant un essai a l'aveugle.
  - **`cargo test` en local demande `libasound2-dev`** depuis que la capture audio passe par
    cpal (qui depend d'`alsa-sys` sans condition sous Linux) : sans lui, le build de
    `alsa-sys` echoue sur `The system library alsa was not found` et AUCUN essai ne tourne.
    Sans droits administrateur, meme recette que mingw (`apt-get download`, `dpkg-deb -x`
    dans un prefixe) puis `PKG_CONFIG_PATH=<prefixe>/usr/lib/x86_64-linux-gnu/pkgconfig` et
    `PKG_CONFIG_SYSROOT_DIR=<prefixe>`.
- **WINDOWS COMPILE ET NE MARCHE PAS — ce que le premier vrai run a dit (v0.38.0,
  2026-08-21).** La compilation croisee rendait 0 erreur et 0 avertissement, et le runner
  `windows-latest` a fait tomber **treize** essais. Ce que ca prouve : `--all-targets`
  n'exerce RIEN, il compile. Les constats, du plus grave au moins :
  - **Ecrire dans le PTY echoue** : « The operation completed successfully. (os error 0) »
    sur `session.ecrire`, et « The handle is invalid. (os error 6) » a la creation d'une
    session. C'est le coeur du produit : sans ca, pas de terminal. Cause non trouvee, et
    elle ne se trouvera pas en lisant — il faut une machine.
  - **Un socket local n'est PAS un fichier sous Windows** : c'est un tuyau nomme, dans
    l'espace `\\.\pipe\`, absent du systeme de fichiers. `interprocess` refuse un chemin de
    fichier avec « not a named pipe path ». Le code de PRODUCTION le savait
    (`tuyau::chemin`), les ESSAIS non : ils fabriquaient un `.sock` dans un dossier
    temporaire. Corrige. A retenir : un helper d'essai qui construit un chemin est un
    endroit ou la portabilite se perd sans que rien ne le signale.
  - **Cinq essais tapent dans le shell** (`printf`, `cat`, `for i in $(seq …)`) : `cmd.exe`
    n'en connait aucun. Ils portent `#[cfg(unix)]`.
  - `le shell ne meurt pas` : la fin d'un process n'a pas la meme semantique.
  - Un `assert_eq!` de `workspace` tombe : separateurs de chemin.
  - **DENOUEMENT : les treize echecs venaient des ESSAIS, pas du produit.** Une fois les
    cinq essais a shell POSIX gardes et le tuyau nomme corrige, la suite entiere passe sur
    `windows-latest` — y compris les essais qui ecrivent dans le PTY et attendent la reponse
    d'un vrai shell, ceux dont l'echec avait fait conclure que le coeur etait casse. Windows
    est donc revenu dans la matrice de `release.yml` (et dans `PLATEFORMES_ATTENDUES`), avec
    un installeur NSIS de ~6,6 Mo produit par la CI. Lecon a garder : quand un lot d'essais
    tombe d'un bloc sur une nouvelle plateforme, chercher d'abord ce que les essais
    supposent de leur environnement — un seul essai qui part en boucle d'attente (30 s de
    `PATIENCE`) en fait tomber d'autres autour de lui, et le tableau ressemble alors a une
    panne du produit.
  - **UN ESSAI QUI NE TOMBE QUE SUR macOS OU WINDOWS SE VERIFIE AVANT LE TAG.**
    `.github/workflows/essais.yml`, sur `workflow_dispatch` (`gh workflow run essais.yml`) :
    matrice macOS + Windows, memes verifications que la release, plus l'installeur Windows en
    artefact, et il ne publie RIEN. Il existe parce que deux versions de suite sont parties
    incompletes (v0.38.0 et v0.39.0) pour un essai qui ne tombait que sur macOS : on ne
    l'apprenait qu'apres le tag, donc apres publication, et chaque tentative coutait une
    version aux utilisateurs. Le reflexe : avant `npm run release`, lancer `essais.yml` des
    que le changement touche le service de terminaux ou quoi que ce soit de dependant du
    systeme.
- **`cmd.exe` REAFFICHE SON INVITE ET SON TITRE A CHAQUE TOUCHE.** L'essai qui verifie que
  l'echo d'une touche part tel quel mesurait la taille du premier lot recu apres avoir tape
  un caractere. Sous Windows ce lot faisait **87 octets** : ce n'etait pas l'echo, c'etait
  `C:\Users\...\Temp>` suivi d'une sequence de titre de fenetre. L'essai concluait donc que
  l'echo d'une touche etait gros.
  Le correctif n'est pas une garde de plateforme mais une VRAIE correction de l'essai : on
  vide ce que le shell a dit en demarrant (attente du silence) AVANT de taper. La meme course
  existait sous Unix, simplement plus etroite — un essai qui depend de ce que le shell a fini
  d'ecrire doit toujours attendre le calme d'abord.
- **RETIRER UN FILTRE MAISON FAIT ENTRER CE QU'IL CACHAIT AUSSI.** En 0.38 les six points de
  montage ecrits en dur ont ete supprimes, a raison : ils ne matchaient rien sous Windows et
  laissaient tomber le volume des fichiers de l'utilisateur sous macOS. Effet non prevu : notre
  PROPRE AppImage est reapparue dans la liste des disques. Elle se monte en `squashfs` sur
  `/tmp/.mount_cockpitXXXX`, en lecture seule, donc pleine a 100 % par construction — et la
  cloche annoncait « disque presque plein » a chaque lancement. Signale par le premier
  utilisateur de la 0.41.0.
  Le filtre remis est sur le TYPE de systeme de fichiers (`IMAGES_MONTEES` : squashfs, iso9660,
  erofs, cramfs), pas sur le chemin : un type veut dire la meme chose sur les trois systemes,
  un chemin non — c'est toute la lecon du filtre precedent. Critere a retenir pour juger :
  peut-on LIBERER de la place dessus ? Sur une image montee, non, donc l'alerte n'a pas de sens.
- **UN NOMBRE D'ENVOIS N'EST PAS UN INVARIANT : C'EST UNE MESURE DE LA VITESSE DE LA MACHINE.**
  L'essai de rafale bornait le nombre d'envois (`envois < 600`). Or un lot part au plus toutes
  les 8 ms, donc le nombre suit la DUREE de la rafale : quelques dizaines ici (`seq 1 200000`
  prend une fraction de seconde), largement plus de 600 sur un runner lent — sans qu'aucun
  regroupement n'ait cesse de marcher. La v0.40.0 a echoue sous LINUX pour cette seule raison,
  apres avoir passe macOS et Windows. L'invariant est desormais la **taille moyenne d'un
  envoi** (> 1 Ko) : elle attrape les deux vrais defauts connus (16 461 envois soit ~85 octets
  sans regroupement, 3 047 soit ~295 octets quand la regle ne se declenchait pas) et laisse
  passer n'importe quelle machine. Regle generale : borner ce que la fonctionnalite GARANTIT,
  jamais ce que la machine se trouve a produire.
- **GITHUB MASQUE LA VALEUR DES SECRETS DANS LES LOGS, Y COMPRIS AU MILIEU D'UN NOMBRE.**
  `COCKPIT_REPORT_ALLOW_HTTP` vaut `1`, donc tous les `1` des logs de CI sortent en `***` :
  « 1489157 octets en 156 envois » se lit « ***489***57 octets en ***56 envois ». Un chiffre
  de mesure devient illisible et on peut passer un moment a mal le reconstruire. Deux parades :
  faire IMPRIMER par l'essai la grandeur qui porte la conclusion (ici la moyenne par envoi,
  pas seulement le compte), et se souvenir qu'un `***` au milieu d'un nombre est une redaction,
  pas un caractere.
- **UN BANC QUI CONSTRUIT UN BUNDLE A BESOIN DE LA CLE DE SIGNATURE.** `tauri.conf.json`
  porte la cle PUBLIQUE de l'updater : la CLI reclame donc la privee et echoue APRES avoir
  produit l'installeur (« A public key has been found, but no private key »). Le banc rendait
  rouge un essai qui avait tout reussi — tests verts, `Cockpit_x.y.z_x64-setup.exe` bien la —
  et ca fait chercher un probleme la ou il n'y en a pas. `essais.yml` passe donc
  `TAURI_SIGNING_PRIVATE_KEY` comme `release.yml`. A ne pas confondre avec le build LOCAL, ou
  cet echec est voulu (d'ou `--no-bundle` en local).
  Corollaire de methode : un `continue-on-error` sur une etape de build MASQUE ce genre de
  chose. Le premier atelier Windows est passe « vert » avec cette meme erreur dedans.
- **UN CHEMIN QUI TRAVERSE L'IPC EST UN IDENTIFIANT : IL S'ECRIT AVEC DES `/`.** Toutes les
  fonctions de `workspace/` rendaient leur chemin relatif par `to_string_lossy()`, donc avec le
  separateur du systeme. Sous Windows, `src\notes.md` — et le frontend, lui, DECOUPE et RECOLLE
  sur `/` (`relPath.split("/")` pour deplier l'arbre, `path.split("/").pop()` pour le nom du
  fichier). Consequence : l'arbre de l'onglet Fichiers ne se deplie plus et le nom affiche
  devient le chemin entier. Un seul essai l'a signale (`left: "src\\notes.md"`), mais il
  concernait cinq fonctions.
  `chemin_relatif()` recolle les COMPOSANTS avec `/` — et ne fait PAS un `replace('\\', "/")` :
  l'antislash est un caractere de nom de fichier valide sous Unix, un remplacement global
  corromprait des noms legitimes. Tout nouveau chemin rendu au frontend passe par la.
- **LA FIN D'UN SHELL SE CONSTATE SUR LE PROCESS, PAS SUR LE TUYAU.** Le thread lecteur
  concluait a la fin en recevant la fin de fichier du PTY. Vrai sous Unix, ou la mort du shell
  ferme l'esclave. FAUX sous Windows : **ConPTY garde son tuyau ouvert apres la mort du
  shell** — c'est `conhost` qui le tient, pas le shell — donc la lecture ne rend jamais rien.
  Mesures du runner le 2026-08-21 : `vivant` restait vrai indefiniment, la fin d'un terminal
  n'etait JAMAIS annoncee a l'application (`terminal_exit` jamais emis), la session ne se
  refermait pas cote service, et `fermer()` attendait son delai pour rien. D'ou un thread
  GUETTEUR par session, qui bloque sur `enfant.wait()` — ce qui marche partout, et ramasse le
  shell du meme coup. Il relache ensuite le maitre pour FERMER le pseudo-terminal, seule facon
  de debloquer le lecteur sous Windows, apres une grace de 300 ms : sous Unix le lecteur a
  deja fini d'avaler ce qui restait, et fermer plus tot lui couperait les derniers octets
  d'un programme qui s'arrete.
  Corollaire : `maitre` est un `Option`, et `redimensionner` ne fait RIEN — sans erreur — quand
  le shell est mort. Le frontend continue de mesurer son conteneur pendant qu'un onglet se
  referme, et ce n'est pas une panne a montrer.
- **`ChildKiller::kill()` DE `portable-pty` 0.9.0 REND `Err` QUAND IL REUSSIT, SOUS WINDOWS.**
  Le test est inverse dans `WinChildKiller::kill` (`src/win/mod.rs`) :
  `let res = TerminateProcess(...); if res != 0 { Err(last_os_error()) } else { Ok(()) }` —
  or `TerminateProcess` rend NON-ZERO en cas de succes. Donc un kill qui a marche remonte
  « The operation completed successfully. (os error 0) » (rien n'a pose de code) ou une erreur
  PERIMEE d'un appel anterieur du meme thread, d'ou des « The handle is invalid. (os error 6) »
  incomprehensibles ; et un kill qui a echoue remonte `Ok(())`. `WinChild::kill` avale le sien
  par `.ok()`, c'est le killer CLONE — celui qu'on utilise — qui le propage.
  D'ou la regle : **`Session::fermer` CONSTATE au lieu de croire le code de retour.** Le thread
  lecteur passe `vivant` a faux des que le PTY rend la fin de fichier ; c'est ca qu'on attend
  (`DELAI_FERMETURE`, 2 s). Une seule regle pour les trois systemes, qui ne depend d'aucune
  bibliotheque. Cas ordinaire couvert au passage : un shell DEJA termine (`exit` puis fermeture
  de l'onglet) rendait une erreur a l'utilisateur pour un geste qui avait parfaitement marche.
  Deux lecons de methode, payees toutes les deux :
  - **lire la ligne exacte avant de nommer l'operation en cause.** Le premier diagnostic
    ecrit ici etait « ecrire dans le PTY echoue » — faux. Les numeros pointaient tous la meme
    colonne 20, celle du `.unwrap()` de `s.fermer()`. Ce faux diagnostic a fait sortir Windows
    de la matrice de release pour rien.
  - **quand une bibliotheque rend une erreur qui n'a pas de sens, aller lire sa source** : elle
    est dans `~/.cargo/registry/src/`, et ici la reponse tenait en cinq lignes.
- **UN GROS LOT NE VEUT PAS DIRE UN DEBIT INGERABLE.** `VOLUME_INSOUTENABLE`
  (`terminal/service/session.rs`) valait 256 Ko, justifie par « 256 Ko dans une fenetre de
  8 ms, c'est 32 Mo/s, aucun affichage humain ne suit ». Le raisonnement est faux : un gros
  lot dit seulement que le PTY a livre son tampon d'un coup, ce que macOS fait beaucoup plus
  que Linux. Consequence mesuree sur le runner macOS de la v0.39.0 : sur 1,3 Mo de
  `seq 1 200000` — une sortie tout a fait ordinaire — **368 Ko seulement arrivaient**, le
  reste remplace par sept redessins. L'utilisateur remonte a la molette et ne trouve pas sa
  sortie, ce que le flux brut existe justement pour eviter. Le seuil est un PLAFOND DE
  MEMOIRE (4 Mo), pas un jugement sur le debit : borne pour un flux sans fin
  (`cat /dev/urandom`), et au-dessus de toute sortie de commande normale.
  A retenir plus largement : une constante justifiee par un calcul de debit merite qu'on
  verifie sur quoi ce debit est mesure — ici la fenetre de 8 ms etait le denominateur, et
  elle ne dit rien de ce que la machine tiendra la seconde suivante.
- **LE REGROUPEMENT DE LA SORTIE NE DOIT PAS SE DECIDER SUR « IL A FALLU ATTENDRE ».** La
  regle a d'abord dit : « si la suite attendait deja quand on est revenu, c'est une rafale »
  (`!a_attendu`, `terminal/service/session.rs`). Ca ne mesure pas le debit, ca mesure lequel
  de deux threads va plus vite — vrai sous Linux, FAUX sous macOS, ou chaque reveil trouvait
  ~295 octets et repartait aussitot : **3 047 envois pour 0,9 Mo** sur le runner de la
  v0.38.0. Et le meme defaut expliquait un second symptome qui n'avait pas l'air lie : un
  emetteur qui part 3 047 fois draine trop lentement, donc la fermeture de la session jetait
  ~400 Ko jamais transmis (l'essai le voyait comme « la rafale n'a pas ete transmise »).
  La regle est desormais **la cadence de nos propres envois** (« le lot precedent est-il parti
  il y a moins de 8 ms »), ce qui ne depend d'aucun ordonnancement. Gain sous Linux au
  passage : 19 a 21 envois pour 1,5 Mo, contre 99 a 158 avant.
  **Piege paye en chemin** : la cadence SEULE prend une frappe rapide pour une rafale —
  l'essai de latence enchaine 200 allers-retours, donc en cadence soutenue par construction,
  et le surcout du service est passe de 0,06 ms a **8,5 ms**, soit precisement la surcouche
  que ce projet interdit sur le chemin de frappe. D'ou la seconde condition, `TAILLE_ECHO`
  (64 octets) : ce qui arrive en cadence n'est groupe que si le lot est plus gros que l'echo
  d'une touche. Ne pas monter cette valeur « pour mieux regrouper ».
- **UN SIGNAL POSIX COMPILE SOUS WINDOWS ET RATE A L'EXECUTION.** `sysinfo::Signal::Term`
  existe sur toutes les plateformes ; c'est sa CONVERSION qui rend `None` sous Windows
  (`windows/mod.rs`, branche `_ => None`), ou seul `Signal::Kill` est accepte et applique par
  `taskkill.exe /F`. Donc `kill_with(Signal::Term)` compilait sans un mot et notre code
  affichait « failed to send SIGTERM », un message qui NOMME un mecanisme inexistant sur ce
  systeme et envoie le diagnostic vers un probleme de permission. La regle qui en sort :
  quand une bibliotheque expose un enum commun a trois systemes, chercher la table de
  conversion de chaque plateforme avant de croire que la valeur est acceptee. Corrige le
  2026-08-21 (`system/process.rs`, `SIGNAL_D_ARRET` + `ARRET_FORCE`).
- **`interprocess::PeerCreds::euid()` est declare `#[cfg(unix)]`** : il n'EXISTE pas a la
  compilation sous Windows, ou la structure ne porte qu'un pid. C'etait le SEUL bloqueur de
  compilation Windows de tout le crate au 2026-08-21 (une erreur, `terminal/service/tuyau.rs`)
  — l'etude `docs/portabilite/divers.md` en annoncait un autre (`PermissionsExt` non
  conditionne) qui avait deja ete corrige entre-temps. Lecon d'usage : prendre le compilateur
  pour verite, pas l'etude de lecture.
- **`sysinfo` n'expose AUCUNE notion de cache, de buffers ni de memoire partagee, sur aucune
  plateforme** — sept nombres de memoire, point (verifie dans la source vendoree). Le detail
  memoire n'est donc pas un manque qu'une montee de version comblerait : c'est du code natif
  par systeme, ou rien. D'ou le choix acte : socle commun partout, detail LINUX en supplement
  (`MemoryMetrics.detail: Option<...>`). Ne pas rouvrir ce debat sans une raison neuve : les
  categories ne se traduisent pas (macOS compresse la RAM, Windows n'a ni buffers ni partage),
  et deux des trois branches ne seraient pas testables ici.
- **Le nom d'un socket de domaine Unix est limite a ~108 OCTETS**, et l'erreur ne le dit pas.
  Constate le 2026-08-21 en pointant `COCKPIT_TERMINAUX_SOCKET` dans un dossier de travail
  profond : le service demarrait, n'ouvrait rien, et l'application rendait seulement « le
  service de terminaux n'a pas ouvert son socket en 10 s ». Consequence pratique : pour un banc
  d'essai, mettre le socket dans `$XDG_RUNTIME_DIR` (chemin court), pas dans le dossier de
  travail du scratchpad.
- **Du code d'apparence portable peut etre mort a moitie.** `exe_est_llm` lisait
  `/proc/<pid>/exe` sans `#[cfg]` : ca compile partout, et ca rend TOUJOURS faux ailleurs que
  sous Linux. La detection des agents avait donc bien une branche non-Linux (construite sur
  `sysinfo`), mais la moitie anti-usurpation d'argv y etait inerte, sans erreur ni trace. Meme
  famille de piege : `basename()` ne coupait que sur `/`, donc `C:\...\claude.cmd` rendait le
  chemin entier, et `est_commande_llm` ne retirait que `.js`/`.mjs`, donc jamais `.cmd` ni
  `.exe` — trois defauts silencieux dans une fonction de vingt lignes. Chercher les chemins et
  les separateurs en dur AVANT de conclure qu'un module est portable.

- **`[cockpit] <defunct>` derriere l'application : ce N'EST PAS le double fork du service, et
  la cause reste a trouver (mesures du 2026-08-21, a ne pas refaire).** Symptome : un ou deux
  zombies dont le parent est l'application, qui apparaissent a des moments imprevisibles
  (t+82 s apres le demarrage, puis 37 min plus tard) et restent jusqu'a la fermeture.
  - **Signature du zombie**, lue dans `/proc/<pid>/stat` : `comm` = `cockpit`, `exit_code`
    (champ 52) = 0, `minflt` (champ 10) = 205-229, `utime` = 0. Un `cockpit` qui a VRAIMENT
    exec puis quitte tout de suite fait 1348 fautes mineures et 1215 majeures (mesure :
    `/usr/bin/time -v ./cockpit --service-terminaux /chemin/absent`). Donc **ce zombie n'a
    jamais exec** : c'est un fork de l'application qui a fait `_exit(0)`.
  - **`lancer_detache` a ete disculpe trois fois** : (1) 2000 lancements d'affilee avec le
    meme double fork, dans un processus multi-thread ou tokio lance et moissonne des process
    en continu — zero fuite ; (2) service tue a la main sur une instance isolee, relance par
    l'application, aucun zombie ; (3) socket rendu injoignable, l'application reessaie et
    echoue en boucle, aucun zombie. `enfant.wait()` recoit bien l'intermediaire, et la
    lecture de la source de `std` confirme que le chemin d'ERREUR de `spawn()` moissonne
    aussi (`assert!(p.wait().is_ok())`).
  - **Ce qui reste comme suspect** : le seul autre code du processus qui fasse « fork puis
    `_exit(0)` sans exec » est l'enfant INTERMEDIAIRE de `g_spawn` (GLib), utilise par
    WebKitGTK pour lancer ses processus auxiliaires. Il porte le `comm` du parent, donc
    `cockpit`, et GLib le moissonne lui-meme par un `waitpid` qui peut perdre la course.
  - **Ne PAS "corriger" par un `waitpid(-1, WNOHANG)` de nettoyage** : il volerait les enfants
    de tokio (`docker`, `git`) et de GLib, qui verraient leurs commandes echouer sans raison.
    Un zombie par heure est une fuite a comprendre, pas a masquer.
  - **Pour trancher, il faut un strace de l'instance qui fuit** : `ptrace_scope` vaut 1 sur
    cette machine, donc on ne peut tracer QUE ses propres descendants — lancer l'application
    soi-meme (`xvfb-run -a dbus-run-session -- strace -f -tt -e trace=clone,clone3,execve,wait4 ...`)
    avec une COPIE de la vraie base, sinon le monitor Docker ne tourne pas et la moitie des
    forks n'ont pas lieu. Le guetteur qui sert a ca : lire `/proc/*/stat` toutes les 50 ms et
    ne signaler que les enfants du pid vise.
  - Detail qui a fait perdre du temps : les enfants de tokio passent par l'etat `Z` une
    fraction de seconde avant d'etre moissonnes. Un compteur de zombies naif les compte et
    annonce une fuite qui n'existe pas. Ne compter que ceux qui portent NOTRE nom de programme
    (donc n'ont pas exec) et qui sont encore la deux secondes plus tard.
- **LES `[cockpit] <defunct>` NE VIENNENT PAS DE NOTRE LANCEMENT — piste fermee, mesuree.**
  Un ou deux zombies portant notre nom apparaissent sous l'application, et le double fork
  du service en etait le suspect evident : son intermediaire s'efface aussitot, et un `wait`
  oublie laisserait exactement ca. Mesure du 2026-08-21 : `lancer_detache` en laisse **zero**,
  dix releves de suite, et l'essai `lancer_le_service_ne_laisse_pas_de_zombie` verrouille
  desormais la propriete. Nos deux autres facons de lancer un process ne peuvent pas produire
  ce symptome : `tokio::process` a sa file d'orphelins, et un serveur LSP qui meurt porte SON
  nom, pas le notre. Ce qui reste : le patron de `g_spawn` de GLib (WebKit et les portails s'en
  servent) fait lui aussi un fork intermediaire, et un intermediaire non ramasse porte le nom
  du programme PARCE QU'IL N'A JAMAIS `exec`. C'est benin — un zombie ne retient qu'un numero
  de process. Ne pas rechercher la cause dans notre code.
- **Un processus detache par double fork n'est PAS adopte par le pid 1** sur un bureau Linux
  moderne : `systemd --user` se declare sous-moissonneur et recupere les orphelins de la session.
  Constate le 2026-08-21 en verifiant le detachement du service de terminaux (parent 6505 =
  `systemd --user`, pas 1). Un test qui exigerait `ppid == 1` echouerait alors que tout va bien :
  la bonne assertion est « le parent n'est plus celui qui a lance ».
- **Un essai qui lance un VRAI processus doit l'arreter dans un `Drop`, pas a la fin du corps
  du test.** Le 2026-08-21, un `assert!` rate au milieu de l'essai de survie a laisse un service
  et ses shells tourner jusqu'a la deconnexion — invisible, puisqu'il est detache et sans
  console. Meme regle pour tout ce qui survit au test (voir `BancDetache` dans
  `terminal/service/tests.rs`).
- **Un test qui guette le resultat d'une commande dans un terminal trouve d'abord ce qu'il vient
  de TAPER.** Le PTY renvoie l'echo de la ligne avant que le shell ne l'execute : un
  `contains("mon-marqueur")` reussit immediatement et on compare deux ecrans pris a des moments
  differents. Remede employe : une commande dont la sortie porte un marqueur que la ligne tapee
  ne contient pas (`printf 'trace%s\n' -avant-la-coupure`). Ne PAS guetter l'invite du shell non
  plus : elle depend de la configuration de l'utilisateur (ici zsh + oh-my-zsh, invite « ➜ »).
- **`interprocess` : `&Stream` implemente `Read` ET `Write`.** Pas besoin de `split()` (que la
  crate deconseille elle-meme) : un `Arc<Stream>` partage entre un thread lecteur et un thread
  ecrivain suffit. `Stream::peer_creds()` (trait `StreamCommon`) donne l'euid ET le pid du pair
  sous Unix — c'est a la fois le controle de proprietaire et un moyen de retrouver le processus
  d'en face.

- **`alacritty_terminal` cache trois etats dont un redessin a besoin** : la region de
  defilement (DECSTBM), le titre et sa pile, le jeu de caracteres actif. `Term` n'a pas
  d'accesseur pour eux. On les suit avec un ESPION : le meme flux d'octets est donne a un
  SECOND analyseur `vte` dont le gestionnaire n'implemente que ces quatre operations
  (`terminal/ecran/mod.rs`). Ne PAS remplacer ca par un gestionnaire qui envelopperait
  `Term` : `Handler` compte 85 methodes a corps vide par defaut, un enveloppeur doit toutes
  les reexpedier, et une faute de frappe dans l'une d'elles casserait l'EMULATION sans
  qu'aucun test d'aller-retour ne le voie — les deux cotes du test passeraient par le meme
  enveloppeur. L'espion, lui, n'a pas de grille : il ne peut rien casser.
- **`swap_alt()` DETRUIT l'ecran alternatif quand on y revient** : il remet la grille
  inactive a zero a chaque entree en ecran alternatif. Il n'existe donc aucun moyen de LIRE
  la grille principale cachee sous une application plein ecran sans perdre l'autre. Le
  redessin ne rend que l'ecran actif ; le service tient l'etat complet et redessinera quand
  l'ecran actif changera.
- **La ligne qui entre par le bas herite du FOND du stylo** : `Cell::reset` ne recopie que
  `bg`. Tout ce qui fait defiler — un saut de ligne, un enroulement — avec un fond actif
  teinte la ligne d'arrivee, donc les fins de ligne qu'un redessin ne redessine pas.
  Remettre le fond par defaut avant de faire defiler (`terminal/ecran/redessin.rs`).
- **`unicode-width` rend parfois 3** (le signe khmer U+17D8, par exemple) alors que
  `Term::input` ne connait que « une colonne » et « deux colonnes ». Compter comme la crate
  au lieu de compter comme l'emulateur sautait une cellule et decalait toute la fin de la
  ligne. Trouve par des octets au hasard, invisible sur des traces reelles.
- **Le fanion `WRAPLINE` ne se pose qu'en ECRIVANT un caractere** alors que le curseur est
  en butee a droite : aucune sequence ne le demande. Un redessin qui saute la ligne (`\r\n`)
  le perd, et un redessin qui l'enchaine sans qu'un caractere suive ne le pose pas. C'est
  pour ca que `dessiner_ligne` rend l'ETAT dans lequel elle laisse le curseur.
- **La tabulation est le seul caractere de commande qui finit DANS une cellule** :
  `put_tab` y ecrit `\t` si elle contient une espace, sans toucher ses attributs, puis saute
  au taquet suivant. Reemettre `\t` a la place du caractere decale donc tout le reste de la
  ligne de huit colonnes. Il faut poser une espace avec les bons attributs, revenir dessus,
  tabuler, puis revenir a la colonne suivante.

- **Build** : jamais `cargo build --release` seul pour le binaire final (mode dev -> cherche Vite
  sur localhost:5173). Toujours `npx tauri build --no-bundle`. Un rebuild du frontend seul ne
  ré-embarque pas toujours les assets : c'est la recompilation de la crate qui les fige.
- **Ordre des invoke Tauri** : des `invoke` rapproches peuvent s'executer dans le desordre ->
  toute ecriture PTY passe par une file par terminal cote frontend (ioQueues).
- **UNE COMMANDE TAURI SANS `async` TOURNE SUR LA BOUCLE PRINCIPALE GTK ET GELE TOUT.**
  Piege de CONCEPTION, invisible a la lecture : le macro `#[tauri::command]` sur un `fn`
  (sans `async`) produit un contexte d'execution bloquant, et la commande s'execute en ligne
  dans le gestionnaire IPC de wry — lequel est un signal GTK. Pendant tout son travail,
  l'interface ne repeint plus et les evenements `terminal_output` ne sont plus livres.
  Constate le 2026-08-20 sur `list_all_terminals`, appelee toutes les 5 s par le magasin
  `terminals` : 50 ms a vide, et jusqu'a 1 s quand des agents tournaient — donc une interface
  figee une seconde toutes les cinq. Mesure : chronometrage de chaque commande externe du
  poll (deux appels tmux ~7 ms a l'epoque, `ps -e -o pid=,ppid=,args=` 47,6 ms sur 1074
  process). A faire : toute commande qui lance un process externe est
  `async fn` ; et un `async fn` avec `tauri::State<'_, _>` doit rendre un `Result`.
  Meme faute trouvee et corrigee le 2026-08-21 sur `machine_report` : un `fn` qui lancait
  `pactl info` et `pw-record --version`, puis qui a interroge le serveur audio — elle est
  passee `async fn` avec le travail sur `spawn_blocking`.
- **Enumerer tous les process de la machine pour en regarder trois** : la detection des
  agents LLM lancait `ps -e -o pid=,ppid=,args=` a chaque passe, des qu'une commande de
  premier plan n'etait pas un nom de LLM — c'est-a-dire toujours, puisque argv mentait sur
  4 sessions sur 9. Remplace par une descente de l'arbre depuis la racine de chaque session,
  avec sortie des qu'un LLM est trouve. Mesure du 2026-08-20, meme resultat de detection verifie
  session par session : 56,5 ms -> 4,0 ms de mediane, dont 3,3 ms allaient a tmux pour trouver
  les racines — le service, lui, les connait deja (il a lance les shells). Le cout du parcours
  lui-meme est de ~0,3 ms, et il depend du nombre de terminaux, plus de celui des process de la
  machine.
- **Un evenement Tauri v2 est du JavaScript construit puis evalue, pas un canal binaire** :
  `emit` fabrique une source `(function () { ... fn({event, payload: <charge>}) })()` et
  l'evalue dans le webview (tauri-2.11.0, `event/mod.rs::emit_js_script` et
  `webview/mod.rs`). Donc 8 Ko d'octets = ~11 Ko de source JS + un saut vers le WebProcess,
  et une rafale de terminal en produisait des milliers. Mesures du 2026-08-20 : 1,9 Mo a
  travers un vrai PTY partait en 2547 evenements (3 apres regroupement) ; cote webview,
  evaluer la meme quantite d'octets en 240 scripts coute 11,2 ms contre 2,3 ms en 15 (banc
  WebKitGTK 2.52 offscreen, python3+gi). A faire : regrouper AVANT d'emettre — mais par
  contre-pression (un thread lecteur, un thread emetteur, une file), jamais par une horloge
  qui retiendrait l'echo des touches.
- **`Uint8Array.from(atob(s), cb)` appelle `cb` une fois PAR CARACTERE** : 75,2 ms pour
  decoder 1,96 Mo, contre 2,8 ms avec `atob` puis une boucle `for` nue qui remplit un
  `Uint8Array` prealloue (mesure du 2026-08-20, node). Sur le chemin de la sortie terminal
  c'est le thread qui dessine qui paie. La version « elegante » est 27 fois plus lente.
- **Ctrl+lettre sous WebKitGTK** : emet aussi un keypress ; n'intercepter que le keydown laisse
  xterm envoyer le caractere de controle au shell. Bloquer tous les types d'events + listener en
  phase capture sur le conteneur.
- **L'ecran alternatif se lit dans la grille du SERVICE**, jamais dans xterm : c'est le service
  qui emule (`Ecran::ecran_alternatif`), et il renvoie un redessin a chaque bascule — sinon
  quitter vim laisserait le frontend sur l'ecran de vim. Rien a demander a personne, la reponse
  est en memoire.
- **Reponses du terminal dans onData** : focus in/out, DA, CPR, reponses DCS/OSC arrivent par le
  meme canal que les frappes -> a filtrer (regex TERMINAL_REPLY) sinon toute heuristique de suivi
  de frappe se fait polluer.
- **POOL PERSISTANT : ni detach ni re-attach au switch.** Les xterm vivent dans un POOL au
  niveau module (TerminalTab, `<script module>`), gares dans un div invisible au demontage et
  re-adoptes au retour ; les listeners terminal_output/exit sont GLOBAUX pour alimenter les
  xterm meme demontes ; `attach_terminal` est un no-op quand le terminal est deja branche. La
  raison a change avec tmux, pas la regle : un xterm re-cree part vide et exige un redessin
  complet (ecran + 10 000 lignes), donc un clignotement et un retour en bas du defilement a
  chaque aller-retour entre deux onglets. Du temps de tmux la cause etait ailleurs — il
  synthetisait des evenements focus vers l'application du pane a chaque attache de client, et
  claude y reagissait par un re-render qui laissait une ligne vide. Benefice inchange : switch
  instantane.
- **REGROUPEMENT DE LA SORTIE : le declencheur est le RYTHME, pas le volume en attente.** La
  premiere version du service attendait d'avoir 8 Ko en attente pour regrouper — elle ne s'est
  jamais declenchee, parce qu'un shell ecrit au fil de l'eau et que le lecteur du PTY est plus
  rapide que lui : chaque lecture rend ~85 octets. Mesure du 2026-08-21 : `seq 1 200000` (1,3 Mo)
  partait en **16 461 envois**, soit autant d'evenements Tauri. Deux details qui ont failli faire
  rater le diagnostic :
  - l'essai qui pretendait mesurer cette rafale attendait « 200000 » a l'ecran, chaine que la
    LIGNE TAPEE contient deja : il repartait avant que le shell ait ecrit un octet et mesurait le
    redessin de l'attache. Un marqueur de fin doit etre construit par le shell (`printf
    'rafale%s' -finie`), jamais present dans la commande ;
  - la regle du rythme seule (« la suite attendait deja, donc c'est une rafale ») retombe a 939
    envois quand la machine est chargee : le shell produit alors par a-coups et l'emetteur attend
    a chaque fois. D'ou la seconde moitie de la regle, sur le volume du lot (`SEUIL_LOT`, 2 Ko).
  Etat verrouille par l'essai : moins de 600 envois pour 1,3 Mo (99 sous charge, 158 a vide).
- **DOUBLE COLLAGE AU CLIC MOLETTE : `preventDefault` sur l'evenement `paste` NE SERT A RIEN.**
  Le symptome est revenu deux fois, avec deux causes DIFFERENTES (abonnements onData empiles la
  premiere fois, puis celle-ci) : ne jamais supposer laquelle, mesurer.
  Ce qui a ete constate au banc (xterm 6.0.0 charge pour de vrai, clic milieu simule par XTEST
  dans le WebKitGTK systeme sous Xvfb, 2026-08-20) :
  - **xterm implemente le collage LUI-MEME** : `handlePasteEvent` lit `e.clipboardData` et
    injecte le texte via `triggerDataEvent` -> il ne depend pas de l'action par defaut du
    navigateur, donc `preventDefault()` ne l'arrete pas. Il pose ce handler sur le textarea cache
    ET sur `.xterm`, pendant `term.open()` — donc AVANT tout handler qu'on ajoute ensuite, et en
    phase cible l'ordre est celui de l'inscription. Un `paste` intercepte sur le textarea arrive
    toujours trop tard.
  - **Il faut ecouter en CAPTURE sur un ANCETRE** (le `.term-host`) et appeler
    `stopImmediatePropagation()` : c'est la seule facon qu'xterm ne voie jamais l'evenement.
    Garder `preventDefault()` en plus, sinon le texte atterrit dans le textarea cache et
    ressort a la frappe suivante (meme mecanisme que le BUG ACCENTS).
  - **Le collage natif du clic molette lit CLIPBOARD, pas la selection PRIMARY** (verifie avec
    deux contenus differents). Les deux collages portaient donc le MEME texte : a l'oeil, ca ne
    ressemble pas a deux collages mais a une commande dupliquee ou a une frappe fantome.
  - `preventDefault` sur `mousedown` n'empeche pas le collage natif, et le reglage GTK
    `gtk-enable-primary-paste` est ignore par WebKitGTK. Ces deux pistes sont fermees.
  - Ctrl+V dans le terminal n'emet AUCUN evenement `paste` : xterm envoie `^V` au PTY (mesure).
    Ne pas construire de raisonnement sur « il faut preserver le collage Ctrl+V ».
- **Une liste chargee au montage n'est pas une source de verite, et une garde posee dessus
  rejette exactement les cas qu'on voulait servir.** L'onglet Terminal n'honorait une demande
  d'ouverture (`pendingTerminalId`) que si l'id figurait dans sa liste locale `sessions`,
  chargee une seule fois au montage. Or les producteurs de cette demande CREAIENT la session
  juste avant de poser l'id (commande rapide, shell d'un conteneur, palette Ctrl+K — ces trois
  la passent DEPUIS par `pendingTerminalCommand`, voir l'entree suivante ; restent la barre
  laterale, le tableau de bord et la palette, qui visent des sessions existantes) : la
  session existait donc bien et la barre laterale la montrait, mais l'onglet n'affichait
  rien, ne disait rien, l'id restait coince dans le magasin — empoisonnant les navigations
  suivantes — et chaque nouvel essai laissait une session de plus derriere lui (issue #14 :
  quatre `QUOTIDIEN - n` dans la barre laterale, deux onglets). Reproduit au banc frontend
  ci-dessous, six scenarios verifies apres correction. Regle : avant de conclure qu'une cible
  n'existe pas, la RECHARGER depuis le backend ; si elle n'existe vraiment plus, le dire et
  vider le magasin. Detail qui compte : quand la cible n'appartient pas a CE projet, ne pas
  vider le magasin — l'onglet du bon projet doit encore pouvoir la prendre.
- **UNE TUI LANCEE A LA CREATION SE DESSINE A LA TAILLE DU PTY, ET PERSONNE NE LA
  REDIMENSIONNE APRES.** Creer un terminal a une taille arbitraire n'est donc pas « une taille
  provisoire que le premier redimensionnement corrigera » : c'est definitif. Le bouton ▶ Cmd, le
  shell de conteneur de l'onglet Docker et la commande rapide de la palette appelaient tous
  `create_terminal(..., 80, 24, commande)` — k9s s'affichait dans un carre de 80x24 en haut a
  gauche d'un conteneur large, et un simple shell coupait ses lignes trop tot (issue #14,
  deuxieme symptome).
  Deux maillons expliquent l'absence de correction, et les DEUX sont voulus ailleurs :
  - `attacher()` (terminal/adaptateur.rs) ne fait RIEN quand le terminal est deja branche : les
    `cols`/`rows` passes a `attach_terminal` sont alors ignores. Or le terminal cree par
    `create_terminal` l'est deja, donc l'arrivee sur l'onglet ne recadre rien.
  - `attachExisting` (TerminalTab) pre-renseigne `lastSentSize` avec la taille mesuree AVANT
    l'attache, donc le `queueResize` du `fitActive()` suivant est saute (il croit la taille
    deja envoyee). Aucun `resize_terminal` ne part.
  Mesure (2026-08-20) : banc frontend, le journal des invoke montre `create_terminal
  {cols:80,rows:24}` pour un conteneur de 1398x732 px = 196x48 cellules, `attach_terminal
  {cols:196,rows:48}` et AUCUN `resize_terminal` ; un shell lance dans un PTY 80x24 dessine un
  htop sur 80 colonnes, contre 177 quand le PTY est cree a la taille mesuree. D'ou la regle : **seul l'onglet Terminal cree une
  session** (`addTerminal`, qui mesure), les autres deposent leur commande dans
  `pendingTerminalCommand`.
- **UN `{@const}` EST UN DERIVE PARESSEUX : le lire depuis une action executee APRES la
  fermeture du menu leve une TypeError, avalee, et l'action ne se fait jamais.** Les menus
  contextuels s'ecrivent tous sur le meme moule — `{#if menu}{@const n = menu.node}` puis des
  items dont l'action lit `n`. `ContextMenu.pick()` appelait `onClose()` AVANT
  `item.action()` : `menu` repassait a null, le `{@const}` se recalculait a la lecture
  suivante (donc dans l'action) et cassait sur `null.node`. Resultat : « Renommer »/« Fermer »
  d'un terminal, « Renommer »/« Supprimer » d'un dossier et TOUT le menu de l'arbre de
  l'onglet Fichiers ne faisaient rien, sans message. Mesure au banc frontend le 2026-08-20
  (`Uncaught TypeError: Cannot read properties of null (reading 'node')` capture par un
  ecouteur `window.onerror`, action non executee ; les memes scenarios passent apres
  inversion). L'ordre `action() puis onClose()` est marque sur place, NE PAS L'INVERSER. Regle
  generale : une valeur tiree de l'etat d'un overlay doit etre capturee en PARAMETRE d'une
  fonction, pas lue paresseusement depuis une fermeture.
- **Banc frontend : jouer un composant Svelte sans lancer l'application** (2026-08-20). Un
  Chrome sans tete + un FAUX backend Tauri suffisent, et le DOM rendu sert de preuve.
  Recette : un dossier de travail HORS du depot avec `node_modules` symlinke, une
  `vite.config.ts` qui remplace `@tauri-apps/api/{core,event,webview}` par des modules maison
  (`invoke` -> table de reponses en memoire, `listen`/`emit` -> map d'abonnes), un `main.ts`
  qui monte le composant, deroule le scenario et ecrit son journal dans un `<pre>`. Puis
  `vite build`, `python3 -m http.server` (les modules ES ne se chargent PAS depuis `file://`)
  et `google-chrome --headless=new --virtual-time-budget=30000 --dump-dom http://127.0.0.1:PORT/`.
  On lit dedans le nombre d'onglets, lequel est actif, les toasts affiches, l'etat des
  magasins. Les `setTimeout` du scenario avancent en temps virtuel, donc c'est instantane.
  Ce que ca ne remplace pas : WebKitGTK (pour un bug de RENDU, garder le banc python3/gi).
- **Rendu xterm** : le renderer DOM + `monospace` generique derive visuellement sur les glyphes
  accentues. Le modele est sain (verifiable par la grille du service) : addon WebGL + police
  explicite.
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
- **Un projet en base doit TOUJOURS apparaitre dans l'UI** : l'ancienne `list_projects` ne
  renvoyait que l'intersection base∩orchestrateur — un `add_project` orchestrateur qui echouait
  en silence rendait le projet invisible (onglet Docker vide, terminal inerte, premier retour
  utilisateur externe). `list_projects` synthetise desormais une entree Stopped pour tout
  projet DB-only, et la creation met a jour l'orchestrateur au lieu d'echouer si le nom existe.
- **`refreshed_state` adopte un projet Stopped dont les conteneurs tournent A TOUT MOMENT**,
  pas seulement au premier scan apres demarrage. L'ancienne garde `initial_done` laissait un
  projet cree en cours de session afficher "stopped" jusqu'au redemarrage. Afficher la realite
  ne se perime pas — ne pas reintroduire cette garde.
- **WebKitGTK et les overlays (3 bugs distincts, 2026-08-14, tous sous image de fond)** :
  1. overlay enfant d'un conteneur `isolation: isolate` = peint SOUS le reste -> `use:portal` ;
  2. surface flottante en tokens `--bg-*` = translucide -> tokens opaques `--surface-*` ;
  3. voile plein ecran PEINT = WebKitGTK desactive les backdrop-filter de toute la page
     en dessous -> le voile porte son propre `blur(12px)`.
  Les trois regles detaillees sont dans « Interdits/Reflexes » ci-dessus et components.css.
- **Bug de RENDU (flou, transparence, empilement) : reproduire dans le WebKitGTK systeme
  AVANT de corriger.** Harnais : page HTML minimale + script python3/gi (Gtk 3.0 + WebKit2 4.1,
  le moteur exact de Tauri), capture `Gdk.pixbuf_get_from_window`, lance sous `xvfb-run -a`
  (aucune fenetre visible), UNE PAGE FRAICHE PAR SCENARIO (les styles injectes persistent).
  L'outil Read affiche les PNG : comparaison a l'oeil, preuve en images. C'est ce banc qui a
  identifie le bug n°3 ci-dessus et invalide deux fausses pistes en 10 minutes.
  Deux details qui coutent du temps quand on les redecouvre :
  - **`Gtk.OffscreenWindow` + `Gdk.pixbuf_get_from_surface` NE REND JAMAIS** sous Xvfb (banc du
    2026-08-20 : trois minutes sans capture, tue au timeout). Utiliser une `Gtk.Window` normale
    et `Gdk.pixbuf_get_from_window`, avec `WEBKIT_DISABLE_DMABUF_RENDERER=1` et un
    `GLib.timeout_add` de secours qui quitte la boucle.
  - le banc sert aussi a VERIFIER un controle sur image de fond sans lancer l'app : une page
    statique qui charge les vrais `styles/{global,theme,components}.css` en `<link>`, `<html
    class="dark has-wallpaper">`, un `.wallpaper` a degrades croises, et le composant recopie.
    C'est comme ca qu'a ete choisi le glyphe du bouton Lecture (les fleches `⇤⇥` sont illisibles
    a 0,78rem, les triangles `▸◂` non) — trente secondes par variante.
- **L'AppImage embarque des bibliotheques de sa machine de construction, et ca casse chez
  les autres** : linuxdeploy y met la `libwayland-client` du runner (22.04 -> 1.20). Sur une
  distro plus recente, le pilote graphique du systeme (Mesa 25+, lui jamais embarque) se
  retrouve lie a cette vieille version : l'init EGL de WebKit rend EGL_BAD_PARAMETER, WebKit
  abort et l'hote affiche son rapporteur de plantage — fenetre jamais ouverte (constate chez
  un testeur sur Ubuntu 26.04, v0.27.0). Contournement dans `lib.rs`
  (`preload_system_libwayland`, LD_PRELOAD herite par le WebKitWebProcess) ; bug amont sans
  correctif ni option d'exclusion : tauri-apps/tauri#15665. Deux fausses pistes ecartees au
  banc : `WEBKIT_DISABLE_DMABUF_RENDERER` / `WEBKIT_DISABLE_COMPOSITING_MODE` n'y changent
  rien, et **changer le runner pour 24.04 non plus** (1.22 melangee a 1.24 abort pareil).
  **Banc de test** : `docker run` sur ubuntu:22.04/24.04/26.04 + `xvfb-run AppRun` sur
  l'AppImage extraite — le temoin 24.04 prouve que l'echec vient de la distro et non de
  l'absence de GPU dans le conteneur. A refaire avant de toucher au bundling.
- **Le shell Claude tourne DANS un terminal Cockpit** : il herite des fuites AppImage
  (PYTHONHOME casse python3, LD_LIBRARY_PATH casse curl). Prefixer les outils sensibles par
  `env -u PYTHONHOME -u PYTHONPATH -u LD_LIBRARY_PATH ...`.
- **Changer la LARGEUR d'une zone de texte deplace la lecture** : les retours a la ligne ne
  tombent plus au meme endroit, donc conserver `scrollTop` ne ramene pas sur le meme paragraphe
  (l'ecart grandit avec la longueur de la note). Toute bascule de ce genre — mode lecture,
  panneau qu'on replie, colonne qu'on cache — doit reperer un BLOC visible et sa distance au
  bord haut, puis le remettre a la meme place apres `await tick()` (voir `repereDeLecture` /
  `restaurerRepere` dans NoteEditor.svelte). Meme chose pour le curseur de saisie : le clic sur
  le bouton fait perdre le focus du contenteditable, il faut cloner le `Range` avant et le
  reposer apres.
- **Commandes de fond (run_in_background) : chemins ABSOLUS uniquement.** Le cwd du shell varie
  d'un appel a l'autre (`cd src-tauri` a echoue car le shell y etait deja) et un `&&` casse en
  tete fait rater TOUTES les etapes suivantes en silence. Toujours relire le log de sortie
  reel avant d'annoncer un succes — la notification de fin ne prouve rien.
- **`npm run i18n:audit` a annonce 0 pendant longtemps avec 42 libelles francais en dur dans
  l'interface.** Ses regles ne voyaient que des formes tres etroites : la chaine devait etre
  COLLEE a la parenthese de `notify(` (un ternaire lui echappait), la classe `[^"{}]+` des
  attributs excluait les accolades pour ignorer les valeurs dynamiques — donc ecartait aussi
  toute phrase CONTENANT une interpolation, c'est-a-dire les phrases redigees — et un libelle
  range dans une variable (`treeError = "Chemin du projet inconnu"`) echappait a tout, la forme
  habituelle des messages d'erreur d'un composant. Corrige le 2026-08-20 : les regles balayent
  l'expression entiere et en extraient les chaines, avec quatre filtres pour ne pas hurler
  partout. **Un audit vert ne prouve pas qu'il regarde au bon endroit : pour le verifier,
  injecter une chaine en dur de chaque forme et voir si elle est signalee.**
- **Un contenteditable ne garantit AUCUNE position de curseur apres son dernier bloc.** Mesure
  au banc WebKitGTK : avec un `<pre>` en dernier, ni la fleche bas, ni la fleche droite, ni un
  clic au ras du bas, ni meme un `Range.setStartAfter()` force n'en sortent — il n'y a rien
  apres, donc rien a viser. Et `Entree` dans un `<pre>` CLONE le bloc au lieu d'ouvrir un
  paragraphe. C'est l'origine du blocage de l'issue #5 : l'editeur de notes garantit desormais
  un paragraphe final apres tout bloc (`pre`/`blockquote`/liste). Attention, le rendu Markdown
  laisse un saut de ligne apres le dernier bloc : le prendre pour le dernier noeud fait sauter
  la garde. Contre-preuve utile : `formatBlock "p"` depuis l'INTERIEUR du bloc desenveloppe
  correctement — la sortie existe, il faut juste l'exposer.
- **Un `<pre>` sans enfant `<code>` n'est pas du code pour turndown** : sa regle lit
  `node.firstChild.textContent`, donc le bloc repartait en simple paragraphe a la sauvegarde et
  les lignes se recollaient. Du code perdu en silence dans les notes. Une regle maison couvre
  les deux formes depuis le 2026-08-20, y compris pour les notes deja enregistrees.
- **Une liste blanche de schemas d'URL cote frontend ET une garde cote Rust doivent dire la
  MEME chose.** `NoteEditor` autorisait `mailto:` que `open_url` refusait, et validait l'URL
  RESOLUE contre une base bidon tout en envoyant le href BRUT : `[x](www.ex.com)` passait le
  controle puis se faisait rejeter par le backend, avec un message technique a l'ecran. Deux
  gardes qui ne s'accordent pas fabriquent des erreurs sur des liens legitimes. Depuis le
  2026-08-20 il n'y a plus qu'UN endroit cote frontend : `SCHEMAS_OUVRABLES` et `analyserLien`
  dans `utils/adresses.ts`, l'ouverture et les messages dans `utils/liens.ts` (`ouvrirLien`).
  Tout nouvel endroit qui ouvre un lien passe par la — ne pas recopier la liste.
- **Un debounce qui repart a chaque frappe n'expire JAMAIS pendant une frappe continue.**
  L'editeur de fichiers recalculait la couche coloree Shiki 120 ms apres la DERNIERE touche :
  a 80 ms par touche (rythme de dactylo ordinaire), 0 caractere sur 33 s'affichait pendant la
  rafale — et le textarea etant transparent, on tapait litteralement dans le vide jusqu'a la
  premiere pause. Mesure au banc, et le piege est ailleurs que la ou on le cherche : ce n'est PAS
  un probleme de cout de coloration (37 ms pour 1500 lignes de markdown, 105 ms pour 1000 lignes
  de TypeScript) ni de taille de fichier (0/33 aussi sur 40 lignes). Correctif : tant que la
  couche coloree est en retard, on affiche le texte BRUT du textarea — les deux couches partagent
  police, taille, interligne et padding, donc la substitution ne deplace rien. **Regle generale :
  un rendu asynchrone superpose a une saisie doit avoir un repli synchrone, sinon la saisie est
  invisible.**
- **Un message d'erreur de SQLite remonte TEL QUEL jusqu'au toast.** `projects.name` est
  `UNIQUE` : renommer un projet vers un nom deja pris affichait « UNIQUE constraint failed:
  projects.name » a l'utilisateur (reproduit par un test sur base en memoire, 2026-08-20). Deux
  parades, les deux necessaires : l'interface controle la collision AVANT d'appeler, avec un
  message traduit (`renommerProjet`, et le modal de creation faisait la meme fuite) ; et cote
  Rust, toute ecriture du nom d'un projet passe par `erreur_nom` (storage/projects.rs) qui
  nomme la cause. Regle : une contrainte de base n'est jamais un message d'interface.
- **UN LIEN NE PEUT PAS ETRE IMBRIQUE DANS UN `<button>`.** Le texte d'une tache est un
  `<button>` (le clic ouvre l'edition), et il doit aussi porter des adresses ouvrables. Un
  `<a>` la-dedans est du HTML invalide et le comportement n'est pas fiable. Les adresses sont
  donc des `<span class="lien" data-href="...">` DANS le bouton, et le clic est trie par
  `closest("[data-href]")` — meme technique que l'editeur de notes avec `closest("a")`. Ce
  n'est PAS une violation de la regle « un controle cliquable est un vrai `<button>` » : le
  controle, c'est le bouton exterieur ; le span n'est qu'un morceau de texte souligne. Ne pas
  le « corriger » en `<a>` ni en `<span role="link">`. Consequence assumee, la meme que dans
  les notes : il n'y a pas de chemin CLAVIER pour ouvrir l'adresse.
- **Un module frontend qu'on veut tester sous node ne doit RIEN importer de l'application.**
  `node --experimental-strip-types` execute un `.ts` sans outillage, mais il resout les imports
  comme node : `import { x } from "../api/workspace"` (sans extension, forme que Vite accepte)
  echoue en `ERR_MODULE_NOT_FOUND`. D'ou la coupe en deux : `utils/adresses.ts` est PUR (zero
  import) et teste par `npm run test:front` (`scripts/tests/*.test.mjs`, node:test, aucune
  dependance a installer), `utils/liens.ts` porte ce qui touche toasts/i18n/IPC et n'est
  couvert que par le banc frontend. Les tests vivent dans `scripts/` et pas dans `src/` :
  `tsconfig.json` n'inclut que `src/**` et `@types/node` n'est pas installe, donc un
  `import ... from "node:test"` sous `src/` ferait echouer `npm run check`.
- **Autolink : ne repere que ce qui est ouvrable TEL QUEL, et rogne la ponctuation finale.**
  `utils/adresses.ts` ne reconnait que `http://`, `https://` et les adresses mail — pas
  `www.exemple.com`, parce que `open_url` le refuserait et qu'on aurait souligne un lien mort.
  Deux cas qui piegent, verrouilles par des tests : « va voir https://exemple.com. » (le point
  n'est pas dans l'URL) et `https://fr.wikipedia.org/wiki/Deja_vu_(homonymie)` (la parenthese
  appariee, elle, en fait partie). Invariant teste aussi : la concatenation des segments rend
  toujours le texte saisi, au caractere pres.
- **`next_position_null(table, "id")` ne filtre RIEN : `WHERE id IS NULL` n'est jamais vrai.**
  `create_project_folder` l'appelait ainsi, donc chaque dossier naissait avec `position = 0` —
  verifie sur la base reelle le 2026-08-20 : 6 dossiers, tous a 0. Consequences invisibles a la
  lecture : l'ordre affiche retombait sur `name` (le `ORDER BY position, name` du SELECT) et
  `reorder_project_folders` etait INERTE, il ecrivait des positions qu'aucun tri ne distinguait
  ensuite. Depuis l'imbrication, la position se calcule PAR FRATRIE avec
  `WHERE parent_id IS ?1` — `IS` et non `=`, sinon la fratrie racine (parent NULL) ne se compte
  jamais. Regle : un helper de position qui prend un nom de colonne en parametre doit etre lu
  sur son APPEL, pas sur son nom.
- **`ON DELETE SET NULL` ajoute par `ALTER TABLE ADD COLUMN` EST bien applique par SQLite**
  (mesure du 2026-08-20 sur une copie de la base reelle : parent supprime -> `parent_id` de
  l'enfant passe a NULL, avec `PRAGMA foreign_keys=ON`). Le commentaire de
  `project_folders.rs` qui affirmait le contraire (« ALTER TABLE ne supporte pas ON DELETE SET
  NULL ») etait FAUX ; il justifiait un `UPDATE ... SET folder_id=NULL` fait a la main avant
  chaque suppression. La garde utile n'est pas la : c'est le REFUS de supprimer un dossier non
  vide. Prerequis rappele par la doc SQLite : la colonne ajoutee doit avoir `DEFAULT NULL`.
- **Un rendu recursif ne doit JAMAIS indenter par `padding-left` imbrique** : chaque niveau
  s'ajouterait au precedent et la barre laterale (largeur fixe, 260 px) deborderait a la
  troisieme profondeur. Le retrait se calcule en ABSOLU depuis la profondeur
  (`base + min(profondeur, 8) * 0.75rem`, comme l'arbre de l'onglet Fichiers) et les `<ul>`
  imbriques portent `padding-left: 0`. Le plafond a 8 niveaux n'est pas une limite de
  l'imbrication — seulement de l'indentation, pour garder un nom lisible.
- **Dans une liste imbriquee, `dragstart` et `dragover` REMONTENT jusqu'au parent glissable.**
  Une ligne projet vit dans le `<li>` de son dossier, lui aussi `draggable` : sans
  `stopPropagation()` sur `dragstart`, glisser un projet demarrait AUSSI le glisser de son
  dossier, et sans `stopPropagation()` sur `dragover`, deux retours visuels differents
  s'allumaient pour un seul depot (la ligne visee ET la zone qui l'englobe). Mesure au banc
  frontend le 2026-08-20. Corollaire : mettre les gestionnaires de depot sur l'EN-TETE du
  dossier, pas sur le `<li>` qui contient toute la branche — sinon survoler un petit-enfant
  vise l'ancetre.
- **Banc frontend : Chrome sans tete passe par le PROXY de la machine.** `--dump-dom
  http://127.0.0.1:<port>` a rendu une page d'un site CCM au lieu du banc (2026-08-20) : la
  config proxy du systeme s'applique aussi a `127.0.0.1`. Ajouter `--no-proxy-server`. Sans ca
  on croit a une erreur de build alors que le build est bon.
- **`scripts/release.mjs` doit bumper `Cargo.lock` en meme temps que `Cargo.toml`.** Sans ca le
  commit taggue se contredisait (lock en retard d'une version) et le premier `cargo build`
  suivant reecrivait le fichier : arbre sale, donc release suivante REFUSEE jusqu'a un commit
  manuel. Corrige le 2026-08-20, la substitution ne cible que le bloc de notre crate.

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
