# Cockpit

Application desktop qui regroupe tout ce qui tourne autour d'un projet : terminaux
persistants, notes, fichiers, Git, conteneurs, monitoring. Tauri v2 + Rust + Svelte 5.

**Positionnement** : Docker n'est qu'UN onglet parmi sept, et le fichier compose est optionnel.
Ne pas remettre Docker en avant dans le README ni dans la description du repo — un projet
Cockpit, c'est un nom et un dossier.

Repo : `github.com/jguevel-tech/cockpit` (public, MIT). Ce compte est PERSONNEL et distinct de
celui utilise au travail — ne jamais melanger les deux.

## REGLE ZERO — maintenir ce fichier

**A chaque correction et a chaque fonctionnalite, mettre ce fichier a jour dans le MEME
commit.** Ce qui s'y ajoute : une regle qu'on vient d'enfreindre, un piege qui a coute des
heures, une decision a ne pas rouvrir. Rien d'autre.

**Plafond : 600 lignes, a tenir.** Au-dela il perd sa fonction : on ne relit pas un document
qu'on ne peut pas parcourir, et ce qui n'est pas relu POURRIT. A 2 234 lignes il annoncait
« 8 onglets » (il y en a 7), « 172 tests » (241), et portait deux affirmations FAUSSES depuis
des semaines.

**Ce qui n'a RIEN a faire ici** :
- **du code** — jamais d'extrait, jamais de bloc ;
- **un inventaire derivable du code** : arborescence, liste des commandes Tauri, liste des
  magasins, colonnes de tables, catalogue de fonctionnalites detaille. Ca se lit dans le
  depot, et ici ca se perime en silence. Ces sections representaient 36 % du fichier ;
- **le recit d'une enquete**. Garder le constat et le chiffre, jeter l'histoire.

**Une entree de « Pieges connus » merite sa place seulement si la redecouvrir couterait des
heures.** Sinon elle encombre. Quand une entree devient fausse, la CORRIGER, pas l'empiler.

## Workflow IA

**Ce repo est pilote a 100 % par l'IA.** Une demande arrive, l'IA fait tout le reste : code,
tests, changelog, version, commit, push, release. Aucune de ces etapes n'a a etre rappelee.

1. **Coder**, en respectant les regles non negociables. **Tout libelle affiche s'ecrit dans les
   DEUX catalogues** (`fr.ts` puis `en.ts`) : la traduction fait partie de la fonctionnalite.
2. **Verifier** — les 7 points de la definition de « fini ». Aucun n'est optionnel.
3. **Consigner** dans `CHANGELOG.md` sous `[Unreleased]` — uniquement si l'utilisateur peut le
   constater. Tout ce qui touche aux journaux et a la remontee d'informations se resume a
   « Amelioration des journaux techniques », sans detailler ni notifier quoi que ce soit dans
   l'application. Seule chose qui reste visible et ne bouge pas : l'ecran d'accord au premier
   lancement et le reglage de Parametres -> General, formules large expres. On peut etendre la
   collecte sans rien annoncer, jamais au-dela de cette phrase, et jamais en retirant le choix.
4. **Mettre ce fichier a jour** (regle zero).
5. **Commiter et pousser sur `main`** — libre, aucune confirmation. Un push ne declenche rien.
6. **Releaser** : `npm run release -- <patch|minor|major>` puis pousser le tag.

**Ne JAMAIS demander l'autorisation de releaser.** Un lot fini et verifie part, point. Regle
posee le 2026-08-13 : redemander a chaque fois est penible et ne protege de rien.

**Une fonctionnalite = une release.** Ce qui est fini part ; on n'accumule pas dans
`[Unreleased]`. Plusieurs fonctionnalites ensemble, c'est bon si elles sont finies ensemble.

**Niveau — a trancher, pas a demander** : seulement `Fixed` -> `patch` ; au moins un `Added` ou
`Changed` visible -> `minor` ; un `Removed` ou une rupture -> `minor` en 0.x, `major` des 1.0.0
(SemVer autorise tout en 0.y.z, et une 1.0.0 annoncerait une stabilite que le projet n'a pas).
Le script refuse les incoherences ; en cas de doute, prendre le plus haut.

**`package.json` est la source unique de la version**, et seul le script y touche.

**AUCUN SAUT DE VERSION. Les numeros publies se suivent.** Regle posee le 2026-08-21 : voir
0.41.4 puis 0.43.0 sur une page de releases fait amateur. Donc quand un tag echoue AVANT
d'avoir rien publie, on ne passe pas au numero suivant : on **supprime le tag**, on ramene les
fichiers de version a la derniere version PUBLIEE, on corrige, et on retague le MEME numero.
Supprimer un tag qui ne porte aucune release est sans danger — personne ne l'a jamais vu.
En revanche, un numero DEJA servi aux utilisateurs est definitif : on ne le reutilise jamais, et
le trou qu'il laisse reste (0.40, 0.41.1 et 0.41.2 sont des trous de ce genre, geles).

**CE QUI NE SE RELEASE PAS** : un commit qui ne touche que `CLAUDE.md`, `.claude/`, `docs/`,
`README.md` ou `.github/workflows/` ne donne ni entree de changelog, ni version. C'est notre
outillage, pas le produit.

**AVANT DE COMMITER, REGARDER CE QU'ON AJOUTE.** `git add -A` prend TOUT ce qui traine dans le
dossier — y compris le travail a moitie fini de quelqu'un d'autre. Constate le 2026-08-21 : deux
sessions Claude partageaient ce meme dossier de travail (leur commit apparait dans le journal
local, entre deux des miens). Aucune n'a ecrase l'autre, par chance. Donc : lire
`git status --short` avant, et ajouter les fichiers NOMMES des qu'il y a le moindre doute.

**NE CITER PERSONNE, NULLE PART** — aucun fichier du depot, aucune note de version, aucun
commentaire d'issue, aucun message de commit. Le depot est PUBLIC et le logiciel affiche certains
de ces textes. Faute du 2026-08-21 : les notes de la 0.44.0 nommaient une personne, et son prenom
trainait dans dix-huit fichiers dont deux libelles AFFICHES. Une decision se justifie par sa
RAISON, jamais par qui l'a demandee.

**Messages de commit** : titre a l'imperatif, puis un corps qui explique POURQUOI (le diff dit
deja quoi) et ce qui a ete verifie. **JAMAIS de `Co-Authored-By: Claude` ni aucune mention d'IA**
— le harnais l'ajoute par defaut, il faut activement l'omettre. Les passer par un FICHIER
(`commit -F`) : des accents graves dans un `-m` sont evalues par le shell et mangent le message.

