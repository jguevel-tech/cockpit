# Changelog

Toutes les modifications notables de Cockpit sont consignées ici.

Format : [Keep a Changelog](https://keepachangelog.com/fr/1.1.0/) —
versionnage : [SemVer](https://semver.org/lang/fr/).

Ce fichier n'est pas décoratif : il est **affiché dans le logiciel** (Paramètres → Général) et
son contenu sert de notes de version à la Release GitHub, donc au message que voient les
utilisateurs quand la cloche de mise à jour s'allume. Une section `[Unreleased]` vide bloque
le script de release.

## [Unreleased]

## [0.9.0] — 2026-08-14

### Changed

- **La vue projet tient sur une seule barre** : le titre du projet à gauche, les onglets au
  centre, et les actions (liens rapides, ⏺ Enregistrer) tout à droite. L'ancien empilement
  de deux bandeaux créait des doubles courbures disgracieuses avec une image de fond. La
  description du projet reste visible au survol du titre.

## [0.8.0] — 2026-08-14

### Changed

- Le logo Claude s'affiche dans la sidebar et le tableau de bord quand un agent IA tourne dans
  un terminal, à la place de la pastille verte.

### Fixed

- **Détection des agents IA perdue** : un claude lancé depuis un shell où traînait
  l'environnement AppImage se présentait sous un faux nom de processus, invisible pour la
  détection. Elle vérifie désormais le binaire réellement exécuté, insensible au déguisement.

## [0.7.2] — 2026-08-14

### Fixed

- **Nom d'onglet incohérent à la création d'un terminal** : la sidebar affichait bien
  « COCKPIT - 1 » mais l'onglet restait sur « Terminal 1 ». Le nom, généré en base, n'était pas
  relu par l'onglet fraîchement créé.

## [0.7.1] — 2026-08-13

### Fixed

- **Saut de ligne au changement de terminal — cause racine éliminée.** tmux fabrique lui-même
  des événements « focus » vers l'application à chaque attache/détache de client, même avec
  `focus-events off` (prouvé en isolant : un cycle attache/détache sans aucune entrée fait
  redessiner Claude Code, et ce redraw laissait la ligne vide). Les correctifs précédents
  visaient d'autres maillons. Désormais Cockpit ne détache plus rien au switch : les terminaux
  et leurs clients tmux restent vivants en permanence, changer d'onglet est un simple
  masquer/montrer. Bénéfice annexe : le retour sur un terminal est instantané.

## [0.7.0] — 2026-08-13

### Changed

- Les nouveaux terminaux sont nommés d'après leur projet : « COCKPIT - 1 », « COCKPIT - 2 »…
  au lieu de « Terminal 1 ». La numérotation est propre à chaque projet et ne produit jamais
  de doublon, même après des fermetures ; les terminaux renommés à la main ne sont pas touchés.

## [0.6.7] — 2026-08-13

### Fixed

- **Saut de ligne en changeant de terminal** : au switch, les événements « focus perdu / focus
  repris » émis par l'interface traversaient jusqu'à l'application dans le terminal (Claude Code,
  vim…), qui redessinait son écran — en laissant parfois une ligne vide. Mesuré octet par octet
  sur une session réelle : ces événements étaient la seule chose reçue par l'application au moment
  du switch. Ils ne sont plus transmis — changer d'onglet dans Cockpit n'est pas une perte de
  focus.
- **`python3` cassé dans les terminaux Cockpit** (lancé depuis l'AppImage) : l'environnement du
  runtime AppImage (`PYTHONHOME`, `PYTHONPATH`, `LD_LIBRARY_PATH`…) fuyait dans tous les shells,
  et python plantait avec « ModuleNotFoundError: encodings ». Ces variables sont retirées à la
  création des terminaux, et purgées des serveurs tmux existants.

## [0.6.6] — 2026-08-13

### Fixed

- **Impossible de créer un terminal depuis la 0.6.5** : l'option tmux `window-size manual`,
  introduite pour corriger les sauts de ligne, fait planter le serveur tmux 3.4 à son démarrage.
  Plus aucun terminal ne pouvait être créé, et les terminaux existants étaient perdus. L'option
  est retirée ; la taille est désormais fixée par `resize-window` juste avant chaque attache —
  même effet contre les sauts de ligne, sans le plantage.
- **Onglets de terminaux morts impossibles à fermer** quand le serveur tmux ne tourne plus :
  « pas de serveur » était traité comme une erreur au lieu d'une réponse (zéro session). Ces
  onglets sont maintenant nettoyés au démarrage et fermables à la main.
- **Menu contextuel Copier/Coller décalé** : il s'affichait loin du clic. Le flou d'arrière-plan
  posé sur les panneaux en 0.6.3 changeait le repère de positionnement des éléments flottants
  (menus, modales) ; le flou est déplacé sur une couche dédiée qui ne peut plus interférer.
- **Sessions fantômes qui continuaient de tourner.** Quand la ligne d'un terminal disparaissait
  de la base sans que sa session tmux soit arrêtée, celle-ci restait vivante et injoignable :
  aucun onglet ne pouvait plus l'afficher, mais son shell consommait toujours de la mémoire.
  Trois terminaux affichés pouvaient ainsi masquer quatorze sessions actives. Cockpit les
  détecte et les arrête au démarrage.
- **Fermeture d'un terminal plus sûre** : la session est arrêtée avant que la ligne ne soit
  supprimée, et si l'arrêt échoue le terminal est conservé pour permettre un nouvel essai —
  au lieu de disparaître de l'interface en laissant son shell derrière lui.

## [0.6.5] — 2026-08-13

### Fixed

- **Saut de ligne à chaque changement de terminal.** Chaque session tmux conservait la taille du
  dernier client qui s'y était attaché : quinze terminaux portaient cinq tailles différentes.
  Revenir sur un terminal déclenchait donc un redimensionnement, et les applications en plein
  écran (Claude Code, vim) se redessinaient en laissant une ligne vide. Cockpit fixe désormais la
  taille avant de s'attacher, et la maîtrise de bout en bout.

## [0.6.4] — 2026-08-13

### Fixed

- **Caractères parasites dans les terminaux** (`^[[?1;2c^[[>0;276;0c`, puis `1;2c0;276;0c` tapé
  dans l'invite) et sauts de ligne intempestifs en revenant sur l'onglet Terminal. Les réponses du
  terminal aux questions de tmux repartaient dans le shell au lieu d'être consommées : en
  rattachant, l'ancien client tmux est remplacé, et ses réponses arrivaient au nouveau, qui n'avait
  rien demandé.
- **Terminaux qui disparaissaient tout seuls.** Un échec passager de `tmux list-sessions` était
  interprété comme « la session n'existe plus », et le terminal était supprimé de la base — alors
  qu'il tournait toujours. Cockpit ne supprime désormais un terminal que si tmux a explicitement
  répondu que sa session avait disparu ; en cas de doute, rien n'est détruit.

## [0.6.3] — 2026-08-13

### Fixed

- **Contenu posé à même l'image de fond** dans plusieurs écrans. Sur onze vues, sept n'avaient
  aucun panneau : les paramètres d'un projet, Workspace, Docker, Plugins, Git, Fichiers, le
  monitoring et la bibliothèque d'agents affichaient leurs libellés et leurs champs directement
  sur la photo, illisibles. Tous les onglets de projet reposent désormais sur un panneau continu.
- **Nom du projet illisible** sur une image de fond : l'en-tête n'avait pas de fond non plus. En-tête,
  barre d'onglets et contenu forment maintenant un seul panneau, du haut vers le bas.

## [0.6.2] — 2026-08-13

### Fixed

- **Fonds gris imposés à tous les boutons** avec une image de fond. Le correctif de la 0.5.1
  donnait un fond à chaque bouton, y compris à ceux déjà posés sur une surface blanche — d'où des
  pastilles grises un peu partout. La lisibilité est désormais assurée par les conteneurs : les
  menus latéraux reçoivent le fond, plus chacune de leurs entrées.

## [0.6.1] — 2026-08-13

### Fixed

- **L'image de fond était devenue invisible** en 0.6.0 : le fond uni posé sur chaque vue la
  masquait entièrement. Le panneau continu suffit à supprimer les trous entre sections ; la vue
  reste transparente et l'image réapparaît.
- **Image de fond stockée sans compression** : Cockpit demandait un encodage WebP, que le moteur
  de rendu ne sait pas produire — il retombait silencieusement sur du PNG. Une photo de 4 Mo
  restait donc à 4 Mo sur le disque et était rechargée telle quelle à chaque démarrage. Le format
  obtenu est désormais vérifié, avec bascule sur JPEG. Réimportez votre image pour en profiter.

## [0.6.0] — 2026-08-13

### Removed

- **Onglet Sitemap** et toute la comparaison de sitemaps. ⚠️ Les paires enregistrées sont
  supprimées de la base au premier démarrage de cette version.

### Changed

- **Les écrans de paramètres forment un panneau continu** au lieu d'une pile de cartes séparées.
  Les sections sont délimitées par des filets, plus par des interstices où le fond apparaissait —
  ce qui donnait des trous au milieu de l'interface, particulièrement visibles avec une image de
  fond. Chaque vue reçoit également un fond uni.

### Fixed

- Le thème choisi n'était pas conservé : au redémarrage, l'application revenait au thème sombre.
  Il en allait de même pour la couleur d'accent et les réglages de l'image de fond.

## [0.5.1] — 2026-08-13

### Fixed

- Boutons invisibles avec une image de fond : les contrôles sans fond propre — barres d'onglets,
  menu du tableau de bord, sidebar — se retrouvaient posés à même l'image. Tous les boutons ont
  désormais un fond, et les conteneurs structurels (en-tête, sidebar, barres d'onglets) reçoivent
  le même verre dépoli que les cartes, qui n'était appliqué qu'à ces dernières.
- Paramètres à l'étroit : espacement des cartes, des pastilles de thème et des curseurs revu.
- Opacité des surfaces à 92 % par défaut au lieu de 82 % — le texte restait pénible à lire
  sur une image chargée.

## [0.5.0] — 2026-08-13

### Changed

- En-tête allégé : le marketplace d'**Agents** a rejoint Paramètres → Agents, et le bouton de
  redémarrage a été retiré — les mises à jour relancent l'application toutes seules.

### Added

- **Thèmes** : sept palettes au choix dans Paramètres → Apparence — Sombre, Bleu nuit, Prune,
  Forêt, Braise, Clair et Papier.
- **Couleur d'accent personnalisable**, indépendamment de la palette.
- **Image de fond** : importez une image et elle habille toute l'application. Les surfaces
  deviennent translucides et floutées (verre dépoli) pour que le texte reste lisible, avec trois
  réglages au curseur — voile, flou de l'image, opacité des surfaces. Cockpit peut aussi reprendre
  la couleur dominante de l'image comme accent de l'interface.
  L'image est redimensionnée et recompressée à l'import ; le terminal reste opaque, pour ne jamais
  sacrifier sa lisibilité.

## [0.4.0] — 2026-08-13

### Added

- Centre de notifications : la cloche est désormais **toujours présente** dans l'en-tête, avec un
  badge du nombre de messages non lus. Un clic ouvre un panneau listant les notifications, la plus
  récente en premier, chacune avec son action. Les mises à jour y apparaissent comme une
  notification parmi d'autres — plus besoin de passer par les paramètres pour savoir s'il y a du
  neuf.

### Changed

- Vérification des mises à jour plus réactive : au démarrage, puis toutes les heures, et à chaque
  retour sur la fenêtre si la dernière vérification date de plus de 15 minutes (auparavant toutes
  les 6 heures).

## [0.3.0] — 2026-08-13

### Added

- Installation en une commande : `curl -fsSL .../install.sh | sh` installe la dernière version
  dans `~/.local/bin` avec une entrée au menu des applications, sans droits root.
- README complet : installation, prérequis, fonctionnalités, utilisation, raccourcis clavier.
- Licence MIT.

## [0.2.0] — 2026-08-13

### Added

- Mise à jour automatique : une cloche apparaît dans l'en-tête quand une nouvelle version est
  disponible. Elle affiche la version installée, la version proposée et ses notes ; un clic
  télécharge, installe et relance l'application.
- Journal des modifications consultable dans Paramètres → Général, avec la version installée
  et un bouton de vérification manuelle.
- Zoom global de l'interface : contrôle `− 100 % +` dans l'en-tête et Ctrl+molette partout,
  terminaux compris. Le niveau est conservé entre les lancements.

### Fixed

- Texte flou aux paliers de zoom intermédiaires : les paliers sont désormais dérivés de la
  taille de police des terminaux pour tomber sur des pixels entiers (108 % et 115 % au lieu
  d'un 110 % qui produisait une police de 14,3 px, lissée par le rasteriseur).

## [0.1.0] — 2026-08-13

Première version suivie en gestion de version. État existant à cette date :

### Added

- Orchestration Docker Compose : démarrage ordonné par tri topologique, détection de cycles,
  arrêt récursif des dépendances orphelines.
- Terminaux intégrés persistants adossés à des sessions tmux, avec détection des agents IA.
- Espace de travail par projet : notes Markdown arborescentes, todos, liens rapides.
- Explorateur de fichiers respectant `.gitignore`, coloration Shiki, édition, et
  « aller à la définition » via LSP.
- Onglet Git : status, diff coloré, stage/unstage, commit, push, gestion des branches.
- Enregistrement de réunions : capture double piste PipeWire, transcription Whisper,
  résumé automatique déposé en note.
- Comparaison de sitemaps avec diff HTML.
- Monitoring système : CPU, mémoire détaillée (dont ARC ZFS), disques, top processus.
- Marketplace d'agents Claude Code par projet.
- Thèmes sombre et clair.
