<script lang="ts">
  import { projects } from "../../stores/projects";
  import { startProject, stopProject, restartProject } from "../../api/docker";
  import { createTerminal } from "../../api/workspace";
  import { activeTab, pendingTerminalId } from "../../stores/ui";
  import { notify } from "../../stores/toast";
  import ContainerLogsModal from "../docker/ContainerLogsModal.svelte";
  import type { Project } from "../../types";

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
    if (!project?.path) { notify("Ce projet n'a pas de chemin configuré."); return; }
    try {
      const cmd = `docker exec -it ${containerName} sh -c '[ -x /bin/bash ] && exec bash || exec sh'`;
      const tid = await createTerminal(name, project.path, 80, 24, cmd);
      pendingTerminalId.set(tid);
      activeTab.set("terminal");
    } catch (e) { notify(String(e)); }
  }

  const stateColors: Record<string, string> = {
    running: "var(--success)", starting: "var(--warning)",
    stopping: "var(--warning)", error: "var(--error)", stopped: "var(--text-muted)",
  };
</script>

{#if project}
<div class="docker-tab">
  {#if !project.path}
    <p class="no-docker">Ce projet n'a pas de repertoire configure. Les controles Docker ne sont pas disponibles.</p>
  {:else}
  <div class="controls">
    <span class="state-badge" style="color:{stateColors[project.state]}">{project.state}</span>
    <button class="btn btn-success" onclick={doStart} disabled={!!loading || project.state === 'running'}>
      {loading === 'starting' ? '...' : 'Start'}
    </button>
    <button class="btn btn-danger" onclick={doStop} disabled={!!loading || project.state === 'stopped'}>
      {loading === 'stopping' ? '...' : 'Stop'}
    </button>
    <button class="btn" onclick={doRestart} disabled={!!loading}>
      {loading === 'restarting' ? '...' : 'Restart'}
    </button>
  </div>

  {#if project.error}
    <div class="error-box">{project.error}</div>
  {/if}

  {#if project.depends_on.length > 0}
    <div class="deps">
      <strong>Depend de:</strong>
      {#each project.depends_on as dep}<span class="dep-badge">{dep}</span>{/each}
    </div>
  {/if}

  {#if project.depended_by.length > 0}
    <div class="deps">
      <strong>Requis par:</strong>
      {#each project.depended_by as dep}<span class="dep-badge">{dep}</span>{/each}
    </div>
  {/if}

  {#if project.containers.length > 0}
    <h3>Conteneurs</h3>
    <table>
      <thead><tr><th>Service</th><th>Nom</th><th>Statut</th><th>Health</th><th>Ports</th><th></th></tr></thead>
      <tbody>
        {#each project.containers as c}
          <tr>
            <td>{c.service}</td>
            <td>{c.name}</td>
            <td><span class="status" class:running={c.status === 'running'}>{c.status}</span></td>
            <td>{c.health || '-'}</td>
            <td class="ports">{c.ports || '-'}</td>
            <td class="row-actions">
              <button class="mini" onclick={() => (logsFor = c.name)} title="Voir les logs">Logs</button>
              {#if c.status === 'running'}
                <button class="mini" onclick={() => openShell(c.name)} title="Ouvrir un shell dans le conteneur">Shell</button>
              {/if}
            </td>
          </tr>
        {/each}
      </tbody>
    </table>
  {:else}
    <p class="no-containers">Aucun conteneur actif</p>
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
</style>
