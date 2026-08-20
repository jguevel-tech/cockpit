<script lang="ts">
  import { projects } from "../../stores/projects";
  import { selectedProject, selectProject, activeTab, pendingTerminalId } from "../../stores/ui";
  import { terminals, loadTerminals } from "../../stores/terminals";
  import { renameTerminal, closeTerminal } from "../../api/workspace";
  import type { TerminalInfo } from "../../types";
  import { reorderProjects, getProjectFolders, createProjectFolder, renameProjectFolder, deleteProjectFolder, moveProjectToFolder } from "../../api/scanner";
  import { loadProjects } from "../../stores/projects";
  import type { Project, ProjectFolder } from "../../types";
  import CreateProjectModal from "./CreateProjectModal.svelte";
  import InlineEdit from "../ui/InlineEdit.svelte";
  import ContextMenu from "../ui/ContextMenu.svelte";
  // Logo officiel Claude (fill terracotta embarque dans le SVG) : affiche quand un agent
  // IA tourne dans le terminal, a la place de l ancienne pastille verte.
  import claudeLogo from "../../assets/claude-logo.svg";
  import { notify } from "../../stores/toast";
  import { onMount } from "svelte";
  import { trad, tradN } from "../../i18n";
  import { signalerErreur } from "../../stores/errors";

  let showCreateModal = $state(false);
  let folders: ProjectFolder[] = $state([]);
  let collapsedIds: Set<number> = $state(new Set());
  let renamingFolderId: number | null = $state(null);
  let contextMenu: { id: number; x: number; y: number } | null = $state(null);
  let creatingFolder = $state(false);
  let newFolderName = $state("");

  let terminalsCollapsed = $state(false);

  onMount(() => {
    loadFolders();
    loadTerminals();
    // Restore collapsed state from localStorage
    try {
      const saved = localStorage.getItem("cockpit-collapsed-folders");
      if (saved) collapsedIds = new Set(JSON.parse(saved));
      terminalsCollapsed = localStorage.getItem("cockpit-terminals-collapsed") === "1";
    } catch (e) {
      signalerErreur("sidebar.onMount", String(e));}
  });

  function toggleTerminals() {
    terminalsCollapsed = !terminalsCollapsed;
    localStorage.setItem("cockpit-terminals-collapsed", terminalsCollapsed ? "1" : "0");
  }

  function gotoTerminal(t: TerminalInfo) {
    pendingTerminalId.set(t.id);
    selectProject(t.project);
    activeTab.set("terminal");
  }

  function terminalLabel(t: TerminalInfo): string {
    return t.name || "Terminal";
  }

  // Menu contextuel + renommage inline des terminaux
  let termContextMenu: { id: number; x: number; y: number } | null = $state(null);
  let renamingTermId: number | null = $state(null);

  function openTermContextMenu(e: MouseEvent, t: TerminalInfo) {
    e.preventDefault();
    termContextMenu = { id: t.id, x: e.clientX, y: e.clientY };
  }

  async function commitRenameTerminal(id: number, next: string) {
    renamingTermId = null;
    try { await renameTerminal(id, next); } catch (e) { notify(String(e)); }
    await loadTerminals();
  }

  async function closeTerminalById(id: number) {
    try { await closeTerminal(id); } catch (e) { notify(String(e)); }
    await loadTerminals();
  }

  async function loadFolders() {
    try { folders = await getProjectFolders(); } catch (e) { notify(String(e)); }
  }

  function saveCollapsed() {
    localStorage.setItem("cockpit-collapsed-folders", JSON.stringify([...collapsedIds]));
  }

  function toggleFolder(id: number) {
    const next = new Set(collapsedIds);
    if (next.has(id)) next.delete(id); else next.add(id);
    collapsedIds = next;
    saveCollapsed();
  }

  // Projects grouped by folder
  let rootProjects = $derived($projects.filter(p => p.folder_id === null));
  let projectsByFolder = $derived(() => {
    const map = new Map<number, Project[]>();
    for (const p of $projects) {
      if (p.folder_id !== null) {
        if (!map.has(p.folder_id)) map.set(p.folder_id, []);
        map.get(p.folder_id)!.push(p);
      }
    }
    return map;
  });

  function getFolderProjects(folderId: number): Project[] {
    return projectsByFolder().get(folderId) || [];
  }

  // Folder CRUD
  async function addFolder() {
    if (!newFolderName.trim()) return;
    try {
      await createProjectFolder(newFolderName.trim());
      newFolderName = "";
      creatingFolder = false;
      await loadFolders();
    } catch (e) { notify(String(e)); }
  }

  async function commitRenameFolder(id: number, next: string) {
    renamingFolderId = null;
    try {
      await renameProjectFolder(id, next);
      await loadFolders();
    } catch (e) { notify(String(e)); }
  }

  function openContextMenu(e: MouseEvent, id: number) {
    e.preventDefault();
    contextMenu = { id, x: e.clientX, y: e.clientY };
  }

  async function deleteFolder(id: number) {
    // Un dossier ne se supprime que VIDE : le supprimer plein detacherait ses projets
    // vers la racine en silence — surprise garantie. On explique au lieu d'agir.
    const count = getFolderProjects(id).length;
    if (count > 0) {
      notify($tradN("sidebar.folderNotEmpty", count));
      return;
    }
    try {
      await deleteProjectFolder(id);
      await loadFolders();
      await loadProjects();
    } catch (e) {
      notify(String(e));
    }
  }

  // Drag & drop: reorder + move to folder
  let dragProjectName: string | null = $state(null);
  let dropTarget: { name: string; pos: "before" | "after" } | null = $state(null);

  function onProjectDragStart(e: DragEvent, name: string) {
    dragProjectName = name;
    e.dataTransfer!.effectAllowed = "move";
    e.dataTransfer!.setData("text/plain", name);
  }

  function onProjectDragOver(e: DragEvent, targetName: string) {
    e.preventDefault();
    if (!dragProjectName || dragProjectName === targetName) { dropTarget = null; return; }
    const rect = (e.currentTarget as HTMLElement).getBoundingClientRect();
    const pos = e.clientY < rect.top + rect.height / 2 ? "before" : "after";
    dropTarget = { name: targetName, pos };
  }

  function onProjectDragLeave() { dropTarget = null; }

  function onProjectDrop(e: DragEvent, targetList: Project[]) {
    e.preventDefault();
    e.stopPropagation();
    if (!dragProjectName || !dropTarget) { dragProjectName = null; dropTarget = null; return; }

    // Reorder within the same list
    const names = targetList.map(p => p.name);
    const fromIdx = names.indexOf(dragProjectName);
    if (fromIdx === -1) {
      // Projet vient d'un autre contexte (autre dossier ou racine) — on le deplace d'abord
      const proj = $projects.find(p => p.name === dragProjectName);
      const targetProj = $projects.find(p => p.name === dropTarget!.name);
      if (proj && targetProj && proj.folder_id !== targetProj.folder_id) {
        moveProjectToFolder(dragProjectName, targetProj.folder_id).then(() => loadProjects());
      }
      dragProjectName = null; dropTarget = null;
      return;
    }

    const items = [...names];
    items.splice(fromIdx, 1);
    let targetIdx = items.indexOf(dropTarget.name);
    if (dropTarget.pos === "after") targetIdx++;
    items.splice(targetIdx, 0, dragProjectName);

    // Optimistic update: recompose full project list preserving non-affected
    const reordered = items.map(n => targetList.find(p => p.name === n)!);
    const otherProjects = $projects.filter(p => !targetList.includes(p));
    projects.set([...otherProjects, ...reordered]);

    reorderProjects(items).catch(() => loadProjects());
    dragProjectName = null; dropTarget = null;
  }

  function onProjectDragEnd() { dragProjectName = null; dropTarget = null; }

  // Drop on folder header = move project into folder
  function onFolderDrop(e: DragEvent, folderId: number | null) {
    e.preventDefault();
    if (!dragProjectName) return;
    const proj = $projects.find(p => p.name === dragProjectName);
    if (proj && proj.folder_id !== folderId) {
      moveProjectToFolder(dragProjectName, folderId).then(() => loadProjects());
    }
    dragProjectName = null; dropTarget = null;
  }

  function onFolderDragOver(e: DragEvent) {
    if (dragProjectName) e.preventDefault();
  }

  // Colors
  const stateColors: Record<string, string> = {
    running: "var(--success)", starting: "var(--warning)", stopping: "var(--warning)",
    error: "var(--error)", stopped: "var(--text-muted)",
  };
  function getColor(state: string) { return stateColors[state] || "var(--text-muted)"; }

  function focusOnMount(node: HTMLElement) { node.focus(); }

  function onNewFolderKeydown(e: KeyboardEvent) { if (e.key === "Enter") addFolder(); if (e.key === "Escape") creatingFolder = false; }
