<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { listen, type UnlistenFn } from "@tauri-apps/api/event";
  import { getDbProjects, deleteDbProject } from "../../api/scanner";
  import { backupDatabase } from "../../api/storage";
  import { save as saveDialog } from "@tauri-apps/plugin-dialog";
  import { getAppSettings, setAppSetting } from "../../api/recorder";
  import { openUrl } from "../../api/workspace";
  import {
    abonnementLlm,
    demarrerConnexionLlm,
    entrerConnexionLlm,
    annulerConnexionLlm,
    choisirLlm,
    poserCleLlm,
    reunionsLlm,
    EVENEMENT_CONNEXION_SORTIE,
    EVENEMENT_CONNEXION_FIN,
    type EtatAbonnement,
    type AffectationsReunion,
  } from "../../api/llm";
  import { catalogue, agentPrefere, rafraichirLlm } from "../../stores/llm";
  import { loadProjects } from "../../stores/projects";
  import { forgetProjectTab } from "../../stores/ui";
  import { updateState, checkForUpdate } from "../../stores/update";
  import { trad, locale, setLocale, LOCALES, type Locale } from "../../i18n";
  import { signalerErreur } from "../../stores/errors";
  import { notify } from "../../stores/toast";
  import type { DbProject } from "../../types";
  import { onMount, onDestroy } from "svelte";
  import AppearanceSettings from "./AppearanceSettings.svelte";
  import CarteCompte from "../compte/CarteCompte.svelte";
  import AgentsView from "../agents/AgentsView.svelte";
  import { marked } from "marked";
  // Le CHANGELOG.md est embarque au build (Vite ?raw) : consultable hors ligne, et toujours
  // celui de la version installee — pas celui d'une branche distante.
  import changelogRaw from "../../../../CHANGELOG.md?raw";
  import { demanderConfirmation } from "../../stores/confirm";
  import { titresEnLangue, couperLesNotes } from "../../stores/notesDeVersion";

  /// Combien de sections du changelog sont rendues d'emblee. Le fichier compte 85 versions et
  /// ne fait que grossir : le rendre en entier coutait ~40 ms et posait 62 Ko de HTML dans la
  /// page a CHAQUE ouverture des Parametres, pour un historique que personne ne deroule.
  const SECTIONS_VISIBLES = 6;
  const MORCEAUX = couperLesNotes(changelogRaw, SECTIONS_VISIBLES);

  let toutLHistorique = $state(false);

  /// Rendus deja calcules, par langue et par morceau. Au niveau du MODULE et non du composant :
  /// les Parametres se montent et se demontent a chaque visite, et c'est justement la repetition
  /// qu'on veut cesser de payer.
  const RENDUS = new Map<string, string>();

  function rendre(md: string, quoi: string, langue: string): string {
    if (!md) return "";
    const cle = `${langue}|${quoi}`;
    const deja = RENDUS.get(cle);
    if (deja !== undefined) return deja;
    const html = marked.parse(titresEnLangue(md, $trad), { async: false }) as string;
    RENDUS.set(cle, html);
    return html;
  }

  // `$derived` et non `const` : les titres de section suivent la langue, qui se change sans
  // redemarrage. La langue fait partie de la cle, donc chaque langue est calculee une fois.
  const changelogHtml = $derived(rendre(MORCEAUX.tete, "tete", $locale));
  const resteHtml = $derived(toutLHistorique ? rendre(MORCEAUX.reste, "reste", $locale) : "");

  type SettingsView = "general" | "appearance" | "agents" | "ia" | "meetings" | "projects";
  let view: SettingsView = $state("general");


  /// Le menu des reglages. `capacite` rend une entree conditionnelle : les agents s'installent
  /// au format de plugins de Claude Code, en ecrivant dans la configuration de ce logiciel-la.
  /// Un fournisseur qui n'a pas ce concept n'a pas cet ecran, plutot qu'un ecran vide.
  const MENU: {
    id: SettingsView;
    icon: string;
    labelKey: `settings.menu.${SettingsView}`;
    capacite?: "plugins";
  }[] = [
    { id: "general", icon: "⚙", labelKey: "settings.menu.general" },
    { id: "appearance", icon: "◐", labelKey: "settings.menu.appearance" },
    { id: "agents", icon: "⬡", labelKey: "settings.menu.agents", capacite: "plugins" },
    { id: "ia", icon: "✳", labelKey: "settings.menu.ia" },
    { id: "meetings", icon: "⏺", labelKey: "settings.menu.meetings" },
    { id: "projects", icon: "▤", labelKey: "settings.menu.projects" },
  ];
  /// L'ordre d'affichage : le fournisseur choisi, puis ceux qu'on peut utiliser ici (CLI
  /// installe ou cle posee), puis le reste. Douze lignes dans un ordre fixe obligeaient a
  /// faire defiler pour trouver celui dont on se sert.
  const catalogueTrie = $derived(
    [...$catalogue].sort((a, b) => rang(a) - rang(b)),
  );

  function rang(f: { prefere: boolean; cli: boolean; cle_posee: boolean }): number {
    if (f.prefere) return 0;
    if (f.cli || f.cle_posee) return 1;
    return 2;
  }

  /// Les entrees affichables. Tant que le catalogue n'est pas lu, on n'en cache aucune.
  const menuVisible = $derived(
    MENU.filter((e) => !e.capacite || !$agentPrefere || $agentPrefere[e.capacite]),
  );

  let attachTranscript = $state(true);
  let dbProjects: DbProject[] = $state([]);
  let importPath = $state("");
  let importResult = $state("");
  let importFailed = $state(false);
  let importing = $state(false);
  let dbPath = $state("");
  let backingUp = $state(false);
  let backupResult = $state("");
  let backupFailed = $state(false);

  async function doBackup() {
    backingUp = true;
    backupResult = "";
    backupFailed = false;
    try {
      const d = new Date();
      const stamp = `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")}`;
      const dest = await saveDialog({
        title: $trad("settings.backup.dialogTitle"),
        defaultPath: `cockpit-sauvegarde-${stamp}.db`,
        filters: [{ name: $trad("settings.backup.fileKind"), extensions: ["db"] }],
      });
      if (!dest) return; // annule par l'utilisateur : pas une erreur
      await backupDatabase(dest);
      backupResult = $trad("settings.backup.written", { path: dest });
    } catch (e) {
      signalerErreur("global.doBackup", String(e));
      backupFailed = true;
      backupResult = `${$trad("common.error")} : ${e}`;
    } finally {
      backingUp = false;
    }
  }

  let summaryModel = $state("");
  let summaryPrompt = $state("");
  let meetingSaving = $state(false);
  let meetingSaved = $state(false);

  // --- Fournisseurs d'IA ---
  //
  // Le catalogue et le fournisseur choisi viennent du magasin : cet ecran les modifie, les
  // autres les lisent. Le detail de l'abonnement se demande a la demande — il coute un
  // lancement de processus pour connaitre la version du CLI.
  let abonnement: EtatAbonnement | null = $state(null);
  let affectations: AffectationsReunion | null = $state(null);
  /// Ce que l'on tape dans les champs de cle, par fournisseur. **La cle posee ne remonte jamais
  /// du backend** : un champ vide veut dire « inchangee », pas « effacee ».
  let clesSaisies: Record<string, string> = $state({});
  let cleEnregistree: string | null = $state(null);
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

  async function rafraichirAbonnement() {
    try { abonnement = await abonnementLlm(); } catch (e) {
      signalerErreur("global.abonnementLlm", String(e)); }
    try { affectations = await reunionsLlm(); } catch (e) {
      signalerErreur("global.reunionsLlm", String(e)); }
  }

  /// Choisit le fournisseur. Tout le reste de l'interface suit par le magasin.
  async function choisirFournisseur(id: string) {
    if (id === $agentPrefere?.id) return;
    try {
      await choisirLlm(id);
      await rafraichirLlm();
      await rafraichirAbonnement();
    } catch (e) {
      signalerErreur("global.choisirLlm", String(e));
      notify(String(e));
    }
  }

  async function enregistrerCle(id: string) {
    const valeur = clesSaisies[id] ?? "";
    if (!valeur.trim()) return;
    try {
      await poserCleLlm(id, valeur.trim());
      clesSaisies[id] = "";
      cleEnregistree = id;
      await rafraichirLlm();
      await rafraichirAbonnement();
    } catch (e) {
      signalerErreur("global.poserCleLlm", String(e));
      notify(String(e));
    }
  }

  async function beginLogin() {
    loginLog = "";
    loginCode = "";
    loginActive = true;
    loginUnlisteners.push(
      await listen<string>(EVENEMENT_CONNEXION_SORTIE, (e) => {
        loginLog = (loginLog + stripAnsi(e.payload)).slice(-4000);
      })
    );
    loginUnlisteners.push(
      await listen(EVENEMENT_CONNEXION_FIN, async () => {
        cleanupLoginListeners();
        loginActive = false;
        await rafraichirAbonnement();
      })
    );
    try { await demarrerConnexionLlm(); } catch (e) {
      signalerErreur("global.demarrerConnexionLlm", String(e)); loginLog = String(e); loginActive = false; }
  }

  function cleanupLoginListeners() {
    loginUnlisteners.forEach((u) => u());
    loginUnlisteners = [];
  }

  async function sendLoginCode() {
    if (!loginCode.trim()) return;
    try { await entrerConnexionLlm(loginCode.trim()); loginCode = ""; } catch (e) {
      signalerErreur("global.entrerConnexionLlm", String(e)); notify(String(e)); }
  }

  async function abortLogin() {
    try { await annulerConnexionLlm(); } catch (e) {
      signalerErreur("global.annulerConnexionLlm", String(e));}
    cleanupLoginListeners();
    loginActive = false;
  }

  /// La date suit la LANGUE de l'interface : `fr-FR` etait ecrit en dur, donc un anglophone
  /// lisait une date a la francaise au milieu d'une page anglaise.
  function expiryLabel(epochSecs: number): string {
    return new Date(epochSecs * 1000).toLocaleString($locale === "fr" ? "fr-FR" : "en-US");
  }

  onDestroy(() => { cleanupLoginListeners(); });

  onMount(async () => {
    await loadDbProjects();
    // Trois pannes differentes portaient le meme `scope` recopie : il sert justement a situer
    // laquelle a eu lieu.
    try { dbPath = await invoke<string>("get_db_path"); } catch (e) {
      signalerErreur("global.cheminBase", String(e));}
    try {
      const s = await getAppSettings();
      summaryModel = s.summary_model ?? "";
      summaryPrompt = s.summary_prompt ?? "";
      // Absent = joindre, pour ne pas changer le comportement des comptes rendus existants.
      attachTranscript = s.attach_transcript !== "off";
    } catch (e) {
      signalerErreur("global.reglagesApplication", String(e));}
    await rafraichirLlm();
    await rafraichirAbonnement();
  });

  async function saveMeetingSettings() {
    meetingSaving = true;
    meetingSaved = false;
    try {
      // La cle d'API ne se pose plus ici : elle vit dans l'ecran IA, par fournisseur.
      await setAppSetting("summary_model", summaryModel.trim());
      await setAppSetting("summary_prompt", summaryPrompt.trim());
      await setAppSetting("attach_transcript", attachTranscript ? "on" : "off");
      meetingSaved = true;
      setTimeout(() => { meetingSaved = false; }, 3000);
    } catch (e) {
      signalerErreur("global.saveMeetingSettings", String(e)); notify(String(e)); }
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
    } catch (e) {
      signalerErreur("global.doImport", String(e));
      importFailed = true;
      importResult = `${$trad("common.error")} : ${e}`;
    } finally { importing = false; }
  }

  async function loadDbProjects() {
    try { dbProjects = await getDbProjects(); } catch (e) {
      signalerErreur("global.loadDbProjects", String(e));}
  }

  async function doDelete(id: number, name: string) {
    if (!(await demanderConfirmation({ message: $trad("settings.projects.deleteConfirm"), action: $trad("common.delete") }))) return;
    try {
      await deleteDbProject(id);
      // Le projet disparait : son onglet memorise aussi, sinon un projet recree sous le
      // meme nom ressortirait celui du precedent.
      forgetProjectTab(name);
      await loadDbProjects();
      await loadProjects();
    } catch (e) {
      signalerErreur("global.doDelete", String(e)); notify(String(e)); }
  }
