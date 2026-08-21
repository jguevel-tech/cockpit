<script lang="ts">
  import { projects } from "../../stores/projects";
  import { selectedProject, selectProject, activeTab, pendingTerminalId } from "../../stores/ui";
  import { terminals, loadTerminals } from "../../stores/terminals";
  import { renameTerminal, closeTerminal } from "../../api/workspace";
  import type { TerminalInfo } from "../../types";
  import { reorderProjects, getProjectFolders, createProjectFolder, renameProjectFolder, deleteProjectFolder, reorderProjectFolders, moveProjectFolder, moveProjectToFolder } from "../../api/scanner";
  import { loadProjects, renommerProjet } from "../../stores/projects";
  import type { Project, ProjectFolder } from "../../types";
  import CreateProjectModal from "./CreateProjectModal.svelte";
  import InlineEdit from "../ui/InlineEdit.svelte";
  import ContextMenu from "../ui/ContextMenu.svelte";
  // Logo officiel Claude (fill terracotta embarque dans le SVG) : affiche quand un agent
  // IA tourne dans le terminal, a la place de l ancienne pastille verte.
  import claudeLogo from "../../assets/claude-logo.svg";
  import { notify } from "../../stores/toast";
  import { onMount } from "svelte";
  import { trad, tradN } from "../../i18n";
  import { signalerErreur } from "../../stores/errors";
  import { demanderConfirmation } from "../../stores/confirm";

  let showCreateModal = $state(false);
  let folders: ProjectFolder[] = $state([]);
  let collapsedIds: Set<number> = $state(new Set());
  let renamingFolderId: number | null = $state(null);
  let contextMenu: { id: number; x: number; y: number } | null = $state(null);
  /** Saisie d'un nouveau dossier : `parentId` a null = premier niveau. */
  let creation: { parentId: number | null } | null = $state(null);
  let newFolderName = $state("");

  /**
   * Indentation : un retrait ABSOLU calcule depuis la profondeur, pas des `padding-left`
   * imbriques qui s'additionneraient sans fin. Meme approche que l'arbre de l'onglet
   * Fichiers. Plafonne parce que la barre laterale a une largeur fixe (260 px) : au-dela
   * de 8 niveaux on cesse d'indenter plutot que de reduire le nom a rien.
   */
  const RETRAIT_PAS = 0.75;
  const RETRAIT_MAX = 8;
  function retrait(base: number, profondeur: number): string {
    return `${base + Math.min(profondeur, RETRAIT_MAX) * RETRAIT_PAS}rem`;
  }

  let terminalsCollapsed = $state(false);

  onMount(() => {
    loadFolders();
    loadTerminals();
    // Restore collapsed state from localStorage
    try {
      const saved = localStorage.getItem("cockpit-collapsed-folders");
      if (saved) collapsedIds = new Set(JSON.parse(saved));
      terminalsCollapsed = localStorage.getItem("cockpit-terminals-collapsed") === "1";
    } catch (e) {
      signalerErreur("sidebar.onMount", String(e));}
  });

  function toggleTerminals() {
    terminalsCollapsed = !terminalsCollapsed;
    localStorage.setItem("cockpit-terminals-collapsed", terminalsCollapsed ? "1" : "0");
  }

  function gotoTerminal(t: TerminalInfo) {
    pendingTerminalId.set(t.id);
    selectProject(t.project);
    activeTab.set("terminal");
  }

  function terminalLabel(t: TerminalInfo): string {
    return t.name || "Terminal";
  }

  // Menu contextuel + renommage inline des terminaux
  let termContextMenu: { id: number; x: number; y: number } | null = $state(null);
  let renamingTermId: number | null = $state(null);

  function openTermContextMenu(e: MouseEvent, t: TerminalInfo) {
    e.preventDefault();
    termContextMenu = { id: t.id, x: e.clientX, y: e.clientY };
  }

  async function commitRenameTerminal(id: number, next: string) {
    renamingTermId = null;
    try { await renameTerminal(id, next); } catch (e) { notify(String(e)); }
    await loadTerminals();
  }

  async function closeTerminalById(id: number) {
    try { await closeTerminal(id); } catch (e) { notify(String(e)); }
    await loadTerminals();
  }

  // Renommage d'un projet : meme paire de gestes que les dossiers et les terminaux
  // (double-clic sur le nom, clic droit -> Renommer). Le renommage marchait depuis
  // toujours mais nulle part ici : personne ne le trouvait (issue #6).
  let projContextMenu: { name: string; x: number; y: number } | null = $state(null);
  let renamingProjectName: string | null = $state(null);

  function openProjContextMenu(e: MouseEvent, name: string) {
    e.preventDefault();
    projContextMenu = { name, x: e.clientX, y: e.clientY };
  }

  async function commitRenameProject(oldName: string, next: string) {
    renamingProjectName = null;
    await renommerProjet(oldName, next);
  }

  async function loadFolders() {
    try { folders = await getProjectFolders(); } catch (e) { notify(String(e)); }
  }

  function saveCollapsed() {
    localStorage.setItem("cockpit-collapsed-folders", JSON.stringify([...collapsedIds]));
  }

  function toggleFolder(id: number) {
    const next = new Set(collapsedIds);
    if (next.has(id)) next.delete(id); else next.add(id);
    collapsedIds = next;
    saveCollapsed();
  }

  // Projects grouped by folder
  let rootProjects = $derived($projects.filter(p => p.folder_id === null));
  let projectsByFolder = $derived(() => {
    const map = new Map<number, Project[]>();
    for (const p of $projects) {
      if (p.folder_id !== null) {
        if (!map.has(p.folder_id)) map.set(p.folder_id, []);
        map.get(p.folder_id)!.push(p);
      }
    }
    return map;
  });

  function getFolderProjects(folderId: number): Project[] {
    return projectsByFolder().get(folderId) || [];
  }

  // --- Arborescence des dossiers (imbrication sans limite, issue #2) ---

  /**
   * Dossiers ranges par parent. Un dossier dont le `parent_id` ne resout pas (parent
   * disparu) est rattache a la RACINE plutot que perdu : un dossier en base doit toujours
   * apparaitre quelque part, sinon il devient inaccessible sans etre supprime.
   */
  let dossiersParParent = $derived.by(() => {
    const connus = new Set(folders.map((f) => f.id));
    const map = new Map<number | null, ProjectFolder[]>();
    for (const f of folders) {
      const parent = f.parent_id !== null && connus.has(f.parent_id) ? f.parent_id : null;
      if (!map.has(parent)) map.set(parent, []);
      map.get(parent)!.push(f);
    }
    return map;
  });

  function sousDossiers(parentId: number | null): ProjectFolder[] {
    return dossiersParParent.get(parentId) ?? [];
  }

  function dossierParId(id: number): ProjectFolder | undefined {
    return folders.find((f) => f.id === id);
  }

  /**
   * Compteur affiche a cote du nom : les projets du dossier ET de tous ses sous-dossiers.
   * Le compte direct afficherait « 0 » sur un dossier replie qui contient pourtant des
   * projets deux niveaux plus bas — le badge ne servirait alors a rien, justement quand il
   * est le seul indice disponible.
   */
  function compteProjets(folderId: number): number {
    let n = getFolderProjects(folderId).length;
    for (const sous of sousDossiers(folderId)) n += compteProjets(sous.id);
    return n;
  }

  /** `cible` est-il `id` lui-meme ou un de ses descendants ? (refus des boucles) */
  function estDescendant(id: number, cible: number | null): boolean {
    let courant = cible;
    let pas = 0;
    while (courant !== null && pas++ < 1000) {
      if (courant === id) return true;
      courant = dossierParId(courant)?.parent_id ?? null;
    }
    return false;
  }

  function deplier(id: number) {
    if (!collapsedIds.has(id)) return;
    const next = new Set(collapsedIds);
    next.delete(id);
    collapsedIds = next;
    saveCollapsed();
  }

  // Folder CRUD
  function demarrerCreation(parentId: number | null) {
    newFolderName = "";
    creation = { parentId };
    // Sinon le champ de saisie apparait dans une branche repliee, donc nulle part.
    if (parentId !== null) deplier(parentId);
  }

  async function addFolder() {
    if (!creation) return;
    const nom = newFolderName.trim();
    if (!nom) {
      notify($trad("sidebar.folderNameRequired"), "error", 4000, { report: false });
      return;
    }
    const parentId = creation.parentId;
    try {
      await createProjectFolder(nom, parentId);
      newFolderName = "";
      creation = null;
      await loadFolders();
    } catch (e) { notify(String(e)); }
  }

  async function commitRenameFolder(id: number, next: string) {
    renamingFolderId = null;
    try {
      await renameProjectFolder(id, next);
      await loadFolders();
    } catch (e) { notify(String(e)); }
  }

  function openContextMenu(e: MouseEvent, id: number) {
    e.preventDefault();
    contextMenu = { id, x: e.clientX, y: e.clientY };
  }

  /** Ce qui empeche de supprimer un dossier, redige — ou null s'il est vide. */
  function raisonNonVide(id: number): string | null {
    const projets = getFolderProjects(id).length;
    const dossiers = sousDossiers(id).length;
    if (projets === 0 && dossiers === 0) return null;
    const morceaux: string[] = [];
    if (projets > 0) morceaux.push($tradN("sidebar.countProjects", projets));
    if (dossiers > 0) morceaux.push($tradN("sidebar.countSubfolders", dossiers));
    return $trad("sidebar.folderNotEmptyDetail", {
      contenu: morceaux.join(` ${$trad("common.and")} `),
    });
  }

  async function deleteFolder(id: number) {
    // Un dossier ne se supprime que VIDE : le supprimer plein detacherait son contenu en
    // silence — et avec l'imbrication ce serait une branche entiere, peut-etre repliee donc
    // invisible. On explique au lieu d'agir.
    const raison = raisonNonVide(id);
    if (raison) {
      notify(raison, "error", 4000, { report: false });
      return;
    }
    // Le dossier est vide, donc rien ne disparait avec lui — mais la question nomme quand meme
    // ce qu'on supprime : c'est le geste qu'on confirme, pas la quantite perdue.
    const nom = folders.find((f) => f.id === id)?.name ?? "";
    const question = $trad("sidebar.deleteFolderConfirm", { nom });
    if (!(await demanderConfirmation({ message: question, action: $trad("common.delete") }))) return;
    try {
      await deleteProjectFolder(id);
      await loadFolders();
      await loadProjects();
    } catch (e) {
      notify(String(e));
    }
  }

  /**
   * Range un dossier sous un autre (`parentId` a null = racine).
   * Le refus des boucles est controle ICI pour donner un message traduit : le backend a la
   * meme garde, mais son message est en dur cote Rust — une contrainte de stockage n'est
   * jamais un message d'interface.
   */
  async function rangerDossier(id: number, parentId: number | null): Promise<boolean> {
    const dossier = dossierParId(id);
    if (!dossier) {
      notify($trad("sidebar.folderGone"), "error", 4000, { report: false });
      return false;
    }
    if (parentEffectif(dossier) === parentId) return true; // deja au bon endroit
    if (parentId !== null && (parentId === id || estDescendant(id, parentId))) {
      notify($trad("sidebar.folderCycle"), "error", 4000, { report: false });
      return false;
    }
    try {
      await moveProjectFolder(id, parentId);
    } catch (e) {
      notify(String(e), "error", 4000, { scope: "dossier.deplacement" });
      return false;
    }
    // Le resultat doit se VOIR : un depot dans un dossier replie serait un clic sans effet.
    if (parentId !== null) deplier(parentId);
    const nomCible = parentId === null ? null : dossierParId(parentId)?.name;
    notify(
      nomCible
        ? $trad("sidebar.folderMovedInto", { folder: dossier.name, target: nomCible })
        : $trad("sidebar.folderMovedToRoot", { folder: dossier.name }),
      "success",
      2500,
    );
    await loadFolders();
    return true;
  }

  // Drag & drop: reorder + move to folder
  let dragProjectName: string | null = $state(null);
  let dropTarget: { name: string; pos: "before" | "after" } | null = $state(null);

  function onProjectDragStart(e: DragEvent, name: string) {
    // stopPropagation : la ligne projet est DANS le <li> du dossier, lui aussi glissable.
    // Sans ca, glisser un projet demarrerait aussi le glisser de son dossier.
    e.stopPropagation();
    dragProjectName = name;
    dragFolderId = null;
    e.dataTransfer!.effectAllowed = "move";
    e.dataTransfer!.setData("text/plain", name);
  }

  function onProjectDragOver(e: DragEvent, targetName: string) {
    e.preventDefault();
    // La ligne visee gagne : sans ca la zone racine qui l'englobe s'allumerait aussi et
    // deux retours visuels differents s'afficheraient pour un seul depot.
    e.stopPropagation();
    cibleDossier = null;
    survolRacine = false;
    if (!dragProjectName || dragProjectName === targetName) { dropTarget = null; return; }
    const rect = (e.currentTarget as HTMLElement).getBoundingClientRect();
    const pos = e.clientY < rect.top + rect.height / 2 ? "before" : "after";
    dropTarget = { name: targetName, pos };
  }

  function onProjectDragLeave() { dropTarget = null; }

  function onProjectDrop(e: DragEvent, targetList: Project[]) {
    e.preventDefault();
    e.stopPropagation();
    if (!dragProjectName || !dropTarget) { finGlisser(); return; }

    // Reorder within the same list
    const names = targetList.map(p => p.name);
    const fromIdx = names.indexOf(dragProjectName);
    if (fromIdx === -1) {
      // Projet vient d'un autre contexte (autre dossier ou racine) — on le deplace d'abord
      const targetProj = $projects.find(p => p.name === dropTarget!.name);
      if (targetProj) void deplacerProjet(dragProjectName, targetProj.folder_id);
      finGlisser();
      return;
    }

    const items = [...names];
    items.splice(fromIdx, 1);
    let targetIdx = items.indexOf(dropTarget.name);
    if (dropTarget.pos === "after") targetIdx++;
    items.splice(targetIdx, 0, dragProjectName);

    // Optimistic update: recompose full project list preserving non-affected
    const reordered = items.map(n => targetList.find(p => p.name === n)!);
    const otherProjects = $projects.filter(p => !targetList.includes(p));
    projects.set([...otherProjects, ...reordered]);

    // L'echec ne peut pas rester muet : l'ordre affiche vient d'etre change de facon
    // optimiste, il faut dire pourquoi il revient en arriere.
    reorderProjects(items).catch((e) => {
      notify(String(e), "error", 4000, { scope: "projet.reordonnancement" });
      void loadProjects();
    });
    finGlisser();
  }

  function onProjectDragEnd() { finGlisser(); }

  // --- Glisser-deposer des DOSSIERS ---
  // Un en-tete de dossier a trois zones : le quart haut et le quart bas reordonnent DANS LA
  // FRATRIE (trait bleu au-dessus / au-dessous), la moitie centrale range DEDANS (l'en-tete
  // entier s'entoure). Sans cette distinction visible, l'utilisateur ne sait pas ce que son
  // depot va faire.
  let dragFolderId: number | null = $state(null);
  let cibleDossier: { id: number; zone: "avant" | "dedans" | "apres" | "interdit" } | null = $state(null);
  let survolRacine = $state(false);

  function finGlisser() {
    dragProjectName = null;
    dragFolderId = null;
    dropTarget = null;
    cibleDossier = null;
    survolRacine = false;
  }

  /** Parent reellement affiche (un parent disparu = dossier remonte a la racine). */
  function parentEffectif(f: ProjectFolder): number | null {
    return f.parent_id !== null && dossierParId(f.parent_id) ? f.parent_id : null;
  }

  function onFolderDragStart(e: DragEvent, folder: ProjectFolder) {
    e.stopPropagation();
    dragFolderId = folder.id;
    dragProjectName = null;
    e.dataTransfer!.effectAllowed = "move";
    e.dataTransfer!.setData("text/plain", folder.name);
  }

  function onFolderHeaderDragOver(e: DragEvent, folder: ProjectFolder) {
    if (!dragProjectName && dragFolderId === null) return;
    e.preventDefault();
    e.stopPropagation();
    survolRacine = false;
    // Un projet ne se depose que DEDANS : il n'a pas de place « a cote » d'un dossier.
    if (dragProjectName) {
      cibleDossier = { id: folder.id, zone: "dedans" };
      return;
    }
    if (estDescendant(dragFolderId!, folder.id)) {
      // Le dossier vise est le dossier glisse, ou l'un de ses descendants : aucune zone
      // n'est possible. On le MONTRE au lieu de laisser croire que le depot va marcher.
      cibleDossier = { id: folder.id, zone: "interdit" };
      e.dataTransfer!.dropEffect = "none";
      return;
    }
    const r = (e.currentTarget as HTMLElement).getBoundingClientRect();
    const part = (e.clientY - r.top) / r.height;
    cibleDossier = { id: folder.id, zone: part < 0.25 ? "avant" : part > 0.75 ? "apres" : "dedans" };
  }

  function onFolderHeaderDragLeave() { cibleDossier = null; }

  async function onFolderHeaderDrop(e: DragEvent, folder: ProjectFolder) {
    e.preventDefault();
    e.stopPropagation();
    const zone = cibleDossier?.id === folder.id ? cibleDossier.zone : "dedans";
    const projet = dragProjectName;
    const dossier = dragFolderId;
    finGlisser();

    if (projet) {
      await deplacerProjet(projet, folder.id);
      return;
    }
    if (dossier === null) return;
    if (zone === "interdit") {
      notify($trad("sidebar.folderCycle"), "error", 4000, { report: false });
      return;
    }
    if (zone === "dedans") {
      await rangerDossier(dossier, folder.id);
      return;
    }
    await placerDossierAupres(dossier, folder, zone);
  }

  /** Reordonne un dossier dans la fratrie de `cible` (en le reparentant si besoin). */
  async function placerDossierAupres(id: number, cible: ProjectFolder, zone: "avant" | "apres") {
    const parent = parentEffectif(cible);
    const source = dossierParId(id);
    if (!source) {
      notify($trad("sidebar.folderGone"), "error", 4000, { report: false });
      return;
    }
    if (parentEffectif(source) !== parent && !(await rangerDossier(id, parent))) return;

    // Les positions sont calculees PAR FRATRIE : n'envoyer que les freres, sinon le second
    // niveau se retrouverait numerote avec la racine.
    const freres = sousDossiers(parent).filter((f) => f.id !== id).map((f) => f.id);
    const idx = freres.indexOf(cible.id) + (zone === "apres" ? 1 : 0);
    freres.splice(idx, 0, id);
    try {
      await reorderProjectFolders(freres);
    } catch (e) {
      notify(String(e), "error", 4000, { scope: "dossier.reordonnancement" });
    }
    await loadFolders();
  }

  async function deplacerProjet(name: string, folderId: number | null) {
    const proj = $projects.find((p) => p.name === name);
    if (!proj || proj.folder_id === folderId) return;
    try {
      await moveProjectToFolder(name, folderId);
    } catch (e) {
      notify(String(e), "error", 4000, { scope: "projet.deplacement" });
      return;
    }
    if (folderId !== null) deplier(folderId);
    await loadProjects();
  }

  // Zone racine : sortir un projet OU un dossier de son dossier.
  function onRootDragOver(e: DragEvent) {
    if (!dragProjectName && dragFolderId === null) return;
    e.preventDefault();
    survolRacine = true;
    cibleDossier = null;
  }

  function onRootDragLeave() { survolRacine = false; }

  async function onRootDrop(e: DragEvent) {
    e.preventDefault();
    const projet = dragProjectName;
    const dossier = dragFolderId;
    finGlisser();
    if (projet) { await deplacerProjet(projet, null); return; }
    if (dossier !== null) await rangerDossier(dossier, null);
  }

  // Colors
  const stateColors: Record<string, string> = {
    running: "var(--success)", starting: "var(--warning)", stopping: "var(--warning)",
    error: "var(--error)", stopped: "var(--text-muted)",
  };
  function getColor(state: string) { return stateColors[state] || "var(--text-muted)"; }

  function focusOnMount(node: HTMLElement) { node.focus(); }

  function onNewFolderKeydown(e: KeyboardEvent) {
    if (e.key === "Enter") void addFolder();
    if (e.key === "Escape") creation = null;
  }
