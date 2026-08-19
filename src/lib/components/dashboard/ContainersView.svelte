<script lang="ts">
  import { onMount } from "svelte";
  import {
    listAllContainers, containerAction, containerActionBulk,
    dockerDiskUsage, listDockerVolumes, listDockerImages,
    removeDockerVolume, removeDockerImage, dockerPrune,
  } from "../../api/docker";
  import { groupBy } from "../../utils/reorder";
  import { notify } from "../../stores/toast";
  import ContainerLogsModal from "../docker/ContainerLogsModal.svelte";
  import type { DockerContainer, DiskUsage, DockerVolume, DockerImage } from "../../types";
  import { trad } from "../../i18n";

  let containers: DockerContainer[] = $state([]);
  let logsFor: DockerContainer | null = $state(null);
  let containersLoaded = $state(false);
  let containersError = $state("");
  let containerBusy = $state("");

  let groupedContainers = $derived(
    groupBy(containers, (c) => c.project || "(sans projet)").map((g) => ({ project: g.key, list: g.items }))
  );

  // Sous-onglet de la vue Docker
  let dockerTab: "containers" | "volumes" | "images" = $state("containers");
  let diskUsage: DiskUsage[] = $state([]);
  let volumes: DockerVolume[] = $state([]);
  let images: DockerImage[] = $state([]);
  let pruning = $state("");

  let volumesLoaded = $state(false);
  let imagesLoaded = $state(false);

  async function loadContainers() {
    containersError = "";
    try {
      containers = await listAllContainers();
    } catch (e) { containersError = String(e); containers = []; }
    containersLoaded = true;
    // docker system df calcule la taille de chaque volume/image : tres lent
    // (10s+) -> charge en arriere-plan, le bandeau apparait quand c'est pret
    dockerDiskUsage().then((d) => (diskUsage = d)).catch(() => {});
  }

  async function loadVolumes() {
    try { volumes = await listDockerVolumes(); } catch (e) { notify(String(e)); }
    volumesLoaded = true;
  }
  async function loadImages() {
    try { images = await listDockerImages(); } catch (e) { notify(String(e)); }
    imagesLoaded = true;
  }

  onMount(loadContainers);

  $effect(() => {
    if (dockerTab === "volumes" && !volumesLoaded) loadVolumes();
    if (dockerTab === "images" && !imagesLoaded) loadImages();
  });

  async function doContainerAction(c: DockerContainer, action: "start" | "stop" | "restart" | "remove") {
    if (containerBusy) return;
    if (action === "remove" && !confirm($trad("cont.deleteContainerConfirm", { name: c.name }))) return;
    containerBusy = c.id + action;
    try { await containerAction(c.id, action); await loadContainers(); }
    catch (e) { notify(String(e)); }
    finally { containerBusy = ""; }
  }

  // Lot : demarre/arrete tous les conteneurs d'un groupe (projet) d'un coup
  async function doGroupAction(list: DockerContainer[], action: "start" | "stop" | "restart") {
    if (containerBusy) return;
    const ids = list
      .filter((c) => (action === "start" ? c.state !== "running" : c.state === "running") || action === "restart")
      .map((c) => c.id);
    if (ids.length === 0) return;
    containerBusy = "group";
    try { await containerActionBulk(ids, action); await loadContainers(); }
    catch (e) { notify(String(e)); }
    finally { containerBusy = ""; }
  }

  async function doRemoveVolume(v: DockerVolume) {
    if (!confirm($trad("cont.deleteVolumeConfirm", { name: v.name }))) return;
    try { await removeDockerVolume(v.name); await loadVolumes(); await loadContainers(); } catch (e) { notify(String(e)); }
  }
  async function doRemoveImage(img: DockerImage) {
    if (!confirm($trad("cont.deleteImageConfirm", { name: `${img.repository}:${img.tag}` }))) return;
    try { await removeDockerImage(img.id); await loadImages(); await loadContainers(); } catch (e) { notify(String(e)); }
  }

  async function doPrune(target: "containers" | "images" | "images_all" | "volumes" | "builder", label: string) {
    if (pruning) return;
    if (!confirm($trad("cont.pruneConfirm", { label }))) return;
    pruning = target;
    try {
      const msg = await dockerPrune(target);
      const reclaimed = msg.match(/Total reclaimed space:\s*(.+)/i)?.[1] ?? "terminé";
      notify($trad("cont.pruneDone", { reclaimed }), "success");
      await loadContainers();
      if (target === "volumes") await loadVolumes();
      if (target.startsWith("images")) await loadImages();
    } catch (e) { notify(String(e)); }
    finally { pruning = ""; }
  }
