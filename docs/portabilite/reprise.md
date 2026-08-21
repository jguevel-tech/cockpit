# Reprise — à lire en PREMIER

État au 2026-08-21, 13h. Écrit pour qu'une session qui reprend n'ait rien à redemander.

## En une phrase

Le chantier de portabilité est **fini** : tmux remplacé par notre service de terminaux,
capture audio dans le processus, et **les trois systèmes** (Linux, macOS, Windows) sortent de
la CI. Reste à publier une version complète — voir « Où en sont les versions » ci-dessous.

## Où en sont les versions

| Version | État | Pourquoi |
|---|---|---|
| **0.37.1** | **servie aux utilisateurs**, complète | c'est elle que `latest.json` rend |
| 0.38.0 | publiée puis passée en **préversion** | `latest.json` n'avait que Linux (le test de rafale tombait sur macOS) |
| 0.39.0 | publiée puis passée en **préversion** | même cause, deuxième symptôme (sortie tronquée) |

Le repli est le geste documenté dans le `CLAUDE.md` : `gh release edit vX.Y.Z --prerelease
--latest=false`. `releases/latest` exclut les préversions, donc l'endpoint retombe aussitôt
sur la dernière version COMPLÈTE et personne ne voit d'erreur d'updater. **À faire AVANT de
diagnostiquer**, chaque fois qu'une release sort incomplète.

## La leçon qui a coûté deux versions

Un essai qui ne tombe que sur macOS ou Windows ne se découvrait **qu'après le tag**, donc
après publication. D'où `.github/workflows/essais.yml` (`gh workflow run essais.yml`) :
matrice macOS + Windows, mêmes vérifications que la release, installeur Windows en artefact,
et **il ne publie rien**. Le réflexe : le lancer avant `npm run release` dès que le changement
touche le service de terminaux ou quoi que ce soit qui dépende du système.

## Ce qui a été vérifié avant le tag, à la main

