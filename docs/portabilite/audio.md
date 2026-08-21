# Capture audio des reunions sur Linux, macOS, Windows

Etude en lecture seule (2026-08-20), suivie de sa mise en oeuvre.

---

## FAIT le 2026-08-21 — ce que le banc a tranche

La capture est passee a `cpal` dans le processus (`recorder/capture.rs`), avec la mise au
format dans `recorder/pcm.rs`. Ce que l'etude laissait ouvert, mesure sur la machine :

- **Le nom en `.monitor` tient.** `default_output_device()` rend l'identifiant
  `alsa_output.pci-0000_00_1f.3.analog-stereo`, la source
  `alsa_output.pci-0000_00_1f.3.analog-stereo.monitor` existe dans `devices()`, un flux
  d'entree se construit dessus et rend le son joue : ton de 440 Hz a 8 000 d'amplitude
  envoye sur la sortie, retrouve dans `system.raw` a **440,0 Hz et 7 999 d'amplitude**.
  La strategie « aucune bibliotheque C de plus » n'a donc pas eu besoin d'etre revue.
  **ATTENTION** : c'est `Device::id()` qui porte cette convention, pas `Display` — ce
  dernier rend la DESCRIPTION (« Monitor of Built-in Audio »).
- **Ce que l'etude a rate : sous Linux, cpal depend d'`alsa-sys` sans condition.** Le host
  ALSA n'est pas derriere une feature (`[target.'cfg(linux)'.dependencies] alsa`,
  `alsa-sys`). Il faut donc `libasound2-dev` au build et `libasound.so.2` a l'execution :
  une bibliotheque C de plus dans l'AppImage, ce que le choix du host PulseAudio visait
  justement a eviter. Le risque reste bien plus faible que celui de la libwayland — rien
  d'exterieur ne se lie a NOTRE libasound, et elle ne charge ses greffons qu'a l'ouverture
  d'un peripherique ALSA, ce que le chemin PulseAudio ne fait jamais. La feature
  `pipewire` ajouterait libpipewire EN PLUS de celle-la.
- **Le format natif n'est pas `f32`** : la machine du banc livre **48 000 Hz, 2 canaux,
  I32**. `en_flottants` couvre donc les douze formats de `SampleFormat`, et un format
  inconnu est refuse a l'OUVERTURE (un rappel audio ne peut rien remonter).
- **Un appareil de sortie refuse `default_input_config()`** (WASAPI : « Device does not
  support input »). La forme du flux de loopback se demande par `default_output_config()`.
  D'ou `config_capture()`, qui essaie les deux dans cet ordre.
- **Le reechantillonnage est fait a la main** (sinc fenetre Blackman, `pcm.rs`) plutot
  qu'avec `rubato` 5, dont l'API est passee aux adaptateurs `audioadapter` et qui aurait
  tire rustfft. ~80 lignes, aucune dependance, dix tests dont l'attenuation d'un ton de
  10 kHz (rien de mesurable en regime etabli) et l'egalite entre un traitement d'un bloc
  et le meme decoupe en lots de 137 trames.
