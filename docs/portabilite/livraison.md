# Construction, signature, distribution, mise a jour

Etude de la chaine de livraison de Cockpit (Tauri v2) pour Linux + macOS + Windows.
Lecture seule, faite le 2026-08-20. Fichiers examines :
`.github/workflows/release.yml`, `scripts/release.mjs`, `scripts/install.sh`,
`scripts/build-tmux-static.sh`, `src-tauri/tauri.conf.json`,
`src/lib/stores/update.ts`, plus l'historique CI reel du depot.

---

## 1. Le tableau

| Systeme | Bundle | Signature requise | Prix reel du certificat | Entree updater | Canal de distribution |
|---|---|---|---|---|---|
| **Linux x86_64** | AppImage | minisign Tauri seulement | **0 €** — deja en place (`TAURI_SIGNING_PRIVATE_KEY`) | `linux-x86_64` + `linux-x86_64-appimage` | `scripts/install.sh` (curl \| sh) ; AppImage manuelle |
| **macOS universel** | app + dmg | Developer ID Application + notarisation Apple | **99 €/an** (Apple Developer Program, compte individuel suffisant) ; notarisation incluse dans l'adhesion | `darwin-aarch64` + `darwin-x86_64` (meme bundle universel) | dmg de la page releases ; tap Homebrew perso (pertinent seulement apres signature) |
| **Windows x86_64** | nsis (`-setup.exe` + `.nsis.zip`) | certificat de signature de code | **~10 $/mois** annonce pour Azure Trusted Signing Basic (5 000 signatures/mois) — la page de tarifs Azure ne l'affiche plus et renvoie vers un devis, a reconfirmer. Alternatives **inutilisables sur runner GitHub** : OV a token USB 200-400 €/an, EV 400-700 €/an | `windows-x86_64` | `-setup.exe` de la page releases ; `install.ps1` ; winget seulement apres signature |

**Deux signatures a ne pas confondre.** La signature minisign de Tauri protege le canal de
mise a jour ; elle est deja en place, gratuite, et fonctionne identiquement sur les trois
systemes. Elle ne dit rien ni a Gatekeeper ni a SmartScreen. Payer Apple ne dispense pas de
payer Microsoft, et inversement.

