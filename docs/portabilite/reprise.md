# Reprise — à lire en PREMIER

État au 2026-08-21, 11h35. Écrit pour qu'une session qui reprend n'ait rien à redemander.

## En une phrase

Tout est committé sur `main`, **rien n'est publié**, et une seule chose bloque : l'audio des
réunions. Jimmy a demandé qu'il parte dans la même version que le reste.

## Ce qui est fini, vérifié, et attend un tag

| Sujet | Preuve |
|---|---|
| **tmux remplacé par notre service de terminaux** | 13 fonctionnalités vérifiées une par une sur le binaire, liste dans [plan-terminaux.md](plan-terminaux.md). Jimmy l'utilise depuis ce matin et trouve les terminaux plus rapides. |
| **Portage Windows du code** | `cargo check --target x86_64-pc-windows-gnu --all-targets` → 0 erreur, 0 avertissement (vérifié à la main, pas seulement par l'agent) |
| **Windows dans la CI** | matrice `release.yml`, bundle `nsis`, et `windows-` ajouté à `PLATEFORMES_ATTENDUES` du job `publier` |
| ~30 corrections révélées par la compilation croisée | `SIGTERM` qui mentait, `/proc/<pid>/exe` sans garde, six `HOME` muets, hook de panic écrivant dans le vide, filtre de disques, fenêtres console Windows |

Vérifications au 11h35 : `npm run check` 0/0 · `cargo test` 232 verts · `test:front` 12 ·
`i18n:audit` 0 · compilation Windows 0 erreur.

## Ce qui bloque, et c'est tout

**L'audio des réunions.** L'agent `audio-1` travaille dessus. Consignes dans
[audio.md](audio.md), et trois points non négociables :

1. **Sur Linux : host PulseAudio (Rust pur), PAS la feature `pipewire`.** Celle-ci tirerait
   `libpipewire` dans l'AppImage, ce qui rouvre la famille de pannes de la `libwayland` —
   l'app qui ne s'ouvre pas chez un testeur sur une autre distribution.
2. **La frontière du fichier `.raw` ne bouge pas** : `mic.raw` / `system.raw`, s16le mono
   16 kHz. Tout l'aval (morceaux de 10 min, fusion Moi/Eux, transcription, résumé) reste
   intact. C'est ce qui garde la décision réversible.
3. **AUCUN SON AUDIBLE, à aucun volume.** Le banc jouait une sinusoïde 440 Hz dix secondes
   dans les enceintes de Jimmy, deux fois, pendant qu'il travaillait. C'était une mauvaise
   consigne de ma part. Si un vrai signal est nécessaire, c'est **Jimmy** qui lance une
   musique quand il veut.

macOS : sans certificat Apple — décision prise, pas de certificat — la capture du son système
rendra du silence. Ce qu'on attend là-bas est seulement que ce cas soit **dit** au lieu de
finir en « aucune parole détectée », qui envoie chercher au mauvais endroit.

## Ce qui reste après ça

- **Le processus zombie** : l'app laisse un `[cockpit] <defunct>` par lancement. Bénin. Le code
  appelle bien `wait()`, donc la cause n'est pas trouvée — reproduire avant de patcher.
- Windows compile mais **n'a jamais tourné**. Le premier installeur produit par la CI le dira.

## Décisions déjà prises, à ne pas rouvrir

- Multiplexeur maison sur les trois systèmes, pas deux mécanismes en parallèle.
- WSL écarté : le terminal verrait les fichiers sous `/mnt/c/` quand les autres onglets les
  voient sous `C:\`. Contraire au principe de l'app.
- Pas de certificat de signature (Apple 99 €/an, Windows ~10 $/mois). Distribution par la page
  des releases, **pas de store**.
- macOS universel : une seule version pour Intel et Apple Silicon, gratuit.
- Détail mémoire (Cache, Buffers, ZFS) : **Linux seulement**. Les notions ne se traduisent pas.
- Le service ne persiste rien sur disque et meurt avec la machine.

## Le jour de la publication

Le prochain tag emporte tmux + Windows + audio d'un coup : la plus grosse version du projet.
Deux choses à faire dans les notes de version, en clair :

- ce qui tournait dans les terminaux est **perdu une fois** à cette mise à jour ;
- les anciennes sessions tmux continuent de tourner en fond et se retrouvent par
  `tmux -L cockpit attach`.

Rappel du `CLAUDE.md` : cette release-là est l'exception où Jimmy essaie en local avant
publication. Il l'a fait ce matin.

## À réarmer après une reprise

La surveillance des issues (elle ne survit pas à la session) :

```
Monitor: while true; do node scripts/issues-nouveautes.mjs --brut --marquer \
  --repere=.claude/issues-vues-surveillance.json 2>&1 | grep -E "^(ISSUE #|⚠)" || true; sleep 60; done
```

Sans elle, une réponse d'auteur passe inaperçue — c'est arrivé trois fois le 2026-08-20, et
c'est Jimmy qui a dû le signaler chaque fois.

## Compilation Windows : le piège à connaître

Sans compilateur C croisé, `cargo check --target x86_64-pc-windows-gnu` échoue sur `ring` et
`libsqlite3-sys` **avant** d'analyser notre crate : deux erreurs qui ne parlent pas de notre
code, et on croit à tort que le portage est cassé. La recette complète, sans droits
administrateur, est dans les « Pièges connus » du `CLAUDE.md`. Un préfixe déjà extrait :
`<scratchpad>/mingw/prefixe/usr/bin`.
