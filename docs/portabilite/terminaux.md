# Terminaux et persistance sur Linux, macOS, Windows

Étude d'architecture — Cockpit (Tauri v2 + Rust + Svelte 5).
Lecture seule ; aucune modification apportée au dépôt.

Toutes les mesures de la section 4 ont été prises sur cette machine, sur le serveur tmux
vivant (`tmux -L cockpit`, 7 sessions `ckpt_*` réelles, tmux 3.4, 1 067 processus).
Ce sont des faits mesurés, pas des estimations.

---

## Résumé

1. **Un vrai émulateur de terminal côté serveur est obligatoire.** Rejouer les octets bruts
   ne marche que si on les rejoue depuis le premier octet de la session ; dès qu'on tronque —
   et Cockpit tronque déjà à 200 Ko — on perd l'écran, le curseur, les couleurs, l'écran
   alternatif et les modes souris. Un seul élément de la liste se prête aux octets bruts.
2. **Mais l'émulateur est justement la partie qu'on n'écrit pas.** `alacritty_terminal` la
   fournit, et `portable-pty` — déjà une dépendance de Cockpit — contient déjà ConPTY pour
   Windows. La couche PTY est **déjà portable** ; seul le multiplexeur ne l'est pas.
3. **tmux nous coûte dix familles de bugs documentées, dont sept disparaîtraient.** Elles ont
   toutes la même forme : elles existent parce que tmux est un programme séparé, avec sa
   propre notion de *client*, piloté en forkant une interface faite pour des humains.
4. **« Le terminal tmux est lent » est fondé, et tmux n'y est pour rien.** tmux ajoute 0,4 ms
   à une frappe et *supprime* la moitié des octets à dessiner sur une rafale. Le temps part
   dans un `ps -e` de 42 ms exécuté sur la boucle principale GTK toutes les 5 secondes, et
   dans une ligne de décodage base64 qui coûte 82 ms au lieu de 5.
5. **Recommandation : le multiplexeur maison est la bonne architecture, sur les trois
   plateformes.** Défendu en section 5 sur l'unicité du mécanisme, la classe de bugs
   supprimée, la petitesse de l'interface réellement utilisée, et le fait que la partie
   risquée est exactement celle qui se teste de façon exacte.

---

## 1. L'état à conserver pour qu'un terminal survive à la fermeture

### Poser le problème correctement

La question n'est pas « quelles données stocker ». Elle est : **le shell doit continuer à
tourner**, donc quelque chose en dehors de l'app tient le PTY ouvert. Et ce quelque chose est
le seul à lire la sortie pendant que personne ne regarde. Donc la vraie question est : que
fait-il de ces octets ?

Deux réponses possibles, et une seule survit à l'examen :

- **(a) Garder un journal des octets bruts et le rejouer à la reconnexion.**
- **(b) Les donner à un émulateur qui tient une grille de caractères, et regénérer des
  séquences d'échappement à la reconnexion.**

### Élément par élément

| Élément à conserver | Les octets bruts suffisent-ils ? |
|---|---|
| **Contenu de l'écran** | **Non.** Le rejeu ne marche que depuis le tout premier octet de la session. Une session de trois jours produit des gigaoctets. Dès qu'on tronque — et c'est exactement ce que fait `REPLAY_BUFFER_MAX = 200 * 1024` dans `terminal/mod.rs` — on démarre au milieu d'une séquence d'échappement, et tous les déplacements de curseur, couleurs, effacements et défilements d'avant sont définitivement perdus. |
| **Position du curseur** | **Non.** C'est le résultat cumulé de tous les octets précédents, pas une donnée présente dans le flux. Irrécupérable après troncature. Coût en mémoire : deux entiers. |
| **Forme du curseur** (DECSCUSR) | **Non**, même raison. Un octet. |
| **Attributs en cours** (gras, couleur avant/arrière, souligné, barré, inverse) | **Non.** L'état SGR au point de coupure. Irrécupérable après troncature, trivial à tenir. |
| **Écran alternatif** (claude, vim, htop, less) | **Non, et c'est cet élément qui tranche la question.** Il faut (1) savoir qu'on y est, (2) y entrer à la reconnexion (`\e[?1049h`), (3) peindre la grille alternative, **et (4) garder intacte la grille principale en dessous** — parce que quand vim sort il émet `\e[?1049l` et l'écran du shell doit réapparaître tel qu'il était. Il faut donc **deux grilles simultanées**. Aucun rejeu d'octets ne produit ça. |
| **Régions de défilement** (DECSTBM), mode origine (DECOM) | **Non.** Ce sont des réglages qui changent où le texte suivant se pose. À tenir et à réémettre à la reconnexion. |
| **Historique de défilement** | C'est **le seul point** où les octets bruts sont bons. Mais il faut des **lignes de grille**, pas des octets : la recherche et la sélection travaillent sur des positions de caractères, pas sur des positions dans un flux. Aujourd'hui c'est tmux qui le tient (`history-limit 10000`) et `terminal_search` délègue à son copy-mode. |
| **Modes souris** (1000, 1002, 1003, 1006 SGR) | **Non.** L'application a dit au terminal « envoie-moi les clics ». Si la nouvelle vue n'est pas remise dans le même mode, les clics dans claude cessent de fonctionner — ou pire, arrivent sous forme de texte dans le shell. Peu de bits, mais un oubli ici est un bug muet. |
| **Même famille** : collage encadré (2004), touches curseur applicatives (DECCKM), mode clavier numérique, report de focus (1004), sortie synchronisée (2026) | **Non.** Mêmes raisons, mêmes conséquences. C'est une liste qui s'allonge avec le temps : d'où l'intérêt qu'un tiers la tienne à jour (§2). |
| **Taille et sa renégociation** | La grille **est** la vérité ; le PTY doit s'y aligner (`TIOCSWINSZ` + SIGWINCH). Ce n'est pas un problème de stockage mais de conception : tmux prend par défaut la taille du plus petit client attaché, et c'est précisément la source du `resize-window` avant chaque attache dans `attach()`. Un multiplexeur avec **un seul propriétaire par session** n'a pas ce conflit du tout. |
| **Encodage** | **Non.** Le décodage UTF-8 est à état : un caractère multi-octets peut être coupé entre deux lectures du PTY. L'émulateur tient le morceau en attente une fois pour toutes. Le rejeu brut repousse ce problème sur le client, et la troncature coupe des caractères en deux — c'est exactement la famille de bugs des accents. |

