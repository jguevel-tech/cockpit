# Portabilité Linux · macOS · Windows

Cinq études faites le 2026-08-20, une par domaine, en lecture seule. Tout ce qui est
affirmé dedans est mesuré sur la machine ou vérifié dans les sources — pas de mémoire.

| Fichier | Domaine |
|---|---|
| [terminaux.md](terminaux.md) | Terminaux et persistance. **Le domaine décisif.** |
| [audio.md](audio.md) | Capture audio des réunions sur les trois systèmes |
| [systeme.md](systeme.md) | Métriques, processus, détection des agents IA |
| [livraison.md](livraison.md) | Construction, signature, distribution, mise à jour |
| [divers.md](divers.md) | Chemins, commandes externes, Docker, Git, rendu, raccourcis |

## Ce qu'il faut retenir sans les lire

**Tauri n'est pas la limite.** Tout ce que Tauri fournit — fenêtre, webview, IPC,
updater, empaquetage — marche déjà sur les trois systèmes. Ce qui ne se porte pas, c'est
ce que nous avons écrit nous-mêmes pour aller chercher ce que Tauri ne fournit pas :
tmux pour la persistance des terminaux, `pw-record` pour les réunions, `/proc` pour le
détail mémoire, `SIGTERM` pour arrêter un processus, `GTK_IM_MODULE` pour le bug des
accents.

**La couche PTY est déjà portable.** `portable-pty`, déjà une dépendance, contient déjà
ConPTY pour Windows. Seul le multiplexeur ne l'est pas.

**Un émulateur de terminal côté serveur est obligatoire** si l'on quitte tmux : sur
treize éléments d'état à conserver, un seul se prête au rejeu d'octets bruts. Mais c'est
justement la partie qu'on n'écrit pas — `alacritty_terminal` la fournit.

**« Le terminal tmux est lent » était fondé, et tmux n'y était pour rien.** tmux ajoute
0,4 ms à une frappe. Les quatre vraies causes ont été corrigées en v0.33.2 : un `ps -e`
de 42 ms sur la boucle principale toutes les 5 s (jusqu'à 1 074 ms sous charge), un
décodage base64 seize fois trop lent, l'absence de regroupement des sorties, et 157 ms de
forks au démarrage.

**Ce qui coûte de l'argent, pas du code** : sans certificat, macOS refuse le premier
lancement d'un dmg téléchargé (le « clic droit → Ouvrir » a été retiré dans macOS 15) et
Windows affiche trois avertissements en cascade. Décision du 2026-08-20 : pas de
certificat pour l'instant, distribution par la page des releases, pas de store.

**Le piège le plus sous-estimé** : le PATH d'une application graphique sur macOS. Rien ne
casse en développement — `tauri dev` hérite du bon PATH — et la panne n'apparaît qu'après
empaquetage, chez l'utilisateur, sous la forme de cinq pannes indépendantes (`docker`
introuvable, `claude` absent, aucun serveur LSP) alors qu'il n'y a qu'une cause.