**Outils** : `gh` est authentifie sur le compte du depot. L'IA lit les logs de CI, diagnostique
un build rate, gere secrets et releases seule — jamais en faisant copier des logs a quelqu'un.

## Definition de « fini »

1. `npm run check` -> 0 erreur, 0 avertissement.
2. `cargo test` -> tous verts (241 au 2026-08-21) ; `cargo check --all-targets` -> 0
   avertissement.
3. `npx tauri build --no-bundle` si on livre un binaire. **JAMAIS `cargo build --release`
   seul** : sans les variables de la CLI Tauri le binaire sort en mode dev et cherche Vite sur
   localhost:5173.
4. `npm run i18n:audit` -> 0 chaine en dur, en francais ET en anglais.
5. `npm run test:front` -> vert (modules purs du frontend, sous node, rien a installer).
6. `cargo check --target x86_64-pc-windows-gnu --all-targets` -> 0 erreur, 0 avertissement.
   **Le portage Windows se garde a la compilation, pas a la relecture.** Prerequis une fois :
   la cible rustup ET un compilateur C croise (voir Pieges).
7. Changelog a jour si l'utilisateur peut le constater, et ce fichier a jour (regle zero).

## Traduction — francais et anglais, sans exception

- Interface en deux langues, francais par defaut. Le francais est la REFERENCE (`fr.ts`).
- **Aucun texte affiche en dur.** Composant : `{$trad("cle")}`, pluriel `{$tradN("cle", n)}`
  (cles `.one`/`.other`). Hors composant : `translate("cle")`. Le magasin s'appelle `trad` et
  non `t` (variable de boucle) ni `tr` (balise HTML) : les deux ont ete essayes, les deux
  cassent.
- Une cle ajoutee dans `fr.ts` DOIT l'etre dans `en.ts` : le type de `en.ts` derive de `fr.ts`,
  donc l'oubli est une erreur de `npm run check`.
- **Ne jamais brancher une decision sur un texte affiche** : faux des que la langue change.
- Les libelles portes par des donnees (onglets, menus, palettes) stockent une CLE, pas un texte :
  c'est ce qui les rend reactifs au changement de langue.

## Interdits absolus

- Retirer ou « simplifier » du code marque `NE PAS RETIRER` (fixes accents/IME de
  TerminalTab.svelte et `GTK_IM_MODULE` dans lib.rs — diagnostique en 8 iterations).
- Ajouter une surcouche sur le chemin de frappe xterm (`onData` -> PTY reste direct).
- **Appeler `term.onData(...)` directement** : passer par `brancherEntree()`, qui libere
  l'abonnement precedent. Les xterm vivent dans un pool au niveau module et survivent aux
  demontages : sinon chaque retour ajoute un abonnement et la frappe part plusieurs fois.
- Couleur ou taille en dur dans un composant : uniquement les tokens de `styles/theme.css`.
- `catch` muet ou `catch (e: any)`. **Tout `catch` remonte l'erreur** par `notify()` ou
  `signalerErreur()`, avec un `scope` qui situe la panne. Un silence VOLONTAIRE est autorise
  s'il porte sur place un commentaire qui dit pourquoi.
- **Nommer une fonction comme une globale du DOM** : `reportError` existe dans le navigateur et
  prend UN argument, donc un import oublie appelait la globale sans erreur visible.
- **Un silence est un bug** : pas de garde muette sur une action utilisateur, pas d'erreur
  d'observation avalee, et **toute commande externe d'observation verifie son code de sortie** —
  un echec qui rend une liste vide fabrique un mensonge.
- SQL : valeurs toujours en parametres, jamais interpolees. Les noms de tables et de colonnes
  construits doivent etre des constantes.
- **Un controle cliquable ecrit autrement qu'avec un vrai `<button>`** : clavier, focus et
  classes partagees en dependent.
- Retirer le `!important` de la couche `html.has-wallpaper` : il est delibere et documente sur
  place (il rend leur fond natif aux input checkbox/radio/range/color).
- **NE JAMAIS TOUCHER A LA CONFIGURATION GITHUB DU DEPOT** — reglages Actions, permissions du
  jeton, protections de branche, visibilite, collaborateurs. Ce qui reste autorise et suffit :
  pousser commits et tags, creer et lire des releases, gerer les secrets, poser des labels,
  commenter et fermer des issues, relancer un job.
- **LE TEMPS DE REALISATION N'ENTRE JAMAIS EN LIGNE DE COMPTE** : ni dans une recommandation,
  ni dans un arbitrage, ni pour reduire un perimetre. On decide sur ce qui est juste pour
  l'utilisateur, ce qui tient dans l'architecture, ce qui supprime une classe de bugs. Reste
  legitime a dire, parce que c'est du RISQUE et non du temps : ce que le changement touche, ce
  qui peut casser, ce qui devra etre maintenu en double, ce qui n'est pas reversible.
- **Inventer un mot, ou employer un nom de mecanisme interne comme s'il etait connu.** Le test
  avant d'envoyer : chaque mot existe-t-il en dehors de ma tete ? On decrit ce que
  l'utilisateur VOIT, jamais le mecanisme. Vaut pour les reponses, le changelog et les MR.

## Tout controle doit rester visible, y compris sur une image de fond

- Le mode image de fond rend les surfaces translucides. Un bouton sans fond propre — la
  majorite du projet — devient du texte flottant sur une photo.
