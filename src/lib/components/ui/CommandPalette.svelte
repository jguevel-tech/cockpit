<script lang="ts">
  /**
   * Palette de commandes (Ctrl+K) : saut clavier vers un projet, un terminal, un onglet,
   * une vue, une commande rapide ou un fichier du projet courant.
   *
   * Ctrl+K n'est PAS intercepte quand le focus est dans un terminal : ce raccourci
   * appartient au shell (kill-line). Cliquer hors du terminal suffit pour l'utiliser.
   */
  import { onMount, tick } from "svelte";
  import { portal } from "../../actions/portal";
  import { projects } from "../../stores/projects";
  import { terminals } from "../../stores/terminals";
  import {
    activeView, selectedProject, activeTab, dashboardView,
    pendingTerminalId, pendingTerminalCommand, pendingFilePath, selectProject, openView,
  } from "../../stores/ui";
  import { getProjectCommands } from "../../api/storage";
  import { searchProject } from "../../api/workspace";
  import { notify } from "../../stores/toast";
  import type { ProjectCommand, SearchNameHit } from "../../types";
  import { trad, translate } from "../../i18n";
  import { signalerErreur } from "../../stores/errors";

  let open = $state(false);
  let query = $state("");
  let selected = $state(0);
  let inputEl: HTMLInputElement | undefined = $state();
  let quickCommands: ProjectCommand[] = $state([]);
  let fileHits: SearchNameHit[] = $state([]);
  let fileSeq = 0;
  let fileDebounce: ReturnType<typeof setTimeout> | undefined;

  interface Entry {
    section: string;
    label: string;
    hint?: string;
    run: () => void | Promise<void>;
  }

  const currentProject = $derived(
    $activeView === "project" && $selectedProject
      ? $projects.find((p) => p.name === $selectedProject)
      : undefined
  );

  function norm(s: string): string {
    return s.normalize("NFD").replace(/\p{M}/gu, "").toLowerCase();
  }

  const PROJECT_TABS: {
    id: "workspace" | "docker" | "terminal" | "files" | "git" | "plugins" | "settings";
    labelKey: Parameters<typeof translate>[0];
  }[] = [
    { id: "workspace", labelKey: "tab.workspace" }, { id: "docker", labelKey: "tab.docker" },
    { id: "terminal", labelKey: "tab.terminal" }, { id: "files", labelKey: "tab.files" },
    { id: "git", labelKey: "tab.git" }, { id: "plugins", labelKey: "tab.plugins" },
    { id: "settings", labelKey: "tab.projectSettings" },
  ];

  const entries: Entry[] = $derived.by(() => {
    const out: Entry[] = [];
    const q = norm(query.trim());
    const match = (label: string, hint = "") => !q || norm(label).includes(q) || norm(hint).includes(q);
    const cap = <T,>(arr: T[]) => arr.slice(0, 8);

    if (currentProject) {
      for (const t of PROJECT_TABS) {
        const tabLabel = $trad(t.labelKey);
        if (match(tabLabel)) out.push({
          section: $trad("palette.sectionTabs"), label: tabLabel, hint: currentProject.name,
          run: () => activeTab.set(t.id),
        });
      }
      for (const c of quickCommands) {
        if (match(c.label, c.command)) out.push({
          section: $trad("palette.sectionQuickCommands"), label: `▶ ${c.label}`, hint: c.command,
          run: () => runQuickCommand(c),
        });
      }
    }

    out.push(...cap($projects.filter((p) => match(p.name, p.description)).map((p) => ({
      section: $trad("palette.sectionProjects"), label: p.name, hint: p.description,
      run: () => selectProject(p.name),
    }))));

    out.push(...cap($terminals.filter((t) => match(t.name, t.project)).map((t) => ({
      section: $trad("palette.sectionTerminals"), label: t.name || `Terminal ${t.id}`, hint: t.project,
      run: () => {
        selectProject(t.project);
        activeTab.set("terminal");
        pendingTerminalId.set(t.id);
      },
    }))));

    const views: Entry[] = [
      { section: $trad("palette.sectionViews"), label: $trad("palette.dashTasks"), run: () => { openView("dashboard"); dashboardView.set("tasks"); } },
      { section: $trad("palette.sectionViews"), label: $trad("palette.dashMonitoring"), run: () => { openView("dashboard"); dashboardView.set("monitoring"); } },
      { section: $trad("palette.sectionViews"), label: $trad("palette.dashTerminals"), run: () => { openView("dashboard"); dashboardView.set("terminals"); } },
      { section: $trad("palette.sectionViews"), label: $trad("palette.dashContainers"), run: () => { openView("dashboard"); dashboardView.set("containers"); } },
      { section: $trad("palette.sectionViews"), label: $trad("settings.title"), run: () => openView("settings") },
    ];
    out.push(...views.filter((v) => match(v.label)));

    for (const f of fileHits) {
      out.push({
        section: $trad("palette.sectionFiles"), label: f.rel_path, hint: currentProject?.name,
        run: () => {
          activeTab.set("files");
          pendingFilePath.set(f.rel_path);
        },
      });
    }

    return out;
  });

  // Le terminal est cree par l'onglet Terminal, seul a connaitre la taille de son
  // conteneur : une TUI lancee a la creation garde celle du PTY (voir honorerCommande
  // dans TerminalTab).
  function runQuickCommand(c: ProjectCommand) {
    const p = currentProject;
    if (!p?.path) { notify($trad("palette.noPathConfigured")); return; }
    pendingTerminalCommand.set({ project: p.name, command: c.command });
    activeTab.set("terminal");
  }

  // Fichiers du projet courant : recherche par NOM, en async debounce
  $effect(() => {
    const q = query.trim();
    const proj = currentProject;
    clearTimeout(fileDebounce);
    if (!open || !proj?.path || q.length < 2) { fileHits = []; return; }
    const path = proj.path;
    fileDebounce = setTimeout(async () => {
      const seq = ++fileSeq;
      try {
        const res = await searchProject(path, q);
        if (seq === fileSeq) fileHits = res.names.filter((n) => !n.is_dir).slice(0, 8);
      } catch (e) {
      signalerErreur("commandPalette.runQuickCommand", String(e));
        if (seq === fileSeq) fileHits = [];
      }
    }, 250);
  });

  async function openPalette() {
    open = true;
    query = "";
    selected = 0;
    fileHits = [];
    quickCommands = [];
    if (currentProject) {
      try { quickCommands = await getProjectCommands(currentProject.name); } catch (e) {
      signalerErreur("commandPalette.openPalette", String(e)); quickCommands = []; }
    }
    await tick();
    inputEl?.focus();
  }

  function close() {
    open = false;
  }

  function runEntry(e: Entry) {
    close();
    e.run();
  }

  function onGlobalKeydown(e: KeyboardEvent) {
    if ((e.ctrlKey || e.metaKey) && !e.shiftKey && (e.key === "k" || e.key === "K")) {
      // Le focus est dans un terminal : Ctrl+K est au shell, on ne vole rien.
      const el = document.activeElement as HTMLElement | null;
      if (el?.classList.contains("xterm-helper-textarea")) return;
      e.preventDefault();
      e.stopPropagation();
      if (!open) openPalette();
      else close();
    }
  }

  function onPaletteKeydown(e: KeyboardEvent) {
    if (e.key === "Escape") { close(); return; }
    if (e.key === "ArrowDown") { e.preventDefault(); selected = Math.min(selected + 1, entries.length - 1); }
    else if (e.key === "ArrowUp") { e.preventDefault(); selected = Math.max(selected - 1, 0); }
    else if (e.key === "Enter") {
      e.preventDefault();
      const entry = entries[selected];
      if (entry) runEntry(entry);
    }
  }

  // La selection suit la liste filtree
  $effect(() => {
    if (selected >= entries.length) selected = 0;
  });

  onMount(() => {
    window.addEventListener("keydown", onGlobalKeydown, { capture: true });
    return () => window.removeEventListener("keydown", onGlobalKeydown, { capture: true });
  });
