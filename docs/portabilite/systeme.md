# Metriques systeme, processus, detection des agents IA
## Portabilite Linux / macOS / Windows — Cockpit

Perimetre : `src-tauri/src/system/metrics.rs`, `src-tauri/src/system/process.rs`, et la
detection des CLI LLM dans `src-tauri/src/terminal/mod.rs` (lignes 318-432).

Tout ce qui est affirme ici sur `sysinfo` est verifie dans la source vendoree sur cette
machine (`~/.cargo/registry/src/index.crates.io-*/sysinfo-0.30.13/`), pas de memoire.
Lecture seule : rien n'a ete modifie.

---

## 1. Ce qui est lu aujourd'hui

| Ce qui est lu | Ou (fichier:ligne) | Equivalent sysinfo | Linux | macOS | Windows | Verdict |
|---|---|---|---|---|---|---|
| RAM totale / dispo / swap | `metrics.rs:126-145` (deja sysinfo) | `total_memory`, `available_memory`, `total_swap`, `used_swap` | oui | oui | oui | rien a faire |
| `Cached:` | `/proc/meminfo`, `metrics.rs:216` | **aucun** | fichier | non | non | **decision de conception** (§2) |
| `Buffers:` | `/proc/meminfo`, `metrics.rs:217` | **aucun** | fichier | notion inexistante | notion inexistante | **decision de conception** (§2) |
| `Shmem:` | `/proc/meminfo`, `metrics.rs:218` | **aucun** | fichier | non | non | **decision de conception** (§2) |
| `SReclaimable:` | `/proc/meminfo`, `metrics.rs:219` | **aucun** | fichier | non | non | **decision de conception** (§2) |
| ZFS ARC | `/proc/spl/kstat/zfs/arcstats`, `metrics.rs:227` | **aucun** | fichier | `sysctl kstat.zfs.misc.arcstats.size` si OpenZFS | quasi inexistant | Linux-only ; se masque deja tout seul (`if m.zfs_arc > 0`) |
| CPU global / par coeur / modele / nb coeurs | `metrics.rs:110-121` (deja sysinfo) | `cpus()`, `cpu_usage()`, `brand()` | oui | oui | oui | rien a faire |
| hostname | `metrics.rs:79` | `System::host_name()` | oui | oui | oui | rien a faire |
| uptime | `metrics.rs:102` | `System::uptime()` | oui | oui | oui | rien a faire |
| `kernel_version` | `metrics.rs:80` | `System::kernel_version()` | `6.17.0-35-generic` | `23.6.0` (version Darwin) | `22631` (numero de build) | **mecanique** : passer a `long_os_version()` |
| Disques | `metrics.rs:161-168`, filtre de 6 montages en dur | `Disks` | oui | oui | oui | **mecanique** : jeter le filtre (§5) |
| Liste des processus | `process.rs:21-46` (deja sysinfo) | `processes()` | oui | oui | oui | rien a faire |
| `user` du processus | `process.rs:37-40` | `user_id()` | uid `1000` | uid `501` | SID `S-1-5-21-...` | affiche deja un identifiant brut ; `Users::get_user_by_id()` donne le vrai nom sur les 3 |
| SIGTERM | `process.rs:97` | `kill_with(Signal::Term)` | oui | oui | **renvoie `None`** | **decision + perte** (§4) |
| `/proc/<pid>/exe` | `terminal/mod.rs:340` | `Process::exe()` | oui | oui (`proc_pidpath`) | oui (`GetModuleFileNameExW`) | **mecanique, garantie conservee** (§3) |
| `ps -e -o pid=,ppid=,args=` | `terminal/mod.rs:400` | `Process::parent()` + `cmd()` | oui | oui | **pas de `ps`** | **mecanique** : arbre sysinfo |
| `tmux list-panes` (racine de l'arbre) | `terminal/mod.rs:375` | — | oui | oui | pas de tmux | depend du chantier terminaux |

**Le fait qui structure tout le reste** : sysinfo n'expose que **7 nombres de memoire**
(`total_memory`, `free_memory`, `available_memory`, `used_memory` et les 3 du swap), plus
`cgroup_limits()` qui est Linux-only. Verifie en listant les methodes publiques de
`common.rs`. Il n'existe **aucune** API cache / buffers / partage / compresse, sur aucune
plateforme. Le detail memoire n'est donc pas un manque de sysinfo qu'une montee de version
comblerait : c'est du code natif par systeme, ou rien.

---

## 2. Decision de conception n°1 — le detail memoire

L'ecran affiche 5 barres : Processus, ZFS ARC, Cache, Partage, Buffers. La barre
« Processus » est obtenue par soustraction, cote frontend
(`src/lib/components/dashboard/MonitoringView.svelte:26`) :

```js
const processBytes = Math.max(0, m.used - m.cached - m.buffers - m.s_reclaimable - m.zfs_arc);
```

Ce calcul n'a de sens que sous Linux. Ailleurs, les categories ne se traduisent pas : ce ne
sont pas d'autres noms pour les memes choses, ce sont d'autres decoupages.

- **macOS** compte : *wired* (non deplacable), *active*, *inactive*, **compresse** (macOS
  compresse la RAM au lieu de swapper — aucun equivalent Linux), *purgeable*, *speculative*.
  Lisible via `host_statistics64` / `HOST_VM_INFO64`. Ironie : sysinfo lit deja exactement
  ces champs en interne (`unix/apple/system.rs:145-157`) pour calculer `available` et `used`,
  mais ne les expose pas.
- **Windows** compte : *En cours d'utilisation*, *Cache*, *Pool pagine*, *Pool non pagine*,
  *Valide / Limite de validation*. Lisible via `GetPerformanceInfo`, en plus du
  `GlobalMemoryStatusEx` que sysinfo utilise deja (`windows/system.rs:144-146`). Ni
  « buffers », ni « memoire partagee », ni « reclaimable » n'existent.

### Les trois architectures possibles

**A — Socle commun partout, detail Linux seulement.**
Jauges CPU/memoire, total, utilise, swap, disques, processus : identiques sur les 3. Le
panneau des 5 barres n'apparait que sous Linux (un `#[cfg]` cote Rust, une garde cote Svelte,
les 4 champs Linux passent en `Option`).
- *Nature du travail* : mecanique, plus une garde d'affichage.
- *Ce qu'on perd* : rien pour l'utilisateur Linux, et rien pour les autres qui n'ont jamais
  eu cet ecran. Le prix reel est conceptuel : « le monitoring » n'est plus une seule chose,
  il faut se souvenir que l'ecran differe selon le systeme.
