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

Un trait Rust `Terminaux` : créer, attacher, écrire, redimensionner, fermer, lister,
sélectionner, chercher. L'implémentation tmux actuelle passe derrière, sans changer un
seul comportement.

Valeur propre de cette étape, indépendamment de la suite : elle **prouve** que l'interface
dont Cockpit a besoin est petite, et elle la fige **avant** qu'on écrive le service. Sans
ça, le service serait dessiné à l'image de tmux — donc avec ses défauts.

**État : à faire.**

### B. Écrire le service, sans le brancher

Le service tient les shells et leur écran. Il tourne à part et survit à la fermeture de
l'app. Personne ne l'utilise encore : il se teste seul.

Écrire **en premier** la partie qui redessine l'écran au retour, avec son test : c'est là
qu'est tout le risque. Le test est un aller-retour — on redessine, on relit dans un
émulateur neuf, les deux doivent être identiques cellule par cellule. Le nourrir avec de
vraies traces de `claude`, `vim`, `htop`, `less`.

Trois choses à décider dès le départ, parce qu'on ne revient pas en arrière dessus :

1. **Le service n'écrit rien sur disque.** Il tient tout en mémoire et meurt avec la
   machine. Ça correspond au besoin — survivre à la fermeture de l'app, pas au
   redémarrage — et ça supprime toute question de migration de format plus tard.
2. **Un numéro de version dans la poignée de main, dès la première version.** Le service
   survit à l'app, donc une app neuve parlera un jour à un service ancien. Sans ce numéro
   on hérite du « protocol version mismatch » de tmux, avec en plus la responsabilité.
3. **Un service par utilisateur, pas un service système.** Les terminaux appartiennent à
   une session utilisateur : son environnement, son presse-papier, son `HOME`.

**État : à faire.**

### C. Brancher, puis supprimer tmux

Basculer l'implémentation derrière le trait, vérifier que tout marche, puis **retirer
vraiment** le code devenu inutile :

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
