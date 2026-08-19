<script lang="ts">
  import { activeTab, selectProject, pendingTerminalId } from "../../stores/ui";
  import { projects, loadProjects } from "../../stores/projects";
  import { recordingStatus, lastRecordingEvent } from "../../stores/recording";
  import { getUrls, getProjectCommands, checkUrls } from "../../api/storage";
  import { createTerminal } from "../../api/workspace";
  import { renameProject } from "../../api/scanner";
  import { startRecording, stopRecording, getFailedRecordings, retryRecording, deleteRecording } from "../../api/recorder";
  import ContextMenu from "../ui/ContextMenu.svelte";
  import type { Url, UrlHealth, Recording, ProjectCommand } from "../../types";
  import { onMount } from "svelte";
  import DockerTab from "./DockerTab.svelte";
  import WorkspaceTab from "./WorkspaceTab.svelte";
  import SettingsTab from "./SettingsTab.svelte";
  import PluginsTab from "./PluginsTab.svelte";
  import TerminalTab from "./TerminalTab.svelte";
  import FilesTab from "./FilesTab.svelte";
  import GitTab from "./GitTab.svelte";
  import InlineEdit from "../ui/InlineEdit.svelte";
  import { notify } from "../../stores/toast";
  import { trad } from "../../i18n";
  import { signalerErreur } from "../../stores/errors";

  let { name }: { name: string } = $props();
  let urls: Url[] = $state([]);
  let renaming = $state(false);

  let project = $derived($projects.find(p => p.name === name));

  // --- Enregistrement de reunion ---
  let failedRecordings: Recording[] = $state([]);
  let recBusy = $state(false);
  let recDoneFlash = $state(false);
  let now = $state(Date.now());

  let rec = $derived($recordingStatus);
  let recHere = $derived(rec?.project === name ? rec : null);
  let recElapsed = $derived.by(() => {
    if (!recHere || recHere.state !== "recording") return "";
    const start = new Date(recHere.started_at.replace(" ", "T")).getTime();
    const total = Math.max(0, Math.floor((now - start) / 1000));
    const h = Math.floor(total / 3600), m = Math.floor((total % 3600) / 60), s = total % 60;
    const mm = String(m).padStart(2, "0"), ss = String(s).padStart(2, "0");
    return h > 0 ? `${h}:${mm}:${ss}` : `${mm}:${ss}`;
  });

  // Statut up/down des liens rapides (pastille verte/rouge), re-verifie toutes les 60 s
  let urlHealth = $state(new Map<string, UrlHealth>());

  async function checkQuickUrls() {
    if (urls.length === 0) return;
    try {
      const res = await checkUrls(urls.map((u) => u.url));
      const next = new Map<string, UrlHealth>();
      urls.forEach((u, i) => { if (res[i]) next.set(u.url, res[i]); });
      urlHealth = next;
    } catch (e) { console.error("checkUrls:", e); }
  }

  onMount(() => {
    (async () => {
      try { urls = await getUrls(name); } catch (e) {
      signalerErreur("projectDetail.next", String(e));}
      await checkQuickUrls();
      await loadFailedRecordings();
    })();
    const timer = setInterval(() => { now = Date.now(); }, 1000);
    const healthTimer = setInterval(checkQuickUrls, 60_000);
    return () => { clearInterval(timer); clearInterval(healthTimer); };
  });

  async function loadFailedRecordings() {
    try { failedRecordings = await getFailedRecordings(name); } catch (e) {
      signalerErreur("projectDetail.loadFailedRecordings", String(e));}
  }

  $effect(() => {
    const ev = $lastRecordingEvent;
    if (!ev || ev.project !== name) return;
    if (ev.state === "done" || ev.state === "error") {
      loadFailedRecordings();
      if (ev.state === "done") {
        recDoneFlash = true;
        setTimeout(() => { recDoneFlash = false; }, 5000);
      }
    }
  });

  async function toggleRecording() {
    recBusy = true;
    try {
      if (recHere?.state === "recording") await stopRecording();
      else await startRecording(name);
    } catch (e) { notify(String(e)); }
    finally { recBusy = false; }
  }

  async function doRetryRecording(id: number) {
    try { await retryRecording(id); await loadFailedRecordings(); } catch (e) { notify(String(e)); }
  }

  async function doDeleteRecording(id: number) {
    if (!confirm($trad("rec.deleteConfirm"))) return;
    try { await deleteRecording(id); await loadFailedRecordings(); } catch (e) { notify(String(e)); }
  }

  // --- Commandes rapides (bouton ▶) ---
  // Rechargees a CHAQUE ouverture du menu : une commande ajoutee dans Parametres
  // est disponible immediatement, sans dependre d'un remount.
  let cmdMenu: { x: number; y: number; cmds: ProjectCommand[] } | null = $state(null);

  async function openCmdMenu(e: MouseEvent) {
    try {
      const cmds = await getProjectCommands(name);
      if (cmds.length === 0) {
        notify($trad("project.noQuickCommand"), "info");
        return;
      }
      cmdMenu = { x: e.clientX, y: e.clientY, cmds };
    } catch (e2) { notify(String(e2)); }
  }

  async function runCommand(c: ProjectCommand) {
    if (!project?.path) { notify($trad("project.noPath")); return; }
    try {
      const tid = await createTerminal(name, project.path, 80, 24, c.command);
      pendingTerminalId.set(tid);
      activeTab.set("terminal");
    } catch (e) { notify(String(e)); }
  }

  function startRename() {
    renaming = true;
  }

  async function commitRename(next: string) {
    renaming = false;
    if (next === name) return;
    try {
      await renameProject(name, next);
      await loadProjects();
      selectProject(next);
    } catch (e) { notify(String(e)); }
  }

  // Ajouter un onglet = une seule entree ici (+ le type activeTab dans ui.ts)
  const tabs = [
    { id: "workspace" as const, labelKey: "tab.workspace" as const, component: WorkspaceTab },
    { id: "docker" as const, labelKey: "tab.docker" as const, component: DockerTab },
    { id: "terminal" as const, labelKey: "tab.terminal" as const, component: TerminalTab },
    { id: "files" as const, labelKey: "tab.files" as const, component: FilesTab },
    { id: "git" as const, labelKey: "tab.git" as const, component: GitTab },
    { id: "plugins" as const, labelKey: "tab.plugins" as const, component: PluginsTab },
    { id: "settings" as const, labelKey: "tab.settings" as const, component: SettingsTab },
  ];
  let CurrentTab = $derived(tabs.find((t) => t.id === $activeTab)?.component ?? WorkspaceTab);