- *Risque* : faible. Un seul chemin de code, testable sur la machine de developpement.

**B — Un detail par systeme.**
Trois decoupages, trois jeux de libelles dans `fr.ts` et `en.ts` (« Compressee », « Wired »,
« Pool pagine »...), deux blocs de code natif nouveaux (mach sur macOS, psapi sur Windows).
- *Nature du travail* : deux implementations natives neuves + trois vocabulaires a maintenir
  dans les catalogues pour un seul ecran.
- *Ce qu'on perd* : l'unite du code. Chaque evolution de cet ecran devra etre pensee trois
  fois.
- *Risque* : **le plus eleve des trois, et pour une raison qui ne depend pas de l'effort** :
  deux branches sur trois ne sont pas testables ici. Leur regression ne se verra que chez un
  utilisateur, sur une machine a laquelle nous n'avons pas acces — donc diagnostic a
  l'aveugle. C'est exactement le scenario qui a coute cher sur le bug des accents et sur
  celui de `libwayland` dans l'AppImage.

**C — Supprimer le detail.**
Total / utilise / swap, rien d'autre.
- *Nature du travail* : suppression.
- *Ce qu'on perd* : la seule information de cet ecran qui ne se trouve pas ailleurs. Sur la
  machine de Jimmy, l'ARC ZFS peut avaler des dizaines de Go et fait passer une memoire saine
  pour une fuite ; c'est precisement ce que la barre violette explique.
- *Risque* : nul techniquement. Mais c'est une perte de fonctionnalite assumee sur la
  plateforme principale, pour le confort des deux autres.

