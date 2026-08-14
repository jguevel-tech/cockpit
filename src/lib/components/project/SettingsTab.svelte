<script lang="ts">
  import { getProjectSettings, updateProjectSettings, deleteDbProject } from "../../api/scanner";
  import { getProjectSummaryPrompt, setProjectSummaryPrompt } from "../../api/recorder";
  import { loadProjects } from "../../stores/projects";
  import { notify } from "../../stores/toast";
  import { goHome } from "../../stores/ui";
  import UrlList from "../urls/UrlList.svelte";
  import CommandList from "./CommandList.svelte";
  import type { DbProject } from "../../types";
  import { onMount } from "svelte";

  let { name }: { name: string } = $props();

  let settings: DbProject | null = $state(null);
  let path = $state("");
  let composeFile = $state("");
  let description = $state("");
  let dependsOnStr = $state("");
  let summaryPrompt = $state("");
  let saving = $state(false);

  onMount(async () => {
    try {
      settings = await getProjectSettings(name);
      path = settings.path;
      composeFile = settings.compose_file;
      description = settings.description;
      dependsOnStr = settings.depends_on.join(", ");
    } catch (e) { notify(`Chargement des paramètres impossible : ${String(e)}`); }
    try { summaryPrompt = (await getProjectSummaryPrompt(name)) ?? ""; } catch (e) { notify(String(e)); }
  });

  async function save() {
    saving = true;
    try {
      const deps = dependsOnStr.split(",").map(s => s.trim()).filter(Boolean);
      await updateProjectSettings(name, path, composeFile, description, deps);
      await setProjectSummaryPrompt(name, summaryPrompt.trim() || null);
      await loadProjects();
      notify("Paramètres sauvegardés", "success");
    } catch(e) { notify(String(e)); }
    finally { saving = false; }
  }

  let deleting = $state(false);
  async function deleteProject() {
    if (!settings) return;
    if (!confirm(`Supprimer entièrement le projet « ${name} » et toutes ses données de Cockpit (notes, todos, URLs, terminaux, enregistrements) ?\n\nLes fichiers sur le disque ne sont pas touchés.`)) return;
    deleting = true;
    try {
      await deleteDbProject(settings.id);
      await loadProjects();
      goHome();
    } catch (e) { alert(e); }
    finally { deleting = false; }
  }
</script>

<div class="settings-tab">
  <div class="settings-layout">
    <div class="settings-form card">
      <h3>Parametres de {name}</h3>

      <label>
        Chemin
        <input type="text" bind:value={path} />
      </label>

      <label>
        Fichier Compose
        <input type="text" bind:value={composeFile} placeholder="docker-compose.yml" />
      </label>

      <label>
        Description
        <textarea bind:value={description} rows="3"></textarea>
      </label>

      <label>
        Dependances (separees par des virgules)
        <input type="text" bind:value={dependsOnStr} placeholder="projet1, projet2" />
      </label>

      <label>
        Prompt de résumé de réunion (laisser vide pour utiliser le prompt global)
        <textarea bind:value={summaryPrompt} rows="6" placeholder="Prompt global utilisé si vide"></textarea>
      </label>

      <button class="btn-save" onclick={save} disabled={saving}>
        {saving ? 'Sauvegarde...' : 'Sauvegarder'}
      </button>

      <div class="danger-zone">
        <h4>Zone de danger</h4>
        <p>Retire le projet et toutes ses données de Cockpit. Les fichiers sur le disque ne sont pas touchés.</p>
        <button class="btn-delete" onclick={deleteProject} disabled={deleting}>
          {deleting ? 'Suppression...' : 'Supprimer ce projet'}
        </button>
      </div>
    </div>

    <div class="settings-urls">
      <UrlList project={name} />
      <CommandList project={name} />
    </div>
  </div>
</div>

<style>
  .settings-tab { width: 100%; }
  .settings-layout { display: flex; gap: 2rem; align-items: flex-start; }
  /*  porte le fond : sans panneau, les libelles et les champs etaient poses a meme
     l image de fond, illisibles. Le padding est local,  ne le definit pas. */
  /* La classe .card porte le fond : sans panneau, les libelles et les champs etaient poses a
     meme l'image de fond, illisibles. Le padding est local, .card ne le definit pas. */
  .settings-form { flex: 1; max-width: 500px; padding: 1.35rem 1.4rem; }
  /* Pas de .card ici : UrlList fournit deja son propre panneau, on aurait un double cadre. */
  .settings-urls { flex: 1; display: flex; flex-direction: column; gap: 1rem; }
  h3 { margin-bottom: 1rem; }
  label { display: block; margin-bottom: 0.75rem; font-size: 0.85rem; color: var(--text-secondary); }
  input, textarea {
    display: block; width: 100%; margin-top: 0.25rem; padding: 0.4rem 0.6rem;
    border: 1px solid var(--border-color); border-radius: 6px; font-size: 0.9rem;
    background: var(--bg-secondary); color: var(--text-primary);
  }
  textarea { resize: vertical; }
  .btn-save {
    padding: 0.5rem 1.2rem; background: var(--accent); color: white; border: none;
    border-radius: 6px; cursor: pointer; font-size: 0.9rem;
  }
  .btn-save:hover { background: var(--accent-hover); }
  .btn-save:disabled { opacity: 0.5; }
  .danger-zone {
    margin-top: 2rem; padding-top: 1rem; border-top: 1px solid var(--border-color);
  }
  .danger-zone h4 { margin: 0 0 0.3rem; font-size: 0.85rem; color: var(--error, #e5484d); }
  .danger-zone p { margin: 0 0 0.6rem; font-size: 0.78rem; color: var(--text-muted); }
  .btn-delete {
    padding: 0.45rem 1rem; background: none; color: var(--error, #e5484d);
    border: 1px solid var(--error, #e5484d); border-radius: 6px; cursor: pointer; font-size: 0.85rem;
  }
  .btn-delete:hover { background: var(--error, #e5484d); color: white; }
  .btn-delete:disabled { opacity: 0.5; }
</style>