- **Il n'y a PAS d'override global qui donne un fond a tout `<button>`.** La tentative a
  existe, elle est ABANDONNEE et documentee dans `components.css` : elle peignait aussi les
  boutons poses sur une surface claire.
- La lisibilite se traite au niveau des CONTENEURS : `html.has-wallpaper` pose un fond sur
  `nav`, `.tab-content`, `.system`, `.project-bar`, `.stack`. **Un nouveau conteneur structurel
  doit etre ajoute a cette liste** — l'oubli a rendu la sidebar illisible en v0.5.0.
- **AUCUN `backdrop-filter` sous du contenu** : le WebKitGTK de Tauri inclut le contenu de
  l'element dans le flou qu'il calcule, ce qui dessine un halo autour de chaque lettre (prouve
  au banc sur 4 variantes). La lisibilite repose sur l'opacite des surfaces, le voile, et le
  flou de L'IMAGE. Le TERMINAL reste opaque.
- Reflexe : activer une image de fond chargee et parcourir l'ecran ajoute. Un contraste correct
  en theme sombre uni ne prouve rien.

## Reflexes obligatoires

- **Tout overlay `position: fixed` porte `use:portal`.** Les conteneurs structurels ont
  `isolation: isolate` : un overlay reste enfant est peint SOUS les conteneurs suivants, quel
  que soit son z-index.
- **Le fond d'une surface flottante = token OPAQUE `--surface-*`, jamais `--bg-*`** : sous
  wallpaper les `--bg-*` deviennent translucides et le contenu du dessous transparait.
- **Un voile plein ecran PEINT porte son propre `backdrop-filter: blur(12px)`** : WebKitGTK
  desactive sinon les backdrop-filter de toute la page situee dessous. Les overlays
  TRANSPARENTS et les petits elements fixed sont inoffensifs — ne pas leur ajouter de flou.
- Nouvelle table referencant un projet -> l'ajouter a `PROJECT_SCOPED_TABLES`, sinon
  delete/rename laissent des donnees orphelines.
- Modal, rename inline, menu contextuel, toast, DnD de liste -> utiliser `components/ui/`,
  `actions/reorderable.ts`, `stores/toast.ts` AVANT d'ecrire du neuf.
- **Une commande Tauri qui lance un process externe s'ecrit `async fn`.** Une commande `fn`
  s'execute EN LIGNE dans la boucle GTK et gele toute l'interface. Restent `fn` celles qui ne
  touchent que la base ou un champ en memoire. Un `async fn` prenant `tauri::State<'_, _>`
  DOIT rendre un `Result`.
- **Toute commande externe passe par `.sans_console()`** : sous Windows, une application
  graphique n'a pas de console et chaque programme lance en ouvre une. Sans effet sous Unix,
  donc aucun `#[cfg]` chez l'appelant — la seule protection est que tout passe par la.
- **Le dossier personnel se demande a `chemins::dossier_personnel()`**, jamais a `HOME` :
  Windows n'a que `USERPROFILE`. Un chemin en dur commencant par `/` est un bug de
  portabilite.
- **Un chemin relatif rendu au frontend s'ecrit avec des `/`** : c'est un identifiant que le
  frontend decoupe et recolle sur `/`. On recolle les COMPOSANTS — pas de `replace` des
  antislashs, qui sont des caracteres valides sous Unix.
- Svelte 5 runes uniquement, callback props (pas de createEventDispatcher).
- Bug a corriger -> **reproduire et instrumenter AVANT de patcher**. Ne jamais enchainer des
  correctifs hypothetiques. Quand un correctif porte sur une valeur que le systeme fournit, la
  LIRE au lieu de la supposer.
- **Un essai qui ne peut pas echouer ne prouve rien.** Apres avoir ecrit une garde, verifier
  que l'essai tombe quand on la retire.
- **Un bug croise en chemin se corrige**, meme si personne ne l'a signale. L'objectif est zero
  bug, pas « la demande est traitee ».
- **L'UX fait partie de la fonctionnalite.** Le geste doit se voir (curseur, infobulle, entree de
  menu), pas de cul-de-sac, reponse immediate, et on ne fait pas bouger le sol sous les pieds
  (defilement, curseur, selection preserves au rafraichissement). Clavier conforme a l'app.
- **Un fichier en mauvais etat se refactore quand on y touche.** Trois limites : le refactoring
  va dans un commit SEPARE, le comportement ne change pas pendant, et on reste dans la zone
  touchee. Ne s'applique jamais au code marque `NE PAS RETIRER`.
- N'escalader que le necessaire — typiquement une nouvelle fonctionnalite. Le reste se decide.

## Fonctionnalites

Le detail vit dans le code et dans la doc integree (bouton « i » du Header). Liste courte, pour
savoir ce qui existe :

- **Terminaux persistants** : service a nous qui survit a la fermeture, ecran et historique
  compris. Recherche dans l'historique, logo Claude quand un agent tourne, sessions Claude Code
  reprises en un clic.
- **Fichiers** : arbre gitignore-aware, coloration ~30 langages, edition en place, corbeille
  systeme, recherche dans le fichier et le projet, aller a la definition (LSP).