</script>

<aside>
  {#if $terminals.length > 0}
    <div class="sidebar-header terminals-header">
      <button class="section-toggle" onclick={toggleTerminals}>
        {terminalsCollapsed ? '▸' : '▾'} {$trad("sidebar.terminals")}
      </button>
      <span class="terminals-count">{$terminals.length}</span>
    </div>
    {#if !terminalsCollapsed}
      <ul class="terminals-list">
        {#each $terminals as t (t.id)}
          <li>
            {#if renamingTermId === t.id}
              <div class="terminal-item">
                {#if t.llm}<img class="term-llm" src={claudeLogo} alt="Claude" title={$trad("sidebar.claudeRunning")} />{:else}<span class="term-dot" title={$trad("sidebar.terminal")}></span>{/if}
                <InlineEdit
                  value={t.name}
                  placeholder={$trad("sidebar.terminalNamePlaceholder")}
                  onCommit={(next) => commitRenameTerminal(t.id, next)}
                  onCancel={() => (renamingTermId = null)}
                />
              </div>
            {:else}
              <button
                class="terminal-item"
                onclick={() => gotoTerminal(t)}
                oncontextmenu={(e) => openTermContextMenu(e, t)}
                title={$trad("sidebar.gotoTerminal", { project: t.project })}
              >
                {#if t.llm}<img class="term-llm" src={claudeLogo} alt="Claude" title={$trad("sidebar.claudeRunning")} />{:else}<span class="term-dot" title={$trad("sidebar.terminal")}></span>{/if}
                <span class="terminal-name">{terminalLabel(t)}</span>
                <span class="terminal-project">{t.project}</span>
              </button>
            {/if}
          </li>
        {/each}
      </ul>
    {/if}
  {/if}

  <div class="sidebar-header">
    <span>{$trad("sidebar.projects")}</span>
    <div class="header-actions">
      <button class="add-btn labeled" onclick={() => showCreateModal = true} title={$trad("sidebar.newProjectHint")}>{$trad("sidebar.newProject")}</button>
      <button class="add-btn labeled" onclick={() => creatingFolder = true} title={$trad("sidebar.newFolderHint")}>{$trad("sidebar.newFolder")}</button>
    </div>
  </div>

  {#if creatingFolder}
    <div class="folder-create">
      <input type="text" bind:value={newFolderName} placeholder={$trad("sidebar.folderNamePlaceholder")} onkeydown={onNewFolderKeydown} use:focusOnMount />
      <button onclick={addFolder}>OK</button>
      <button class="cancel" onclick={() => creatingFolder = false}>×</button>
    </div>
  {/if}

  <ul>
    <!-- Folders -->
    {#each folders as folder}
      <li class="folder-item"
        ondragover={onFolderDragOver}
        ondrop={(e) => onFolderDrop(e, folder.id)}
      >
        <div class="folder-header" role="toolbar" tabindex="-1" oncontextmenu={(e) => openContextMenu(e, folder.id)}>
          <button class="folder-toggle" onclick={() => toggleFolder(folder.id)}>
            {collapsedIds.has(folder.id) ? '▸' : '▾'}
          </button>
          {#if renamingFolderId === folder.id}
            <InlineEdit
              value={folder.name}
              onCommit={(next) => commitRenameFolder(folder.id, next)}
              onCancel={() => (renamingFolderId = null)}
            />
          {:else}
            <span
              class="folder-name"
              role="button"
              tabindex="0"
              ondblclick={() => (renamingFolderId = folder.id)}
              onkeydown={(e) => { if (e.key === "Enter") renamingFolderId = folder.id; }}
            >{folder.name}</span>
          {/if}
          <span class="folder-count">{getFolderProjects(folder.id).length}</span>
          <button
            class="folder-delete"
            title={getFolderProjects(folder.id).length > 0
              ? $trad("sidebar.folderNotEmptyHint")
              : $trad("sidebar.deleteFolderHint")}
            onclick={() => deleteFolder(folder.id)}
          >🗑</button>
        </div>
        {#if !collapsedIds.has(folder.id)}
          <ul class="folder-projects">
            {#each getFolderProjects(folder.id) as proj}
              <li
                draggable="true"
                ondragstart={(e) => onProjectDragStart(e, proj.name)}
                ondragover={(e) => onProjectDragOver(e, proj.name)}
                ondragleave={onProjectDragLeave}
                ondrop={(e) => onProjectDrop(e, getFolderProjects(folder.id))}
                ondragend={onProjectDragEnd}
                class:drag-over-top={dropTarget?.name === proj.name && dropTarget?.pos === "before"}
                class:drag-over-bottom={dropTarget?.name === proj.name && dropTarget?.pos === "after"}
              >
                <button
                  class="project-item"
                  class:active={$selectedProject === proj.name}
                  onclick={() => selectProject(proj.name)}
                >
                  <div class="project-main">
                    <span class="state-dot" style="background:{getColor(proj.state)}"></span>
                    <div class="project-info">
                      <span class="project-name">{proj.name}</span>
                      {#if proj.description}
                        <span class="project-desc">{proj.description}</span>
                      {/if}
                    </div>
                  </div>
                  <div class="project-meta">
                    <span class="project-state">{proj.state}</span>
                    {#if proj.containers.length > 0}
                      <span class="container-count">{$tradN("sidebar.containers", proj.containers.length)}</span>
                    {/if}
                  </div>
                </button>
              </li>
            {/each}
          </ul>
        {/if}
      </li>
    {/each}

    <!-- Root projects (no folder) -->
    <div class="root-drop-zone" role="list" ondragover={onFolderDragOver} ondrop={(e) => onFolderDrop(e, null)}>
      {#each rootProjects as proj}
        <li
          draggable="true"
          ondragstart={(e) => onProjectDragStart(e, proj.name)}
          ondragover={(e) => onProjectDragOver(e, proj.name)}
          ondragleave={onProjectDragLeave}
          ondrop={(e) => onProjectDrop(e, rootProjects)}
          ondragend={onProjectDragEnd}
          class:drag-over-top={dropTarget?.name === proj.name && dropTarget?.pos === "before"}
          class:drag-over-bottom={dropTarget?.name === proj.name && dropTarget?.pos === "after"}
        >
          <button
            class="project-item"
            class:active={$selectedProject === proj.name}
            onclick={() => selectProject(proj.name)}
          >
            <div class="project-main">
              <span class="state-dot" style="background:{getColor(proj.state)}"></span>
              <div class="project-info">
                <span class="project-name">{proj.name}</span>
                {#if proj.description}
                  <span class="project-desc">{proj.description}</span>
                {/if}
              </div>
            </div>
            <div class="project-meta">
              <span class="project-state">{proj.state}</span>
              {#if proj.containers.length > 0}
                <span class="container-count">{$tradN("sidebar.containers", proj.containers.length)}</span>
              {/if}
            </div>
          </button>
        </li>
      {/each}
    </div>

    {#if $projects.length === 0}
      <li class="empty">{$trad("sidebar.noProject")}</li>
    {/if}
  </ul>
  <CreateProjectModal bind:open={showCreateModal} />
</aside>

{#if termContextMenu}
  {@const tid = termContextMenu.id}
  <ContextMenu
    x={termContextMenu.x}
    y={termContextMenu.y}
    onClose={() => (termContextMenu = null)}
    items={[
      { label: $trad("common.rename"), action: () => (renamingTermId = tid) },
      { label: $trad("sidebar.closeTerminal"), danger: true, action: () => closeTerminalById(tid) },
    ]}
  />
{/if}

{#if contextMenu}
  {@const fid = contextMenu.id}
  <ContextMenu
    x={contextMenu.x}
    y={contextMenu.y}
    onClose={() => (contextMenu = null)}
    items={[
      { label: $trad("common.rename"), action: () => (renamingFolderId = fid) },
      { label: $trad("sidebar.deleteFolder"), danger: true, action: () => deleteFolder(fid) },
    ]}
  />
{/if}

<style>
  aside {
    width: var(--sidebar-width); min-width: var(--sidebar-width);
    background: var(--bg-secondary); border-right: 1px solid var(--border-color);
    overflow-y: auto; display: flex; flex-direction: column;
  }
  .sidebar-header {
    padding: 0.75rem 1rem; font-weight: 600; font-size: 0.85rem;
    color: var(--text-muted); text-transform: uppercase; letter-spacing: 0.05em;
    display: flex; align-items: center; justify-content: space-between;
  }
  .header-actions { display: flex; gap: 0.25rem; }

  /* Section Terminaux (raccourcis) */
  .terminals-header { padding-bottom: 0.25rem; border-bottom: 1px solid var(--border-color); }
  .section-toggle {
    background: none; border: none; cursor: pointer; padding: 0;
    color: var(--text-muted); font-weight: 600; font-size: 0.85rem;
    text-transform: uppercase; letter-spacing: 0.05em;
  }
  .section-toggle:hover { color: var(--text-primary); }
  .terminals-count {
    font-size: 0.7rem; background: var(--bg-tertiary); color: var(--text-secondary);
    padding: 0.05rem 0.45rem; border-radius: 10px;
  }
  .terminals-list { border-bottom: 1px solid var(--border-color); }
  .terminal-item {
    display: flex; align-items: center; gap: 0.5rem;
    width: 100%; padding: 0.35rem 1rem; border: none; background: none;
    color: var(--text-secondary); cursor: pointer; text-align: left; font-size: 0.82rem;
  }
  .terminal-item:hover { background: var(--bg-tertiary); color: var(--text-primary); }
  /* Gris = terminal normal, vert = un agent LLM (claude, codex...) tourne dedans */
  .term-dot {
    width: 7px; height: 7px; border-radius: 50%; flex-shrink: 0;
    background: var(--text-muted);
  }
  /* Icone Claude (✳) quand un agent IA tourne — plus parlant qu une pastille verte */
  .term-llm { width: 11px; height: 11px; flex-shrink: 0; }
  .terminal-name {
    font-weight: 600; overflow: hidden; text-overflow: ellipsis; white-space: nowrap;
    flex-shrink: 1; min-width: 0;
  }
  .terminal-project {
    margin-left: auto; flex-shrink: 0; font-size: 0.7rem; color: var(--text-muted);
    max-width: 45%; overflow: hidden; text-overflow: ellipsis; white-space: nowrap;
  }
  .add-btn {
    width: 22px; height: 22px; border-radius: 4px; border: 1px solid var(--border-color);
    background: var(--bg-tertiary); color: var(--text-secondary); cursor: pointer;
    font-size: 0.75rem; line-height: 1; display: flex; align-items: center; justify-content: center;
    padding: 0;
  }
  .add-btn:hover { background: var(--accent); color: white; border-color: var(--accent); }
  /* Libelles explicites : le 📁 seul n etait pas compris (retour utilisateur 2026-08-14) */
  .add-btn.labeled { width: auto; padding: 0 0.5rem; font-size: 0.72rem; font-weight: 600; }

  .folder-create {
    display: flex; gap: 0.25rem; padding: 0.25rem 0.75rem; align-items: center;
  }
  .folder-create input {
    flex: 1; padding: 0.2rem 0.4rem; border: 1px solid var(--border-color);
    border-radius: 4px; background: var(--bg-primary); color: var(--text-primary); font-size: 0.8rem;
  }
  .folder-create button {
    padding: 0.2rem 0.4rem; border: 1px solid var(--border-color); border-radius: 4px;
    background: var(--bg-tertiary); color: var(--text-secondary); cursor: pointer; font-size: 0.75rem;
  }
  .folder-create .cancel { color: var(--error); }

  ul { list-style: none; padding: 0; margin: 0; }

  .folder-item { border-bottom: 1px solid var(--border-color); }
  .folder-header {
    display: flex; align-items: center; gap: 0.25rem;
    padding: 0.4rem 0.5rem; font-size: 0.8rem; color: var(--text-secondary);
    background: var(--bg-tertiary);
  }
  .folder-toggle {
    background: none; border: none; cursor: pointer; color: var(--text-muted);
    font-size: 0.75rem; padding: 0; width: 16px; text-align: center;
  }
  .folder-name { flex: 1; font-weight: 600; cursor: default; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .folder-count { font-size: 0.7rem; color: var(--text-muted); }
  /* Corbeille discrete : visible au survol de l'en-tete (et au focus clavier).
     Le clic sur un dossier plein EXPLIQUE au lieu de supprimer — le bouton reste donc
     toujours cliquable, pas de disabled muet. */
  .folder-delete {
    background: none; border: none; cursor: pointer; padding: 0 0.2rem;
    font-size: 0.75rem; color: var(--text-muted); line-height: 1;
    opacity: 0; transition: opacity 0.1s ease;
  }
  .folder-header:hover .folder-delete, .folder-delete:focus-visible { opacity: 1; }
  .folder-delete:hover { color: var(--error); }

  .folder-projects { padding-left: 0.5rem; }
  .root-drop-zone { min-height: 2rem; }

  .project-item {
    display: flex; flex-direction: column; gap: 0.2rem;
    width: 100%; padding: 0.6rem 1rem; border: none; background: none;
    color: var(--text-primary); cursor: pointer; text-align: left;
    border-bottom: 1px solid var(--border-color);
  }
  .project-item:hover { background: var(--bg-tertiary); }
  .project-item.active { background: var(--bg-tertiary); border-left: 3px solid var(--accent); }
  .project-main { display: flex; align-items: flex-start; gap: 0.5rem; }
  .state-dot { width: 8px; height: 8px; border-radius: 50%; flex-shrink: 0; margin-top: 0.3rem; }
  .project-info { display: flex; flex-direction: column; flex: 1; min-width: 0; }
  .project-name { font-size: 0.9rem; font-weight: 600; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .project-desc {
    font-size: 0.75rem; color: var(--text-muted); overflow: hidden;
    text-overflow: ellipsis; white-space: nowrap; margin-top: 0.1rem;
  }
  .project-meta { display: flex; align-items: center; gap: 0.5rem; padding-left: 1.1rem; }
  .project-state { font-size: 0.7rem; color: var(--text-muted); }
  .container-count { font-size: 0.7rem; color: var(--text-secondary); }
  .empty { padding: 1rem; color: var(--text-muted); font-size: 0.85rem; }

  li[draggable="true"] { cursor: grab; border-top: 2px solid transparent; border-bottom: 2px solid transparent; transition: border-color 0.1s; }
  li[draggable="true"]:active { cursor: grabbing; }
  li.drag-over-top { border-top-color: var(--accent); }
  li.drag-over-bottom { border-bottom-color: var(--accent); }
</style>
