<script lang="ts" module>
  import { ecouter as listenGlobal } from "../../coquille";
  import { cheminDuFichier } from "../../coquille";
  import { writeTerminal } from "../../api/workspace";
  import { notify as notifyGlobal } from "../../stores/toast";
  import type { Terminal as XTerminal } from "@xterm/xterm";
  import type { FitAddon as XFitAddon } from "@xterm/addon-fit";
  import { trad, translate } from "../../i18n";
  import { signalerErreur } from "../../stores/errors";

  /// POOL PERSISTANT — LE COEUR DE L'ARCHITECTURE TERMINAUX (NE PAS RE-LOCALISER).
  ///
  /// Les instances xterm SURVIVENT au demontage de l'onglet. Changer de projet ou d'onglet
  /// ne detache rien : on gare les elements DOM dans un conteneur invisible et on les
  /// re-adopte au retour. Le switch est un pur masquer/montrer.
  ///
  /// Pourquoi c'est indispensable, et pas une optimisation : un xterm re-cree part vide, et
  /// le faire remplir exige de redemander au serveur un redessin complet — ecran ET
  /// historique. C'est cher, ca clignote, et ca ramene l'utilisateur en bas du defilement a
  /// chaque aller-retour entre deux onglets. Cote serveur, `attachTerminal` est d'ailleurs
  /// sans effet quand le terminal est deja branche, pour exactement la meme raison.
  ///
  /// A l'epoque de tmux, la meme regle tenait pour une autre cause : tmux synthetisait des
  /// evenements focus vers l'application du pane a chaque attache de client, et claude y
  /// reagissait par un re-render qui laissait une ligne vide. Le pool a survecu a tmux.
  /// `dataSub` : abonnement au flux de frappe (onData).
  ///
  /// Il DOIT etre retenu et libere avant tout nouvel abonnement. Les xterm vivent dans ce
  /// pool au niveau module et survivent aux demontages du composant : sans cela, chaque
  /// retour sur un terminal ajoutait un abonnement de plus, et tout ce qui etait tape ou
  /// colle partait autant de fois vers le PTY. C'est la cause du « collage en double »
  /// signale par un utilisateur — mesure au banc : 1 clic molette, 1 appel de collage,
  /// 3 insertions dans le terminal.
  ///
  /// Le meme symptome est revenu depuis, pour une cause TOUTE AUTRE (xterm qui colle de son
  /// cote, voir createXterm) alors que `brancherEntree` etait intact : ne pas conclure d'ici.
  export type PoolEntry = {
    term: XTerminal;
    fit: XFitAddon;
    el: HTMLDivElement;
    dataSub?: { dispose(): void };
  };
  const pool = new Map<number, PoolEntry>();

  let parkingEl: HTMLDivElement | null = null;
  /// Garage DOM invisible : un canvas WebGL detache du document perd son contexte, on ne
  /// laisse donc jamais un element du pool orphelin.
  function parking(): HTMLDivElement {
    if (!parkingEl) {
      parkingEl = document.createElement("div");
      parkingEl.style.display = "none";
      document.body.appendChild(parkingEl);
    }
    return parkingEl;
  }
  function parkAll() {
    for (const { el } of pool.values()) parking().appendChild(el);
  }
  function disposePoolEntry(id: number) {
    const e = pool.get(id);
    if (!e) return;
    e.dataSub?.dispose();
    e.term.dispose();
    e.el.remove();
    pool.delete(id);
  }

  /// Branche la frappe de ce terminal vers son PTY, en REMPLACANT l'abonnement precedent.
  /// Passer par ici est obligatoire : appeler `term.onData` directement empile les
  /// abonnements et multiplie chaque caractere envoye.
  function brancherEntree(entry: PoolEntry, envoyer: (data: string) => void) {
    entry.dataSub?.dispose();
    entry.dataSub = entry.term.onData(envoyer);
  }

  /// Decodage base64 de la sortie du PTY. La boucle `for` n'est PAS une coquetterie :
  /// `Uint8Array.from(atob(data), cb)` appelle la fonction de transformation une fois par
  /// caractere, et ce code tourne sur le thread qui dessine. Mesure du 2026-08-20 sur une
  /// rafale reelle (1,96 Mo) : 75,2 ms contre 2,8 ms ici. NE PAS « simplifier ».
  function b64ToBytes(data: string): Uint8Array {
    const texte = atob(data);
    const octets = new Uint8Array(texte.length);
    for (let i = 0; i < texte.length; i++) octets[i] = texte.charCodeAt(i);
    return octets;
  }

  // Listeners GLOBAUX, enregistres une fois pour la vie de l'app : la sortie doit continuer
  // d'alimenter les xterm du pool meme quand aucun onglet Terminal n'est monte, sinon on
  // retrouverait un ecran fige au retour.
  // Sortie brute et redessins arrivent par le MEME evenement : un redessin commence par une
  // remise a plat (RIS), xterm n'a donc rien de particulier a faire pour l'appliquer.
  // La sortie est ecrite a xterm DES qu'elle arrive. Il ne faut RIEN mettre entre les deux :
  // xterm decoupe deja son travail en tranches de 12 ms et rend la main entre deux, donc le
  // clavier ne se bloque pas pendant une grosse sortie. Une file qui attendait une image
  // (requestAnimationFrame) a ete essayee dans la 0.54.10 : chaque echo de frappe payait une image
  // de retard, et quand le moteur ne peint plus — cas documente dans le guetteur — la sortie
  // s'arretait completement au lieu d'etre au moins analysee. Retiree dans la 0.54.12.
  listenGlobal<{ id: number; data: string }>("terminal_output", (e) => {
    pool.get(e.payload.id)?.term.write(b64ToBytes(e.payload.data));
  });
  listenGlobal<number>("terminal_exit", (e) => {
    pool.get(e.payload)?.term.write("\r\n\x1b[2m[processus terminé]\x1b[0m\r\n");
  });

  // Un seul appel IPC a la fois : l'ordre des frappes est une regle du terminal. Le client Rust
  // les depose ensuite dans son fil d'ecriture, sans bloquer sur le socket.
  const ioQueues = new Map<number, Promise<unknown>>();

  function enqueue(id: number, op: () => Promise<unknown>) {
    const next = (ioQueues.get(id) ?? Promise.resolve()).then(op, op);
    ioQueues.set(id, next.catch(() => {}));
  }
  function queueWrite(id: number, data: string) {
    enqueue(id, () => writeTerminal(id, data));
  }

  /// GLISSER-DEPOSER DE FICHIERS -> chemin insere dans le terminal.
  ///
  /// Pourquoi ca ne marchait pas : Tauri intercepte le glisser-deposer natif du webview
  /// Le glisser-deposer est celui du NAVIGATEUR. Tauri interceptait ces evenements et les
  /// remplacait par les siens ; en 0.59.0 la coquille Electron ne les a pas remplaces, donc
  /// un fichier lache sur un terminal ne faisait plus rien. Les evenements du DOM suffisent,
  /// a une chose pres : le CHEMIN, que seule la coquille peut rendre.
  ///
  /// Une seule inscription pour la vie de l'app (les ecouteurs sont globaux) ; le montage
  /// actif de TerminalTab declare sa cible ci-dessous.
  type DropTarget = {
    el: HTMLElement;
    /// Lu a chaque evenement, jamais capture : l'onglet actif change sans reinscription.
    activeId: () => number | null;
    over: (v: boolean) => void;
  };
  let dropTarget: DropTarget | null = null;
  export function setDropTarget(t: DropTarget | null) {
    dropTarget = t;
  }

  /// **LES COORDONNEES D'UN EVENEMENT DU DOM SONT DEJA EN PIXELS CSS**, zoom compris : il n'y
  /// a rien a diviser. L'evenement de Tauri, lui, donnait des pixels PHYSIQUES, et il fallait
  /// le ramener a l'echelle de la page.
  function overTerminal(x: number, y: number): boolean {
    if (!dropTarget) return false;
    const el = document.elementFromPoint(x, y);
    return !!el && dropTarget.el.contains(el);
  }

  /// Le chemin part dans un PTY : il sera relu par un shell. On echappe a la maniere d'un
  /// terminal natif (antislash devant les caracteres interpretes) plutot qu'en entourant de
  /// guillemets, qui genent la detection du chemin par les agents type Claude Code. Un chemin
  /// ordinaire, sans caractere special, ressort donc intact.
  function escapeForShell(path: string): string {
    return path.replace(/[ \t\n"'`$&|;<>()!*?[\]\\#]/g, (c) => "\\" + c);
  }

  /// Les chemins des fichiers laches. **`File.path` N'EXISTE PLUS DEPUIS ELECTRON 32** :
  /// seule la coquille sait rendre le chemin d'un fichier depose.
  function cheminsDeposes(transfert: DataTransfer | null): string[] {
    if (!transfert) return [];
    return Array.from(transfert.files)
      .map((fichier) => cheminDuFichier(fichier))
      .filter((chemin) => chemin.length > 0);
  }

  window.addEventListener("dragover", (e) => {
    // **SANS CE `preventDefault`, LE DEPOT N'A JAMAIS LIEU** : le navigateur refuse par
    // defaut de laisser tomber quoi que ce soit sur la page.
    e.preventDefault();
    dropTarget?.over(overTerminal(e.clientX, e.clientY));
  });

  window.addEventListener("dragleave", () => dropTarget?.over(false));

  window.addEventListener("drop", (e) => {
    // Sans ca, Chromium OUVRE le fichier a la place de l'interface.
    e.preventDefault();
    dropTarget?.over(false);
    const chemins = cheminsDeposes(e.dataTransfer);
    if (chemins.length === 0) return;
    // Un depot est un geste delibere : s'il n'aboutit pas, on dit pourquoi plutot que de
    // ne rien faire (un silence, c'est un bug).
    if (!overTerminal(e.clientX, e.clientY)) {
      notifyGlobal(translate("term.dropOnTerminal"));
      return;
    }
    const id = dropTarget?.activeId() ?? null;
    if (id === null) {
      notifyGlobal(translate("term.noTerminalOpen"));
      return;
    }
    queueWrite(id, chemins.map(escapeForShell).join(" ") + " ");
    pool.get(id)?.term.focus();
  });
</script>

<script lang="ts">
  import { onMount, untrack } from "svelte";
  import { get } from "svelte/store";
  import { ecouter, type Detacher } from "../../coquille";
  import { Terminal } from "@xterm/xterm";
  import { FitAddon } from "@xterm/addon-fit";
  import { WebglAddon } from "@xterm/addon-webgl";
  import { WebLinksAddon } from "@xterm/addon-web-links";
  import "@xterm/xterm/css/xterm.css";
  import {
    pendingTerminalId, pendingTerminalCommand, TERMINAL_FONT_SIZE, consumeTabRestored,
  } from "../../stores/ui";
  // themeBase et non la palette : xterm n a que deux jeux de couleurs.
  import { themeBase } from "../../stores/appearance";
  import { projects } from "../../stores/projects";
  import { loadTerminals } from "../../stores/terminals";
  import {
    createTerminal, resizeTerminal, closeTerminal,
    attachTerminal, renameTerminal, listTerminals, listAllTerminals,
    setClipboard, getClipboard,
    terminalSearch, openUrl, saveTerminalScreens,
  } from "../../api/workspace";
  import { notify } from "../../stores/toast";
  import ContextMenu from "../ui/ContextMenu.svelte";
  import VoletTerminal from "./VoletTerminal.svelte";
  import SelecteurWorktree from "./SelecteurWorktree.svelte";
  import { grouper, teintes, worktreeDe, type Groupe } from "../../terminaux/worktrees";
  import { gitWorktrees, gitWorktreeAdd, gitWorktreeRemove } from "../../api/workspace";
  import { demanderTexte } from "../../stores/saisie";
  import { demanderConfirmation } from "../../stores/confirm";
  import type { Worktree } from "../../types";
  import {
    deplacer, depuisJson, diviser, feuille, fixerRatio, nettoyer, nombreDeVolets,
    poserLaSession, retirer, sessionsAffichees, type Chemin, type Cote, type Noeud,
  } from "../../terminaux/disposition";
  import { coteVise, dansLeCadre, voisinLePlusProche } from "../../terminaux/visee";
  import { getAppSettings, setAppSetting } from "../../api/recorder";
  import {
    conversationsLlm,
    renommerConversationLlm,
    commandesLlm,
    type ConversationLlm,
  } from "../../api/llm";
  import { agentPrefere } from "../../stores/llm";

  let { name }: { name: string } = $props();

  let sessions: { id: number; alive: boolean; name: string; cwd: string }[] = $state([]);
  let activeId: number | null = $state(null);
  let container: HTMLDivElement | undefined = $state(undefined);
  // Menu contextuel Copier/Coller du terminal
  let ctxMenu: { x: number; y: number } | null = $state(null);
  /// Le clic droit sur une puce de dossier de travail. Le groupe est GARDE ICI et repasse en
  /// parametre aux actions : le menu se ferme avant qu'elles ne s'executent.
  let menuWorktree: { x: number; y: number } | null = $state(null);
  let renamingId: number | null = $state(null);
  let renameValue = $state("");
  // Un fichier survole le terminal : on l'annonce, sinon on ne sait pas que le geste est permis.
  let dropOver = $state(false);

  // --- Les volets ---------------------------------------------------------------------
  //
  // **UN VOLET AFFICHE UNE SESSION QUI EXISTE DEJA.** L'arbre ne cree jamais de terminal :
  // c'est toujours `addTerminal` qui le fait, parce que lui seul mesure son conteneur (une
  // TUI se dessine a la taille du PTY et personne ne la redimensionne apres).
  let disposition: Noeud | null = $state(null);
  /// Le conteneur de chaque volet affiche, tenu par le composant qui le rend.
  const hotes = new Map<number, HTMLDivElement>();
  const voletsAffiches = $derived(sessionsAffichees(disposition));
  // --- Les dossiers de travail (worktrees) ---------------------------------------------
  //
  // **UN SUJET, UNE BRANCHE, UN DOSSIER, UN LOT DE TERMINAUX.** Le rattachement existait deja
  // (chaque terminal garde le dossier ou il a ete ouvert), il n'etait simplement pas montre.
  let worktrees: Worktree[] = $state([]);
  let worktreeActif: string | null = $state(null);

  const groupes = $derived(grouper(worktrees, sessions));
  const couleursWorktree = $derived(teintes(worktrees));
  /// La couleur du dossier affiche, reprise sur le liseré du volet actif : meme reperage aux
  /// deux endroits.
  const couleurActive = $derived(
    worktreeActif ? (couleursWorktree.get(worktreeActif) ?? null) : null,
  );

  /// Les onglets du dossier affiche.
  ///
  /// **CE FILTRE PORTE SUR LE DOSSIER, JAMAIS SUR `alive`.** Un terminal endormi reste
  /// visible dans son dossier : le filtre sur `alive` a deja fait disparaitre tous les
  /// onglets a chaque extinction du poste. Et rien ne se perd ici non plus, puisque chaque
  /// puce affiche COMBIEN de terminaux elle contient, et que la barre laterale les montre
  /// tous.
  const sessionsVisibles = $derived.by(() => {
    if (!worktreeActif || worktrees.length === 0) return sessions;
    const groupe = groupes.find((g) => g.chemin === worktreeActif);
    if (!groupe) return sessions;
    const dedans = new Set(groupe.terminaux);
    return sessions.filter((s) => dedans.has(s.id));
  });

  /// Relit les dossiers de travail du projet.
  ///
  /// Un projet qui n'est pas un depot git n'en a aucun : ce n'est PAS une panne, c'est le cas
  /// ordinaire d'un projet qui est juste un dossier. On ne dit donc rien et la barre ne
  /// s'affiche pas — le silence est ici volontaire, et c'est pour ca qu'il est ecrit.
  async function relireLesWorktrees() {
    if (!project?.path) {
      worktrees = [];
      return;
    }
    try {
      worktrees = await gitWorktrees(project.path);
    } catch (e) {
      // **CE CATCH NE COUVRE PLUS LE CAS ORDINAIRE.** Un projet qui n'est pas un depot git
      // rend desormais une liste VIDE cote backend ; ce qui arrive ici est une vraie panne
      // (dossier disparu, git absent), et elle se dit — sinon on cherche pendant vingt
      // minutes pourquoi une barre ne s'affiche pas.
      worktrees = [];
      signalerErreur("terminal.worktrees", String(e));
    }
    if (worktrees.length === 0) {
      worktreeActif = null;
      return;
    }
    // Le dossier affiche reste celui qu'on regardait s'il existe encore ; sinon on retombe
    // sur celui du terminal actif, et a defaut sur le principal.
    const connus = new Set(worktrees.map((w) => w.chemin));
    if (worktreeActif && connus.has(worktreeActif)) return;
    const duTerminal = activeId === null
      ? null
      : worktreeDe(sessions.find((s) => s.id === activeId)?.cwd, worktrees);
    worktreeActif = duTerminal ?? worktrees.find((w) => w.principal)?.chemin ?? worktrees[0].chemin;
  }

  /// Ouvre la liste des dossiers de travail sous le selecteur.
  ///
  /// **SOUS LE BOUTON, PAS SOUS LE POINTEUR** : un menu qui s'ouvre la ou le pointeur se
  /// trouvait donne l'impression d'un menu contextuel, alors que c'est une liste de choix.
  function ouvrirLeMenuDuWorktree(evenement: MouseEvent) {
    const bouton = (evenement.currentTarget as HTMLElement | null)?.getBoundingClientRect();
    menuWorktree = bouton
      ? { x: bouton.left, y: bouton.bottom + 4 }
      : { x: evenement.clientX, y: evenement.clientY };
  }

  /// Le dossier affiche, quand il y en a un. Capture avant d'entrer dans un menu.
  const groupeActif = $derived(groupes.find((g) => g.chemin === worktreeActif) ?? null);

  /// Le nom du dossier affiche, pour les textes qui le nomment.
  const libelleDuWorktreeActif = $derived(
    groupes.find((g) => g.chemin === worktreeActif)?.libelle ?? "",
  );

  /// Change de dossier de travail : les onglets, leurs volets et leur disposition suivent.
  async function choisirLeWorktree(chemin: string) {
    if (chemin === worktreeActif) return;
    // La disposition du dossier qu'on QUITTE est rangee avant de changer : sinon le dernier
    // agencement se perdrait au passage.
    enregistrerLaDisposition();
    worktreeActif = chemin;
    const groupe = groupes.find((g) => g.chemin === chemin);
    const premier = groupe?.terminaux[0] ?? null;
    disposition = await relireLaDisposition(groupe?.terminaux ?? []);
    if (premier !== null) {
      await activate(premier);
    } else {
      // Un dossier sans terminal : on n'en ouvre PAS un d'office. Ouvrir un shell parce que
      // l'utilisateur a clique sur une puce serait une action qu'il n'a pas demandee, et
      // elle survit a la fermeture de l'application.
      activeId = null;
      disposition = null;
    }
    enregistrerLaDisposition();
  }

  /// Ouvre un terminal dans un dossier de travail, en y basculant d'abord.
  async function ouvrirDansLeWorktree(groupe: Groupe) {
    await choisirLeWorktree(groupe.chemin);
    await addTerminal(undefined, groupe.chemin);
  }

  /// Cree une branche, son dossier de travail, et y ouvre un terminal.
  ///
  /// **UN SEUL GESTE POUR CE QUI EN DEMANDAIT SIX** : c'est tout l'interet. Le chemin complet
  /// est ANNONCE apres coup — rien ne doit apparaitre sur le disque de quelqu'un sans qu'il
  /// sache ou.
  async function creerUnWorktree() {
    if (!project?.path) return;
    const branche = await demanderTexte({
      message: $trad("worktree.creerTitre"),
      action: $trad("worktree.creerQuestion"),
      exemple: "feat/mon-sujet",
    });
    if (!branche?.trim()) return;
    try {
      const chemin = await gitWorktreeAdd(project.path, branche.trim(), true);
      await relireLesWorktrees();
      worktreeActif = chemin;
      disposition = null;
      activeId = null;
      notify($trad("worktree.cree", { chemin }), "success");
      await addTerminal(undefined, chemin);
    } catch (e) {
      notify(String(e));
    }
  }

  /// Retire un dossier de travail, et DIT ce que ca emporte avant de le faire.
  async function supprimerUnWorktree(groupe: Groupe) {
    if (!project?.path) return;
    if (groupe.principal) {
      notify($trad("worktree.supprimePrincipal"));
      return;
    }
    const ok = await demanderConfirmation({
      message: $trad("worktree.supprimerQuestion", {
        branche: groupe.libelle,
        chemin: groupe.chemin,
        n: groupe.terminaux.length,
      }),
      action: $trad("common.delete"),
      danger: true,
    });
    if (!ok) return;
    // Les terminaux partent AVANT le dossier : leur shell y a son dossier courant, et un
    // dossier efface sous un shell vivant laisse un processus dans le vide.
    for (const id of [...groupe.terminaux]) await closeTab(id);
    try {
      await gitWorktreeRemove(project.path, groupe.chemin, false);
    } catch (e) {
      notify(String(e));
    }
    worktreeActif = null;
    await relireLesWorktrees();
    if (worktreeActif) await choisirLeWorktree(worktreeActif);
  }

  /// Le volet qu'on est en train de deplacer, et l'endroit vise sous le pointeur.
  let voletDeplace: number | null = $state(null);
  let voletVise: { cible: number; cote: Cote } | null = $state(null);
  const plusieursVolets = $derived(nombreDeVolets(disposition) > 1);

  // Conversations passees de l'agent choisi dans les reglages. Le fournisseur vient du
  // magasin : ce composant ne sait pas lequel c'est, et n'a pas a le savoir.
  let agentOuvert = $state(false);
  let conversations: ConversationLlm[] = $state([]);
  let chargementConversations = $state(false);
  let renommeId: string | null = $state(null);
  let renommeValeur = $state("");

  const project = $derived($projects.find((p) => p.name === name));

  // Les instances xterm vivent dans le POOL persistant (script module ci-dessus) : elles
  // survivent au demontage. Ce Set trace uniquement les ids adoptes par CE montage.
  const mounted = new Set<number>();
  let unlisteners: Detacher[] = [];
  let resizeObserver: ResizeObserver | null = null;
  let fitTimer: ReturnType<typeof setTimeout> | null = null;

  // La file d'ecriture par terminal (enqueue/queueWrite) vit au niveau MODULE, avec le pool.
  const lastSentSize = new Map<number, string>();

  function queueResize(id: number, cols: number, rows: number) {
    const key = `${cols}x${rows}`;
    if (lastSentSize.get(id) === key) return;
    lastSentSize.set(id, key);
    enqueue(id, () => resizeTerminal(id, cols, rows));
  }

  // Frappe -> PTY. Certains accents (é, à) arrivent sous WebKitGTK dans un seul
  // evenement prefixe par espace + espace insecable (U+0020 U+00A0) : artefact
  // de composition GTK. On retire uniquement ce motif precis (un espace SUIVI
  // d'un insecable, ou un insecable seul) — jamais un espace normal isole.
  function sendInput(id: number, data: string) {
    // Voir TERMINAL_REPLY : une reponse du terminal n'est pas une frappe, elle ne doit pas
    // repartir dans le PTY. NE PAS RETIRER.
    if (TERMINAL_REPLY.test(data)) return;
    const clean = data.indexOf("\u00a0") === -1 ? data : data.replace(/\u0020?\u00a0/g, "");
    if (clean) queueWrite(id, clean);
  }

  /// REPONSES du terminal, a ne PAS renvoyer au PTY (NE PAS RETIRER).
  ///
  /// Un redessin du serveur contient des sequences auxquelles xterm REPOND (identification
  /// DA1 `ESC[c`, DA2 `ESC[>c`, position du curseur `ESC[6n`), et il repond par le MEME
  /// canal `onData` que les frappes. Ces reponses ne s'adressent pas au shell : renvoyees
  /// telles quelles, elles atterrissent dans l'invite (`1;2c0;276;0c` tape tout seul).
  /// L'emulateur du serveur repond deja pour de vrai a ce que le PROGRAMME demande.
  ///
  /// Diagnostique du temps de tmux (2026-08-13) PAR INSTRUMENTATION : le journal ne montrait
  /// aucun `resize` apres les `attach`, ce qui a elimine l'hypothese d'un repaint du a un
  /// changement de taille — hypothese qu'on aurait autrement "corrigee" a tort.
  ///
  /// Les evenements de focus (`ESC[I` / `ESC[O`) sont AUSSI filtres — revirement documente :
  /// c'est eux qui causaient un saut de ligne au changement de terminal (claude re-rendait
  /// son interface sur un simple blur+focus). Un changement d'onglet dans Cockpit n'est de
  /// toute facon pas une perte de focus du point de vue de l'utilisateur. Cout : les TUI ne
  /// peuvent plus attenuer leur bordure au blur.
  const TERMINAL_REPLY =
    /^(?:\x1b\[(?:\?[0-9;]*c|>[0-9;]*c|[0-9;]*R|[0-9;]*n|\?[0-9;]*\$y|[IO])|\x1bP[^\x1b]*\x1b\\|\x1b\][^\x07\x1b]*(?:\x07|\x1b\\))+$/;

  const XTERM_THEMES = {
    dark: { background: "#111318", foreground: "#d4d7dd", cursor: "#d4d7dd", selectionBackground: "#33415580" },
    light: { background: "#ffffff", foreground: "#24292f", cursor: "#24292f", selectionBackground: "#b6d7ff80" },
  };

  /// Chargement initial de ce montage. Une demande d'ouverture (voir `honorerDemande`)
  /// arrivee pendant ce chargement s'enchaine APRES lui : sans cela les deux ecrivent
  /// `sessions` en parallele et la derniere ecriture ecrase l'autre.
  let montage: Promise<unknown> = Promise.resolve();

  onMount(() => {
    // Les trois valeurs se consomment une fois, et les effets plus bas peuvent prendre la
    // main pendant nos `await` : on les lit AVANT.
    const demande = $pendingTerminalId;
    const commande = $pendingTerminalCommand;
    const restaure = consumeTabRestored();

    montage = (async () => {
      // La sortie et le message de fin sont geres par les listeners GLOBAUX du pool
      // (script module) : ici on ne suit que l'etat d'UI de ce montage.
      unlisteners.push(
        await ecouter<number>("terminal_exit", (e) => {
          const s = sessions.find((s) => s.id === e.payload);
          if (s) s.alive = false;
        })
      );

      // PAS DE FILTRE SUR `alive` ICI, et c'est ce qui rend les terminaux au retour.
      //
      // Un terminal dont le service n'a plus la session n'est pas perdu : son shell est mort
      // avec la machine, sa ligne a survecu, et l'activer rouvre un shell dans le meme
      // dossier avec l'ecran qu'il affichait (voir `restaurer`, cote Rust). Le filtre qui
      // vivait ici faisait disparaitre TOUS les onglets de terminal a chaque extinction du
      // poste. `alive` reste vrai a l'affichage : ce qui barre un onglet, c'est un shell qui
      // meurt sous les yeux de l'utilisateur (evenement `terminal_exit`), pas un terminal
      // qui dort.
      const existing = await listTerminals(name);
      sessions = existing.map((t) => ({ id: t.id, alive: true, name: t.name, cwd: t.cwd }));

      // Les dossiers de travail AVANT la disposition : c'est le dossier affiche qui decide
      // quelle disposition relire, et quels onglets montrer.
      await relireLesWorktrees();

      // **LA DISPOSITION SE RELIT AVANT TOUTE ACTIVATION, ET C'EST LA REGRESSION DE LA
      // 0.63.0.** Les deux lignes qui suivent SORTENT du montage quand elles ont fait leur
      // travail ; la relecture vivait apres elles, donc revenir sur l'onglet Terminal en
      // cliquant un terminal de la barre laterale (ou par la palette, le tableau de bord,
      // une commande rapide, un `docker exec`) activait une session sur une disposition
      // encore vide : `poserLaSession` repartait alors sur un volet unique et les volets
      // disparaissaient de l'ecran. Ils etaient intacts en base, jamais relus.
      if (sessions.length > 0) {
        // Les terminaux du dossier AFFICHE, pas tous : sinon la disposition garderait des
        // volets qui appartiennent a une autre branche, et `nettoyer` les laisserait passer.
        disposition = await relireLaDisposition(sessionsVisibles.map((s) => s.id));
      }

      // La commande d'abord : elle CREE un terminal, alors qu'une demande d'ouverture ne
      // fait qu'activer un terminal existant.
      if (commande !== null && (await honorerCommande(commande))) return;
      if (demande !== null && (await honorerDemande(demande))) return;
      // Une demande traitee entre-temps a deja ouvert un terminal : on ne lui passe pas
      // devant en activant autre chose.
      if (activeId !== null) return;
      // Onglet REPOSE par la memoire par projet (simple retour sur le projet) : on n'ouvre
      // rien d'office. Sinon parcourir trois projets laisses sur l'onglet Terminal creerait
      // trois shells que personne n'a demandes — et ils survivent a l'app.
      // L'etat vide et son bouton prennent le relais.
      if (sessions.length === 0) {
        if (!restaure) await addTerminal();
      } else {
        const premier = sessionsAffichees(disposition)[0] ?? sessionsVisibles[0]?.id ?? sessions[0].id;
        await activate(premier);
      }
      // Un echec de chargement laissait l'onglet vide sans un mot : la liste des terminaux
      // vient du backend, son absence doit se voir.
    })().catch((e) => notify(String(e)));

    // Debounce : pendant un drag de fenetre, on n'envoie que la taille finale
    resizeObserver = new ResizeObserver(() => {
      if (fitTimer) clearTimeout(fitTimer);
      fitTimer = setTimeout(() => fitLesVolets(), 80);
    });
    if (container) resizeObserver.observe(container);

    // Cible du glisser-deposer de fichiers (listener global, voir script module).
    if (container) {
      setDropTarget({
        el: container,
        activeId: () => activeId,
        over: (v) => (dropOver = v),
      });
    }

    return () => {
      setDropTarget(null);
      dropOver = false;
      resizeObserver?.disconnect();
      unlisteners.forEach((u) => u());
      // On quitte la vue des terminaux : le moment de photographier leur ecran, pour qu'ils
      // reviennent comme on les laisse. Le backend borne la frequence, donc un aller-retour
      // entre deux onglets ne coute rien.
      void saveTerminalScreens().catch((e) => signalerErreur("terminal.photo", String(e)));
      // NI detach, NI dispose : les xterm restent vivants dans le pool et le serveur
      // continue de leur envoyer la sortie. On gare simplement les elements DOM hors du
      // document visible (voir le commentaire du pool).
      parkAll();
      mounted.clear();
    };
  });

  // Raccourci vers un terminal du MEME projet (barre laterale, tableau de bord, palette,
  // commande rapide, `docker exec`) : le composant n'est pas remonte, donc on reagit au
  // magasin. `untrack` parce que la suite lit ET ecrit `sessions` : sans lui l'effet se
  // redeclencherait sur sa propre ecriture.
  $effect(() => {
    const wanted = $pendingTerminalId;
    if (wanted === null) return;
    untrack(() => {
      void montage.then(() => honorerDemande(wanted)).catch((e) => notify(String(e)));
    });
  });

  // Commande rapide / shell de conteneur demandee alors que l'onglet est DEJA monte :
  // meme enchainement que ci-dessus (apres le montage, hors suivi reactif).
  $effect(() => {
    const commande = $pendingTerminalCommand;
    if (commande === null) return;
    untrack(() => {
      void montage.then(() => honorerCommande(commande)).catch((e) => notify(String(e)));
    });
  });

  /// Recharge la liste des sessions du projet et la FUSIONNE avec celle affichee.
  /// Fusion et non remplacement : une session terminee reste visible (barree) jusqu'a ce
  /// que l'utilisateur ferme son onglet, un remplacement la ferait disparaitre sous ses yeux.
  ///
  /// Sans filtre sur `alive`, pour la meme raison qu'au montage : un terminal endormi est
  /// reclamable — la barre laterale et le tableau de bord le proposent — et l'activer rouvre
  /// son shell avec l'ecran d'avant.
  async function fusionnerSessions() {
    const frais = await listTerminals(name);
    for (const t of frais) {
      const connue = sessions.find((s) => s.id === t.id);
      if (connue) connue.name = t.name;
      else sessions.push({ id: t.id, alive: true, name: t.name, cwd: t.cwd });
    }
  }

  /// Ouvre le terminal reclame par `pendingTerminalId`. Rend vrai s'il a ete pris en charge.
  ///
  /// `sessions` est un INSTANTANE : il date du montage de l'onglet. Or la cible vient
  /// souvent d'etre creee a l'instant par une voie qui ne passe pas par ici — commande
  /// rapide, `docker exec` de l'onglet Docker, palette Ctrl+K appellent tous
  /// `create_terminal` puis posent l'id. Tester la seule liste locale rejetait donc
  /// exactement les cas pour lesquels ce magasin existe : la session etait bien creee
  /// (elle apparaissait dans la barre laterale) mais l'onglet n'affichait rien et l'id
  /// restait coince dans le magasin. D'ou : on RECHARGE avant de conclure.
  async function honorerDemande(wanted: number): Promise<boolean> {
    // Demande deja traitee, ou remplacee par une autre depuis : on ne la rejoue pas.
    if (get(pendingTerminalId) !== wanted) return false;

    if (!sessions.some((s) => s.id === wanted)) {
      try { await fusionnerSessions(); }
      catch (e) { notify(String(e)); return false; }
    }
    if (sessions.some((s) => s.id === wanted)) {
      pendingTerminalId.set(null);
      if (activeId !== wanted) await activate(wanted);
      return true;
    }

    // Toujours absente de ce projet : soit la session appartient a un AUTRE projet et son
    // onglet la prendra, soit elle n'existe plus. Dans ce dernier cas on le DIT et on vide
    // le magasin : un clic sans effet est vecu comme une panne, et un id jamais consomme
    // empoisonnerait les navigations suivantes.
    let ailleurs: boolean;
    try { ailleurs = (await listAllTerminals()).some((t) => t.id === wanted); }
    catch (e) { notify(String(e)); return false; }
    if (!ailleurs) {
      pendingTerminalId.set(null);
      notify($trad("term.sessionGone"));
    }
    return false;
  }

  /// Lance dans un NOUVEAU terminal la commande posee dans `pendingTerminalCommand`
  /// (bouton ▶ Cmd de l'en-tete, shell d'un conteneur, palette Ctrl+K). Rend vrai si elle
  /// a ete prise en charge.
  ///
  /// Le terminal est cree ICI, et pas chez l'appelant : `addTerminal` MESURE le conteneur
  /// avant d'ouvrir le PTY. Une TUI lancee par `init_command` (k9s, htop, top) se dessine a
  /// la taille du PTY et rien ne la redimensionne apres coup. Ces appelants creaient la
  /// session en 80x24 : la TUI restait dans un petit carre en haut a gauche d'un conteneur
  /// large (issue #14).
  async function honorerCommande(demande: { project: string; command: string; dossier?: string }): Promise<boolean> {
    // Demande deja traitee, ou remplacee par une autre depuis : on ne la rejoue pas.
    if (get(pendingTerminalCommand) !== demande) return false;
    // Vide AVANT de creer : une commande consommee ne doit pas pouvoir etre rejouee au
    // prochain passage sur l'onglet, meme si la creation echoue (addTerminal dit alors
    // pourquoi).
    pendingTerminalCommand.set(null);
    // Un autre projet : on ne lance rien chez celui-ci, et on le DIT — le magasin est
    // vide, donc personne ne se demandera plus tard pourquoi une commande a demarre.
    if (demande.project !== name) {
      notify($trad("term.commandOtherProject", { project: demande.project }));
      return false;
    }
    await addTerminal(demande.command, demande.dossier);
    return true;
  }

  // Suit le theme de l'app
  $effect(() => {
    const t = $themeBase;
    // Tout le pool suit le theme, y compris les terminaux gares d autres projets.
    pool.forEach(({ term }) => (term.options.theme = XTERM_THEMES[t]));
  });

  // --- Copier / Coller ---
  /// Copie la selection du terminal dans le presse-papier systeme. Source unique : le menu
  /// clic droit ET le Ctrl+C avec selection passent par ici.
  ///
  /// La selection appartient a xterm — c'est lui qui tient l'ecran et tout l'historique de
  /// defilement. Du temps de tmux elle appartenait au serveur (copy-mode) et il fallait la
  /// lui demander ; ce detour a disparu avec lui.
  async function copySelection() {
    const entry = activeId === null ? undefined : pool.get(activeId);
    if (!entry) {
      notify($trad("term.noTerminalOpen"));
      return;
    }
    const sel = entry.term.hasSelection() ? entry.term.getSelection() : "";
    // Rien de selectionne : on le DIT. Un « Copier » qui ne fait rien est vecu comme une
    // panne, et l'utilisateur ne sait pas que son geste de selection n'a pas pris.
    if (!sel) {
      notify($trad("term.nothingSelected"));
      entry.term.focus();
      return;
    }
    try {
      await setClipboard(sel);
      entry.term.clearSelection();
      notify($trad("term.copied"), "success");
    } catch (e) {
      signalerErreur("terminal.copie", String(e));
      notify(String(e));
    }
    entry.term.focus();
  }

  /// Colle le presse-papier SYSTEME dans le terminal actif. Source unique de tout collage
  /// Cockpit : clic droit -> « Coller » ET clic molette passent par ici, donc les deux
  /// collent exactement la meme chose.
  async function pasteClipboard() {
    const entry = activeId === null ? undefined : pool.get(activeId);
    if (!entry) {
      notify($trad("term.noTerminalOpen"));
      return;
    }
    try {
      const text = await getClipboard();
      // term.paste() passe par onData (bracketed paste) -> chemin d'entree normal
      if (text) entry.term.paste(text);
      else notify($trad("term.pasteEmpty"));
    } catch (e) {
      notify(String(e));
    }
    entry.term.focus();
  }

  function openCtxMenu(e: MouseEvent) {
    e.preventDefault();
    ctxMenu = { x: e.clientX, y: e.clientY };
  }

  function createXterm(): { term: Terminal; fit: FitAddon; el: HTMLDivElement } {
    const el = document.createElement("div");
    el.className = "term-host";
    container!.appendChild(el);
    const term = new Terminal({
      // Police explicite : le fallback "monospace" generique melange des glyphes
      // accentues venant d'autres polices -> derive visuelle.
      fontFamily: "'DejaVu Sans Mono', 'Liberation Mono', 'Noto Sans Mono', monospace",
      // Les paliers de zoom sont derives de cette valeur (ZOOM_LEVELS dans ui.ts) pour
      // que la police tombe toujours sur des pixels entiers : la changer ici suffit.
      fontSize: TERMINAL_FONT_SIZE,
      // 10 000 lignes : c'est LA molette. Le serveur en garde autant de son cote (il en
      // renvoie l'integralite a chaque redessin), le terminal doit pouvoir les tenir.
      scrollback: 10000,
      rescaleOverlappingGlyphs: true,
      // Surlignage de l'occurrence trouvee : `registerMarker` et `registerDecoration`
      // sont des API « proposees » d'xterm, refusees sans ce drapeau — et le refus est une
      // exception a l'appel, pas un retour vide (constate au banc, 2026-08-21 : la
      // recherche affichait « You must set the allowProposedApi option to true »).
      allowProposedApi: true,
      theme: XTERM_THEMES[$themeBase],
    });
    const fit = new FitAddon();
    term.loadAddon(fit);
    term.open(el);
    // Renderer WebGL : place chaque glyphe au pixel dans sa cellule.
    try {
      const webgl = new WebglAddon();
      webgl.onContextLoss(() => webgl.dispose());
      term.loadAddon(webgl);
    } catch {
      // WebGL indisponible : le renderer DOM reste utilisable
    }

    // Liens cliquables : Ctrl+clic (ou Cmd) ouvre l'URL dans le navigateur.
    // Le clic simple reste a la selection souris — pas de conflit.
    term.loadAddon(
      new WebLinksAddon((event, uri) => {
        if (event.ctrlKey || event.metaKey) {
          openUrl(uri).catch((e) => notify(String(e)));
        }
      })
    );

    // OSC 52 : un programme qui demande a poser du texte dans le presse-papier systeme.
    // Chemin de SORTIE uniquement (parser), aucune surcouche sur la frappe.
    term.parser.registerOscHandler(52, (data) => {
      const semi = data.indexOf(";");
      if (semi === -1) return true;
      const b64 = data.slice(semi + 1);
      if (!b64 || b64 === "?") return true; // "?" = demande de lecture, ignoree
      try {
        const texte = atob(b64);
        const bytes = new Uint8Array(texte.length);
        for (let i = 0; i < texte.length; i++) bytes[i] = texte.charCodeAt(i);
        const text = new TextDecoder().decode(bytes);
        if (text) setClipboard(text).catch(() => {});
      } catch {
        // base64 invalide : on avale la sequence sans rien copier
      }
      return true;
    });

    // Ctrl+C COPIE QUAND UNE SELECTION EST AFFICHEE, sinon il interrompt (SIGINT).
    // C'est le geste qu'on avait du temps de tmux, ou le copy-mode s'en chargeait. Ce
    // n'est PAS une surcouche sur le chemin de frappe : `onData` n'est pas touche, on
    // decide seulement, AVANT qu'xterm ne traduise la touche, de ne pas la lui donner.
    term.attachCustomKeyEventHandler((e) => {
      const copie =
        e.type === "keydown" && e.ctrlKey && !e.shiftKey && !e.altKey &&
        (e.key === "c" || e.key === "C") && term.hasSelection();
      if (!copie) return true;
      void copySelection();
      return false;
    });

    // FIX ESSENTIEL (accents) : sous WebKitGTK, le textarea cache d'xterm ne se
    // vide pas apres une composition (dead-key). Il accumule "è","èè","èèè"...
    // et xterm reenvoie tout le buffer a chaque frappe -> caracteres/espaces en
    // trop. On le vide apres chaque compositionend (au tick suivant, une fois
    // qu'xterm a lu la valeur). NE PAS RETIRER.
    const ta = el.querySelector(".xterm-helper-textarea") as HTMLTextAreaElement | null;
    if (ta) {
      ta.addEventListener("compositionend", () => {
        setTimeout(() => { ta.value = ""; }, 0);
      });
    }

    // FIX ESSENTIEL (clic molette) : le clic molette colle le presse-papier, exactement
    // comme « Coller » du menu clic droit — meme fonction, meme source. Deux
    // comportements se disputaient ce clic, il n'en reste qu'un :
    //  - le WebView colle, lui aussi, dans le textarea cache d'xterm : annule ici ;
    //  - notre propre collage, le seul qui reste.
    //
    // MESURE AU BANC WebKitGTK (clic milieu reel par XTEST, xterm 6.0.0 charge, 2026-08-20) :
    //  - `preventDefault` sur `mousedown` n'empeche PAS le collage natif, et le reglage GTK
    //    `gtk-enable-primary-paste` est ignore par WebKitGTK : il faut agir sur l'evenement
    //    `paste` lui-meme ;
    //  - `preventDefault` sur cet evenement NE SUFFIT PAS : xterm ne compte pas sur l'action
    //    par defaut du navigateur, il lit `clipboardData` lui-meme et injecte le texte dans
    //    le PTY (`handlePasteEvent` -> `triggerDataEvent`). Il pose ce handler sur le
    //    textarea ET sur `.xterm` pendant `term.open()`, donc AVANT nous : en phase cible,
    //    l'ordre est celui de l'inscription, et un `preventDefault` pose apres ne defait
    //    rien. C'est la cause du DEUXIEME collage — un clic molette, deux insertions ;
    //  - le collage natif du clic molette lit le presse-papier CLIPBOARD (pas la selection
    //    PRIMARY) : les deux collages portaient donc le MEME texte, invisible a l'oeil.
    // D'ou : ecoute en CAPTURE sur `el` (l'ancetre), qui passe avant tout ce qu'xterm a pose
    // plus bas, + `stopImmediatePropagation` pour qu'xterm ne voie jamais l'evenement, +
    // `preventDefault` pour que le texte n'atterrisse pas dans le textarea cache (d'ou il
    // ressortirait a la frappe suivante, cf. FIX ACCENTS ci-dessus). NE PAS RETIRER.
    //
    // Drapeau d'etat, PAS de fenetre de temps : l'ancienne version comparait `Date.now()` au
    // mousedown, ce qui melange la cause (« ce collage est celui du clic molette ») avec une
    // duree qu'aucune mesure ne garantit.
    let collageNatifAAnnuler = false;
    el.addEventListener(
      "mousedown",
      (e) => {
        collageNatifAAnnuler = (e as MouseEvent).button === 1;
        if (collageNatifAAnnuler) void pasteClipboard();
      },
      true,
    );
    // Filet : si le clic molette n'a produit aucun evenement `paste`, le drapeau ne doit pas
    // survivre jusqu'a un collage clavier ulterieur.
    el.addEventListener("keydown", () => { collageNatifAAnnuler = false; }, true);
    el.addEventListener(
      "paste",
      (e) => {
        if (!collageNatifAAnnuler) return;
        collageNatifAAnnuler = false;
        e.preventDefault();
        e.stopImmediatePropagation();
      },
      true,
    );

    return { term, fit, el };
  }

  async function addTerminal(initCommand?: string, dossier?: string) {
    // **UN TERMINAL NEUF S'OUVRE DANS LE DOSSIER QU'ON REGARDE.** Sans ca, le bouton « + »
    // ouvrirait a la racine du projet alors que la barre affiche une branche : on taperait
    // dans un autre dossier que celui qu'on croit.
    dossier = dossier ?? worktreeActif ?? undefined;
    // JAMAIS de retour silencieux ici : c'est exactement ce qui a laisse le premier
    // utilisateur externe cliquer sur + sans que rien ne se passe ni ne s'affiche.
    // Y COMPRIS pour le conteneur : c'est lui qui donne la taille du PTY, et une commande
    // rapide arrive par un magasin — un abandon muet ferait disparaitre la commande.
    if (!container) {
      notify($trad("term.viewNotReady"));
      return;
    }
    if (!project) {
      notify($trad("term.projectNotFound"));
      return;
    }
    if (!project.path) {
      notify($trad("term.noProjectPath"));
      return;
    }
    // On mesure AVANT de creer le PTY : le shell (et une TUI lancee via
    // init_command) demarre directement a la bonne taille, pas en 80x24.
    const entry = createXterm();
    mounted.forEach((tid) => { const e = pool.get(tid); if (e) e.el.style.display = "none"; });
    entry.el.style.display = "block";
    // `fit()` peut echouer sur un conteneur pas encore mesure : sans consequence, la
    // taille est renvoyee au prochain ResizeObserver. Silence VOULU, pas un oubli.
    try { entry.fit.fit(); } catch {}
    const cols = entry.term.cols || 80;
    const rows = entry.term.rows || 24;

    try {
      // `dossier` sert aux worktrees git : le shell demarre dans le dossier du worktree, pas
      // dans celui du projet. Absent = le projet, comme avant.
      //
      // Une commande VIDE vaut « pas de commande » : « ouvrir un terminal ici » depose une
      // demande sans commande, et taper une ligne vide dans le shell laisserait une invite
      // orpheline au demarrage.
      const id = await createTerminal(name, dossier || project.path, cols, rows, initCommand || undefined);
      pool.set(id, entry);
      mounted.add(id);
      lastSentSize.set(id, `${cols}x${rows}`);
      // Le nom par defaut (« PROJET - N ») est genere EN BASE a la creation : on le relit
      // pour que l'onglet affiche la meme chose que la sidebar, au lieu de pousser un nom
      // vide qui retombait sur le fallback « Terminal N ».
      const created = (await listTerminals(name)).find((t) => t.id === id);
      // Le dossier vient de la BASE et pas de la variable locale : c'est lui qui range le
      // terminal dans son dossier de travail, et il doit dire la meme chose des deux cotes.
      sessions.push({ id, alive: true, name: created?.name ?? "", cwd: created?.cwd ?? "" });
      try { await attachTerminal(id, cols, rows); }
      catch (e) { signalerErreur("terminal.attache", String(e)); }
      brancherEntree(entry, (data) => sendInput(id, data));
      activeId = id;
      entry.term.focus();
      loadTerminals();
    } catch (e) {
      entry.term.dispose();
      entry.el.remove();
      notify(String(e));
    }
  }

  /// Place chaque terminal affiche dans SON volet, et gare les autres.
  ///
  /// **RIEN N'EST RECREE, LES ELEMENTS SONT DEPLACES.** Un xterm recree repartirait vide et
  /// exigerait un redessin complet : clignotement, et retour en bas de l'historique a chaque
  /// aller-retour. C'est la meme raison qui fait vivre le pool au niveau du module.
  function disposerLesVolets() {
    const affiches = new Set(sessionsAffichees(disposition));
    mounted.forEach((tid) => {
      const e = pool.get(tid);
      if (!e) return;
      if (!affiches.has(tid)) {
        e.el.style.display = "none";
        return;
      }
      e.el.style.display = "block";
      const hote = hotes.get(tid);
      if (hote && e.el.parentElement !== hote) hote.appendChild(e.el);
    });
  }

  /// Le composant des volets confie (ou reprend) le conteneur d'un volet.
  function surVolet(id: number, element: HTMLDivElement | null) {
    if (element) hotes.set(id, element);
    else hotes.delete(id);
    // Le conteneur arrive APRES le premier rendu : on replace, puis on recalcule la taille.
    disposerLesVolets();
    planifierFit();
  }

  /// Montre `id` dans la disposition. Si la session n'y est pas encore, elle prend la place du
  /// volet actif : « ouvre ce terminal ICI » plutot que « ferme mes volets ».
  function showOnly(id: number) {
    disposition = poserLaSession(disposition, id, activeId);
    disposerLesVolets();
  }

  // --- Recherche dans le terminal, historique compris ---
  //
  // C'est le SERVEUR qui cherche : c'est lui qui tient la grille et son historique, et il
  // sait recoller une ligne trop longue coupee par la largeur du terminal (« --no-bundle »
  // a cheval sur deux rangees se trouve). Il n'a pas d'ecran a peindre : il rend OU se
  // trouve l'occurrence, et c'est ici qu'on defile et qu'on surligne.
  //
  // Aucune interception du chemin de frappe : la barre a son propre champ, et le seul
  // raccourci (Ctrl+Maj+F) est capte en phase capture sur window, AVANT xterm.
  let searchOpen = $state(false);
  let searchQuery = $state("");
  let searchStarted = $state(false);
  let searchTotal = $state(0);
  let searchIndex: number | null = $state(null);
  let searchInputEl: HTMLInputElement | undefined = $state();
  /// Le surlignage de l'occurrence courante. Un seul a la fois, jete avant le suivant.
  let surlignage: { dispose(): void } | null = null;

  function effacerSurlignage() {
    surlignage?.dispose();
    surlignage = null;
  }

  /// Amene l'occurrence a l'ecran et la surligne.
  ///
  /// `ligne` est l'indice de la grille du serveur : 0 est la premiere ligne VISIBLE, les
  /// valeurs negatives remontent dans l'historique. Le terminal, lui, numerote depuis le
  /// haut de son tampon — d'ou `baseY`. Les deux coincident parce qu'ils recoivent les
  /// memes octets et que le redessin du serveur renvoie tout son historique.
  function montrerOccurrence(entry: PoolEntry, ligne: number, colonne: number) {
    effacerSurlignage();
    const tampon = entry.term.buffer.active;
    const absolue = tampon.baseY + ligne;
    if (absolue < 0) return;
    // `registerMarker` compte depuis la ligne du curseur, et rend undefined sur l'ecran
    // alternatif (vim, htop) : la, il n'y a de toute facon pas d'historique a surligner.
    const marqueur = entry.term.registerMarker(ligne - tampon.cursorY);
    if (marqueur) {
      surlignage = entry.term.registerDecoration({
        marker: marqueur,
        x: colonne,
        width: Math.max(1, searchQuery.trim().length),
        backgroundColor: couleurSurlignage(),
        layer: "top",
      }) ?? null;
    }
    // Centree, pas collee en haut : on veut voir ce qu'il y a autour.
    entry.term.scrollToLine(Math.max(0, absolue - Math.floor(entry.term.rows / 2)));
  }

  /// La couleur d'accent de la palette active. Lue dans le theme plutot qu'ecrite ici :
  /// xterm veut une valeur, pas une variable CSS, mais le surlignage doit suivre la
  /// palette choisie par l'utilisateur.
  function couleurSurlignage(): string {
    const accent = getComputedStyle(document.documentElement).getPropertyValue("--accent").trim();
    return accent || "#4a9eff";
  }

  function openSearch() {
    // Sans terminal actif, il n'y a rien ou chercher. On le dit plutot que d'avaler le
    // clic (ou le Ctrl+Maj+F).
    if (activeId === null) {
      notify($trad("term.noTerminalOpen"));
      return;
    }
    searchOpen = true;
    requestAnimationFrame(() => { searchInputEl?.focus(); searchInputEl?.select(); });
  }

  async function appliquer(id: number, action: "start" | "next" | "prev") {
    const res = await terminalSearch(id, action, searchQuery.trim());
    // Des que le serveur a repondu, la recherche EST en cours : le compteur et les fleches
    // doivent apparaitre meme si le surlignage qui suit echoue.
    searchStarted = true;
    searchTotal = res.total;
    searchIndex = res.index;
    const entry = pool.get(id);
    if (entry && res.ligne !== null && res.colonne !== null) {
      montrerOccurrence(entry, res.ligne, res.colonne);
    } else {
      effacerSurlignage();
    }
    return res;
  }

  async function runSearch() {
    if (activeId === null || !searchQuery.trim()) return;
    const id = activeId;
    try {
      // Deja lancee sur ce motif : Entree passe a l'occurrence suivante, comme partout.
      const res = await appliquer(id, searchStarted ? "next" : "start");
      // Un resultat vide est une reponse, pas un silence.
      if (res.total === 0) notify($trad("term.searchNoMatch", { query: searchQuery.trim() }));
    } catch (e) { notify(String(e)); }
  }

  async function searchStep(dir: "next" | "prev") {
    if (activeId === null || !searchStarted) return;
    try { await appliquer(activeId, dir); } catch (e) { notify(String(e)); }
  }

  async function closeSearch(forId: number | null = activeId, refocus = true) {
    searchOpen = false;
    effacerSurlignage();
    searchTotal = 0;
    searchIndex = null;
    if (forId !== null && searchStarted) {
      searchStarted = false;
      try { await terminalSearch(forId, "cancel"); } catch { /* best effort */ }
    }
    if (refocus && activeId !== null) pool.get(activeId)?.term.focus();
  }

  function onSearchShortcut(e: KeyboardEvent) {
    if (!(e.ctrlKey || e.metaKey) || !e.shiftKey) return;
    const touche = e.key.toLowerCase();
    if (touche === "f") {
      e.preventDefault();
      e.stopPropagation();
      openSearch();
      return;
    }
    // D comme droite, B comme bas. **Capture avant xterm** : sans ca la combinaison part
    // dans le shell, qui n'en fait rien et l'avale.
    if (touche === "d" || touche === "b") {
      e.preventDefault();
      e.stopPropagation();
      void diviserLeVolet(touche === "d" ? "colonnes" : "lignes");
    }
  }

  onMount(() => {
    window.addEventListener("keydown", onSearchShortcut, { capture: true });
    return () => window.removeEventListener("keydown", onSearchShortcut, { capture: true });
  });

  /// Monte une session et la place dans son volet, SANS toucher au focus ni au volet actif.
  ///
  /// **CHAQUE VOLET AFFICHE DOIT PASSER PAR LA.** Un volet dont la session n'est pas branchee
  /// reste VIDE : le contenu d'un terminal n'arrive que par son attache. C'est ce qui manquait
  /// aux volets autres que l'actif, et ca se voyait a la relance : deux volets, un seul rempli.
  async function assurerMonte(id: number) {
    const existing = pool.get(id);
    if (!existing) {
      await attachExisting(id);
      return;
    }
    if (mounted.has(id)) return;
    // RE-ADOPTION : le xterm a survecu au demontage precedent, avec tout son ecran et son
    // historique — on remet simplement l'element en place. Rien n'est redemande au serveur,
    // donc rien ne clignote et le defilement ne bouge pas.
    container?.appendChild(existing.el);
    mounted.add(id);
    disposerLesVolets();
    try { existing.fit.fit(); } catch {}
    // Sans effet si le terminal est deja branche (le cas normal) ; s'il ne l'est plus
    // — service redemarre — cet appel le rebranche et le serveur renvoie un redessin.
    try { await attachTerminal(id, existing.term.cols || 80, existing.term.rows || 24); }
    catch (e) { signalerErreur("terminal.reattache", String(e)); }
  }

  /// Monte TOUS les volets affiches. Appelee des que la disposition change.
  async function assurerLesVolets() {
    for (const id of sessionsAffichees(disposition)) {
      await assurerMonte(id);
    }
    disposerLesVolets();
    planifierFit();
  }

  async function activate(id: number) {
    // Une recherche ouverte concerne l'ANCIEN terminal : on la clot chez lui
    if (searchOpen) await closeSearch(activeId, false);
    activeId = id;
    await assurerMonte(id);
    showOnly(id);
    // Les autres volets peuvent avoir besoin d'etre montes eux aussi (relecture d'une
    // disposition, session remplacee dans un volet).
    await assurerLesVolets();
    requestAnimationFrame(() => {
      fitLesVolets();
      pool.get(id)?.term.focus();
    });
  }

  async function attachExisting(id: number) {
    if (!container) {
      notify($trad("term.viewNotReady"));
      return;
    }
    const entry = createXterm();
    pool.set(id, entry);
    mounted.add(id);

    // Fit AVANT l'attach : le serveur aligne la session sur cette taille AVANT d'envoyer
    // son redessin, sinon le premier dessin arrive a l'ancienne taille et se recadre sous
    // les yeux de l'utilisateur. Le terminal doit donc deja etre DANS son volet, sinon il
    // est mesure a la taille du conteneur entier.
    disposerLesVolets();
    // Meme regle qu'au recalcul : une mesure prise sur un volet sans taille donnerait deux
    // colonnes, et le service alignerait la session dessus AVANT son redessin.
    if (mesurable(entry.el)) {
      try { entry.fit.fit(); } catch {}
    }
    const cols = Math.max(entry.term.cols || 80, COLONNES_MIN);
    const rows = Math.max(entry.term.rows || 24, LIGNES_MIN);
    lastSentSize.set(id, `${cols}x${rows}`);

    try {
      // L'attache ne rend RIEN : l'etat retrouve arrive par le meme canal que la suite
      // (evenement `terminal_output`), sous forme d'un redessin qui porte l'ecran ET les
      // 10 000 lignes d'historique. Une source unique, donc pas de course entre un
      // « replay » retourne et le flux vivant — c'est cette course qui dechirait
      // l'affichage au retour sur l'onglet.
      await attachTerminal(id, cols, rows);
    } catch (e) {
      // Session morte cote serveur : on retire l'onglet, mais on DIT pourquoi il disparait —
      // un onglet qui s'evapore sous les yeux de l'utilisateur ressemble a une panne.
      mounted.delete(id);
      disposePoolEntry(id);
      sessions = sessions.filter((s) => s.id !== id);
      signalerErreur("terminal.attacheExistant", String(e));
      notify($trad("term.sessionGone"), "error", 4000, { report: false });
      return;
    }

    brancherEntree(entry, (data) => sendInput(id, data));
  }

  /// Recalcule la taille de CHAQUE volet affiche.
  ///
  /// **UNE TUI SE DESSINE A LA TAILLE DU PTY ET PERSONNE NE LA REDIMENSIONNE APRES** : un
  /// volet dont on oublie la taille garde celle qu'il avait avant la division, et htop y
  /// deborde. `queueResize` ecarte deja les tailles identiques, donc appeler large ne coute
  /// rien.
  function fitLesVolets() {
    for (const id of sessionsAffichees(disposition)) {
      const entry = pool.get(id);
      if (!entry || entry.el.style.display === "none") continue;
      try {
        if (!mesurable(entry.el)) {
          // Le volet n'a pas encore de taille : mesurer ici donnerait deux colonnes, et on
          // les enverrait au PTY. On repasse plus tard.
          planifierFit();
          continue;
        }
        entry.fit.fit();
        if (entry.term.cols < COLONNES_MIN || entry.term.rows < LIGNES_MIN) {
          planifierFit();
          continue;
        }
        queueResize(id, entry.term.cols, entry.term.rows);
      } catch {}
    }
  }

  /// **UNE TAILLE MESUREE TROP TOT DENATURE LE TERMINAL POUR DE BON, ET C'EST ARRIVE APRES
  /// UNE MISE A JOUR (2026-09-11).** Au redemarrage, le fit tombait avant que le volet ait
  /// sa taille : le PTY etait alors mis a deux colonnes, et tout ce que le shell ou une TUI
  /// ecrivait ensuite s'empilait en une colonne de lettres. Rien ne le rattrape apres coup —
  /// « une TUI se dessine a la taille du PTY et personne ne la redimensionne apres ».
  /// Une mesure qui n'est pas plausible n'est donc jamais ENVOYEE : on repasse plus tard,
  /// le recalcul differe et l'observateur de taille s'en chargent.
  const COLONNES_MIN = 8;
  const LIGNES_MIN = 3;
  /// En dessous, le conteneur n'est pas encore dispose : ce n'est pas un petit volet, c'est
  /// un volet qui n'a pas de taille. Le plus etroit qu'on autorise vaut un dixieme de la
  /// largeur, soit bien plus que ca.
  const PIXELS_MIN = 60;

  function mesurable(element: HTMLElement): boolean {
    const r = element.getBoundingClientRect();
    return r.width >= PIXELS_MIN && r.height >= PIXELS_MIN / 2;
  }

  /// Le calcul de taille est DIFFERE : pendant un glissement de separateur ou de fenetre, on
  /// n'envoie que la taille finale. Meme raison que le debounce du redimensionnement.
  function planifierFit() {
    if (fitTimer) clearTimeout(fitTimer);
    fitTimer = setTimeout(() => fitLesVolets(), 80);
  }

  async function closeTab(id: number) {
    try { await closeTerminal(id); } catch (e) { notify(String(e)); }
    mounted.delete(id);
    disposePoolEntry(id);
    sessions = sessions.filter((s) => s.id !== id);
    // Le voisin prend toute la place : c'est ce a quoi on s'attend en fermant un volet.
    disposition = retirer(disposition, id);
    if (activeId === id) {
      const restants = sessionsAffichees(disposition);
      if (restants.length > 0) await activate(restants[restants.length - 1]);
      else if (sessions.length > 0) await activate(sessions[sessions.length - 1].id);
      else activeId = null;
    }
    disposerLesVolets();
    planifierFit();
    enregistrerLaDisposition();
    loadTerminals();
  }

  /// Divise le volet actif et ouvre un terminal NEUF a cote.
  ///
  /// La creation passe par `addTerminal`, seul endroit qui cree une session : il mesure son
  /// conteneur, et une session ouverte a une taille arbitraire garde cette taille pour ses
  /// TUI.
  async function diviserLeVolet(sens: "colonnes" | "lignes") {
    if (activeId === null) return;
    const cible = activeId;
    // La disposition d'AVANT est gardee ici : en creant le terminal, `addTerminal` l'affiche,
    // donc il prend la place du volet actif. On repart de l'etat d'avant pour diviser.
    const avant = disposition ?? feuille(cible);
    const connus = new Set(sessions.map((s) => s.id));
    await addTerminal();
    const nouveau = sessions.map((s) => s.id).find((id) => !connus.has(id));
    if (nouveau === undefined) return;
    disposition = diviser(avant, cible, sens, nouveau);
    disposerLesVolets();
    planifierFit();
    enregistrerLaDisposition();
  }

  /// Un separateur vient d'etre saisi : on suit le pointeur et on met le ratio a jour.
  ///
  /// **LA CAPTURE EST OBLIGATOIRE** : sans elle, un glissement qui passe au-dessus d'un
  /// terminal fait perdre les evenements au separateur, et le volet se fige a mi-chemin.
  function surSeparateur(evenement: PointerEvent, chemin: Chemin, sens: "colonnes" | "lignes") {
    const separateur = evenement.currentTarget as HTMLElement | null;
    const division = separateur?.parentElement;
    if (!division || !disposition) return;
    evenement.preventDefault();
    separateur?.setPointerCapture?.(evenement.pointerId);
    const cadre = division.getBoundingClientRect();
    const suivre = (e: PointerEvent) => {
      if (!disposition) return;
      const part =
        sens === "colonnes"
          ? (e.clientX - cadre.left) / cadre.width
          : (e.clientY - cadre.top) / cadre.height;
      disposition = fixerRatio(disposition, chemin, part);
      planifierFit();
    };
    const finir = () => {
      window.removeEventListener("pointermove", suivre);
      window.removeEventListener("pointerup", finir);
      planifierFit();
      enregistrerLaDisposition();
    };
    window.addEventListener("pointermove", suivre);
    window.addEventListener("pointerup", finir);
  }

  // --- Deplacer un volet ----------------------------------------------------------------
  //
  // **LE GESTE PART DE LA POIGNEE, PAS DU TERMINAL.** Saisir le terminal lui-meme prendrait
  // le geste a xterm, dont la selection de texte commence exactement pareil. La poignee est
  // l'etiquette du volet, qui ne sert a rien d'autre.

  /// En dessous, c'est un clic sur l'etiquette et pas un deplacement. Sans ce seuil, le
  /// moindre tremblement pendant un clic ferait sauter un volet.
  const SEUIL_GLISSEMENT = 4;

  /// Les cadres des volets affiches, sauf celui qu'on deplace. La geometrie s'arrete ici :
  /// ce qu'on en deduit vit dans `terminaux/visee.ts`, avec ses essais.
  function cadresDesVolets(sauf: number) {
    return sessionsAffichees(disposition).flatMap((id) => {
      if (id === sauf) return [];
      const hote = hotes.get(id);
      return hote ? [{ id, cadre: hote.getBoundingClientRect() }] : [];
    });
  }

  /// Quel volet est sous le pointeur, et de quel cote on y atterrirait.
  function viseSousLePointeur(x: number, y: number, source: number) {
    for (const { id, cadre } of cadresDesVolets(source)) {
      if (!dansLeCadre(cadre, x, y)) continue;
      return { cible: id, cote: coteVise(cadre, x, y) };
    }
    return null;
  }

  /// Saisie de la poignee d'un volet.
  ///
  /// **LA CAPTURE DU POINTEUR EST OBLIGATOIRE**, comme pour le separateur : sans elle, le
  /// geste se perd des qu'il passe au-dessus d'un terminal.
  function surPoignee(evenement: PointerEvent, id: number) {
    if (evenement.button !== 0) return;
    const poignee = evenement.currentTarget as HTMLElement | null;
    evenement.preventDefault();
    poignee?.setPointerCapture?.(evenement.pointerId);
    const departX = evenement.clientX;
    const departY = evenement.clientY;
    let parti = false;
    const suivre = (e: PointerEvent) => {
      if (!parti) {
        if (Math.hypot(e.clientX - departX, e.clientY - departY) < SEUIL_GLISSEMENT) return;
        parti = true;
        voletDeplace = id;
      }
      voletVise = viseSousLePointeur(e.clientX, e.clientY, id);
    };
    const finir = () => {
      window.removeEventListener("pointermove", suivre);
      window.removeEventListener("pointerup", finir);
      const vise = voletVise;
      voletDeplace = null;
      voletVise = null;
      // Un clic simple sur l'etiquette focalise le volet : le geste ne doit pas etre un
      // cul-de-sac quand on le relache sans avoir bouge.
      if (!parti) {
        focaliserLeVolet(id);
        return;
      }
      if (vise) deplacerLeVolet(id, vise.cible, vise.cote);
    };
    window.addEventListener("pointermove", suivre);
    window.addEventListener("pointerup", finir);
  }

  /// Pose le volet `source` contre `cible`. Source unique du deplacement : le glissement ET
  /// les entrees du menu contextuel passent par ici.
  function deplacerLeVolet(source: number, cible: number, cote: Cote) {
    if (!disposition) return;
    const avant = disposition;
    disposition = deplacer(disposition, source, cible, cote);
    // Rien n'a bouge (sur soi-meme, volet ferme entre-temps) : ne pas ecrire pour rien.
    if (disposition === avant) return;
    enregistrerLaDisposition();
    void assurerLesVolets();
  }

  /// Le voisin a viser quand le deplacement vient du MENU et non d'un geste : le volet
  /// affiche le plus proche dans cette direction, sur la ligne ou la colonne du volet visé.
  /// Rend `null` s'il n'y en a pas — l'entree de menu est alors cachee, pas inerte.
  function voisinDans(source: number, cote: Cote): number | null {
    const depart = hotes.get(source)?.getBoundingClientRect();
    if (!depart) return null;
    return voisinLePlusProche(depart, cadresDesVolets(source), cote);
  }

  /// Les entrees « Deplacer vers… » du clic droit.
  ///
  /// **UNE ENTREE SANS VOISIN DE CE COTE N'EXISTE PAS**, elle n'est pas grisee : un menu qui
  /// propose un geste sans effet envoie chercher une difference qui n'existe pas. Et la
  /// source est CAPTUREE ici, pas relue dans l'action : le menu se ferme avant qu'elle ne
  /// s'execute, et `activeId` aura change.
  function entreesDeDeplacement(): { label: string; action: () => void }[] {
    const source = activeId;
    if (source === null || !plusieursVolets) return [];
    const directions = [
      ["gauche", "term.ctxMoveLeft"],
      ["droite", "term.ctxMoveRight"],
      ["haut", "term.ctxMoveUp"],
      ["bas", "term.ctxMoveDown"],
    ] as const;
    return directions.flatMap(([cote, cle]) => {
      const voisin = voisinDans(source, cote);
      if (voisin === null) return [];
      return [{ label: $trad(cle), action: () => deplacerLeVolet(source, voisin, cote) }];
    });
  }

  /// Le clic dans un volet lui donne le focus : c'est lui que visent ensuite la frappe, le
  /// bouton de commande et le depot de fichier.
  function focaliserLeVolet(id: number) {
    if (activeId === id) return;
    void activate(id);
  }

  // --- Garder la disposition d'un lancement a l'autre ----------------------------------
  //
  // Elle vit dans les reglages, sous le nom du projet : les terminaux, eux, sont deja en base
  // et reviennent seuls. **Une disposition qui parle de sessions disparues est nettoyee a la
  // relecture** — sinon un volet resterait vide et ne se fermerait pas.
  /// **LA DISPOSITION EST RANGEE PAR DOSSIER DE TRAVAIL**, pour que chaque sujet retrouve
  /// son ecran. Le dossier PRINCIPAL garde la cle d'avant, sans suffixe : les dispositions
  /// deja enregistrees continuent donc d'etre relues au lieu d'etre perdues a la mise a jour.
  const cleDisposition = $derived.by(() => {
    const base = `terminaux.volets.${name}`;
    if (!worktreeActif) return base;
    const w = worktrees.find((x) => x.chemin === worktreeActif);
    return !w || w.principal ? base : `${base}#${worktreeActif}`;
  });

  function enregistrerLaDisposition() {
    const valeur = disposition ? JSON.stringify(disposition) : "";
    setAppSetting(cleDisposition, valeur).catch((e) =>
      signalerErreur("terminal.dispositionEcriture", String(e)),
    );
  }

  async function relireLaDisposition(vivantes: number[]): Promise<Noeud | null> {
    try {
      const reglages = await getAppSettings();
      return nettoyer(depuisJson(reglages[cleDisposition]), vivantes);
    } catch (e) {
      // Une disposition illisible ne doit jamais empecher d'ouvrir ses terminaux.
      signalerErreur("terminal.dispositionLecture", String(e));
      return null;
    }
  }

  // --- Renommage des onglets ---

  function startRename(s: { id: number; name: string }, index: number) {
    renamingId = s.id;
    renameValue = s.name || `Terminal ${index + 1}`;
  }

  async function commitRename() {
    const id = renamingId;
    renamingId = null;
    if (id === null) return;
    const value = renameValue.trim();
    const s = sessions.find((s) => s.id === id);
    if (!s) {
      notify($trad("term.sessionGone"));
      return;
    }
    // Sans ce message l'onglet affichait le nouveau nom alors que la base gardait l'ancien :
    // le nom revenait au retour sur le projet, sans explication. On remet aussi le nom
    // precedent, pour que l'onglet dise la verite.
    const avant = s.name;
    s.name = value;
    try { await renameTerminal(id, value); }
    catch (e) { s.name = avant; notify(String(e)); }
    loadTerminals();
  }

  function tabLabel(s: { name: string }, index: number): string {
    return s.name || `Terminal ${index + 1}`;
  }

  // --- Conversations passees de l'agent ---

  async function basculerAgent() {
    agentOuvert = !agentOuvert;
    renommeId = null;
    if (agentOuvert && project?.path) {
      chargementConversations = true;
      try {
        conversations = await conversationsLlm(project.path);
      } catch (e) {
        conversations = [];
        signalerErreur("terminal.conversationsAgent", String(e));
      } finally {
        chargementConversations = false;
      }
    }
  }

  function demarrerRenommage(conversation: ConversationLlm) {
    renommeId = conversation.id;
    renommeValeur = conversation.renamed ? conversation.label : "";
  }

  async function validerRenommage() {
    const id = renommeId;
    renommeId = null;
    if (id === null) return;
    try {
      await renommerConversationLlm(id, renommeValeur);
      if (project?.path) conversations = await conversationsLlm(project.path);
    } catch (e) {
      signalerErreur("terminal.renommageConversation", String(e));
    }
  }

  /// Ouvre un terminal neuf sur une conversation. **LA COMMANDE VIENT DU FOURNISSEUR** :
  /// `--resume` ne veut rien dire ailleurs que chez Claude Code, et un fournisseur qui reprend
  /// ses conversations autrement n'aurait rien a changer ici.
  async function reprendre(conversation: ConversationLlm) {
    agentOuvert = false;
    let commande: string | null = null;
    try {
      commande = (await commandesLlm(conversation.id)).reprise;
    } catch (e) {
      return signalerErreur("terminal.commandeAgent", String(e));
    }
    if (!commande) return;
    await addTerminal(commande);
    const active = sessions.find((s) => s.id === activeId);
    if (active) {
      const marque = $agentPrefere?.nom ?? "";
      active.name = `${marque} · ${conversation.label.slice(0, 24)}`;
      try { await renameTerminal(active.id, active.name); }
      catch (e) { signalerErreur("terminal.renommage", String(e)); }
      loadTerminals();
    }
  }

  async function nouvelleConversation() {
    agentOuvert = false;
    try {
      await addTerminal((await commandesLlm()).neuve);
    } catch (e) {
      signalerErreur("terminal.commandeAgent", String(e));
    }
  }

  function tempsRelatif(epochSecs: number): string {
    const diff = Math.floor(Date.now() / 1000) - epochSecs;
    if (diff < 3600) return $trad("time.minutesAgo", { n: Math.max(1, Math.floor(diff / 60)) });
    if (diff < 86400) return $trad("time.hoursAgo", { n: Math.floor(diff / 3600) });
    return $trad("time.daysAgoShort", { n: Math.floor(diff / 86400) });
  }
</script>

<div class="terminal-tab">
  <div class="term-tabs">
    {#if groupes.length > 0}
      <SelecteurWorktree
        libelle={libelleDuWorktreeActif}
        couleur={couleurActive}
        nombre={groupes.length}
        surOuvrir={ouvrirLeMenuDuWorktree}
      />
      <span class="term-separateur" aria-hidden="true"></span>
    {/if}
    {#each sessionsVisibles as s, i (s.id)}
      {#if renamingId === s.id}
        <!-- svelte-ignore a11y_autofocus -->
        <input
          class="term-rename"
          type="text"
          bind:value={renameValue}
          onblur={commitRename}
          onkeydown={(e) => { if (e.key === "Enter") commitRename(); if (e.key === "Escape") renamingId = null; }}
          autofocus
        />
      {:else}
        <button
          class="term-tab"
          class:active={activeId === s.id}
          class:affiche={plusieursVolets && voletsAffiches.includes(s.id)}
          class:dead={!s.alive}
          onclick={() => activate(s.id)}
          ondblclick={() => startRename(s, i)}
          oncontextmenu={(e) => { e.preventDefault(); startRename(s, i); }}
          title={$trad("term.renameHint")}
        >
          {tabLabel(s, i)}
          <span
            class="term-close"
            role="button"
            tabindex="-1"
            onclick={(e) => { e.stopPropagation(); closeTab(s.id); }}
            onkeydown={() => {}}
          >×</span>
        </button>
      {/if}
    {/each}
    <button class="term-add" onclick={() => addTerminal()} title={$trad("term.new")}>+</button>
    <!-- Diviser ouvre un terminal NEUF a cote de celui qui a le focus. Les deux boutons ne
         s'affichent que s'il y a un volet a diviser. -->
    {#if activeId !== null}
      <button
        class="term-split"
        onclick={() => diviserLeVolet("colonnes")}
        title={$trad("term.splitRight")}
        aria-label={$trad("term.splitRight")}
      >▥</button>
      <button
        class="term-split"
        onclick={() => diviserLeVolet("lignes")}
        title={$trad("term.splitDown")}
        aria-label={$trad("term.splitDown")}
      >▤</button>
    {/if}

    {#if searchOpen}
      <span class="term-search">
        <input
          class="term-search-input"
          bind:this={searchInputEl}
          bind:value={searchQuery}
          placeholder={$trad("term.searchPlaceholder")}
          oninput={() => { searchStarted = false; searchTotal = 0; searchIndex = null; }}
          onkeydown={(e) => {
            if (e.key === "Enter") { e.preventDefault(); runSearch(); }
            else if (e.key === "Escape") closeSearch();
          }}
        />
        <!-- Le compteur remplace le « n/N » que tmux affichait dans le coin du pane. -->
        {#if searchStarted}
          <span class="term-search-count">
            {searchTotal === 0
              ? $trad("term.searchNone")
              : `${(searchIndex ?? 0) + 1}/${searchTotal}`}
          </span>
        {/if}
        <button class="term-search-btn" onclick={() => searchStep("next")} title={$trad("term.searchOlder")} disabled={!searchStarted || searchTotal === 0}>↑</button>
        <button class="term-search-btn" onclick={() => searchStep("prev")} title={$trad("term.searchNewer")} disabled={!searchStarted || searchTotal === 0}>↓</button>
        <button class="term-search-btn" onclick={() => closeSearch()} title={$trad("term.searchClose")}>×</button>
      </span>
    {:else}
      <button class="term-search-btn" onclick={openSearch} title={$trad("term.searchHint")}>🔍</button>
    {/if}

    <!-- Le bouton n'existe QUE si le fournisseur choisi sait retrouver ses conversations :
         un bouton qui promet ce qu'il ne sait pas faire est un mensonge. Son libelle vient de
         lui — symbole et nom — donc aucun nom de produit n'est ecrit ici. -->
    {#if $agentPrefere?.conversations}
      <div class="agent-menu">
        <button class="term-agent" onclick={basculerAgent} title={$trad("term.agentMenuHint", { nom: $agentPrefere.nom })}>
          <span class="agent-symbole" style:color={$agentPrefere.couleur}>{$agentPrefere.symbole}</span>
          {$agentPrefere.nom} ▾
        </button>
        {#if agentOuvert}
          <div class="agent-dropdown">
            <button class="agent-item new" onclick={nouvelleConversation}>
              {$trad("term.agentNewSession")}
            </button>
            {#if chargementConversations}
              <div class="agent-item muted">{$trad("common.loading")}</div>
            {:else if conversations.length === 0}
              <div class="agent-item muted">{$trad("term.noPastConversation")}</div>
            {:else}
              {#each conversations as conversation (conversation.id)}
                {#if renommeId === conversation.id}
                  <!-- svelte-ignore a11y_autofocus -->
                  <input
                    class="agent-rename"
                    type="text"
                    bind:value={renommeValeur}
                    placeholder={$trad("term.sessionNamePlaceholder")}
                    onblur={validerRenommage}
                    onkeydown={(e) => {
                      if (e.key === "Enter") validerRenommage();
                      if (e.key === "Escape") renommeId = null;
                    }}
                    autofocus
                  />
                {:else}
                  <div class="agent-row">
                    <button class="agent-item" onclick={() => reprendre(conversation)} title={conversation.id}>
                      <span class="agent-label" class:renamed={conversation.renamed}>{conversation.label}</span>
                      <span class="agent-time">{tempsRelatif(conversation.updated_at)}</span>
                    </button>
                    <button
                      class="agent-edit"
                      title={$trad("term.renameSession")}
                      onclick={(e) => { e.stopPropagation(); demarrerRenommage(conversation); }}
                    >✎</button>
                  </div>
                {/if}
              {/each}
            {/if}
          </div>
        {/if}
      </div>
    {/if}
  </div>
  <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
  <div
    class="term-container"
    class:drop-over={dropOver}
    bind:this={container}
    role="application"
    oncontextmenu={openCtxMenu}
  >
    {#if sessionsVisibles.length === 0}
      <!-- **UN DOSSIER DE TRAVAIL SANS TERMINAL N'EST PAS UN ECRAN VIDE.** Sans ce bloc, y
           entrer donnait une grande zone noire et un « + » minuscule dans un coin : le geste
           menait a un cul-de-sac. On dit ou l'on est, et on propose la seule chose a faire. -->
      <div class="term-empty">
        <p>
          {worktreeActif && worktrees.length > 0
            ? $trad("worktree.vide", { branche: libelleDuWorktreeActif })
            : $trad("term.empty")}
        </p>
        <button class="btn" onclick={() => addTerminal()}>{$trad("term.openOne")}</button>
      </div>
    {:else if disposition}
      <!-- L'arbre ne rend que des conteneurs VIDES : c'est l'onglet qui y deplace les
           terminaux du pool, et c'est ce qui les garde intacts d'un volet a l'autre. -->
      <div class="volets">
        <VoletTerminal
          noeud={disposition}
          actif={activeId}
          {surVolet}
          surClic={focaliserLeVolet}
          {surSeparateur}
          libelle={(id) => {
            const s = sessions.find((s) => s.id === id);
            const index = sessions.findIndex((s) => s.id === id);
            return s ? tabLabel(s, index) : "";
          }}
          seul={!plusieursVolets}
          {surPoignee}
          deplace={voletDeplace}
          vise={voletVise}
          couleur={couleurActive}
        />
      </div>
    {/if}
    {#if dropOver}
      <div class="drop-hint">{$trad("term.dropHint")}</div>
    {/if}
  </div>
</div>

{#if menuWorktree}
  <!-- La liste des dossiers de travail. **CHAQUE ENTREE EST CAPTUREE EN PARAMETRE** : le menu
       se ferme avant que l'action ne s'execute, et relire l'etat a ce moment donnerait null. -->
  <ContextMenu
    x={menuWorktree.x}
    y={menuWorktree.y}
    items={[
      { section: $trad("worktree.barreTitre") },
      ...groupes.map((g) => ({
        label: g.libelle,
        couleur: couleursWorktree.get(g.chemin),
        courant: g.chemin === worktreeActif,
        suffixe: String(g.terminaux.length),
        action: () => void choisirLeWorktree(g.chemin),
      })),
      { section: $trad("worktree.sectionActions") },
      { label: $trad("worktree.creerTitre"), action: () => void creerUnWorktree() },
      ...(groupeActif
        ? [{ label: $trad("worktree.ouvrirTerminal"), action: () => void ouvrirDansLeWorktree(groupeActif) }]
        : []),
      ...(groupeActif && !groupeActif.principal
        ? [{
            label: $trad("worktree.supprimer"),
            danger: true,
            action: () => void supprimerUnWorktree(groupeActif),
          }]
        : []),
    ]}
    onClose={() => (menuWorktree = null)}
  />
{/if}

{#if ctxMenu}
  <ContextMenu
    x={ctxMenu.x}
    y={ctxMenu.y}
    items={[
      // **RANGE PAR SUJET, PARCE QUE CE MENU S'ALLONGE.** Une liste plate melangeait deja le
      // presse-papiers, les volets et le terminal lui-meme.
      { section: $trad("term.ctxSectionClipboard") },
      { label: $trad("common.copy"), action: copySelection },
      { label: $trad("common.paste"), action: pasteClipboard },
      // C'est ici qu'on cherche la division : les boutons de la barre d'onglets restent,
      // pour ceux qui les ont vus, mais le clic droit est le geste naturel.
      { section: $trad("term.ctxSectionPanes") },
      { label: $trad("term.ctxSplitRight"), action: () => void diviserLeVolet("colonnes") },
      { label: $trad("term.ctxSplitDown"), action: () => void diviserLeVolet("lignes") },
      ...entreesDeDeplacement(),
      { section: $trad("term.ctxSectionTerminal") },
      // La cible est CAPTUREE : le menu se ferme avant que l'action ne s'execute, et
      // `activeId` aura change. Meme piege que pour les deplacements.
      ...(activeId === null
        ? []
        : [{ label: $trad("term.ctxClose"), danger: true, cible: activeId }].map((e) => ({
            label: e.label,
            danger: e.danger,
            action: () => void closeTab(e.cible),
          }))),
    ]}
    onClose={() => (ctxMenu = null)}
  />
{/if}

<style>
  .terminal-tab { display: flex; flex-direction: column; height: 100%; min-height: 0; }
  .term-tabs {
    display: flex; gap: 0.25rem; align-items: center;
    padding-bottom: 0.4rem; flex-wrap: wrap;
  }
  .term-tab {
    display: inline-flex; align-items: center; gap: 0.4rem;
    padding: 0.2rem 0.6rem; font-size: 0.8rem; cursor: pointer;
    border: 1px solid var(--border-color); border-radius: 4px;
    background: var(--bg-secondary); color: var(--text-secondary);
    max-width: 220px; overflow: hidden; white-space: nowrap;
  }
  .term-tab.active { color: var(--accent); border-color: var(--accent); }
  .term-tab.dead { opacity: 0.5; text-decoration: line-through; }
  .term-rename {
    font-size: 0.8rem; padding: 0.2rem 0.4rem; width: 140px;
    border: 1px solid var(--accent); border-radius: 4px;
    background: var(--bg-primary); color: var(--text-primary); outline: none;
  }
  .term-close { opacity: 0.6; padding: 0 0.1rem; }
  .term-close:hover { opacity: 1; color: var(--error, #e5484d); }
  .term-add {
    padding: 0.2rem 0.55rem; font-size: 0.85rem; cursor: pointer;
    border: 1px solid var(--border-color); border-radius: 4px;
    background: var(--bg-secondary); color: var(--text-secondary);
  }
  .term-search { display: inline-flex; align-items: center; gap: 0.25rem; }
  .term-search-input {
    width: 15rem; font-size: 0.78rem; padding: 0.2rem 0.45rem;
    border: 1px solid var(--border-color); border-radius: 4px;
    background: var(--bg-primary); color: var(--text-primary);
  }
  .term-search-count {
    font-size: 0.72rem; color: var(--text-muted); font-variant-numeric: tabular-nums;
    min-width: 3.5rem; text-align: center;
  }
  .term-search-btn {
    padding: 0.2rem 0.4rem; font-size: 0.8rem; cursor: pointer;
    border: 1px solid var(--border-color); border-radius: 4px;
    background: var(--bg-secondary); color: var(--text-secondary);
  }
  .term-search-btn:hover:not(:disabled) { color: var(--text-primary); border-color: var(--accent); }
  .term-search-btn:disabled { opacity: 0.45; cursor: default; }
  .term-add:hover { color: var(--accent); border-color: var(--accent); }

  .agent-menu { position: relative; margin-left: auto; }
  .term-agent {
    padding: 0.2rem 0.6rem; font-size: 0.8rem; cursor: pointer;
    border: 1px solid var(--border-color); border-radius: 4px;
    background: var(--bg-secondary); color: var(--text-secondary);
  }
  .term-agent:hover { color: var(--accent); border-color: var(--accent); }
  /* La couleur vient du fournisseur, posee en style en ligne : elle lui appartient et n'a
     donc pas de token. Le reste du bouton suit le theme. */
  .agent-symbole { font-weight: 700; }
  .agent-dropdown {
    position: absolute; right: 0; top: calc(100% + 4px); z-index: 20;
    width: 380px; max-height: 320px; overflow-y: auto;
    background: var(--bg-secondary); border: 1px solid var(--border-color);
    border-radius: 6px; box-shadow: 0 6px 20px rgba(0, 0, 0, 0.25);
    padding: 0.25rem;
  }
  .agent-item {
    display: flex; justify-content: space-between; align-items: baseline; gap: 0.6rem;
    width: 100%; padding: 0.35rem 0.5rem; font-size: 0.78rem;
    background: none; border: none; color: var(--text-secondary);
    cursor: pointer; text-align: left; border-radius: 4px;
  }
  .agent-item:hover { background: var(--bg-tertiary); color: var(--text-primary); }
  .agent-item.muted { color: var(--text-muted); cursor: default; }
  .agent-item.new { color: var(--accent); font-weight: 600; }
  .agent-label {
    flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap;
  }
  .agent-label.renamed { font-weight: 600; color: var(--text-primary); }
  .agent-time { flex-shrink: 0; color: var(--text-muted); font-size: 0.7rem; }
  .agent-row { display: flex; align-items: center; }
  .agent-row .agent-item { flex: 1; min-width: 0; }
  .agent-edit {
    flex-shrink: 0; background: none; border: none; cursor: pointer;
    color: var(--text-muted); font-size: 0.75rem; padding: 0 0.4rem;
    opacity: 0; transition: opacity 0.12s;
  }
  .agent-row:hover .agent-edit { opacity: 1; }
  .agent-edit:hover { color: var(--accent); }
  .agent-rename {
    width: calc(100% - 0.5rem); margin: 0.15rem 0.25rem;
    padding: 0.3rem 0.5rem; font-size: 0.78rem; font-family: monospace;
    border: 1px solid var(--accent); border-radius: 4px;
    background: var(--bg-primary); color: var(--text-primary); outline: none;
  }

  .term-container {
    flex: 1; min-height: 0; position: relative;
    border: 1px solid var(--border-color); border-radius: 6px;
    overflow: hidden; padding: 4px; background: #111318;
  }
  :global(html:not(.dark)) .term-container { background: #ffffff; }
  /* Le terminal reste OPAQUE meme avec une image de fond, et ne recoit aucun flou.
     xterm dessine dans un canvas WebGL : le rendre translucide est un terrain a
     regressions d'affichage (voir "Pieges connus" du CLAUDE.md), et un terminal doit
     rester lisible avant d'etre joli. Les couleurs viennent de XTERM_THEMES. */
  :global(html.has-wallpaper) .term-container { background: #111318; backdrop-filter: none; }
  :global(html.has-wallpaper:not(.dark)) .term-container { background: #ffffff; }
  .term-container :global(.term-host) { width: 100%; height: 100%; }
  /* L'arbre des volets occupe toute la zone ; chaque volet accueille un terminal du pool. */
  .volets { width: 100%; height: 100%; min-width: 0; min-height: 0; }
  .term-split {
    padding: 0 0.35rem;
    background: none;
    border: none;
    color: var(--text-muted);
    cursor: pointer;
    font-size: 0.95rem;
    line-height: 1;
  }
  .term-split:hover { color: var(--text-primary); }
  /* Un onglet AFFICHE dans un volet, sans avoir le focus : un point suffit a le dire, sans
     ajouter une seconde couleur qui entrerait en concurrence avec l'onglet actif. */
  .term-tab.affiche:not(.active)::before {
    content: "•";
    margin-right: 0.3rem;
    color: var(--accent);
  }
  /* Depot de fichier en cours : la cible doit etre evidente pendant le survol. */
  .term-container.drop-over { border-color: var(--accent); }
  .drop-hint {
    position: absolute; left: 50%; bottom: 1rem; transform: translateX(-50%);
    padding: 0.35rem 0.7rem; font-size: 0.8rem; pointer-events: none;
    border: 1px solid var(--accent); border-radius: 6px;
    background: var(--surface-raised); color: var(--text-primary);
    box-shadow: 0 4px 14px rgba(0, 0, 0, 0.3);
  }
  .term-empty {
    display: flex; flex-direction: column; gap: 0.75rem;
    align-items: center; justify-content: center; height: 100%;
    color: var(--text-muted); font-size: 0.85rem;
  }
  .term-empty p { margin: 0; }
</style>