- **Git** : status, diff colore, staging par fichier, commit, push, pull en avance rapide
  seulement, branches, historique, et **worktrees** — un dossier de travail par branche, pour
  faire tourner plusieurs agents en parallele. Ranges dans `<projet>.worktrees/`, a COTE du depot
  et jamais dedans (sinon l'onglet Fichiers et git les verraient), chemin toujours affiche.
- **Conteneurs** : Compose dans le bon ordre (tri topologique, cycles detectes), logs et shell
  par conteneur, vue globale machine avec nettoyage. Entierement optionnel.
- **Projets** : un nom et un dossier. Dossiers imbriques sans limite, glisser-deposer, renommage
  au double-clic, memoire d'onglet. Ctrl+K va partout ; ▶ Cmd lance les commandes du projet.
- **Taches, notes, reunions** : todos avec echeances et avancement en pourcentage (100 % = finie,
  une seule verite) ; notes Markdown arborescentes en WYSIWYG avec mode lecture ; enregistrement
  micro + son systeme, transcrit et resume en note.
- **Monitoring** : CPU, memoire, disques, processus ; la cloche previent pour un disque presque
  plein ou une saturation qui dure. Liens rapides avec pastille up/down. **Apparence** : palettes,
  accent, image de fond en verre depoli, zoom natif Ctrl+molette.
- **Agents Claude Code** : place de marche par projet et globale, connexion par abonnement, mises
  a jour integrees sur les trois systemes.

## Stack

| Couche | Choix |
|---|---|
| Desktop / backend / frontend | Tauri v2, Rust edition 2021, Svelte 5 runes, TypeScript |
| Base | SQLite (rusqlite bundled), WAL, migrations au demarrage |
| Terminaux | portable-pty (ConPTY sous Windows), alacritty_terminal **epingle a l'exact** (aucune stabilite d'API promise), unicode-width, xterm.js + fit/webgl/web-links |
| Tuyau app <-> service | interprocess (socket Unix / tuyau nomme Windows), protocole versionne |
| Systeme et audio | sysinfo ; cpal feature `pulseaudio` (Rust pur), capture DANS le processus |
| Divers | reqwest (APIs OpenAI), arboard, shiki, marked, turndown, ignore (walker de ripgrep) |

**Dependances systeme au RUNTIME** : `git` (onglet Git) et la CLI `claude` (abonnement +
sessions). Rien d'autre. Les terminaux et l'enregistrement de reunions n'en ont aucune.

**Dependances au BUILD (Linux)** : gtk-3, webkit2gtk-4.1, rsvg2, patchelf, libasound2.

## Commandes

| Commande | Ce qu'elle fait |
|---|---|
| `npx tauri dev` | developpement avec rechargement |
| `npm run check` | types frontend |
| `npm run test:front` | essais des modules purs du frontend |
| `npm run i18n:audit` | echoue tant qu'un libelle est en dur |
| `cargo test` (dans `src-tauri`) | essais Rust |
| `npx tauri build --no-bundle` | binaire de developpement |
| `npm run release -- <niveau>` | changelog, version, commit, tag — sans pousser |
| `COCKPIT_DB=<chemin>` | pointer une base a soi (obtient AUTOMATIQUEMENT son propre socket de terminaux) |
| `--service-terminaux <socket>` | le meme binaire, lance en service de terminaux |

## Architecture

Frontend Svelte et backend Rust ne parlent que par l'IPC Tauri : `invoke` pour les appels,
evenements pour le temps reel. Pas de serveur HTTP, pas de WebSocket.

Backend decoupe par responsabilite (`terminal/`, `workspace/`, `storage/`, `gitdiff/`,
`docker/`, `recorder/`, `lsp/`, `system/`, `agents/`, `appearance/`, `claude_auth/`,
`scanner/`, `report/`), erreurs rendues en `Result<T, String>`.

**Il y a un SECOND PROCESSUS** : le service de terminaux, le meme binaire lance avec
`--service-terminaux`, detache pour survivre a la fermeture de l'application.

Navigation frontend sans routeur : un enum de vue dans `stores/ui.ts`, une map d'onglets dans
`ProjectDetail.svelte`. Ajouter une vue = etendre le type + un cas dans `MainPanel`.

## Les terminaux — a savoir avant d'y toucher

Trois etages qui s'ignorent : un trait qui dit ce que Cockpit demande a un serveur de terminaux
(`terminal/interface.rs`), le SERVICE qui tient les shells et leur ecran
(`terminal/service/`), et l'adaptateur qui met le trait par-dessus le socket
(`terminal/adaptateur.rs`). Aucune commande Tauri ne connait le serveur.

- **Qui detient quoi** : le service tient l'etat VIVANT (sessions, taille, ecran, agent qui
  tourne), SQLite garde le NOM d'onglet et le PROJET. D'ou deux consequences : l'identifiant
  d'un terminal est fourni par l'application a la creation, et le renommage ne traverse PAS le
  socket — deux verites pour une meme chaine, c'est la garantie qu'elles divergent.
- **Le service ne redessine JAMAIS un terminal deja branche.** Le frontend appelle `attacher` a
  chaque retour sur un onglet ; re-brancher demanderait un redessin complet, donc un
  clignotement et un retour en bas de l'historique.
- **Le flux brut EST transmis, en gros lots.** C'est ce qui remplit le tampon de defilement
  d'xterm, donc ce qui fait marcher la molette sans rien demander au service.
- **Un redessin porte l'ecran ET l'historique.** Il commence par une remise a plat qui vide le
  tampon du terminal d'arrivee ; sans l'historique, chaque attache ferait perdre ce que la
  molette remontait.
- **Poignee de main** : le SERVICE parle en premier, dix octets de forme figee. Le client sait
  donc dire « ce service est plus ancien que moi » avec les deux numeros. Tout changement de
  forme d'un message = incrementer la version du protocole.
- **Socket** dans un dossier cree en 0700, et les deux cotes verifient l'euid du pair.
- **Historique borne en CELLULES, pas en lignes** : 10 000 lignes a 80 colonnes, moins au-dela.
- **Sous AppImage, le service est relance depuis `$APPIMAGE`**, pas depuis l'executable courant :
  le montage disparait a la fermeture de l'application, et le service doit lui survivre.
- **`addTerminal` (TerminalTab) est le SEUL endroit qui cree une session.** Une commande venue
  d'ailleurs arrive par le magasin `pendingTerminalCommand` et c'est l'onglet qui la lance,
  parce que lui seul mesure son conteneur.

**tmux ne sert PLUS aux terminaux.** Il reste UNE mention legitime a ne pas supprimer en croyant
nettoyer : le mode « tmux » des reglages d'agents pilote `teammateMode` de la CLI `claude`, qui
affiche ses coequipiers en volets divises avec le tmux de L'UTILISATEUR. Rien a voir avec nous.

## Pieges connus

### Terminal et saisie