**Pourquoi le .pfx dans un secret GitHub n'existe plus.** Depuis juin 2023, les regles du
CA/Browser Forum imposent que la cle privee d'un certificat de signature de code reside sur du
materiel certifie : token USB ou HSM en ligne. Les variables `WINDOWS_CERTIFICATE` /
`WINDOWS_CERTIFICATE_PASSWORD` documentees par Tauri ne servent donc plus qu'aux vieux
certificats et aux certificats auto-signes, qui ne resolvent rien. Un token USB demanderait un
runner auto-heberge avec le token branche en permanence. Azure Trusted Signing (cles dans le
HSM de Microsoft, appelable depuis le workflow via `signCommand` dans `tauri.conf.json` ou
l'action `azure/trusted-signing-action`) est la seule voie compatible avec une CI hebergee.

### Delais externes — ce sont eux qui commandent le calendrier

- **Apple** : validation de l'adhesion (quelques jours). Ensuite, le certificat *Developer ID
  Application* ne peut etre cree **que par le titulaire du compte**. Puis chaque build attend
  son tour dans la file de notarisation d'Apple, a chaque release.
- **Azure Trusted Signing** : l'eligibilite individuelle est conditionnee a environ **trois ans
  d'anteriorite verifiable de l'identite**, et la validation prend de **quelques jours a
  quelques semaines**. A verifier AVANT de payer quoi que ce soit — c'est le point qui peut
  rendre l'option indisponible.

### Cout des runners : zero

Le depot est public, donc les minutes GitHub Actions sont gratuites et illimitees sur les
runners standards, macOS et Windows compris. Ajouter Windows a la matrice ne coute rien en
argent.

---

## 2. Sans signature, ce que voit VRAIMENT l'utilisateur

Dans les deux cas c'est un avertissement contournable, **pas** un blocage absolu. Mais le
contournement n'est decouvrable par personne qui ne sait pas deja qu'il existe — et sur macOS
il existe un cas ou il n'y a plus aucun contournement dans l'interface.

### macOS — telechargement du dmg depuis un navigateur

Le dmg s'ouvre normalement, le glisser-deposer vers Applications fonctionne. C'est le premier
lancement qui casse : « macOS n'a pas pu verifier que cette app ne contient pas de logiciel
malveillant », avec deux boutons — *Placer dans la corbeille* et *Annuler*. **Aucun bouton
« Ouvrir quand meme » dans cette fenetre.**

Le « clic droit → Ouvrir » que le README recommande aujourd'hui **ne fonctionne plus** : Apple
l'a retire dans macOS 15 Sequoia. La documentation Apple actuelle ne decrit plus qu'un seul
chemin : Reglages Systeme → Confidentialite et securite → faire defiler jusqu'au message →
*Ouvrir quand meme* → saisir son mot de passe. L'utilisateur doit donc refuser la seule action
qu'on lui propose, puis aller chercher un bouton dans un panneau de reglages dont rien ne lui
a parle.

**Le mauvais cas.** Selon l'etat exact de la signature ad-hoc que le linker pose sur les
binaires arm64, macOS annonce a la place « Cockpit est endommagee et ne peut pas etre ouverte.
Placez-la dans la corbeille. » Ce message-la **n'offre aucun contournement dans l'interface** —
le bouton *Ouvrir quand meme* n'apparait pas dans les reglages. Seul recours :
`xattr -d com.apple.quarantine` dans un terminal. La documentation Tauri mentionne
explicitement ce symptome pour les apps non signees telechargees depuis un navigateur. **Pour
un utilisateur non technique, ce cas-la est un blocage pur.**

**Point rassurant** : les mises a jour passent deja sans signature Apple. L'updater telecharge
le `.app.tar.gz` lui-meme, et l'attribut de quarantaine n'est pose que sur ce qui vient d'un
navigateur. Toute la douleur est concentree sur la premiere installation.

### Windows — telechargement du `-setup.exe`

Trois obstacles successifs, chacun demandant a l'utilisateur de confirmer explicitement qu'il
veut executer quelque chose de dangereux :

1. **Le navigateur peut bloquer le telechargement lui-meme** — Edge et Chrome signalent un
   fichier « rarement telecharge » ou « potentiellement dangereux ». Il faut ouvrir le menu des
   telechargements et choisir *Conserver*.
2. **SmartScreen a l'execution** : « Windows a protege votre PC », avec un seul bouton visible,
   *Ne pas executer*. Le lien *Informations complementaires*, en petit, revele un bouton
   *Executer quand meme*.
3. **Defender peut supprimer le fichier sans demander**, selon la configuration de la machine.

Et comme la reputation SmartScreen se rattache au binaire, **elle repart de zero a chaque
nouvelle version**. Au rythme de plusieurs releases par jour de ce projet, elle ne se
construira jamais. Un certificat Azure Trusted Signing est de classe OV, donc la reputation se
construit avec le nombre de telechargements plutot que d'etre acquise d'emblee — mais les
binaires signes en heritent, ce qui n'est pas le cas d'un binaire nu.

### Verdict

- **Windows sans certificat : non distribuable a quelqu'un d'autre que le mainteneur.** Trois
  avertissements en cascade a chaque version, dont un que le navigateur impose avant meme le
  telechargement.
- **macOS sans signature : distribuable a un utilisateur technique averti, et pas au-dela.**
  Dans le meilleur cas un detour dans les reglages systeme que rien n'annonce ; dans le mauvais
  cas une commande dans un terminal.
- **A corriger dans tous les cas** : le README recommande le « clic droit → Ouvrir », retire par
  Apple. L'instruction actuelle envoie l'utilisateur dans un mur.

---

## 3. Ce que le job `publier` doit devenir

### Le cas vecu aujourd'hui

Sur la v0.32.0 (run 32373276422), le job macOS a echoue sur un test Rust qui ne casse que sur
macOS : `workspace::tests::test_stat_refuse_de_sortir_de_la_racine`
(`src-tauri/src/workspace/mod.rs:663`). Le job Linux a reussi. `publier` a leve le brouillon,
et la release est devenue `latest` avec un `latest.json` ne contenant que les deux entrees
Linux :

    version 0.32.0, plateformes: ['linux-x86_64', 'linux-x86_64-appimage']

Tous les utilisateurs macOS ont ete coupes des mises a jour — **et le run `publier` est sorti
VERT.** Personne n'a ete averti.

Cote application, le message d'erreur est bien attrape : la regexp `/platforms` object/` de
`src/lib/stores/update.ts` matche les DEUX variantes d'erreur du plugin (verifie dans
`tauri-plugin-updater-2.10.1/src/error.rs` : « the platform `X` was not found in the response
`platforms` object » et « None of the fallback platforms ... were found »). Les utilisateurs mac
ont donc vu « la mise a jour n'est pas prete » : le bon message pour une fenetre de quelques
minutes entre deux jobs, un mensonge poli quand le fichier reste incomplet indefiniment.