| Vérification | Résultat |
|---|---|
| `npm run check` | 0 erreur, 0 avertissement |
| `npm run test:front` | 12 verts |
| `npm run i18n:audit` | 0 chaîne en dur |
| `cargo check --all-targets` (Linux) | 0 avertissement |
| `cargo test --release` | **238 verts**, 1 ignoré (l'essai qui demande une carte son) |
| `cargo check --target x86_64-pc-windows-gnu --all-targets` | 0 erreur, 0 avertissement |
| `tauri build --no-bundle` | OK |
| Enregistrement réel sur cette machine, **sans jouer un son** | micro 64 ko en 2 s, crête 3243 ; son système 69 ko, crête 0 (le monitor s'ouvre et débite, muet parce que rien ne jouait) |

L'enregistrement se vérifie par `cargo test --lib capture_reelle -- --ignored --nocapture`.
**Ne jamais faire jouer un son pour ça** : router un ton vers un sink nul et capter son
monitor (recette dans les Pièges connus du `CLAUDE.md`). Les enceintes de quelqu'un ne sont
pas un banc de test.

## Deux prérequis locaux, sans droits administrateur

Les deux suivent la même recette : `apt-get download`, `dpkg-deb -x` dans un préfixe à soi.

- **`libasound2-dev`** — sinon `alsa-sys` échoue et **aucun** essai Rust ne tourne. Poser
  `PKG_CONFIG_PATH=<préfixe>/usr/lib/x86_64-linux-gnu/pkgconfig` et
  `PKG_CONFIG_SYSROOT_DIR=<préfixe>`.
- **mingw-w64** pour la compilation croisée Windows — sinon `ring` et `libsqlite3-sys`
  échouent avant que notre code soit analysé, et on croit à tort que le portage est cassé.
  Le binaire s'appelle `x86_64-w64-mingw32-gcc-13-win32`, **pas** `-gcc`.

Recettes complètes dans les Pièges connus du `CLAUDE.md`.

## Windows : les treize échecs venaient des ESSAIS, pas du produit

Le premier vrai passage (v0.38.0) faisait tomber treize essais, dont l'écriture dans le PTY
(« The operation completed successfully. (os error 0) ») — de quoi conclure que le cœur était
cassé. C'était faux. Deux causes, toutes deux dans les essais :

| Cause | Correction |
|---|---|
| Cinq essais tapent `printf` / `cat` / `for i in $(seq …)` dans le shell — `cmd.exe` n'en connaît aucun, et un essai qui part en boucle d'attente (30 s de `PATIENCE`) fait tomber ceux d'à côté | `#[cfg(unix)]`, avec sur place ce qui reste couvert et ce qui ne l'est pas |
| Un socket local n'est pas un fichier sous Windows, c'est un tuyau nommé (`\\.\pipe\`) — le code de production le savait, les essais non | `emplacement()` rend un nom de tuyau sous Windows |

**Résultat : la suite entière passe sur `windows-latest`**, y compris les essais qui écrivent
dans le PTY et attendent la réponse d'un vrai shell. La CI produit un installeur NSIS de
~6,6 Mo. Windows est donc **revenu** dans la matrice de `release.yml` et dans
`PLATEFORMES_ATTENDUES`.

Ce qui n'est toujours pas vérifié : que l'application soit **agréable** une journée durant
sur Windows. Personne ne s'en est encore servi. Le README le dit.

## Ce qui reste## Ce qui reste
- **macOS : la capture audio rendra du silence** sans certificat Apple — décision prise, pas
  de certificat. Le cas est maintenant **dit** (`mute_track`, le pipeline s'arrête avant
  Whisper) au lieu de finir en « aucune parole détectée ».
- ~~Le processus zombie~~ **réglé comme question** : mesuré le 2026-08-21, notre lancement
  détaché n'en laisse aucun (dix relevés, et un test le verrouille). Les `[cockpit] <defunct>`
  viennent du fork intermédiaire de `g_spawn` (GLib), utilisé par WebKit et les portails. Bénin,
  et hors de notre code.

## Décisions déjà prises, à ne pas rouvrir

- Multiplexeur maison sur les trois systèmes, pas deux mécanismes en parallèle.
- WSL écarté : le terminal verrait les fichiers sous `/mnt/c/` quand les autres onglets les
  voient sous `C:\`. Contraire au principe de l'application.
- Pas de certificat de signature (Apple, Windows). Distribution par la page des releases,
  **pas de store**.
- macOS universel : une seule version pour Intel et Apple Silicon.
- Détail mémoire (Cache, Buffers, ZFS) : **Linux seulement**. Les notions ne se traduisent pas.
- Le service ne persiste rien sur disque et meurt avec la machine.
- Host PulseAudio (Rust pur) sous Linux, **pas** la feature `pipewire` de cpal : elle
  embarquerait libpipewire dans l'AppImage, et la libwayland du runner a déjà coûté une
  fenêtre qui ne s'ouvrait pas chez un testeur.

## À réarmer après une reprise

La surveillance des issues (elle ne survit pas à la session) :

```
Monitor: while true; do node scripts/issues-nouveautes.mjs --brut --marquer \
  --repere=.claude/issues-vues-surveillance.json 2>&1 | grep -E "^(ISSUE #|⚠)" || true; sleep 60; done
```

Sans elle, une réponse d'auteur passe inaperçue — c'est arrivé trois fois le 2026-08-20, et
c'est Jimmy qui a dû le signaler chaque fois.

## Issues en attente

- **#11** (Windows) — à mettre à jour dès que la CI publie l'installeur, puis `attente-retour`.
- **#14** `attente-retour`, **#13** `attente-infos`, **#1** à **#8** `attente-retour`
  (confirmations, fermeture automatique le 30/08).
- **#10** (git worktree) `a-livrer` : promesse tenue, gardée pour la fin.
