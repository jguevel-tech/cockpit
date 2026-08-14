<script lang="ts" module>
  import { listen as listenGlobal } from "@tauri-apps/api/event";
  import type { Terminal as XTerminal } from "@xterm/xterm";
  import type { FitAddon as XFitAddon } from "@xterm/addon-fit";

  /// POOL PERSISTANT — LE COEUR DE L'ARCHITECTURE TERMINAUX (NE PAS RE-LOCALISER).
  ///
  /// Les instances xterm ET les clients tmux SURVIVENT au demontage de l'onglet. Changer de
  /// projet ou d'onglet ne detache plus rien : on gare les elements DOM dans un conteneur
  /// invisible et on les re-adopte au retour. Le switch est un pur masquer/montrer.
  ///
  /// Pourquoi c'est indispensable, et pas une optimisation (prouve le 2026-08-13) :
  /// tmux FABRIQUE lui-meme des evenements focus (in/out) vers l'application du pane a CHAQUE
  /// attache/detache de client — meme avec `focus-events off`, qui ne gouverne que le focus
  /// venant du terminal exterieur. Demonstration en isolation : un cycle attache/tue/rattache
  /// SANS AUCUNE entree fait emettre a claude sa reaction focus (`ESC(B SI CSI<u CSI>1u
  /// CSI>4;2m` + re-render), et ce re-render laisse regulierement une ligne vide a l'ecran.
  /// Trois correctifs (0.6.5, 0.6.7) ont vise d'autres maillons sans eteindre le symptome :
  /// la seule solution est de NE PLUS churner les clients. Benefice annexe : switch instantane.
  ///
  /// L'ancienne doctrine « attach = client tmux frais obligatoirement » (sequence d'init,
  /// molette, redraw) reste vraie pour un xterm NEUF — et c'est exactement pourquoi le pool
  /// conserve le xterm d'origine : il a deja recu l'init de son client, les deux vieillissent
  /// ensemble.
  export type PoolEntry = { term: XTerminal; fit: XFitAddon; el: HTMLDivElement };
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
    e.term.dispose();
    e.el.remove();
    pool.delete(id);
  }

  function b64ToBytes(data: string): Uint8Array {
    return Uint8Array.from(atob(data), (c) => c.charCodeAt(0));
  }

  // Listeners GLOBAUX, enregistres une fois pour la vie de l'app : la sortie doit continuer
  // d'alimenter les xterm du pool meme quand aucun onglet Terminal n'est monte, sinon on
  // retrouverait un ecran fige au retour.
  listenGlobal<{ id: number; data: string }>("terminal_output", (e) => {
    pool.get(e.payload.id)?.term.write(b64ToBytes(e.payload.data));
  });
  listenGlobal<number>("terminal_exit", (e) => {
    pool.get(e.payload)?.term.write("\r\n\x1b[2m[processus terminé]\x1b[0m\r\n");
  });
</script>