</script>

<div class="containers-panel">
  <div class="panel-header">
    <div class="docker-tabs">
      <button class:active={dockerTab === "containers"} onclick={() => (dockerTab = "containers")}>{$trad("cont.containers")}</button>
      <button class:active={dockerTab === "volumes"} onclick={() => (dockerTab = "volumes")}>{$trad("cont.volumes")}</button>
      <button class:active={dockerTab === "images"} onclick={() => (dockerTab = "images")}>{$trad("cont.images")}</button>
    </div>
    <button class="metrics-btn" onclick={() => { loadContainers(); loadVolumes(); loadImages(); }} title={$trad("common.refresh")}>↻</button>
  </div>

  <!-- Bandeau espace disque + prune -->
  {#if diskUsage.length > 0}
    <div class="df-bar">
      {#each diskUsage as d}
        <div class="df-item">
          <span class="df-kind">{d.kind}</span>
          <span class="df-size">{d.size}</span>
          {#if d.reclaimable && !d.reclaimable.startsWith("0B")}
            <span class="df-reclaim">récupérable {d.reclaimable}</span>
          {/if}
        </div>
      {/each}
      <div class="prune-actions">
        <button disabled={!!pruning} onclick={() => doPrune("containers", "conteneurs arrêtés")}>{$trad("cont.pruneContainers")}</button>
        <button disabled={!!pruning} onclick={() => doPrune("images", "images sans tag")}>{$trad("cont.pruneImages")}</button>
        <button disabled={!!pruning} onclick={() => doPrune("images_all", "toutes les images inutilisées")}>{$trad("cont.pruneImagesAll")}</button>
        <button disabled={!!pruning} onclick={() => doPrune("volumes", "volumes non utilisés")}>{$trad("cont.pruneVolumes")}</button>
        <button disabled={!!pruning} onclick={() => doPrune("builder", "cache de build")}>{$trad("cont.pruneBuildCache")}</button>
      </div>
    </div>
  {/if}

  {#if dockerTab === "containers"}
    {#if containersError}
      <p class="empty error">{containersError}</p>
    {:else if !containersLoaded}
      <p class="empty">{$trad("common.loading")}</p>
    {:else if containers.length === 0}
      <p class="empty">{$trad("cont.noContainer")}</p>
    {:else}
      {#each groupedContainers as group (group.project)}
        {@const running = group.list.filter((c) => c.state === "running").length}
        <div class="ctn-group">
          <div class="ctn-group-head">
            <span>{group.project} <span class="grp-count">{running}/{group.list.length}</span></span>
            <div class="grp-actions">
              <button disabled={!!containerBusy} onclick={() => doGroupAction(group.list, "start")} title={$trad("cont.startAll")}>{$trad("cont.allStart")}</button>
              <button disabled={!!containerBusy} onclick={() => doGroupAction(group.list, "stop")} title={$trad("cont.stopAll")}>{$trad("cont.allStop")}</button>
              <button disabled={!!containerBusy} onclick={() => doGroupAction(group.list, "restart")} title={$trad("cont.restartAll")}>⟳</button>
            </div>
          </div>
          {#each group.list as c (c.id)}
            <div class="ctn-row">
              <span class="ctn-dot" class:running={c.state === "running"} title={c.state}></span>
              <div class="ctn-info">
                <span class="ctn-name">{c.name}</span>
                <span class="ctn-meta">{c.image}{#if c.ports} · {c.ports}{/if}</span>
              </div>
              <span class="ctn-status">{c.status}</span>
              <div class="ctn-actions">
                <button title={$trad("docker.logsHint")} onclick={() => (logsFor = c)}>≡</button>
                {#if c.state === "running"}
                  <button title={$trad("cont.restart")} disabled={!!containerBusy} onclick={() => doContainerAction(c, "restart")}>⟳</button>
                  <button title={$trad("cont.stop")} disabled={!!containerBusy} onclick={() => doContainerAction(c, "stop")}>⏹</button>
                {:else}
                  <button title={$trad("cont.start")} disabled={!!containerBusy} onclick={() => doContainerAction(c, "start")}>▶</button>
                  <button title={$trad("common.delete")} class="danger" disabled={!!containerBusy} onclick={() => doContainerAction(c, "remove")}>🗑</button>
                {/if}
              </div>
            </div>
          {/each}
        </div>
      {/each}
    {/if}

  {:else if dockerTab === "volumes"}
    {#if !volumesLoaded}
      <p class="empty">{$trad("common.loading")}</p>
    {:else if volumes.length === 0}
      <p class="empty">{$trad("cont.noVolume")}</p>
    {:else}
      {#each volumes as v (v.name)}
        <div class="ctn-row">
          <span class="ctn-dot" class:running={!v.dangling} title={v.dangling ? "non utilisé" : "utilisé"}></span>
          <div class="ctn-info">
            <span class="ctn-name">{v.name}</span>
            <span class="ctn-meta">{v.driver}{#if v.dangling} · non utilisé{/if}</span>
          </div>
          <div class="ctn-actions">
            <button title={$trad("common.delete")} class="danger" onclick={() => doRemoveVolume(v)}>🗑</button>
          </div>
        </div>
      {/each}
    {/if}

  {:else}
    {#if !imagesLoaded}
      <p class="empty">{$trad("common.loading")}</p>
    {:else if images.length === 0}
      <p class="empty">{$trad("cont.noImage")}</p>
    {:else}
      {#each images as img (img.id)}
        <div class="ctn-row">
          <span class="ctn-dot" class:running={!img.dangling} title={img.dangling ? "sans tag" : ""}></span>
          <div class="ctn-info">
            <span class="ctn-name">{img.repository === "<none>" ? img.id.slice(0, 12) : `${img.repository}:${img.tag}`}</span>
            <span class="ctn-meta">{#if img.dangling}sans tag · {/if}{img.id.slice(0, 12)}</span>
          </div>
          <span class="ctn-status">{img.size}</span>
          <div class="ctn-actions">
            <button title={$trad("common.delete")} class="danger" onclick={() => doRemoveImage(img)}>🗑</button>
          </div>
        </div>
      {/each}
    {/if}
  {/if}
</div>

{#if logsFor}
  <ContainerLogsModal id={logsFor.id} name={logsFor.name} onClose={() => (logsFor = null)} />
{/if}

<style>
  .containers-panel {
    flex: 1; min-width: 0;
    background: var(--bg-secondary); border: 1px solid var(--border-color); border-radius: 8px;
    overflow: hidden;
  }
  .panel-header {
    display: flex; justify-content: space-between; align-items: center;
    padding: 0.75rem 1rem; border-bottom: 1px solid var(--border-color);
  }
  .metrics-btn {
    padding: 0.25rem 0.6rem; border: 1px solid var(--border-color); border-radius: 4px;
    background: var(--bg-secondary); color: var(--text-secondary); cursor: pointer; font-size: 0.75rem;
  }
  .metrics-btn:hover { background: var(--bg-tertiary); color: var(--text-primary); }

  .ctn-group { border-bottom: 1px solid var(--border-color); }
  .ctn-group:last-child { border-bottom: none; }
  .ctn-group-head {
    display: flex; align-items: center; justify-content: space-between;
    padding: 0.4rem 1rem; font-size: 0.75rem; font-weight: 600; color: var(--text-muted);
    background: var(--bg-tertiary); text-transform: uppercase; letter-spacing: 0.03em;
  }
  .grp-count { color: var(--accent); margin-left: 0.3rem; }
  .grp-actions { display: flex; gap: 0.25rem; }
  .grp-actions button {
    padding: 0.1rem 0.4rem; font-size: 0.7rem; text-transform: none; cursor: pointer;
    border: 1px solid var(--border-color); border-radius: 4px;
    background: var(--bg-secondary); color: var(--text-secondary);
  }
  .grp-actions button:hover:not(:disabled) { border-color: var(--accent); color: var(--accent); }
  .grp-actions button:disabled { opacity: 0.4; }

  /* Onglets Docker + bandeau espace disque */
  .docker-tabs { display: flex; gap: 0.25rem; }
  .docker-tabs button {
    padding: 0.25rem 0.7rem; font-size: 0.82rem; cursor: pointer;
    border: 1px solid transparent; border-radius: 6px;
    background: none; color: var(--text-secondary);
  }
  .docker-tabs button:hover { color: var(--text-primary); }
  .docker-tabs button.active { background: var(--bg-tertiary); color: var(--accent); font-weight: 600; }
  .df-bar {
    display: flex; align-items: center; flex-wrap: wrap; gap: 0.5rem 1rem;
    padding: 0.5rem 1rem; border-bottom: 1px solid var(--border-color); background: var(--bg-primary);
  }
  .df-item { display: flex; align-items: baseline; gap: 0.35rem; font-size: 0.75rem; }
  .df-kind { color: var(--text-muted); }
  .df-size { font-weight: 600; color: var(--text-primary); }
  .df-reclaim { color: var(--warning, #d29922); font-size: 0.7rem; }
  .prune-actions { display: flex; flex-wrap: wrap; gap: 0.3rem; margin-left: auto; }
  .prune-actions button {
    padding: 0.2rem 0.5rem; font-size: 0.72rem; cursor: pointer;
    border: 1px solid var(--border-color); border-radius: 4px;
    background: var(--bg-secondary); color: var(--text-secondary);
  }
  .prune-actions button:hover:not(:disabled) { border-color: var(--warning, #d29922); color: var(--warning, #d29922); }
  .prune-actions button:disabled { opacity: 0.5; }
  .ctn-row {
    display: flex; align-items: center; gap: 0.6rem;
    padding: 0.4rem 1rem; border-top: 1px solid var(--border-color);
  }
  .ctn-group-head + .ctn-row { border-top: none; }
  .ctn-dot { width: 8px; height: 8px; border-radius: 50%; flex-shrink: 0; background: var(--text-muted); }
  .ctn-dot.running { background: var(--success, #46a758); }
  .ctn-info { display: flex; flex-direction: column; min-width: 0; flex: 1; }
  .ctn-name { font-size: 0.85rem; font-weight: 600; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .ctn-meta { font-size: 0.72rem; color: var(--text-muted); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .ctn-status { font-size: 0.72rem; color: var(--text-secondary); flex-shrink: 0; white-space: nowrap; }
  .ctn-actions { display: flex; gap: 0.2rem; flex-shrink: 0; }
  .ctn-actions button {
    width: 26px; height: 26px; border: 1px solid var(--border-color); border-radius: 4px;
    background: var(--bg-secondary); color: var(--text-secondary); cursor: pointer; font-size: 0.8rem;
  }
  .ctn-actions button:hover:not(:disabled) { border-color: var(--accent); color: var(--accent); }
  .ctn-actions button.danger:hover:not(:disabled) { border-color: var(--error, #e5484d); color: var(--error, #e5484d); }
  .ctn-actions button:disabled { opacity: 0.4; }
</style>
