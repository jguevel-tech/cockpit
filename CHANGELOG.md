# Changelog

Toutes les modifications notables de Cockpit sont consignées ici.

Format : [Keep a Changelog](https://keepachangelog.com/fr/1.1.0/) —
versionnage : [SemVer](https://semver.org/lang/fr/).

Ce fichier n'est pas décoratif : il est **affiché dans le logiciel** (Paramètres → Général) et
son contenu sert de notes de version à la Release GitHub, donc au message que voient les
utilisateurs quand la cloche de mise à jour s'allume. Une section `[Unreleased]` vide bloque
le script de release.

## [Unreleased]

## [0.55.0] — 2026-09-04

### Added

- Les terminaux reviennent comme on les a quittés. En rouvrant Cockpit après avoir éteint son
  ordinateur, on retrouve ses onglets de terminal, dans le même dossier, avec le texte qu'ils
  affichaient. Une ligne grise sépare ce qui vient d'avant du shell qui vient de démarrer : le
  shell lui-même ne survit pas à l'extinction, il est relancé.
- Un terminal qui faisait tourner un agent IA le rouvre sur sa conversation. Deux terminaux du
  même projet reprennent chacun la sienne, la plus récente d'abord. Cela marche avec les
  fournisseurs qui savent retrouver leurs conversations passées ; les autres ouvrent un shell.

## [0.54.12] — 2026-09-02

### Fixed

- La frappe dans les terminaux n'arrive plus en retard, avec Retour arrière et Espace qui
  semblaient agir sur la touche précédente : la sortie du terminal est de nouveau affichée dès
  qu'elle arrive, sans attendre une image.
- Le contournement d'affichage pour NVIDIA n'est plus imposé automatiquement : il ralentissait
  toute l'interface. Il reste disponible en lançant l'application avec `COCKPIT_SANS_DMABUF=1`
  si la fenêtre cesse de se redessiner après une veille.

## [0.54.11] — 2026-09-02

### Fixed

- L'ordre des frappes est conservé dans les terminaux, y compris pour Retour arrière et Espace.

## [0.54.10] — 2026-09-02

### Fixed

- Les sorties importantes d'un terminal sont traitées par petites tranches, pour que le clavier
  garde la main pendant qu'un agent écrit beaucoup.

## [0.54.9] — 2026-09-02

### Fixed

- Les frappes successives, notamment Retour arrière, ne sont plus retardées par l'attente de la
  touche précédente.

## [0.54.8] — 2026-09-02

### Fixed

- Les frappes dans un terminal restent réactives pendant une grosse sortie : l'écriture vers le
  service ne bloque plus la boucle graphique quand le socket est plein.
- Les notes longues ne recalculent plus tout leur contenu Markdown à chaque frappe.
- Les surveillances système et Docker ne lancent plus plusieurs relevés en retard en même temps.

## [0.54.7] — 2026-09-02

### Fixed

- Les gels d'affichage apres une veille ou une reprise sous NVIDIA et Wayland sont evites en
  desactivant automatiquement le chemin DMA-BUF concerne.

## [0.54.6] — 2026-08-31

### Fixed

- En passant d'un écran à l'autre, la fenêtre restait dessinée à son ancienne taille : un
  rectangle de contenu dans un coin, du noir autour, et il fallait redémarrer. Cockpit force
  maintenant le recalcul quand l'écran change de définition.

## [0.54.5] — 2026-08-31

### Fixed

- Cockpit interrogeait Docker **une fois par projet**, l'un après l'autre, toutes les cinq
  secondes. Mesuré sur une installation de 32 projets : plus de six secondes par passage pour une
  période de cinq — le passage n'avait jamais fini avant le suivant, Docker tournait en
  permanence à pleine charge et tout le poste ralentissait. Une seule question suffit désormais,
  quel que soit le nombre de projets.
- La recherche du fichier compose lisait le disque à chaque rafraîchissement de la liste des
  projets, soit 119 ms toutes les cinq secondes sur cette même installation. Le résultat est
  maintenant retenu ; l'ouverture de l'onglet Docker et le bouton « chercher de nouveau » le
  relisent immédiatement.

### Changed

- Amélioration des journaux techniques.

## [0.54.4] — 2026-08-31

### Fixed

- Le retour au chemin d'affichage normal, livré en 0.54.3, restait sans effet : le réglage posé
  par la 0.54.2 se transmettait à la nouvelle version, parce que la mise à jour relance
  l'application depuis l'ancien processus. Cockpit remet désormais cet état à plat à chaque
  démarrage. Si l'interface était lente et que des lettres sautaient, c'est réglé — et il n'y a
  rien à faire de votre côté.

## [0.54.3] — 2026-08-31

### Fixed

- Retour en arrière sur la 0.54.2 : l'interface était devenue plus lente et des lettres
  sautaient en cours de frappe. Deux causes, toutes deux introduites par cette version — un
  chemin d'affichage de secours imposé aux cartes NVIDIA, qui fait composer la page par le
  processeur, et une mesure interne qui interrogeait l'affichage soixante fois par seconde.
  Cockpit reprend le chemin d'affichage normal, et la mesure ne coûte plus rien.
- Contrepartie assumée : sur les cartes NVIDIA, la fenêtre peut de nouveau cesser de se
  redessiner. Le journal indique alors quoi faire, et le réglage reste disponible à la main.

## [0.54.2] — 2026-08-31

### Fixed

- La fenêtre se figeait au hasard, sans autre issue que de fermer Cockpit de force : l'écran
  cessait d'être redessiné alors que l'application, elle, fonctionnait toujours. C'est un défaut
  connu du moteur d'affichage avec le pilote NVIDIA propriétaire ; Cockpit détecte ce pilote et
  emprunte l'autre chemin d'affichage. Sur les autres cartes, rien ne change.

### Changed

- Amélioration des journaux techniques.

## [0.54.1] — 2026-08-28

### Fixed

- Les variables d'environnement de votre shell n'arrivaient toujours pas à Docker : Cockpit
  interrogeait un shell de connexion **non interactif**, qui ne lit pas `~/.zshrc` — là où la
  plupart des `export` sont écrits. Mesuré : 0 variable attendue sur 3 avant, 3 sur 3 après. La
  même correction fait trouver les outils installés dans un dossier ajouté au PATH par `.zshrc`.

## [0.54.0] — 2026-08-28

### Changed

- Plus besoin d'indiquer le fichier compose d'un projet : Cockpit le trouve seul. Il reconnaît
  les noms de docker, leurs variantes suffixées (`docker-compose.local.yml`) et les
  sous-dossiers habituels sur trois niveaux. Les paramètres du projet affichent le fichier
  retenu et permettent d'en choisir un autre quand il y en a plusieurs — parmi ceux qui existent
  vraiment. Le champ où l'on saisissait un chemin a disparu, à la création comme dans les
  paramètres.

### Fixed

- Les commandes Docker s'exécutaient sans les variables d'environnement définies dans votre
  shell. Un fichier compose qui s'appuie sur l'une d'elles échouait donc dans Cockpit alors que
  la même commande marchait dans un terminal, sur un message incompréhensible du genre
  « invalid spec: :/mnt/data: empty section between colons ».
- Deux définitions différentes de « fichier compose » cohabitaient : un projet pouvait être
  reconnu à sa création, puis déclaré sans compose au moment de le démarrer.

## [0.53.5] — 2026-08-28

### Changed

- Amélioration des journaux techniques.

## [0.53.4] — 2026-08-27

### Fixed

- La fenêtre pouvait se figer définitivement, sans autre issue que de fermer Cockpit de force
  et de le relancer. Cela se produisait quand le service qui tient les terminaux cessait de
  répondre : la frappe attendait alors sans limite de temps, et elle attend sur le fil qui
  dessine l'interface. Les terminaux ouverts n'étaient pas perdus pour autant.
- Quand le service ne répondait pas, tous les terminaux étaient annoncés éteints sans qu'aucun
  message ne le dise.
- Dix-huit actions qui lisent ou écrivent des fichiers s'exécutaient sur le fil qui dessine
  l'interface : ouvrir un dossier, enregistrer un fichier, poser une image de fond, sauvegarder
  la base. Instantané sur un disque local, mais un dossier sur un montage réseau qui ne répond
  plus figeait la fenêtre entière.

### Changed

- Amélioration des journaux techniques.

## [0.53.3] — 2026-08-26

### Changed

- Amélioration des journaux techniques.

## [0.53.2] — 2026-08-26

### Fixed

- **La liste de choix de la langue était illisible sur un thème sombre** — texte clair sur fond
  clair, dès qu'une image de fond était active. Les listes déroulantes suivent maintenant le
  thème, celle qui s'ouvre comprise.

## [0.53.1] — 2026-08-26

### Fixed

- **Cockpit annonçait « CLI introuvable » pour un agent parfaitement installé.** Une application
  lancée depuis un menu de bureau n'hérite pas du `PATH` de votre shell : `~/.local/bin`, où
  vivent la plupart des CLI installés par un utilisateur, en est absent. Cockpit cherche
  maintenant aussi là où les outils s'installent vraiment, et lance l'agent par son chemin
  complet — la connexion à l'abonnement échouait pour la même raison.
- **Un bouton désactivé ne se lisait pas comme désactivé** : sur une image de fond, un aplat de
  couleur à moitié transparent ressemble à un bouton tout à fait cliquable.

## [0.53.0] — 2026-08-26

### Added

- **Cockpit n'est plus lié à un seul fournisseur d'IA.** Un nouvel écran, Paramètres → IA, liste
  les fournisseurs et laisse en choisir un : tout le reste suit — les conversations reprises depuis
  un terminal, les comptes rendus de réunion, les agents. Chaque ligne dit ce que le fournisseur
  sait faire sur cette machine (CLI installé, clé posée, conversations, abonnement, rédaction,
  transcription, plugins), et l'interface n'affiche que ce qui existe : plus de bouton qui promet
  ce que votre fournisseur ne sait pas faire.
- **Douze fournisseurs reconnus d'emblée** : Claude Code, OpenAI, Codex, Gemini, Aider, Goose,
  OpenCode, Copilot, Cursor, Amp, Qwen et Ollama. Un agent qui tourne dans un terminal est repéré
  quel qu'il soit.
- **La clé d'API se pose par fournisseur**, dans le même écran. Celle d'OpenAI est reprise telle
  quelle : rien à ressaisir.

### Changed

- **Le bouton des conversations du terminal porte le nom de VOTRE agent**, et disparaît quand
  celui-ci ne garde pas de conversations passées.
- **Le repère d'un terminal où un agent travaille ne porte plus de logo de marque.** Il affichait
  le logo de Claude même quand c'était codex ou gemini qui tournait.
- **Transcription et compte rendu de réunion suivent le fournisseur choisi**, et l'écran des
  réunions AFFICHE qui fera le travail. Un fournisseur qui ne sait pas transcrire laisse la main
  au premier qui sait et qui est configuré — ce n'est plus à découvrir après l'enregistrement.
- L'onglet Plugins et l'écran Agents n'apparaissent que pour un fournisseur dont les agents
  s'installent au format de Claude Code.

### Fixed

- **L'écran de connexion à l'abonnement annonçait des « fonctionnalités IA » qui n'existaient
  pas** (« suggestions de commande »). Il dit maintenant ce qu'il fait vraiment.
- La date d'expiration d'un jeton s'affichait au format français dans l'interface anglaise, et
  l'âge d'une conversation restait en français lui aussi.

## [0.52.1] — 2026-08-25

### Fixed

- **Des textes restaient en français quand l'interface était en anglais** : les échéances des
  tâches (« aujourd'hui », « en retard de 2 j »), l'âge d'un commit et des notifications
  (« il y a 3 min »), l'état d'une adresse surveillée, et les unités de taille de fichier
  (« 50 o » au lieu de « 50 B »). Les dates courtes suivent aussi la langue : 29/08 en français,
  08/29 en anglais.
- **Les tailles de fichier ne s'écrivaient pas pareil d'un écran à l'autre** — « Mo » dans les
  fichiers d'un projet, « MB » dans les process, quelle que soit la langue choisie.

## [0.52.0] — 2026-08-24

### Changed

- **La connexion Google se fait sans code.** Le navigateur s'ouvre, vous choisissez votre compte,
  la fenêtre se ferme et c'est fini. L'ancien écran affichait un code à comparer qui se lisait
  comme un code à recopier, alors qu'il n'y avait jamais rien à taper.

## [0.51.1] — 2026-08-24

### Fixed

- **macOS : les boutons « nouveau dossier » et « nouvelle note » ne faisaient rien.** Ils
  ouvraient une fenêtre de saisie que macOS n'affiche pas. Même cause pour le bouton lien de
  l'éditeur de notes. Cockpit a maintenant sa propre fenêtre, qui suit le thème et la langue.
- **macOS : l'alerte « mémoire saturée » se déclenchait à tort et ne s'éteignait plus.** La
  mémoire utilisée était déduite d'une soustraction qui ne veut rien dire sur macOS ; elle est
  maintenant demandée au système. Le chiffre affiché dans Monitoring était faux de la même façon.
- **macOS : certains échecs étaient totalement silencieux** — enregistrer les réglages de
  réunion, supprimer un projet, envoyer un code de connexion. Le message passe désormais par les
  notifications de l'application.
- **Le bouton ▶ Cmd emmène là où les commandes se déclarent** au lieu d'indiquer un chemin.
  Il renvoyait vers « Paramètres → Commandes », alors que deux écrans portent le nom
  « Paramètres » et que la section s'appelle « Commandes rapides » : on cherchait au mauvais
  endroit et on concluait que l'option n'existait pas.
- **Les alertes de la cloche parlaient français en anglais** : disque, mémoire, processeur, et
  l'unité « Go » qui restait « Go ».

## [0.51.0] — 2026-08-23

### Changed

- **Cockpit a une vraie adresse : cockpitdesktop.com.** La synchronisation passe désormais par
  `api.cockpitdesktop.com`. L'ancienne adresse continue de fonctionner : rien à faire, et les
  versions plus anciennes ne perdent pas leur compte.

## [0.50.0] — 2026-08-23

### Added

- **L'écran du compte dit ce que le dernier échange a déplacé** — combien est parti, combien est
  arrivé, et que l'échange se fait tout seul toutes les trois minutes. Une date seule ne
  permettait pas de savoir si ça marchait.

### Fixed

- **Ce qu'une machine possédait déjà part enfin vers les autres.** Si vous utilisiez Cockpit
  avant d'avoir un compte, vos projets, notes et tâches ne quittaient jamais ce poste : une
  seconde machine ne recevait rien, et rien ne l'expliquait. Tout est mis en file au prochain
  démarrage, sans rien faire.
- **Le nom affiché et l'image de profil arrivent sur les autres machines.** Ils n'étaient lus
  qu'à la connexion : changés ici, l'autre poste gardait les anciens indéfiniment.

## [0.49.0] — 2026-08-23

### Added

- **Une image de profil se cadre avant d'être envoyée.** Glissez-la pour la placer, agrandissez-la
  si besoin : ce qui est dans le rond est ce qui sera gardé. Avant, seul le centre de l'image
  était retenu — et un visage n'est presque jamais au centre.

### Removed

- **L'adresse du serveur ne s'affiche plus.** Tout le monde passe par le serveur du projet : la
  voir n'apprenait rien et exposait l'hébergement.

### Fixed

- **Le bouton du compte ressemble enfin aux autres boutons de l'en-tête.** Il s'affichait sur un
  fond clair, en plein en-tête sombre.
- **Le rail des curseurs est visible sur un thème sombre.** Quand le curseur était tout à gauche,
  il ne restait qu'un point flottant, sans rien qui dise jusqu'où on pouvait aller.

## [0.48.0] — 2026-08-23

### Added

- **La documentation intégrée explique le compte** : à quoi il sert, où il se trouve, ce que la
  page contient, et ce qui se passe hors connexion. Elle n'en parlait pas du tout.

### Changed

- **Le compte est une page, comme les Paramètres, et non plus une fenêtre par-dessus
  l'application.** On y reste, on y revient, et on peut y aller depuis les Paramètres.
- **Tout ce qui touche au compte est au même endroit.** L'état de la synchronisation, la
  déconnexion et l'adresse du serveur étaient à la fois dans les Paramètres et dans le profil :
  il fallait se souvenir lequel des deux écrans portait quoi.
- **Les Paramètres s'ouvrent bien plus vite.** Ils affichaient l'historique complet des
  versions à chaque ouverture ; ils montrent maintenant les dernières, avec un bouton pour
  dérouler le reste.

### Fixed

- **Les informations de la carte « Compte » des Paramètres sont alignées** comme le reste de la
  page. Leur mise en forme ne s'y appliquait pas.

## [0.47.2] — 2026-08-22

### Fixed

- **Le changelog des Paramètres s'affiche aussi dans la langue de lecture.** Ses titres de
  section restaient en anglais, sur tout l'historique — la correction précédente ne portait que
  sur la fenêtre de mise à jour.

## [0.47.1] — 2026-08-22

### Fixed

- **Les notes d'une mise à jour s'affichent entièrement dans la langue de lecture.** Les titres
  de section restaient en anglais : « Added » au-dessus de puces françaises.
- **Fermer un terminal le ferme vraiment, même quand le programme qui y tourne refuse de
  partir.** Avant, l'onglet affichait « le shell n'a pas rendu la main » et le programme
  continuait de tourner en arrière-plan.

## [0.47.0] — 2026-08-22

### Added

- **Un bouton de compte en haut à droite.** On voit d'un coup d'œil si on est connecté et sous
  quel compte — avant, rien ne le disait. Un clic donne accès au profil et à la déconnexion.
- **Un profil, dans l'application.** Votre nom, votre image, où en est la synchronisation et
  quelles machines sont connectées, sans avoir à ouvrir un navigateur.
- **Un nom affiché et une image de profil.** Ils suivent le compte, donc ils sont les mêmes sur
  toutes vos machines. Sans image, ce sont vos initiales qui s'affichent.

### Changed

- **La création de compte demande une confirmation du mot de passe** et propose de choisir un
  nom affiché. Une faute de frappe sur un champ qu'on ne voit pas se découvrait sinon à la
  première connexion.
- **Le bouton de connexion par le navigateur dit ce qu'il fait.** Il n'annonce Google que si le
  serveur sait vraiment le faire ; sinon il propose simplement de se connecter dans le
  navigateur, ce qui marche dans les deux cas.

## [0.46.1] — 2026-08-22

### Fixed

- **Une mise à jour qui échoue dit maintenant pourquoi.** Le message était « Installation de la
  mise à jour impossible », sans rien de plus : il n'y avait rien à faire de cette phrase. Le cas
  le plus courant est nommé — Cockpit a été déplacé ou renommé depuis son installation, donc il ne
  retrouve plus son propre fichier — avec ce qu'il faut faire pour s'en sortir. Et quand la cause
  n'est pas reconnue, elle s'affiche sous le message plutôt que de finir seulement dans un journal.

## [0.46.0] — 2026-08-21

### Added

- **Un compte Cockpit, pour retrouver ses projets sur une autre machine.** À la première
  ouverture, Cockpit propose de se connecter ou d'en créer un — par mot de passe, ou avec
  Google. Continuer sans compte ne retire rien : tout fonctionne pareil, y compris hors
  connexion. Le compte se gère ensuite dans Paramètres → Général, où l'on voit sous quel nom
  cette machine apparaît et où l'on peut la déconnecter.
- **Vos projets, dossiers, tâches, liens, commandes et notes suivent d'une machine à l'autre.**
  Ce que vous changez ici arrive là-bas, et ce que vous supprimez reste supprimé. Ce qui se
  contredit se tranche sur la modification la plus récente. Une seule chose ne voyage pas : le
  dossier d'un projet, qui n'existe pas sur l'autre machine — un projet qui arrive attend que
  vous lui indiquiez le sien. Tout continue de fonctionner sans réseau ; ce qui n'est pas parti
  part au retour.

### Changed

- **L'écran affiché au tout premier lancement est maintenant celui de la connexion.** Le
  réglage de la remontée des erreurs reste là où il était, dans Paramètres → Général.

## [0.45.1] — 2026-08-21

### Fixed

- **Le curseur d'avancement n'entre plus en conflit avec le déplacement d'une tâche.** Le
  déplacement est désactivé dès que le pointeur arrive sur le curseur, et non au moment du clic —
  entre les deux, le navigateur avait le temps de démarrer un glisser.


## [0.45.0] — 2026-08-21

### Added

- **Supprimer une tâche, un lien rapide, une commande rapide ou un dossier de projets demande
  confirmation.** Ces quatre suppressions étaient immédiates, sans rien demander. La question
  nomme ce qui va disparaître.

### Fixed

- **Le curseur d'avancement ne déclenche plus le déplacement de la tâche.** Tirer le curseur
  démarrait un glisser-déposer, et les deux gestes se battaient. La ligne sort du déplacement le
  temps du réglage.


## [0.44.0] — 2026-08-21

### Changed

- **Les confirmations de suppression sont des fenêtres de l'application**, à la place de la
  boîte grise du système. Elles suivent le thème et la langue, se ferment avec Échap, et le
  bouton qui détruit est en rouge. Les vingt-quatre endroits qui demandaient confirmation y
  passent : fichiers, notes, projets, branches, worktrees, conteneurs, volumes, images, agents,
  enregistrements, processus.
- **Le zoom démarre un cran plus grand**, et c'est lui qui s'affiche « 100 % » : c'est le rendu
  le plus confortable à l'usage. Si tu avais gardé le zoom par défaut, tu passes
  automatiquement au nouveau ; si tu en avais choisi un autre, il ne bouge pas — seule son
  étiquette change, puisque les pourcentages se comptent maintenant depuis le nouveau défaut.


## [0.43.2] — 2026-08-21

### Fixed

- La barre d'avancement d'une tâche est **plus grande** — presque deux fois plus large dans le
  tableau de bord, et plus épaisse. Elle était trop petite pour être visée confortablement.


## [0.43.1] — 2026-08-21

### Fixed

- **La barre d'avancement d'une tâche se voit sur les thèmes sombres.** Elle s'appuyait sur le
  rendu par défaut du système : la partie non remplie se confondait avec le fond, et il ne
  restait qu'un point flottant. Elle porte maintenant ses propres couleurs, lisibles aussi sur
  une image de fond.


## [0.43.0] — 2026-08-21

### Added

- **Avancement d'une tâche, de 0 à 100 %.** Chaque tâche porte un curseur, réglable au clic ou
  aux flèches par pas de 10. On voit d'un coup lesquelles sont en cours et où elles en sont —
  dans la colonne Tâches du projet comme dans le tableau de bord. À 100 % la tâche passe en
  terminée, et redescendre la rouvre.


## [0.42.0] — 2026-08-21

### Fixed

- Barre latérale : le nom d'un dossier de projets s'affichait plus petit que les projets
  qu'il contient. Il est maintenant à la même taille.

### Added

- **Worktrees git.** Un worktree est un second dossier de travail sur le même dépôt, sur une
  autre branche : de quoi faire tourner plusieurs agents en parallèle sans que l'un change le
  code sous les pieds des autres. Onglet Git → **Worktrees** : tape un nom de branche pour en
  créer un (le bouton dit s'il va créer la branche ou réutiliser une existante), **▶** ouvre un
  terminal directement dedans, **🗑** le supprime. Ils sont rangés à côté du projet, dans
  `<ton-projet>.worktrees/`, et le chemin complet est toujours affiché.

## [0.41.4] — 2026-08-21

### Fixed

- **L'alerte « disque presque plein » ne se déclenche plus sur Cockpit lui-même.** La
  correction de la version précédente ne marchait pas : elle cherchait le mauvais indice.
  Cockpit écarte maintenant tout emplacement où il n'y a rien à libérer, parce qu'on ne peut
  pas y écrire.


## [0.41.3] — 2026-08-21

### Fixed

- **L'alerte « disque presque plein » ne se déclenche plus sur Cockpit lui-même.** Sous Linux,
  l'AppImage se monte comme un disque en lecture seule, rempli à 100 % par nature : la cloche
  annonçait donc un disque saturé à chaque lancement. Ce montage n'apparaît plus dans le
  monitoring non plus — il n'y a rien à y libérer.

## [0.41.0] — 2026-08-21

### Changed

- **En installant cette version, tu perds tes terminaux ouverts — une seule fois.** Cockpit
  ne s'appuie plus sur tmux pour les garder en vie : il s'en occupe lui-même, donc les
  terminaux d'avant ne sont pas repris. Ils tournent encore en fond si tu en as besoin :
  `tmux -L cockpit attach` les retrouve, `tmux -L cockpit kill-server` les arrête pour de
  bon. Après cette mise à jour, fermer Cockpit ne fait plus rien perdre.
- **Les terminaux ne dépendent plus de tmux.** Cockpit tient désormais lui-même les shells,
  dans un service à lui qui survit à la fermeture de l'application : on rouvre Cockpit, les
  terminaux sont là où on les avait laissés, écran et historique compris. Ils répondent plus
  vite (0,06 ms de retard sur la frappe au lieu de 0,4 ms), et il n'y a plus aucun programme
  externe à installer.
- La recherche dans un terminal affiche maintenant le nombre d'occurrences et surligne
  celle en cours dans la couleur d'accent, au lieu du compteur discret que tmux dessinait
  dans un coin. **Ctrl+C** copie la sélection quand il y en a une affichée, et interrompt
  sinon — comme avant, mais c'est désormais Cockpit qui s'en charge.
- Le monitoring système affiche désormais **tous les disques locaux**, et plus seulement six
  points de montage écrits en dur : un disque monté sur `/mnt/data` ou `/srv` était invisible
  sans aucune explication.
- Le badge à côté du nom de la machine annonce maintenant **le système et sa version**
  (« Ubuntu 26.04 ») plutôt que le numéro de version du noyau, et la durée de fonctionnement
  s'affiche enfin en anglais quand l'interface est en anglais.
- Le détail de la mémoire (Processus, Cache, Partage, Buffers, ZFS ARC) n'est publié que par
  Linux : ailleurs, le panneau se masque au lieu d'afficher cinq barres à zéro, et le survol
  du titre explique pourquoi.
- La croix qui arrête un processus dit ce qu'elle fait au survol, se désactive pendant
  l'opération, et signale un échec par une notification au lieu d'une boîte de dialogue.
- Paramètres → Claude & IA : quand le statut de connexion n'a pas pu être **déterminé**
  (fichier de jetons illisible, dossier personnel introuvable), l'écran le dit au lieu
  d'afficher « non connecté » et de proposer une connexion qui ne changera rien.
- **L'enregistrement de réunions marche maintenant sur Windows**, micro et son système
  compris, sans rien installer. Sur Linux, il n'a plus besoin de `pw-record` ni de
  `parecord` : Cockpit enregistre lui-même, ce qui règle du même coup les machines où
  PipeWire répondait sans exposer le moindre micro. Sur macOS, la capture demande une
  autorisation liée à la signature de l'application : sans elle, les pistes ressortent
  entièrement muettes, et Cockpit le **dit** désormais au lieu d'annoncer qu'aucune parole
  n'a été détectée.
- Amélioration des journaux techniques.

### Added

- **Cockpit s'installe maintenant sur Windows.** Un installeur `.exe` est publié à chaque
  version, à côté de l'AppImage Linux et du `.dmg` macOS. Terminaux persistants, fichiers,
  Git, notes et enregistrement de réunions y marchent sans rien installer d'autre. C'est la
  toute première version Windows : la suite de tests y passe en entier, mais personne ne s'en
  est encore servi une journée. Si quelque chose ne va pas, ouvrir une issue est ce qui
  aidera le plus.

### Fixed

- **Une grosse sortie de terminal n'est plus perdue.** Une commande qui écrit beaucoup — un
  `seq`, un log de build — voyait la plus grande partie de son texte disparaître : on
  remontait à la molette et on ne le retrouvait pas. Sur macOS, sur 1,3 Mo affichés, il n'en
  restait que 368 Ko.
- **Fermer un terminal dont le programme s'est déjà arrêté ne signale plus d'erreur.** On tape
  `exit`, on ferme l'onglet, et une notification d'erreur s'affichait — alors que la
  fermeture avait parfaitement fonctionné.
- **Les chemins de fichiers du projet s'écrivent toujours avec des `/`.** Sous Windows ils
  sortaient avec des antislashs, ce qui cassait l'arbre de l'onglet Fichiers : un dossier ne
  se dépliait plus, et le nom affiché devenait le chemin entier.
- **L'affichage du terminal ne saccade plus quand une commande écrit beaucoup.** Sur macOS,
  le texte arrivait à l'écran par tout petits morceaux, et la fin pouvait même manquer si on
  fermait le terminal juste après. Sous Linux, l'affichage est aussi nettement plus fluide
  qu'avant.

## [0.37.1] — 2026-08-20

### Fixed

- Une commande rapide (bouton **▶ Cmd** ou palette `Ctrl+K`) et le shell d'un conteneur
  (onglet Docker) ouvrent enfin leur terminal **à la taille de la fenêtre**. Ces trois
  raccourcis créaient la session en 80 colonnes sur 24 lignes : une application plein écran
  lancée par une commande — `k9s`, `htop`, `top` — se dessinait dans un petit carré en haut à
  gauche et y restait, et un simple shell coupait ses lignes bien avant le bord.

## [0.37.0] — 2026-08-20

### Added

- **Les dossiers de projets s'imbriquent, sans limite de profondeur.** Un dossier dans un
  dossier dans un dossier, autant de niveaux que voulu. Pour créer un sous-dossier : « +▸ » au
  survol de l'en-tête d'un dossier, ou clic droit → **Nouveau sous-dossier**. Un dossier se
  déplace au glisser-déposer, et la ligne visée dit ce qui va se passer : le **milieu** le range
  DEDANS (cadre bleu), le **haut** et le **bas** le réordonnent À CÔTÉ (trait bleu). Déposer dans
  la zone du bas — ou clic droit → **Sortir du dossier** — le ramène au premier niveau. Un
  déplacement impossible (un dossier dans un de ses propres sous-dossiers) se signale en rouge
  pendant le glisser et l'explique au lâcher. Le compteur d'un dossier compte désormais les
  projets de **toute** sa branche, un dossier vide dit quoi en faire, et la suppression reste
  réservée aux dossiers vides — sous-dossiers compris.

## [0.36.0] — 2026-08-20

### Added

- Les adresses écrites dans une tâche sont maintenant **soulignées et cliquables** :
  `Ctrl`+clic ouvre le lien dans le navigateur, ou le client mail pour une adresse `@`. Le clic
  simple continue d'ouvrir l'édition du texte. Vaut dans la colonne Todos d'un projet comme dans
  le tableau de bord.

### Fixed

- Tableau de bord → Tâches : l'échéance d'une tâche se **modifie sur place** (clic sur le badge)
  et se **pose** (📅 au survol de la ligne). Le badge n'était qu'un affichage, alors que c'est
  l'écran où on trie ses tâches.

## [0.35.0] — 2026-08-20

### Added

- **Renommer un projet se voit enfin.** Clic droit sur un projet dans la barre latérale →
  **Renommer**, ou double-clic sur son nom : les mêmes gestes que pour un dossier ou un
  terminal. Dans la barre du projet, un crayon apparaît au survol du titre et le double-clic
  reste là. Renommer vers un nom déjà pris explique le problème au lieu d'afficher un message
  technique — à la création d'un projet aussi.

### Fixed

- Les menus au clic droit font enfin ce qu'ils annoncent. **Renommer** et **Fermer** sur un
  terminal de la barre latérale, **Renommer** et **Supprimer** sur un dossier de projets, et
  toutes les entrées du menu de l'arbre de l'onglet Fichiers (nouveau fichier, nouveau
  dossier, renommer, copier le chemin, corbeille) ne faisaient rien : le menu se refermait et
  l'action était perdue sans un mot.

## [0.34.1] — 2026-08-20

### Fixed

- Les commandes rapides d'un projet (bouton **▶ Cmd**, palette Ctrl+K) et le shell d'un
  conteneur (onglet Docker) ouvrent enfin leur terminal. Quand l'onglet Terminal était déjà
  affiché, la session était bien créée — elle apparaissait dans la barre latérale — mais aucun
  onglet ne s'ouvrait et rien ne l'expliquait : chaque nouvel essai laissait un terminal de
  plus derrière lui. Un clic sur un terminal de la barre latérale ou du tableau de bord ouvre
  lui aussi la bonne session, et si celle-ci s'est terminée entre-temps, c'est écrit à l'écran.
- Terminal : renommer un onglet ne peut plus faire mentir l'affichage. Quand l'enregistrement
  du nouveau nom échouait, l'onglet le montrait quand même et l'ancien nom revenait au retour
  sur le projet, sans un mot. Un onglet retiré parce que sa session tmux avait disparu
  s'explique aussi, au lieu de s'évaporer, et Copier ou la recherche sans terminal ouvert
  disent pourquoi il n'y a rien à faire.

## [0.34.0] — 2026-08-20

### Added

- Notes : un bouton **▸◂ Lecture** dans l'en-tête de la note replie d'un coup la liste des
  notes et la colonne des tâches. Le compte rendu occupe alors toute la zone, centré et borné
  pour rester lisible. Le même bouton (ou Échap) ramène les deux colonnes, et l'endroit où on
  lisait — comme le curseur de saisie — est conservé dans les deux sens. Le choix est retenu
  d'une session à l'autre.

## [0.33.2] — 2026-08-20

### Fixed

- L'interface ne se fige plus par à-coups. Toutes les cinq secondes, le rafraîchissement
  de la liste des terminaux gelait toute la fenêtre — jusqu'à une seconde entière quand des
  agents tournaient, c'est-à-dire précisément quand on s'en sert. Frappe, affichage et
  défilement restent fluides pendant ce rafraîchissement.
- Terminal : les grosses sorties (compilation, log, `cat` d'un gros fichier) s'affichent
  beaucoup plus vite et sans saccade. La frappe garde exactement la même réactivité.
- Le démarrage de l'application est plus rapide de ~150 ms.

## [0.33.1] — 2026-08-20

### Fixed

- Terminal : le clic molette colle une seule fois. Le presse-papier partait deux fois dans
  le terminal, avec le même texte : la commande était doublée, ou le début de la seconde
  copie s'ajoutait à la fin de la première.
- Terminal : coller quand le presse-papier est vide, ou sans terminal ouvert, le dit au lieu
  de ne rien faire.
- Barre latérale : après un redémarrage de la machine, les terminaux de la session précédente
  restaient affichés sans pouvoir être fermés. Ils disparaissent maintenant au lancement.

### Changed

- Amélioration des journaux techniques.

## [0.32.1] — 2026-08-20

### Fixed

- macOS : les mises à jour repartent. La version 0.32.0 est sortie sans archive macOS parce
  qu'une vérification interne échouait sur ce système uniquement — les utilisateurs Linux
  n'étaient pas concernés.

## [0.32.0] — 2026-08-20

### Added

- Éditeur de notes : bouton **¶** pour revenir au texte normal. Un titre, une citation, un
  élément de liste ou un bloc de code redevient un paragraphe — jusqu'ici, une ligne passée
  en titre ne pouvait plus en sortir. Le gras, l'italique et les liens sont conservés.

### Changed

- Chaque projet se souvient de son onglet : aller voir autre chose puis revenir sur un projet
  le retrouve là où on l'avait laissé, au lieu de repartir de Workspace à chaque aller-retour.
  Les raccourcis qui visent un onglet précis (un terminal depuis la barre latérale, le tableau
  de bord ou la palette Ctrl+K, une tâche en retard) continuent d'y emmener directement.
- Onglet Terminal : revenir sur un projet laissé sur cet onglet n'ouvre plus de session tout
  seul quand il n'y en a aucune. L'écran propose de l'ouvrir, ce qui évite de créer des
  terminaux en série juste en parcourant ses projets.

### Fixed

- Onglet Fichiers : un fichier resté ouvert suit ce qui se passe sur le disque. Quand un agent
  le réécrit dans un terminal Cockpit, le contenu affiché se met à jour tout seul en moins de
  deux secondes, sans faire bouger la position de lecture — jusqu'ici l'onglet montrait
  l'instantané pris à l'ouverture et il fallait recliquer le fichier dans l'arbre. Une
  modification qui arrive pendant qu'on édite n'écrase rien : elle est signalée par un bandeau,
  avec un bouton pour prendre la version du disque. Un fichier disparu se dit aussi.
- Onglet Fichiers : taper dans l'éditeur affiche les caractères tout de suite. La coloration
  attend une pause de frappe, et comme cette pause repartait de zéro à chaque touche, une
  frappe continue ne réaffichait rien : on tapait dans le vide. Le texte apparaît maintenant
  sans attendre et les couleurs se posent à la pause.

## [0.31.2] — 2026-08-20

### Fixed

- Éditeur de notes : un bloc de code en fin de note ne bloque plus la saisie. On en sort en
  appuyant deux fois sur Entrée (ou Ctrl+Entrée), le bouton `</>` le défait quand le curseur
  est dedans, et une note finit toujours par une ligne où écrire. Entrée à l'intérieur d'un
  bloc ajoute désormais une ligne de code au lieu d'ouvrir un deuxième bloc.
- Éditeur de notes : un bloc de code n'est plus perdu à l'enregistrement. Il repartait en
  simple paragraphe, et les blocs de plusieurs lignes voyaient leurs lignes se recoller.
- Notes : le Ctrl+clic sur une adresse mail ouvre le client mail au lieu d'afficher une
  erreur. Un lien incomplet (`www.exemple.com`, `../doc.md`) explique maintenant qu'il
  manque une adresse complète, au lieu de remonter un message technique.
- Interface en anglais : quelques libellés restaient en français — le message qui explique
  pourquoi un dossier de projets ne peut pas être supprimé, l'infobulle des terminaux de la
  barre latérale, celle du nom de projet à renommer, et l'aide des boutons Docker quand le
  projet n'a pas de fichier compose.
- Interface en anglais, suite : une quarantaine d'autres libellés restaient en français —
  onglet Conteneurs (volumes non utilisés, images sans tag, boutons de nettoyage), compteur
  de tâches et échéances, cœurs du monitoring, recherche de fichiers, retard sur l'upstream
  dans Git, réglages d'apparence et vue Agents. Y compris des messages d'erreur : chemin de
  projet inconnu dans les onglets Fichiers et Git, aperçu d'un fichier binaire, nom refusé à
  la création d'un projet ou d'un agent.

## [0.31.1] — 2026-08-20

### Fixed

- **« Vérifier la mise à jour » n'affiche plus d'erreur technique.** Quand une version vient
  d'être publiée mais que le fichier d'installation de ton système n'est pas encore en ligne,
  Cockpit le dit en une phrase et t'invite à réessayer. Même chose hors connexion : un message
  clair au lieu du texte anglais du composant de mise à jour.
- **Une nouvelle version n'est plus annoncée avant d'être installable.** Elle n'apparaît
  désormais que lorsque le fichier d'installation de chaque système est effectivement publié.
- **La notification « Mise à jour disponible » suit la langue choisie** — elle restait en
  français en anglais.

## [0.31.0] — 2026-08-19

### Added

- **Les liens des notes s'ouvrent maintenant, au Ctrl+clic**, dans ton navigateur. Le clic
  simple reste réservé à l'édition du texte, comme dans les autres éditeurs — et comme le
  Ctrl+clic du terminal et de l'onglet Fichiers. Au survol avec Ctrl, le curseur indique que
  le lien est actif. Seules les adresses http, https et mailto sont ouvertes ; toute autre est
  refusée avec un message plutôt qu'en silence.

## [0.30.0] — 2026-08-19

### Added

- **La transcription complète devient optionnelle dans les comptes rendus de réunion**
  (Paramètres → Réunions). Elle était ajoutée par Cockpit après le résumé, donc demander dans
  le prompt de ne pas l'inclure n'avait aucun effet — c'est maintenant une case à cocher, et
  le prompt n'a plus à s'en occuper.

### Fixed

- **Supprimer une note demande confirmation.** Un compte rendu de réunion disparaissait sur un
  simple clic, sans retour possible.

## [0.29.5] — 2026-08-19

### Fixed

- **Cockpit ne gèle plus à l'ouverture d'un terminal sur les distributions récentes.** Sur
  Fedora, afficher un emoji dans le terminal figeait la fenêtre sans message : le moteur
  d'affichage embarqué est plus ancien que les polices emoji en couleur du système. Ces
  polices sont maintenant écartées pour Cockpit seulement — les emoji s'affichent avec la
  police de remplacement, le reste du système n'est pas touché.
- **Les outils lancés dans un terminal Cockpit retrouvent l'environnement de la machine.**
  L'application transmettait aux terminaux des chemins qui la concernaient elle seule, ce qui
  faisait échouer des outils comme `mise` — sans que ce soit un problème d'installation. Les
  listes de chemins du système (PATH, dossiers de données) gardent maintenant tout ce qui
  appartient à l'utilisateur et perdent seulement ce qui venait de Cockpit.

## [0.29.4] — 2026-08-19

### Fixed

- **Ce qui est tapé ou collé dans un terminal n'est plus envoyé plusieurs fois.** Chaque
  retour sur un terminal (changement d'onglet, de projet) rebranchait son entrée sans
  débrancher la précédente : après deux passages le texte partait deux fois, après trois,
  trois fois. C'est l'origine du collage en double au clic molette, et cela touchait aussi la
  frappe.

## [0.29.3] — 2026-08-19

### Fixed

- **Cockpit ne s'ouvre plus en double, et une seconde fenêtre ne peut plus détruire tes
  terminaux.** Deux instances partagent la même base et le même serveur de terminaux, et
  chacune supprime au démarrage les sessions qu'elle ne connaît pas : une seconde instance
  faisait donc disparaître les terminaux de la première. Relancer Cockpit ramène désormais la
  fenêtre déjà ouverte.
- **Une installation de développement ne touche plus aux terminaux de l'installation
  normale** (lancement avec une base choisie à la main).

## [0.29.2] — 2026-08-19

### Changed

- Amélioration des journaux techniques.

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
