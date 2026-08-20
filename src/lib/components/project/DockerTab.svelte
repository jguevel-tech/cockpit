<script lang="ts">
  import { projects } from "../../stores/projects";
  import { startProject, stopProject, restartProject } from "../../api/docker";
  import { createTerminal } from "../../api/workspace";
  import { activeTab, pendingTerminalId } from "../../stores/ui";
  import { notify } from "../../stores/toast";
  import ContainerLogsModal from "../docker/ContainerLogsModal.svelte";
  import type { Project } from "../../types";
  import { trad } from "../../i18n";

  let { name }: { name: string } = $props();
  let loading = $state("");
  let logsFor: string | null = $state(null);

  let project: Project | undefined = $derived($projects.find(p => p.name === name));

  async function doStart() {
    loading = "starting"; try { await startProject(name); } catch(e) { notify(String(e)); } finally { loading = ""; }
  }
  async function doStop() {
    loading = "stopping"; try { await stopProject(name); } catch(e) { notify(String(e)); } finally { loading = ""; }
  }
  async function doRestart() {
    loading = "restarting"; try { await restartProject(name); } catch(e) { notify(String(e)); } finally { loading = ""; }
  }

  /// Ouvre un shell DANS le conteneur : un vrai terminal Cockpit du projet, avec
  /// docker exec injecte (bash si l'image en a un, sinon sh).
  async function openShell(containerName: string) {
    if (!project?.path) { notify($trad("docker.noPathConfigured")); return; }
    try {
      const cmd = `docker exec -it ${containerName} sh -c '[ -x /bin/bash ] && exec bash || exec sh'`;
      const tid = await createTerminal(name, project.path, 80, 24, cmd);
      pendingTerminalId.set(tid);
      activeTab.set("terminal");
    } catch (e) { notify(String(e)); }
  }

  const composeHint = $derived($trad("docker.composeMissingHint"));

  const stateColors: Record<string, string> = {
    running: "var(--success)", starting: "var(--warning)",
    stopping: "var(--warning)", error: "var(--error)", stopped: "var(--text-muted)",
  };
</script>