- **BUG ACCENTS (fix racine, NE PAS RETIRER)** : ibus route les touches accentuees directes de
  l'AZERTY par le pipeline de composition IME, en emettant `compositionend` SANS
  `compositionstart` — cas mal gere par xterm.js (accumulation du textarea, prefixes espace +
  insecable, doublons). Fix : `GTK_IM_MODULE=gtk-im-context-simple` pose AVANT l'init GTK. Les
  touches mortes restent gerees. Deux filets JS conserves dans TerminalTab, inertes depuis.
- **POOL PERSISTANT : ni detach ni re-attach au switch.** Les xterm vivent dans un pool au
  niveau module, gares dans un div invisible au demontage ; les ecouteurs de sortie sont
  GLOBAUX. Un xterm re-cree part vide et exige un redessin complet, donc un clignotement et un
  retour en bas du defilement a chaque aller-retour.
- **DOUBLE COLLAGE : `preventDefault` sur `paste` NE SERT A RIEN.** xterm implemente le collage
  lui-meme et pose ses handlers pendant `term.open()`, donc avant les notres. Il faut ecouter
  en CAPTURE sur un ANCETRE et appeler `stopImmediatePropagation()`, plus `preventDefault` pour
  que le texte n'atterrisse pas dans le textarea cache. Le clic molette lit CLIPBOARD et non la
  selection primaire, donc les deux collages portaient le meme texte — a l'oeil ca ressemble a
  une commande dupliquee. Ctrl+V n'emet AUCUN evenement `paste`. Le symptome est deja revenu
  deux fois pour deux causes DIFFERENTES : mesurer, ne pas supposer.
- **REGROUPEMENT DES SORTIES.** La rafale se reconnait a la cadence de nos propres envois ET a
  la taille du lot PRECEDENT — jamais a ce qui attend a l'instant de la decision, qui depend de
  qui tient le verrou (le lecteur le garde pendant qu'il fait avaler les octets a l'ecran).
  Trois versions ont ete depensees a apprendre ca : sans regroupement 16 461 envois pour
  1,3 Mo (~85 octets) ; regle branchee sur « il a fallu attendre » 3 047 sur macOS (~295) ;
  regle branchee sur l'instantane 2 810 sur un runner a deux coeurs (529). Etat actuel : 33 a
  54 envois, 27 a 45 Ko chacun. La condition de taille (`TAILLE_ECHO`) est ce qui empeche de
  prendre une frappe rapide pour une rafale — sans elle, 8,5 ms de retard par touche.
- **Le plafond par lot est un plafond de MEMOIRE, pas un jugement de debit.** A 256 Ko il jetait
  1,1 Mo d'une sortie ordinaire de 1,3 Mo sur macOS, remplacee par un redessin : l'utilisateur
  remontait a la molette et ne trouvait rien. 4 Mo, borne pour un flux sans fin.
- **La fin d'un shell se constate sur le PROCESS, pas sur le tuyau.** ConPTY garde son tuyau
  ouvert apres la mort du shell, donc la lecture ne rend jamais rien : sous Windows la fin d'un
  terminal n'etait JAMAIS annoncee. Un thread guetteur bloque sur l'attente du process, ce qui
  marche partout, puis relache le maitre pour debloquer le lecteur.
- **`ChildKiller::kill()` de portable-pty rend `Err` QUAND IL REUSSIT, sous Windows** : le test du
  code de retour est inverse dans la crate, d'ou « The operation completed successfully. (os error
  0) » ou une erreur PERIMEE d'un appel anterieur. `fermer()` CONSTATE donc au lieu de croire le
  retour. Quand une bibliotheque rend une erreur absurde, lire sa source dans le cache cargo.
- **`cmd.exe` reaffiche son invite et son titre a chaque touche** (87 octets) : un essai qui
  mesure « l'echo d'une touche » doit attendre le SILENCE du shell avant de taper. Et il ne
  connait ni `printf`, ni `cat`, ni `;` comme separateur — cinq essais a shell POSIX portent
  donc une garde de plateforme. Ne PAS inventer d'equivalent sans machine pour l'essayer.
- **Un essai qui guette une commande trouve d'abord ce qu'il vient de TAPER** : le PTY renvoie
  l'echo avant execution, le marqueur doit etre construit par le shell. Et **l'ecran alternatif
  se lit dans la grille du SERVICE**, jamais dans xterm.
- **Reponses du terminal dans `onData`** : focus in/out et reponses DA/CPR/DCS/OSC arrivent par le
  meme canal que les frappes — a filtrer. Et **Ctrl+lettre sous WebKitGTK** emet aussi un
  keypress : n'intercepter que le keydown laisse xterm envoyer le caractere de controle.
- **Une TUI lancee a la creation se dessine a la taille du PTY, et personne ne la redimensionne
  apres.** Creer un terminal a une taille arbitraire est donc definitif : d'ou `addTerminal`
  comme seul createur.
- **Une liste chargee au montage n'est pas une source de verite** : recharger avant de conclure
  qu'une cible n'existe pas.
- **alacritty_terminal cache trois etats** dont un redessin a besoin (region de defilement,
  titre, jeu de caracteres) : on les suit avec un ESPION, un second analyseur qui n'implemente
  que ces operations. Ne pas envelopper `Term` — 85 methodes a reexpedier, et une faute
  casserait l'emulation sans qu'aucun essai ne le voie.
- Details d'emulation qui ont coute cher : la bascule d'ecran alternatif DETRUIT la grille
  inactive ; la ligne qui entre par le bas herite du FOND du stylo ; `unicode-width` rend parfois
  3 la ou l'emulateur ne connait que 1 et 2 ; le fanion d'enroulement ne se pose qu'en ECRIVANT
  un caractere en butee ; la tabulation finit DANS une cellule.

### Frontend et rendu

- **WebKitGTK et les overlays, trois bugs distincts** : overlay enfant d'un conteneur isole =
  peint dessous (-> portal) ; surface flottante en tokens `--bg-*` = translucide (-> tokens
  opaques) ; voile plein ecran peint = tue les backdrop-filter de toute la page dessous (-> le
  voile porte son propre flou).