### Le verdict

**Un vrai émulateur, tenant une grille de caractères, est obligatoire.** Sur treize lignes du
tableau, une seule se prête aux octets bruts, et même celle-là veut des lignes de grille pour
être utilisable. Tout le reste est de l'**état accumulé en analysant le flux** : dès qu'on
tronque le flux, on l'a perdu.

Formulé autrement : le rejeu d'octets ne conserve pas un état, il **reconstitue** un état — et
il ne sait le faire que s'il n'a rien oublié depuis le début. C'est intenable pour une session
qui doit vivre des jours.

### Ce que cela veut dire pour l'ampleur du chantier

C'est la nuance décisive, et elle va à l'inverse de l'intuition : **la partie difficile n'est
pas celle qu'on écrit.**

Ce qu'on n'écrit pas, parce que ça existe (§2) :
- l'analyseur de séquences d'échappement et la grille (`alacritty_terminal`) ;
- la gestion du PTY sur les trois plateformes, ConPTY inclus (`portable-pty`).

Ce qu'on écrit :
1. **le service de fond** : cycle de vie, socket, reconnexion, réconciliation ;
2. **le protocole** entre l'app et le service, avec une tolérance de version ;
3. **la sérialisation grille → séquences d'échappement** pour la reconnexion ;
4. **sélection, recherche, historique** par-dessus la grille (`alacritty_terminal` fournit
   `Selection` et `term::search::RegexSearch`, le modèle d'interaction est à construire).

Le point 3 est le seul vrai risque : c'est là que vivent les bugs de fidélité, et c'est ce que
tmux corrige depuis dix-huit ans. **Mais c'est aussi la seule partie qui se teste de façon
exacte** — j'y reviens en section 5, parce que c'est ce qui fait la différence entre un pari
et une décision.

---

## 2. Les briques Rust existantes et leur maturité

### PTY : c'est déjà réglé

**`portable-pty` 0.9** (wezterm) est **déjà une dépendance de Cockpit** (`src-tauri/Cargo.toml`).
Vérifié dans les sources du cache cargo : la crate contient `src/win/conpty.rs` et
`src/win/psuedocon.rs`, qui appellent **`CreatePseudoConsole`**. Windows 10 d'octobre 2018 ou
plus récent est exigé, et la crate préfère un `conpty.dll` / `openconsole.exe` posé à côté
s'il y en a un. **Aucun flag de compilation à activer** : c'est du `#[cfg(windows)]`.
12 millions de téléchargements, 5,6 millions récents.

**Conséquence majeure : la couche PTY de Cockpit est déjà portable sur les trois plateformes.
Ce qui ne l'est pas, c'est uniquement le multiplexeur.** C'est le fait le plus utile de cette
section, et il réduit beaucoup la surface du problème Windows.

### Émulateur en bibliothèque

| Brique | État constaté | Verdict |
|---|---|---|
| **`alacritty_terminal` 0.26** | Publiée en avril 2026. 1 181 703 téléchargements dont 599 040 récents. Expose `Grid`, `Term`, `Cell`, `Colors`, le module `selection` (`Selection`, `SelectionRange`, `SelectionType`), `term::search::RegexSearch` et `RegexIter`, les régions de défilement (`grid::Scroll`), l'écran alternatif, et même son propre module `tty`. C'est le moteur d'Alacritty, terminal grand public qui tourne sur Windows. | **Mûre, et le bon choix.** Deux réserves à connaître : aucune promesse de stabilité d'API (0.25 → 0.26 a demandé des adaptations), et **aucune fonction de sauvegarde/restauration d'état** — la sérialisation grille → ANSI reste à écrire. La première réserve produit des erreurs de compilation, pas des pannes silencieuses. |
| **`vt100` 0.16.2** (doy) | 9,96 millions de téléchargements, mais dernière version en juillet 2025. Fournit `contents_formatted()`, qui rend directement les séquences pour repeindre l'écran — c'est-à-dire la sérialisation qu'on écrirait sinon. | **Solide, mais trop étroite.** Modélise un seul écran avec un historique limité ; elle est pensée pour « analyser la sortie d'un programme », pas pour piloter un terminal complet. L'écran alternatif est là où elle lâcherait, et c'est justement le point décisif du §1. |
| **`libghostty-vt` 0.2.1** | Le moteur VT de Ghostty exposé en Rust via FFI. Publiée en juillet 2026, 69 536 téléchargements, quatre versions depuis mars 2026. | **Excellent moteur, mauvais paquet.** L'émulation de Ghostty est parmi les plus correctes qui existent, mais c'est du FFI vers une bibliothèque Zig : chaîne de compilation supplémentaire et compilation croisée à régler sur trois runners de CI. Mauvais choix pour une app qu'on publie en AppImage, dmg et exe. |
| **`wezterm-term` / `termwiz`** | **N'existent pas sur crates.io** (vérifié : la recherche « wezterm » ne les renvoie pas, seuls des paquets satellites comme `wezterm-bidi`, `wezterm-color-types`). Seul un fork tiers est publié : `tattoy-wezterm-term 0.1.0-fork.5`, 10 246 téléchargements. | **À écarter** : ce serait une dépendance git ou un fork tiers, pour une app publiée. |
| `par-term-emu-core-rust` (3 608), `justerm-core` (244), `bed-vt100` (93), `shadow-terminal` (4 114) | Quelques centaines à quelques milliers de téléchargements, tous en 0.x. | **Jouets.** À ne pas utiliser. |

### Service de fond multiplateforme

**Il n'existe pas de brique pour ça, et il n'en faut pas.**

- **Linux / macOS** : double `fork` + `setsid`, ou simplement lancer le processus détaché et
  ignorer SIGHUP. C'est ce que fait déjà le serveur tmux.
- **Windows** : il n'y a pas de `fork`. On appelle `CreateProcess` avec
  `DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP` et l'enfant survit au parent. C'est le
  mécanisme normal, pas un contournement.
- **Un vrai *Service* Windows serait une erreur** : un Service est à l'échelle de la machine,
  alors que les terminaux appartiennent à une session utilisateur (variables d'environnement,
  presse-papier, `HOME`). Un processus détaché par utilisateur est la bonne granularité.
- **Socket** : socket de domaine Unix d'un côté, **tuyau nommé** (`\\.\pipe\...`) de l'autre.
  La crate **`interprocess`** couvre les deux derrière une même interface et est mûre.

### Preuve d'existence

**Zellij** fait exactement cette architecture, en Rust : un client, et un serveur démonisé au
premier lancement qui tient l'état de la session (programmes ouverts, disposition, grilles),
avec IPC entre les deux. C'est la démonstration que l'approche tient debout.

Son plafond est instructif : après un **redémarrage machine**, Zellij restaure la disposition
et l'historique mais **pas les processus**. Ça correspond exactement au besoin de Cockpit —
survivre à la fermeture de l'app, pas au reboot — mais il faut le savoir et ne pas promettre
plus.

---

## 3. Ce que tmux nous coûte aujourd'hui

Chaque entrée est tirée du code ou des commentaires de `src-tauri/src/terminal/mod.rs` et du
CLAUDE.md, pas d'une supposition.

### Disparaîtrait avec un multiplexeur maison

**1. Les événements de focus synthétisés à chaque attache/détache.**
tmux envoie des événements de focus (in/out) à l'application du pane chaque fois qu'un client
s'attache ou se détache — même avec `focus-events off`, qui ne gouverne que le focus du
terminal extérieur. Un cycle attache/tue/rattache **sans aucune frappe** fait redessiner
claude, et ce redessin laissait un saut de ligne à chaque changement d'onglet.
Coût constaté : **trois vagues de correctifs** (0.6.5, 0.6.7, puis le revirement complet du
2026-08-13 vers le pool persistant), dont deux qui visaient d'autres maillons.
La cause racine est la **notion de client** : une attache est un événement observable par
l'application. Un multiplexeur maison n'a pas de clients, seulement des lecteurs d'une grille
— il n'y a rien à synthétiser. **Disparaît entièrement.**

**2. `set -g window-size manual` fait planter tmux 3.4.**
Mise dans un fichier de conf, cette ligne seule tue le serveur au démarrage (« server exited
unexpectedly ») : plus aucun terminal créable. Prouvé par bissection. Contournement en place :
un `resize-window` par fenêtre avant chaque attache, plus sept lignes de commentaire dans
`TMUX_CONF` suppliant qu'on ne la remette pas, et le même avertissement répété dans
`apply_server_options`. **Disparaît** : on possède le redimensionnement.

**3. L'écran alternatif est toujours actif côté client.**
Le client tmux met **toujours** le terminal hôte en écran alternatif, donc
`term.buffer.active.type` d'xterm est inutilisable pour savoir si une TUI tourne. Il faut
demander à tmux : `display-message -p '#{alternate_on}'` — un fork+exec par question
(`inner_alternate`). **Disparaît** : l'émulateur le sait en mémoire, c'est la lecture d'un
champ.

**4. Le collage du clic molette de tmux doit être désarmé.**
tmux colle de son côté sur `MouseDown2Pane` alors que Cockpit colle déjà : deux collages.
Quatre `unbind` sont dans la conf **et** réappliqués au serveur vivant à chaque démarrage. Le
commentaire sur place est explicite : un échec silencieux de ce `unbind` ramène le bug « sans
laisser la moindre trace », et le symptôme a été **rediagnostiqué de zéro deux fois**.
**Disparaît.**
(À ne pas confondre avec le double abonnement `onData` du pool xterm, qui est notre bug à nous
et resterait — d'où la règle `brancherEntree()`.)

**5. Analyser la prose de `stderr` de tmux pour savoir si une session existe.**
`absence_definitive()` fait du `contains()` sur des phrases anglaises. tmux 3.4 a **deux
formulations pour le même fait** : « no server running on <chemin> » quand le fichier de socket
traîne encore, « error connecting to <chemin> (No such file or directory) » quand il a disparu.
Seule la première était reconnue — et c'est la seconde qui se produit après un redémarrage
machine, puisque `/tmp` est vide. Résultat : des terminaux affichés que l'utilisateur ne
pouvait plus fermer.
C'est le symptôme d'un problème de fond : **la CLI tmux est une interface faite pour des
humains**, et on s'en sert comme interface de programme. **Disparaît**, remplacé par une
réponse typée sur notre protocole.

**6. La chaîne de copie en cinq sauts.**
`copy-pipe-and-cancel` → `tmux load-buffer -w -` (et surtout pas `set-buffer`, qui ne lit pas
l'entrée standard — piège déjà payé) → OSC 52 → analyseur xterm → commande `set_clipboard` →
instance arboard gardée vivante sinon X11 vide le presse-papier. Cinq maillons pour copier une
sélection, chacun capable de casser en silence. **Disparaît en grande partie** : le service
possède la sélection, c'est un appel.

**7. Les 41 fork+exec au démarrage.**
`apply_server_options` envoie **41 commandes tmux séparées** (12 options + 29
`set-environment -g -r`) au serveur vivant. Cette fonction existe **uniquement** parce que le
serveur tmux lit sa configuration une seule fois, à sa propre naissance, et qu'il survit à
l'app. Mesuré : **157,3 ms de forks bloquants à chaque lancement** (§4). **Disparaît** : notre
service reçoit sa configuration par le protocole, ou il n'en a pas besoin.

### Changerait de forme, sans disparaître

**8. L'incompatibilité de version du protocole.**
Un client tmux d'une version différente du serveur échoue en « protocol version mismatch ».
D'où `refresh_deployed_tmux`, qui ne remplace le binaire déployé **que si aucun serveur ne
tourne** — donc une mise à jour de Cockpit peut laisser un utilisateur sur un ancien tmux
indéfiniment, et personne ne le sait.
Avec notre propre service, **le problème existe à l'identique** (app 0.40 parlant à un service
lancé par la 0.31). La différence est qu'on le **contrôle** : un champ de version dans la
poignée de main, une compatibilité arrière assumée, et un chemin « le service est ancien,
relance tes terminaux quand ça t'arrange » — au lieu d'un échec dur sur une chaîne de
caractères. C'est un coût qu'on hérite, pas qu'on supprime, et il faut le dire.

### Resterait à l'identique

**9. La fuite d'environnement de l'AppImage.**
`APPIMAGE_LEAKED_VARS` fait 29 entrées, plus trois listes de chemins nettoyées entrée par
entrée (`XDG_DATA_DIRS`, `XDG_CONFIG_DIRS`, `PATH`). La cause est structurelle : **ce qui
survit à l'app est lancé par l'app et hérite de son environnement**. Un service maison a
exactement le même problème. La liste déménage de `tmux_cmd` vers le lancement du service, un
point c'est tout.

**10. Réconcilier ce que le service a et ce que la base dit.**
Sessions orphelines (« 3 terminaux affichés, 14 sessions vivantes », constaté le 2026-08-13),
`purge_dead`, le préfixe `ckpt_` pour ne jamais toucher une session étrangère, le garde-fou
`COCKPIT_DB` pour qu'un build de dev ne tue pas les vrais terminaux de l'utilisateur — tout ça
reste nécessaire dès qu'un état vit hors de l'app.
Mais **ça s'allège** : aujourd'hui la vérité est coupée en trois (SQLite pour les métadonnées,
le serveur tmux pour les sessions, le pool xterm du frontend). Un service maison peut tenir ses
métadonnées lui-même et ramener ça à deux.

### Ce qu'on perdrait en quittant tmux

**11. Historique, copy-mode, recherche, sélection.**
`terminal_search` fait quatre lignes qui délèguent à `search-backward-text`, `search-again`,
`search-reverse` et `cancel` de tmux. La navigation dans l'historique à la molette, les 10 000
lignes, la sélection à la souris qui reste affichée au relâchement : tout est à tmux.
`alacritty_terminal` fournit les pièces (`RegexSearch`, `Selection`), le comportement est à
construire et à régler.

**12. Dix-huit ans de correctifs de fidélité.**
Chaque programme que le mainteneur lance dans un terminal Cockpit — claude, vim, htop, less, top,
git log en pager — devient un cas de test qu'on passe pour la première fois.

**Bilan : sept familles sur dix disparaissent, une change de forme, deux restent, et deux
choses sont à reconstruire.** Et les sept qui disparaissent ont toutes la même origine : tmux
est un programme séparé, avec sa propre notion de client, qu'on pilote en forkant une interface
conçue pour des humains et en lisant sa prose.

---

## 4. La mesure : « le terminal tmux est lent »

**L'affirmation est fondée. La cause n'est pas tmux.**

Méthode : mesures sur la machine réelle, sur le serveur `tmux -L cockpit` vivant pour les
latences de commandes, et sur un socket de test séparé pour les attaches (afin de ne pas
détacher les sessions claude en cours). Comparaison PTY nu / tmux avec le même harnais Python
(`pty.fork`, `select`), qui fait ce que fait `portable-pty`. Charge système notée à chaque
passe : les premières mesures, prises à une charge de 15, étaient gonflées d'un facteur dix —
elles ont été refaites.

### Ce que tmux coûte réellement

| Mesure | PTY nu | À travers tmux |
|---|---|---|
| Écho d'une touche — médiane | 0,16 ms | **0,59 ms** |
| Écho d'une touche — p95 | 0,35 ms | **1,02 ms** |
| Rafale de 4,2 Mo (`yes \| head -100000`) — durée | 250 ms | **417 ms** |
| Rafale de 4,2 Mo — octets livrés à l'app | 4 200 161 | **1 962 855** |
| Une commande tmux (fork+exec + aller-retour socket) | fork nu : 0,98 ms | **3,7 ms** |
| Attache d'un client neuf | — | **1er octet à 4-5 ms**, repeinte ~2,2 Ko |
| `capture-pane` de l'écran visible | — | 3,1 ms (4,1 Ko) |

Lecture :

- **tmux ajoute 0,4 ms à une frappe.** C'est invisible. Ce n'est pas là que se joue une
  sensation de lenteur.
- **Sur une rafale, tmux jette 53 % des octets** en écrasant les lignes qui ont défilé. Il
  livre 1,96 Mo au lieu de 4,2 Mo. Pour le frontend, qui doit dessiner chaque octet, c'est un
  **gain**, pas un coût.
- **Une attache coûte 4-5 ms** et une repeinte de 2,2 Ko. Le chemin d'attache n'est pas lent
  non plus.

### Où le temps part vraiment

#### (a) `ps -e -o args=` sur la boucle principale GTK, toutes les 5 secondes — 42 ms

`stores/terminals.ts:26` appelle `listAllTerminals()` toutes les 5 secondes.
`list_all_terminals` (`lib.rs:755`) est un `#[tauri::command] fn` **sans `async`**. Vérifié
dans `tauri-macros` : sans `async`, le macro produit `ExecutionContext::Blocking`, variante
`"sync"` — la commande s'exécute **en ligne dans le gestionnaire IPC de wry**. Et vérifié dans
`wry-0.55.1/src/webkitgtk/mod.rs:638` : ce gestionnaire est un
`connect_script_message_received`, c'est-à-dire **un signal GTK, donc la boucle principale**.

Donc toute l'interface gèle pendant l'appel — y compris la livraison de la sortie des
terminaux, qui passe par la même boucle (voir (c)).

Décomposition du poll, mesurée à part (charge 17, `awk` sur 25 itérations) :

| Étape | Durée |
|---|---|
| `tmux list-sessions -F '#S'` | 3,69 ms |
| `tmux list-panes -a -F ...` | 3,81 ms |
| **`ps -e -o pid=,ppid=,args=`** | **42,23 ms** |
| `readlink /proc/<pid>/exe` sur l'arbre | 0,53 ms |
| **Total bloquant** | **~50 ms** |

**42 des 50 ms sont `ps`.** Et il part à presque **chaque** passe : dans `tmux_llm_sessions`,
le `ps` se déclenche dès qu'une commande de premier plan n'est pas un nom de LLM — un shell
posé au prompt suffit. Sur cette machine, quatre des sept sessions rapportent même `cockpit`
comme commande de premier plan (le mensonge d'argv[0] documenté dans le code), donc le `ps`
part toujours.

**Sous charge, c'est bien pire.** Le même `ps` mesuré pendant que des agents claude tournent :
médiane 394 ms, p90 693 ms, **maximum 1 074 ms**. Or « pendant que des agents claude
tournent » est précisément le cas d'usage de Cockpit. Une interface qui gèle une seconde toutes
les cinq secondes, c'est exactement la description d'un terminal lent.

**Rien de tout ça n'est tmux** : les deux commandes tmux du poll coûtent 7,5 ms sur 50.

#### (b) La ligne de décodage base64 du frontend — 82 ms au lieu de 5

`src/lib/components/project/TerminalTab.svelte:78` :

```js
return Uint8Array.from(atob(data), (c) => c.charCodeAt(0));
```

`Uint8Array.from` avec une fonction de transformation appelle cette fonction **une fois par
caractère**. Mesuré sur la rafale réelle (1,96 Mo en 240 morceaux de 8 Ko) :

| Variante | Durée |
|---|---|
| `Uint8Array.from(atob(d), c => c.charCodeAt(0))` — **actuel** | **82,0 ms** |
| `atob(d)` + une boucle `for` nue | **5,3 ms** |

**Facteur 16 pour une ligne**, sur le thread qui dessine. Mesuré sous V8 ; JavaScriptCore
(WebKitGTK) n'a aucune raison d'être meilleur sur ce motif.

#### (c) Aucun regroupement entre le PTY et le webview

Le thread lecteur de `spawn_attach` lit par morceaux de 8 192 octets et **émet un événement
Tauri par morceau** : 240 événements pour cette rafale.

Vérifié dans `tauri-2.11.0/src/event/mod.rs:194` (`emit_js_script`) et
`src/webview/mod.rs:1971` (`emit_js` → `self.eval`) : Tauri v2 livre un événement en
**construisant une source JavaScript** avec la charge insérée dedans en clair, puis en
l'évaluant dans le webview. Donc :

- 8 192 octets de terminal → 10 924 caractères de base64 (**+33 %**) → un script de **10,8 Ko** ;
- pour la rafale : **2,65 Mo de source JavaScript pour 1,96 Mo de terminal**, et
  **240 appels `evaluate_javascript` inter-processus** vers le WebProcess de WebKit.

Analyse et exécution des 240 scripts distincts (donc sans cache de compilation) : **10,8 ms**.

**Le trou honnête de cette mesure** : les 240 sauts inter-processus vers le WebProcess, je
n'ai pas pu les mesurer depuis l'extérieur de l'app. C'est le seul poste que je chiffre par
raisonnement et non par mesure. Ce qui est mesuré, c'est le volume (2,65 Mo de source pour
1,96 Mo utile) et le nombre d'appels (240).

#### (d) 157 ms de forks bloquants à chaque démarrage

`apply_server_options` envoie 41 commandes tmux séparées au serveur vivant. Les mêmes 41
enchaînées par `;` dans un **seul** appel tmux :

| Forme | Durée |
|---|---|
| 41 appels tmux séparés — **actuel** | **157,3 ms** |
| Les mêmes 41 en un appel enchaîné | **9,9 ms** |

Même constat en petit : 3 commandes en 3 appels = 11,4 ms ; les mêmes 3 en un appel = 3,2 ms.
Le coût est presque entièrement le fork+exec, pas le travail.

### Conclusion de la mesure

**Classement des postes réels, par ordre d'effet sur la sensation de lenteur :**

1. Le poll de 5 secondes qui gèle la boucle principale — 50 ms au repos, jusqu'à 1,1 s sous
   charge. **Ce n'est pas tmux**, c'est `ps` sur une commande non-`async`.
2. Le décodage base64 du frontend — 82 ms par rafale de 2 Mo au lieu de 5.
3. L'absence de regroupement des morceaux de sortie — 240 allers-retours et +33 % de volume.
4. Les 157 ms de forks au démarrage.
5. tmux lui-même : **0,4 ms par frappe, et la moitié des octets à dessiner en moins.**

**Réponse à la question posée** — tmux, notre chemin d'attache, ou le rendu xterm ? Ni tmux
(0,4 ms) ni le chemin d'attache (4-5 ms). C'est **notre code autour** : une commande
synchrone qui bloque la boucle GTK, et une ligne de JavaScript qui décode caractère par
caractère.

Point important pour la section suivante : **ces quatre problèmes ne sont pas résolus par un
changement de multiplexeur.** Les postes 1, 2 et 3 existeraient à l'identique avec un
multiplexeur maison — le poste 1 serait même pire, puisqu'un service maison devrait aussi
répondre à « quelles sessions sont vivantes ». Ce sont des problèmes **indépendants**, et les
confondre avec la question d'architecture serait une erreur de diagnostic.

---

## 5. Recommandation

**Le multiplexeur maison est la bonne architecture, sur les trois plateformes.**

Je la défends sur quatre axes, dans l'ordre de leur poids.

### 5.1 Un seul mécanisme, ou deux pour toujours

C'est l'argument décisif, et il est structurel.

L'option « tmux sur Linux et macOS, PTY direct sans persistance sur Windows » ne coûte pas un
effort : elle coûte **deux mécanismes de terminal à maintenir en parallèle, indéfiniment**.
Concrètement, ça veut dire :

- deux comportements différents pour la même action, qu'il faut connaître et documenter ;
- toute fonctionnalité nouvelle sur le terminal à écrire deux fois, ou à écrire une fois puis à
  déclarer indisponible sur une plateforme ;
- tout bug à reproduire deux fois pour savoir s'il est commun ou spécifique ;
- une doc intégrée qui doit dire « sauf sous Windows » ;
- et la tentation permanente de ne tester que sur celui des deux qu'on a sous la main.

Un projet piloté par l'IA sans build local chez l'utilisateur est **particulièrement** mal armé
pour tenir deux chemins de code divergents : le seul retour d'usage vient de la version
publiée, et un chemin peu exercé pourrit sans que personne s'en aperçoive. C'est déjà la leçon
du premier utilisateur externe (projet invisible, Docker « stopped » à tort, bouton inerte :
trois bugs qui vivaient dans du code peu exercé).

Sur dix ans, la question n'est pas « lequel est plus dur à écrire » mais « lequel est encore
juste dans dix ans ». **Un mécanisme unique sur trois plateformes est juste. Deux mécanismes
divergents ne le sont pas, et l'écart ne fait que grandir.**

### 5.2 Une classe entière de bugs supprimée, pas des bugs individuels

La section 3 n'énumère pas dix bugs indépendants : elle décrit **une famille**, avec une cause
commune. tmux est un programme séparé, avec sa propre notion de *client*, qu'on pilote en
forkant une interface conçue pour des humains.

De cette cause unique découlent :

- les événements de focus (il y a des clients, donc s'attacher est un événement observable) ;
- l'écran alternatif toujours actif (le client est un terminal, il a son propre écran) ;
- le conflit de taille et le `resize-window` avant chaque attache (plusieurs clients, plusieurs
  tailles) ;
- le collage en double (tmux a ses propres liaisons de touches, qu'il faut désarmer) ;
- l'analyse de la prose de `stderr` (l'interface est faite pour être lue par un humain) ;
- la chaîne de copie en cinq sauts (le presse-papier doit traverser la frontière du programme) ;
- les 41 forks au démarrage (la configuration ne se transmet qu'au démarrage du serveur).

Supprimer la cause supprime les sept d'un coup. Et surtout : **elle supprime les prochains**.
La liste de la section 3 n'est pas fermée — elle s'est allongée à chaque version. Chaque
nouvelle fonctionnalité de terminal qu'on ajoutera devra à nouveau se demander « et qu'est-ce
que tmux en fait de son côté ? ». Avec un service maison, cette question n'existe plus : la
grille, la sélection, les modes et le presse-papier sont **à nous**, dans un seul processus,
sans frontière à traverser ni prose à lire.

C'est la différence entre corriger des bugs et supprimer l'endroit où ils naissent.

### 5.3 L'interface réellement utilisée est petite ; tmux est énorme

Ce que Cockpit demande à un multiplexeur : créer, attacher, écrire, redimensionner, fermer,
lister, plus sélection / recherche / historique. C'est tout.

Ce que Cockpit **désactive** de tmux : la barre de statut, les panes, les fenêtres multiples,
la touche de préfixe, les tables de touches, les menus contextuels, le collage au clic molette.
La configuration générée est presque entièrement une liste de choses à éteindre.

On utilise une fraction de tmux et on paie le décalage à chaque appel. Ce décalage n'est pas un
accident qu'on pourrait corriger : il est dans la nature de l'outil. tmux est un multiplexeur
pour humains devant un clavier ; Cockpit a besoin d'un **serveur de sessions pour un
programme**. Ce ne sont pas les mêmes objets, et c'est pour ça que le raccord fuit.

À l'inverse, l'interface d'un service maison serait dessinée pour ce besoin précis, donc petite,
donc stable. Une interface petite et stable, c'est ce qui reste maintenable dix ans.

### 5.4 La partie risquée est exactement celle qui se teste de façon exacte

C'est ce qui fait la différence entre un pari et une décision, et c'est le point que je veux
défendre le plus explicitement.

Le risque du multiplexeur maison est concentré en un endroit : **la sérialisation
grille → séquences d'échappement** au moment de la reconnexion. « L'écran est revenu de
travers » est le pire genre de bug dans un terminal, parce qu'il est silencieux.

Or cette fonction a une propriété rare : **elle se vérifie par un aller-retour exact.**

> Pour n'importe quel état de grille : sérialiser en séquences d'échappement, puis analyser le
> résultat dans un émulateur neuf. Les deux grilles doivent être **identiques** — cellule par
> cellule, attributs compris, plus la position et la forme du curseur, l'écran actif, les
> régions de défilement et les modes DEC.

C'est une égalité, pas une appréciation. Donc c'est automatisable, et donc :

- on peut la nourrir avec des états produits au hasard (n'importe quel flux d'octets donne un
  état de grille valide) ;
- on peut la nourrir avec des **traces réelles** : enregistrer la sortie brute d'une session
  claude, d'un vim, d'un htop, la rejouer, et vérifier l'aller-retour à chaque étape ;
- chaque bug de fidélité trouvé en usage se réduit à une trace ajoutée à la liste, qui ne
  régresse plus jamais.

C'est un filet que tmux n'a jamais eu, et c'est précisément ce qui transforme « dix-huit ans de
correctifs » en un travail borné et mécanique.

Et il faut voir où va le reste du risque : **l'analyse des séquences d'échappement, qui est la
partie vraiment sans fin, n'est pas à nous.** Les nouveautés qui arrivent — protocole clavier
de Kitty, sortie synchronisée, liens OSC 8, images — sont le travail des gens qui maintiennent
`alacritty_terminal`, dont c'est le métier et dont le terminal est utilisé par beaucoup plus de
monde que Cockpit. Ce qui nous reste, c'est le service, le protocole et la sérialisation. Trois
choses de taille finie.

### 5.5 Ce qui n'est pas réversible

Il faut l'écrire noir sur blanc, parce que c'est le seul endroit où une erreur ne se rattrape
pas.

**Réversible sans difficulté :**
- Le choix de la bibliothèque d'émulation. En Rust, changer de moteur est un travail de
  compilation, pas un pari.
- Revenir à tmux. tmux existera encore ; le chemin actuel est dans l'historique git.
- Passer de « sans persistance » à « avec persistance » sur une plateforme : c'est un gain pur
  pour l'utilisateur, rien à défaire.

**Irréversible, et à traiter explicitement :**

1. **Les sessions tmux vivantes des utilisateurs au moment de la bascule.** C'est le point que
   personne ne voit venir. Aujourd'hui des utilisateurs ont des sessions `ckpt_*` qui tournent
   depuis des jours sur un serveur tmux, avec du travail dedans. La version qui passe au service
   maison ne saura pas les reprendre : ces shells seront soit tués, soit abandonnés vivants et
   injoignables — c'est-à-dire perdus, en continuant à consommer de la mémoire. Ce sont les
   machines d'autres personnes. Il faut donc une décision **prise à l'avance** : garder le
   chemin tmux en lecture seule le temps que les anciennes sessions s'éteignent d'elles-mêmes,
   ou les lister à l'utilisateur et le laisser trancher. Ne pas décider, c'est décider de les
   perdre en silence.

2. **Tout format écrit sur disque par le service.** Si le service persiste l'historique ou son
   état, ce format devient une charge de migration permanente. Le choix juste est de **ne rien
   persister** : le service tient tout en mémoire et meurt avec la machine. Ça correspond au
   besoin (survivre à la fermeture de l'app, pas au reboot — le même plafond que Zellij) et ça
   supprime la question des migrations. À décider au début, parce que revenir en arrière
   après coup demande de gérer les anciens formats.

3. **Le protocole entre l'app et le service.** Le service survit à l'app, donc une app neuve
   parlera à un service ancien. Il faut un numéro de version dans la poignée de main **dès la
   première version**, sinon on hérite du « protocol version mismatch » de tmux avec, en plus,
   la responsabilité. C'est le point 8 de la section 3 : on ne le supprime pas, on le prend en
   charge — mais seulement si on l'a prévu d'entrée.

### 5.6 Pourquoi tmux via WSL sur Windows est à rejeter

Sur le fond, indépendamment de tout effort : le terminal tournerait dans un autre système de
fichiers et un autre espace utilisateur que les sept autres onglets de Cockpit. Les fichiers du
projet sont sur Windows, le terminal les verrait sous `/mnt/c/...`, l'onglet Fichiers et
l'onglet Git sous `C:\...`. Il faudrait traduire les chemins dans les deux sens, et cette
traduction fuirait dans les chemins de projet, les commandes rapides, la détection d'agents,
les sessions Claude Code lues depuis `~/.claude/projects/`.

Cockpit est un outil dont le principe est que tout ce qui concerne un projet est au même
endroit. Faire vivre le terminal dans un autre monde que les fichiers du projet contredit le
principe même de l'application. **C'est incorrect, pas seulement coûteux.**

### 5.7 Ce qu'il faut faire dans quel ordre, et pourquoi

L'ordre suivant n'est pas dicté par la vitesse mais par la **justesse du diagnostic**.

**D'abord les quatre corrections de la section 4**, avant toute décision d'architecture. Non
pas parce qu'elles sont rapides, mais parce que **ce sont des problèmes indépendants du
multiplexeur** et qu'ils resteraient identiques après la bascule. Les laisser en place
reviendrait à juger la nouvelle architecture sur des symptômes qui ne lui appartiennent pas —
et, si la lenteur persistait après la bascule, à conclure faussement que le service maison est
lent. Le CLAUDE.md dit déjà la règle applicable : reproduire et instrumenter avant de patcher,
ne jamais enchaîner des correctifs hypothétiques. Ici c'est la même règle à l'échelle de
l'architecture.

Les quatre : rendre `list_all_terminals` `async` (et cesser de lancer `ps` à chaque passe) ;
remplacer la ligne 78 de `TerminalTab.svelte` par `atob` + boucle ; regrouper les morceaux de
sortie avant de les émettre ; enchaîner les 41 commandes de `apply_server_options` en un appel.

**Ensuite, sortir l'interface.** Un trait Rust `Terminaux` — créer, attacher, écrire,
redimensionner, fermer, lister, sélectionner, chercher — avec l'implémentation tmux actuelle
derrière. Cette étape a une valeur propre : elle **prouve** que l'interface dont Cockpit a
besoin est petite (§5.3), et elle la fige avant qu'on écrive le service, ce qui évite que le
service soit dessiné à l'image de tmux.

**Puis le service maison, et lui seul, sur les trois plateformes.** `alacritty_terminal` pour
la grille, `portable-pty` pour le PTY (ConPTY inclus, déjà là), `interprocess` pour le socket
et le tuyau nommé. **Écrire la sérialisation grille → ANSI en premier, avec son test
d'aller-retour (§5.4)** : c'est là qu'est tout le risque, et c'est la seule partie qui se
vérifie exactement. Traiter les trois points irréversibles de §5.5 **avant** la première
version publiée.

**Le PTY direct sans persistance a une place, mais une seule : un état de passage assumé**, le
temps que le service existe, derrière le même trait. Il est acceptable comme étape. Il est à
refuser comme destination — c'est exactement le « deux mécanismes pour toujours » de §5.1.

---

## Annexe — méthode et reproductibilité des mesures

Scripts dans le répertoire de travail de la session :

| Fichier | Ce qu'il mesure |
|---|---|
| `lat.sh`, `lat2.sh` | Latence des commandes tmux sur le serveur vivant ; séparation du coût de fork+exec ; comparaison serveur chargé / serveur neuf ; appels séparés contre appels enchaînés. |
| `thr.py`, `thr2.py` | Débit et latence de frappe, PTY nu contre tmux, via `pty.fork` + `select`. Le sentinelle de la première version était reconnu dans l'écho de la ligne de commande elle-même ; corrigé dans `thr2.py` en signalant la fin par un fichier. |
| `poll.py`, `poll2.sh` | Reproduction pas à pas de `TerminalState::list(db, None)`. `poll.py` mesure aussi la variation sous charge ; ses valeurs absolues sont gonflées par le fork de l'interpréteur Python, d'où `poll2.sh` en shell pur pour les chiffres retenus. |
| `b64.mjs`, `chain.mjs`, `chain2.mjs` | Coût du décodage base64 du frontend et de l'évaluation des scripts émis par Tauri. `chain2.mjs` utilise 240 scripts **distincts** : avec un script identique répété, V8 met en cache la compilation et le résultat n'a aucun sens. |

Précautions prises :

- **Charge système notée à chaque passe.** Les premières mesures, à une charge de 15 (les
  agents de cette session tournant en parallèle), donnaient 30 ms pour une commande tmux au
  lieu de 3,7. Refaites et écartées. Les valeurs retenues sont celles à charge connue.
- **Les sessions réelles n'ont pas été perturbées** : les attaches ont été mesurées sur des
  sockets de test (`bench2`, `bench3`, `bench4`), tous tués après usage. Sur le socket
  `cockpit`, uniquement des commandes de lecture.
- **Codes de sortie jamais lus derrière un tuyau** (règle du CLAUDE.md).
- Les affirmations sur le fonctionnement interne de Tauri et de wry sont vérifiées **dans les
  sources du cache cargo**, pas dans la documentation : `tauri-2.11.0/src/event/mod.rs:194`,
  `tauri-2.11.0/src/webview/mod.rs:1971`, `tauri-macros/src/command/wrapper.rs:249-266`,
  `wry-0.55.1/src/webkitgtk/mod.rs:638`, `portable-pty-0.9.0/src/win/psuedocon.rs:34`.
- Maturité des bibliothèques relevée sur l'API de crates.io (versions, dates, téléchargements),
  pas de mémoire.

### Fichiers du projet concernés

*(Depuis l'étape A du chantier, le 2026-08-20, `terminal/mod.rs` a été renommé en
`terminal/tmux.rs` et le trait vit dans `terminal/interface.rs` : les numéros de ligne
ci-dessous datent de l'analyse.)*

- `src-tauri/src/terminal/mod.rs` — `list()`, `apply_server_options()` (l. 524),
  `tmux_llm_sessions()` (l. 372), `absence_definitive()` (l. 289), `attach()` (l. 800),
  `spawn_attach()` (l. 671), `TMUX_CONF` (l. 475), `REPLAY_BUFFER_MAX` (l. 22).
- `src-tauri/src/lib.rs` — l. 755 `list_all_terminals` (à passer en `async`), l. 701-825
  l'ensemble des commandes terminal, toutes synchrones ; l. 1421-1428 le démarrage.
- `src/lib/components/project/TerminalTab.svelte` — l. 78 le décodage base64, l. 84-88 les
  écouteurs globaux, l. 96-103 la file d'écriture, l. 44-66 le pool de xterm.
- `src/lib/stores/terminals.ts` — l. 26 le poll de 5 secondes.
- `src-tauri/Cargo.toml` — `portable-pty = "0.9"` déjà présent.
