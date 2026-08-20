<script lang="ts">
  import { addProject } from "../../api/scanner";
  import { loadProjects } from "../../stores/projects";
  import { selectProject, activeTab, DEFAULT_TAB } from "../../stores/ui";
  import { open as openDialog } from "@tauri-apps/plugin-dialog";
  import { portal } from "../../actions/portal";
  import { trad } from "../../i18n";
  import { signalerErreur } from "../../stores/errors";

  async function browsePath() {
    try {
      const selected = await openDialog({ directory: true, multiple: false, title: $trad("newProject.browseTitle") });
      if (typeof selected === "string") {
        path = selected;
        // Pre-remplit le nom si vide, depuis le dossier choisi
        if (!name.trim()) name = selected.split("/").filter(Boolean).pop() ?? "";
      }
    } catch (e) {
      signalerErreur("projet.parcourir", String(e));
    }
  }

  let { open = $bindable(false) }: { open: boolean } = $props();

  let name = $state("");
  let path = $state("");
  let composeFile = $state("");
  let description = $state("");
  let dependsOn = $state("");
  let error = $state("");
  let creating = $state(false);

  function reset() {
    name = "";
    path = "";
    composeFile = "";
    description = "";
    dependsOn = "";
    error = "";
    creating = false;
  }

  function close() {
    open = false;
    reset();
  }

  function onKeydown(e: KeyboardEvent) {
    if (e.key === "Escape") close();
  }

  function onOverlayClick(e: MouseEvent) {
    if (e.target === e.currentTarget) close();
  }

  async function submit() {
    const trimmedName = name.trim();
    if (!trimmedName) {
      error = $trad("project.nameRequired");
      return;
    }

    creating = true;
    error = "";

    const deps = dependsOn
      .split(",")
      .map(d => d.trim())
      .filter(d => d.length > 0);

    try {
      await addProject(trimmedName, path.trim(), composeFile.trim(), description.trim(), deps);
      await loadProjects();
      selectProject(trimmedName);
      // Onglet d'arrivee pose explicitement : un projet neuf n'a ni compose ni depot git, et
      // un nom deja utilise plus tot dans la session ne doit pas ressortir son ancien onglet.
      activeTab.set(DEFAULT_TAB);
      close();
    } catch (e) {
      // L'erreur s'affiche DANS le modal (c'est la que l'utilisateur regarde), mais elle
      // doit aussi etre journalisee : un affichage local ne remonte rien.
      error = String(e);
      signalerErreur("projet.creation", error);
    } finally {
      creating = false;
    }
  }
</script>

