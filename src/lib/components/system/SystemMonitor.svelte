<script lang="ts">
  import { systemMetrics, metricsLive, refreshMetrics, startLiveMetrics, stopLiveMetrics } from "../../stores/system";
  import ProcessList from "./ProcessList.svelte";
  import { trad } from "../../i18n";

  let isLive: boolean = $derived($metricsLive);

  function formatBytes(bytes: number): string {
    if (bytes === 0) return "0 B";
    const k = 1024;
    const sizes = ["B", "KB", "MB", "GB", "TB"];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return (bytes / Math.pow(k, i)).toFixed(1) + " " + sizes[i];
  }
</script>

<div class="system">
  <div class="sys-header">
    <h2>{$trad("sys.title")}</h2>
    <div class="metrics-controls">
      <button class="metrics-btn" onclick={refreshMetrics}>{$trad("sys.snapshot")}</button>
      {#if isLive}
        <button class="metrics-btn live-active" onclick={stopLiveMetrics}>{$trad("sys.liveOn")}</button>
      {:else}
        <button class="metrics-btn" onclick={startLiveMetrics}>{$trad("sys.liveOff")}</button>
      {/if}
    </div>
  </div>

  {#if $systemMetrics}
    {@const m = $systemMetrics}

    <div class="info-row">
      <span>{m.hostname}</span>
      <span>Kernel {m.kernel_version}</span>
      <span>Uptime {m.uptime}</span>
    </div>

    <div class="metrics-grid">
      <!-- CPU -->
      <div class="metric-card">
        <h3>{$trad("sys.cpuCores", { count: m.cpu.cores })}</h3>
        <div class="bar-container">
          <div class="bar" style="width:{m.cpu.usage_percent}%"></div>
        </div>
        <span class="metric-value">{m.cpu.usage_percent.toFixed(1)}%</span>
        <div class="per-core">
          {#each m.cpu.per_core as core, i}
            <div class="mini-bar-wrap" title={$trad("sys.core", { index: i, percent: core.toFixed(0) })}>
              <div class="mini-bar" style="height:{core}%"></div>
            </div>
          {/each}
        </div>
      </div>

      <!-- Memory -->
      <div class="metric-card">
        <h3>{$trad("sys.memory")}</h3>
        <div class="bar-container">
          <div class="bar" style="width:{m.memory.percent}%" class:warning={m.memory.percent > 80}></div>
        </div>
        <span class="metric-value">{formatBytes(m.memory.used)} / {formatBytes(m.memory.total)} ({m.memory.percent.toFixed(1)}%)</span>
        {#if m.memory.swap_total > 0}
          <span class="metric-sub">Swap: {formatBytes(m.memory.swap_used)} / {formatBytes(m.memory.swap_total)}</span>
        {/if}
      </div>

      <!-- Disks -->
      {#each m.disks as disk}
        <div class="metric-card">
          <h3>{disk.mount} ({disk.device})</h3>
          <div class="bar-container">
            <div class="bar" style="width:{disk.percent}%" class:warning={disk.percent > 90}></div>
          </div>
          <span class="metric-value">{formatBytes(disk.used)} / {formatBytes(disk.total)} ({disk.percent.toFixed(1)}%)</span>
        </div>
      {/each}
    </div>

    <ProcessList topCpu={m.top_cpu} topMemory={m.top_memory} />

  {:else}
    <p class="no-data">{$trad("sys.startHint")}</p>
  {/if}
</div>

<style>
  .system { max-width: 1000px; }
  .sys-header { display: flex; align-items: center; justify-content: space-between; margin-bottom: 1rem; }
  h2 { margin: 0; }
  .metrics-controls { display: flex; gap: 0.4rem; }
  .metrics-btn {
    padding: 0.3rem 0.8rem; border: 1px solid var(--border-color); border-radius: 4px;
    background: var(--bg-secondary); color: var(--text-secondary); cursor: pointer; font-size: 0.8rem;
  }
  .metrics-btn:hover { background: var(--bg-tertiary); color: var(--text-primary); }
  .metrics-btn.live-active { background: var(--success); color: white; border-color: var(--success); }
  .no-data { color: var(--text-muted); font-size: 0.9rem; }
  .info-row { display: flex; gap: 1.5rem; margin-bottom: 1rem; font-size: 0.85rem; color: var(--text-secondary); }
  .metrics-grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(280px, 1fr)); gap: 1rem; margin-bottom: 1.5rem; }
  .metric-card { background: var(--bg-secondary); border: 1px solid var(--border-color); border-radius: 8px; padding: 1rem; }
  .metric-card h3 { margin: 0 0 0.5rem; font-size: 0.9rem; }
  .bar-container { height: 8px; background: var(--bg-tertiary); border-radius: 4px; overflow: hidden; margin-bottom: 0.25rem; }
  .bar { height: 100%; background: var(--accent); border-radius: 4px; transition: width 0.3s; }
  .bar.warning { background: var(--warning); }
  .metric-value { font-size: 0.8rem; color: var(--text-secondary); }
  .metric-sub { font-size: 0.75rem; color: var(--text-muted); display: block; margin-top: 0.2rem; }
  .per-core { display: flex; gap: 2px; margin-top: 0.5rem; height: 40px; align-items: flex-end; }
  .mini-bar-wrap { flex: 1; height: 100%; background: var(--bg-tertiary); border-radius: 2px; display: flex; align-items: flex-end; }
  .mini-bar { width: 100%; background: var(--accent); border-radius: 2px; transition: height 0.3s; min-height: 1px; }
</style>