</script>

{#if open}
  <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
  <div class="palette-overlay" role="dialog" aria-modal="true" tabindex="-1" use:portal onclick={(e) => { if (e.target === e.currentTarget) close(); }} onkeydown={onPaletteKeydown}>
    <div class="palette">
      <input
        bind:this={inputEl}
        bind:value={query}
        placeholder={$trad("palette.placeholder")}
      />
      <div class="results">
        {#each entries as entry, i (entry.section + entry.label)}
          {#if i === 0 || entries[i - 1].section !== entry.section}
            <div class="section">{entry.section}</div>
          {/if}
          <button
            class="entry"
            class:selected={i === selected}
            onclick={() => runEntry(entry)}
            onmouseenter={() => (selected = i)}
          >
            <span class="entry-label">{entry.label}</span>
            {#if entry.hint}<span class="entry-hint">{entry.hint}</span>{/if}
          </button>
        {/each}
        {#if entries.length === 0}
          <div class="section">{$trad("palette.noResult")}</div>
        {/if}
      </div>
      <div class="footer">{$trad("palette.hints")}</div>
    </div>
  </div>
{/if}

<style>
  .palette-overlay {
    position: fixed; inset: 0; z-index: 1000;
    background: rgba(0, 0, 0, 0.4);
    /* Voile plein ecran PEINT : porte son propre flou (bug WebKitGTK, voir CLAUDE.md) */
    backdrop-filter: blur(12px);
    display: flex; align-items: flex-start; justify-content: center;
    padding-top: 12vh;
  }
  .palette {
    width: min(38rem, 92vw);
    /* Surface flottante : token OPAQUE, jamais --bg-* (translucides sous wallpaper) */
    background: var(--surface-base);
    border: 1px solid var(--border-color);
    border-radius: var(--radius-lg, 12px);
    box-shadow: var(--shadow-lg, 0 16px 48px rgba(0, 0, 0, 0.35));
    display: flex; flex-direction: column; overflow: hidden;
  }
  .palette input {
    border: none; outline: none; background: transparent;
    color: var(--text-primary); font-size: 1rem;
    padding: 0.85rem 1rem; border-bottom: 1px solid var(--border-color);
  }
  .results { max-height: 50vh; overflow-y: auto; padding: 0.3rem; }
  .section {
    padding: 0.4rem 0.7rem 0.15rem; font-size: 0.68rem; font-weight: 700;
    text-transform: uppercase; letter-spacing: 0.05em; color: var(--text-muted);
  }
  .entry {
    display: flex; align-items: center; gap: 0.6rem; width: 100%;
    background: none; border: none; cursor: pointer; text-align: left;
    padding: 0.4rem 0.7rem; border-radius: 6px;
    color: var(--text-primary); font-size: 0.88rem;
  }
  .entry.selected { background: var(--accent-soft); }
  .entry-label { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .entry-hint {
    margin-left: auto; color: var(--text-muted); font-size: 0.72rem;
    overflow: hidden; text-overflow: ellipsis; white-space: nowrap; max-width: 45%;
  }
  .footer {
    padding: 0.4rem 0.8rem; border-top: 1px solid var(--border-color);
    font-size: 0.7rem; color: var(--text-muted);
  }
</style>
