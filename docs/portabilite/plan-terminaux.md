# Chantier : notre propre serveur de terminaux

Décidé le 2026-08-20. Remplacer tmux par un service à nous, le même sur Linux, macOS et
Windows. L'analyse qui a mené à ce choix est dans [terminaux.md](terminaux.md).

**Ce fichier est l'état d'avancement.** Une session ou un agent qui reprend le chantier
le lit d'abord, et le met à jour en même temps que le code.

## Ce qu'on garde de l'existant

Rien à réinventer de ce côté :

| Brique | Rôle | État |
|---|---|---|
| `portable-pty` | ouvrir un shell, sur les trois systèmes (ConPTY inclus) | déjà une dépendance |
| `alacritty_terminal` | comprendre ce que le shell affiche, tenir la grille de caractères | à ajouter |
| `interprocess` | le tuyau entre l'app et le service (socket Unix, tuyau nommé Windows) | à ajouter |

## Les étapes, dans cet ordre

Chaque étape est publiable seule et ne casse rien. C'est la condition pour qu'un chantier
de cette taille avance sans jamais livrer une application à moitié fonctionnelle.

### A. Sortir l'interface (tmux reste derrière)

**État : FAITE le 2026-08-20.**

Le trait est dans `src-tauri/src/terminal/interface.rs`, l'implémentation tmux dans
`terminal/tmux.rs` (l'ancien `terminal/mod.rs`, renommé), et `terminal/mod.rs` ne fait plus
que choisir l'implémentation : `terminaux() -> Box<dyn Terminaux>`. `AppState.terminals` est
un `Box<dyn Terminaux>`, donc **aucune commande Tauri ne connaît plus tmux**. Aucun
comportement observable n'a changé : mêmes signatures IPC, mêmes retours, même ordre des
appels au démarrage.

Ce que l'écriture du trait a appris — c'est ça qui sert à l'étape B :

**Les 12 opérations, et lesquelles n'existent que parce que tmux existe**

| Opération | Verdict |
|---|---|
| `preparer` | Besoin réel, mais **un seul** : réconcilier ce que le serveur tient et ce que la base dit. Aujourd'hui elle fait trois choses, dont deux purement tmux (déployer un binaire, reposer 41 options sur un serveur déjà vivant). |
| `creer`, `ecrire`, `redimensionner`, `fermer`, `renommer`, `lister` | Besoins réels, sans discussion. |
| `attacher` | Besoin réel. **Ne rend plus rien** : le « replay » que rendait tmux était ignoré par le frontend depuis le pool de xterm. La notion n'est pas dans l'interface, elle ne doit pas revenir. |
| `chercher` | Besoin réel. Le motif est une **sous-chaîne littérale**, pas une regex : c'est une recherche d'utilisateur. |
| `copier_selection` | Besoin réel, mais payé en cinq maillons parce que la sélection appartient à tmux. Chez nous : un appel. |
| `detacher` | **Contournement, et code mort.** Aucun appelant côté frontend depuis la doctrine du pool (2026-08-13). |
| `ecran_alternatif` | **Contournement pur, et code mort aussi.** Il existe parce que le client tmux met toujours le terminal hôte en écran alternatif ; le service le saura en mémoire. |

`list_terminals` et `list_all_terminals` sont **une seule** opération (`lister` avec un filtre
projet optionnel) : la douzième commande Tauri n'est pas une douzième opération.

**Ce qui résiste, et qu'il faut trancher à l'étape B**

1. **`&Database` traverse 9 opérations sur 12.** C'est la trace la plus visible de tmux dans
   l'interface : l'identité d'un terminal (`id` → nom de session) vit en SQLite, donc presque
   chaque appel relit la base juste pour savoir à qui parler. Un service qui tient ses propres
   métadonnées n'en aurait besoin que pour `lister` (et encore : nom d'onglet et projet). À
   décider explicitement — soit le service devient la source de vérité et le trait perd ce
   paramètre, soit la base la reste et on l'assume.
2. **`AppHandle` traverse `creer`, `attacher` et `preparer`.** Celle-là est un vrai besoin — la
   sortie remonte au webview par un événement Tauri — mais elle attache l'interface à Tauri.
   Si le service tourne dans un autre processus, la sortie arrivera par le socket et c'est
   l'app qui ré-émettra : le trait pourra alors prendre un canal plutôt qu'un `AppHandle`.