</script>

<div class="detail">
  <!-- UNE seule barre : titre a gauche, onglets, puis les actions — Enregistrer tout au bout.
       Les deux bandeaux d'avant (titre puis onglets) empilaient deux jeux de coins arrondis. -->
  <div class="project-bar">
    {#if renaming}
      <span class="title-edit">
        <InlineEdit value={name} onCommit={commitRename} onCancel={() => (renaming = false)} />
      </span>
    {:else}
      <!-- svelte-ignore a11y_no_static_element_interactions -->
      <h2
        ondblclick={startRename}
        title={project?.description ? `${project.description} — double-clic pour renommer` : "Double-clic pour renommer"}
      >{name}</h2>
    {/if}

    <div class="tabs">
      {#each tabs as tab}
        <button
          class="tab" class:active={$activeTab === tab.id}
          onclick={() => activeTab.set(tab.id)}
        >{$trad(tab.labelKey)}</button>
      {/each}
    </div>

    <div class="header-actions">
      <button class="cmd-btn" onclick={openCmdMenu} title={$trad("project.runCommandHint")}>{$trad("project.runCommand")}</button>
      {#if urls.length > 0}
        {#each urls as u}
          {@const h = urlHealth.get(u.url)}
          <a
            class="quick-url"
            href={u.url}
            target="_blank"
            rel="noopener noreferrer"
            title={h ? (h.ok ? $trad("project.urlOnline", { status: h.status }) : $trad("project.urlUnreachable", { detail: h.error || `HTTP ${h.status}` })) : u.url}
          >
            <span class="url-dot" class:up={h?.ok} class:down={h && !h.ok}></span>{u.label}
          </a>
        {/each}
      {/if}
      {#if failedRecordings.length > 0}
        <span class="rec-failed" title={failedRecordings[0].error ?? ""}>
          {$trad("rec.failed")}
          <button class="rec-failed-action" onclick={() => doRetryRecording(failedRecordings[0].id)} title={$trad("rec.retryTranscription")}>↻</button>
          <button class="rec-failed-action" onclick={() => doDeleteRecording(failedRecordings[0].id)} title={$trad("common.delete")}>✕</button>
        </span>
      {/if}
      {#if recHere?.state === "recording"}
        <span class="rec-timer"><span class="rec-dot"></span>{recElapsed}</span>
        <button class="rec-btn stop" onclick={toggleRecording} disabled={recBusy} title={$trad("rec.stopHint")}>{$trad("rec.stop")}</button>
      {:else if recHere}
        <span class="rec-pipeline">
          {recHere.state === "transcribing" ? $trad("rec.transcribing") : $trad("rec.summarizing")}
        </span>
      {:else if recDoneFlash}
        <span class="rec-done">{$trad("rec.noteCreated")}</span>
      {:else}
        <button
          class="rec-btn"
          onclick={toggleRecording}
          disabled={recBusy || !!rec}
          title={rec ? $trad("rec.busyElsewhere", { project: rec.project }) : $trad("rec.startHint")}
        >{$trad("rec.start")}</button>
      {/if}
    </div>
  </div>

  <div class="tab-content">
    <CurrentTab {name} />
  </div>
</div>

{#if cmdMenu}
  <ContextMenu
    x={cmdMenu.x}
    y={cmdMenu.y}
    items={cmdMenu.cmds.map((c) => ({ label: c.label, action: () => runCommand(c) }))}
    onClose={() => (cmdMenu = null)}
  />
{/if}

<style>
  .detail { display: flex; flex-direction: column; height: 100%; }
  /* Barre unique : les enfants s'etirent pour que le soulignement des onglets touche la
     bordure basse ; titre et actions se recentrent individuellement. */
  .project-bar {
    display: flex; align-items: stretch; gap: 1rem; flex-wrap: wrap;
    border-bottom: 1px solid var(--border-color);
  }
  .project-bar h2 {
    margin: 0; font-size: 1.05rem; cursor: default; align-self: center;
    white-space: nowrap; padding-bottom: 2px;
  }
  .title-edit { display: inline-block; width: 14rem; font-size: 1.05rem; font-weight: 700; align-self: center; }
  .header-actions {
    display: flex; gap: 0.5rem; flex-wrap: wrap; margin-left: auto;
    align-items: center; align-self: center; padding-bottom: 2px;
  }
  .rec-btn {
    font-size: 0.8rem; padding: 0.15rem 0.6rem; border: 1px solid var(--error);
    border-radius: var(--radius-sm); background: var(--bg-secondary); color: var(--error); cursor: pointer;
    transition: background 0.12s ease, color 0.12s ease;
  }
  .rec-btn:hover:not(:disabled) { background: var(--error); color: white; }
  .rec-btn:disabled { opacity: 0.5; cursor: default; }
  .rec-btn.stop { background: var(--error); color: white; }
  .rec-timer {
    display: inline-flex; align-items: center; gap: 0.4rem;
    font-family: var(--font-mono); font-size: 0.85rem; color: var(--error); font-weight: 700;
  }
  .rec-dot {
    width: 8px; height: 8px; border-radius: 50%; background: var(--error);
    animation: rec-pulse 1.2s ease-in-out infinite;
  }
  @keyframes rec-pulse { 0%, 100% { opacity: 1; } 50% { opacity: 0.25; } }
  .rec-pipeline { font-size: 0.8rem; color: var(--accent); font-style: italic; }
  .rec-done { font-size: 0.8rem; color: var(--success); }
  .rec-failed {
    display: inline-flex; align-items: center; gap: 0.3rem;
    font-size: 0.75rem; color: var(--error);
    border: 1px solid var(--error); border-radius: var(--radius-sm); padding: 0.1rem 0.4rem;
    background: var(--error-soft);
  }
  .rec-failed-action {
    background: none; border: none; color: inherit; cursor: pointer;
    font-size: 0.8rem; padding: 0 0.15rem;
  }
  .rec-failed-action:hover { opacity: 0.7; }
  .cmd-btn {
    font-size: 0.8rem; padding: 0.15rem 0.6rem; border: 1px solid var(--border-color);
    border-radius: var(--radius-sm); background: var(--bg-secondary); color: var(--accent);
    cursor: pointer; transition: background 0.12s ease;
  }
  .cmd-btn:hover { background: var(--bg-tertiary); }
  .quick-url {
    font-size: 0.8rem; color: var(--accent); text-decoration: none;
    padding: 0.15rem 0.5rem; border: 1px solid var(--border-color);
    border-radius: var(--radius-sm); background: var(--bg-secondary);
    transition: background 0.12s ease;
  }
  .quick-url:hover { background: var(--bg-tertiary); text-decoration: underline; }
  .url-dot {
    display: inline-block; width: 7px; height: 7px; border-radius: 50%;
    margin-right: 0.35rem; background: var(--text-muted); opacity: 0.5; vertical-align: middle;
  }
  .url-dot.up { background: var(--success); opacity: 1; }
  .url-dot.down { background: var(--error); opacity: 1; }
  .tabs { display: flex; gap: 0; align-items: stretch; }
  .tab {
    display: flex; align-items: center;
    padding: 0.55rem 0.9rem; border: none; background: none; color: var(--text-secondary);
    cursor: pointer; font-size: 0.9rem; border-bottom: 2px solid transparent;
  }
  .tab:hover { color: var(--text-primary); }
  .tab.active { color: var(--accent); border-bottom-color: var(--accent); }
  .tab-content { flex: 1; overflow-y: auto; margin-top: 1rem; }
</style>
