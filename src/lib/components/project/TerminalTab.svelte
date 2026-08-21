<script lang="ts" module>
  import { listen as listenGlobal } from "@tauri-apps/api/event";
  import { getCurrentWebview } from "@tauri-apps/api/webview";
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
  listenGlobal<{ id: number; data: string }>("terminal_output", (e) => {
    pool.get(e.payload.id)?.term.write(b64ToBytes(e.payload.data));
  });
  listenGlobal<number>("terminal_exit", (e) => {
    pool.get(e.payload)?.term.write("\r\n\x1b[2m[processus terminé]\x1b[0m\r\n");
  });

  // File d'ecriture/resize PAR TERMINAL : chaque invoke part apres le retour du precedent.
  // Sans ca, des invoke rapproches peuvent s'executer dans le desordre cote Tauri -> octets
  // melanges dans le PTY. Au niveau MODULE comme le pool : un terminal survit au demontage,
  // sa file doit vivre aussi longtemps que lui — et le depot de fichiers (plus bas) doit
  // emprunter la MEME file que la frappe, sinon deux chemins d'ecriture s'entrelacent.
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
  /// (`dragDropEnabled`, actif par defaut) et l'expose comme un evenement applicatif. Les
  /// handlers HTML5 `ondrop` ne voient donc rien passer, et personne n'ecoutait cet
  /// evenement — le fichier lache sur le terminal disparaissait dans le vide.
  ///
  /// C'est aussi le SEUL canal qui porte le CHEMIN du fichier : `DataTransfer` ne l'expose
  /// plus depuis Tauri v2. Or le chemin est precisement ce qu'on veut ecrire — un shell le
  /// consomme tel quel, et Claude Code lit l'image qu'il designe.
  ///
  /// Une seule inscription pour la vie de l'app (le listener est global) ; le montage actif
  /// de TerminalTab declare sa cible ci-dessous.
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

  /// La position d'un evenement de depot est PHYSIQUE. Les coordonnees CSS s'en deduisent en
  /// divisant par devicePixelRatio : mesure faite dans le WebKitGTK systeme, il suit
  /// exactement le zoom de la page (zoom 1.15 -> dpr 1.15, zoom 2 -> dpr 2) en plus de la
  /// densite de l'ecran. Sans cette division, un depot serait mal route des que le zoom
  /// global de Cockpit n'est pas a 100 %.
  function overTerminal(pos: { x: number; y: number }): boolean {
    if (!dropTarget) return false;
    const ratio = window.devicePixelRatio || 1;
    const el = document.elementFromPoint(pos.x / ratio, pos.y / ratio);
    return !!el && dropTarget.el.contains(el);
  }

  /// Le chemin part dans un PTY : il sera relu par un shell. On echappe a la maniere d'un
  /// terminal natif (antislash devant les caracteres interpretes) plutot qu'en entourant de
  /// guillemets, qui genent la detection du chemin par les agents type Claude Code. Un chemin
  /// ordinaire, sans caractere special, ressort donc intact.
  function escapeForShell(path: string): string {
    return path.replace(/[ \t\n"'`$&|;<>()!*?[\]\\#]/g, (c) => "\\" + c);
  }

  getCurrentWebview()
    .onDragDropEvent((event) => {
      const p = event.payload;
      if (p.type === "leave") {
        dropTarget?.over(false);
        return;
      }
      if (p.type === "enter" || p.type === "over") {
        dropTarget?.over(overTerminal(p.position));
        return;
      }
      // p.type === "drop"
      dropTarget?.over(false);
      if (p.paths.length === 0) return;
      // Un depot est un geste delibere : s'il n'aboutit pas, on dit pourquoi plutot que de
      // ne rien faire (un silence, c'est un bug).
      if (!overTerminal(p.position)) {
        notifyGlobal(translate("term.dropOnTerminal"));
        return;
      }
      const id = dropTarget?.activeId() ?? null;
      if (id === null) {
        notifyGlobal(translate("term.noTerminalOpen"));
        return;
      }
      queueWrite(id, p.paths.map(escapeForShell).join(" ") + " ");
      pool.get(id)?.term.focus();
    })
    .catch((e) => notifyGlobal(`Glisser-déposer indisponible : ${e}`));
</script>

<script lang="ts">
  import { onMount, untrack } from "svelte";
  import { get } from "svelte/store";
  import { listen, type UnlistenFn } from "@tauri-apps/api/event";
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
    listClaudeSessions, renameClaudeSession, setClipboard, getClipboard,
    terminalSearch, openUrl,
  } from "../../api/workspace";
  import { notify } from "../../stores/toast";
  import ContextMenu from "../ui/ContextMenu.svelte";
  import type { ClaudeSession } from "../../types";

  let { name }: { name: string } = $props();

  let sessions: { id: number; alive: boolean; name: string }[] = $state([]);
  let activeId: number | null = $state(null);
  let container: HTMLDivElement | undefined = $state(undefined);
  // Menu contextuel Copier/Coller du terminal
  let ctxMenu: { x: number; y: number } | null = $state(null);
  let renamingId: number | null = $state(null);
  let renameValue = $state("");
  // Un fichier survole le terminal : on l'annonce, sinon on ne sait pas que le geste est permis.
  let dropOver = $state(false);

  // Sessions Claude Code
  let claudeOpen = $state(false);
  let claudeSessions: ClaudeSession[] = $state([]);
  let claudeLoading = $state(false);
  let renamingClaudeId: string | null = $state(null);
  let renameClaudeValue = $state("");

  const project = $derived($projects.find((p) => p.name === name));

  // Les instances xterm vivent dans le POOL persistant (script module ci-dessus) : elles
  // survivent au demontage. Ce Set trace uniquement les ids adoptes par CE montage.
  const mounted = new Set<number>();
  let unlisteners: UnlistenFn[] = [];
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
        await listen<number>("terminal_exit", (e) => {
          const s = sessions.find((s) => s.id === e.payload);
          if (s) s.alive = false;
        })
      );

      const existing = (await listTerminals(name)).filter((t) => t.alive);
      sessions = existing.map((t) => ({ id: t.id, alive: t.alive, name: t.name }));

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
        await activate(sessions[0].id);
      }
      // Un echec de chargement laissait l'onglet vide sans un mot : la liste des terminaux
      // vient du backend, son absence doit se voir.
    })().catch((e) => notify(String(e)));

    // Debounce : pendant un drag de fenetre, on n'envoie que la taille finale
    resizeObserver = new ResizeObserver(() => {
      if (fitTimer) clearTimeout(fitTimer);
      fitTimer = setTimeout(() => fitActive(), 80);
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
  async function fusionnerSessions() {
    const frais = (await listTerminals(name)).filter((t) => t.alive);
    for (const t of frais) {
      const connue = sessions.find((s) => s.id === t.id);
      if (connue) connue.name = t.name;
      else sessions.push({ id: t.id, alive: true, name: t.name });
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
        const bytes = Uint8Array.from(atob(b64), (c) => c.charCodeAt(0));
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
      sessions.push({ id, alive: true, name: created?.name ?? "" });
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

  function showOnly(id: number) {
    mounted.forEach((tid) => {
      const e = pool.get(tid);
      if (e) e.el.style.display = tid === id ? "block" : "none";
    });
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
    if ((e.ctrlKey || e.metaKey) && e.shiftKey && (e.key === "F" || e.key === "f")) {
      e.preventDefault();
      e.stopPropagation();
      openSearch();
    }
  }

  onMount(() => {
    window.addEventListener("keydown", onSearchShortcut, { capture: true });
    return () => window.removeEventListener("keydown", onSearchShortcut, { capture: true });
  });

  async function activate(id: number) {
    // Une recherche ouverte concerne l'ANCIEN terminal : on la clot chez lui
    if (searchOpen) await closeSearch(activeId, false);
    activeId = id;
    const existing = pool.get(id);
    if (existing && !mounted.has(id)) {
      // RE-ADOPTION : le xterm a survecu au demontage precedent, avec tout son ecran et
      // son historique — on remet simplement l'element dans le conteneur. Rien n'est
      // redemande au serveur, donc rien ne clignote et le defilement ne bouge pas.
      container?.appendChild(existing.el);
      mounted.add(id);
      // Sans effet si le terminal est deja branche (le cas normal) ; s'il ne l'est plus
      // — service redemarre — cet appel le rebranche et le serveur renvoie un redessin.
      try { existing.fit.fit(); } catch {}
      try { await attachTerminal(id, existing.term.cols || 80, existing.term.rows || 24); }
      catch (e) { signalerErreur("terminal.reattache", String(e)); }
    } else if (!existing) {
      await attachExisting(id);
    }
    showOnly(id);
    requestAnimationFrame(() => {
      fitActive();
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
    // les yeux de l'utilisateur.
    showOnly(id);
    try { entry.fit.fit(); } catch {}
    const cols = entry.term.cols || 80;
    const rows = entry.term.rows || 24;
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

  function fitActive() {
    if (activeId === null) return;
    const entry = pool.get(activeId);
    if (!entry || entry.el.style.display === "none") return;
    try {
      entry.fit.fit();
      queueResize(activeId, entry.term.cols, entry.term.rows);
    } catch {}
  }

  async function closeTab(id: number) {
    try { await closeTerminal(id); } catch (e) { notify(String(e)); }
    mounted.delete(id);
    disposePoolEntry(id);
    sessions = sessions.filter((s) => s.id !== id);
    if (activeId === id) {
      if (sessions.length > 0) await activate(sessions[sessions.length - 1].id);
      else activeId = null;
    }
    loadTerminals();
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

  // --- Sessions Claude ---

  async function toggleClaude() {
    claudeOpen = !claudeOpen;
    renamingClaudeId = null;
    if (claudeOpen && project?.path) {
      claudeLoading = true;
      try { claudeSessions = await listClaudeSessions(project.path); }
      catch (e) { claudeSessions = []; signalerErreur("terminal.sessionsClaude", String(e)); }
      finally { claudeLoading = false; }
    }
  }

  function startRenameClaude(cs: ClaudeSession) {
    renamingClaudeId = cs.id;
    renameClaudeValue = cs.renamed ? cs.label : "";
  }

  async function commitRenameClaude() {
    const id = renamingClaudeId;
    renamingClaudeId = null;
    if (id === null) return;
    try {
      await renameClaudeSession(id, renameClaudeValue);
      if (project?.path) claudeSessions = await listClaudeSessions(project.path);
    } catch (e) {
      signalerErreur("terminal.renommageSessionClaude", String(e));
    }
  }

  async function resumeClaude(session: ClaudeSession) {
    claudeOpen = false;
    await addTerminal(`claude --resume ${session.id}`);
    const active = sessions.find((s) => s.id === activeId);
    if (active) {
      active.name = `Claude · ${session.label.slice(0, 24)}`;
      try { await renameTerminal(active.id, active.name); }
      catch (e) { signalerErreur("terminal.renommage", String(e)); }
      loadTerminals();
    }
  }

  function relativeTime(epochSecs: number): string {
    const diff = Math.floor(Date.now() / 1000) - epochSecs;
    if (diff < 3600) return `il y a ${Math.max(1, Math.floor(diff / 60))} min`;
    if (diff < 86400) return `il y a ${Math.floor(diff / 3600)} h`;
    return `il y a ${Math.floor(diff / 86400)} j`;
  }
</script>

<div class="terminal-tab">
  <div class="term-tabs">
    {#each sessions as s, i (s.id)}
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

    <div class="claude-menu">
      <button class="term-claude" onclick={toggleClaude} title={$trad("term.claudeMenuHint")}>
        {$trad("term.claudeMenu")}
      </button>
      {#if claudeOpen}
        <div class="claude-dropdown">
          <button class="claude-item new" onclick={() => { claudeOpen = false; addTerminal("claude"); }}>
            {$trad("term.claudeNewSession")}
          </button>
          {#if claudeLoading}
            <div class="claude-item muted">{$trad("common.loading")}</div>
          {:else if claudeSessions.length === 0}
            <div class="claude-item muted">{$trad("term.noPastConversation")}</div>
          {:else}
            {#each claudeSessions as cs (cs.id)}
              {#if renamingClaudeId === cs.id}
                <!-- svelte-ignore a11y_autofocus -->
                <input
                  class="claude-rename"
                  type="text"
                  bind:value={renameClaudeValue}
                  placeholder={$trad("term.sessionNamePlaceholder")}
                  onblur={commitRenameClaude}
                  onkeydown={(e) => {
                    if (e.key === "Enter") commitRenameClaude();
                    if (e.key === "Escape") renamingClaudeId = null;
                  }}
                  autofocus
                />
              {:else}
                <div class="claude-row">
                  <button class="claude-item" onclick={() => resumeClaude(cs)} title={cs.id}>
                    <span class="claude-label" class:renamed={cs.renamed}>{cs.label}</span>
                    <span class="claude-time">{relativeTime(cs.updated_at)}</span>
                  </button>
                  <button
                    class="claude-edit"
                    title={$trad("term.renameSession")}
                    onclick={(e) => { e.stopPropagation(); startRenameClaude(cs); }}
                  >✎</button>
                </div>
              {/if}
            {/each}
          {/if}
        </div>
      {/if}
    </div>
  </div>
  <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
  <div
    class="term-container"
    class:drop-over={dropOver}
    bind:this={container}
    role="application"
    oncontextmenu={openCtxMenu}
  >
    {#if sessions.length === 0}
      <div class="term-empty">
        <p>{$trad("term.empty")}</p>
        <button class="btn" onclick={() => addTerminal()}>{$trad("term.openOne")}</button>
      </div>
    {/if}
    {#if dropOver}
      <div class="drop-hint">{$trad("term.dropHint")}</div>
    {/if}
  </div>
</div>

{#if ctxMenu}
  <ContextMenu
    x={ctxMenu.x}
    y={ctxMenu.y}
    items={[
      { label: $trad("common.copy"), action: copySelection },
      { label: $trad("common.paste"), action: pasteClipboard },
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

  .claude-menu { position: relative; margin-left: auto; }
  .term-claude {
    padding: 0.2rem 0.6rem; font-size: 0.8rem; cursor: pointer;
    border: 1px solid var(--border-color); border-radius: 4px;
    background: var(--bg-secondary); color: var(--text-secondary);
  }
  .term-claude:hover { color: var(--accent); border-color: var(--accent); }
  .claude-dropdown {
    position: absolute; right: 0; top: calc(100% + 4px); z-index: 20;
    width: 380px; max-height: 320px; overflow-y: auto;
    background: var(--bg-secondary); border: 1px solid var(--border-color);
    border-radius: 6px; box-shadow: 0 6px 20px rgba(0, 0, 0, 0.25);
    padding: 0.25rem;
  }
  .claude-item {
    display: flex; justify-content: space-between; align-items: baseline; gap: 0.6rem;
    width: 100%; padding: 0.35rem 0.5rem; font-size: 0.78rem;
    background: none; border: none; color: var(--text-secondary);
    cursor: pointer; text-align: left; border-radius: 4px;
  }
  .claude-item:hover { background: var(--bg-tertiary); color: var(--text-primary); }
  .claude-item.muted { color: var(--text-muted); cursor: default; }
  .claude-item.new { color: var(--accent); font-weight: 600; }
  .claude-label {
    flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap;
  }
  .claude-label.renamed { font-weight: 600; color: var(--text-primary); }
  .claude-time { flex-shrink: 0; color: var(--text-muted); font-size: 0.7rem; }
  .claude-row { display: flex; align-items: center; }
  .claude-row .claude-item { flex: 1; min-width: 0; }
  .claude-edit {
    flex-shrink: 0; background: none; border: none; cursor: pointer;
    color: var(--text-muted); font-size: 0.75rem; padding: 0 0.4rem;
    opacity: 0; transition: opacity 0.12s;
  }
  .claude-row:hover .claude-edit { opacity: 1; }
  .claude-edit:hover { color: var(--accent); }
  .claude-rename {
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
