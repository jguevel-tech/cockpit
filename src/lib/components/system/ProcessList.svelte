<script lang="ts">
  import { killProcess } from "../../api/system";
  import type { ProcessMetrics } from "../../types";
  import { trad } from "../../i18n";
  import { notify } from "../../stores/toast";
  import { demanderConfirmation } from "../../stores/confirm";
  import { formatBytes } from "../../utils/format";

  // `killForced` vient du backend (`kill_is_forced`) et non d'une detection de systeme cote
  // interface : c'est lui qui sait quel signal il peut envoyer. Sous Windows il n'y a que
  // `taskkill /F`, donc le bouton doit ANNONCER qu'il force — sinon un utilisateur habitue
  // a Cockpit sous Linux croit que son editeur va pouvoir enregistrer.
  let {
    topCpu,
    topMemory,
    killForced = false,
  }: { topCpu: ProcessMetrics[]; topMemory: ProcessMetrics[]; killForced?: boolean } = $props();
  let tab: "cpu" | "memory" = $state("cpu");
  let arretEnCours: number | null = $state(null);
  let libelleArret = $derived($trad(killForced ? "proc.stopForced" : "proc.stop"));

  async function kill(pid: number) {
    if (!(await demanderConfirmation({ message: $trad("proc.killConfirm", { pid }), action: $trad("proc.stopAction") }))) return;
    // Le bouton se desactive pendant l'appel : sans ca un clic sans effet visible passe
    // pour un bug, et un second clic partait sur un PID deja mort.
    arretEnCours = pid;
    try {
      await killProcess(pid);
    } catch (e) {
      // `notify` journalise et remonte tout seul : pas de `signalerErreur` en double.
      notify($trad("proc.stopFailed", { pid, detail: String(e) }), "error", 4000, {
        scope: "process.kill",
      });
    } finally {
      arretEnCours = null;
    }
  }
</script>

<div class="processes">
  <div class="proc-tabs">
    <button class:active={tab === 'cpu'} onclick={() => tab = 'cpu'}>{$trad("proc.topCpu")}</button>
    <button class:active={tab === 'memory'} onclick={() => tab = 'memory'}>{$trad("proc.topMemory")}</button>
  </div>

  <table>
    <thead>
      <tr>
        <th>{$trad("proc.pid")}</th><th>{$trad("proc.name")}</th><th>{$trad("proc.user")}</th><th>{$trad("proc.cpu")}</th><th>{$trad("proc.mem")}</th><th>{$trad("proc.rss")}</th>
        {#if tab === 'memory'}<th>{$trad("proc.instances")}</th>{/if}
        <th></th>
      </tr>
    </thead>
    <tbody>
      {#each (tab === 'cpu' ? topCpu : topMemory) as proc}
        <tr>
          <td class="mono">{proc.pid}</td>
          <td>{proc.name}</td>
          <td>{proc.user}</td>
          <td>{proc.cpu.toFixed(1)}</td>
          <td>{proc.memory.toFixed(1)}</td>
          <td>{formatBytes(proc.memory_rss)}</td>
          {#if tab === 'memory'}<td>{proc.count || 1}</td>{/if}
          <td>
            <button
              class="kill-btn"
              onclick={() => kill(proc.pid)}
              disabled={arretEnCours !== null}
              aria-label={libelleArret}
              title={libelleArret}
            >{arretEnCours === proc.pid ? "…" : "✕"}</button>
          </td>
        </tr>
      {/each}
    </tbody>
  </table>
</div>

<style>
  .processes { margin-top: 1rem; }
  .proc-tabs { display: flex; gap: 0; margin-bottom: 0.5rem; }
  .proc-tabs button {
    padding: 0.4rem 0.8rem; border: 1px solid var(--border-color); background: var(--bg-secondary);
    color: var(--text-secondary); cursor: pointer; font-size: 0.85rem;
  }
  .proc-tabs button:first-child { border-radius: 6px 0 0 6px; }
  .proc-tabs button:last-child { border-radius: 0 6px 6px 0; }
  .proc-tabs button.active { background: var(--accent); color: white; border-color: var(--accent); }
  table { width: 100%; border-collapse: collapse; font-size: 0.8rem; }
  th, td { text-align: left; padding: 0.3rem 0.5rem; border-bottom: 1px solid var(--border-color); }
  th { color: var(--text-muted); font-weight: 600; }
  .mono { font-family: monospace; }
  .kill-btn { background: none; border: none; color: var(--error); cursor: pointer; opacity: 0.3; font-size: 0.8rem; }
  .kill-btn:hover { opacity: 1; }
  .kill-btn:disabled { cursor: progress; opacity: 0.6; }
</style>
