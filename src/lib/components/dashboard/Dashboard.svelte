<script lang="ts">
  import { dashboardView } from "../../stores/ui";
  import TasksView from "./TasksView.svelte";
  import MonitoringView from "./MonitoringView.svelte";
  import TerminalsView from "./TerminalsView.svelte";
  import ContainersView from "./ContainersView.svelte";
</script>

<div class="dashboard">
  <h2>Tableau de bord</h2>

  <div class="dashboard-layout">
    <nav class="dash-menu">
      <button class="dash-menu-item" class:active={$dashboardView === "tasks"} onclick={() => dashboardView.set("tasks")}>
        ☑ Tâches
      </button>
      <button class="dash-menu-item" class:active={$dashboardView === "monitoring"} onclick={() => dashboardView.set("monitoring")}>
        📈 Monitoring
      </button>
      <button class="dash-menu-item" class:active={$dashboardView === "terminals"} onclick={() => dashboardView.set("terminals")}>
        &gt;_ Terminaux
      </button>
      <button class="dash-menu-item" class:active={$dashboardView === "containers"} onclick={() => dashboardView.set("containers")}>
        🐳 Conteneurs
      </button>
    </nav>

    {#if $dashboardView === "tasks"}
      <TasksView />
    {:else if $dashboardView === "monitoring"}
      <MonitoringView />
    {:else if $dashboardView === "terminals"}
      <TerminalsView />
    {:else}
      <ContainersView />
    {/if}
  </div>
</div>

<style>
  .dashboard { width: 100%; }
  h2 { margin-bottom: 1rem; font-size: 1.3rem; }

  .dashboard-layout {
    display: flex; gap: 1.5rem; align-items: flex-start;
  }

  /* Menu du tableau de bord */
  .dash-menu {
    display: flex; flex-direction: column; gap: 0.25rem;
    width: 170px; flex-shrink: 0;
  }
  .dash-menu-item {
    text-align: left; padding: 0.5rem 0.8rem; font-size: 0.88rem;
    background: none; border: 1px solid transparent; border-radius: 6px;
    color: var(--text-secondary); cursor: pointer;
  }
  .dash-menu-item:hover { background: var(--bg-secondary); color: var(--text-primary); }
  .dash-menu-item.active {
    background: var(--bg-secondary); border-color: var(--border-color);
    color: var(--accent); font-weight: 600;
  }

  @media (max-width: 900px) {
    .dashboard-layout { flex-direction: column; }
  }
</style>