### Ma reponse tranchee : A

Categories differentes selon le systeme **non**, socle commun **oui**, avec un supplement
Linux. Trois raisons :

1. C'est la seule des trois options qui **ne detruit rien d'existant**. B ajoute du risque
   non testable, C retire une information utile a l'utilisateur principal.
2. B pourra se greffer plus tard, systeme par systeme, **sans rien casser** : c'est la meme
   structure `Option`. Choisir A maintenant ne ferme pas la porte ; choisir B maintenant
   installe une divergence permanente qu'on ne retire plus.
3. Un ecran qui montre moins sur macOS/Windows est honnete. Un ecran qui montre trois
   vocabulaires differents pour la meme jauge demande a l'utilisateur d'apprendre son
   systeme d'exploitation avant de lire Cockpit.

**Ce qui reste a valider par Jimmy** : accepte-t-il qu'un ecran de Cockpit ne montre pas la
meme chose selon le systeme ? C'est la seule question ; le reste suit.

---

## 3. Decision de conception n°2 — detection des agents IA

Le commentaire de `terminal/mod.rs:331-338` explique le choix de `/proc/<pid>/exe` : `argv`
peut mentir. Constate le 2026-08-14, un `claude` natif lance depuis un shell ou trainait la
variable `APPIMAGE` s'affichait comme `.../target/release/cockpit -r` dans `ps` **et** dans
`pane_current_command` — la detection par nom de commande devenait aveugle.

La documentation de sysinfo dit exactement la meme chose de `Process::exe()`
(`common.rs:932-943`) :

> On Linux, this method will return an empty path if there was an error trying to read
> `/proc/<pid>/exe`. [...] It is also the case that `cmd[0]` is _not_ usually a correct
> replacement for this. A process may change its `cmd[0]` value freely, making this an
> untrustworthy source of information.

### Reponse directe : sur le chemin `exe()`, la resistance est CONSERVEE sur les trois systemes

| | Comment sysinfo obtient `exe()` | Resiste a l'usurpation d'argv ? |
|---|---|---|
| Linux | `readlink /proc/<pid>/exe` — exactement le code qu'on ecrit a la main | oui |
| macOS | `proc_pidpath()` (`unix/apple/macos/process.rs:405`) | oui |
| Windows | `GetModuleFileNameExW`, repli `get_executable_path` (`windows/process.rs:318-325, 1034`) | oui |

`exe_is_llm()` est donc du **remplacement mecanique** :
`std::fs::read_link(format!("/proc/{}/exe", pid))` devient
`sys.process(pid).and_then(|p| p.exe())`, et la correspondance par composant de chemin —
celle qui reconnait `~/.local/share/claude/versions/2.1.231` — ne change pas d'une ligne.

Ce qui n'est pas portable, c'est la **facon d'obtenir l'arbre de processus** :
`ps -e -o pid=,ppid=,args=` (`terminal/mod.rs:400`) n'existe pas sous Windows. `sysinfo`
donne `parent()` et `cmd()` sur les trois systemes, et on tient deja un `System` vivant dans
le `Collector`. On construit la table `children` en iterant `sys.processes()` au lieu de
parser du texte : moins de code que l'existant, plus de parsing fragile, et un process
externe en moins toutes les 5 secondes (`src/lib/stores/terminals.ts:26`).

### Ce qu'on perd, point par point

1. **Rien sur `exe()`.** C'est le point important a retenir : la preuve anti-usurpation
   survit intacte, Windows compris.

2. **Les CLI lances par un script node restent detectes par la ligne de commande**
   (`args_are_llm`, `terminal/mod.rs:352`), donc restent usurpables. C'est **deja vrai
   aujourd'hui sous Linux** : `exe()` ne sauve que les binaires natifs. Mais sous Windows,
   ou un `claude` installe par npm est un shim `.cmd` autour de `node.exe`, ce repli devient
   la regle et non l'exception.
   → **Perte de fonctionnalite a assumer : sous Windows, le logo Claude est un indice, pas
   une preuve.** Et ce n'est **pas rattrapable** : on ne peut pas distinguer un `node.exe`
   qui execute Claude d'un `node.exe` qui pretend l'executer sans lire ses arguments —
   c'est-a-dire sans revenir a la source qui mentait.
   → *Risque* : faible en soi (personne ne prend de decision sur ce logo), mais il faut
   arreter de le presenter comme une certitude dans la doc integree.