L'incident est ferme — la v0.32.1 a ete publiee pendant cette etude avec les quatre entrees
(`darwin-aarch64`, `darwin-aarch64-app`, `linux-x86_64`, `linux-x86_64-appimage`). Ce qui l'a
rendu possible ne l'est pas.

### Les trois changements, par ordre d'importance

1. **Garder une plateforme de reference qui BLOQUE la publication.** Linux, la ou sont les
   utilisateurs. Ne pas assouplir en « au moins une plateforme reussie » : un echec Linux
   publierait alors une release que personne ne peut installer.
2. **Verifier que `latest.json` porte une entree pour chaque plateforme dont le job de build a
   REUSSI.** C'est le vrai garde-fou. Il attrape aussi la classe de bug « l'artefact est
   uploade mais la fusion de `latest.json` n'a pas eu lieu » — l'incident « Error updating
   policy » de la v0.6.2 est exactement ca. `needs.release.result` ne suffit pas : il faut les
   conclusions par entree de matrice, le plus direct etant que chaque job depose un artefact
   temoin nomme d'apres sa plateforme, et que `publier` compare cette liste aux cles du
   fichier.
3. **Quand une plateforme a echoue : publier quand meme, puis faire ECHOUER le run avec un
   message qui nomme la plateforme absente.** C'est le point qui a manque sur la v0.32.0. Les
   systemes qui ont reussi recoivent leur version — donc un seul systeme en echec ne bloque
   jamais la release entiere — et la panne devient visible au lieu de dormir sous un run vert.

### La reparation ne demande pas une nouvelle version

`gh run rerun <id> --failed` refait le job manquant ; tauri-action retrouve la release
existante et fusionne sa plateforme dans `latest.json`. C'est deja le remede documente pour
l'incident « Error updating policy », il s'applique tel quel. Le job `publier` gere deja le cas
du rerun (variable `DEJA_PUBLIEE`).

Deux nuances a garder en tete :
- quand la release est deja publiee, le rerun **reecrit** `latest.json` : il existe une courte
  fenetre ou le fichier est remplace. Negligeable devant un fichier qui reste faux
  indefiniment.
- la regle existante du projet reste valable : **ne jamais rerun un vieux tag si une version
  plus recente est deja publiee**, sa release redeviendrait « latest ».

### Ce qu'il manque au workflow pour Windows

- une entree de matrice `os: windows-latest`, `bundles: nsis` ;
- rien a installer sur le runner : WebView2 est deja present, et les etapes `apt-get` et cache
  tmux sont deja conditionnees `if: runner.os == 'Linux'` ;
- `resources: ["resources/bin/*"]` ne matche rien hors Linux (le depot ne garde qu'un
  `.gitkeep`) : c'est deja le cas sur macOS et ca ne gene pas ;
