# Changelog

Toutes les modifications notables de Cockpit sont consignées ici.

Format : [Keep a Changelog](https://keepachangelog.com/fr/1.1.0/) —
versionnage : [SemVer](https://semver.org/lang/fr/).

Ce fichier n'est pas décoratif : il est **affiché dans le logiciel** (Paramètres → Général) et
son contenu sert de notes de version à la Release GitHub, donc au message que voient les
utilisateurs quand la cloche de mise à jour s'allume. Une section `[Unreleased]` vide bloque
le script de release.

## [Unreleased]

## [0.29.1] — 2026-08-19

### Fixed

- **La remontée des erreurs fonctionne maintenant vers un serveur sans certificat.** Elle
  exigeait une adresse sécurisée et restait donc inactive sur un serveur d'équipe en HTTP.
- **Les erreurs remontées disent d'où elles viennent.** Certaines portaient une origine
  dérivée d'un nom de variable (« global.dest »), inexploitable ; elles nomment désormais
  l'action concernée (« agents.doDeletePlugin », « appearance.loadWallpaper »). Une erreur
  qui partait en double n'est plus signalée qu'une fois.

## [0.29.0] — 2026-08-19

### Added

- **Les erreurs sont maintenant consignées et peuvent être transmises pour être corrigées.**
  Chaque erreur est écrite dans un journal sur ton poste — toujours, même hors ligne et sans
  rien accepter. Si tu l'acceptes (une question posée une seule fois, refusable sans rien
  perdre), elle est aussi transmise à l'équipe de développement avec les caractéristiques
  techniques de la machine : système, serveur audio réellement actif, versions des outils,
  type d'installation. Ce sont précisément les informations qui manquaient lors des derniers
  correctifs. Réglable à tout moment dans Paramètres → Général, avec le nom affiché.
  Ne sont jamais transmis : le contenu des projets, les notes, les tâches, ni ce qui se passe
  dans les terminaux.

### Fixed

- **Des erreurs qui restaient invisibles sont désormais signalées.** Une soixantaine
  d'endroits se contentaient d'écrire dans la console du navigateur — c'est-à-dire nulle
  part : copie du terminal, rattachement d'une session, sessions Claude, renommages,
  chargements de la marketplace. Les silences qui restent sont volontaires et expliqués sur
  place.

## [0.28.1] — 2026-08-19

### Fixed

- **L'enregistrement de réunion fonctionne maintenant sur les machines où le son est géré
  par PulseAudio.** C'est le cas d'Ubuntu 22.04 : PipeWire y est installé, mais ce n'est pas
  lui qui gère l'audio, si bien que Cockpit ne voyait aucun micro (« no node available »)
  alors qu'il y en avait un. Cockpit essaie désormais PipeWire puis, pour chaque piste qui
  ne démarre pas, bascule sur l'outil PulseAudio. Rien à installer ni à mettre à jour.
- **Un enregistrement démarre même si une seule des deux pistes répond**, avec un
  avertissement indiquant laquelle manque, au lieu de tout refuser. Il n'échoue que si
  aucune des deux ne fonctionne — et le message dit alors ce que chaque outil a répondu.

### Fixed

- **Un enregistrement de réunion démarre désormais même si une seule des deux pistes est
  disponible.** Sur une machine où le micro n'est pas exposé par PipeWire, l'enregistrement
  entier échouait alors que le son système, lui, se captait très bien : on perdait la seule
  piste qui fonctionnait. Cockpit enregistre maintenant ce qui est disponible et prévient
  clairement de la piste manquante. Il ne refuse que si aucune des deux ne répond.

## [0.28.0] — 2026-08-19

### Added

- **Cockpit parle français et anglais.** La langue se choisit dans Paramètres → Général :
  l'interface bascule aussitôt, sans redémarrage, et le choix est retenu au prochain
  lancement. Tout est traduit — les huit onglets de projet, le tableau de bord, les
  paramètres, les messages d'erreur affichés, les menus du clic droit, jusqu'à la
  documentation intégrée et ses illustrations, qui suivent la langue choisie.

### Fixed

- **L'enregistrement de réunion ne casse plus sur les systèmes livrés avec une version
  ancienne de PipeWire.** La capture du son système utilisait une option de `pw-record`
  absente avant PipeWire 0.3.5x (Ubuntu 22.04 par exemple) : la commande refusait de
  démarrer et l'enregistrement échouait aussitôt. Cockpit détecte désormais ce que la
  commande installée accepte et emploie la forme qu'elle comprend — rien à mettre à jour
  sur la machine.
- **Quand aucune entrée audio n'est disponible, le message le dit** et indique quoi
  vérifier, au lieu de laisser l'erreur brute de PipeWire (« no node available »).

## [0.27.4] — 2026-08-19

### Fixed

- **L'échec d'un enregistrement de réunion dit maintenant ce qui s'est réellement passé.**
  Le message annonçait « PipeWire indisponible ? » alors que la sortie d'erreur de
  `pw-record` était jetée : sur une machine où PipeWire et pw-record sont bien installés,
  il n'y avait aucun moyen de savoir ce qui bloquait. Cockpit remonte désormais ce que
  `pw-record` a répondu, piste par piste (micro, son système), et indique quoi vérifier
  quand il s'arrête sans rien dire.

## [0.27.3] — 2026-08-18

### Fixed

- **Le clic molette dans le terminal colle maintenant le presse-papier, comme « Coller » du
  clic droit.** Il collait deux textes différents à la fois, parce que trois mécanismes se
  disputaient le clic. Un seul reste, et c'est exactement celui du menu : même source, même
  résultat. La correction de la version précédente était incomplète — elle ne collait que la
  sélection faite dans le terminal, et rien quand le texte venait d'une autre application.

## [0.27.2] — 2026-08-18

### Fixed

- **Le clic molette dans le terminal ne colle plus deux fois.** Un seul clic produisait
  deux collages, car deux mécanismes se déclenchaient en même temps : tmux colle de
  lui-même au clic molette, et le moteur d'affichage y ajoutait le collage de la
  sélection en cours. Le doublon est supprimé ; le clic molette colle la sélection faite
  dans le terminal, une seule fois. Le collage par le menu contextuel et au clavier n'est
  pas affecté.

## [0.27.1] — 2026-08-18

### Fixed

- **Cockpit ne plante plus au démarrage sur les distributions récentes (Ubuntu 26.04).** Le
  système affichait « Plantage de l'application » en désignant le moteur d'affichage, et la
  fenêtre ne s'ouvrait jamais. L'AppImage embarquait une bibliothèque graphique bas niveau
  (`libwayland-client`) prise sur sa machine de construction : mélangée au pilote graphique
  plus récent de la machine de l'utilisateur, l'initialisation du rendu de WebKit échouait et
  le processus d'affichage s'arrêtait net. Cockpit utilise désormais la version installée sur
  la machine. Les distributions plus anciennes ne changent pas de comportement (vérifié sur
  Ubuntu 22.04, 24.04 et 26.04).

## [0.27.0] — 2026-08-17

### Added

- **Glisser-déposer un fichier dans le terminal** : lâche une image, une capture ou n'importe
  quel fichier sur le terminal, son chemin s'écrit directement à l'invite (plusieurs fichiers
  d'un coup fonctionnent). Utile pour donner une capture d'écran à Claude Code sans taper le
  chemin à la main. Le cadre du terminal s'allume pendant le survol ; déposer à côté explique
  où viser.

## [0.26.1] — 2026-08-17

### Fixed

- **Le bouton Start de l'onglet Docker ne renvoie plus une erreur incompréhensible quand le
  projet n'a pas de fichier compose.** Il affichait le message brut de Docker (« no
  configuration file provided: not found ») sans dire où le fichier avait été cherché ni quoi
  faire. Le fichier compose étant optionnel dans Cockpit, l'onglet explique maintenant
  l'absence, indique le dossier concerné, propose d'ouvrir les paramètres du projet pour
  nommer le fichier à utiliser, et grise Start / Stop / Restart tant qu'aucun n'est trouvé.

## [0.26.0] — 2026-08-15

### Fixed

- **La vraie cause de la « sur-brillance » est éliminée.** Le halo lumineux autour des
  textes et boutons sur image de fond venait du moteur d'affichage : il incluait à tort le
  contenu des panneaux dans le flou d'arrière-plan, affichant une copie floutée de chaque
  lettre (prouvé en isolation sur quatre variantes). Le flou des panneaux est retiré —
  l'interface est nette partout ; la lisibilité passe par l'opacité des surfaces, le voile
  et le curseur « Flou de l'image », déjà présents dans Apparence.

### Removed

- L'option « Éclat du verre dépoli » ajoutée en 0.24.0, devenue sans objet avec le retrait
  du flou des panneaux (c'était un correctif à côté de la vraie cause).

## [0.25.1] — 2026-08-15

### Fixed

- **Mises à jour automatiques sur macOS** : la 0.25.0 publiait bien l'application mais pas
  son artefact de mise à jour — la cloche restait muette sur Mac. C'est corrigé à partir de
  cette version.

## [0.25.0] — 2026-08-14

### Added

- **Cockpit existe désormais pour macOS** (Apple Silicon, bêta) : un `.dmg` est publié avec
  chaque version et les mises à jour intégrées fonctionnent. L'application n'étant pas
  encore notarisée par Apple, le premier lancement se fait par clic droit → Ouvrir ; les
  terminaux persistants demandent `brew install tmux` (l'application l'indique). Les
  fonctionnalités liées à Linux (enregistrement de réunions, détail mémoire) s'y désactivent
  proprement.

## [0.24.0] — 2026-08-14

### Changed

- **Fini la « sur-brillance » du verre dépoli** : la saturation des couleurs derrière les
  panneaux est désormais neutre par défaut, et devient une option — « Éclat du verre
  dépoli » dans Paramètres → Apparence — pour qui aime l'effet.

### Fixed

- **Documentation illisible sur une image de fond** : le contenu flottait directement sur
  la photo. Il repose maintenant sur le même panneau lisible que le reste de l'interface.

## [0.23.0] — 2026-08-14

### Added

- **Sauvegarde de la base en un clic** : Paramètres → Général → « Exporter la base… » écrit
  une copie de toutes tes données (projets, notes, tâches, URLs, commandes…) dans le fichier
  de ton choix — cohérente même si l'application écrit au même moment.
- **Aperçu des images dans l'onglet Fichiers** : png, jpg, webp, gif… s'affichent
  directement (sur un damier qui révèle la transparence) au lieu de « fichier binaire ».

## [0.22.0] — 2026-08-14

### Added

- **Statut des liens rapides** : chaque URL du projet porte une pastille — verte en ligne,
  rouge injoignable (survoler donne le code HTTP ou l'erreur) — dans la barre du projet et
  dans Paramètres → URLs, re-vérifiée chaque minute. D'un coup d'œil, tu sais que la
  préprod est tombée.

## [0.21.0] — 2026-08-14

### Added

- **Alertes système dans la cloche** : disque presque plein (≥ 90 %), mémoire ou CPU
  saturés pendant plusieurs minutes — une notification avec le bouton « Voir le
  monitoring », qui disparaît d'elle-même quand la situation redevient normale. Un pic
  passager (compilation…) n'alerte jamais.

## [0.20.0] — 2026-08-14

### Added

- **Documentation intégrée** : le bouton « i » en haut à droite ouvre un guide illustré de
  toutes les fonctionnalités — menu par thème à gauche, et surtout des exemples visuels
  (maquettes, raccourcis clavier) plutôt que du texte.
- **Historique Git** : un onglet « Historique » dans la vue Git liste les 100 derniers
  commits (sujet, auteur, date relative, branches/tags) ; un clic montre le diff complet
  du commit, fichier par fichier, avec les mêmes couleurs que les modifications en cours.
- **Bouton Pull** à côté de Push, avec le nombre de commits en retard. Toujours en
  avance rapide seule (`--ff-only`) : jamais de merge surprise depuis un bouton — en cas
  de divergence, un message l'explique.

## [0.19.0] — 2026-08-14

### Added

- **Palette de commandes (Ctrl+K)** : tape quelques lettres et saute n'importe où —
  projets, terminaux ouverts, onglets du projet courant, vues du tableau de bord,
  commandes rapides (lancées directement), et fichiers du projet par leur nom.
  ↑↓ pour naviguer, Entrée pour ouvrir, Échap pour fermer. Quand le focus est dans un
  terminal, Ctrl+K reste au shell — clique ailleurs pour ouvrir la palette.

## [0.18.0] — 2026-08-14

### Added

- **Recherche dans l'historique du terminal** (bouton 🔍 ou Ctrl+Maj+F) : tape ton texte,
  Entrée cherche vers le haut, ↑/↓ naviguent entre les occurrences — surlignage et compteur
  affichés directement dans le terminal, Échap referme et rend la main au shell. La frappe
  normale n'est jamais interceptée (Ctrl+F reste au shell).

## [0.17.0] — 2026-08-14

### Added

- **Créer, renommer et supprimer des fichiers et dossiers** dans l'onglet Fichiers : clic
  droit sur l'arbre (ou boutons + de l'en-tête pour la racine). La suppression envoie à la
  **corbeille du système**, jamais de suppression définitive — une erreur de clic se
  rattrape. Un nouveau fichier s'ouvre directement dans l'éditeur ; renommer un dossier
  suit le fichier ouvert à l'intérieur.

## [0.16.0] — 2026-08-14

### Added

- **Commandes rapides par projet** : déclare tes commandes habituelles (`make up`,
  `npm run dev`…) dans Paramètres du projet → Commandes rapides, et lance-les depuis le
  bouton « ▶ Cmd » de la barre du projet — chaque commande s'exécute dans un nouveau
  terminal Cockpit du projet.

## [0.15.0] — 2026-08-14

### Added

- **Échéances sur les tâches** : un 📅 apparaît au survol d'une tâche pour lui donner une
  date (vider le champ la retire). Badge coloré sur la tâche — gris à venir, orange
  aujourd'hui, rouge en retard — dans le projet comme dans le tableau de bord. Et la
  **cloche prévient** : une tâche pour aujourd'hui ou en retard pose une notification avec
  un bouton « Voir le projet » ; elle disparaît d'elle-même quand la tâche est terminée.

## [0.14.0] — 2026-08-14

### Added

- **Logs des conteneurs** : bouton « Logs » sur chaque conteneur (onglet Docker du projet et
  vue Conteneurs du tableau de bord) — les 500 dernières lignes, rafraîchies toutes les 2 s
  tant que le suivi est actif, stdout et stderr fusionnés dans l'ordre chronologique.
- **Shell dans un conteneur** : bouton « Shell » sur un conteneur en marche — ouvre un vrai
  terminal Cockpit du projet avec `docker exec` (bash si l'image en a un, sinon sh).

## [0.13.0] — 2026-08-14

### Added

- **Recherche dans le fichier ouvert (Ctrl+F)** : barre de recherche avec compteur
  d'occurrences (3/17), navigation Entrée / Maj+Entrée ou flèches, respect de la casse en
  option, toutes les occurrences surlignées et l'occurrence courante mise en évidence —
  comme dans un IDE. Bouton 🔍 dans l'en-tête du fichier, Échap pour fermer.
- **Recherche globale dans le projet (Ctrl+Maj+F)** : un champ au-dessus de l'arborescence
  cherche partout — noms de dossiers, noms de fichiers et contenu des fichiers (en
  respectant le .gitignore). Résultats groupés par fichier avec extrait de la ligne ;
  un clic ouvre le fichier directement sur la bonne ligne.
- **Numéros de ligne** dans le visualiseur de fichiers, et total de lignes + taille du
  fichier dans le coin de l'en-tête.
- **Copier le chemin du fichier** (bouton ⧉) et **retour à la ligne automatique**
  (bouton ⏎, pratique pour le Markdown et les logs).

## [0.12.0] — 2026-08-14

### Added

- **Suppression des dossiers de projets** : une corbeille apparaît au survol d'un dossier
  dans la sidebar. Un dossier qui contient encore des projets n'est jamais supprimé : un
  message explique qu'il faut d'abord les déplacer (le clic droit sur le dossier suit la
  même règle).

## [0.11.2] — 2026-08-14

### Fixed

- **Ouvrir un modal rendait le reste de la page transparent** (image de fond active) : le
  moteur d'affichage désactive le verre dépoli de toute la page sous un voile de modal. Le
  voile porte désormais son propre flou : tout ce qui est derrière le dialogue apparaît
  élégamment flouté, et plus jamais transparent.

## [0.11.1] — 2026-08-14

### Fixed

- **Modals et menus transparents avec une image de fond** : le contenu de la page
  transparaissait au travers du dialogue « Nouveau projet », des menus contextuels, du panneau
  de notifications et des toasts. Toutes les surfaces flottantes sont désormais opaques, dans
  toutes les palettes.

## [0.11.0] — 2026-08-14

### Added

- **tmux est maintenant embarqué dans Cockpit** : ouvrir un terminal ne demande plus
  d'installer quoi que ce soit. Si tmux est déjà présent sur la machine, il reste utilisé
  (vos sessions existantes ne bougent pas) ; sinon Cockpit déploie le sien, autonome, qui
  survit à la fermeture de l'application comme avant.

## [0.10.1] — 2026-08-14

### Fixed

- **Le modal « Nouveau projet » était invisible quand une image de fond était active** : la
  zone se grisait sans qu'aucune fenêtre n'apparaisse. Tous les modals, menus contextuels et
  panneaux sont maintenant rendus au-dessus de l'application quelle que soit l'apparence.
- **Un projet dont les conteneurs tournaient déjà restait affiché « stopped »** : les
  conteneurs en cours ne sont plus adoptés uniquement au démarrage de l'application, mais à
  chaque rafraîchissement — un projet créé en cours de session est reconnu immédiatement.
- **Une panne d'accès à Docker était silencieuse** (permissions sur docker.sock, docker
  absent…) : le projet paraissait simplement arrêté. L'onglet Docker affiche désormais la
  cause exacte.
- **Les conteneurs sont retrouvés même sans fichier compose au nom standard** : détection de
  repli par les labels que Docker Compose pose sur chaque conteneur (dossier de lancement).
- Un projet recréé après une suppression incomplète pouvait rester figé : la création répare
  maintenant l'enregistrement au lieu d'échouer en silence.

## [0.10.0] — 2026-08-14

### Changed

- Les boutons de la sidebar affichent désormais « + Projet » et « + Dossier » au lieu d'icônes
  seules — le dossier 📁 n'était pas compris comme un bouton de création.

### Fixed

- **Projet fraîchement créé inutilisable** (onglet Docker vide, bouton + du terminal sans
  réaction) : quand l'enregistrement auprès de l'orchestrateur échouait, le projet restait en
  base mais disparaissait de la liste de l'application. Un projet en base apparaît maintenant
  toujours, et ses terminaux, fichiers et Git fonctionnent même si Docker est indisponible.
- **Le bouton + du terminal explique désormais pourquoi il ne peut pas créer** (projet sans
  chemin, tmux manquant…) au lieu de ne rien faire silencieusement.

## [0.9.1] — 2026-08-14

### Fixed

- **Titres de page illisibles sur une image de fond** : « Tableau de bord » et « Paramètres »
  flottaient directement sur la photo. Ils sont désormais intégrés en tête du menu latéral de
  leur vue, qui porte déjà un panneau lisible.

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