3. **La racine de l'arbre vient de tmux** (`list-panes -F #{pane_pid}`,
   `terminal/mod.rs:375`), absent sous Windows. Ce point appartient au chantier terminaux.
   A noter quand meme : si les terminaux Windows passent par un PTY direct (ConPTY) au lieu
   de tmux, Cockpit **connait deja** le PID du shell qu'il a lance — la racine devient plus
   fiable qu'aujourd'hui, pas moins. C'est un des rares endroits ou l'absence de tmux aide.

4. **Sur macOS, `exe()` et `cmd()` d'un processus appartenant a un AUTRE utilisateur exigent
   des privileges** (`KERN_PROCARGS2`). Sans effet ici : les agents tournent dans nos propres
   terminaux, donc sous notre uid.

---

## 4. Tuer un processus sans signaux sous Windows

`process.rs:97` envoie `Signal::Term`. Ce que fait reellement sysinfo 0.30 :

| | Signaux acceptes par `kill_with` | Mecanisme |
|---|---|---|
| Linux | tous les POSIX | `kill(2)` |
| macOS | tous les POSIX (`unix/apple/mod.rs:41-76`, `Signal::Term => libc::SIGTERM`) | `kill(2)` |
| Windows | **`Signal::Kill` uniquement** (`windows/mod.rs:29-33`) | `taskkill.exe /PID <pid> /F` (`windows/process.rs:441-449`) |

### Le piege est silencieux

Sur Windows, `kill_with(Signal::Term)` **compile sans erreur** — la variante `Signal::Term`
existe sur toutes les plateformes, c'est la conversion qui rend `None`
(`windows/mod.rs:32`, la branche `_ => None`). A l'execution, notre code
(`process.rs:97-99`) en fait :

```
Err("failed to send SIGTERM to PID 1234")
```

Un message qui **nomme un mecanisme inexistant sur ce systeme** et laisse croire a un
probleme de permission. C'est le motif « un silence, c'est un bug » du CLAUDE.md, dans sa
version « une erreur qui raconte n'importe quoi » — la pire des deux, parce qu'elle envoie
le diagnostic dans la mauvaise direction.

### La distinction « demander gentiment » / « forcer » : a garder, mais asymetrique

- **Linux / macOS** : **Arreter** = `Signal::Term` (le processus peut sauvegarder et se
  fermer proprement), **Forcer** = `Signal::Kill`.
- **Windows** : `taskkill /F` *est* l'arret force, et c'est le seul chemin que sysinfo
  expose. Un bouton unique, qui doit s'appeler **« Forcer l'arret »** et pas « Arreter ».

Un arret doux existe sous Windows (`WM_CLOSE` sur les fenetres, `GenerateConsoleCtrlEvent`
pour un process console) mais il ne marche que pour certains types de processus et sysinfo
ne l'expose pas.
→ **Je ne le recommande pas** : une action qui reussit pour certains processus et echoue
pour d'autres, sans qu'on puisse dire lesquels a l'avance, est pire qu'une action franche.

- *Nature du travail* : mecanique cote Rust (un `#[cfg]` sur le signal choisi), **decision de
  conception** cote interface (un bouton ou deux), et **perte de fonctionnalite** assumee.
- *Ce qu'on perd* : sous Windows, la possibilite de laisser un processus se fermer proprement.
  Concretement, un editeur tue depuis Cockpit perd son travail non sauvegarde, la ou sous
  Linux il aurait eu la main.
- *Risque* : la perte doit etre **visible dans le libelle**, sinon un utilisateur Windows
  habitue a Cockpit sous Linux cliquera « Arreter » en croyant que son editeur va sauvegarder.
  Le risque n'est pas technique, il est dans le malentendu.

### Detail a corriger en meme temps

