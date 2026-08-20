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
2. **35 Mo par terminal à historique plein, c'est beaucoup.** Dix terminaux ouverts = 350 Mo.
   À surveiller avant de livrer : soit on assume, soit on descend l'historique, soit on
   compacte. Ne pas découvrir ça chez un utilisateur.
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

**État : à faire.** Tenir les shells (`portable-pty`), le tuyau avec l'app
(`interprocess`), la poignée de main versionnée, la réconciliation au démarrage.

### C. Brancher, puis supprimer tmux

Basculer l'implémentation derrière le trait — c'est une ligne, `terminaux()` dans
`terminal/mod.rs` — vérifier que tout marche, puis **retirer vraiment** le code devenu
inutile :

- `src-tauri/src/terminal/tmux.rs` en entier (tout ce qui est propre à tmux y est enfermé
  depuis l'étape A)
- les commandes `detach_terminal` et `terminal_alt_screen`, leurs wrappers dans
  `src/lib/api/workspace.ts`, et les opérations correspondantes du trait : elles n'ont aucun
  appelant
- `TMUX_CONF` et sa génération
- `apply_server_options` et ses 41 commandes
- `absence_definitive` et l'analyse des messages de tmux
- `refresh_deployed_tmux`, `setup_bundled_tmux`, `copy_executable`
- `scripts/build-tmux-static.sh`, la ressource de l'AppImage, l'étape de cache en CI
- le repli `brew install tmux` du README
- toutes les mentions de tmux dans `CLAUDE.md` (section terminaux, pièges connus,
  dépendances système)

Le jour de la bascule : rien de spécial à prévoir. On ouvre un terminal, on lance
`claude`, on reprend une conversation — tout se comporte comme avant. Les conversations
Claude sont des fichiers dans `~/.claude/projects/`, elles n'ont jamais rien eu à voir
avec tmux. Ce qui tournait au moment de la mise à jour est perdu, une fois : une phrase
dans les notes de version suffit.

**État : à faire.**

## Ce qu'il faut reprendre, sans rien perdre

Liste de contrôle. Chaque ligne existe aujourd'hui grâce à tmux et doit continuer de
marcher — c'est ce qui décide si le chantier est fini.

| Fonctionnalité | Où c'est visible |
|---|---|
| Le shell survit à la fermeture de l'app | on rouvre Cockpit, le terminal est là |
| Historique de défilement (molette) | 10 000 lignes aujourd'hui |
| Sélection à la souris, qui reste affichée au relâchement | Ctrl+C copie ensuite |
| Copie vers le presse-papier système | clic droit → Copier |
| Recherche dans le terminal | la loupe de la barre d'onglets |
| Applications plein écran (`claude`, `vim`, `htop`, `k9s`) | elles doivent occuper tout le conteneur |
| Détection d'un agent IA dans la session | le logo Claude dans la barre latérale |
| Commande lancée à l'ouverture | bouton ▶ Cmd, shell de conteneur, palette |
| Redimensionnement quand la fenêtre change | pas de texte coupé |
| Zoom (Ctrl+molette) | la police change, le contenu se recadre |
| Accents et touches mortes | le correctif `GTK_IM_MODULE` reste, il est côté GTK |
| Plusieurs terminaux par projet, renommables | les onglets |
| Purge des sessions disparues au démarrage | pas de terminal fantôme |

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
