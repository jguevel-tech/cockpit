<script lang="ts">
  import { systemMetrics, cpuHistory, memoryHistory, metricsLive, refreshMetrics, startLiveMetrics, stopLiveMetrics } from "../../stores/system";
  import { formatBytes } from "../../utils/format";
  import type { SystemMetrics } from "../../types";
  import { trad, translate } from "../../i18n";

  // Metrics shortcuts
  let metrics: SystemMetrics | null = $derived($systemMetrics);
  let cpuHist: number[] = $derived($cpuHistory);
  let memHist: number[] = $derived($memoryHistory);
  let isLive: boolean = $derived($metricsLive);

  // SVG donut helper
  const circumference = 2 * Math.PI * 45;
  function donutOffset(percent: number): number {
    return circumference - (percent / 100) * circumference;
  }

  function memoryBreakdown(m: SystemMetrics["memory"]) {
    const items: {
      labelKey: Parameters<typeof translate>[0];
      bytes: number;
      percent: number;
      color: string;
    }[] = [];
    const processBytes = Math.max(0, m.used - m.cached - m.buffers - m.s_reclaimable - m.zfs_arc);
    items.push({ labelKey: "mon.processes", bytes: processBytes, percent: pct(processBytes, m.total), color: "var(--success)" });
    if (m.zfs_arc > 0) {
      items.push({ labelKey: "mon.zfsArc", bytes: m.zfs_arc, percent: pct(m.zfs_arc, m.total), color: "#a855f7" });
    }
    items.push({ labelKey: "mon.cache", bytes: m.cached, percent: pct(m.cached, m.total), color: "var(--accent)" });
    items.push({ labelKey: "mon.shared", bytes: m.shmem, percent: pct(m.shmem, m.total), color: "var(--error)" });
    items.push({ labelKey: "mon.buffers", bytes: m.buffers, percent: pct(m.buffers, m.total), color: "var(--warning)" });
    return items;
  }

  function pct(val: number, total: number): number {
    return total > 0 ? Math.round((val / total) * 1000) / 10 : 0;
  }

  function chartPoints(data: number[], width: number, height: number): string {
    if (data.length === 0) return "";
    const step = width / 59;
    const offset = (60 - data.length) * step;
    return data.map((v, i) => `${offset + i * step},${height - (v / 100) * height}`).join(" ");
  }

  function chartAreaPoints(data: number[], width: number, height: number): string {
    if (data.length === 0) return "";
    const step = width / 59;
    const offset = (60 - data.length) * step;
    const line = data.map((v, i) => `${offset + i * step},${height - (v / 100) * height}`).join(" ");
    const startX = offset;
    const endX = offset + (data.length - 1) * step;
    return `${startX},${height} ${line} ${endX},${height}`;
  }

  let showTopCpu = $state(true);
</script>

