# Reprise — à lire en PREMIER

État au 2026-08-21, fin de journée.

## En une phrase

**Le chantier de portabilité est fini et livré.** La v0.43.0 est publiée et servie sur les trois
systèmes : Linux, macOS et Windows. Il n'y a plus de chantier ouvert.

## Ce qui est parti le 2026-08-21

| Version | Contenu |
|---|---|
| 0.41.0 | tmux remplacé par notre service de terminaux, audio dans le processus, **premier installeur Windows** |
| 0.41.3 | affichage du terminal qui ne saccade plus, fin d'un terminal détectée sur le programme, chemins en `/`, fermeture sans fausse erreur |
| 0.41.4 | plus de fausse alerte « disque presque plein » sur l'AppImage elle-même |
| 0.42.0 | worktrees git avec terminal dedans, nom de dossier à la bonne taille dans la barre latérale |
| 0.43.0 | avancement d'une tâche de 0 à 100 % |

Les versions 0.38.0 et 0.39.0 sont restées en **préversion** : sorties sans macOS, repliées dans
la minute. Elles laissent des trous dans la numérotation (0.40, 0.41.1, 0.41.2), qui sont gelés —
on ne renumérote pas ce qui a déjà été servi.

## Ce qui n'est pas vérifié

**Personne n'a utilisé Cockpit une journée entière sous Windows.** La suite de tests y passe,
l'installeur est signé, et le mainteneur l'a installé — mais l'usage réel dira le reste. C'est la seule
zone d'ombre qui reste sur le portage.

## Décisions à ne pas rouvrir

- Multiplexeur maison sur les trois systèmes, pas deux mécanismes en parallèle.
- WSL écarté : le terminal verrait les fichiers sous `/mnt/c/` quand les autres onglets les
  voient sous `C:\`. Contraire au principe de l'application.
- Pas de certificat de signature (Apple, Windows). Distribution par la page des releases, **pas
  de store**. Chaque système affiche donc un avertissement au premier lancement, expliqué dans le
  README.
- macOS universel : une seule version pour Intel et Apple Silicon.
- Détail mémoire (Cache, Buffers, ZFS) : **Linux seulement**. Les notions ne se traduisent pas.
- Le service de terminaux ne persiste rien sur disque et meurt avec la machine.
- Host PulseAudio (Rust pur) sous Linux, **pas** la feature `pipewire` de cpal : elle
  embarquerait libpipewire dans l'AppImage, et la libwayland du runner a déjà coûté une fenêtre
  qui ne s'ouvrait pas chez un testeur.
- **Pas d'étape de vérification en CI avant le tag.** Ça a été essayé et retiré : la CI de
  release vérifie déjà les trois systèmes, et un numéro de version ne coûte rien. Demande
  explicite du mainteneur, deux fois.

## Prérequis locaux, sans droits administrateur

Les deux suivent la même recette : `apt-get download`, puis extraction dans un préfixe à soi.

- **`libasound2-dev`** — sinon `alsa-sys` échoue et **aucun** essai Rust ne tourne. Poser
  `PKG_CONFIG_PATH=<préfixe>/usr/lib/x86_64-linux-gnu/pkgconfig` et
  `PKG_CONFIG_SYSROOT_DIR=<préfixe>`.
- **mingw-w64** pour la compilation croisée Windows — sinon `ring` et `libsqlite3-sys` échouent
  avant que notre code soit analysé, et on croit à tort que le portage est cassé. Le binaire
  s'appelle `x86_64-w64-mingw32-gcc-13-win32`, **pas** `-gcc` ; il faut aussi un lien au nom
  court, que l'outil de ressources appelle ainsi.

Recettes complètes dans les Pièges connus du `CLAUDE.md`.

## À réarmer après une reprise

La surveillance des issues, qui ne survit pas à la session :

```
Monitor: echecs=0; while true; do
  sortie=$(node scripts/issues-nouveautes.mjs --brut --marquer \
    --repere=../.claude/issues-vues-surveillance.json 2>&1)
  if printf '%s' "$sortie" | grep -qiE "error connecting|ENOTFOUND|ETIMEDOUT|EAI_AGAIN"; then
    echecs=$((echecs + 1))
    [ "$echecs" -eq 5 ] && echo "⚠ GitHub injoignable depuis cinq minutes"
  else
    echecs=0; printf '%s' "$sortie" | grep -E "^(ISSUE #|⚠)" || true
  fi
  sleep 60
done
```

Le comptage d'échecs n'est pas décoratif : un hoquet réseau n'est pas un événement — il n'y a rien
à en faire — mais un silence complet ferait passer une panne durable pour « rien de neuf ».

Sans cette surveillance, une réponse d'auteur passe inaperçue : c'est arrivé trois fois le
2026-08-20, et il a fallu à chaque fois qu'on nous le signale.

## Issues ouvertes

- **#16** et **#15** : livrées (0.42.0 et 0.43.0), en attente du retour de gmarchault. Elles se
  ferment d'elles-mêmes après 24 h de silence, règle posée le 2026-08-21.
- **#13** : en attente de précisions de gmarchault sur les liaisons entre notes et réunions.

Tout le reste est fermé.
