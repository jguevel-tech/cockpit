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
    attachTerminal, detachTerminal, renameTerminal, listTerminals,
    listClaudeSessions, renameClaudeSession, setClipboard, getClipboard,
    terminalCopySelection, openUrl,
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

  // Instances xterm par session (non reactif : objets lourds)
  const terms = new Map<number, { term: Terminal; fit: FitAddon; el: HTMLDivElement }>();
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
    const clean = data.indexOf("\u00a0") === -1 ? data : data.replace(/\u0020?\u00a0/g, "");
    if (clean) queueWrite(id, clean);
  }

  const XTERM_THEMES = {
    dark: { background: "#111318", foreground: "#d4d7dd", cursor: "#d4d7dd", selectionBackground: "#33415580" },
    light: { background: "#ffffff", foreground: "#24292f", cursor: "#24292f", selectionBackground: "#b6d7ff80" },
  };

  function b64ToBytes(data: string): Uint8Array {
    return Uint8Array.from(atob(data), (c) => c.charCodeAt(0));
  }

  onMount(() => {
    (async () => {
      unlisteners.push(
        await listen<{ id: number; data: string }>("terminal_output", (e) => {
          terms.get(e.payload.id)?.term.write(b64ToBytes(e.payload.data));
        })
      );
      unlisteners.push(
        await listen<number>("terminal_exit", (e) => {
          const s = sessions.find((s) => s.id === e.payload);
          if (s) s.alive = false;
          terms.get(e.payload)?.term.write("\r\n\x1b[2m[processus terminé]\x1b[0m\r\n");
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
      // Detache cote backend (plus d'events IPC) et libere l'UI ;
      // les shells continuent de tourner, tmux repeindra au prochain attach.
      terms.forEach((_, id) => { detachTerminal(id); });
      terms.forEach(({ term }) => term.dispose());
      terms.clear();
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
    terms.forEach(({ term }) => (term.options.theme = XTERM_THEMES[t]));
  });

  // --- Copier / Coller (clic droit) ---
  // Copie la selection : locale xterm (Shift+glisser) en priorite, sinon la
  // selection copy-mode tmux (surlignage bleu). Chemin souris uniquement.
  async function copySelection() {
    if (activeId === null) return;
    const entry = terms.get(activeId);
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
    const entry = terms.get(activeId);
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
    if (!project?.path || !container) return;
    // On mesure AVANT de creer le PTY : le shell (et une TUI lancee via
    // init_command) demarre directement a la bonne taille, pas en 80x24.
    const entry = createXterm();
    terms.forEach(({ el }) => { el.style.display = "none"; });
    entry.el.style.display = "block";
    try { entry.fit.fit(); } catch {}
    const cols = entry.term.cols || 80;
    const rows = entry.term.rows || 24;

    try {
      const id = await createTerminal(name, project.path, cols, rows, initCommand);
      terms.set(id, entry);
      lastSentSize.set(id, `${cols}x${rows}`);
      sessions.push({ id, alive: true, name: "" });
      try { await attachTerminal(id, cols, rows); } catch {}
      entry.term.onData((data) => sendInput(id, data));
      activeId = id;
      entry.term.focus();
      loadTerminals();
    } catch (e) {
      entry.term.dispose();
      entry.el.remove();
      alert(e);
    }
  }

  async function activate(id: number) {
    activeId = id;
    if (!terms.has(id)) await attachExisting(id);
    terms.forEach(({ el }, tid) => { el.style.display = tid === id ? "block" : "none"; });
    requestAnimationFrame(() => {
      fitActive();
      terms.get(id)?.term.focus();
    });
  }

  async function attachExisting(id: number) {
    if (!container) return;
    const entry = createXterm();
    terms.set(id, entry);

    // Fit AVANT l'attach : le client tmux demarre a la bonne taille et
    // repeint l'ecran de la session tout seul.
    terms.forEach(({ el }, tid) => { el.style.display = tid === id ? "block" : "none"; });
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
      entry.term.dispose();
      entry.el.remove();
      terms.delete(id);
      sessions = sessions.filter((s) => s.id !== id);
      return;
    }

    entry.term.onData((data) => sendInput(id, data));
  }

  function fitActive() {
    if (activeId === null) return;
    const entry = terms.get(activeId);
    if (!entry || entry.el.style.display === "none") return;
    try {
      entry.fit.fit();
      queueResize(activeId, entry.term.cols, entry.term.rows);
    } catch {}
  }

  async function closeTab(id: number) {
    try { await closeTerminal(id); } catch {}
    const entry = terms.get(id);
    if (entry) { entry.term.dispose(); entry.el.remove(); terms.delete(id); }
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