- **NSIS et pas MSI.** MSI passe par WiX, s'installe par machine donc demande l'elevation, et
  double les fichiers a signer. NSIS s'installe par utilisateur, c'est le chemin que l'updater
  Tauri v2 prefere, et le seul ou la mise a jour en place se fait sans UAC ;
- un reglage a ajouter dans `tauri.conf.json` : `plugins.updater.windows.installMode` —
  `"passive"` (barre de progression) est le bon defaut ; `"quiet"` ne relance pas l'app
  proprement.

### Etat du reste du workflow

Le job macOS **marche** : sur les releases verifiees (v0.27.1 a v0.31.2) il a reussi a chaque
fois, et les releases portent bien les six fichiers attendus — dmg, AppImage, .sig,
`Cockpit_aarch64.app.tar.gz` + .sig, latest.json. La chaine macOS est fonctionnelle, elle n'est
simplement pas signee par Apple.

---

## 4. Les architectures de puces

- **Linux x86_64** : suffisant. ARM Linux n'a pas d'utilisateur ici, et demanderait en plus un
  tmux statique aarch64.
- **macOS** : aujourd'hui `darwin-aarch64` seulement, donc **rien pour les Mac Intel**. Le bon
  geste est **une seule cible universelle** — `args: -b app,dmg --target
  universal-apple-darwin`, avec les deux cibles Rust installees. tauri-action ecrit alors les
  DEUX entrees `darwin-x86_64` et `darwin-aarch64` vers le meme bundle, et il n'y a qu'un seul
  dmg a publier et a signer. Preferable a un deuxieme job sur runner Intel : les runners
  `macos-13` sont en fin de vie.
- **Windows x86_64** : le seul a couvrir. **Windows ARM : non** — Windows sur ARM execute les
  binaires x64 en emulation transparente, une cible `aarch64-pc-windows-msvc` n'ajouterait
  qu'un troisieme jeu d'artefacts a signer pour un gain marginal.
- **Windows i686** : non.

---

## 5. Le tmux statique embarque

- **Linux** : rien a changer. Binaire musl construit dans un conteneur Alpine (tmux 3.5a,
  checksum epingle dans `scripts/build-tmux-static.sh`), cache sur le hash du script, embarque
  comme ressource de l'AppImage.
- **macOS** : le vraiment-statique est **impossible** — Apple ne supporte pas la liaison
  statique de libSystem. Ce qui est faisable : compiler libevent et tmux sur le runner mac pour
  les deux arches, les fusionner, et embarquer les dylibs dans le `.app` avec des `@rpath`
  corriges, ncurses etant fourni par le systeme. Ca cree une surface a re-verifier a chaque
  montee de macOS. **Recommandation : garder le repli `brew install tmux`.** L'app detecte
  l'absence et le dit deja, le README le documente.
- **Windows** : tmux **n'existe pas**, et ce n'est pas un probleme d'empaquetage.
  `portable-pty` (deja une dependance) sait parler ConPTY, donc des terminaux fonctionnels sont
  possibles — mais **sans la persistance apres fermeture de l'app**, qui est la promesse du
  produit. L'autre voie est de faire tourner les terminaux dans WSL : tmux y marche, mais WSL
  devient une dependance dure. **C'est une decision produit a trancher avant meme de brancher
  un job Windows dans la CI** : elle change ce que « Cockpit pour Windows » veut dire. Hors du
  perimetre de la chaine de livraison, signale ici parce que ca conditionne tout le reste.

---

## 6. Distribution

- **Linux** : `scripts/install.sh` reste le bon canal, rien a changer. Il lit `releases/latest`
  via l'API GitHub, donc il n'y a rien a mettre a jour dedans quand une version sort. Il refuse
  explicitement tout ce qui n'est pas Linux/x86_64, il n'a donc pas besoin d'etre generalise.
