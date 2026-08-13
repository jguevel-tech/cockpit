# Changelog

Toutes les modifications notables de Cockpit sont consignées ici.

Format : [Keep a Changelog](https://keepachangelog.com/fr/1.1.0/) —
versionnage : [SemVer](https://semver.org/lang/fr/).

Ce fichier n'est pas décoratif : il est **affiché dans le logiciel** (Paramètres → Général) et
son contenu sert de notes de version à la Release GitHub, donc au message que voient les
utilisateurs quand la cloche de mise à jour s'allume. Une section `[Unreleased]` vide bloque
le script de release.

## [Unreleased]

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