{#if project}
<div class="docker-tab">
  {#if !project.path}
    <p class="no-docker">{$trad("docker.noPath")}</p>
  {:else}
  {#if !project.has_compose}
    <!-- Le fichier compose est optionnel dans Cockpit : cet ecran doit expliquer l'absence,
         pas laisser docker repondre "no configuration file provided: not found". -->
    <div class="notice">
      <p>
        {$trad("docker.noComposeIn")} <code>{project.path}</code>. {$trad("docker.noComposeWhy")}
      </p>
      <p class="notice-fix">
        {$trad("docker.noComposeFixBefore")} <code>docker-compose.yml</code>
        {$trad("docker.noComposeFixAfter")}
      </p>
      <button class="btn" onclick={() => activeTab.set("settings")}>{$trad("docker.openProjectSettings")}</button>
    </div>
  {/if}
  <div class="controls">
    <span class="state-badge" style="color:{stateColors[project.state]}">{project.state}</span>
    <button class="btn btn-success" onclick={doStart} disabled={!!loading || !project.has_compose || project.state === 'running'} title={project.has_compose ? '' : composeHint}>
      {loading === 'starting' ? '...' : 'Start'}
    </button>
    <button class="btn btn-danger" onclick={doStop} disabled={!!loading || !project.has_compose || project.state === 'stopped'} title={project.has_compose ? '' : composeHint}>
      {loading === 'stopping' ? '...' : 'Stop'}
    </button>
    <button class="btn" onclick={doRestart} disabled={!!loading || !project.has_compose} title={project.has_compose ? '' : composeHint}>
      {loading === 'restarting' ? '...' : 'Restart'}
    </button>
  </div>

  {#if project.error}
    <div class="error-box">{project.error}</div>
  {/if}

  {#if project.depends_on.length > 0}
    <div class="deps">
      <strong>{$trad("docker.dependsOn")}</strong>
      {#each project.depends_on as dep}<span class="dep-badge">{dep}</span>{/each}
    </div>
  {/if}

  {#if project.depended_by.length > 0}
    <div class="deps">
      <strong>{$trad("docker.requiredBy")}</strong>
      {#each project.depended_by as dep}<span class="dep-badge">{dep}</span>{/each}
    </div>
  {/if}

  {#if project.containers.length > 0}
    <h3>{$trad("docker.containers")}</h3>
    <table>
      <thead><tr><th>{$trad("docker.colService")}</th><th>{$trad("docker.colName")}</th><th>{$trad("docker.colStatus")}</th><th>{$trad("docker.colHealth")}</th><th>{$trad("docker.colPorts")}</th><th></th></tr></thead>
      <tbody>
        {#each project.containers as c}
          <tr>
            <td>{c.service}</td>
            <td>{c.name}</td>
            <td><span class="status" class:running={c.status === 'running'}>{c.status}</span></td>
            <td>{c.health || '-'}</td>
            <td class="ports">{c.ports || '-'}</td>
            <td class="row-actions">
              <button class="mini" onclick={() => (logsFor = c.name)} title={$trad("docker.logsHint")}>{$trad("docker.logs")}</button>
              {#if c.status === 'running'}
                <button class="mini" onclick={() => openShell(c.name)} title={$trad("docker.shellHint")}>{$trad("docker.shell")}</button>
              {/if}
            </td>
          </tr>
        {/each}
      </tbody>
    </table>
  {:else}
    <p class="no-containers">{$trad("docker.noContainer")}</p>
  {/if}
  {/if}
</div>
{/if}

{#if logsFor}
  <ContainerLogsModal id={logsFor} name={logsFor} onClose={() => (logsFor = null)} />
{/if}

<style>
  .docker-tab { max-width: 800px; }
  .row-actions { display: flex; gap: 0.3rem; }
  .mini {
    padding: 0.15rem 0.5rem; border: 1px solid var(--border-color); border-radius: 5px;
    cursor: pointer; font-size: 0.75rem; background: var(--bg-secondary); color: var(--text-secondary);
  }
  .mini:hover { border-color: var(--accent); color: var(--text-primary); }
  .controls { display: flex; align-items: center; gap: 0.5rem; margin-bottom: 1rem; }
  .state-badge { font-weight: 600; font-size: 0.9rem; text-transform: uppercase; }
  .btn {
    padding: 0.4rem 0.8rem; border: 1px solid var(--border-color); border-radius: 6px;
    cursor: pointer; font-size: 0.85rem; background: var(--bg-secondary); color: var(--text-primary);
  }
  .btn:hover { background: var(--bg-tertiary); }
  .btn:disabled { opacity: 0.5; cursor: not-allowed; }
  .btn-success { border-color: var(--success); color: var(--success); }
  .btn-danger { border-color: var(--error); color: var(--error); }
  .error-box { background: color-mix(in srgb, var(--error) 10%, transparent); border: 1px solid var(--error); padding: 0.5rem; border-radius: 6px; margin-bottom: 1rem; color: var(--error); font-size: 0.85rem; }
  .deps { margin-bottom: 0.5rem; font-size: 0.85rem; display: flex; align-items: center; gap: 0.5rem; flex-wrap: wrap; }
  .dep-badge { background: var(--bg-tertiary); padding: 0.15rem 0.5rem; border-radius: 4px; font-size: 0.8rem; }
  h3 { margin: 1rem 0 0.5rem; font-size: 1rem; }
  table { width: 100%; border-collapse: collapse; font-size: 0.85rem; }
  th, td { text-align: left; padding: 0.4rem 0.6rem; border-bottom: 1px solid var(--border-color); }
  th { color: var(--text-muted); font-weight: 600; }
  .status { color: var(--text-muted); }
  .status.running { color: var(--success); }
  .ports { font-family: monospace; font-size: 0.8rem; }
  .no-containers { color: var(--text-muted); font-size: 0.9rem; }
  .no-docker { color: var(--text-muted); font-size: 0.9rem; font-style: italic; }
  .notice {
    background: color-mix(in srgb, var(--warning) 10%, transparent);
    border: 1px solid var(--warning); border-radius: 6px;
    padding: 0.7rem 0.8rem; margin-bottom: 1rem; font-size: 0.85rem;
    color: var(--text-primary); display: flex; flex-direction: column;
    align-items: flex-start; gap: 0.5rem;
  }
  .notice p { margin: 0; line-height: 1.45; }
  .notice-fix { color: var(--text-secondary); }
  .notice code {
    font-family: monospace; font-size: 0.8rem;
    background: var(--bg-tertiary); padding: 0.05rem 0.3rem; border-radius: 4px;
  }
</style>