La garde `if pid <= 1` (`process.rs:90`) raisonne en Unix (PID 1 = init/launchd). Sous
Windows il faut aussi refuser **0** (System Idle Process) et **4** (le processus `System`).
Sinon un clic sur la premiere ligne du tableau part en `taskkill /F` sur le noyau — ca
echouera, mais le refus doit etre explicite et explique, pas confie au hasard des permissions.

---

## 5. Le filtrage des disques

Le filtre de `metrics.rs:163-169` garde 6 points de montage Unix en dur :

```rust
matches!(mount.as_ref(), "/" | "/home" | "/boot" | "/var" | "/tmp" | "/opt")
```

- **Sous Windows** il ne matcherait **rien** : les montages sont `C:\`, `D:\`. Liste de
  disques vide, sans message — encore un silence.
- **Sous macOS** il ne garderait que `/`, en laissant tomber `/System/Volumes/Data`,
  c'est-a-dire le volume ou sont reellement les fichiers de l'utilisateur.

### Ce que je n'attendais pas : sysinfo filtre deja, et bien, sur les trois systemes

- **Linux** (`unix/linux/disk.rs:247-272`) : ecarte `sysfs`, `proc`, `devtmpfs`, `cgroup`,
  `cgroup2`, `pstore`, `squashfs` (donc **tous les montages snap**, la raison principale
  d'avoir un filtre maison), `rpc_pipefs`, `iso9660`, `tmpfs`, et tout ce qui est monte sous
  `/sys`, `/proc`, `/run` (sauf `/run/media`).
- **macOS** (`unix/apple/disk.rs:170-197`) : ecarte les volumes non *browsable*
  (`kCFURLVolumeIsBrowsableKey` — instantanes APFS, volumes systeme caches) et les volumes
  non locaux (`kCFURLVolumeIsLocalKey` — montages SMB, NFS). Le commentaire de sysinfo dit
  explicitement que c'est pour eviter de dupliquer les disques.
- **Windows** (`windows/disk.rs:234-238`) : ne garde que `DRIVE_FIXED` et `DRIVE_REMOVABLE` —
  pas de lecteur reseau, pas de CD-ROM, pas de RAM disk.

### Verdict : remplacement mecanique, supprimer le filtre en dur

Une closure `.filter()` qui disparait. Ca corrige au passage un defaut actuel sous Linux :
un disque monte sur `/mnt/data` ou `/srv` est aujourd'hui **invisible** dans Cockpit, sans
aucune explication.

- *Ce qu'on perd* : la maitrise de la longueur de la liste. Le filtre garantissait au plus
  6 cartes ; en s'en remettant a sysinfo, une machine avec des sous-volumes btrfs, des
  datasets ZFS ou des montages Docker peut en afficher beaucoup plus.
- *Risque* : cosmetique, plus deux effets a connaitre :

1. **Sous macOS (APFS) et sur ZFS, plusieurs « disques » partagent le meme espace physique.**
   `/` et `/System/Volumes/Data` sont deux volumes du meme conteneur : meme total, meme
   libre. Additionner les totaux donnerait un chiffre faux. L'ecran actuel
   (`src/lib/components/system/SystemMonitor.svelte:69-71`) affiche une carte par disque et
   n'additionne jamais — **il est donc deja correct**, il montrera simplement deux cartes qui
   se ressemblent.
   → **Ne pas « corriger » ca par une deduplication maison** : c'est de l'heuristique qui se
   trompera (deux volumes distincts peuvent legitimement avoir le meme total).
2. **Sous Windows, `disk.name()` est l'etiquette du volume et peut etre vide.** L'affichage
   `{disk.mount} ({disk.device})` donnerait « C:\ () ». Il faut un repli quand l'etiquette
   est vide.

**A trancher par Jimmy** : tous les disques locaux, ou seulement le disque systeme ?

---

## 6. Recapitulatif par nature de travail

### Remplacement mecanique — meme comportement, autre appel, rien de perdu

- `read_link("/proc/<pid>/exe")` → `Process::exe()` : la garantie anti-usurpation survit sur
  les trois systemes
- `ps -e -o pid=,ppid=,args=` → arbre construit sur `Process::parent()` : moins de code
  qu'aujourd'hui, un process externe en moins toutes les 5 s
- filtre disques en dur → confiance a sysinfo, avec un repli sur l'etiquette vide
- `kernel_version()` → `long_os_version()` : sinon Windows affiche « 22631 » a cote du
  hostname, et macOS « 23.6.0 »
- garde `pid <= 1` → refuser aussi 0 et 4 sous Windows
- `format_uptime` (`metrics.rs:242-251`) ecrit « j » / « h » / « m » en dur dans le Rust :
  c'est du texte affiche qui ne passe pas par les catalogues i18n, a remonter cote frontend
- `user_id()` → resolution par `Users::get_user_by_id()` : portable sur les 3, et evite
  d'afficher un SID brut sous Windows (aujourd'hui on affiche deja un uid brut)

### Decisions de conception

1. **Detail memoire** (§2). Socle commun avec detail Linux-only — **c'est ma reponse** — ou
   un detail par systeme, ou suppression du detail. Les notions ne se traduisent pas : macOS
   a de la memoire *compressee* que Linux n'a pas, Linux a des *buffers* que Windows n'a pas.
   L'option « un detail par systeme » porte un risque particulier : deux branches sur trois
   ne sont pas testables ici.
2. **Detection des agents** (§3). La preuve tient pour les binaires natifs sur les trois
   systemes ; on tombe sur l'indice usurpable pour les CLI enveloppes dans node, et ce cas
   devient majoritaire sous Windows. A valider explicitement, puisque la resistance a
   l'usurpation etait un choix conscient.
3. **Arret de processus** (§4). Un seul bouton « Forcer l'arret » sous Windows la ou Linux et
   macOS en ont deux.
4. **Disques** (§5). Tous les disques locaux, ou seulement le disque systeme ?

### Pertes de fonctionnalite, listees pour qu'elles soient dites et pas subies

- Sous Windows : pas d'arret doux d'un processus. Non rattrapable proprement.
- Sous Windows : le logo Claude devient un indice usurpable pour les CLI lances via node.
  Non rattrapable.
- Sous macOS et Windows : pas de detail memoire, si on retient l'option A.

---

## 7. Point de vigilance, hors questions posees

`collect()` (`metrics.rs:86-107`) appelle `refresh_processes()` a **chaque** passage, soit
toutes les 3 s en mode Live (`src/lib/stores/system.ts:7`). Sous Windows, sysinfo ouvre un
handle par processus pour lire CPU et memoire — nettement plus lourd que lire `/proc`.

Et sans droits administrateur, les processus appartenant a d'autres utilisateurs rendront
CPU et memoire a **zero** plutot qu'une erreur : un tableau qui affiche des zeros au lieu de
dire qu'il ne sait pas. C'est le meme defaut que les commandes d'observation qui fabriquent
un mensonge en cas d'echec (« aucun conteneur » != « docker en panne »).

A mesurer sur une vraie machine Windows avant de laisser le mode Live activable la-bas.

---

## 8. Fichiers concernes

- `/home/jguevel/Documents/workspace/core/cockpit/src-tauri/src/system/metrics.rs`
- `/home/jguevel/Documents/workspace/core/cockpit/src-tauri/src/system/process.rs`
- `/home/jguevel/Documents/workspace/core/cockpit/src-tauri/src/terminal/mod.rs`
  (lignes 318-432 : `LLM_COMMANDS`, `exe_is_llm`, `args_are_llm`, `tmux_llm_sessions`)
- `/home/jguevel/Documents/workspace/core/cockpit/src/lib/components/dashboard/MonitoringView.svelte`
  (ligne 26 : le calcul de la barre « Processus »)
- `/home/jguevel/Documents/workspace/core/cockpit/src/lib/components/system/SystemMonitor.svelte`
  (lignes 69-71 : l'affichage des disques)
- `/home/jguevel/Documents/workspace/core/cockpit/src/lib/stores/system.ts` (cadence Live 3 s)
- `/home/jguevel/Documents/workspace/core/cockpit/src/lib/stores/terminals.ts`
  (ligne 26 : sondage 5 s de la detection des agents)