- **macOS** : le dmg de la page des releases, deja documente. L'equivalent « une commande »
  serait un **tap Homebrew personnel** (`brew tap jguevel-tech/cockpit && brew install --cask
  cockpit`) : un tap perso echappe aux criteres de notoriete du homebrew-cask officiel. Deux
  precautions — marquer `auto_updates true` pour que `brew upgrade` ne se batte pas avec
  l'updater integre, et savoir que sans notarisation les utilisateurs devront installer avec
  `--no-quarantine`. Le tap devient donc vraiment interessant **apres** la signature Apple.
- **Windows** : le `-setup.exe` de la page des releases, plus eventuellement un `install.ps1`
  symetrique de `install.sh` (`irm ... | iex`). **Winget : pas avant la signature** — la
  soumission passe par une PR sur `winget-pkgs` avec validation automatique, et les installeurs
  non signes se font regulierement recaler par les scans Defender. De toute facon l'updater
  integre prend le relais des la deuxieme version : le canal d'installation initiale n'est pas
  le point critique.

---

## 7. Ce qui bloque de vrais utilisateurs, et ce qui est du detail

**Bloquant.**

1. **`publier` publie en silence une release incomplete.** Cause de fond de l'incident
   v0.32.0 : les utilisateurs macOS ont ete coupes des mises a jour et le run est reste vert.
   Avec trois systemes, la meme panne peut couper deux plateformes a la fois. A corriger avant
   d'ajouter Windows.
2. **La signature Windows.** Sans elle, une release Windows n'est pas distribuable a un
   inconnu.
3. **La signature Apple.** Moins grave — les mises a jour passent deja sans elle — mais la
   premiere installation est hors de portee d'un utilisateur non technique.
4. **Les terminaux persistants sur Windows.** Question d'architecture, pas de livraison, mais
   elle prime sur tout le chantier Windows.
5. **Le README macOS est perime** : « clic droit → Ouvrir » a ete retire par Apple dans macOS
   15. L'instruction actuelle envoie l'utilisateur dans un mur.

**Du detail.**

- Le tmux statique macOS : le repli brew suffit.
- Winget, Homebrew : confort, et de toute facon a faire apres la signature.
- Windows ARM, Linux ARM, Windows 32 bits : non.
- MSI en plus de NSIS : non.

---

## 8. Ce que le mainteneur doit decider ou payer

1. **99 €/an, Apple Developer Program** — pour que le dmg s'ouvre normalement sur macOS. Le
   plus rentable des deux. Attention au delai de validation de l'adhesion, et au fait que le
   certificat Developer ID ne peut etre cree que par le titulaire du compte.
2. **~10 $/mois, Azure Trusted Signing** — sans quoi Windows reste une plateforme pour lui
   seul. **A verifier avant tout engagement** : eligibilite individuelle (environ trois ans
   d'anteriorite d'identite), delai de validation de quelques jours a quelques semaines, et le
   tarif exact que la page Azure n'affiche plus.
3. **Windows, oui ou non, et sous quelle forme** — terminaux non persistants en ConPTY, ou
   dependance a WSL ? Cette reponse conditionne s'il faut engager le point 2.
4. **macOS universel ou Apple Silicon seulement** — aucun cout en argent, les runners sont
   gratuits sur un depot public. Universel ouvre les Mac Intel avec un seul artefact a signer.

---

## Fichiers concernes

- `/home/jguevel/Documents/workspace/core/cockpit/.github/workflows/release.yml`
- `/home/jguevel/Documents/workspace/core/cockpit/src-tauri/tauri.conf.json`
- `/home/jguevel/Documents/workspace/core/cockpit/src/lib/stores/update.ts`
- `/home/jguevel/Documents/workspace/core/cockpit/scripts/install.sh`
- `/home/jguevel/Documents/workspace/core/cockpit/scripts/build-tmux-static.sh`
- `/home/jguevel/Documents/workspace/core/cockpit/README.md` (section macOS a corriger)
- `/home/jguevel/Documents/workspace/core/cockpit/src-tauri/src/workspace/mod.rs:663` (le test
  qui a casse la v0.32.0, corrige depuis)