</script>

<div class="settings" class:wide={view === "agents"}>

  <div class="settings-layout">
    <nav class="settings-menu">
      <h2 class="menu-title">{$trad("settings.title")}</h2>
      {#each menuVisible as item (item.id)}
        <button
          class="settings-menu-item"
          class:active={view === item.id}
          onclick={() => (view = item.id)}
        >
          <span class="menu-icon">{item.icon}</span>
          {$trad(item.labelKey)}
        </button>
      {/each}
    </nav>

    <div class="settings-content stack">
      {#if view === "general"}
        <CarteCompte />
        <section class="card">
          <div class="card-head">
            <h3>{$trad("settings.app.title")}</h3>
            <p>{$trad("settings.app.subtitle")}</p>
          </div>
          <div class="field-row">
            <span class="field-label">{$trad("settings.app.database")}</span>
            <code class="mono-value">{dbPath}</code>
          </div>
          <div class="field-row">
            <span class="field-label">{$trad("settings.app.version")}</span>
            <span class="field-value">{$updateState.currentVersion || '…'}</span>
          </div>
          <div class="field-row">
            <span class="field-label">{$trad("settings.app.build")}</span>
            <span class="field-value">{__BUILD_TIME__}</span>
          </div>
          <div class="inline-row">
            <button class="btn" onclick={() => checkForUpdate()} disabled={$updateState.phase === 'checking'}>
              {$updateState.phase === 'checking' ? $trad("settings.app.checking") : $trad("settings.app.checkUpdates")}
            </button>
            {#if $updateState.newVersion}
              <span class="feedback">{$trad("settings.app.updateAvailable", { version: $updateState.newVersion })}</span>
            {/if}
          </div>
        </section>

        <section class="card">
          <div class="card-head">
            <h3>{$trad("settings.language")}</h3>
            <p>{$trad("settings.languageHelp")}</p>
          </div>
          <div class="field-row">
            <span class="field-label">{$trad("settings.language")}</span>
            <select
              class="input"
              value={$locale}
              onchange={(e) => setLocale((e.currentTarget as HTMLSelectElement).value as Locale)}
            >
              {#each LOCALES as l (l.id)}
                <option value={l.id}>{l.label}</option>
              {/each}
            </select>
          </div>
        </section>

        <section class="card">
          <div class="card-head">
            <h3>{$trad("settings.changelog.title")}</h3>
            <p>{$trad("settings.changelog.subtitle")}</p>
          </div>
          <div class="changelog">{@html changelogHtml}</div>
          {#if MORCEAUX.reste && !toutLHistorique}
            <button class="btn small" onclick={() => (toutLHistorique = true)}>
              {$trad("settings.changelog.toutVoir")}
            </button>
          {:else if resteHtml}
            <div class="changelog">{@html resteHtml}</div>
          {/if}
        </section>

        <section class="card">
          <div class="card-head">
            <h3>{$trad("settings.backup.title")}</h3>
            <p>{$trad("settings.backup.subtitle")}</p>
          </div>
          <button class="btn primary" onclick={doBackup} disabled={backingUp}>
            {backingUp ? $trad("settings.backup.running") : $trad("settings.backup.button")}
          </button>
          {#if backupResult}
            <p class="feedback" class:error={backupFailed}>{backupResult}</p>
          {/if}
        </section>

        <section class="card">
          <div class="card-head">
            <h3>{$trad("settings.import.title")}</h3>
            <p>{$trad("settings.import.subtitle")}</p>
          </div>
          <div class="inline-row">
            <input type="text" bind:value={importPath} placeholder={$trad("settings.import.pathPlaceholder")} spellcheck="false" />
            <button class="btn primary" onclick={doImport} disabled={importing}>
              {importing ? $trad("settings.import.running") : $trad("settings.import.button")}
            </button>
          </div>
          {#if importResult}
            <p class="feedback" class:error={importFailed}>{importResult}</p>
          {/if}
        </section>

      {:else if view === "appearance"}
        <AppearanceSettings />

      {:else if view === "agents"}
        <!-- Encastree dans les parametres : la grille de AgentsView est fluide, elle s'adapte
             a la colonne. `.settings.wide` elargit la page pour lui laisser de l'air. -->
        <div class="embedded-view"><AgentsView /></div>

      {:else if view === "ia"}
        <!-- LE CHOIX DU FOURNISSEUR, et rien qu'ici. Tout le reste de l'application le lit :
             le bouton des conversations du terminal, l'onglet Plugins, les comptes rendus de
             reunion. Chaque ligne dit ce que le fournisseur SAIT FAIRE sur cette machine —
             cacher ce qu'il ne sait pas faire vaut mieux qu'un bouton qui promet. -->
        <!-- L'abonnement : seulement pour un fournisseur qui en gere un. Les autres n'ont
             rien a connecter, et un bouton grise sans explication vaut mieux absent. -->
        {#if abonnement?.gere_abonnement}
          <section class="card">
            <div class="card-head">
              <h3>{$trad("settings.ia.abonnementTitre", { nom: abonnement.nom })}</h3>
              <p>{$trad("settings.ia.abonnementSoustitre", { nom: abonnement.nom })}</p>
            </div>
            <div class="abonnement-row">
              {#if !abonnement.cli_installe}
                <span class="badge off">{$trad("settings.ia.cliMissing", { nom: abonnement.nom })}</span>
              {:else if abonnement.connecte}
                <span class="badge on">{$trad("settings.ia.connected")}</span>
                {#if abonnement.formule}
                  <span class="detail">{$trad("settings.ia.subscription")} <strong>{abonnement.formule}</strong></span>
                {/if}
                {#if abonnement.palier}
                  <span class="detail">{$trad("settings.ia.tier", { tier: abonnement.palier })}</span>
                {/if}
                {#if abonnement.expire_le}
                  <span class="detail">{$trad("settings.ia.tokenValidUntil", { date: expiryLabel(abonnement.expire_le) })}</span>
                {/if}
              {:else}
                <span class="badge off">{$trad("settings.ia.notConnected")}</span>
              {/if}
              {#if abonnement.cli_version}
                <span class="detail muted">{abonnement.cli_version}</span>
              {/if}
              {#if abonnement.probleme}
                <span class="detail probleme">{$trad("settings.ia.problem")} {abonnement.probleme}</span>
              {/if}
              <button class="icon-btn" onclick={rafraichirAbonnement} title={$trad("common.refresh")}>↻</button>
            </div>

            {#if abonnement.connexion_guidee}
              {#if !loginActive}
                <button class="btn primary" onclick={beginLogin} disabled={!abonnement.cli_installe}>
                  {abonnement.connecte ? $trad("settings.ia.regenerate") : $trad("settings.ia.connect")}
                </button>
              {:else}
                <div class="login-flow">
                  <div class="login-steps">
                    {$trad("settings.ia.steps")}
                  </div>
                  {#if loginUrl}
                    <button class="btn primary" onclick={() => openUrl(loginUrl!)}>
                      {$trad("settings.ia.openBrowser")}
                    </button>
                  {/if}
                  <pre class="login-log">{loginLog || $trad("settings.ia.loginStarting")}</pre>
                  <div class="inline-row">
                    <input
                      type="text"
                      class="mono"
                      bind:value={loginCode}
                      placeholder={$trad("settings.ia.codePlaceholder")}
                      spellcheck="false"
                      onkeydown={(e) => e.key === "Enter" && sendLoginCode()}
                    />
                    <button class="btn primary" onclick={sendLoginCode}>{$trad("settings.ia.validate")}</button>
                    <button class="btn danger" onclick={abortLogin}>{$trad("common.cancel")}</button>
                  </div>
                </div>
              {/if}
            {/if}
          </section>
        {/if}

        <section class="card">
          <div class="card-head">
            <h3>{$trad("settings.ia.title")}</h3>
            <p>{$trad("settings.ia.subtitle")}</p>
          </div>

          <ul class="fournisseurs">
            {#each catalogueTrie as f (f.id)}
              <li class="fournisseur" class:choisi={f.prefere}>
                <label class="fournisseur-choix">
                  <input
                    type="radio"
                    name="fournisseur-ia"
                    checked={f.prefere}
                    onchange={() => choisirFournisseur(f.id)}
                  />
                  <span class="fournisseur-symbole" style:color={f.couleur}>{f.symbole}</span>
                  <span class="fournisseur-nom">{f.nom}</span>
                </label>

                <div class="fournisseur-etat">
                  {#if f.a_un_cli}
                    <span class="badge" class:on={f.cli} class:off={!f.cli}>
                      {f.cli ? $trad("settings.ia.cliPresent") : $trad("settings.ia.cliAbsent")}
                    </span>
                  {/if}
                  {#if f.cle_requise}
                    <span class="badge" class:on={f.cle_posee} class:off={!f.cle_posee}>
                      {f.cle_posee ? $trad("settings.ia.clePosee") : $trad("settings.ia.cleAbsente")}
                    </span>
                  {/if}
                </div>

                <div class="fournisseur-capacites">
                  {#if f.conversations}<span class="capacite">{$trad("settings.ia.capConversations")}</span>{/if}
                  {#if f.abonnement}<span class="capacite">{$trad("settings.ia.capAbonnement")}</span>{/if}
                  {#if f.texte}<span class="capacite">{$trad("settings.ia.capTexte")}</span>{/if}
                  {#if f.transcription}<span class="capacite">{$trad("settings.ia.capTranscription")}</span>{/if}
                  {#if f.plugins}<span class="capacite">{$trad("settings.ia.capPlugins")}</span>{/if}
                </div>

                {#if f.cle_requise}
                  <div class="inline-row cle">
                    <input
                      type="password"
                      class="mono"
                      bind:value={clesSaisies[f.id]}
                      placeholder={f.cle_posee ? $trad("settings.ia.clePlaceholderPosee") : $trad("settings.ia.clePlaceholder")}
                      autocomplete="off"
                      spellcheck="false"
                      onkeydown={(e) => e.key === "Enter" && enregistrerCle(f.id)}
                    />
                    <button class="btn" onclick={() => enregistrerCle(f.id)} disabled={!clesSaisies[f.id]?.trim()}>
                      {$trad("common.save")}
                    </button>
                    {#if cleEnregistree === f.id}<span class="saved">✓</span>{/if}
                  </div>
                {/if}
              </li>
            {/each}
          </ul>
          <p class="field-hint">{$trad("settings.ia.ajouter")}</p>
        </section>

      {:else if view === "meetings"}
        <section class="card">
          <div class="card-head">
            <h3>{$trad("settings.meetings.title")}</h3>
            <p>{$trad("settings.meetings.subtitle")}</p>
          </div>
          <!-- QUI FAIT LE TRAVAIL EST AFFICHE, pas devine. Le fournisseur choisi s'il sait
               transcrire et rediger, sinon le premier capable et configure : choisir Claude et
               voir la transcription partir ailleurs est normal (il ne transcrit pas), mais ca
               ne doit pas se decouvrir apres coup. La regle vit cote Rust, une seule fois. -->
          {#if affectations}
            <p class="field-hint qui-travaille">
              {#if affectations.transcription && affectations.redaction}
                {$trad("settings.meetings.par", {
                  transcription: affectations.transcription,
                  redaction: affectations.redaction,
                })}
              {:else}
                <span class="probleme">{$trad("settings.meetings.aucunFournisseur")}</span>
              {/if}
            </p>
          {/if}
          <label class="field">
            {$trad("settings.meetings.model")}
            <input type="text" class="short" bind:value={summaryModel} placeholder="gpt-4o" spellcheck="false" />
          </label>
          <label class="field">
            {$trad("settings.meetings.prompt")}
            <textarea bind:value={summaryPrompt} rows="12"></textarea>
            <span class="field-hint">{$trad("settings.meetings.promptHint")}</span>
          </label>
          <label class="check-row">
            <input type="checkbox" bind:checked={attachTranscript} />
            <span>{$trad("settings.meetings.attachTranscript")}</span>
          </label>
          <p class="field-hint">{$trad("settings.meetings.attachTranscriptHelp")}</p>
          <div class="actions-row">
            <button class="btn primary" onclick={saveMeetingSettings} disabled={meetingSaving}>
              {meetingSaving ? $trad("projectSettings.saving") : $trad("common.save")}
            </button>
            {#if meetingSaved}<span class="feedback">{$trad("settings.meetings.saved")}</span>{/if}
          </div>
        </section>

      {:else}
        <section class="card">
          <div class="card-head">
            <h3>{$trad("settings.projects.title")} <span class="count">{dbProjects.length}</span></h3>
            <p>{$trad("settings.projects.subtitle")}</p>
          </div>
          <table>
            <thead><tr><th>{$trad("settings.projects.colName")}</th><th>{$trad("settings.projects.colPath")}</th><th>{$trad("settings.projects.colCompose")}</th><th></th></tr></thead>
            <tbody>
              {#each dbProjects as p (p.id)}
                <tr>
                  <td class="name-cell">{p.name}</td>
                  <td class="path-cell">{p.path}</td>
                  <td>{p.compose_file || '—'}</td>
                  <td class="actions-cell">
                    <button class="btn danger small" onclick={() => doDelete(p.id, p.name)}>{$trad("common.delete")}</button>
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
  /* .card-head : remonte dans components.css, voir le commentaire la-bas. */
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
  .inline-row input.mono { font-family: monospace; font-size: 0.8rem; }
  .field textarea { resize: vertical; font-family: inherit; line-height: 1.5; }
  .field-hint { display: block; margin-top: 0.3rem; font-size: 0.72rem; color: var(--text-muted); }
  /* .field-row, .field-label, .field-value : remontes dans components.css. */
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
  .abonnement-row { display: flex; align-items: center; gap: 0.6rem; flex-wrap: wrap; margin-bottom: 0.8rem; }
  .detail { font-size: 0.78rem; color: var(--text-secondary); }
  .detail.probleme { color: var(--error); }
  .detail.muted { color: var(--text-muted); }
  .login-flow { display: flex; flex-direction: column; gap: 0.5rem; }

  /* ── La liste des fournisseurs d'IA.
     Une ligne par fournisseur : le choix, ce qui est installe, ce qu'il sait faire. Les
     capacites sont ecrites en clair et non devinees : c'est ce qui evite de chercher pourquoi
     un bouton n'apparait pas ailleurs. */
  .fournisseurs { list-style: none; margin: 0; padding: 0; display: flex; flex-direction: column; gap: 0.5rem; }
  .fournisseur {
    display: grid;
    grid-template-columns: 1fr auto;
    gap: 0.35rem 0.8rem;
    padding: 0.7rem 0.85rem;
    border: 1px solid var(--border);
    border-radius: 8px;
    background: var(--bg-secondary);
  }
  .fournisseur.choisi { border-color: var(--accent); background: var(--bg-tertiary); }
  .fournisseur-choix { display: flex; align-items: center; gap: 0.55rem; cursor: pointer; }
  .fournisseur-symbole { font-weight: 700; font-size: 1rem; line-height: 1; }
  .fournisseur-nom { font-weight: 600; }
  .fournisseur-etat { display: flex; align-items: center; gap: 0.4rem; flex-wrap: wrap; justify-content: flex-end; }
  .fournisseur-capacites { grid-column: 1 / -1; display: flex; gap: 0.35rem; flex-wrap: wrap; }
  .capacite {
    font-size: 0.72rem;
    color: var(--text-secondary);
    background: var(--bg-primary);
    border: 1px solid var(--border);
    border-radius: 999px;
    padding: 0.05rem 0.5rem;
  }
  .inline-row.cle { grid-column: 1 / -1; }
  .inline-row.cle input { flex: 1; min-width: 0; }
  .saved { color: #46a758; font-weight: 700; }
  .qui-travaille { margin-top: 0; }
  /* Un fournisseur manquant EMPECHE le compte rendu : ca ne se dit pas en gris clair. */
  .qui-travaille .probleme { color: var(--error); font-weight: 600; }
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