- **macOS reste non verifie** : pas de machine, pas de signature. Le code y prend le meme
  chemin que Windows (`default_output_device()` + flux d'entree).

---

## Les deux reponses fermes, tout de suite

### Capter ce que la machine JOUE sur macOS, sans composant tiers : POSSIBLE

**C'est un fait, pas une estimation.** Depuis macOS 14.4, `AudioHardwareCreateProcessTap` est
une API **publique** de Core Audio qui capte la sortie du systeme. L'utilisateur n'installe
rien : pas de BlackHole, pas de Soundflower, pas de changement d'appareil de sortie. On passe
une liste de processus vide avec le drapeau `exclusive`, ce qui signifie « tout capter », et le
son continue de sortir des enceintes (`CATapMuteBehavior::Unmuted`).

L'ancienne reponse — « CoreAudio ne sait pas capter sa propre sortie, il faut un pilote tiers » —
etait vraie et **ne l'est plus**. La fonctionnalite n'est donc pas a abandonner sur macOS.

Mais il y a une condition dure, et elle n'est pas technique : **l'autorisation TCC est indexee
sur l'identite de signature de l'application.** Un binaire non signe ne declenche meme pas la
demande d'autorisation. Il obtient une piste de silence, sans aucune erreur.

Donc, net : **pas de compte developpeur Apple, pas d'enregistrement de reunions sur macOS.**
Aucune decision de code ne contourne ca. Le workflow de release le dit deja en commentaire
(« la signature Apple/notarisation demanderait un compte developpeur »).

### WASAPI loopback depuis Rust avec une bibliotheque mature : OUI

**Fait, verifie dans l'historique git de cpal**, pas seulement dans la doc :

| Commit | Date | Sujet |
|---|---|---|
| `dcabad10` | 2019-10-12 | Implement WASAPI loopback support |
| `78e8452` | 2020-09-10 | Reenable WasApi loopback (apres une perte en refonte) |
| `417b7ff` | 2022-07-15 | migration du backend WASAPI vers la crate officielle `windows` |

Le drapeau a survecu a la migration de 2022 et il est toujours la dans la version **publiee**
0.18.2 : `src/host/wasapi/device.rs:855` —
`if self.data_flow() == eRender { stream_flags |= AUDCLNT_STREAMFLAGS_LOOPBACK }`.

**Six ans dans l'arbre, en continu.** C'est du code eprouve, pas une nouveaute. Et cote build,
Windows ne coute rien de particulier : la crate `windows` est du Rust pur qui appelle les DLL du
systeme, aucun SDK a installer, rien de nouveau a embarquer.

**A l'inverse, le chemin macOS est jeune, et il serait malhonnete de presenter les deux comme
equivalents.** Son historique reel :

| Commit | Date | Sujet |
|---|---|---|
| `52724f6` | 2025-09-25 | Support loopback recording on macOS (#1003) |
| `405e840` | 2026-03-16 | fix : comportement indefini et **echec silencieux** a la creation de l'appareil de loopback |
| `2c7acf8` | 2026-05-11 | fix : collision d'UUID d'appareil agrege + demarrage automatique du tap |

Onze mois d'existence, dernier correctif de justesse il y a trois mois. Windows est le terrain
sur ; macOS est officiel mais recent.

**Aucun des trois systemes ne rend la fonctionnalite impossible.** Sur macOS elle est
conditionnee a une signature Apple, ce qui est une contrainte administrative et non un mur
technique. C'est la seule reserve a poser.

---

## Tableau

| Systeme | Micro | Sortie systeme (« loopback ») | Permission requise | Maturite |
|---|---|---|---|---|
| **Linux / PipeWire** | source par defaut | monitor du sink, propriete de noeud `stream.capture.sink=true` | aucune | en production dans Cockpit |
| **Linux / PulseAudio** | `@DEFAULT_SOURCE@` | source `<sink>.monitor` (les monitors sont des sources normales) | aucune | en production (repli 22.04) |
| **Windows** | endpoint de capture WASAPI | **WASAPI loopback natif** : `IAudioClient::Initialize` avec `AUDCLNT_STREAMFLAGS_LOOPBACK` sur un endpoint de **sortie**. Rien a installer, marche meme sans « Stereo Mix » | pas de permission dediee pour le loopback, mais Windows fait passer les flux de capture par le reglage de confidentialite micro — a traiter comme une erreur explicite. Le micro, lui, l'exige | API stable depuis Vista |
| **macOS 14.6+** | CoreAudio, appareil d'entree | **Core Audio process taps** : `AudioHardwareCreateProcessTap` + appareil agrege prive. API publique depuis 14.4. **Rien a installer** | `NSAudioCaptureUsageDescription` dans l'Info.plist + **categorie TCC propre**, distincte du micro. **Et une identite de signature stable** : TCC indexe l'autorisation dessus | API officielle, voie recommandee en 2026 ; jeune |
| **macOS 13 – 14.5** | idem | ScreenCaptureKit (`SCStreamConfiguration.capturesAudio`) | permission **Enregistrement de l'ecran** — indicateur dans la barre de menus, et macOS 15 redemande periodiquement | marche, mais on emprunte l'enregistrement d'ecran pour de l'audio |
| **macOS < 13** | idem | pilote tiers (BlackHole, Loopback) — l'utilisateur installe un composant systeme et change son appareil de sortie | — | a ecarter |

### Un choix macOS qui n'est pas reversible

Le plancher de version se decide avant d'ecrire une ligne, parce que les deux options ne
s'annulent pas :

- **plancher a 14.6** : un seul chemin audio sur macOS, les taps ;
- **plancher a 13** : il faut ScreenCaptureKit **en plus**, donc **deux** chemins audio macOS a
  maintenir pour toujours, avec deux permissions differentes et un indicateur dans la barre de
  menus sur l'un des deux.

Monter le plancher plus tard est indolore. Le descendre coute un second chemin permanent. C'est
le vrai point de non-retour du dossier macOS.

---

## Ce que fait le code aujourd'hui

`src-tauri/src/recorder/capture.rs` lance **deux programmes externes** et lit leur stdout dans un
fichier `.raw` :

- essai n°1 `pw-record` (PipeWire), essai n°2 `parecord` (PulseAudio) ;
- le repli se decide **au constat d'echec, piste par piste** (300 ms d'attente, `try_wait()`),
  pas sur une cause devinee ;
- le son des autres participants vient du **monitor du sink par defaut** :
  `-P stream.capture.sink=true` pour pw-record, `--device=@DEFAULT_MONITOR@` pour parecord ;
- le programme externe fait le travail de format : `--rate 16000 --channels 1 --format s16`,
  donc du PCM directement consommable.

Deux details qui pesent sur la suite.

D'abord, il existe deja toute une mecanique dont la seule raison d'etre est qu'on pilote un outil
etranger : lecture de `pw-record --help` pour savoir si `-P` existe (absent avant 0.3.5x), deux
fichiers `.err` distincts, assemblage d'un message d'erreur a partir de leur contenu. C'est du
code qui ne decrit pas notre probleme — il decrit les caprices de version d'un binaire tiers.

Ensuite, `CaptureHandles::stop()` fait `Command::new("kill")`. C'est deja non portable, et le
recorder ne porte **aucun** `#[cfg(target_os)]`. Il compile donc sur le job macOS de la CI
aujourd'hui, il echoue seulement a l'execution.

---

## Une seule bibliotheque pour les trois : oui

`cpal` **0.18.2** couvre les trois systemes. Verifie dans le code de la version publiee :

| Fichier de cpal v0.18.2 | Ce qu'on y trouve |
|---|---|
| `src/host/wasapi/device.rs:855` | le drapeau `AUDCLNT_STREAMFLAGS_LOOPBACK` sur les appareils de sortie |
| `src/host/coreaudio/macos/loopback.rs` | `CATapDescription` + `AudioHardwareCreateProcessTap` + appareil agrege prive, detruit au `Drop` |
| `src/host/pipewire/device.rs:168` | `if role == Sink && direction == Input { properties.insert(STREAM_CAPTURE_SINK, "true") }` — exactement le `-P` d'aujourd'hui |

Le point qui decide, c'est que **l'idiome est identique partout** : on construit un flux
d'**entree** sur un appareil de **sortie**, et cpal fait ce qu'il faut selon le systeme. Sur
macOS, `build_input_stream_raw` teste `if self.supports_input()` et bascule tout seul sur le tap
quand l'appareil est en sortie seule. Une seule expression dans notre code, trois mecanismes
systeme derriere.

MSRV de cpal : 1.85. Le rustc de la machine est en 1.90.

### Ce que cpal ne fait pas, et qui est gratuit aujourd'hui

Le format. Les rappels livrent le format natif du materiel — typiquement 48 000 Hz stereo en
`f32`. Le mixage stereo -> mono, le reechantillonnage vers 16 kHz et la conversion en `i16`
deviennent notre code. 48 000 -> 16 000 est un facteur 3 exact, mais 44 100 -> 16 000 ne l'est
pas : il faut un vrai reechantillonneur (`rubato`). Et comme un rappel audio ne doit jamais
bloquer, l'ecriture fichier passe par un tampon circulaire vers un thread d'ecriture.

C'est un vrai deplacement de responsabilite, et il faut le regarder en face : on recupere du
traitement du signal qu'un outil externe assurait. En echange, ce code est a nous, il est le meme
partout, et il est testable — un tampon de PCM connu en entree, un tampon attendu en sortie.
Alors que `args_capture()` et ses cinq tests ne testent pas notre logique : ils testent notre pari
sur la ligne de commande d'un binaire dont la version varie d'une machine a l'autre.

### Detail Linux qui change une decision

Les hosts PipeWire et PulseAudio de cpal sont derriere des `features`, ALSA est le host par defaut
et ALSA ne sait pas capter un monitor. Les deux features n'engagent pas la meme chose :

- `pipewire` tire des liaisons vers **libpipewire** -> `libpipewire-0.3-dev` + clang au build, et
  **une bibliotheque systeme de plus embarquee dans l'AppImage** ;
- `pulseaudio` tire la crate `pulseaudio`, qui est une implementation **du protocole en Rust pur**
  — aucune bibliotheque C, rien de nouveau dans l'AppImage.

Et le host PulseAudio suffit pour le son systeme : `devices()` appelle `list_sources()`, qui rend
aussi les monitors (le type `SourceInfo` de la crate porte `monitor_of_sink_index` et
`monitor_of_sink_name`, donc les monitors sont bien dans la liste). On prend le nom de
`default_output_device()`, on lui ajoute `.monitor`, on construit un flux d'entree dessus. Comme
tout le monde tourne avec `pipewire-pulse`, ce chemin couvre les machines PipeWire **et** les
machines PulseAudio — y compris l'Ubuntu 22.04 qui a justement force le repli `parecord`.

C'est le choix a faire, et pas pour une raison de confort : **embarquer une bibliotheque C de plus
dans l'AppImage est ce qui n'est pas reversible.** Le projet a deja paye cette lecon — la
`libwayland` du runner embarquee par linuxdeploy, le pilote graphique du systeme lie contre elle,
`EGL_BAD_PARAMETER`, WebKit qui abandonne, fenetre jamais ouverte chez un testeur sur Ubuntu
26.04, et un bug amont sans correctif ni option d'exclusion. Ajouter `libpipewire` rouvre
exactement cette famille de pannes, et une panne de cette famille ne se voit pas chez nous : elle
se voit chez quelqu'un d'autre, sur une distribution qu'on n'a pas. Le host en Rust pur n'ouvre
rien.

Nuance honnete : le host PulseAudio de cpal refuse `build_input_stream` sur un `Device::Sink`, il
ne connait le loopback que par la source monitor, et le nom en `.monitor` est une convention de
PulseAudio, pas une garantie d'API. A verifier au banc avant de s'engager — c'est la seule
inconnue de la strategie Linux.

---

## Architecture

**Capture dans le processus, avec cpal. Un seul chemin de code, trois systemes.**

L'argument central n'est pas la quantite de code, c'est sa nature. Aujourd'hui, la couche de
capture decrit trois choses melees : ce qu'on veut enregistrer, comment un binaire tiers accepte
de le formuler, et comment deviner ce qui a echoue en lisant son stderr. Les deux dernieres ne
sont pas notre sujet et pourtant ce sont elles qui ont produit les pannes : l'option `-P` refusee
par une version ancienne, le serveur PipeWire qui repond sans exposer d'appareil, un diagnostic
invente affiche a la place d'une cause reelle.

Garder le modele « un outil externe par systeme » multiplierait ce travail par trois, avec trois
stderr a interpreter — et ne resoudrait rien sur macOS, ou aucun outil en ligne de commande ne
contourne TCC parce que la permission est liee a l'application signee. C'est le pire des deux
mondes : trois codes audio a maintenir, et le systeme le plus contraint reste bloque.

### Ce qui disparait

| Aujourd'hui | Apres |
|---|---|
| `supports_properties_flag`, `help_mentions_properties` — lire l'aide d'un binaire pour deviner sa version | rien : plus d'options tierces a negocier |
| deux fichiers `.pw.err` / `.pa.err`, `read_err`, `borne`, `startup_error` | erreurs `cpal::Error` typees, avec un `ErrorKind` |
| `Command::new("kill")` sur le pid — deja non portable | `drop(stream)` |
| `pw-record` en dependance runtime (README, CLAUDE.md) | plus aucune dependance runtime pour l'audio |

### Ce qui ne bouge pas — et c'est le choix de conception qui compte

**La frontiere est le fichier `.raw`.** Le pipeline en aval lit `mic.raw` et `system.raw` en s16le
mono 16 kHz, et on conserve ce format a l'octet pres. Les chunks de 10 min, `max_amplitude`,
l'en-tete WAV de 44 octets, la fusion Moi/Eux : rien a toucher.

C'est aussi ce qui rend la decision **reversible**. Tant que la couche de capture ne fait que
produire ce fichier, on peut la remplacer sans que la transcription et le resume s'en apercoivent.
Si le loopback macOS de cpal decoit, on met une implementation ScreenCaptureKit derriere la meme
frontiere.

Ne pas ceder a la tentation de faire parler cpal directement au chunker « pour eviter un
aller-retour disque » : ce serait echanger la seule chose qui garde ce dossier ouvert.

### Ce qui reste valable tel quel, et qu'il faut garder

- **Le repli au constat d'echec, piste par piste.** Il se transpose directement : au lieu
  d'essayer `pw-record` puis `parecord`, on essaie les hosts cpal dans l'ordre. Le principe est le
  bon et il a ete appris sur une machine reelle — on ne diagnostique pas, on constate et on passe
  au suivant.
- **`lost_track_code()`**, mot pour mot : une piste vivante sur deux, ca enregistre quand meme en
  le signalant. Et le code renvoye reste un code, traduit par l'interface.

### La fiche de diagnostic

C'est le morceau qui change le plus. `pw_record_version()` et `audio_server_from_pactl()`
(`src-tauri/src/report/mod.rs`) deviennent sans objet — plus de `pw-record`, plus de `pactl`, et
leurs deux tests de parsing partent avec.

A la place, ce que cpal sait dire, qui est portable et plus utile pour comprendre une panne : host
retenu, nom des deux appareils choisis, frequence et nombre de canaux natifs, `ErrorKind` du
dernier echec. `distro` reste (a completer par l'equivalent mac/Windows), `packaging` reste.

---

## Le pipeline en aval est-il portable tel quel ?

Oui, entierement. Apres la capture, **il n'y a rien de specifique a Linux** :

- `transcribe.rs` — `std::fs::read`, `reqwest` multipart, `rustls` (donc pas d'OpenSSL systeme a
  trouver) ;
- `wav.rs` — de l'arithmetique sur des octets ;
- `summarize.rs` — un POST JSON ;
- `mod.rs` — chemins via `app_data_dir()` de Tauri, `chrono`, SQLite en `bundled`,
  `remove_dir_all`.

Le seul point non portable de tout le module est le `Command::new("kill")` de `capture.rs`, et il
disparait avec le passage a cpal.

Deux reserves qui ne sont pas de la portabilite mais qui se reveilleront sur une autre machine :

1. `transcribe_chunk` envoie `.text("language", "fr")` en dur. Le catalogue anglais existe deja
   cote interface : un utilisateur anglophone verra ses reunions transcrites comme si elles etaient
   en francais.
2. Le dossier de depot s'appelle `"Réunions"` en dur dans `pipeline_inner`. C'est un nom en base
   SQLite, donc sans risque — mais a ne pas transformer en nom de dossier disque, macOS normalisant
   l'Unicode differemment.

---

## A trancher au banc avant de s'engager

1. **Le nom en `.monitor` du host PulseAudio de cpal.** Toute la strategie Linux « aucune
   bibliotheque C de plus dans l'AppImage » repose la-dessus. La verification est directe : lister
   les appareils d'entree, chercher le suffixe, enregistrer quelques secondes. Si ca ne tient pas,
   il faut la feature `pipewire`, et il faut alors refaire le banc
   `docker run ubuntu:22.04/24.04/26.04 + xvfb-run AppRun` — parce qu'on rouvre la famille de
   pannes de la libwayland.

2. **Le loopback macOS de cpal, sur une machine signee.** Onze mois d'existence, deux correctifs
   de justesse dont un « echec silencieux ». Ne rien annoncer avant d'avoir entendu une piste
   `system.raw` non nulle.

### Un piege a prevoir des maintenant

Le symptome d'un tap macOS sans autorisation TCC est un fichier **plein de zeros, sans aucune
erreur** — c'est exactement ce qu'a rapporte l'auteur de l'issue cpal #1030. Le filtre
`max_amplitude(chunk) < SILENCE_AMPLITUDE` de `transcribe.rs` sauterait alors tous les chunks, et
le pipeline finirait sur « Aucune parole detectee dans l'enregistrement ». Un message qui envoie
chercher au mauvais endroit, sur une piste qui n'a jamais recu un seul octet utile.

Il faut distinguer « piste entierement muette » de « piste avec des passages muets », et nommer le
premier cas comme un probleme de permission. C'est le meme piege que le `Err(_) => continue` : une
erreur avalee qui fabrique un mensonge plausible.

---

## Sources

Code et historique de cpal :
- https://github.com/RustAudio/cpal/blob/master/CHANGELOG.md
- https://github.com/RustAudio/cpal/blob/v0.18.2/src/host/wasapi/device.rs
- https://github.com/RustAudio/cpal/blob/v0.18.2/src/host/coreaudio/macos/loopback.rs
- https://github.com/RustAudio/cpal/blob/v0.18.2/src/host/pipewire/device.rs
- https://github.com/RustAudio/cpal/issues/1030 (loopback macOS <= 14.6)
- https://github.com/RustAudio/cpal/pull/894 (ScreenCaptureKit, ouverte)
- https://github.com/RustAudio/cpal/issues/906 (loopback Linux)

macOS :
- https://developer.apple.com/documentation/coreaudio/audiohardwarecreateprocesstap(_:_:)
- https://developer.apple.com/documentation/bundleresources/information-property-list/nsaudiocaptureusagedescription
- https://github.com/insidegui/AudioCap (exemple de reference)
- https://dgrlabs.co/blog/2026-04-25-capturing-system-audio-on-macos-in-2026.html

Windows :
- https://learn.microsoft.com/en-us/windows/win32/coreaudio/loopback-recording

Linux :
- https://github.com/colinmarc/pulseaudio-rs/blob/main/src/protocol/command/source_info.rs
