<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { listen, type UnlistenFn } from "@tauri-apps/api/event";
  import { getDbProjects, deleteDbProject } from "../../api/scanner";
  import { getAppSettings, setAppSetting } from "../../api/recorder";
  import {
    claudeAuthStatus, startClaudeLogin, claudeLoginInput, cancelClaudeLogin, openUrl,
    type ClaudeAuthStatus,
  } from "../../api/workspace";
  import { loadProjects } from "../../stores/projects";
  import { updateState, checkForUpdate } from "../../stores/update";
  import type { DbProject } from "../../types";
  import { onMount, onDestroy } from "svelte";
  import AppearanceSettings from "./AppearanceSettings.svelte";
  import AgentsView from "../agents/AgentsView.svelte";
  import { marked } from "marked";
  // Le CHANGELOG.md est embarque au build (Vite ?raw) : consultable hors ligne, et toujours
  // celui de la version installee — pas celui d'une branche distante.
  import changelogRaw from "../../../../CHANGELOG.md?raw";

  const changelogHtml = marked.parse(changelogRaw, { async: false });

  type SettingsView = "general" | "appearance" | "agents" | "claude" | "meetings" | "projects";
  let view: SettingsView = $state("general");

  const MENU: { id: SettingsView; icon: string; label: string }[] = [
    { id: "general", icon: "⚙", label: "Général" },
    { id: "appearance", icon: "◐", label: "Apparence" },
    { id: "agents", icon: "⬡", label: "Agents" },
    { id: "claude", icon: "✳", label: "Claude & IA" },
    { id: "meetings", icon: "⏺", label: "Réunions" },
    { id: "projects", icon: "▤", label: "Projets" },
  ];

  let dbProjects: DbProject[] = $state([]);
  let importPath = $state("");
  let importResult = $state("");
  let importing = $state(false);
  let dbPath = $state("");

  let apiKey = $state("");
  let summaryModel = $state("");
  let summaryPrompt = $state("");
  let meetingSaving = $state(false);
  let meetingSaved = $state(false);

  // --- Connexion Claude Code ---
  let claudeStatus: ClaudeAuthStatus | null = $state(null);
  let loginActive = $state(false);
  let loginLog = $state("");
  let loginCode = $state("");
  let loginUnlisteners: UnlistenFn[] = [];

  const loginUrl = $derived.by(() => {
    const m = loginLog.match(/https:\/\/[^\s\x1b"']+/);
    return m ? m[0] : null;
  });

  function stripAnsi(s: string): string {
    // Sequences CSI/OSC + retours chariot du PTY
    // eslint-disable-next-line no-control-regex
    return s.replace(/\x1b\[[0-9;?]*[a-zA-Z]|\x1b\][^\x07]*(\x07|\x1b\\)|\x1b[()][A-Z0-9]|\r/g, "");
  }

  async function refreshClaudeStatus() {
    try { claudeStatus = await claudeAuthStatus(); } catch {}
  }

  async function beginLogin() {
    loginLog = "";
    loginCode = "";
    loginActive = true;
    loginUnlisteners.push(
      await listen<string>("claude_login_output", (e) => {
        loginLog = (loginLog + stripAnsi(e.payload)).slice(-4000);
      })
    );
    loginUnlisteners.push(
      await listen("claude_login_done", async () => {
        cleanupLoginListeners();
        loginActive = false;
        await refreshClaudeStatus();
      })
    );
    try { await startClaudeLogin(); } catch (e) { loginLog = String(e); loginActive = false; }
  }

  function cleanupLoginListeners() {
    loginUnlisteners.forEach((u) => u());
    loginUnlisteners = [];
  }

  async function sendLoginCode() {
    if (!loginCode.trim()) return;
    try { await claudeLoginInput(loginCode.trim()); loginCode = ""; } catch (e) { alert(e); }
  }

  async function abortLogin() {
    try { await cancelClaudeLogin(); } catch {}
    cleanupLoginListeners();
    loginActive = false;
  }

  function expiryLabel(epochSecs: number): string {
    return new Date(epochSecs * 1000).toLocaleString("fr-FR");
  }

  onDestroy(() => { cleanupLoginListeners(); });

  onMount(async () => {
    await loadDbProjects();
    try { dbPath = await invoke<string>("get_db_path"); } catch {}
    try {
      const s = await getAppSettings();
      apiKey = s.openai_api_key ?? "";
      summaryModel = s.summary_model ?? "";
      summaryPrompt = s.summary_prompt ?? "";
    } catch {}
    await refreshClaudeStatus();
  });

  async function saveMeetingSettings() {
    meetingSaving = true;
    meetingSaved = false;
    try {
      await setAppSetting("openai_api_key", apiKey.trim());
      await setAppSetting("summary_model", summaryModel.trim());
      await setAppSetting("summary_prompt", summaryPrompt.trim());
      meetingSaved = true;
      setTimeout(() => { meetingSaved = false; }, 3000);
    } catch (e) { alert(e); }
    finally { meetingSaving = false; }
  }

  async function doImport() {
    if (!importPath.trim()) return;
    importing = true;
    importResult = "";
    try {
      const result = await invoke<string>("import_database", { path: importPath.trim() });
      importResult = result;
      await loadDbProjects();
      await loadProjects();
    } catch(e) {
      importResult = "Erreur: " + e;
    } finally { importing = false; }
  }

  async function loadDbProjects() {
    try { dbProjects = await getDbProjects(); } catch {}
  }

  async function doDelete(id: number) {
    if (!confirm("Supprimer ce projet ?")) return;
    try { await deleteDbProject(id); await loadDbProjects(); await loadProjects(); } catch(e) { alert(e); }
  }
</script>

<div class="settings" class:wide={view === "agents"}>

  <div class="settings-layout">
    <nav class="settings-menu">
      <h2 class="menu-title">Paramètres</h2>
      {#each MENU as item (item.id)}
        <button
          class="settings-menu-item"
          class:active={view === item.id}
          onclick={() => (view = item.id)}
        >
          <span class="menu-icon">{item.icon}</span>
          {item.label}
        </button>
      {/each}
    </nav>

    <div class="settings-content stack">
      {#if view === "general"}
        <section class="card">
          <div class="card-head">
            <h3>Application</h3>
            <p>Informations sur l'installation en cours.</p>
          </div>
          <div class="field-row">
            <span class="field-label">Base de données</span>
            <code class="mono-value">{dbPath}</code>
          </div>
          <div class="field-row">
            <span class="field-label">Version</span>
            <span class="field-value">{$updateState.currentVersion || '…'}</span>
          </div>
          <div class="field-row">
            <span class="field-label">Build</span>
            <span class="field-value">{__BUILD_TIME__}</span>
          </div>
          <div class="inline-row">
            <button class="btn" onclick={() => checkForUpdate()} disabled={$updateState.phase === 'checking'}>
              {$updateState.phase === 'checking' ? 'Vérification…' : 'Vérifier les mises à jour'}
            </button>
            {#if $updateState.newVersion}
              <span class="feedback">Version {$updateState.newVersion} disponible — voir la cloche.</span>
            {/if}
          </div>
        </section>

        <section class="card">
          <div class="card-head">
            <h3>Journal des modifications</h3>
            <p>Historique des versions, embarqué dans l'application.</p>
          </div>
          <div class="changelog">{@html changelogHtml}</div>
        </section>

        <section class="card">
          <div class="card-head">
            <h3>Importer depuis l'ancienne base</h3>
            <p>Récupère projets, notes, todos et URLs d'une base de l'ancienne version (Go). Uniquement si la base actuelle est vide.</p>
          </div>
          <div class="inline-row">
            <input type="text" bind:value={importPath} placeholder="/chemin/vers/data.db" spellcheck="false" />
            <button class="btn primary" onclick={doImport} disabled={importing}>
              {importing ? 'Import…' : 'Importer'}
            </button>
          </div>
          {#if importResult}
            <p class="feedback" class:error={importResult.startsWith('Erreur')}>{importResult}</p>
          {/if}
        </section>

      {:else if view === "appearance"}
        <AppearanceSettings />

      {:else if view === "agents"}
        <!-- Encastree dans les parametres : la grille de AgentsView est fluide, elle s'adapte
             a la colonne. `.settings.wide` elargit la page pour lui laisser de l'air. -->
        <div class="embedded-view"><AgentsView /></div>

      {:else if view === "claude"}
        <section class="card">
          <div class="card-head">
            <h3>Connexion à l'abonnement Claude</h3>
            <p>Utilisée par les fonctionnalités IA (suggestions de commande…) via la CLI Claude Code — aucune clé API nécessaire.</p>
          </div>
          {#if claudeStatus}
            <div class="claude-status-row">
              {#if !claudeStatus.cli_installed}
                <span class="badge off">CLI claude introuvable</span>
              {:else if claudeStatus.logged_in}
                <span class="badge on">✓ Connecté</span>
                {#if claudeStatus.subscription_type}
                  <span class="detail">abonnement <strong>{claudeStatus.subscription_type}</strong></span>
                {/if}
                {#if claudeStatus.rate_limit_tier}
                  <span class="detail">tier {claudeStatus.rate_limit_tier}</span>
                {/if}
                {#if claudeStatus.expires_at}
                  <span class="detail">token valide jusqu'au {expiryLabel(claudeStatus.expires_at)}</span>
                {/if}
              {:else}
                <span class="badge off">Non connecté</span>
              {/if}
              {#if claudeStatus.cli_version}
                <span class="detail muted">{claudeStatus.cli_version}</span>
              {/if}
              <button class="icon-btn" onclick={refreshClaudeStatus} title="Rafraîchir">↻</button>
            </div>

            {#if !loginActive}
              <button class="btn primary" onclick={beginLogin} disabled={!claudeStatus.cli_installed}>
                {claudeStatus.logged_in ? "Régénérer la connexion (setup-token)" : "Se connecter à l'abonnement Claude"}
              </button>
            {:else}
              <div class="login-flow">
                <div class="login-steps">
                  1. Clique sur « Ouvrir le navigateur » et autorise l'accès —
                  2. Copie le code affiché —
                  3. Colle-le ci-dessous et valide
                </div>
                {#if loginUrl}
                  <button class="btn primary" onclick={() => openUrl(loginUrl!)}>
                    🌐 Ouvrir le navigateur
                  </button>
                {/if}
                <pre class="login-log">{loginLog || "Démarrage de claude setup-token…"}</pre>
                <div class="inline-row">
                  <input
                    type="text"
                    class="mono"
                    bind:value={loginCode}
                    placeholder="Colle ici le code d'autorisation"
                    spellcheck="false"
                    onkeydown={(e) => e.key === "Enter" && sendLoginCode()}
                  />
                  <button class="btn primary" onclick={sendLoginCode}>Valider</button>
                  <button class="btn danger" onclick={abortLogin}>Annuler</button>
                </div>
              </div>
            {/if}
          {/if}
        </section>

      {:else if view === "meetings"}
        <section class="card">
          <div class="card-head">
            <h3>Transcription &amp; résumé de réunions</h3>
            <p>Pipeline du bouton ⏺ : capture micro + son système, transcription Whisper, résumé par LLM déposé dans une note du projet.</p>
          </div>
          <label class="field">
            Clé API OpenAI
            <input type="text" class="mono" bind:value={apiKey} placeholder="sk-..." autocomplete="off" spellcheck="false" />
            <span class="field-hint">Utilisée pour la transcription (whisper-1) et le résumé.</span>
          </label>
          <label class="field">
            Modèle de résumé
            <input type="text" class="short" bind:value={summaryModel} placeholder="gpt-4o" spellcheck="false" />
          </label>
          <label class="field">
            Prompt système du résumé
            <textarea bind:value={summaryPrompt} rows="12"></textarea>
            <span class="field-hint">Pilote le niveau de détail du compte rendu. Surchargeable par projet dans ses paramètres.</span>
          </label>
          <div class="actions-row">
            <button class="btn primary" onclick={saveMeetingSettings} disabled={meetingSaving}>
              {meetingSaving ? 'Sauvegarde…' : 'Sauvegarder'}
            </button>
            {#if meetingSaved}<span class="feedback">✓ Enregistré</span>{/if}
          </div>
        </section>

      {:else}
        <section class="card">
          <div class="card-head">
            <h3>Projets enregistrés <span class="count">{dbProjects.length}</span></h3>
            <p>Les projets suivis par Cockpit. La suppression retire le projet de Cockpit sans toucher aux fichiers sur le disque.</p>
          </div>
          <table>
            <thead><tr><th>Nom</th><th>Chemin</th><th>Compose</th><th></th></tr></thead>
            <tbody>
              {#each dbProjects as p (p.id)}
                <tr>
                  <td class="name-cell">{p.name}</td>
                  <td class="path-cell">{p.path}</td>
                  <td>{p.compose_file || '—'}</td>
                  <td class="actions-cell">
                    <button class="btn danger small" onclick={() => doDelete(p.id)}>Supprimer</button>
                  </td>
                </tr>
              {/each}
            </tbody>
          </table>
        </section>
      {/if}
    </div>
  </div>
</div>

<style>
  .settings { max-width: 1060px; }
  /* La bibliotheque d'agents est en trois colonnes : elle respire mal a 1060 px. */
  .settings.wide { max-width: 1500px; }
  /* AgentsView est concu pour occuper une vue entiere (`height: 100%`). Encastre, son parent
     n'a pas de hauteur imposee : on lui en donne une, sinon il s'ecrase a zero. */
  .embedded-view { height: calc(100vh - var(--header-height) - 8rem); min-height: 26rem; }
  h2 { margin-bottom: 1.25rem; }

  .settings-layout { display: flex; gap: 1.5rem; align-items: flex-start; }

  /* Menu lateral */
  /* Titre de page integre au menu : il vivait au-dessus, pose a meme l image de fond,
     quasi illisible. Le menu porte deja un panneau en mode wallpaper. */
  .menu-title {
    margin: 0; padding: 0.35rem 0.8rem 0.55rem;
    font-size: 0.95rem; color: var(--text-primary);
    border-bottom: 1px solid var(--border-color); margin-bottom: 0.35rem;
  }
  .settings-menu {
    display: flex; flex-direction: column; gap: 0.25rem;
    width: 180px; flex-shrink: 0; position: sticky; top: 0;
  }
  .settings-menu-item {
    display: flex; align-items: center; gap: 0.6rem;
    text-align: left; padding: 0.5rem 0.8rem; font-size: 0.88rem;
    background: none; border: 1px solid transparent; border-radius: 6px;
    color: var(--text-secondary); cursor: pointer;
  }
  .settings-menu-item:hover { background: var(--bg-secondary); color: var(--text-primary); }
  .settings-menu-item.active {
    background: var(--bg-secondary); border-color: var(--border-color);
    color: var(--accent); font-weight: 600;
  }
  .menu-icon { width: 1.1rem; text-align: center; }

  /* Contenu */
  .settings-content {
    flex: 1; min-width: 0;
    /* Le gap disparait : les sections sont delimitees par des filets, pas par des interstices
       ou le fond apparaissait. Voir .stack dans components.css. */
    display: flex; flex-direction: column;
  }
  /* Fond, bordure, rayon et padding viennent desormais de `.stack` (components.css) : les
     sections forment un panneau continu au lieu d'ilots separes par des interstices. */
  .card-head { margin-bottom: 1.1rem; }
  .card-head h3 { margin: 0 0 0.25rem; font-size: 1rem; }
  .card-head p { margin: 0; font-size: 0.8rem; color: var(--text-muted); line-height: 1.45; }
  .count {
    font-size: 0.75rem; background: var(--accent); color: white;
    padding: 0.1rem 0.5rem; border-radius: 10px; vertical-align: middle; margin-left: 0.3rem;
  }

  /* Champs */
  .field { display: block; margin-bottom: 0.9rem; font-size: 0.85rem; color: var(--text-secondary); }
  .field:last-child { margin-bottom: 0; }
  .field input, .field textarea {
    display: block; width: 100%; margin-top: 0.3rem; padding: 0.45rem 0.65rem;
    border: 1px solid var(--border-color); border-radius: 6px; font-size: 0.88rem;
    background: var(--bg-primary); color: var(--text-primary);
  }
  .field input.short { max-width: 260px; }
  .field input.mono, .inline-row input.mono { font-family: monospace; font-size: 0.8rem; }
  .field textarea { resize: vertical; font-family: inherit; line-height: 1.5; }
  .field-hint { display: block; margin-top: 0.3rem; font-size: 0.72rem; color: var(--text-muted); }
  .field-row { display: flex; align-items: baseline; gap: 0.8rem; margin-bottom: 0.5rem; font-size: 0.85rem; }
  .field-row:last-child { margin-bottom: 0; }
  .field-label { width: 130px; flex-shrink: 0; color: var(--text-muted); }
  .field-value { color: var(--text-secondary); }
  .mono-value {
    font-family: monospace; font-size: 0.78rem; background: var(--bg-tertiary);
    padding: 0.15rem 0.45rem; border-radius: 4px; word-break: break-all;
  }
  .inline-row { display: flex; gap: 0.5rem; }
  .inline-row input {
    flex: 1; padding: 0.45rem 0.65rem; border: 1px solid var(--border-color); border-radius: 6px;
    background: var(--bg-primary); color: var(--text-primary); font-size: 0.88rem;
  }
  .actions-row { display: flex; align-items: center; gap: 0.75rem; margin-top: 0.75rem; }

  /* Boutons */
  .btn {
    padding: 0.4rem 0.95rem; border: none; border-radius: 6px;
    cursor: pointer; font-size: 0.85rem;
  }
  .btn:disabled { opacity: 0.5; cursor: default; }
  .btn.primary { background: var(--accent); color: white; }
  .btn.primary:hover:not(:disabled) { background: var(--accent-hover, var(--accent)); }
  .btn.danger {
    background: none; color: var(--error, #e5484d);
    border: 1px solid var(--error, #e5484d);
  }
  .btn.danger:hover { background: var(--error, #e5484d); color: white; }
  .btn.small { padding: 0.2rem 0.55rem; font-size: 0.78rem; }
  .icon-btn { background: none; border: none; cursor: pointer; color: var(--text-muted); font-size: 0.9rem; }
  .icon-btn:hover { color: var(--accent); }

  .feedback { margin: 0.5rem 0 0; font-size: 0.85rem; color: var(--success, #46a758); }
  .feedback.error { color: var(--error, #e5484d); }

  /* Claude */
  .badge { font-size: 0.78rem; font-weight: 700; padding: 0.15rem 0.55rem; border-radius: 10px; }
  .badge.on { background: rgba(70, 167, 88, 0.15); color: #46a758; }
  .badge.off { background: rgba(229, 72, 77, 0.12); color: #e5484d; }
  .claude-status-row { display: flex; align-items: center; gap: 0.6rem; flex-wrap: wrap; margin-bottom: 0.8rem; }
  .detail { font-size: 0.78rem; color: var(--text-secondary); }
  .detail.muted { color: var(--text-muted); }
  .login-flow { display: flex; flex-direction: column; gap: 0.5rem; }
  .login-steps { font-size: 0.8rem; color: var(--text-secondary); }
  .login-log {
    max-height: 180px; overflow-y: auto; margin: 0; padding: 0.5rem 0.7rem;
    background: var(--bg-tertiary); border: 1px solid var(--border-color); border-radius: 6px;
    font-size: 0.72rem; white-space: pre-wrap; word-break: break-all; color: var(--text-secondary);
  }

  /* Table projets */
  table { width: 100%; border-collapse: collapse; font-size: 0.85rem; table-layout: fixed; }
  th, td {
    text-align: left; padding: 0.45rem 0.6rem; border-bottom: 1px solid var(--border-color);
    overflow: hidden; text-overflow: ellipsis;
  }
  th:first-child, td:first-child { padding-left: 0; }
  tbody tr:last-child td { border-bottom: none; }
  tbody tr:hover td { background: var(--bg-tertiary); }
  th { color: var(--text-muted); font-weight: 600; font-size: 0.78rem; }
  .name-cell { font-weight: 600; color: var(--text-primary); width: 22%; white-space: normal; }
  .path-cell { font-family: monospace; font-size: 0.76rem; color: var(--text-secondary); }
  th:last-child, .actions-cell { width: 92px; text-align: right; padding-right: 0; }

  .changelog {
    max-height: 45vh; overflow-y: auto;
    font-size: 0.85rem; line-height: 1.65; color: var(--text-secondary);
  }
  .changelog :global(h1) { font-size: 1rem; color: var(--text-primary); margin: 0 0 0.5rem; }
  .changelog :global(h2) {
    font-size: 0.92rem; color: var(--text-primary);
    margin: 1.1rem 0 0.4rem; padding-top: 0.6rem;
    border-top: 1px solid var(--border-color);
  }
  .changelog :global(h1 + p), .changelog :global(h2 + p) { margin-top: 0; }
  .changelog :global(h3) { font-size: 0.82rem; color: var(--text-primary); margin: 0.7rem 0 0.25rem; }
  .changelog :global(p) { margin: 0 0 0.5rem; }
  .changelog :global(ul) { padding-left: 1.1rem; margin: 0 0 0.5rem; }
  .changelog :global(li) { margin: 0.15rem 0; }
  .changelog :global(a) { color: var(--accent); }
  .changelog :global(code) {
    font-family: var(--font-mono); font-size: 0.9em;
    background: var(--bg-tertiary); padding: 0.1em 0.3em; border-radius: 3px;
  }
</style>