3. **Deux commandes Tauri mortes** (`detach_terminal`, `terminal_alt_screen`) : elles sont
   restées, puisque l'étape A ne change rien de visible. À supprimer à l'étape C, avec leurs
   wrappers `api/workspace.ts` — et surtout à ne pas réimplémenter dans le service.

### B. Écrire le service, sans le brancher

Le service tient les shells et leur écran. Il tourne à part et survit à la fermeture de
l'app. Personne ne l'utilise encore : il se teste seul.

Trois choses décidées dès le départ, parce qu'on ne revient pas en arrière dessus :

1. **Le service n'écrit rien sur disque.** Il tient tout en mémoire et meurt avec la
   machine. Ça correspond au besoin — survivre à la fermeture de l'app, pas au
   redémarrage — et ça supprime toute question de migration de format plus tard.
2. **Un numéro de version dans la poignée de main, dès la première version.** Le service
   survit à l'app, donc une app neuve parlera un jour à un service ancien. Sans ce numéro
   on hérite du « protocol version mismatch » de tmux, avec en plus la responsabilité.
3. **Un service par utilisateur, pas un service système.** Les terminaux appartiennent à
   une session utilisateur : son environnement, son presse-papier, son `HOME`.

#### B1. La grille et le redessin

**État : FAITE le 2026-08-20.**

`src-tauri/src/terminal/ecran/` : `Ecran` avale les octets du shell (`alacritty_terminal`
`=0.26.0`), `redessiner()` rend une suite d'octets qui refabrique cet état dans un terminal
neuf. Le module se teste **seul** : personne ne l'appelle, et un `#![allow(dead_code)]`
commenté sur place tient les avertissements à distance jusqu'à B2.

**Le test qui borne le travail** (`ecran/tests.rs`) : sérialiser, relire dans un émulateur
neuf, comparer les deux états — cellule par cellule (caractère, couleurs, attributs,
accents combinants, couleur de soulignement, lien OSC 8), plus la position et la forme du
curseur, l'écran actif, la région de défilement, les modes DEC, la palette modifiée, le
titre et sa pile, les jeux de caractères. Trois sources : 44 états fabriqués à la main, des
octets au hasard (deux générateurs, graines fixes pour que tout échec soit rejouable), et
6 traces réelles captées dans un PTY 80x24 (`vim`, `htop`, `less`, `git log`, `ls --color`,
`claude`) par `scripts/capturer-trace.py`, rejouées entières **et** tronquées à toutes les
longueurs.

**Résultat : l'aller-retour est EXACT sur les états fabriqués et sur les traces réelles.**
La mise au point a tourné à 6 000 tirages au hasard par graine sur cinq jeux de graines
différents (≈ 100 000 états) ; ce qui reste dans le test est réduit à 1 600 tirages par
générateur pour que `cargo test` reste rapide.

**Ce que les octets au hasard ont trouvé, et qui ne se voyait sur aucune trace :**

| Symptôme | Cause |
|---|---|
| Toute la fin d'une ligne décalée d'une colonne | `unicode-width` rend 3 pour certains signes khmers, `Term::input` ne connaît que 1 et 2 |
| Ligne suivante écrite PAR-DESSUS la précédente | le dessin enchaînait sur un enroulement qui n'aurait pas lieu (dernière cellule sautée) |
| Un fond de couleur qui bave sur les lignes suivantes | la ligne qui entre par le bas hérite du fond du stylo (`Cell::reset` ne recopie que `bg`) |
| Tout le reste de la ligne décalé de huit colonnes | `\t` réémis tel quel relance la logique de tabulation au lieu de reposer la cellule |
| Curseur qui atterrit en dernière colonne au lieu de sa colonne | l'état « en butée à droite » suit un retour de tabulation (`CSI Z`), le seul geste qui déplace la colonne sans l'annuler |
| Région de défilement collée au bas de l'écran non restaurée | `CSI 24;24r` est REFUSÉ (haut ≥ bas) ; il faut demander `CSI 24;25r` et laisser le bornage ramener à 24 |