<script lang="ts">
  import { onMount } from "svelte";
  import { listen, type UnlistenFn } from "@tauri-apps/api/event";
  import { Terminal } from "@xterm/xterm";
  import { FitAddon } from "@xterm/addon-fit";
  import { WebglAddon } from "@xterm/addon-webgl";
  import { WebLinksAddon } from "@xterm/addon-web-links";
  import "@xterm/xterm/css/xterm.css";
  import { pendingTerminalId, TERMINAL_FONT_SIZE } from "../../stores/ui";
  // themeBase et non la palette : xterm n a que deux jeux de couleurs.
  import { themeBase } from "../../stores/appearance";
  import { projects } from "../../stores/projects";
  import { loadTerminals } from "../../stores/terminals";
  import {
    createTerminal, writeTerminal, resizeTerminal, closeTerminal,
    attachTerminal, renameTerminal, listTerminals,
    listClaudeSessions, renameClaudeSession, setClipboard, getClipboard,
    terminalCopySelection, terminalSearch, openUrl,
  } from "../../api/workspace";
  import { notify } from "../../stores/toast";
  import ContextMenu from "../ui/ContextMenu.svelte";
  import type { ClaudeSession } from "../../types";

  let { name }: { name: string } = $props();

  let sessions: { id: number; alive: boolean; name: string }[] = $state([]);
  let activeId: number | null = $state(null);
  let container: HTMLDivElement | undefined = $state(undefined);
  // Menu contextuel Copier/Coller du terminal (remplace celui de tmux, retire)
  let ctxMenu: { x: number; y: number } | null = $state(null);
  let renamingId: number | null = $state(null);
  let renameValue = $state("");

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

  // File d'ecriture/resize par terminal : chaque invoke part apres le retour du
  // precedent. Sans ca, des invoke rapproches peuvent s'executer dans le desordre
  // cote Tauri -> octets melanges dans le PTY.
  const ioQueues = new Map<number, Promise<unknown>>();
  const lastSentSize = new Map<number, string>();

  function enqueue(id: number, op: () => Promise<unknown>) {
    const next = (ioQueues.get(id) ?? Promise.resolve()).then(op, op);
    ioQueues.set(id, next.catch(() => {}));
  }
  function queueWrite(id: number, data: string) {
    enqueue(id, () => writeTerminal(id, data));
  }
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
  /// Un client tmux interroge le terminal a son demarrage : attributs (DA1 `ESC[c`,
  /// DA2 `ESC[>c`), position du curseur (`ESC[6n`), etat. xterm.js repond par le MEME canal
  /// `onData` que les frappes. En regime etabli la reponse est consommee par le client qui a
  /// pose la question, et tout va bien.
  ///
  /// Mais `attach` TUE l'ancien client et en lance un neuf — indispensable, seul un client
  /// frais renvoie la sequence d'initialisation complete. Les reponses aux questions de
  /// l'ANCIEN client arrivent apres coup et partent dans le PTY du NOUVEAU, qui n'a rien
  /// demande. tmux ne les reconnait donc pas comme des reponses et les transmet au pane : le
  /// shell affiche `^[[?1;2c^[[>0;276;0c`, et `1;2c0;276;0c` atterrit dans l'invite.
  ///
  /// Diagnostique le 2026-08-13 PAR INSTRUMENTATION : le log ne montrait aucun `resize` apres
  /// les `attach`, ce qui a elimine l'hypothese d'un repaint du a un changement de taille —
  /// hypothese qu'on aurait autrement "corrigee" a tort.
  ///
  /// tmux ne perd rien : les capacites qu'il tirait de ces sondages sont declarees
  /// explicitement dans `terminal-features` (conf generee, terminal/mod.rs).
  ///
  /// Les evenements de focus (`ESC[I` / `ESC[O`) sont AUSSI filtres — revirement documente.
  /// Une premiere version les laissait passer (« ils sont destines a l'application »). Or
  /// c'est exactement eux qui causaient le saut de ligne au changement de terminal :
  /// - pipe-pane sur la session reelle pendant un switch : la SEULE entree recue par claude
  ///   etait blur+focus, et sa reaction capturee octet par octet (`ESC(B SI CSI<u CSI>1u
  ///   CSI>4;2m` + re-render) ;
  /// - les sauts de ligne coincidaient 1:1 avec les switchs, toutes les autres causes ayant
  ///   ete eliminees par mesure (taille constante 177x41, churn d'attach innocente en labo).
  /// Un changement d'onglet dans Cockpit n'est de toute facon pas une perte de focus du point
  /// de vue de l'utilisateur. Cout : les TUI ne peuvent plus attenuer leur bordure au blur.
  const TERMINAL_REPLY =
    /^(?:\x1b\[(?:\?[0-9;]*c|>[0-9;]*c|[0-9;]*R|[0-9;]*n|\?[0-9;]*\$y|[IO])|\x1bP[^\x1b]*\x1b\\|\x1b\][^\x07\x1b]*(?:\x07|\x1b\\))+$/;

  const XTERM_THEMES = {
    dark: { background: "#111318", foreground: "#d4d7dd", cursor: "#d4d7dd", selectionBackground: "#33415580" },
    light: { background: "#ffffff", foreground: "#24292f", cursor: "#24292f", selectionBackground: "#b6d7ff80" },
  };

  onMount(() => {
    (async () => {
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

      const wanted = $pendingTerminalId;
      pendingTerminalId.set(null);
      if (wanted !== null && sessions.some((s) => s.id === wanted)) {
        await activate(wanted);
      } else if (sessions.length === 0) {
        await addTerminal();
      } else {
        await activate(sessions[0].id);
      }
    })();

    // Debounce : pendant un drag de fenetre, on n'envoie que la taille finale
    resizeObserver = new ResizeObserver(() => {
      if (fitTimer) clearTimeout(fitTimer);
      fitTimer = setTimeout(() => fitActive(), 80);
    });
    if (container) resizeObserver.observe(container);

    return () => {
      resizeObserver?.disconnect();
      unlisteners.forEach((u) => u());
      // NI detach, NI dispose : clients tmux et xterm restent vivants dans le pool.
      // Detacher/rattacher ferait synthetiser par tmux des evenements focus vers les
      // applications (voir le commentaire du pool) — c'etait la cause du saut de ligne.
      // On gare simplement les elements DOM hors du document visible.
      parkAll();
      mounted.clear();
    };
  });

  // Raccourci depuis la sidebar/dashboard vers un terminal du MEME projet :
  // le composant n'est pas remonte (meme projet), donc on reagit au store.
  $effect(() => {
    const wanted = $pendingTerminalId;
    if (wanted === null) return;
    if (sessions.some((s) => s.id === wanted)) {
      pendingTerminalId.set(null);
      if (activeId !== wanted) activate(wanted);
    }
  });

  // Suit le theme de l'app
  $effect(() => {
    const t = $themeBase;
    // Tout le pool suit le theme, y compris les terminaux gares d autres projets.
    pool.forEach(({ term }) => (term.options.theme = XTERM_THEMES[t]));
  });

  // --- Copier / Coller (clic droit) ---
  // Copie la selection : locale xterm (Shift+glisser) en priorite, sinon la
  // selection copy-mode tmux (surlignage bleu). Chemin souris uniquement.
  async function copySelection() {
    if (activeId === null) return;
    const entry = pool.get(activeId);
    if (entry?.term.hasSelection()) {
      const sel = entry.term.getSelection();
      entry.term.clearSelection();
      if (sel) { try { await setClipboard(sel); } catch {} }
    } else {
      try { await terminalCopySelection(activeId); } catch {}
    }
    entry?.term.focus();
  }

  async function pasteClipboard() {
    if (activeId === null) return;
    const entry = pool.get(activeId);
    if (!entry) return;
    try {
      const text = await getClipboard();
      // term.paste() passe par onData (bracketed paste) -> chemin d'entree normal
      if (text) entry.term.paste(text);
    } catch {}
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
      scrollback: 5000,
      rescaleOverlappingGlyphs: true,
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
    // Le clic simple reste a tmux (selection souris) — pas de conflit.
    term.loadAddon(
      new WebLinksAddon((event, uri) => {
        if (event.ctrlKey || event.metaKey) {
          openUrl(uri).catch((e) => notify(String(e)));
        }
      })
    );

    // COPIE : la selection souris est geree par tmux (mouse on), pas par xterm.
    // Avec `set-clipboard on`, tmux emet la selection en OSC 52 (base64) au
    // relachement du clic -> on la pousse dans le presse-papier systeme via Rust.
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

    return { term, fit, el };
  }

  async function addTerminal(initCommand?: string) {
    if (!container) return;
    // JAMAIS de retour silencieux ici : c'est exactement ce qui a laisse le premier
    // utilisateur externe cliquer sur + sans que rien ne se passe ni ne s'affiche.
    if (!project) {
      notify("Projet introuvable dans la liste — redémarre Cockpit ou vérifie sa configuration.");
      return;
    }
    if (!project.path) {
      notify("Ce projet n'a pas de chemin : renseigne-le dans l'onglet Paramètres du projet.");
      return;
    }
    // On mesure AVANT de creer le PTY : le shell (et une TUI lancee via
    // init_command) demarre directement a la bonne taille, pas en 80x24.
    const entry = createXterm();
    mounted.forEach((tid) => { const e = pool.get(tid); if (e) e.el.style.display = "none"; });
    entry.el.style.display = "block";
    try { entry.fit.fit(); } catch {}
    const cols = entry.term.cols || 80;
    const rows = entry.term.rows || 24;

    try {
      const id = await createTerminal(name, project.path, cols, rows, initCommand);
      pool.set(id, entry);
      mounted.add(id);
      lastSentSize.set(id, `${cols}x${rows}`);
      // Le nom par defaut (« PROJET - N ») est genere EN BASE a la creation : on le relit
      // pour que l'onglet affiche la meme chose que la sidebar, au lieu de pousser un nom
      // vide qui retombait sur le fallback « Terminal N ».
      const created = (await listTerminals(name)).find((t) => t.id === id);
      sessions.push({ id, alive: true, name: created?.name ?? "" });
      try { await attachTerminal(id, cols, rows); } catch {}
      entry.term.onData((data) => sendInput(id, data));
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

  // --- Recherche dans l'historique (copy-mode tmux) ---
  // Le scrollback vit dans TMUX, pas dans xterm (ecran alternatif du client) : c'est donc
  // la recherche NATIVE du copy-mode qui cherche, surligne et compte (n/N dans le pane).
  // Aucune interception du chemin de frappe : la barre a son propre input, et le seul
  // raccourci (Ctrl+Maj+F) est capte en phase capture sur window, AVANT xterm.
  let searchOpen = $state(false);
  let searchQuery = $state("");
  let searchStarted = $state(false);
  let searchInputEl: HTMLInputElement | undefined = $state();

  function openSearch() {
    if (activeId === null) return;
    searchOpen = true;
    requestAnimationFrame(() => { searchInputEl?.focus(); searchInputEl?.select(); });
  }

  async function runSearch() {
    if (activeId === null || !searchQuery.trim()) return;
    try {
      await terminalSearch(activeId, "start", searchQuery.trim());
      searchStarted = true;
    } catch (e) { notify(String(e)); }
  }

  async function searchStep(dir: "next" | "prev") {
    if (activeId === null || !searchStarted) return;
    try { await terminalSearch(activeId, dir); } catch (e) { notify(String(e)); }
  }

  async function closeSearch(forId: number | null = activeId, refocus = true) {
    searchOpen = false;
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
      // RE-ADOPTION : le xterm et son client tmux ont surveu au demontage precedent — on
      // remet simplement l'element dans le conteneur. Aucun detach/attach, donc tmux ne
      // synthetise aucun evenement focus vers l'application : c'est LE correctif du saut
      // de ligne au switch (voir le commentaire du pool).
      container?.appendChild(existing.el);
      mounted.add(id);
      // Si le client est mort entre-temps (reboot du serveur tmux), le backend en relance
      // un ; s'il est vivant, l'appel est un no-op silencieux.
      try { existing.fit.fit(); } catch {}
      try { await attachTerminal(id, existing.term.cols || 80, existing.term.rows || 24); } catch {}
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
    if (!container) return;
    const entry = createXterm();
    pool.set(id, entry);
    mounted.add(id);

    // Fit AVANT l'attach : le client tmux demarre a la bonne taille et
    // repeint l'ecran de la session tout seul.
    showOnly(id);
    try { entry.fit.fit(); } catch {}
    const cols = entry.term.cols || 80;
    const rows = entry.term.rows || 24;
    lastSentSize.set(id, `${cols}x${rows}`);

    try {
      // Le replay retourne est IGNORE volontairement : le client tmux
      // fraichement attache repeint tout l'ecran lui-meme (source unique).
      // Rejouer en plus notre buffer creait une course entre les deux sources
      // (events live vs retour d'invoke) -> affichage dechire/duplique au
      // retour sur l'onglet, et reponses parasites aux vieilles requetes
      // DA/CPR ("1;2c0;276;0c" tape dans le shell). L'historique molette
      // reste complet via le copy-mode tmux (history-limit 10000).
      await attachTerminal(id, cols, rows);
    } catch (e) {
      // Session morte cote tmux : on la retire de la liste
      mounted.delete(id);
      disposePoolEntry(id);
      sessions = sessions.filter((s) => s.id !== id);
      return;
    }

    entry.term.onData((data) => sendInput(id, data));
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
    if (!s) return;
    s.name = value;
    try { await renameTerminal(id, value); } catch {}
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
      catch { claudeSessions = []; }
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
    } catch {}
  }

  async function resumeClaude(session: ClaudeSession) {
    claudeOpen = false;
    await addTerminal(`claude --resume ${session.id}`);
    const active = sessions.find((s) => s.id === activeId);
    if (active) {
      active.name = `Claude · ${session.label.slice(0, 24)}`;
      try { await renameTerminal(active.id, active.name); } catch {}
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
          title="Double-clic ou clic droit pour renommer"
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
    <button class="term-add" onclick={() => addTerminal()} title="Nouveau terminal">+</button>

    {#if searchOpen}
      <span class="term-search">
        <input
          class="term-search-input"
          bind:this={searchInputEl}
          bind:value={searchQuery}
          placeholder="Rechercher dans l'historique…"
          onkeydown={(e) => {
            if (e.key === "Enter") { e.preventDefault(); runSearch(); }
            else if (e.key === "Escape") closeSearch();
          }}
        />
        <button class="term-search-btn" onclick={() => searchStep("next")} title="Occurrence plus ancienne" disabled={!searchStarted}>↑</button>
        <button class="term-search-btn" onclick={() => searchStep("prev")} title="Occurrence plus récente" disabled={!searchStarted}>↓</button>
        <button class="term-search-btn" onclick={() => closeSearch()} title="Fermer (Échap)">×</button>
      </span>
    {:else}
      <button class="term-search-btn" onclick={openSearch} title="Rechercher dans l'historique (Ctrl+Maj+F)">🔍</button>
    {/if}

    <div class="claude-menu">
      <button class="term-claude" onclick={toggleClaude} title="Reprendre une conversation Claude Code">
        ✳ Claude ▾
      </button>
      {#if claudeOpen}
        <div class="claude-dropdown">
          <button class="claude-item new" onclick={() => { claudeOpen = false; addTerminal("claude"); }}>
            + Nouvelle session claude
          </button>
          {#if claudeLoading}
            <div class="claude-item muted">Chargement…</div>
          {:else if claudeSessions.length === 0}
            <div class="claude-item muted">Aucune conversation passée sur ce projet</div>
          {:else}
            {#each claudeSessions as cs (cs.id)}
              {#if renamingClaudeId === cs.id}
                <!-- svelte-ignore a11y_autofocus -->
                <input
                  class="claude-rename"
                  type="text"
                  bind:value={renameClaudeValue}
                  placeholder="Nom (vide = label auto)"
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
                    title="Renommer cette session"
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
  <div class="term-container" bind:this={container} role="application" oncontextmenu={openCtxMenu}>
    {#if sessions.length === 0}
      <div class="term-empty">Aucun terminal. Clique sur + pour en ouvrir un.</div>
    {/if}
  </div>
</div>

{#if ctxMenu}
  <ContextMenu
    x={ctxMenu.x}
    y={ctxMenu.y}
    items={[
      { label: "Copier", action: copySelection },
      { label: "Coller", action: pasteClipboard },
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
  .term-empty {
    display: flex; align-items: center; justify-content: center; height: 100%;
    color: var(--text-muted); font-size: 0.85rem;
  }
</style>