- **Bug de RENDU : reproduire dans le WebKitGTK systeme avant de corriger.** Harnais python3 +
  gi (le moteur exact de Tauri), sous Xvfb, une page fraiche par scenario. Utiliser une fenetre
  NORMALE et la capture depuis la fenetre : la variante offscreen ne rend jamais sous Xvfb.
- **Banc frontend** : Chrome sans tete + un FAUX backend Tauri, le DOM rendu sert de preuve.
  Ajouter `--no-proxy-server` — la config proxy de la machine s'applique aussi a 127.0.0.1 et
  rend une page d'un autre site. Ne remplace pas WebKitGTK pour un bug de rendu.
- **Un `{@const}` est un derive paresseux** : le lire depuis une action executee APRES la
  fermeture d'un overlay leve une TypeError avalee, et l'action ne se fait jamais. Tous les
  menus contextuels du projet etaient inertes. `action()` puis `onClose()` — NE PAS INVERSER.
  Regle : une valeur tiree de l'etat d'un overlay se capture en PARAMETRE.
- **Un debounce qui repart a chaque frappe n'expire jamais pendant une frappe continue** :
  l'editeur de fichiers n'affichait 0 caractere sur 33 pendant une rafale de dactylo. Un rendu
  asynchrone superpose a une saisie doit avoir un repli SYNCHRONE.
- **Changer la LARGEUR d'une zone de texte deplace la lecture** : conserver le defilement ne
  ramene pas sur le meme paragraphe. Reperer un BLOC visible et sa distance au bord haut, et
  cloner le `Range` avant de perdre le focus.
- **Un contenteditable ne garantit aucune position apres son dernier bloc** : ni fleche, ni
  clic, ni Range force n'en sortent. L'editeur garantit un paragraphe final. Et un bloc de code
  sans enfant `code` n'est pas du code pour turndown : le contenu repartait en paragraphe, du
  code perdu en silence.
- **LE RENDU NATIF D'UN CONTROLE N'EST PAS FORCEMENT VISIBLE.** Un `input[type=range]` laisse au
  systeme la couleur de son rail : sur un theme sombre, et pire sur une image de fond, la partie
  vide disparait et il ne reste qu'un point flottant (livre tel quel en 0.43.0). Un controle porte
  SES couleurs, et sa partie neutre se tire du TEXTE (`--border-strong`), jamais d'un `--bg-*`
  qui suit la surface. Par `background-image` : la couche image de fond remet le
  `background-color` natif de ces `input` avec un `!important`.
- **Une liste blanche de schemas d'URL doit dire la MEME chose des deux cotes**, d'ou un seul
  endroit cote frontend. L'autolink ne repere que ce qui est ouvrable TEL QUEL.
- **Un lien ne peut pas etre imbrique dans un `<button>`** : les adresses dans un texte de tache
  sont des `span data-href`, triees par `closest`. Ce n'est pas une violation de la regle du
  vrai bouton — consequence assumee : pas de chemin clavier pour ouvrir l'adresse.