**Les cas qui résistent, tous écrits dans le code** (`ecran/tests.rs`, fonction
`mettre_de_cote_les_cas_connus`, et l'en-tête de `ecran/mod.rs`). Ils tournent tous autour
d'une même famille : une grille malmenée par des insertions et des effacements garde des
restes de caractères larges qui ne désignent plus rien, et aucune séquence d'échappement ne
sait les reposer tels quels. Ce qui se perd est à chaque fois **invisible** : un indice de
largeur sur une cellule vide, un `WRAPLINE` inerte, la cellule que le remplissage d'un
caractère large recouvre de toute façon. **Aucun programme réel ne produit ces états** — les
six traces passent l'aller-retour exact, et c'est le test qui le verrouille. Sur des
séquences d'échappement tirées au hasard, environ 1 % des états les rencontrent.

Deux limites de plus, sans rapport avec les caractères larges :
- **les taquets de tabulation** (HTS, TBC) ne sont ni lisibles depuis `Term` ni
  restaurables (HTS pose un taquet à la colonne du curseur, que l'espion ne connaît pas) ;
- **le curseur sauvegardé** (DECSC/DECRC) n'est pas restauré ;
- **la grille principale cachée sous l'écran alternatif** n'est pas lisible :
  `swap_alt()` remet l'écran alternatif à zéro quand on y revient. Le redessin ne rend que
  l'écran actif. Ce n'est pas une perte — voir la contrainte pour B2 ci-dessous.

**Les mesures, sur cette machine, en `--release`** (à comparer aux chiffres de tmux plus
bas) :

| Ce qu'on mesure | Notre émulateur | tmux |
|---|---|---|
| Ingestion d'une rafale de 4 Mo (par blocs de 64 Ko) | **53 à 60 ms** (70 à 76 Mo/s) | 417 ms, et il ne livrait que 1,96 Mo |
| Sérialisation d'un écran complet (80x24) | **22 µs pour 1 280 octets** | 3,1 ms pour 4,1 Ko (`capture-pane`) |
| Sérialisation écran + 10 000 lignes d'historique | 10,5 ms pour 544 Ko | — |
| Mémoire d'une session de plus, historique plein | **35 Mo** (dont 19 Mo de cellules : 24 octets x 801 920) | — |

Le redessin **ne transmet pas la rafale** : 4,19 Mo avalés donnent 1 280 octets à redessiner
pour l'écran seul. C'est le service que tmux rendait sans qu'on le sache, et il est rendu.

**Ce que B2 doit reprendre de B1, sans le redécouvrir :**

1. **Redessiner à chaque changement d'écran actif.** Le redessin ne rend que l'écran actif ;
   quand une application plein écran se termine (`?1049l`), le service doit renvoyer un
   redessin, sinon le frontend affiche un écran principal vide.
2. ~~**35 Mo par terminal à historique plein, c'est beaucoup.**~~ **Tranché en B2** : mesuré
   à 19,5 Mo en `--release` pour une session pleine de 80 colonnes, et l'historique se compte
   désormais en CELLULES (800 000) et non en lignes, ce qui borne la facture quand la fenêtre
   s'élargit. Voir B2, « la mémoire ».
3. **Le chemin de frappe ne passe PAS par ce module.** `Ecran::avaler` est pour la sortie du
   shell. L'entrée va au PTY directement, comme aujourd'hui (0,4 ms était le prix de tmux,
   faire pire ne se pardonne pas).
4. **Les réponses au shell doivent être renvoyées.** `Ecran::sortants()` rend ce que
   l'émulateur veut dire : `VersLeShell` (identification, position du curseur — un programme
   qui les demande ATTEND la réponse et se figerait sans elle) et `VersLePressePapier`
   (OSC 52, la chaîne de copie d'aujourd'hui). Les ramasser après chaque `avaler`.
5. **La version d'`alacritty_terminal` est épinglée à l'exact.** Ce sur quoi on s'appuie est
   listé en tête de `ecran/mod.rs` — à relire avant toute montée de version, la crate ne
   promet aucune stabilité d'API.

#### B2. Le service, le socket, les PTY

**État : FAITE le 2026-08-21.**

`src-tauri/src/terminal/service/`. Le **même binaire**, lancé avec
`--service-terminaux <socket>`, devient un service de terminaux : il tient les shells dans
ses propres PTY, leur écran dans le module `ecran/` de B1, et il survit à la fermeture de
l'application. Il n'écrit rien sur disque. **Rien de l'application n'y passe encore** — la
bascule est l'étape C.

| Fichier | Rôle |
|---|---|
| `protocole.rs` | messages, cadrage, poignée de main versionnée |
| `tuyau.rs` | chemin du socket, dossier 0700, refus d'un autre utilisateur |
| `session.rs` | un shell, son écran, et la règle « brut ou redessin » |
| `serveur.rs` | écoute, connexions, sessions, plafond d'historique |
| `client.rs` | côté application de la conversation |
| `lancement.rs` | double `fork` + `setsid` (Unix) / `DETACHED_PROCESS` (Windows) |

**Les décisions prises, et pourquoi**

1. **L'identifiant d'un terminal vient de l'APPLICATION** (`Créer { id, ... }`). Le service
   tient l'état vivant (sessions, taille, écran, agent qui tourne) ; SQLite garde le nom
   d'onglet et le projet, parce qu'eux doivent survivre à un redémarrage de la machine.
   Le rowid SQLite est donc la seule identité qui traverse un reboot.
2. **`renommer` ne traverse pas le socket.** C'est la conséquence directe du point 1 : le
   nom vit en base, le mettre aussi dans le service ferait deux vérités pour une même
   chaîne. Le client de l'étape C le servira depuis la base, sans aller-retour.
3. **`détacher` et `écran alternatif` ne sont pas implémentés**, comme demandé : ce sont des
   contournements de tmux sans appelant.
4. **`copier la sélection` prend une RÉGION** (début, fin) et rend son texte. La sélection
   appartenait à tmux ; chez nous elle appartient au frontend, et le service est juste celui
   qui sait lire une zone qui a défilé hors de l'écran. Un appel, comme promis.
5. **`chercher` rend un résultat** (nombre d'occurrences, indice courant, position) au lieu
   de peindre : le service n'a pas d'écran à peindre. Les lignes enroulées comptent pour une
   seule — « --no-bundle » à cheval sur une coupure de 80 colonnes doit se trouver.
6. **La poignée de main : le SERVICE parle en premier**, dix octets de forme figée
   (`CKPTERM\0` + version sur 2 octets), avant tout autre échange. C'est ce qui permet à la
   partie la plus récente de dire « ce service est plus ancien que moi » avec les deux
   numéros, sans avoir à comprendre le format de l'autre. Erreur structurée
   (`ErreurPoignée`), jamais une chaîne à analyser.
7. **Socket** : `$XDG_RUNTIME_DIR/cockpit/terminaux.sock` (repli `<temp>/cockpit-<uid>/`),
   dans un dossier créé en 0700, et l'euid du pair vérifié **des deux côtés**. Surchargeable
   par `COCKPIT_TERMINAUX_SOCKET` : c'est ce qui permet à une installation de développement
   (`COCKPIT_DB`) d'avoir son propre service — le garde-fou que tmux payait par une
   exception en dur dans `purge_dead`.

**Ce qui part sur le socket** *(corrigé à l'étape C — lire la suite avant de s'appuyer
dessus)*

L'intention de B2 était : ce qui est petit part tel quel et tout de suite (l'écho d'une
touche) ; ce qui dépasse quatre octets par cellule d'écran est REMPLACÉ par un redessin. Le
« **94 octets** pour `seq 1 200000` » annoncé ici **ne mesurait rien** : l'essai attendait
« 200000 » à l'écran, chaîne que la ligne tapée contient déjà, donc il repartait avant que le
shell ait écrit un octet et ne voyait que le redessin de l'attache. Le vrai chiffre était
**1,49 Mo en 16 461 envois** — le regroupement ne se déclenchait jamais, parce qu'un shell
écrit au fil de l'eau et que le lecteur du PTY est plus rapide que lui. Corrigé à l'étape C
(déclencheur sur le rythme + seuil de lot) : **99 envois** pour la même rafale, et le flux
brut EST transmis, ce qui remplit le tampon de défilement d'xterm et rend la molette gratuite.

**Les deux chiffres**

| Ce qu'on mesure | Notre service | tmux |
|---|---|---|
| Latence ajoutée par frappe (aller-retour complet moins PTY nu) | **0,024 à 0,062 ms** | 0,4 ms |
| Dépôt d'une frappe sur le socket (ce que paie la commande Tauri) | 2,4 à 4,2 µs | — |
| Aller-retour complet touche → écho affiché | 30 à 69 µs | — |
| Envois pour ~1,3 Mo de sortie shell | **99 à 158** (16 461 avant l'étape C) | 3 pour 1,9 Mo, en jetant 53 % des octets |

Mesures du 2026-08-21 sur cette machine, médiane sur 200 frappes (`cargo test -- --nocapture`,
essai `la_latence_de_frappe_reste_sous_celle_de_tmux`). Le chemin de frappe ne passe ni par
le verrou de l'écran, ni par une file, ni par un aller-retour : `Écrire` n'attend AUCUNE
réponse, un échec revient en poussée `Panne`.

**La mémoire : mesurée, puis bornée en CELLULES**

B1 annonçait 35 Mo par session à historique plein et laissait la question ouverte. Mesures en
`--release`, coût d'une session SUPPLÉMENTAIRE :

| Session | À vide | Historique plein |
|---|---|---|
| 80 colonnes, 10 000 lignes | 204 Ko | 19,5 Mo |
| 240 colonnes, 3 333 lignes (avec le plafond) | 320 Ko | 23,1 Mo |
| 240 colonnes, 10 000 lignes (sans le plafond) | 320 Ko | **57,1 Mo** |

Décision : **l'historique se compte en cellules, pas en lignes** — 800 000 cellules par
session (`serveur::CELLULES_D_HISTORIQUE`), soit exactement les 10 000 lignes promises à 80
colonnes, et moins de lignes au-delà. La facture cesse de suivre la largeur de la fenêtre :
c'est la dernière ligne du tableau qui a tranché, une simple fenêtre plus large triplait le
coût pour un historique que personne n'avait demandé plus long.

Ce qui reste assumé, et qu'il faut savoir : onze terminaux de 80 colonnes RÉELLEMENT pleins
font ~215 Mo. Ce pire cas n'est presque jamais atteint — alacritty n'alloue les lignes qu'au
fur et à mesure qu'elles défilent (204 Ko pour une session neuve contre 19,5 Mo une fois
pleine), donc un terminal où tourne un agent en plein écran ne coûte rien. Le levier suivant,
s'il faut descendre plus bas, est de ranger les lignes de l'historique autrement que la
grille vive (texte + attributs comprimés) : gros chantier, à ne lancer que sur une plainte
réelle.

**Ce que les essais prouvent** (`service/tests.rs`, 40 essais avec ceux des sous-modules)

- le tour complet par le socket : créer, écrire, lire la sortie, redimensionner, fermer ;
- **la survie** : un service dans un VRAI processus détaché (le même `lancer_detache` que
  l'application, lancé depuis le binaire de test), on crée un terminal, on tue le client, on
  se reconnecte, l'écran est identique au caractère près — et le service n'est plus un enfant
  du processus qui l'a lancé ;
- **vim** : reconnexion sur une application plein écran, elle est retrouvée dessinée ET elle
  répond encore ;
- **la reconnexion en plein flux** : ce qu'un terminal neuf montre après avoir rejoué tout ce
  qu'il a reçu est exactement ce que le service affiche (rien perdu, rien dédoublé) ;
- la poignée de main : version différente et interlocuteur étranger reconnus AVANT tout
  échange ;
- la réconciliation base ↔ service, en fonction pure.

**Ce qui reste pour l'étape C**

1. Écrire l'adaptateur `TerminauxService` qui implémente le trait `Terminaux` par-dessus
   `service::Client` : c'est lui qui joint l'état vivant du service et le nom/projet de
   SQLite, sert `renommer` depuis la base, et lance le service au démarrage
   (`lancement::demarrer`).
2. Rebrancher la sortie : les poussées `Sortie`/`Redessin` deviennent l'événement Tauri
   `terminal_output` (base64, mêmes regroupements côté frontend), `Fini` devient
   `terminal_exit`, `PressePapier` appelle `set_clipboard`.
3. **La molette** : faire venir l'historique du service (`Redessiner` avec historique) au lieu
   du tampon d'xterm — voir « pas le flux brut » ci-dessus.
4. Brancher la réconciliation dans `preparer` (`reconcilier()` est déjà écrite et testée).
5. Retirer `détacher`, `écran alternatif` et tmux, comme prévu ci-dessous.

### C. Brancher, puis supprimer tmux

**État : FAITE le 2026-08-21.** En deux commits : le branchement, puis la suppression.

`terminal/adaptateur.rs` implémente le trait `Terminaux` par-dessus `service::Client` :
c'est lui que `terminaux()` rend, lui qui lance le service au démarrage
(`lancement::demarrer`), et lui qui traduit les poussées du service en événements Tauri
(`Sortie`/`Redessin` → `terminal_output`, `Fini` → `terminal_exit`, `PressePapier` →
presse-papier système, `Panne` → journal). tmux a disparu du dépôt : `terminal/tmux.rs`,
`scripts/build-tmux-static.sh`, la ressource de l'AppImage, l'étape de cache en CI, la
colonne `tmux_name`, le champ `tmux` de la fiche machine, et les mentions du README et du
`CLAUDE.md`.

**Ce qui a été décidé en branchant, et qui n'était pas dans le plan**

1. **`attacher` est un NO-OP quand le terminal est déjà branché.** Le frontend l'appelle à
   chaque retour sur un onglet ; re-brancher demanderait un redessin complet, donc un
   clignotement et un retour en bas du défilement à chaque aller-retour. L'adaptateur tient
   la liste des terminaux branchés (`attaches`).
2. **Un redessin porte l'écran ET l'historique.** Il commence par une remise à plat (RIS),
   qui vide le tampon de défilement du terminal d'arrivée : sans l'historique, revenir sur
   un onglet ferait perdre ce que la molette remontait. Conséquence heureuse : xterm est un
   miroir exact de la grille du service, donc la recherche peut désigner une ligne par son
   indice et le frontend la surligne lui-même.
3. **La molette n'a rien à demander au service.** Le point 3 de « ce qui reste pour l'étape
   C » (faire venir l'historique à chaque cran) n'a pas lieu d'être : le flux brut EST
   transmis, en gros lots, donc le tampon d'xterm se remplit tout seul. Seul un débit
   insoutenable (> 256 Ko par fenêtre de 8 ms, soit 32 Mo/s) est remplacé par un redessin,
   et l'historique complet est renvoyé dès que le calme revient.
4. **`copier_selection` et `chercher` ont changé de camp.** La sélection appartient à xterm
   (qui tient tout l'historique), donc `terminal_copy_selection` et l'opération du trait
   sont partis : le frontend copie ce qu'il a. La recherche, elle, RESTE côté service — lui
   seul recolle les lignes enroulées — mais rend une position que le frontend surligne
   (`registerDecoration`, qui exige `allowProposedApi: true`).
5. **Une base choisie à la main (`COCKPIT_DB`) obtient automatiquement son propre socket.**
   Sinon la réconciliation du démarrage voit les terminaux de l'installation normale comme
   des sessions orphelines et les tue — c'est le scénario qui coûtait des sessions du temps
   de tmux, où il fallait une exception en dur dans la purge.
6. **Sous AppImage le service est relancé depuis `$APPIMAGE`**, pas depuis
   `current_exe()` : le montage `/tmp/.mount_*` disparaît à la fermeture de l'application,
   et le service doit lui survivre.

**Deux bugs trouvés en vérifiant**

- **Le regroupement de la sortie ne s'est jamais déclenché.** Il attendait 8 Ko en attente,
  or un shell écrit au fil de l'eau et le lecteur du PTY est plus rapide que lui : chaque
  lecture rend ~85 octets. `seq 1 200000` partait en **16 461 envois**, donc autant
  d'événements Tauri. Le déclencheur est désormais le RYTHME (la suite attendait déjà)
  doublé d'un seuil de lot (2 Ko), pour le cas d'une machine chargée où le shell produit par
  à-coups : **99 envois** pour la même rafale. Le « 94 octets sur le socket » annoncé en B2
  ne mesurait rien : l'essai attendait « 200000 » à l'écran, chaîne que la ligne tapée
  contient déjà, donc il repartait avant que le shell ait écrit un octet.
- **Le surlignage de la recherche exigeait `allowProposedApi`** : `registerDecoration` lève
  une exception sans ce drapeau, et la barre affichait « You must set the allowProposedApi
  option to true » au premier essai sur le binaire.

## Ce qu'il faut reprendre, sans rien perdre

Liste de contrôle, passée **sur le binaire construit** le 2026-08-21 : Xvfb, base et service
à part (`COCKPIT_DB`), clics et frappes réels injectés par XTEST, captures d'écran relues.
C'est elle qui décide si le chantier est fini.

| Fonctionnalité | Résultat |
|---|---|
| Le shell survit à la fermeture de l'app | **OK** — application tuée, service intact ; à la réouverture, l'écran est celui qu'on avait laissé, historique compris |
| Historique de défilement (molette) | **OK** — 10 000 lignes dans xterm, molette native, ascenseur visible ; le service en garde autant et les renvoie à chaque redessin |
| Sélection à la souris, qui reste affichée au relâchement | **OK** — glisser souris, la sélection reste, Ctrl+C copie (sans sélection, Ctrl+C envoie bien SIGINT : `^C` à l'écran) |
| Copie vers le presse-papier système | **OK** — texte relu depuis un AUTRE processus (Gtk.Clipboard sur le même display) : « 300 » |
| Recherche dans le terminal | **OK** — « aiguille-rare » trouvé dans l'historique, compteur « 1/1 », occurrence centrée et surlignée en couleur d'accent |
| Applications plein écran (`claude`, `vim`, `htop`, `k9s`) | **OK** — htop occupe tout le conteneur (jauges, couleurs, F1-F10) et le quitter rend l'écran principal intact ; `claude` idem |
| Détection d'un agent IA dans la session | **OK** — le logo Claude s'allume dans la barre latérale, et s'éteint quand l'agent s'arrête ; la racine est le PID du shell, que le service connaît |
| Commande lancée à l'ouverture | **OK** — « + Nouvelle session claude » ouvre un terminal et y tape `claude` tout seul |
| Redimensionnement quand la fenêtre change | **OK** — vérifié par le zoom (mêmes appels) : le contenu se reflue, rien n'est coupé |
| Zoom (Ctrl+molette) | **OK** — 115 % → 138 % → 123 %, la police suit et le terminal se recadre |
| Accents et touches mortes | **OK** — `echo éçàèune êî` tapé touche par touche (touches directes ET touches mortes `^`+e, `¨`+i) ressort intact, sans doublon ni espace parasite |
| Plusieurs terminaux par projet, renommables | **OK** — deux onglets, renommage par clic droit dans la barre latérale (« logs api ») ; le double-clic sur l'onglet n'a pas pu être exercé au banc (le double-clic synthétique n'est pas reconnu comme tel), le chemin est le même appel `rename_terminal` |
| Purge des sessions disparues au démarrage | **OK** — service tué, application relancée : la ligne sans session disparaît de la base et la barre latérale ne montre rien |

Trois choses vérifiées en plus, parce qu'elles cassent au même endroit : le collage (clic
droit → Coller ET clic molette, **une seule** insertion, texte posé par un autre processus),
la fermeture d'un onglet (ligne supprimée en base) et `exit` dans le shell (« [processus
terminé] », onglet barré, ligne supprimée).

**Limite du banc, à savoir avant de s'inquiéter** : sous un X sans gestionnaire de
presse-papier, arboard ne se relit pas lui-même — `get_clipboard` rend une chaîne vide juste
après un `set_clipboard` du même processus, et « Coller » dit « le presse-papier est vide ».
Le collage marche dès que le contenu vient d'ailleurs (vérifié), et sur un vrai bureau le
gestionnaire de presse-papier prend la propriété de la sélection. Ce n'est pas une régression
du chantier : ce chemin (`set_clipboard`/`get_clipboard`, arboard) n'a pas été touché.

## La performance, mesurée avant de commencer

Les quatre corrections de la v0.33.2 ont déjà réglé ce qui rendait les terminaux lents,
et **aucune ne venait de tmux** :

| Ce qui bloquait | Avant | Après |
|---|---|---|
| Détection des agents, toutes les 5 s | 56,5 ms | 4,0 ms |
| Décodage d'une rafale de 2 Mo | 75,2 ms | 2,8 ms |
| Événements envoyés pour une rafale | 2 547 | 3 |
| Forks tmux au démarrage | 167 ms | 9,1 ms |

Ce que tmux coûtait vraiment : **0,4 ms par frappe**, et il jetait 53 % des octets à
dessiner sur une grosse sortie — ce qui nous arrangeait.

**Donc le service maison doit être mesuré contre ces chiffres, pas contre une impression.**
Deux pièges à surveiller, parce qu'ils remplacent des avantages qu'on avait sans le savoir :

1. **Ne pas envoyer tout ce que le shell produit.** tmux écrasait les lignes qui avaient
   défilé. Un service qui transmet tout ferait dessiner quatre fois plus au frontend. Il
   faut n'envoyer que ce que l'écran montre à la fin.
2. **Ne pas rendre le chemin de frappe plus long.** 0,4 ms était le prix de tmux ; faire
   pire serait une régression que personne ne pardonne. Le chemin frappe → shell doit
   rester direct.