{#if open}
<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
<!-- svelte-ignore a11y_interactive_supports_focus -->
<div class="overlay" role="dialog" aria-modal="true" use:portal onclick={onOverlayClick} onkeydown={onKeydown}>
  <div class="modal">
    <div class="modal-header">
      <h3>{$trad("newProject.title")}</h3>
      <button class="close-btn" onclick={close}>&times;</button>
    </div>

    <div class="modal-body">
      <label>
        <span>{$trad("newProject.name")} <span class="required">*</span></span>
        <!-- svelte-ignore a11y_autofocus -->
        <input type="text" bind:value={name} placeholder={$trad("newProject.namePlaceholder")} autofocus />
      </label>

      <label>
        <span>{$trad("newProject.path")} <span class="optional">{$trad("common.optional")}</span></span>
        <div class="path-row">
          <input type="text" bind:value={path} placeholder={$trad("newProject.pathPlaceholder")} />
          <button type="button" class="browse-btn" onclick={browsePath} title={$trad("newProject.browse")}>📁</button>
        </div>
      </label>

      <label>
        <span>{$trad("newProject.composeFile")} <span class="optional">{$trad("common.optional")}</span></span>
        <input type="text" bind:value={composeFile} placeholder="docker-compose.yml" disabled={!path.trim()} />
      </label>

      <label>
        <span>{$trad("newProject.description")} <span class="optional">{$trad("common.optional")}</span></span>
        <textarea bind:value={description} placeholder={$trad("newProject.descriptionPlaceholder")} rows="2"></textarea>
      </label>

      <label>
        <span>{$trad("newProject.dependencies")} <span class="optional">{$trad("common.optional")}</span></span>
        <input type="text" bind:value={dependsOn} placeholder={$trad("newProject.dependenciesPlaceholder")} />
      </label>

      {#if error}
        <p class="error">{error}</p>
      {/if}
    </div>

    <div class="modal-footer">
      <button class="btn-cancel" onclick={close}>{$trad("common.cancel")}</button>
      <button class="btn-create" onclick={submit} disabled={creating}>
        {creating ? $trad("newProject.creating") : $trad("newProject.create")}
      </button>
    </div>
  </div>
</div>
{/if}

<style>
  .path-row { display: flex; gap: 0.4rem; }
  .path-row input { flex: 1; min-width: 0; }
  .browse-btn {
    flex-shrink: 0; padding: 0 0.7rem; border: 1px solid var(--border-color);
    border-radius: 6px; background: var(--bg-secondary); color: var(--text-secondary);
    cursor: pointer; font-size: 0.95rem;
  }
  .browse-btn:hover { border-color: var(--accent); }
  .overlay {
    position: fixed; inset: 0; z-index: 1000;
    background: rgba(0, 0, 0, 0.5);
    /* OBLIGATOIRE sur un voile plein ecran PEINT : WebKitGTK desactive les
       backdrop-filter de toute la page situee dessous (verre depoli des panneaux mort,
       image nette au travers — prouve par reproduction isolee le 2026-08-14). Le flou
       du voile lui-meme fonctionne, lui, et masque totalement l'artefact. */
    backdrop-filter: blur(12px);
    display: flex; align-items: center; justify-content: center;
  }
  .modal {
    /* Surface flottante : fond OPAQUE obligatoire (--surface-*, pas --bg-* qui
       deviennent translucides sous wallpaper — le contenu transparaissait). */
    background: var(--surface-base); border: 1px solid var(--border-color);
    border-radius: 10px; width: 420px; max-width: 90vw;
    box-shadow: 0 8px 32px rgba(0, 0, 0, 0.3);
  }
  .modal-header {
    display: flex; align-items: center; justify-content: space-between;
    padding: 0.75rem 1rem; border-bottom: 1px solid var(--border-color);
  }
  .modal-header h3 { margin: 0; font-size: 1rem; }
  .close-btn {
    background: none; border: none; color: var(--text-muted);
    font-size: 1.4rem; cursor: pointer; padding: 0; line-height: 1;
  }
  .close-btn:hover { color: var(--text-primary); }
  .modal-body {
    padding: 1rem; display: flex; flex-direction: column; gap: 0.75rem;
  }
  label {
    display: flex; flex-direction: column; gap: 0.25rem; font-size: 0.85rem;
  }
  label span { color: var(--text-secondary); font-weight: 600; }
  .required { color: var(--error); }
  .optional { color: var(--text-muted); font-weight: 400; font-size: 0.75rem; }
  input, textarea {
    padding: 0.4rem 0.6rem; border: 1px solid var(--border-color);
    border-radius: 6px; background: var(--bg-secondary); color: var(--text-primary);
    font-size: 0.85rem; font-family: inherit;
  }
  input:disabled { opacity: 0.4; cursor: not-allowed; }
  textarea { resize: vertical; }
  .error {
    color: var(--error); font-size: 0.8rem; margin: 0;
    padding: 0.3rem 0.5rem; background: color-mix(in srgb, var(--error) 10%, transparent);
    border-radius: 4px;
  }
  .modal-footer {
    display: flex; justify-content: flex-end; gap: 0.5rem;
    padding: 0.75rem 1rem; border-top: 1px solid var(--border-color);
  }
  .btn-cancel {
    padding: 0.4rem 0.8rem; border: 1px solid var(--border-color);
    border-radius: 6px; background: var(--bg-secondary); color: var(--text-primary);
    cursor: pointer; font-size: 0.85rem;
  }
  .btn-cancel:hover { background: var(--bg-tertiary); }
  .btn-create {
    padding: 0.4rem 0.8rem; border: none; border-radius: 6px;
    background: var(--accent); color: white; cursor: pointer; font-size: 0.85rem;
  }
  .btn-create:hover { background: var(--accent-hover); }
  .btn-create:disabled { opacity: 0.5; cursor: not-allowed; }
</style>
