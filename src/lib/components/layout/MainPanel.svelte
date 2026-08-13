<script lang="ts">
  import { activeView, selectedProject } from "../../stores/ui";
  import Dashboard from "../dashboard/Dashboard.svelte";
  import ProjectDetail from "../project/ProjectDetail.svelte";
  import GlobalSettings from "../settings/GlobalSettings.svelte";
  import SystemMonitor from "../system/SystemMonitor.svelte";
  import AgentsView from "../agents/AgentsView.svelte";
</script>

<main>
  {#if $activeView === "settings"}
    <GlobalSettings />
  {:else if $activeView === "system"}
    <SystemMonitor />
  {:else if $activeView === "agents"}
    <AgentsView />
  {:else if $activeView === "project" && $selectedProject}
    {#key $selectedProject}
      <ProjectDetail name={$selectedProject} />
    {/key}
  {:else}
    <Dashboard />
  {/if}
</main>

<style>
  main {
    flex: 1; overflow-y: auto; padding: 1.5rem;
    background: var(--bg-primary);
  }
</style>