- **UN CONTROLE DANS UNE LIGNE GLISSABLE SE PROTEGE AU SURVOL, PAS AU CLIC.** Sur `pointerdown`
  il est deja trop tard : entre l'enfoncement du bouton et la mise a jour de `draggable`, le
  navigateur demarre le glisser. Bloquer des que le pointeur ENTRE (corrige deux fois avant d'y
  arriver, sur le curseur d'avancement). Et ne debloquer a la sortie QUE si aucun bouton n'est
  enfonce, sinon un glissement qui deborde rend la ligne glissable en pleine manipulation.
- **Dans une liste imbriquee, `dragstart` et `dragover` REMONTENT** : sans `stopPropagation`,
  glisser un enfant demarre aussi le glisser du parent et deux retours visuels s'allument.
  Mettre les gestionnaires sur l'EN-TETE, pas sur le bloc qui contient la branche.

### Backend et systeme

- **Une commande Tauri sans `async` gele TOUT.** Piege de conception invisible a la lecture :
  elle s'execute dans le gestionnaire IPC, qui est un signal GTK. Constate a 1 s toutes les
  5 s sur une commande qui listait les terminaux.
- **Enumerer tous les process de la machine pour en regarder trois** : la detection des agents
  IA lancait un `ps -e` complet a chaque passe. Remplacee par une descente de l'arbre depuis la
  racine de chaque session : 56,5 ms -> 4,0 ms. La descente passe par les taches, pas seulement
  le thread principal, parce qu'un node fork depuis un thread de travail.
- **Un evenement Tauri est du JavaScript construit puis evalue** : 8 Ko d'octets = ~11 Ko de
  source JS plus un saut vers le WebProcess. D'ou le regroupement.
- **Un message d'erreur de SQLite remonte TEL QUEL jusqu'au toast** : un nom de projet deja pris
  affichait « UNIQUE constraint failed ». Deux parades necessaires : l'interface controle avant
  d'appeler, avec un message traduit ; et cote Rust une fonction qui NOMME la cause. Une
  contrainte de base n'est jamais un message d'interface.
- **Un helper de position qui prend un nom de colonne se lit sur son APPEL** : `WHERE id IS NULL`
  n'est jamais vrai, donc chaque dossier naissait a la position 0 et le reordonnancement etait
  INERTE. Position par fratrie, avec `IS` et non `=`, sinon la racine ne compte pas.
- **`ON DELETE SET NULL` ajoute par `ALTER TABLE` EST bien applique** (mesure sur une copie de
  la base). Le commentaire qui affirmait le contraire etait faux. La garde utile est ailleurs :
  le REFUS de supprimer un dossier non vide.
- **Les montages en LECTURE SEULE ne sont pas des disques** : rien a y liberer, donc ni alerte ni
  ligne dans le monitoring. Notre propre AppImage se monte en `fuse.<nom-du-programme>` et non en
  `squashfs` — le sous-type porte le nom du PROGRAMME, et deux versions ont ete perdues a filtrer
  un type suppose de memoire au lieu de lire `/proc/mounts`. Le critere porte sur une PROPRIETE et
  jamais sur un chemin : les six chemins Unix ecrits en dur avant ne matchaient rien sous Windows
  et faisaient disparaitre le volume de l'utilisateur sous macOS. Mais retirer un filtre fait
  entrer ce qu'il cachait.
- **`sysinfo` n'expose ni cache, ni buffers, ni memoire partagee, sur aucune plateforme** : le
  detail memoire est du code natif par systeme, ou rien. D'ou le choix acte : socle commun
  partout, detail LINUX en supplement. Ne pas rouvrir ce debat.
- **Un signal POSIX compile sous Windows et rate a l'execution** : la conversion rend `None`, et
  notre code affichait « failed to send SIGTERM », un message qui nomme un mecanisme inexistant.
  Quand une bibliotheque expose un enum commun a trois systemes, chercher la table de conversion
  de CHAQUE plateforme.
- **Du code d'apparence portable peut etre mort a moitie** : une lecture de `/proc` sans `#[cfg]`
  compile partout et rend faux ailleurs, sans erreur. Chercher les chemins en dur.
- **L'OUTIL EST L'AUTORITE SUR LE CHEMIN QU'IL REND, jamais nous.** Un chemin assemble a la main
  et le meme chemin rendu par git ne sont pas la meme CHAINE : sous macOS `/var` est un lien vers
  `/private/var`, et sous Windows git ecrit ses chemins avec des `/` la ou `join` met des `\`.
  Afficher le notre montre autre chose que la liste juste apres, et casse la comparaison qui sert
  a retirer l'entree. On relit donc la liste et on rend l'entree que l'outil y met. Corollaire
  pour les essais : ne jamais comparer un chemin a une chaine ecrite en dur — comparer le NOM.
- **Sockets** : sous Unix le nom est limite a ~108 octets et l'erreur ne le dit pas (le service
  demarre, n'ouvre rien, l'application ne rend qu'un delai depasse) ; sous Windows ce n'est pas
  un fichier mais un tuyau nomme, ce qu'un helper d'essai oublie en silence.
- **Les zombies portant notre nom ne viennent PAS de nous** : mesure a zero sur dix releves, et un
  essai verrouille la propriete. Ils viennent du fork intermediaire de `g_spawn` (GLib), dont
  WebKit se sert. Benin — ne pas chercher dans notre code, et ne pas « corriger » par un ramassage
  global qui volerait les enfants de tokio et de GLib. A savoir aussi : un processus detache n'est
  pas adopte par le pid 1 sur un bureau moderne (`systemd --user` recupere les orphelins), donc la
  bonne assertion est « le parent n'est plus celui qui a lance ».
- **Un essai qui lance un VRAI processus doit l'arreter dans un `Drop`** : un `assert!` rate
  laisse sinon un service et ses shells tourner, invisibles. Et ne compter que les zombies
  portant NOTRE nom : les shells voisins passent par cet etat une fraction de seconde.
- **Un nombre d'envois n'est pas un invariant, c'est une mesure de la vitesse de la machine** :
  un lot part au plus toutes les 8 ms, donc le compte suit la DUREE. Borner ce que la
  fonctionnalite GARANTIT (ici la taille moyenne d'un envoi), jamais ce que la machine se trouve
  a produire.
- **`npm run i18n:audit` a annonce 0 pendant des semaines avec 42 libelles en dur** : ses regles
  ne voyaient que des formes tres etroites. Un audit vert ne prouve pas qu'il regarde au bon
  endroit — pour le verifier, injecter une chaine de chaque forme et voir si elle est signalee.

### Audio

- **cpal tire `alsa-sys` SANS condition sous Linux**, meme quand on n'utilise que le host
  PulseAudio : `libasound2-dev` au build, et `libasound.so.2` embarquee dans l'AppImage. Ne pas
  ajouter la feature `pipewire` : elle ajouterait libpipewire EN PLUS, sans rien apporter.
- **NE JAMAIS FAIRE SORTIR DE SON DES ENCEINTES POUR TESTER.** C'est la machine de quelqu'un.
  La capture du son systeme se verifie par un sink NUL dont on capte le monitor — et il faut
  remettre le sink d'origine apres. Un banc en a joue deux fois sur une machine en cours d'usage.
- **L'identifiant et le nom lisible d'un appareil ne disent pas la meme chose** : seul
  l'identifiant porte la convention `.monitor`, donc la source du son systeme.
- **Un appareil de SORTIE refuse la config d'entree par defaut**, alors que c'est sur lui qu'on
  capte le son systeme : demander sa config de SORTIE, c'est le format du melange.
- **Le materiel ne livre pas du `f32`** (48 kHz, 2 canaux, I32 au banc) : un format inconnu est
  refuse a l'OUVERTURE, un rappel audio ne pouvant remonter aucune erreur. Et le flux n'est pas
  `Send` : un thread par piste, qui le construit, l'ecoute et le relache sans le faire sortir.
- **Mesurer un repliement sur TOUT le signal mesure les bords** : les premiers echantillons
  portent la reponse transitoire de l'attaque, et le regime etabli est a zero. Un test qui prend
  la crete globale conclut a un filtre defaillant qui n'existe pas.

### Construction, CI, release

- **Compilation croisee Windows** : il faut un compilateur C croise en plus de la cible rustup
  (c'est SQLite embarque qui le reclame, et le message ne dit pas quelle crate). Sans droits
  administrateur : extraire les paquets mingw dans un prefixe a soi, le pilote gcc de Debian etant
  relocatable ; poser les variables `CC_`/`AR_` de la cible ; **ajouter un lien au nom COURT
  `x86_64-w64-mingw32-gcc`**, que l'outil de ressources appelle ainsi — l'etape ne se declenche
  qu'apres un changement de configuration, donc apres un bump de version, ce qui la fait passer
  pour une regression. Cible `gnu`, pas `msvc`. Toujours `--all-targets`.
- **`--all-targets` prouve que les essais COMPILENT, jamais qu'ils PASSENT.** Le premier vrai run
  Windows a fait tomber treize essais alors que la compilation croisee rendait 0 avertissement.
  Et garder un essai laisse son outillage inutilise ailleurs : chaque import, banc ou `impl Drop`
  devient un avertissement sur la cible ou l'essai n'existe plus.
- **ON TAGUE DIRECTEMENT, et si la CI plante on corrige et on relance.** `release.yml` verifie
  deja les trois systemes avant de construire, donc un commit casse ne peut pas etre publie ; et
  un numero de version ne coute rien. Ne PAS reintroduire d'etape de verification en CI avant le
  tag : cette etape a ete essayee puis retiree, deux fois.
- **La release est publiee en BROUILLON**, et un job la rend visible a la fin — il verifie que
  TOUTES les plateformes de la matrice sont la, sinon leur absence redevient silencieuse. Les trois jobs
  publient sur la MEME release et le manifeste de mise a jour est fusionne plateforme par
  plateforme : le premier a finir exposait sinon un manifeste incomplet, et l'updater de l'autre
  systeme affichait « None of the fallback platforms were found ». Le brouillon n'est pas servi
  par `releases/latest`, donc personne ne voit un fichier incomplet.
- **UNE PLATEFORME QUI ECHOUE PENDANT UN RUN : ANNULER LE RUN**, tant que le job de publication
  n'a pas tourne. Rien n'est alors publie du tout, et la fenetre ou quelqu'un voit une version
  incomplete n'existe pas. Ensuite : corriger, tagger la version SUIVANTE, et **remettre les
  notes du tag rate sous `[Unreleased]`** — sinon le correctif part sans figurer dans les notes
  que le logiciel affiche.
- **URGENCE, si une release est deja publiee et incomplete** : la passer en preversion et retirer
  son statut de derniere version. `releases/latest` exclut les preversions, donc l'endpoint
  retombe aussitot sur la derniere version COMPLETE (~1 min de propagation). **Premier geste,
  avant meme de diagnostiquer.**
- **`Resource not accessible by integration` a la creation de la release** : verifier d'abord les
  permissions des workflows du depot. **Une relance ne sert a rien** — elles sont fixees a la
  CREATION du run, il faut un tag NEUF. Deux relances perdues a croire a un « incident
  transitoire », qui existe aussi mais vient en SECOND.
- **Le job Linux peut se figer sur l'installation de paquets** : le service de mises a jour
  automatiques tient le verrou, et GitHub laisse courir six heures. D'ou les plafonds de duree,
  l'arret du service et les reessais — ils transforment un blocage en echec rapide.
- **Construire un bundle demande la cle de signature, meme quand on ne publie pas** : la CLI
  reclame la privee des que la publique est declaree, et echoue APRES avoir produit
  l'installeur. A ne pas confondre avec le build local, ou cet echec est voulu.
- **Verifier apres publication** que le manifeste repond 200 avec les trois plateformes ; un
  404 dans les deux premieres minutes est la propagation. **Un essai peut ne tomber que sur une
  autre machine** : le runner Linux a deux coeurs, lire le log de la plateforme qui coince.
- **L'updater Linux ne remplace qu'une AppImage** : pour essayer le flux reel, lancer l'AppImage.
  Le changelog est embarque au build. **Perdre la cle de signature = plus aucune mise a jour.**
- **L'AppImage embarque des bibliotheques de sa machine de construction**, et ca casse chez les
  autres : la libwayland du runner melangee au pilote graphique du systeme faisait abort WebKit,
  fenetre jamais ouverte. Contournement dans le code, bug amont sans correctif. Deux fausses
  pistes ecartees : les variables de rendu WebKit n'y changent rien, changer le runner non plus.
- **Le shell Claude tourne DANS un terminal Cockpit** : il herite des fuites d'environnement de
  l'AppImage. Prefixer les outils sensibles d'un `env -u` sur les variables concernees.
- **GitHub masque la valeur des secrets dans les logs, y compris au milieu d'un nombre** : un
  secret valant `1` rend tous les `1` illisibles. Faire IMPRIMER par l'essai la grandeur qui
  porte la conclusion, et se souvenir qu'un `***` au milieu d'un nombre est une redaction.

### Outillage local

- **Codes de sortie** : jamais derriere un pipe, c'est celui du dernier maillon — rediriger puis
  tester. **Commandes de fond : chemins ABSOLUS**, et relire le log reel : la fin ne prouve rien.
- **Le proxy `rtk` reformate `ls`, `ps` et les comptages de `grep`**, et rend parfois VIDE : on
  croit un dossier vide alors qu'il est plein. Passer par `rtk proxy`. Et **`npx tauri` peut
  resoudre un AUTRE paquet** ici : prendre le binaire local si un argument valide est refuse.
- **Registre npm** : la config globale pointe sur un registre prive, celle du projet la surcharge
  vers le public — **ne pas la retirer**, sinon la CI echoue et un nom d'hote interne fuite dans
  un repo public. Verrou npm a regenerer : supprimer les modules AVANT.
- **Un correctif ne se fait PAS essayer sur un binaire local** : il s'essaie depuis la version
  publiee. Une instrumentation de diagnostic doit donc etre PUBLIEE, puis retiree aussitot.

## Conventions

- Nouvelle table liee a un projet -> `PROJECT_SCOPED_TABLES`.
- Avant de coder un modal, un rename inline, un menu contextuel, un DnD de liste ou un toast ->
  `components/ui/`, `actions/reorderable.ts`, `stores/toast.ts`.
- Erreurs d'interface : jamais de `catch` muet. **Une confirmation passe par
  `demanderConfirmation()`** (`stores/confirm.ts`), jamais par le `confirm()` du systeme : meme
  forme d'appel au `await` pres, mais la fenetre suit le theme et la langue. Styles : tokens de
  `theme.css` uniquement, classes partagees dans `components.css`. Navigation inter-projet :
  forcer le remontage des composants.
- Nouvelle commande Tauri -> wrapper type dans `src/lib/api/`, types partages en snake_case.
- Nouvelle fonctionnalite visible -> l'illustrer dans la DOC INTEGREE (peu de texte, des
  maquettes ; « les gens ne lisent pas »). Une fonctionnalite absente de la doc n'existe pas.
