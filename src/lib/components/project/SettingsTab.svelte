<script lang="ts">
  import { getProjectSettings, updateProjectSettings, deleteDbProject } from "../../api/scanner";
  import { composeDetecte, type ComposeDetecte } from "../../api/docker";
  import { getProjectSummaryPrompt, setProjectSummaryPrompt } from "../../api/recorder";
  import { loadProjects } from "../../stores/projects";
  import { notify } from "../../stores/toast";
  import { goHome, forgetProjectTab } from "../../stores/ui";
  import UrlList from "../urls/UrlList.svelte";
  import CommandList from "./CommandList.svelte";
  import type { DbProject } from "../../types";
  import { onMount } from "svelte";
  import { trad } from "../../i18n";
  import { signalerErreur } from "../../stores/errors";
  import { demanderConfirmation } from "../../stores/confirm";

  let { name }: { name: string } = $props();

  let settings: DbProject | null = $state(null);
  let path = $state("");
  let composeFile = $state("");
  let detection: ComposeDetecte | null = $state(null);
  let relecture = $state(false);
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
    } catch (e) { notify($trad("projectSettings.loadFailed", { error: String(e) })); }
    try { summaryPrompt = (await getProjectSummaryPrompt(name)) ?? ""; } catch (e) { notify(String(e)); }
    await relireLaDetection();
  });

  // Ce qui a remplace le champ ou l'on saisissait un chemin : on AFFICHE ce qui a ete trouve, et
  // on ne propose de changer que parmi ce qui existe vraiment sur le disque.
  async function relireLaDetection(rafraichir = false) {
    relecture = rafraichir;
    try {
      detection = await composeDetecte(name, rafraichir);
      // Un ancien chemin saisi a la main qui ne designe plus rien est nettoye : sinon il
      // repartirait en base a chaque enregistrement, sans jamais servir a personne.
      if (!detection.choisi_a_la_main) composeFile = "";
    } catch (e) {
      signalerErreur("projet.composeDetecte", String(e));
    } finally {
      relecture = false;
    }
  }

  async function save() {
    saving = true;
    try {
      const deps = dependsOnStr.split(",").map(s => s.trim()).filter(Boolean);
      await updateProjectSettings(name, path, composeFile, description, deps);
      await setProjectSummaryPrompt(name, summaryPrompt.trim() || null);
      await loadProjects();
      notify($trad("projectSettings.saved"), "success");
    } catch(e) { notify(String(e)); }
    finally { saving = false; }
  }

  let deleting = $state(false);
  async function deleteProject() {
    if (!settings) return;
    const question = $trad("projectSettings.deleteConfirmFull", { name });
    if (!(await demanderConfirmation({ message: question, action: $trad("common.delete") }))) return;
    deleting = true;
    try {
      await deleteDbProject(settings.id);
      await loadProjects();
      forgetProjectTab(name);
      goHome();
    } catch (e) {
      signalerErreur("settings.deleteProject", String(e)); notify(String(e)); }
    finally { deleting = false; }
  }
</script>

<div class="settings-tab">
  <div class="settings-layout">
    <div class="settings-form card">
      <h3>{$trad("projectSettings.title", { name })}</h3>

      <label>
        {$trad("projectSettings.path")}
        <input type="text" bind:value={path} />
      </label>

      <div class="compose">
        <span class="compose-titre">{$trad("projectSettings.composeFile")}</span>
        {#if detection && detection.retenu}
          <p class="compose-retenu"><code>{detection.retenu}</code></p>
          {#if detection.candidats.length > 1}
            <label class="compose-choix">
              {$trad("projectSettings.composeChoisir")}
              <select bind:value={composeFile}>
                <option value="">{$trad("projectSettings.composeAuto", { fichier: detection.candidats[0] })}</option>
                {#each detection.candidats as candidat (candidat)}
                  <option value={candidat}>{candidat}</option>
                {/each}
              </select>
            </label>
          {/if}
        {:else}
          <p class="compose-vide">{$trad("projectSettings.composeAucun")}</p>
        {/if}
        <button class="btn-relire" onclick={() => relireLaDetection(true)} disabled={relecture}>
          {relecture ? $trad("projectSettings.composeRelecture") : $trad("projectSettings.composeRelire")}
        </button>
      </div>

      <label>
        {$trad("projectSettings.description")}
        <textarea bind:value={description} rows="3"></textarea>
      </label>

      <label>
        {$trad("projectSettings.dependencies")}
        <input type="text" bind:value={dependsOnStr} placeholder={$trad("projectSettings.dependenciesPlaceholder")} />
      </label>

      <label>
        {$trad("projectSettings.summaryPrompt")}
        <textarea bind:value={summaryPrompt} rows="6" placeholder={$trad("projectSettings.summaryPromptPlaceholder")}></textarea>
      </label>

      <button class="btn-save" onclick={save} disabled={saving}>
        {saving ? $trad("projectSettings.saving") : $trad("common.save")}
      </button>

      <div class="danger-zone">
        <h4>{$trad("projectSettings.dangerZone")}</h4>
        <p>{$trad("projectSettings.dangerHelp")}</p>
        <button class="btn-delete" onclick={deleteProject} disabled={deleting}>
          {deleting ? $trad("projectSettings.deleting") : $trad("projectSettings.delete")}
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
  /* Le fichier compose ne se saisit plus : il se constate. Le bloc a la meme place et le
     meme rythme que les champs voisins, pour que l'oeil ne cherche pas. */
  .compose { margin-bottom: 0.75rem; }
  .compose-titre { display: block; font-size: 0.85rem; color: var(--text-secondary); }
  .compose-retenu { margin: 0.3rem 0 0; }
  .compose-retenu code {
    padding: 0.15rem 0.4rem; border-radius: 4px;
    background: var(--bg-tertiary); color: var(--text-primary); font-size: 0.85rem;
  }
  .compose-vide { margin: 0.3rem 0 0; font-size: 0.85rem; color: var(--text-tertiary); }
  .compose-choix { margin: 0.6rem 0 0; }
  select {
    display: block; width: 100%; margin-top: 0.25rem; padding: 0.4rem 0.6rem;
    border: 1px solid var(--border-color); border-radius: 6px; font-size: 0.9rem;
    background: var(--bg-secondary); color: var(--text-primary);
  }
  .btn-relire {
    margin-top: 0.5rem; padding: 0.3rem 0.7rem; font-size: 0.8rem; cursor: pointer;
    border: 1px solid var(--border-color); border-radius: 6px;
    background: var(--bg-tertiary); color: var(--text-secondary);
  }
  .btn-relire:hover:not(:disabled) { color: var(--text-primary); border-color: var(--border-strong); }
  .btn-relire:disabled { cursor: default; opacity: 0.6; }
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