<div class="monitoring-panel">
  <div class="monitoring-header">
    <h3>{$trad("mon.title")}</h3>
    <div class="metrics-controls">
      <button class="metrics-btn" onclick={refreshMetrics} title={$trad("mon.snapshotHint")}>{$trad("sys.snapshot")}</button>
      {#if isLive}
        <button class="metrics-btn live-active" onclick={stopLiveMetrics} title={$trad("mon.liveStopHint")}>{$trad("sys.liveOn")}</button>
      {:else}
        <button class="metrics-btn" onclick={startLiveMetrics} title={$trad("mon.liveStartHint")}>{$trad("sys.liveOff")}</button>
      {/if}
    </div>
  </div>
  {#if metrics}
    <div class="system-info">
      <span class="sys-badge">{metrics.hostname}</span>
      <span class="sys-detail">{metrics.kernel_version}</span>
      <span class="sys-detail">Uptime: {metrics.uptime}</span>
    </div>

    <!-- Gauges CPU + Memory -->
    <div class="gauges-row">
      <div class="gauge-card">
        <div class="gauge-icon">⚙</div>
        <div class="gauge-title">CPU</div>
        <svg viewBox="0 0 120 120" class="donut">
          <circle cx="60" cy="60" r="45" fill="none" stroke="var(--border-color)" stroke-width="10"/>
          <circle cx="60" cy="60" r="45" fill="none" stroke="var(--accent)" stroke-width="10"
            stroke-dasharray={circumference} stroke-dashoffset={donutOffset(metrics.cpu.usage_percent)}
            transform="rotate(-90 60 60)" stroke-linecap="round"/>
          <text x="60" y="55" text-anchor="middle" fill="var(--text-primary)" font-size="20" font-weight="bold">
            {metrics.cpu.usage_percent.toFixed(1)}%
          </text>
          <text x="60" y="75" text-anchor="middle" fill="var(--text-muted)" font-size="10">
            {metrics.cpu.cores} cœurs
          </text>
        </svg>
        <div class="gauge-sub">{metrics.cpu.model_name}</div>
      </div>

      <div class="gauge-card">
        <div class="gauge-icon">▪</div>
        <div class="gauge-title">{$trad("mon.memoryCaps")}</div>
        <svg viewBox="0 0 120 120" class="donut">
          <circle cx="60" cy="60" r="45" fill="none" stroke="var(--border-color)" stroke-width="10"/>
          <circle cx="60" cy="60" r="45" fill="none"
            stroke={metrics.memory.percent > 80 ? "var(--warning)" : "var(--success)"}
            stroke-width="10"
            stroke-dasharray={circumference} stroke-dashoffset={donutOffset(metrics.memory.percent)}
            transform="rotate(-90 60 60)" stroke-linecap="round"/>
          <text x="60" y="55" text-anchor="middle" fill="var(--text-primary)" font-size="20" font-weight="bold">
            {metrics.memory.percent.toFixed(1)}%
          </text>
          <text x="60" y="75" text-anchor="middle" fill="var(--text-muted)" font-size="10">
            {formatBytes(metrics.memory.used)} / {formatBytes(metrics.memory.total)}
          </text>
        </svg>
        <div class="gauge-sub">Swap: {formatBytes(metrics.memory.swap_used)} / {formatBytes(metrics.memory.swap_total)}</div>
      </div>
    </div>

    <div class="memory-breakdown">
      {#each memoryBreakdown(metrics.memory) as item}
        <div class="mem-item">
          <span class="mem-dot" style="background:{item.color}"></span>
          <span class="mem-label">{$trad(item.labelKey)}</span>
          <span class="mem-value">{formatBytes(item.bytes)}</span>
          <span class="mem-pct">{item.percent}%</span>
        </div>
      {/each}
    </div>

    <div class="chart-section">
      <div class="chart-label">CPU</div>
      <svg viewBox="0 0 400 80" class="chart" preserveAspectRatio="none">
        {#each [25, 50, 75] as y}
          <line x1="0" y1={80 - (y / 100) * 80} x2="400" y2={80 - (y / 100) * 80}
            stroke="var(--border-color)" stroke-width="0.5" stroke-dasharray="2,2"/>
        {/each}
        <polygon points={chartAreaPoints(cpuHist, 400, 80)} fill="rgba(74,158,255,0.12)"/>
        <polyline points={chartPoints(cpuHist, 400, 80)} fill="none" stroke="var(--accent)" stroke-width="1.5"/>
      </svg>
    </div>

    <div class="chart-section">
      <div class="chart-label">{$trad("mon.memoryCaps")}</div>
      <svg viewBox="0 0 400 80" class="chart" preserveAspectRatio="none">
        {#each [25, 50, 75] as y}
          <line x1="0" y1={80 - (y / 100) * 80} x2="400" y2={80 - (y / 100) * 80}
            stroke="var(--border-color)" stroke-width="0.5" stroke-dasharray="2,2"/>
        {/each}
        <polygon points={chartAreaPoints(memHist, 400, 80)} fill="rgba(34,197,94,0.12)"/>
        <polyline points={chartPoints(memHist, 400, 80)} fill="none" stroke="var(--success)" stroke-width="1.5"/>
      </svg>
    </div>

    <div class="top-processes">
      <div class="top-tabs">
        <button class="top-tab" class:active={showTopCpu} onclick={() => showTopCpu = true}>{$trad("proc.topCpu")}</button>
        <button class="top-tab" class:active={!showTopCpu} onclick={() => showTopCpu = false}>{$trad("proc.topMemory")}</button>
      </div>
      <table>
        <thead>
          <tr>
            <th>{$trad("proc.pid")}</th>
            <th>{$trad("proc.name")}</th>
            <th>{$trad("proc.user")}</th>
            <th>{$trad("mon.cpuPct")}</th>
            <th>{$trad("mon.memPct")}</th>
            <th>{$trad("proc.rss")}</th>
            {#if !showTopCpu}<th>{$trad("mon.inst")}</th>{/if}
          </tr>
        </thead>
        <tbody>
          {#each (showTopCpu ? metrics.top_cpu : metrics.top_memory) as proc}
            <tr>
              <td class="mono">{proc.pid}</td>
              <td class="proc-name">{proc.name}</td>
              <td>{proc.user}</td>
              <td>{proc.cpu.toFixed(1)}</td>
              <td>{proc.memory.toFixed(1)}</td>
              <td>{formatBytes(proc.memory_rss)}</td>
              {#if !showTopCpu}<td>{proc.count ?? 1}</td>{/if}
            </tr>
          {/each}
        </tbody>
      </table>
    </div>
  {:else}
    <div class="monitoring-loading">{$trad("sys.startHint")}</div>
  {/if}
</div>

<style>
  .monitoring-panel {
    flex: 1; min-width: 0;
    background: var(--bg-secondary); border: 1px solid var(--border-color); border-radius: 8px;
    padding: 1rem; overflow-y: auto; max-height: calc(100vh - 140px);
  }
  .monitoring-header { display: flex; align-items: center; justify-content: space-between; margin-bottom: 1rem; }
  .monitoring-header h3 { font-size: 1rem; margin: 0; }
  .metrics-controls { display: flex; gap: 0.4rem; }
  .metrics-btn {
    padding: 0.25rem 0.6rem; border: 1px solid var(--border-color); border-radius: 4px;
    background: var(--bg-secondary); color: var(--text-secondary); cursor: pointer; font-size: 0.75rem;
  }
  .metrics-btn:hover { background: var(--bg-tertiary); color: var(--text-primary); }
  .metrics-btn.live-active { background: var(--success); color: white; border-color: var(--success); }
  .system-info { display: flex; align-items: center; gap: 0.5rem; flex-wrap: wrap; }
  .sys-badge {
    background: var(--accent); color: white; padding: 0.15rem 0.5rem;
    border-radius: 4px; font-size: 0.75rem; font-weight: 600;
  }
  .sys-detail { font-size: 0.75rem; color: var(--text-muted); }

  /* Gauges */
  .gauges-row { display: flex; gap: 1rem; margin-bottom: 0.75rem; }
  .gauge-card { flex: 1; text-align: center; }
  .gauge-icon { font-size: 0.85rem; color: var(--text-muted); }
  .gauge-title { font-size: 0.85rem; font-weight: 600; margin-bottom: 0.25rem; }
  .donut { width: 110px; height: 110px; display: block; margin: 0 auto; }
  .gauge-sub { font-size: 0.7rem; color: var(--text-muted); margin-top: 0.25rem; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }

  /* Memory breakdown */
  .memory-breakdown {
    display: flex; flex-wrap: wrap; gap: 0.25rem 1rem;
    padding: 0.5rem 0; border-top: 1px dashed var(--border-color);
    margin-bottom: 0.75rem;
  }
  .mem-item { display: flex; align-items: center; gap: 0.35rem; font-size: 0.75rem; }
  .mem-dot { width: 8px; height: 8px; border-radius: 50%; flex-shrink: 0; }
  .mem-label { color: var(--text-secondary); }
  .mem-value { font-family: monospace; color: var(--text-primary); }
  .mem-pct { color: var(--text-muted); }

  /* Charts */
  .chart-section { margin-bottom: 0.75rem; }
  .chart-label { font-size: 0.8rem; font-weight: 600; margin-bottom: 0.25rem; }
  .chart {
    width: 100%; height: 60px; display: block;
    background: var(--bg-primary); border: 1px solid var(--border-color); border-radius: 4px;
  }

  /* Top processes */
  .top-processes { margin-top: 0.5rem; }
  .top-tabs { display: flex; gap: 0; margin-bottom: 0.5rem; }
  .top-tab {
    flex: 1; padding: 0.35rem; background: var(--bg-tertiary); border: 1px solid var(--border-color);
    color: var(--text-secondary); cursor: pointer; font-size: 0.8rem; text-align: center;
  }
  .top-tab:first-child { border-radius: 4px 0 0 4px; }
  .top-tab:last-child { border-radius: 0 4px 4px 0; }
  .top-tab.active { background: var(--accent); color: white; border-color: var(--accent); }

  table { width: 100%; border-collapse: collapse; font-size: 0.75rem; }
  th {
    text-align: left; padding: 0.3rem 0.4rem; border-bottom: 1px solid var(--border-color);
    color: var(--text-muted); font-weight: 600; white-space: nowrap;
  }
  td { padding: 0.25rem 0.4rem; border-bottom: 1px solid var(--border-color); }
  .mono { font-family: monospace; font-size: 0.7rem; }
  .proc-name { max-width: 100px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }

  .monitoring-loading { padding: 2rem; text-align: center; color: var(--text-muted); }

  @media (max-width: 900px) {
    .monitoring-panel { width: 100%; min-width: 0; }
  }
</style>