</script>

<!-- Une seule definition de la ligne projet : la racine et les dossiers rendaient le meme
     balisage en double, et toute retouche devait etre faite deux fois.
     `liste` est une fonction, pas un tableau : le handler de depot la lisait au moment du
     drop, la garder paresseuse conserve exactement ce comportement. -->
{#snippet ligneProjet(proj: Project, liste: () => Project[], profondeur: number)}
  <li
    draggable="true"
    ondragstart={(e) => onProjectDragStart(e, proj.name)}
    ondragover={(e) => onProjectDragOver(e, proj.name)}
    ondragleave={onProjectDragLeave}
    ondrop={(e) => onProjectDrop(e, liste())}
    ondragend={onProjectDragEnd}
    class:drag-over-top={dropTarget?.name === proj.name && dropTarget?.pos === "before"}
    class:drag-over-bottom={dropTarget?.name === proj.name && dropTarget?.pos === "after"}
  >
    {#if renamingProjectName === proj.name}
      <div class="project-item renaming" style="padding-left: {retrait(1, profondeur)}">
        <span class="state-dot" style="background:{getColor(proj.state)}"></span>
        <InlineEdit
          value={proj.name}
          placeholder={$trad("sidebar.projectNamePlaceholder")}
          onCommit={(next) => commitRenameProject(proj.name, next)}
          onCancel={() => (renamingProjectName = null)}
        />
      </div>
    {:else}
      <button
        class="project-item"
        class:active={$selectedProject === proj.name}
        style="padding-left: {retrait(1, profondeur)}"
        onclick={() => selectProject(proj.name)}
        ondblclick={() => (renamingProjectName = proj.name)}
        oncontextmenu={(e) => openProjContextMenu(e, proj.name)}
        title={$trad("sidebar.projectHint")}
      >
        <div class="project-main">
          <span class="state-dot" style="background:{getColor(proj.state)}"></span>
          <div class="project-info">
            <span class="project-name">{proj.name}</span>
            {#if proj.description}
              <span class="project-desc">{proj.description}</span>
            {/if}
          </div>
        </div>
        <div class="project-meta">
          <span class="project-state">{proj.state}</span>
          {#if proj.containers.length > 0}
            <span class="container-count">{$tradN("sidebar.containers", proj.containers.length)}</span>
          {/if}
        </div>
      </button>
    {/if}
  </li>
{/snippet}

<!-- Champ de saisie d'un nouveau dossier, au bon niveau de retrait (racine ou sous-dossier). -->
{#snippet champNouveauDossier(profondeur: number)}
  <div class="folder-create" style="padding-left: {retrait(0.75, profondeur)}">
    <input type="text" bind:value={newFolderName} placeholder={$trad("sidebar.folderNamePlaceholder")} onkeydown={onNewFolderKeydown} use:focusOnMount />
    <button onclick={addFolder}>OK</button>
    <button class="cancel" onclick={() => (creation = null)} title={$trad("common.cancel")}>×</button>
  </div>
{/snippet}

<!-- Un dossier et TOUT ce qu'il contient. Le rendu est recursif (profondeur illimitee,
     issue #2) mais le retrait est calcule depuis `profondeur` : les <ul> imbriques n'ont
     aucun padding propre, sinon l'indentation s'additionnerait a chaque niveau. -->
{#snippet ligneDossier(folder: ProjectFolder, profondeur: number)}
  {@const replie = collapsedIds.has(folder.id)}
  {@const cible = cibleDossier?.id === folder.id ? cibleDossier.zone : null}
  {@const enfantsDossiers = sousDossiers(folder.id)}
  {@const projetsDedans = getFolderProjects(folder.id)}
  {@const bloquant = raisonNonVide(folder.id)}
  <li
    class="folder-item"
    draggable="true"
    ondragstart={(e) => onFolderDragStart(e, folder)}
    ondragend={finGlisser}
  >
    <div
      class="folder-header"
      class:drop-avant={cible === "avant"}
      class:drop-dedans={cible === "dedans"}
      class:drop-apres={cible === "apres"}
      class:drop-interdit={cible === "interdit"}
      style="padding-left: {retrait(0.5, profondeur)}"
      data-profondeur={profondeur}
      role="toolbar"
      tabindex="-1"
      oncontextmenu={(e) => openContextMenu(e, folder.id)}
      ondragover={(e) => onFolderHeaderDragOver(e, folder)}
      ondragleave={onFolderHeaderDragLeave}
      ondrop={(e) => onFolderHeaderDrop(e, folder)}
    >
      {#if renamingFolderId === folder.id}
        <span class="folder-caret">{replie ? '▸' : '▾'}</span>
        <InlineEdit
          value={folder.name}
          onCommit={(next) => commitRenameFolder(folder.id, next)}
          onCancel={() => (renamingFolderId = null)}
        />
      {:else}
        <!-- Un vrai <button> : c'est lui qui porte le clic, le focus clavier et l'infobulle
             qui ANNONCE les gestes (sans quoi personne ne les trouve). -->
        <button
          class="folder-main"
          onclick={() => toggleFolder(folder.id)}
          ondblclick={() => (renamingFolderId = folder.id)}
          title={$trad("sidebar.folderHint")}
        >
          <span class="folder-caret">{replie ? '▸' : '▾'}</span>
          <span class="folder-name">{folder.name}</span>
        </button>
      {/if}
      <span class="folder-count" title={$tradN("sidebar.folderCount", compteProjets(folder.id))}>
        {compteProjets(folder.id)}
      </span>
      <button
        class="folder-add"
        title={$trad("sidebar.newSubfolderHint")}
        onclick={() => demarrerCreation(folder.id)}
      >+▸</button>
      <button
        class="folder-delete"
        title={bloquant ? $trad("sidebar.folderNotEmptyHint") : $trad("sidebar.deleteFolderHint")}
        onclick={() => deleteFolder(folder.id)}
      >🗑</button>
    </div>
    {#if !replie}
      <ul class="folder-content">
        {#if creation && creation.parentId === folder.id}
          {@render champNouveauDossier(profondeur + 1)}
        {/if}
        {#each enfantsDossiers as sous (sous.id)}
          {@render ligneDossier(sous, profondeur + 1)}
        {/each}
        {#each projetsDedans as proj}
          {@render ligneProjet(proj, () => getFolderProjects(folder.id), profondeur + 1)}
        {/each}
        {#if enfantsDossiers.length === 0 && projetsDedans.length === 0 && creation?.parentId !== folder.id}
          <!-- Un dossier vide dit quoi en faire au lieu de rester un trou. -->
          <li class="folder-empty" style="padding-left: {retrait(1, profondeur + 1)}">
            {$trad("sidebar.folderEmpty")}
          </li>
        {/if}
      </ul>
    {/if}
  </li>
{/snippet}

<aside>
  {#if $terminals.length > 0}
    <div class="sidebar-header terminals-header">
      <button class="section-toggle" onclick={toggleTerminals}>
        {terminalsCollapsed ? '▸' : '▾'} {$trad("sidebar.terminals")}
      </button>
      <span class="terminals-count">{$terminals.length}</span>
    </div>
    {#if !terminalsCollapsed}
      <ul class="terminals-list">
        {#each $terminals as t (t.id)}
          <li>
            {#if renamingTermId === t.id}
              <div class="terminal-item">
                {#if t.llm}<img class="term-llm" src={claudeLogo} alt="Claude" title={$trad("sidebar.claudeRunning")} />{:else}<span class="term-dot" title={$trad("sidebar.terminal")}></span>{/if}
                <InlineEdit
                  value={t.name}
                  placeholder={$trad("sidebar.terminalNamePlaceholder")}
                  onCommit={(next) => commitRenameTerminal(t.id, next)}
                  onCancel={() => (renamingTermId = null)}
                />
              </div>
            {:else}
              <button
                class="terminal-item"
                onclick={() => gotoTerminal(t)}
                oncontextmenu={(e) => openTermContextMenu(e, t)}
                title={$trad("sidebar.gotoTerminal", { project: t.project })}
              >
                {#if t.llm}<img class="term-llm" src={claudeLogo} alt="Claude" title={$trad("sidebar.claudeRunning")} />{:else}<span class="term-dot" title={$trad("sidebar.terminal")}></span>{/if}
                <span class="terminal-name">{terminalLabel(t)}</span>
                <span class="terminal-project">{t.project}</span>
              </button>
            {/if}
          </li>
        {/each}
      </ul>
    {/if}
  {/if}

  <div class="sidebar-header">
    <span>{$trad("sidebar.projects")}</span>
    <div class="header-actions">
      <button class="add-btn labeled" onclick={() => showCreateModal = true} title={$trad("sidebar.newProjectHint")}>{$trad("sidebar.newProject")}</button>
      <button class="add-btn labeled" onclick={() => demarrerCreation(null)} title={$trad("sidebar.newFolderHint")}>{$trad("sidebar.newFolder")}</button>
    </div>
  </div>

  {#if creation && creation.parentId === null}
    {@render champNouveauDossier(0)}
  {/if}

  <ul>
    {#each sousDossiers(null) as folder (folder.id)}
      {@render ligneDossier(folder, 0)}
    {/each}

    <!-- Projets sans dossier. La zone accepte aussi le depot d'un DOSSIER : c'est le geste
         « sortir de son dossier » au glisser. -->
    <div
      class="root-drop-zone"
      class:drop-root={survolRacine}
      role="list"
      ondragover={onRootDragOver}
      ondragleave={onRootDragLeave}
      ondrop={onRootDrop}
    >
      {#if dragFolderId !== null}
        <p class="drop-hint">{$trad("sidebar.dropToRoot")}</p>
      {/if}
      {#each rootProjects as proj}
        {@render ligneProjet(proj, () => rootProjects, 0)}
      {/each}
    </div>

    {#if $projects.length === 0}
      <li class="empty">{$trad("sidebar.noProject")}</li>
    {/if}
  </ul>
  <CreateProjectModal bind:open={showCreateModal} />
</aside>

{#if termContextMenu}
  {@const tid = termContextMenu.id}
  <ContextMenu
    x={termContextMenu.x}
    y={termContextMenu.y}
    onClose={() => (termContextMenu = null)}
    items={[
      { label: $trad("common.rename"), action: () => (renamingTermId = tid) },
      { label: $trad("sidebar.closeTerminal"), danger: true, action: () => closeTerminalById(tid) },
    ]}
  />
{/if}

{#if projContextMenu}
  {@const pname = projContextMenu.name}
  <ContextMenu
    x={projContextMenu.x}
    y={projContextMenu.y}
    onClose={() => (projContextMenu = null)}
    items={[
      { label: $trad("common.rename"), action: () => (renamingProjectName = pname) },
    ]}
  />
{/if}

{#if contextMenu}
  {@const fid = contextMenu.id}
  {@const dansUnDossier = (dossierParId(fid)?.parent_id ?? null) !== null}
  <ContextMenu
    x={contextMenu.x}
    y={contextMenu.y}
    onClose={() => (contextMenu = null)}
    items={[
      { label: $trad("sidebar.newSubfolder"), action: () => demarrerCreation(fid) },
      { label: $trad("common.rename"), action: () => (renamingFolderId = fid) },
      // « Sortir du dossier » n'a de sens que pour un dossier imbrique : c'est le pendant
      // clavier/souris du glisser vers la zone racine.
      ...(dansUnDossier
        ? [{ label: $trad("sidebar.folderToRoot"), action: () => void rangerDossier(fid, null) }]
        : []),
      { label: $trad("sidebar.deleteFolder"), danger: true, action: () => deleteFolder(fid) },
    ]}
  />
{/if}

<style>
  aside {
    width: var(--sidebar-width); min-width: var(--sidebar-width);
    background: var(--bg-secondary); border-right: 1px solid var(--border-color);
    overflow-y: auto; display: flex; flex-direction: column;
  }
  .sidebar-header {
    padding: 0.75rem 1rem; font-weight: 600; font-size: 0.85rem;
    color: var(--text-muted); text-transform: uppercase; letter-spacing: 0.05em;
    display: flex; align-items: center; justify-content: space-between;
  }
  .header-actions { display: flex; gap: 0.25rem; }

  /* Section Terminaux (raccourcis) */
  .terminals-header { padding-bottom: 0.25rem; border-bottom: 1px solid var(--border-color); }
  .section-toggle {
    background: none; border: none; cursor: pointer; padding: 0;
    color: var(--text-muted); font-weight: 600; font-size: 0.85rem;
    text-transform: uppercase; letter-spacing: 0.05em;
  }
  .section-toggle:hover { color: var(--text-primary); }
  .terminals-count {
    font-size: 0.7rem; background: var(--bg-tertiary); color: var(--text-secondary);
    padding: 0.05rem 0.45rem; border-radius: 10px;
  }
  .terminals-list { border-bottom: 1px solid var(--border-color); }
  .terminal-item {
    display: flex; align-items: center; gap: 0.5rem;
    width: 100%; padding: 0.35rem 1rem; border: none; background: none;
    color: var(--text-secondary); cursor: pointer; text-align: left; font-size: 0.82rem;
  }
  .terminal-item:hover { background: var(--bg-tertiary); color: var(--text-primary); }
  /* Gris = terminal normal, vert = un agent LLM (claude, codex...) tourne dedans */
  .term-dot {
    width: 7px; height: 7px; border-radius: 50%; flex-shrink: 0;
    background: var(--text-muted);
  }
  /* Icone Claude (✳) quand un agent IA tourne — plus parlant qu une pastille verte */
  .term-llm { width: 11px; height: 11px; flex-shrink: 0; }
  .terminal-name {
    font-weight: 600; overflow: hidden; text-overflow: ellipsis; white-space: nowrap;
    flex-shrink: 1; min-width: 0;
  }
  .terminal-project {
    margin-left: auto; flex-shrink: 0; font-size: 0.7rem; color: var(--text-muted);
    max-width: 45%; overflow: hidden; text-overflow: ellipsis; white-space: nowrap;
  }
  .add-btn {
    width: 22px; height: 22px; border-radius: 4px; border: 1px solid var(--border-color);
    background: var(--bg-tertiary); color: var(--text-secondary); cursor: pointer;
    font-size: 0.75rem; line-height: 1; display: flex; align-items: center; justify-content: center;
    padding: 0;
  }
  .add-btn:hover { background: var(--accent); color: white; border-color: var(--accent); }
  /* Libelles explicites : le 📁 seul n etait pas compris (retour utilisateur 2026-08-14) */
  .add-btn.labeled { width: auto; padding: 0 0.5rem; font-size: 0.72rem; font-weight: 600; }

  .folder-create {
    display: flex; gap: 0.25rem; padding: 0.25rem 0.75rem; align-items: center;
  }
  .folder-create input {
    flex: 1; padding: 0.2rem 0.4rem; border: 1px solid var(--border-color);
    border-radius: 4px; background: var(--bg-primary); color: var(--text-primary); font-size: 0.8rem;
  }
  .folder-create button {
    padding: 0.2rem 0.4rem; border: 1px solid var(--border-color); border-radius: 4px;
    background: var(--bg-tertiary); color: var(--text-secondary); cursor: pointer; font-size: 0.75rem;
  }
  .folder-create .cancel { color: var(--error); }

  ul { list-style: none; padding: 0; margin: 0; }

  .folder-item { border-bottom: 1px solid var(--border-color); }
  .folder-header {
    display: flex; align-items: center; gap: 0.25rem;
    padding: 0.4rem 0.5rem; color: var(--text-secondary);
    /* Au moins aussi grand que le nom d'un projet (.project-name, 0.9rem) : un dossier
       CONTIENT des projets, il ne doit pas s'ecrire plus petit qu'eux. Il etait a 0.8rem,
       donc le contenant paraissait moins important que son contenu (issue #16). */
    font-size: 0.9rem;
    background: var(--bg-tertiary);
    /* Les traits de depot se posent ici : reserver la place evite que la ligne saute. */
    border-top: 2px solid transparent; border-bottom: 2px solid transparent;
  }
  /* Le retrait est calcule par niveau sur l'en-tete lui-meme : les listes imbriquees ne
     doivent RIEN ajouter, sinon l'indentation double a chaque profondeur. */
  .folder-content { padding-left: 0; }
  /* Toute la ligne est cliquable (replier/deplier) : cible large, et un vrai <button>. */
  .folder-main {
    flex: 1; min-width: 0; display: flex; align-items: center; gap: 0.25rem;
    background: none; border: none; padding: 0; cursor: pointer;
    color: inherit; font: inherit; text-align: left;
  }
  .folder-main:hover .folder-name { color: var(--text-primary); }
  .folder-caret { color: var(--text-muted); font-size: 0.75rem; width: 16px; flex-shrink: 0; text-align: center; }
  .folder-name { flex: 1; font-weight: 600; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .folder-count { font-size: 0.7rem; color: var(--text-muted); }
  .folder-empty { padding: 0.35rem 1rem; font-size: 0.72rem; color: var(--text-muted); font-style: italic; }
  /* Meme discretion que la corbeille : visible au survol de l'en-tete ou au focus clavier. */
  .folder-add {
    background: none; border: none; cursor: pointer; padding: 0 0.2rem;
    font-size: 0.72rem; color: var(--text-muted); line-height: 1;
    opacity: 0; transition: opacity 0.1s ease;
  }
  .folder-header:hover .folder-add, .folder-add:focus-visible { opacity: 1; }
  .folder-add:hover { color: var(--accent); }

  /* Retours de depot d'un DOSSIER : le trait dit « a cote », le cadre dit « dedans ».
     Sans les deux, on ne sait pas ce que le lacher va faire. */
  .folder-header.drop-avant { border-top-color: var(--accent); }
  .folder-header.drop-apres { border-bottom-color: var(--accent); }
  .folder-header.drop-dedans {
    outline: 2px solid var(--accent); outline-offset: -2px;
    background: color-mix(in srgb, var(--accent) 18%, var(--bg-tertiary));
  }
  .folder-header.drop-interdit {
    outline: 2px dashed var(--error); outline-offset: -2px; cursor: not-allowed;
  }
  .root-drop-zone.drop-root {
    outline: 2px dashed var(--accent); outline-offset: -2px;
    background: color-mix(in srgb, var(--accent) 10%, transparent);
  }
  .drop-hint {
    margin: 0; padding: 0.4rem 1rem; font-size: 0.72rem; color: var(--accent);
    text-align: center;
  }
  /* Corbeille discrete : visible au survol de l'en-tete (et au focus clavier).
     Le clic sur un dossier plein EXPLIQUE au lieu de supprimer — le bouton reste donc
     toujours cliquable, pas de disabled muet. */
  .folder-delete {
    background: none; border: none; cursor: pointer; padding: 0 0.2rem;
    font-size: 0.75rem; color: var(--text-muted); line-height: 1;
    opacity: 0; transition: opacity 0.1s ease;
  }
  .folder-header:hover .folder-delete, .folder-delete:focus-visible { opacity: 1; }
  .folder-delete:hover { color: var(--error); }

  .root-drop-zone { min-height: 2rem; }

  .project-item {
    display: flex; flex-direction: column; gap: 0.2rem;
    width: 100%; padding: 0.6rem 1rem; border: none; background: none;
    color: var(--text-primary); cursor: pointer; text-align: left;
    border-bottom: 1px solid var(--border-color);
  }
  .project-item:hover { background: var(--bg-tertiary); }
  /* Meme boite pendant la saisie : la ligne ne saute pas quand on entre en renommage. */
  .project-item.renaming { flex-direction: row; align-items: center; gap: 0.5rem; cursor: default; }
  .project-item.active { background: var(--bg-tertiary); border-left: 3px solid var(--accent); }
  .project-main { display: flex; align-items: flex-start; gap: 0.5rem; }
  .state-dot { width: 8px; height: 8px; border-radius: 50%; flex-shrink: 0; margin-top: 0.3rem; }
  .project-info { display: flex; flex-direction: column; flex: 1; min-width: 0; }
  .project-name { font-size: 0.9rem; font-weight: 600; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .project-desc {
    font-size: 0.75rem; color: var(--text-muted); overflow: hidden;
    text-overflow: ellipsis; white-space: nowrap; margin-top: 0.1rem;
  }
  .project-meta { display: flex; align-items: center; gap: 0.5rem; padding-left: 1.1rem; }
  .project-state { font-size: 0.7rem; color: var(--text-muted); }
  .container-count { font-size: 0.7rem; color: var(--text-secondary); }
  .empty { padding: 1rem; color: var(--text-muted); font-size: 0.85rem; }

  li[draggable="true"] { cursor: grab; border-top: 2px solid transparent; border-bottom: 2px solid transparent; transition: border-color 0.1s; }
  li[draggable="true"]:active { cursor: grabbing; }
  li.drag-over-top { border-top-color: var(--accent); }
  li.drag-over-bottom { border-bottom-color: var(--accent); }
</style>
